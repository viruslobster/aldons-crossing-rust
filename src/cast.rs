//! A simple async runtime for actor state machines
use crate::{
    actor::Actor,
    data::WORLD,
    game::{Dialog, InvalidDataError},
    js,
    stage::Stage,
    thrift::save::{self, ClassType, RaceType},
};
use std::{
    boxed::Box,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake},
};

/// A simple async runtime for actors
pub struct Cast {
    map_id: u16,
    actors: Vec<Rc<Actor>>,
    futures: VecDeque<Pin<Box<dyn Future<Output = ()>>>>,
    pub state: Rc<RefCell<SharedGameState>>,
    stage: Rc<Stage>,
    dialog: Rc<dyn Dialog>,
    actor_save_by_id: HashMap<u16, save::Actor>,
}

impl Cast {
    pub(crate) fn new(stage: Rc<Stage>, dialog: Rc<dyn Dialog>) -> Self {
        Self {
            map_id: 0,
            actors: Vec::new(),
            futures: VecDeque::new(),
            state: Rc::new(RefCell::new(SharedGameState {
                now: 0.0,
                vars: [0; 255],
                response_by_actor: HashMap::new(),
                quest_log: HashSet::new(),
                state_interrupt_by_actor: HashMap::new(),
                map_change_request: None,
            })),
            stage,
            dialog,
            actor_save_by_id: HashMap::new(),
        }
    }

    pub(crate) fn from_save(
        save: &save::Cast,
        stage: Rc<Stage>,
        dialog: Rc<dyn Dialog>,
    ) -> Result<Self, InvalidDataError> {
        let _map_id = save
            .map_id
            .ok_or(InvalidDataError::new("map_id field missing"))?;

        let vars_vec = save
            .vars
            .as_ref()
            .ok_or(InvalidDataError::new("vars field missing"))?;

        let mut vars: [u8; 255] = [0; 255];
        for (i, var) in vars_vec.iter().enumerate() {
            vars[i] = *var as u8;
        }
        let quest_log: HashSet<u16> = save
            .quest_log
            .as_ref()
            .unwrap_or(&BTreeSet::new())
            .iter()
            .map(|msg_id| *msg_id as u16)
            .collect();

        let map_id: u16 = save
            .map_id
            .ok_or(InvalidDataError::new("map_id field missing"))?
            .try_into()
            .map_err(|_| InvalidDataError::new("map_id not u16"))?;

        let mut actor_save_by_id: HashMap<u16, save::Actor> = save
            .actor_save_by_id
            .as_ref()
            .ok_or(InvalidDataError::new("actor_save_by_id field missing"))?
            .iter()
            .map(|(actor_id, save)| (*actor_id as u16, save.clone()))
            .collect();

        let bye_save_state = if let Some(bye_save) = actor_save_by_id.get(&173) {
            bye_save.state
        } else {
            None
        };

        if let Some(elemental_save) = actor_save_by_id.get_mut(&171) {
            let has_body = stage.get_body(171).is_some();
            if !has_body && matches!(bye_save_state, Some(1)) {
                elemental_save.state = Some(0);
                elemental_save.dead = Some(false);
            }
        }

        let mut cast = Self {
            map_id,
            actors: Vec::new(),
            futures: VecDeque::new(),
            state: Rc::new(RefCell::new(SharedGameState {
                now: 0.0,
                vars,
                quest_log,
                response_by_actor: HashMap::new(),
                state_interrupt_by_actor: HashMap::new(),
                map_change_request: None,
            })),
            stage,
            dialog,
            actor_save_by_id,
        };
        cast.load_map(map_id, true /* from_save */);
        Ok(cast)
    }

    pub fn save(&self) -> save::Cast {
        let mut actor_save_by_id: BTreeMap<i32, save::Actor> = self
            .actor_save_by_id
            .iter()
            .map(|(id, save)| (*id as i32, save.clone()))
            .collect();

        // update with currently running actors
        for actor in &self.actors {
            js::log(&format!(
                "{}: has state {:?}",
                actor.res.name, actor.resume_state
            ));
            actor_save_by_id.insert(actor.id().into(), actor.save());
        }
        let state = self.state.borrow();
        let quest_log: BTreeSet<i32> = state
            .quest_log
            .iter()
            .map(|msg_id| *msg_id as i32)
            .collect();

        let vars: Vec<i32> = state.vars.iter().map(|var| *var as i32).collect();

        save::Cast::new(self.map_id as i32, quest_log, actor_save_by_id, vars)
    }

    pub fn load_map(&mut self, map_id: u16, from_save: bool) {
        self.map_id = map_id;
        self.futures.clear();
        let mut state = self.state.borrow_mut();
        state.response_by_actor.clear();
        state.state_interrupt_by_actor.clear();
        drop(state);

        // save the state of actors in the previous map, to be restored on re-entry
        for actor in &self.actors {
            self.actor_save_by_id.insert(actor.id(), actor.save());
        }
        self.actors.clear();
        let name = &WORLD.maps[&map_id.to_string()].name;
        js::log(&format!("loaded map: {}, {}", map_id, name));

        for actor in &WORLD.maps[&map_id.to_string()].actors {
            if let None = actor.actions {
                continue;
            }
            // restore actors to the state they were in when we exited this map, if any
            let maybe_save = self.actor_save_by_id.get(&actor.id).clone();
            let actor = if let Some(save) = maybe_save {
                // Actors loaded from a save do not do the usual thing of running the
                // initial state and then jumping to the resume_state, so directly
                // set the state
                let mut actor = Actor::from_save(
                    self.state.clone(),
                    self.stage.clone(),
                    actor,
                    self.dialog.clone(),
                    &save,
                )
                .unwrap();
                let state: u16 = save.state.unwrap().try_into().unwrap();

                if from_save {
                    js::log(&format!("{}: resume to {}", actor.res.name, state));
                    actor.resume(state);
                } else {
                    js::log(&format!("{}: resume_after_init", actor.res.name));
                    actor.resume_after_init(state);
                }
                js::log(&format!(
                    "{}: constructed with state {:?}",
                    actor.res.name, actor.resume_state
                ));
                Rc::new(actor)
            } else {
                js::log(&format!("{}: constructed without state", actor.name,));
                Rc::new(Actor::new(
                    self.state.clone(),
                    self.stage.clone(),
                    actor,
                    self.dialog.clone(),
                    None, // state
                ))
            };
            self.actors.push(actor.clone());
            let closure = async move { actor.act().await };
            self.futures.push_front(Box::pin(closure));
        }

        // A bug broke the state of some games in the Zipfritzle quest. Leave
        // this in for a while to automatically fix.
        if map_id == 138 {
            let boulder_exists = self.stage.get_body(409).is_some();
            let Some(boulder_actor) = self.actors.iter().find(|a| a.id() == 409) else {
                return;
            };
            if boulder_exists && boulder_actor.dead.get() {
                boulder_actor.dead.set(false);
            }
        }
    }

    /// run all the actor state machines
    pub fn act(&mut self, now: f64) {
        self.state.borrow_mut().now = now;

        for _ in 0..self.futures.len() {
            let mut future = self.futures.pop_back().unwrap();
            let waker = Arc::new(NoopWaker {}).into();
            let mut context = Context::from_waker(&waker);
            if let Poll::Pending = future.as_mut().poll(&mut context) {
                self.futures.push_front(future);
            }
        }
    }

    /// Allows the player to repond to dialog from actors
    pub fn send_response(&mut self, actor_id: u16, response: Response) {
        let prev = self
            .state
            .borrow_mut()
            .response_by_actor
            .insert(actor_id, response);
        assert!(
            matches!(prev, None),
            "actor {} recieved response while previous response was still not processes",
            actor_id
        );
    }
}

/// Conditions that can be expressed in actor state machines with `wait <condition>` or `if <condition>`
#[derive(Debug)]
pub(crate) enum Cond {
    Class { actor_id: u16, kind: ClassType },
    Dead { actor_id: u16 },
    Level { level: i32 },
    Location { actor_id: u16, x: f64, y: f64 },
    NotLocation { actor_id: u16, x: f64, y: f64 },
    PickedUp { actor_id: u16 },
    PlayerHas { prop_id: u16 },
    PlayerHasGold { gold: i32 },
    PlayerHenchmen { prop_id: u16 },
    PlayerQuestPet { prop_id: u16 },
    PlayerSummoned { prop_id: u16 },
    Race { kind: RaceType },
    VarEqual { var: usize, val: u8 },
    VarGreat { var: usize, val: u8 },
    VarLess { var: usize, val: u8 },
}

impl Cond {
    pub fn new(cond_type: u16, param_a: u16, param_b: u16) -> Self {
        match cond_type {
            0 => Cond::PickedUp { actor_id: param_a },
            1 => Cond::Dead { actor_id: param_a },
            2 => Cond::Level {
                level: param_a as i32,
            },
            4 => Cond::PlayerHas { prop_id: param_a },
            5 => Cond::VarEqual {
                var: (param_a >> 8) as usize,
                val: (param_a & 0xff) as u8,
            },
            6 => Cond::VarLess {
                var: (param_a >> 8) as usize,
                val: (param_a & 0xff) as u8,
            },
            7 => Cond::VarGreat {
                var: (param_a >> 8) as usize,
                val: (param_a & 0xff) as u8,
            },
            8 => Cond::Location {
                actor_id: param_a,
                x: (param_b >> 8) as f64,
                y: (param_b & 0xff) as f64,
            },
            9 => Cond::Class {
                actor_id: param_a,
                kind: ClassType::try_from((param_b >> 8) as i32).unwrap(),
            },
            11 => Cond::PlayerHasGold {
                gold: param_a as i32,
            },
            12 => Cond::PlayerHenchmen { prop_id: param_a },
            13 => Cond::PlayerSummoned { prop_id: param_a },
            14 => Cond::Race {
                kind: RaceType::from_u16(param_a >> 8),
            },
            15 => Cond::PlayerQuestPet { prop_id: param_a },
            16 => Cond::NotLocation {
                actor_id: param_a,
                x: (param_b >> 8) as f64,
                y: (param_b & 0xff) as f64,
            },
            _ => panic!("Unrecognized cond type {}", cond_type),
        }
    }

    pub fn satisfied(&self, stage: &Stage, state: &mut SharedGameState) -> bool {
        match self {
            Cond::Class { actor_id, kind } => {
                if let Some(body) = stage.get_body(*actor_id) {
                    js::log(&format!(
                        "actor_id {} is class {:?} testing {:?}",
                        actor_id,
                        body.class.get(),
                        kind
                    ));
                    body.class.get() == *kind
                } else {
                    false
                }
            }
            Cond::Dead { actor_id } => {
                if let Some(body) = stage.get_body(*actor_id) {
                    body.get_health() == 0
                } else {
                    true // may cause races?
                }
            }
            Cond::Level { level } => stage.get_player().level() >= *level,
            Cond::Location { actor_id, x, y } => {
                if let Some(body) = stage.get_body(*actor_id) {
                    let (body_x, body_y) = body.moving_from();
                    body_x == *x && body_y == *y
                } else {
                    false
                }
            }
            Cond::NotLocation { actor_id, x, y } => {
                if !stage.player_has_moved() {
                    // without this teleporting to some places causes infinite loops
                    false
                } else if let Some(body) = stage.get_body(*actor_id) {
                    let (body_x, body_y) = body.moving_from();
                    body_x != *x || body_y != *y
                } else {
                    true
                }
            }
            Cond::PickedUp { actor_id } => {
                if let Some(body) = stage.get_body(*actor_id) {
                    body.get_health() == 0
                } else {
                    true
                }
            }
            Cond::PlayerHas { prop_id } => stage.get_player().has_item(*prop_id),
            Cond::PlayerHasGold { gold } => stage.get_player().gold.get() >= *gold,
            Cond::PlayerHenchmen { prop_id } => {
                let pet = stage.get_player().pet();
                if pet.is_some() {
                    let pet = pet.unwrap();
                    pet.prop_id == *prop_id && pet.get_health() > 0
                } else {
                    false
                }
            }
            Cond::PlayerQuestPet { prop_id } => {
                let quest_pet = stage.get_player().quest_pet();
                if quest_pet.is_some() {
                    let quest_pet = quest_pet.unwrap();
                    quest_pet.prop_id == *prop_id && quest_pet.get_health() > 0
                } else {
                    false
                }
            }
            Cond::PlayerSummoned { prop_id } => {
                let summoned_pet = stage.get_player().summoned_pet();
                if summoned_pet.is_some() {
                    let summoned_pet = summoned_pet.unwrap();
                    summoned_pet.prop_id == *prop_id && summoned_pet.get_health() > 0
                } else {
                    false
                }
            }
            Cond::Race { kind } => stage.get_player().race.get() == Some(*kind),
            Cond::VarEqual { var, val } => state.vars[*var] == *val,
            Cond::VarGreat { var, val } => state.vars[*var] > *val,
            Cond::VarLess { var, val } => state.vars[*var] < *val,
        }
    }
}

/// Async implementation for waiting on a condition
pub(crate) struct WaitFuture {
    pub actor_id: u16,
    pub cond: Cond,
    pub stage: Rc<Stage>,
    pub state: Rc<RefCell<SharedGameState>>,
}

pub enum WaitFutureResult {
    Ok,
    StateInterrupt(u16),
}

impl Future for WaitFuture {
    type Output = WaitFutureResult;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut game_state = self.state.borrow_mut();

        if let Some(actor_state) = game_state.state_interrupt_by_actor.remove(&self.actor_id) {
            Poll::Ready(WaitFutureResult::StateInterrupt(actor_state))
        } else if self.cond.satisfied(&self.stage, &mut game_state) {
            Poll::Ready(WaitFutureResult::Ok)
        } else {
            Poll::Pending
        }
    }
}

/// Async implementation for waiting on a player's response
pub(crate) struct WaitResponseFuture {
    pub state: Rc<RefCell<SharedGameState>>,
    pub actor_id: u16,
}

impl Future for WaitResponseFuture {
    type Output = Response;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut state = self.state.borrow_mut();

        if let Some(state) = state.state_interrupt_by_actor.remove(&self.actor_id) {
            Poll::Ready(Response::StateInterrupt(state))
        } else if let Some(response) = state.response_by_actor.remove(&self.actor_id) {
            Poll::Ready(response)
        } else {
            Poll::Pending
        }
    }
}

/// Useful to allow all actors to init before allowing any one to run to completion
pub(crate) async fn yield_now() {
    YieldNowFuture { yielded: false }.await;
}

pub(crate) struct YieldNowFuture {
    yielded: bool,
}

impl Future for YieldNowFuture {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            let mut state = self.as_mut();
            state.yielded = true;
            Poll::Pending
        }
    }
}

/// A part of Rust's async setup that I'm not using. No need to wake up actors, its not expensive
/// to poll all of them on every cycle
struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
}

pub enum Response {
    A,
    B,
    C,
    StateInterrupt(u16),
}

impl From<u8> for Response {
    fn from(response: u8) -> Response {
        match response {
            0 => Response::A,
            1 => Response::B,
            2 => Response::C,
            _ => panic!("Illegal response {}", response),
        }
    }
}

pub struct SharedGameState {
    // TODO: make these all Cells and RefCells
    pub now: f64,

    /// State machine's have 255 variables to represent things like progress on quests
    pub vars: [u8; 255],

    /// This is used as a channel to notify an actor that the player has given a response to it
    pub response_by_actor: HashMap<u16, Response>,

    pub quest_log: HashSet<u16>,

    /// AldonGame checks this every cycle to see if an actor is trying to change the map
    pub map_change_request: Option<(u16, f64, f64)>,

    // This acts as a channel for actors to set the state of other actors
    pub state_interrupt_by_actor: HashMap<u16, u16>,
}
