//! Implements the Actors which are interactive game elements like treasure chests
//! and monster bosses. Actors are little state machines which are written like
//! ```
//! 000=CREATESELF 87 // speaker, blocker
//! 000=SETINTEL ID=777 MESSAGEBEARER
//! 000=SETTEAM ID=777 NPC //  Sewer Gate
//! 005=SETMESSAGE ID=15 // This door is locked and requires a key., Ok., ,
//! 005=WAIT RESPONSE 10,0,0  // if A:goto 10 if B:goto 0 if C:goto 0
//! 010=IF PLAYERHAS 403 NEWSTATE=60 // check player has key, Sewers
//! 020=SETSTATE 777 5 // actor state, actor =Sewer Gate
//! 060=TELLMESSAGE ID=18 // You use the correct key., Ok., ,
//! 060=WAIT RESPONSE 70,0,0  // if A:goto 70 if B:goto 0 if C:goto 0
//! 070=TAKEITEM ID=0 403  // Player looses key, Sewers
//! 070=REMOVE 777 // Sewer Gate
//! ```
//! and are compiled to byte code. The byte code is run by Actor::act which
//! returns an async Future.

use crate::{
    body,
    cast::{
        yield_now, Cond, Response, SharedGameState, WaitFuture, WaitFutureResult,
        WaitResponseFuture,
    },
    data::PropTypeRes,
    data::{ActorRes, PROPS},
    game::{Dialog, InvalidDataError, TransactionType},
    js,
    stage::{PetKind, Stage},
    thrift::save::{self, IntelType, Team},
};
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use std::{
    cell::{Cell, RefCell},
    cmp::min,
    collections::{HashMap, VecDeque},
    error::Error,
    fmt,
    rc::Rc,
    slice::Iter,
};
use thrift::OrderedFloat;

/// A state machine that implements interactive game elements
pub(crate) struct Actor {
    pub(crate) res: &'static ActorRes,
    pub(crate) resume_state: Option<u16>,
    pub(crate) dead: Cell<bool>,

    game_state: Rc<RefCell<SharedGameState>>,
    dialog: Rc<dyn Dialog>,
    stage: Rc<Stage>,
    compiled: CompiledActions,
    stack: RefCell<VecDeque<u16>>,
    initialized: Cell<bool>,

    // Location when the map is first loaded. Not updated as it moves
    x: f64,
    y: f64,
}

impl Actor {
    pub fn new(
        game_state: Rc<RefCell<SharedGameState>>,
        stage: Rc<Stage>,
        res: &'static ActorRes,
        dialog: Rc<dyn Dialog>,
        resume_state: Option<u16>,
    ) -> Self {
        let compiled = CompiledActions::new(res.actions.as_ref().unwrap());
        Self {
            game_state,
            res,
            stage,
            dialog,
            compiled,
            stack: RefCell::new(VecDeque::new()),
            initialized: Cell::new(false),
            resume_state,
            dead: Cell::new(false),
            x: res.x,
            y: res.y,
        }
    }

    pub fn from_save(
        game_state: Rc<RefCell<SharedGameState>>,
        stage: Rc<Stage>,
        res: &'static ActorRes,
        dialog: Rc<dyn Dialog>,
        save: &save::Actor,
    ) -> Result<Self, InvalidDataError> {
        let compiled = CompiledActions::new(res.actions.as_ref().unwrap());
        let dead = save.dead.unwrap_or(false);
        let x = save.x.ok_or(InvalidDataError::new("x field missing"))?;
        let y = save.y.ok_or(InvalidDataError::new("y field missing"))?;

        let result = Self {
            game_state,
            res,
            stage,
            dialog,
            compiled,
            stack: RefCell::new(VecDeque::new()),
            initialized: Cell::new(false),
            resume_state: None,
            dead: Cell::new(dead),
            x: *x,
            y: *y,
        };
        Ok(result)
    }

    pub(crate) fn save(&self) -> save::Actor {
        let maybe_body = self.stage.get_body(self.res.id);

        let (x, y) = if let Some(body) = maybe_body {
            body.moving_to()
        } else {
            (self.x, self.y)
        };
        js::log(&format!(
            "for actor {}, dead={}, died={}",
            self.res.id,
            self.dead.get(),
            self.stage.died(self.res.id)
        ));
        save::Actor::new(
            self.compiled.state() as i32,
            OrderedFloat::from(x),
            OrderedFloat::from(y),
            self.dead.get() || self.stage.died(self.res.id),
        )
    }

    /// When the actor is run, first run state 0 then
    /// jump to state. This behavior is for loading actors
    /// on a new map.
    pub fn resume_after_init(&mut self, state: u16) {
        self.initialized.set(false);
        self.resume_state = Some(state);
    }

    /// When the actor is run, start right from state. This
    /// behavior is for loading actors from a save
    pub fn resume(&self, state: u16) {
        self.compiled.set_state(state);
    }

    fn now(&self) -> f64 {
        self.game_state.borrow().now
    }

    pub fn id(&self) -> u16 {
        self.res.id
    }

    /// Runs the actor state machine
    pub async fn act(&self) {
        if self.dead.get() {
            js::log(&format!("not running {} because dead", self.res.name));
            return;
        }
        loop {
            let code = self.compiled.next();
            match code {
                None => {
                    break;
                }
                Some(PUSH) => {
                    let size = self.compiled.next().unwrap();
                    for _ in 0..size {
                        self.stack
                            .borrow_mut()
                            .push_front(self.compiled.next().unwrap());
                    }
                }
                Some(CALL) => {
                    let action_id = self.compiled.next().unwrap();
                    let result = self.execute_action(action_id).await;
                    if let Err(e) = result {
                        js::log(&format!("{}: Actor error: {}", self.res.name, e));
                        break;
                    }
                }
                Some(c) => panic!("Unrecognized code {}", c),
            }
            if !self.initialized.get() && self.compiled.state() != 0 {
                self.initialized.set(true);
                if let Some(state) = self.resume_state {
                    // don't resume to state 0, we just ran that
                    if state != 0 {
                        self.compiled.set_state(state);
                        js::log(&format!("{}: jumped to state {}", self.res.name, state));
                    }
                }
                // give every actor a chance to complete initialization
                // before continuing
                yield_now().await;
            }
        }

        // If the actor finishes, but has a body the player could kill
        // don't set the actor to dead, it needs to reappear. Otherwise
        // set the actor dead so it never runs again
        let actor_body = self.stage.get_body(self.res.id);
        if actor_body.is_none() || matches!(actor_body.unwrap().prop_id, 64 | 87) {
            self.dead.set(true);
            js::log(&format!("actor {} has died", self.res.id));
        }
    }

    async fn execute_action(&self, action_id: u16) -> Result<(), ActorError> {
        match action_id {
            0x0006 => self.attack().await?,
            0x010a => self.set_intel().await?,
            0x0206 => self.create_self().await,
            0x0308 => self.give_item().await?,
            0x0408 => self.set_team().await?,
            0x050e => self.wait().await,
            0x060e => self.eval_if().await,
            0x0808 => self.set_state().await,
            0x0a06 => self.set_message().await?,
            0x0b06 => self.tell_message().await?,
            0x0c06 => self.add_var().await,
            0x0d06 => self.sub_var().await,
            0x0e06 => self.set_var().await,
            0x0f08 => self.move_actor().await?,
            0x1008 => self.drop_item().await?,
            0x1108 => self.take_item().await?,
            0x1208 => self.set_loc().await?,
            0x1306 => self.add_sell_item().await?,
            0x1406 => self.execute_trade().await?,
            0x1506 => self.remove().await,
            0x1606 => self.freeze().await?,
            0x1706 => self.unfreeze().await?,
            0x1808 => self.give_gold().await?,
            0x1908 => self.give_exp().await?,
            0x1a08 => self.give_henchmen().await?,
            0x1b08 => self.set_health().await?,
            0x1c08 => self.spawn().await,
            0x1d08 => self.set_portrait().await?,
            0x1e0a => self.patrol().await?,
            0x1f0a => self.wander().await?,
            0x2008 => self.set_level().await?,
            0x2106 => self.add_quest_log().await,
            0x2206 => self.remove_quest_log().await,
            0x2308 => self.take_gold().await?,
            0x2608 => self.give_quest_pet().await?,
            0x2708 => self.take_pet().await?,
            0x2808 => self.map_set_loc().await?,
            _ => panic!("Unrecognized action {:X}", action_id),
        }
        Ok(())
    }

    async fn add_quest_log(&self) {
        let msg_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: add_quest_log({})", self.res.name, msg_id));

        self.game_state.borrow_mut().quest_log.insert(msg_id);
    }

    async fn add_sell_item(&self) -> Result<(), ActorError> {
        let prop_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: add_sell_item({})", self.res.name, prop_id));

        let body = self
            .stage
            .get_body(self.res.id)
            .ok_or(ActorError::BodyNotFound)?;

        let intel = body.intel.borrow();
        intel
            .as_ref()
            .ok_or(ActorError::IntelNotFound)?
            .add_sell_item(prop_id)
    }

    async fn add_var(&self) {
        let param_a = self.stack.borrow_mut().pop_front().unwrap();
        let var = (param_a >> 8) as usize;
        let val = (param_a & 0xff) as u8;

        js::log(&format!("{}: add_var({}, {})", self.res.name, var, val));

        let mut game_state = self.game_state.borrow_mut();
        if game_state.vars[var] <= 255 - val {
            game_state.vars[var] += val;
        } else {
            game_state.vars[var] = 255;
        }
    }

    async fn attack(&self) -> Result<(), ActorError> {
        let actor_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: attack({})", self.res.name, actor_id));

        let attackee = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        self.stage
            .get_body(self.res.id)
            .ok_or(ActorError::BodyNotFound)?
            .attack(attackee);
        Ok(())
    }

    async fn create_self(&self) {
        let prop_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: create_self({})", self.res.name, prop_id));

        let body = self.stage.get_body(self.res.id);
        if body.is_some() {
            // TODO: before I had this returning, what else breaks?
            self.stage.remove_body(self.res.id);
        }
        let prop = &PROPS[&prop_id.to_string()];

        // If its a creature/user with a special name, use that. Otherwise
        // use the item name
        let name = match prop.kind {
            PropTypeRes::Creature { .. } | PropTypeRes::User { .. } => self.res.name.clone(),

            _ => prop.name.to_string(),
        };

        let body = self
            .stage
            .create_body(name, Some(self.res.id), prop_id, self.x, self.y);

        body.equip_default(self.now());
        body.set_portrait(self.res.bmp_offset);
    }

    async fn drop_item(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let prop_id = stack.pop_front().unwrap() as u16;
        let actor_id = stack.pop_front().unwrap() as u16;
        js::log(&format!(
            "{}: drop_item({}, {})",
            self.res.name, actor_id, prop_id
        ));

        let body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        let name = &PROPS[&prop_id.to_string()].name;
        let (x, y) = body.moving_from();
        self.stage
            .create_body(String::from(name), None, prop_id, x, y);
        Ok(())
    }

    async fn eval_if(&self) {
        let mut stack = self.stack.borrow_mut();
        let new_state = stack.pop_front().unwrap();
        let cond_type = stack.pop_front().unwrap() >> 8;
        let param_c = stack.pop_front().unwrap();
        let param_b = stack.pop_front().unwrap();
        let param_a = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: eval_if({}, {}, {}, {}, {})",
            self.res.name, cond_type, param_a, param_b, param_c, new_state
        ));

        let cond = Cond::new(cond_type, param_a, param_b);
        let game_state = &mut self.game_state.borrow_mut();
        if cond.satisfied(&self.stage, game_state) {
            // don't allow resume_state to take affect
            self.initialized.set(true);
            self.compiled.set_state(new_state);
        }
    }

    async fn execute_trade(&self) -> Result<(), ActorError> {
        let trade_type = self.stack.borrow_mut().pop_front().unwrap() >> 8;
        js::log(&format!("{}: execute_trade({})", self.res.name, trade_type));

        match trade_type {
            0 => {
                let body = self
                    .stage
                    .get_body(self.res.id)
                    .ok_or(ActorError::BodyNotFound)?;

                let intel = body.intel.borrow();
                let intel = intel.as_ref().ok_or(ActorError::IntelNotFound)?;

                let items = intel.pop_transaction()?;
                self.dialog
                    .buy_sell(self.stage.get_player(), items, TransactionType::Buy);
            }
            1 => {
                let items = self.stage.get_player().inventory.borrow().clone();
                self.dialog
                    .buy_sell(self.stage.get_player(), items, TransactionType::Sell);
            }
            kind => panic!("Unrecognized trade type {}", kind),
        }
        Ok(())
    }

    async fn freeze(&self) -> Result<(), ActorError> {
        let actor_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: freeze({})", self.res.name, actor_id));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .freeze();
        Ok(())
    }

    async fn give_exp(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let exp = stack.pop_front().unwrap() as i32;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: give_exp({}, {})",
            self.res.name, actor_id, exp
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .give_exp(exp);

        Ok(())
    }

    async fn give_gold(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let gold = stack.pop_front().unwrap() as i32;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: give_gold({}, {})",
            self.res.name, actor_id, gold
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .give_gold(gold);
        Ok(())
    }

    async fn give_henchmen(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let prop_id = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: give_henchmen({}, {})",
            self.res.name, actor_id, prop_id
        ));

        let reciever_body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        let actor_id = Some(self.res.id + 1000);
        let name = body::pet_name();
        self.stage
            .create_pet(name, prop_id, reciever_body, PetKind::Normal, actor_id);

        Ok(())
    }

    async fn give_item(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let prop_id = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: give_item({}, {})",
            self.res.name, actor_id, prop_id
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .give_item(prop_id);

        Ok(())
    }

    async fn give_quest_pet(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let prop_id = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: give_henchmen({}, {})",
            self.res.name, actor_id, prop_id
        ));

        let reciever_body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        let actor_id = Some(self.res.id + 1000);
        self.stage.create_pet(
            &self.res.name,
            prop_id,
            reciever_body,
            PetKind::Quest,
            actor_id,
        );
        Ok(())
    }

    async fn map_set_loc(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let pos = stack.pop_front().unwrap();
        let map_id = stack.pop_front().unwrap();
        let x = (pos >> 8) as f64;
        let y = (pos & 0xff) as f64;
        js::log(&format!(
            "{}: map_set_loc({}, {}, {})",
            self.res.name, map_id, x, y
        ));

        if self.stage.map_id() == map_id {
            js::log(&format!("{}: just setting the body", self.res.name));
            self.stage.set_actor_body_loc(0, x, y);
        } else {
            js::log(&format!("{}: changing the map", self.res.name));
            let mut game_state = self.game_state.borrow_mut();
            game_state.map_change_request = Some((map_id, x, y));
        }
        Ok(())
    }

    async fn move_actor(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let actor_id = stack.pop_front().unwrap();
        let pos = stack.pop_front().unwrap();
        let x = (pos >> 8) as f64;
        let y = (pos & 0xff) as f64;
        js::log(&format!(
            "{}: move_actor({}, {}, {})",
            self.res.name, actor_id, x, y
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .walk_to(x, y);
        Ok(())
    }

    async fn patrol(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let pos2 = stack.pop_front().unwrap();
        let pos1 = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        let x1 = (pos1 >> 8) as f64;
        let y1 = (pos1 & 0xff) as f64;
        let x2 = (pos2 >> 8) as f64;
        let y2 = (pos2 & 0xff) as f64;
        js::log(&format!(
            "{}: patrol({}, {}, {}, {}, {})",
            self.res.name, actor_id, x1, y1, x2, y2
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .patrol(x1, y1, x2, y2);
        Ok(())
    }

    async fn remove_quest_log(&self) {
        let msg_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: remove({})", self.res.name, msg_id));

        self.game_state.borrow_mut().quest_log.remove(&msg_id);
    }

    async fn remove(&self) {
        let actor_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: remove({})", self.res.name, actor_id,));

        self.stage.remove_body(actor_id);
    }

    async fn set_health(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        // TODO: this needs to support -1, does it?
        let health = stack.pop_front().unwrap() as i32;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_health({}, {})",
            self.res.name, actor_id, health
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .set_health_no_max(health);
        Ok(())
    }

    async fn set_intel(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        // This function is passed some extra junk it doesn't use.
        let _junk = stack.pop_front().unwrap();
        let param = stack.pop_front().unwrap();
        let intel_type_u8 = (param >> 8) as u8;
        let hostile_to = (param & 0xff) as u8;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_intel({}, {}, {})",
            self.res.name, actor_id, intel_type_u8, hostile_to
        ));

        let body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        let intel_type: IntelType = intel_type_u8
            .try_into()
            .map_err(|_| ActorError::TypeConversion)
            .unwrap();

        body.set_intel(intel_type);

        if hostile_to > 0 {
            let team: Team = hostile_to
                .try_into()
                .map_err(|_| ActorError::TypeConversion)
                .unwrap();

            body.set_enemy(team);
        }
        Ok(())
    }

    async fn set_level(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let level = (stack.pop_front().unwrap() >> 8) as i32;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_level({}, {})",
            self.res.name, actor_id, level
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .set_level(level);
        Ok(())
    }

    async fn set_loc(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let actor_id = stack.pop_front().unwrap();
        let pos = stack.pop_front().unwrap();
        let x = (pos >> 8) as f64;
        let y = (pos & 0xff) as f64;
        js::log(&format!(
            "{}: set_loc({}, {}, {})",
            self.res.name, actor_id, x, y
        ));

        self.stage.set_actor_body_loc(actor_id, x, y);
        Ok(())
    }

    async fn set_message(&self) -> Result<(), ActorError> {
        let msg_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: set_message({})", self.res.name, msg_id));

        let body = self
            .stage
            .get_body(self.res.id)
            .ok_or(ActorError::BodyNotFound)?;

        let intel = body.intel.borrow();
        intel
            .as_ref()
            .ok_or(ActorError::IntelNotFound)?
            .set_message(msg_id)?;

        Ok(())
    }

    async fn set_portrait(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let portrait_id = stack.pop_front().unwrap() as u16;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_portrait({}, {})",
            self.res.name, actor_id, portrait_id
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .set_portrait(portrait_id);
        Ok(())
    }

    async fn set_state(&self) {
        let mut stack = self.stack.borrow_mut();
        let state = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_state({}, {})",
            self.res.name, actor_id, state
        ));

        if actor_id == self.res.id {
            // don't allow the resume_state to take affect
            self.initialized.set(true);
            self.compiled.set_state(state);

            // some actors will loop forever if we don't yield first when jumping states
            yield_now().await;
        } else {
            self.game_state
                .borrow_mut()
                .state_interrupt_by_actor
                .insert(actor_id, state);
        }
    }

    async fn set_team(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let team_u8 = (stack.pop_front().unwrap() >> 8) as u8;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: set_team({}, {})",
            self.res.name, actor_id, team_u8
        ));

        let body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        let team: Team = team_u8
            .try_into()
            .map_err(|_| ActorError::TypeConversion)
            .unwrap();

        body.set_team(team);
        Ok(())
    }

    async fn set_var(&self) {
        let param_a = self.stack.borrow_mut().pop_front().unwrap();
        let var = (param_a >> 8) as usize;
        let val = (param_a & 0xff) as u8;
        js::log(&format!("{}: set_var({}, {})", self.res.name, var, val));

        self.game_state.borrow_mut().vars[var] = val;
    }

    async fn spawn(&self) {
        let mut stack = self.stack.borrow_mut();
        let position = stack.pop_front().unwrap();
        let x = (position >> 8) as f64;
        let y = (position & 0xff) as f64;
        let prop_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: spawn({}, {}, {})",
            self.res.name, prop_id, x, y
        ));
        let spawn_name = &PROPS[&prop_id.to_string()].name;

        let body = self.stage.create_body(
            spawn_name.to_string(),
            Some(1000 + self.res.id),
            prop_id,
            x,
            y,
        );
        body.equip_default(self.now());

        // Spawns called outside of the initial state need to be persisted
        // when saving
        if self.compiled.state() != 0 {
            body.persist();
        }
    }

    async fn sub_var(&self) {
        let param_a = self.stack.borrow_mut().pop_front().unwrap();
        let var = (param_a >> 8) as usize;
        let val = (param_a & 0xff) as u8;
        js::log(&format!("{}: sub_var({}, {})", self.res.name, var, val));

        let mut game_state = self.game_state.borrow_mut();
        if game_state.vars[var] > val {
            game_state.vars[var] -= val;
        } else {
            game_state.vars[var] = 0;
        }
    }

    async fn take_gold(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let gold = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: take_gold({}, {})",
            self.res.name, actor_id, gold
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .take_gold(gold as i32);
        Ok(())
    }

    async fn take_item(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let prop_id = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: take_item({}, {})",
            self.res.name, actor_id, prop_id
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .take_item(prop_id);
        Ok(())
    }

    async fn take_pet(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let pet_type = stack.pop_front().unwrap() >> 8;
        let actor_id = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: take_pet({}, {})",
            self.res.name, actor_id, pet_type
        ));

        let body = self
            .stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?;

        // TODO: use PetKind here
        let pet = match pet_type {
            0 => body.quest_pet.take(),
            1 => body.summoned_pet.take(),
            2 => body.pet.take(),
            _ => panic!("Unrecognized pet type: {}", pet_type),
        };
        let Some(pet) = pet else {
            return Err(ActorError::PetTypeNotFound(pet_type.to_string()));
        };
        // TODO: needed to prevent memory leaks, maybe use weak ptrs instead?
        pet.clear_follow();
        self.stage.remove_body_ref(pet);
        Ok(())
    }

    async fn tell_message(&self) -> Result<(), ActorError> {
        let msg_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: tell_message({})", self.res.name, msg_id));
        let body = self
            .stage
            .get_body(self.res.id)
            .ok_or(ActorError::BodyNotFound)?;

        self.dialog.tell_message(
            &self.res.name,
            body.portrait_id.get().unwrap(),
            msg_id,
            self.res.id,
        );
        Ok(())
    }

    async fn unfreeze(&self) -> Result<(), ActorError> {
        let actor_id = self.stack.borrow_mut().pop_front().unwrap();
        js::log(&format!("{}: unfreeze({})", self.res.name, actor_id));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .unfreeze();
        Ok(())
    }

    async fn wait(&self) {
        let mut stack = self.stack.borrow_mut();
        let _junk = stack.pop_front().unwrap();
        let cond_type = stack.pop_front().unwrap() >> 8;
        let param_c = stack.pop_front().unwrap();
        let param_b = stack.pop_front().unwrap();
        let param_a = stack.pop_front().unwrap();
        js::log(&format!(
            "{}: wait({}, {}, {}, {})",
            self.res.name, cond_type, param_a, param_b, param_c
        ));

        if cond_type == 3 {
            let response = WaitResponseFuture {
                state: self.game_state.clone(),
                actor_id: self.res.id,
            }
            .await;
            match response {
                Response::A => self.compiled.set_state(param_a),
                Response::B => self.compiled.set_state(param_b),
                Response::C => self.compiled.set_state(param_c),
                Response::StateInterrupt(state) => {
                    // don't allow resume_state to take affect
                    self.initialized.set(true);
                    self.compiled.set_state(state);
                }
            }
            return;
        }
        let cond = Cond::new(cond_type, param_a, param_b);
        js::log(&format!("{}: condition: {:?}", self.res.name, cond));
        let result = WaitFuture {
            actor_id: self.res.id,
            cond,
            stage: self.stage.clone(),
            state: self.game_state.clone(),
        }
        .await;
        js::log(&format!("{}: condition: met!", self.res.name));
        match result {
            WaitFutureResult::Ok => {}
            WaitFutureResult::StateInterrupt(state) => {
                // don't allow resume_state to take affect
                self.initialized.set(true);
                self.compiled.set_state(state);
            }
        }
    }

    async fn wander(&self) -> Result<(), ActorError> {
        let mut stack = self.stack.borrow_mut();
        let pos2 = stack.pop_front().unwrap();
        let pos1 = stack.pop_front().unwrap();
        let actor_id = stack.pop_front().unwrap();
        let x1 = (pos1 >> 8) as f64;
        let y1 = (pos1 & 0xff) as f64;
        let x2 = (pos2 >> 8) as f64;
        let y2 = (pos2 & 0xff) as f64;
        js::log(&format!(
            "{}: wander({}, {}, {}, {}, {})",
            self.res.name, actor_id, x1, y1, x2, y2
        ));

        self.stage
            .get_body(actor_id)
            .ok_or(ActorError::BodyNotFound)?
            .wander(x1, y1, x2, y2);
        Ok(())
    }
}

/// In memory representation of a parsed actor program
struct CompiledActions {
    actions: Vec<Vec<u16>>,
    states: Vec<u16>,
    state_idx: Cell<usize>,
    action_idx: Cell<usize>,
}

impl CompiledActions {
    fn new(action_str: &str) -> Self {
        let actions_bin = general_purpose::STANDARD.decode(action_str).unwrap();
        let mut iter = actions_bin.iter();

        let mut all_actions: Vec<Vec<u16>> = vec![vec![]];
        let mut states: Vec<u16> = vec![0];
        let mut last_state: u16 = 0;

        loop {
            let wrapped_state = next_u16(&mut iter);
            if let None = wrapped_state {
                break;
            }
            let state = wrapped_state.unwrap();
            let op = next_u16(&mut iter).unwrap();
            let size = ACTION_SIZE[&op];

            if last_state != state {
                all_actions.push(Vec::new());
                states.push(state);
            }
            let actions = all_actions.last_mut().unwrap();
            actions.push(PUSH);
            actions.push(size);
            for _ in 0..size {
                let param = next_u16(&mut iter).unwrap();
                actions.push(param);
            }
            actions.push(CALL);
            actions.push(op);
            last_state = state;
        }
        Self {
            states,
            actions: all_actions,
            state_idx: Cell::new(0),
            action_idx: Cell::new(0),
        }
    }

    fn state(&self) -> u16 {
        let i = min(self.state_idx.get(), self.states.len() - 1);
        return self.states[i];
    }

    fn set_state(&self, state: u16) {
        let state_idx = self
            .states
            .iter()
            .position(|&s| s >= state)
            .unwrap_or(usize::MAX);

        self.state_idx.set(state_idx);
        self.action_idx.set(0);
    }

    fn next(&self) -> Option<u16> {
        if self.state_idx.get() >= self.states.len() {
            return None;
        }
        let actions = &self.actions[self.state_idx.get()];
        if self.action_idx.get() >= actions.len() {
            self.state_idx.set(self.state_idx.get() + 1);
            self.action_idx.set(0);
            return self.next();
        }
        let buf = actions[self.action_idx.get()];
        self.action_idx.set(self.action_idx.get() + 1);
        Some(buf)
    }
}

/// A map of action ids to the number of parameters it accepts (each parameter is a u16)
static ACTION_SIZE: Lazy<HashMap<u16, u16>> = Lazy::new(|| {
    HashMap::from([
        (0x0006, 1), // attack
        (0x010a, 3), // set_intel
        (0x0206, 1), // create_self
        (0x0308, 2), // give_item
        (0x0408, 2), // set_team
        (0x050e, 5), // wait
        (0x060e, 5), // if
        (0x0808, 2), // set_state
        (0x0a06, 1), // set_message
        (0x0b06, 1), // tell_message
        (0x0c06, 1), // add_var
        (0x0d06, 1), // sub_var
        (0x0e06, 1), // set_var
        (0x0f08, 2), // move
        (0x1008, 2), // drop_item
        (0x1108, 2), // take_item
        (0x1208, 2), // set_loc
        (0x1306, 1), // add_sell_item
        (0x1406, 1), // execute_trade
        (0x1506, 1), // remove
        (0x1606, 1), // freeze
        (0x1706, 1), // unfreeze
        (0x1808, 2), // give_gold
        (0x1908, 2), // give_exp
        (0x1a08, 2), // give_henchmen
        (0x1b08, 2), // set_health
        (0x1c08, 2), // spawn
        (0x1d08, 2), // set_portrait
        (0x1e0a, 3), // patrol
        (0x1f0a, 3), // wander
        (0x2008, 2), // set_level
        (0x2106, 1), // add_quest_log
        (0x2206, 1), // remove_quest_log
        (0x2308, 2), // take_gold
        (0x2608, 2), // give_quest_pet
        (0x2708, 2), // take_pet
        (0x2808, 2), // map_set_loc
    ])
});

// TODO: these should probably be enums
const PUSH: u16 = 1;
const CALL: u16 = 2;

#[derive(Debug)]
pub enum ActorError {
    BodyNotFound,
    InsufficientIntel,
    IntelNotFound,
    TypeConversion,
    PetTypeNotFound(String),
}

impl Error for ActorError {}

impl fmt::Display for ActorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ActorError::BodyNotFound => write!(f, "Body not found"),
            ActorError::InsufficientIntel => write!(f, "Insufficient intel to perfom action"),
            ActorError::IntelNotFound => write!(f, "Intel not found"),
            ActorError::TypeConversion => write!(f, "Type conversion"),
            ActorError::PetTypeNotFound(msg) => write!(f, "Pet type not found: {}", msg),
        }
    }
}

fn next_u16(iter: &mut Iter<'_, u8>) -> Option<u16> {
    let a = *iter.next()? as u16;
    let b = *iter.next()? as u16;
    return Some(a << 8 | b);
}
