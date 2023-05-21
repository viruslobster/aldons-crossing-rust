//! Implementation for a Body
use crate::{
    actor::ActorError,
    aldon_log,
    combat::{dir, monster_reward, BattleEvent, BattleEventType, Motion},
    condition::{self, Condition},
    data::{PropTypeRes, PROPS, SPELLS},
    game::{EquipType, InvalidDataError, CONSOLE},
    js, stats,
    stats::PlayerStats,
    thrift::{
        save::{self, ConditionType, RaceType},
        util::box_vec,
    },
};
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};
use std::{
    cell::{Cell, RefCell},
    cmp::{max, min, Ordering},
    collections::HashMap,
    fmt::Write,
    rc::Rc,
    vec::Vec,
};
use thrift::OrderedFloat;

/// A Body is any game element that is "physical"; anything that could be ran into, picked up,
/// talked to, or otherwise interacted with.
/// TODO: maybe break this up based on the capabilities a body needs, we'll see.
#[derive(Debug)]
pub struct Body {
    pub actor_id: Option<u16>,
    pub intel: RefCell<Option<Intel>>,
    pub class: Cell<save::ClassType>,
    pub health: Cell<i32>,
    pub magic: Cell<i32>,
    pub level: Cell<i32>,
    pub team: Cell<Option<save::Team>>,
    pub race: Cell<Option<save::RaceType>>,
    pub x: Cell<f64>,
    pub y: Cell<f64>,
    pub gold: Cell<i32>,
    pub prop_id: u16,
    pub last_attack_time: Cell<f64>,
    pub last_spawn: Cell<Option<usize>>,
    pub inventory: RefCell<Vec<Rc<Body>>>,
    pub exp: Cell<i32>,
    pub pet: RefCell<Option<Rc<Body>>>,
    pub quest_pet: RefCell<Option<Rc<Body>>>,
    pub summoned_pet: RefCell<Option<Rc<Body>>>,
    pub portrait_id: Cell<Option<u16>>,
    pub name: String,
    pub action_state: Cell<ActionState>,
    pub hostile_to: Cell<Option<save::Team>>,
    pub male: Cell<bool>,

    pub(crate) patrol_goal: Cell<Option<(f64, f64, f64, f64)>>,
    pub(crate) base_str: Cell<i32>,
    pub(crate) base_int: Cell<i32>,
    pub(crate) base_dex: Cell<i32>,
    pub(crate) base_wis: Cell<i32>,
    pub(crate) base_vit: Cell<i32>,
    pub(crate) base_luck: Cell<i32>,
    pub(crate) battle_events: RefCell<Vec<BattleEvent>>,
    pub(crate) death_time: Cell<f64>,
    pub(crate) prefer_melee: Cell<bool>,

    // for groupable items, how many are in this group
    pub(crate) quantity: Cell<u8>,

    // If true body will be serialized when saving
    // TODO: get rid of this, its always true
    pub(crate) persist: Cell<bool>,

    talk_to: RefCell<Option<Rc<Body>>>,
    attack: RefCell<Option<Rc<Body>>>,
    follow: RefCell<Option<Rc<Body>>>,
    wanderer: RefCell<Option<Wanderer>>,
    conditions: RefCell<Vec<Condition>>,
    frozen: Cell<bool>,
    is_pet: Cell<bool>,
    from_spawner: Cell<bool>,
    last_spell: Cell<Option<CastSpell>>,
    revert_action_state: Cell<Option<RevertActionState>>,

    // Present if the body is in the process of moving from one square to another.
    // A motion only represents movement between two adjacent squares
    motion: Cell<Option<Motion>>,

    // The square the target will move to. May be many squares away
    target_x: Cell<f64>,
    target_y: Cell<f64>,

    // item_id by EquipType
    equiped: RefCell<HashMap<EquipType, Rc<Body>>>,
}

impl Body {
    pub fn new(name: String, actor_id: Option<u16>, prop_id: u16, x: f64, y: f64) -> Self {
        let prop = &PROPS[&prop_id.to_string()];

        let portrait_id = match prop.kind {
            PropTypeRes::Creature { portrait, .. } | PropTypeRes::User { portrait, .. } => {
                Some(portrait)
            }
            _ => None,
        };
        let (base_str, base_int, base_dex, base_wis, base_vit, base_luck) = match prop.kind {
            PropTypeRes::User {
                strength,
                inteligence,
                dexterity,
                wisdom,
                vitality,
                luck,
                ..
            }
            | PropTypeRes::Creature {
                strength,
                inteligence,
                dexterity,
                wisdom,
                vitality,
                luck,
                ..
            } => (strength, inteligence, dexterity, wisdom, vitality, luck),

            _ => (8, 8, 8, 8, 8, 8),
        };
        Self {
            name,
            actor_id,
            x: Cell::new(x),
            y: Cell::new(y),
            prop_id,
            portrait_id: Cell::new(portrait_id),
            intel: RefCell::new(None),
            team: Cell::new(None),
            // The default needs to be Fighter b/c the weapons enemies
            // try to equip by default can all be used by fighters
            class: Cell::new(save::ClassType::FIGHTER),
            race: Cell::new(None),
            health: Cell::new(1),
            magic: Cell::new(0),
            level: Cell::new(1),
            gold: Cell::new(0),
            pet: RefCell::new(None),
            quest_pet: RefCell::new(None),
            summoned_pet: RefCell::new(None),
            inventory: RefCell::new(Vec::new()),
            exp: Cell::new(0),
            motion: Cell::new(None),
            target_x: Cell::new(x),
            target_y: Cell::new(y),
            action_state: Cell::new(ActionState::Idle),
            talk_to: RefCell::new(None),
            equiped: RefCell::new(HashMap::new()),
            patrol_goal: Cell::new(None),
            attack: RefCell::new(None),
            last_attack_time: Cell::new(0.0),
            battle_events: RefCell::new(Vec::new()),
            death_time: Cell::new(0.0),
            hostile_to: Cell::new(None),
            prefer_melee: Cell::new(true),
            base_str: Cell::new(base_str),
            base_int: Cell::new(base_int),
            base_dex: Cell::new(base_dex),
            base_wis: Cell::new(base_wis),
            base_vit: Cell::new(base_vit),
            base_luck: Cell::new(base_luck),
            last_spawn: Cell::new(None),
            follow: RefCell::new(None),
            wanderer: RefCell::new(None),
            quantity: Cell::new(1),
            conditions: RefCell::new(Vec::new()),
            male: Cell::new(true),
            persist: Cell::new(true),
            frozen: Cell::new(false),
            is_pet: Cell::new(false),
            from_spawner: Cell::new(false),
            last_spell: Cell::new(None),
            revert_action_state: Cell::new(None),
        }
    }

    pub fn set_prefer_melee(&self, prefer_melee: bool) {
        self.prefer_melee.set(prefer_melee)
    }

    pub fn magic(&self) -> i32 {
        self.magic.get()
    }

    pub fn action_state(&self) -> ActionState {
        self.action_state.get()
    }

    pub fn set_from_spawner(&self, from_spawner: bool) {
        self.from_spawner.set(from_spawner);
    }

    pub fn prefer_melee(&self) -> bool {
        self.prefer_melee.get()
    }

    pub fn from_spawner(&self) -> bool {
        self.from_spawner.get()
    }

    pub fn quantity(&self) -> u8 {
        self.quantity.get()
    }

    pub fn exp(&self) -> i32 {
        self.exp.get()
    }

    pub fn set_is_pet(&self, is_pet: bool) {
        self.is_pet.set(is_pet);
    }

    pub fn set_target_y(&self, y: f64) {
        self.target_y.set(y);
    }

    pub fn set_target_x(&self, x: f64) {
        self.target_x.set(x);
    }

    pub fn target_x(&self) -> f64 {
        self.target_x.get()
    }

    pub fn target_y(&self) -> f64 {
        self.target_y.get()
    }

    pub fn set_last_attack_time(&self, t: f64) {
        self.last_attack_time.set(t);
    }

    pub fn last_attack_time(&self) -> f64 {
        self.last_attack_time.get()
    }

    pub fn level(&self) -> i32 {
        self.level.get()
    }

    pub fn is_pet(&self) -> bool {
        self.is_pet.get()
    }

    pub fn set_action_state(&self, state: ActionState) {
        self.action_state.set(state);
        self.revert_action_state.set(None);
    }

    pub fn death_time(&self) -> f64 {
        self.death_time.get()
    }

    pub fn frozen(&self) -> bool {
        self.frozen.get()
    }

    pub(crate) fn motion(&self) -> Option<Motion> {
        self.motion.get()
    }

    pub fn team(&self) -> Option<save::Team> {
        self.team.get()
    }

    pub fn x(&self) -> f64 {
        self.x.get()
    }

    pub fn y(&self) -> f64 {
        self.y.get()
    }

    pub fn set_x(&self, x: f64) {
        self.x.set(x);
    }

    pub fn set_y(&self, y: f64) {
        self.y.set(y);
    }

    pub fn set_portrait(&self, portrait_id: u16) {
        self.portrait_id.set(Some(portrait_id));
    }

    pub fn set_enemy(&self, enemy_team: save::Team) {
        self.hostile_to.set(Some(enemy_team));
    }

    pub fn persist(&self) {
        self.persist.set(true);
    }

    pub fn take_gold(&self, gold: i32) {
        self.gold.set(self.gold.get() - gold);
    }

    pub fn pet(&self) -> Option<Rc<Body>> {
        let pet = self.pet.borrow();
        if pet.is_none() {
            return None;
        }
        Some(pet.as_ref().unwrap().clone())
    }

    pub fn give_pet(&self, new_pet: Rc<Body>) {
        let mut pet = self.pet.borrow_mut();
        *pet = Some(new_pet);
    }

    pub fn take_pet(&self) -> Option<Rc<Body>> {
        if self.pet.borrow().is_none() {
            return None;
        }
        self.pet.borrow_mut().take()
    }

    pub fn quest_pet(&self) -> Option<Rc<Body>> {
        let pet = self.quest_pet.borrow();
        if pet.is_none() {
            return None;
        }
        Some(pet.as_ref().unwrap().clone())
    }

    pub fn give_quest_pet(&self, new_pet: Rc<Body>) {
        let mut pet = self.quest_pet.borrow_mut();
        *pet = Some(new_pet);
    }

    pub fn take_quest_pet(&self) -> Option<Rc<Body>> {
        if self.quest_pet.borrow().is_none() {
            return None;
        }
        self.quest_pet.borrow_mut().take()
    }

    pub fn summoned_pet(&self) -> Option<Rc<Body>> {
        let pet = self.summoned_pet.borrow();
        if pet.is_none() {
            return None;
        }
        Some(pet.as_ref().unwrap().clone())
    }

    pub fn give_summoned_pet(&self, new_pet: Rc<Body>) {
        let mut pet = self.summoned_pet.borrow_mut();
        *pet = Some(new_pet);
    }

    pub fn take_summoned_pet(&self) -> Option<Rc<Body>> {
        if self.summoned_pet.borrow().is_none() {
            return None;
        }
        self.summoned_pet.borrow_mut().take()
    }

    pub fn from_save(now: f64, save: &save::Body) -> Result<Self, InvalidDataError> {
        let mut conditions: Vec<Condition> = save
            .conditions
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|s| Condition::from_save(s, now))
            .filter_map(|result| match result {
                Ok(effect) => Some(effect),
                Err(error) => {
                    js::log(&format!("Load effect failed: {}", error));
                    None
                }
            })
            .collect();

        let name = save
            .name
            .as_ref()
            .ok_or(InvalidDataError::new("name field missing"))?
            .to_string();

        let actor_id = save.actor_id.map(|id| id as u16);
        let x = *save.x.ok_or(InvalidDataError::new("x field missing"))?;
        let y = *save.y.ok_or(InvalidDataError::new("y field missing"))?;

        let prop_id: u16 = save
            .prop_id
            .ok_or(InvalidDataError::new("prop_id field missing"))?
            .try_into()
            .map_err(|_| InvalidDataError::new("prop_id not valid u16"))?;

        let portrait_id = save
            .portrait_id
            .map(|id| id.try_into())
            .transpose()
            .map_err(|err| InvalidDataError::new(&format!("portrait_id invalid: {}", err)))?;

        let class = save
            .klass
            .ok_or(InvalidDataError::new("klass field missing"))?;

        let health = save
            .health
            .ok_or(InvalidDataError::new("health field missing"))?;

        let magic = save
            .magic
            .ok_or(InvalidDataError::new("magic field missing"))?;

        let level = save
            .level
            .ok_or(InvalidDataError::new("level field missing"))?;

        let gold = save
            .gold
            .ok_or(InvalidDataError::new("gold field missing"))?;

        let exp: i32 = save
            .exp
            .ok_or(InvalidDataError::new("exp field missing"))?
            .try_into()
            .map_err(|err| InvalidDataError::new(&format!("exp field: {}", err)))?;

        let base_str = save
            .base_str
            .ok_or(InvalidDataError::new("base_str field missing"))?;

        let base_int = save
            .base_int
            .ok_or(InvalidDataError::new("base_int field missing"))?;

        let base_dex = save
            .base_dex
            .ok_or(InvalidDataError::new("base_dex field missing"))?;

        let base_wis = save
            .base_wis
            .ok_or(InvalidDataError::new("base_wis field missing"))?;

        let base_vit = save
            .base_vit
            .ok_or(InvalidDataError::new("base_vit field missing"))?;

        let base_luck = save
            .base_luck
            .ok_or(InvalidDataError::new("base_luck field missing"))?;

        let quantity: u8 = save
            .quantity
            .unwrap_or(1)
            .try_into()
            .map_err(|err| InvalidDataError::new(&format!("quantity field: {}", err)))?;

        let wanderer = save
            .wanderer
            .as_ref()
            .map(|wanderer_save| Wanderer::from_save(wanderer_save, now))
            .transpose()?;

        let male: bool = save.male.unwrap_or(true);
        let persist = save.persist.unwrap_or(true);
        let frozen = save.frozen.unwrap_or(false);
        let is_pet = save.is_pet.unwrap_or(false);
        let from_spawner = save.from_spawner.unwrap_or(false);

        let last_spell: Option<CastSpell> = save
            .last_spell
            .as_ref()
            .map(|s| CastSpell::from_save(s, now))
            .transpose()
            .unwrap_or_else(|err| {
                js::log(&format!("Warning, error while parsing last spell: {}", err));
                None
            });

        let prefer_melee = save.prefer_melee.unwrap_or(true);

        if conditions.len() <= 1 && actor_id == Some(0) {
            // Due to the condition rewrite all conditions may fail to load
            // If thats the case make sure the player at least still is healing
            let c = condition::body_regen(now);
            conditions.push(c);
            let c = condition::body_mana_regen(now);
            conditions.push(c);
        }

        let body = Self {
            name,
            actor_id,
            x: Cell::new(x),
            y: Cell::new(y),
            target_x: Cell::new(x),
            target_y: Cell::new(y),
            prop_id,
            portrait_id: Cell::new(portrait_id),
            intel: RefCell::new(None),
            team: Cell::new(save.team),
            class: Cell::new(class),
            race: Cell::new(save.race),
            health: Cell::new(health),
            magic: Cell::new(magic),
            level: Cell::new(level),
            gold: Cell::new(gold),
            exp: Cell::new(exp),
            pet: RefCell::new(None),
            quest_pet: RefCell::new(None),
            summoned_pet: RefCell::new(None),
            inventory: RefCell::new(Vec::new()),
            equiped: RefCell::new(HashMap::new()),
            motion: Cell::new(None),
            action_state: Cell::new(ActionState::Idle),
            talk_to: RefCell::new(None),
            patrol_goal: Cell::new(None),
            attack: RefCell::new(None),
            last_attack_time: Cell::new(0.0),
            battle_events: RefCell::new(Vec::new()),
            death_time: Cell::new(0.0),
            hostile_to: Cell::new(save.hostile_to),
            prefer_melee: Cell::new(prefer_melee),
            base_str: Cell::new(base_str),
            base_int: Cell::new(base_int),
            base_dex: Cell::new(base_dex),
            base_wis: Cell::new(base_wis),
            base_vit: Cell::new(base_vit),
            base_luck: Cell::new(base_luck),
            last_spawn: Cell::new(None),
            follow: RefCell::new(None),
            quantity: Cell::new(quantity),
            conditions: RefCell::new(conditions),
            male: Cell::new(male),
            persist: Cell::new(persist),
            frozen: Cell::new(frozen),
            is_pet: Cell::new(is_pet),
            from_spawner: Cell::new(from_spawner),
            wanderer: RefCell::new(wanderer),
            last_spell: Cell::new(last_spell),
            revert_action_state: Cell::new(None),
        };
        if let Some(kind) = save.intel_type {
            body.set_intel(kind);
        }
        Ok(body)
    }

    pub(crate) fn save(&self, now: f64) -> save::Body {
        let condition_saves: Vec<save::Condition> = self
            .conditions
            .borrow()
            .iter()
            // save() for item conditions will return None and be filtered out.
            // They will be recreated when equiped
            .filter_map(|c| c.save(now))
            .collect();

        let wanderer = self
            .wanderer
            .borrow()
            .as_ref()
            .map(|w| Box::new(w.save(now)));

        let last_spell = self.last_spell.get().map(|s| s.save(now));

        save::Body::new(
            self.class.get(),
            self.get_health(),
            self.magic.get(),
            self.level(),
            self.race.get(),
            self.actor_id.map(|id| id.into()),
            self.team(),
            // position needs to be floored so when you load you start on a square
            OrderedFloat::from(self.x().floor()),
            OrderedFloat::from(self.y().floor()),
            self.gold.get(),
            self.prop_id as i32,
            self.exp(),
            self.portrait_id.get().map(|id| id.into()),
            self.name.clone(),
            self.hostile_to.get(),
            self.base_str.get(),
            self.base_int.get(),
            self.base_dex.get(),
            self.base_wis.get(),
            self.base_vit.get(),
            self.base_luck.get(),
            self.intel.borrow().as_ref().map(|intel| intel.kind.get()),
            self.quantity() as i32,
            false, // equiped
            self.male.get(),
            self.persist.get(),
            self.frozen(),
            self.is_pet(),
            self.from_spawner(),
            wanderer,
            box_vec(&condition_saves),
            last_spell,
            self.prefer_melee(),
        )
    }

    pub fn save_inventory(&self, now: f64) -> Vec<Box<save::Body>> {
        self.inventory
            .borrow()
            .iter()
            .map(|body| {
                let mut save = body.save(now);
                save.equiped = Some(self.is_equiped(body.clone()).is_some());
                Box::new(save)
            })
            .collect()
    }

    pub fn give_inventory(
        &self,
        now: f64,
        inventory: &[save::Body],
    ) -> Result<(), InvalidDataError> {
        for save in inventory {
            let body = Rc::new(Body::from_save(now, save)?);
            self.give_body_item(body.clone());

            if let Some(true) = save.equiped {
                self.equip(now, body);
            }
        }
        Ok(())
    }

    /// Returns player's inventory sorted by how its equiped
    pub fn inventory(&self) -> Vec<Rc<Body>> {
        let mut inventory = self.inventory.borrow().clone();
        inventory.sort_by(
            |a, b| match (self.is_equiped(a.clone()), self.is_equiped(b.clone())) {
                (None, None) => Ordering::Equal,
                (Some(..), None) => Ordering::Less,
                (None, Some(..)) => Ordering::Greater,
                (Some(equip_a), Some(equip_b)) => equip_a.cmp(&equip_b),
            },
        );
        inventory
    }

    fn sum_condition(&self, kind: save::ConditionType) -> i32 {
        self.conditions
            .borrow()
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.magnitude)
            .sum()
    }

    pub fn strength(&self) -> i32 {
        self.base_str.get() + self.sum_condition(save::ConditionType::STRENGTH)
    }

    pub fn inteligence(&self) -> i32 {
        self.base_int.get() + self.sum_condition(save::ConditionType::INTELIGENCE)
    }

    pub fn dexterity(&self) -> i32 {
        self.base_dex.get() + self.sum_condition(save::ConditionType::DEXTERITY)
    }

    pub fn wisdom(&self) -> i32 {
        self.base_wis.get()
    }

    pub fn vitality(&self) -> i32 {
        self.base_vit.get()
    }

    pub fn luck(&self) -> i32 {
        self.base_luck.get() + self.sum_condition(save::ConditionType::LUCK)
    }

    pub fn speed(&self) -> i32 {
        let factor = 1.0 + self.sum_condition(save::ConditionType::SPEED) as f64 / 4.0;
        (300.0 / factor) as i32
    }

    pub fn hidden(&self) -> bool {
        self.sum_condition(save::ConditionType::HIDDEN) >= 1
    }

    pub fn sneaking(&self) -> bool {
        self.sum_condition(save::ConditionType::SNEAKING) >= 1
    }

    pub fn henchmen(&self) -> Vec<Rc<Body>> {
        let mut bodies: Vec<Rc<Body>> = Vec::new();
        for maybe_body in vec![
            self.pet.borrow().clone(),
            self.quest_pet.borrow().clone(),
            self.summoned_pet.borrow().clone(),
        ] {
            if let Some(body) = maybe_body {
                bodies.push(body.clone());
            }
        }
        bodies
    }

    /// Where the body is at or its immediate destination tile
    pub fn moving_to(&self) -> (f64, f64) {
        if let Some(motion) = self.motion() {
            (motion.x1, motion.y1)
        } else {
            (self.x().floor(), self.y().floor())
        }
    }

    /// Where the body is at or the tile it most recently left
    pub fn moving_from(&self) -> (f64, f64) {
        if let Some(motion) = self.motion() {
            (motion.x0, motion.y0)
        } else {
            (self.x().floor(), self.y().floor())
        }
    }

    pub(crate) fn is_attack_ranged(&self) -> bool {
        if let Some(prop_id) = self.equiped_weapon() {
            let prop = &PROPS[&prop_id.to_string()];

            if let PropTypeRes::Weapon { equip_to, .. } = &prop.kind {
                EquipType::from_str(&equip_to) == EquipType::Range
            } else {
                panic!("{} was equiped as a weapon but isn't a weapon", prop_id);
            }
        } else {
            false
        }
    }

    pub fn equip_default(&self, now: f64) {
        let prop = &PROPS[&self.prop_id.to_string()];

        let (weapon_id, armor_id) = match prop.kind {
            PropTypeRes::Creature { weapon, armor, .. }
            | PropTypeRes::User { weapon, armor, .. } => (weapon, armor),

            _ => (0, 0),
        };
        if weapon_id > 0 {
            let item_id = self.give_item(weapon_id);
            self.force_equip(now, item_id);
        }
        if armor_id > 0 {
            let item_id = self.give_item(armor_id);
            self.force_equip(now, item_id);
        }
    }

    pub(crate) fn equiped_weapon(&self) -> Option<u16> {
        let equiped = self.equiped.borrow();
        let melee_weapon = equiped.get(&EquipType::Melee).map(|body| body.prop_id);
        let range_weapon = equiped.get(&EquipType::Range).map(|body| body.prop_id);

        if melee_weapon.is_some() ^ range_weapon.is_some() {
            melee_weapon.or(range_weapon)
        } else if self.prefer_melee() {
            melee_weapon
        } else {
            range_weapon
        }
    }

    fn armor_value(&self) -> i32 {
        const EQUIP_TYPES: [EquipType; 12] = [
            EquipType::Head,
            EquipType::Neck,
            EquipType::Chest,
            EquipType::Arm,
            EquipType::Hand,
            EquipType::Leg,
            EquipType::Foot,
            EquipType::Back,
            EquipType::Shield,
            EquipType::Ring1,
            EquipType::Ring2,
            EquipType::Suit,
        ];
        let mut value = 0;
        for kind in EQUIP_TYPES {
            value += self
                .equiped
                .borrow()
                .get(&kind)
                .and_then(|body| Some(PROPS[&body.prop_id.to_string()].armor_value()))
                .unwrap_or(0)
        }
        value
    }

    pub(crate) fn attack_damage(&self, prop_id: u16, opponent_level: i32) -> i32 {
        let factor = max(1, (self.level() - opponent_level) / 2);
        if matches!(prop_id, 244 | 314 | 392 | 237) {
            let mut rng = rand::thread_rng();
            let damage_min = 1;
            let damage_max = 2 + 3 * self.level();
            return rng.gen_range(damage_min..=damage_max) * factor;
        }
        let prop = &PROPS[&prop_id.to_string()];

        if let PropTypeRes::Weapon {
            damage_min,
            damage_max,
            ..
        } = prop.kind
        {
            let mut rng = rand::thread_rng();
            let dmg =
                rng.gen_range(damage_min..=damage_max) + stats::strength_to_damage(self.strength());
            return factor * max(dmg, 1);
        }
        panic!("{} was equiped as a weapon but is not a weapon", prop_id);
    }

    pub(crate) fn attack_delay(&self) -> Option<f64> {
        let prop_id = self.equiped_weapon();
        if prop_id.is_none() {
            return None;
        }
        let prop_id = prop_id.unwrap();

        if let PropTypeRes::Weapon { delay, .. } = &PROPS[&prop_id.to_string()].kind {
            return Some(*delay as f64);
        }
        js::log(&format!(
            "Warning, {} was equiped as a weapon but is not a weapon",
            prop_id
        ));
        None
    }

    pub(crate) fn attack_hit_bonus(&self) -> i32 {
        if self.is_attack_ranged() {
            return stats::dexterity_to_hit_bonus(self.dexterity());
        }
        stats::strength_to_hit_bonus(self.strength())
    }

    pub(crate) fn armor_class(&self) -> i32 {
        let armor_value = self.armor_value()
            + stats::dexterity_to_armor_class(self.dexterity())
            + self.sum_condition(save::ConditionType::ARMOR);

        max(armor_value, 0)
    }

    pub(crate) fn talk_to(&self, body: Rc<Body>) {
        let mut talk_to = self.talk_to.borrow_mut();
        *talk_to = Some(body);
    }

    fn motion_active(&self, now: f64) -> bool {
        if let Some(motion) = self.motion() {
            motion.end_t > now
        } else {
            false
        }
    }

    pub(crate) fn needs_walk_update(&self, now: f64) -> Option<(f64, f64)> {
        if (self.x == self.target_x && self.y == self.target_y)
            || self.motion_active(now)
            || self.get_health() <= 0
        {
            None
        } else {
            Some((self.target_x(), self.target_y()))
        }
    }

    pub(crate) fn needs_patrol_update(&self, now: f64) -> Option<(f64, f64, f64, f64)> {
        if self.motion_active(now) {
            None
        } else {
            self.patrol_goal.get()
        }
    }

    pub(crate) fn needs_follow_update(&self, now: f64) -> Option<Rc<Body>> {
        if self.motion_active(now) || self.health.get() <= 0 {
            return None;
        }
        let follow = self.follow.borrow();
        if follow.is_none() {
            return None;
        }
        Some(follow.as_ref().unwrap().clone())
    }

    pub(crate) fn following(&self) -> Option<Rc<Body>> {
        self.follow.borrow().clone()
    }

    pub(crate) fn needs_talk_update(&self, _now: f64) -> Option<Rc<Body>> {
        let talk_to = self.talk_to.borrow();
        if talk_to.is_none() {
            return None;
        }
        Some(talk_to.as_ref().unwrap().clone())
    }

    pub(crate) fn needs_attack_update(&self) -> Option<Rc<Body>> {
        let attack = self.attack.borrow();
        if attack.is_none() {
            return None;
        }
        Some(attack.as_ref().unwrap().clone())
    }

    pub fn patrol(&self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.patrol_goal.set(Some((x1, y1, x2, y2)));
    }

    pub fn set_intel(&self, intel_type: save::IntelType) {
        let mut intel = self.intel.borrow_mut();
        *intel = Some(Intel::new(intel_type));
    }

    pub fn set_team(&self, team: save::Team) {
        self.team.set(Some(team));
    }

    pub fn freeze(&self) {
        self.frozen.set(true);
        self.clear_attack();
        self.clear_talk();
        self.clear_walk_goal();
    }

    pub fn unfreeze(&self) {
        self.frozen.set(false);
    }

    pub fn wander(&self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let mut wanderer = self.wanderer.borrow_mut();
        *wanderer = Some(Wanderer::new(x1, y1, x2, y2));
    }

    pub fn clear_wander(&self) {
        let mut wanderer = self.wanderer.borrow_mut();
        *wanderer = None;
    }

    pub fn set_health_no_max(&self, health: i32) {
        self.health.set(health);
    }

    pub fn set_health(&self, health: i32) {
        let max_health = self.max_health();
        if health < max_health {
            self.health.set(health);
        } else {
            self.health.set(max_health);
        }
    }

    pub fn set_magic(&self, magic: i32) {
        let max_magic = self.max_magic();
        if magic < max_magic {
            self.magic.set(magic);
        } else {
            self.magic.set(max_magic);
        }
    }

    pub fn get_health(&self) -> i32 {
        self.health.get()
    }

    pub fn set_level(&self, level: i32) {
        self.level.set(level);
        self.set_health(i32::MAX);
        self.set_magic(i32::MAX);
    }

    pub fn set_position(&self, x: f64, y: f64) {
        self.set_x(x);
        self.set_y(y);
        self.set_target_x(x);
        self.set_target_y(y);
    }

    /// Motion used for the player. Will walk diagonally to the same row or
    /// column and then walk along that row or column until it hits the target
    pub fn walk_to(&self, target_x: f64, target_y: f64) {
        self.set_target_x(target_x);
        self.set_target_y(target_y);
    }

    pub fn attack(&self, attackee_ref: Rc<Body>) {
        let mut attack = self.attack.borrow_mut();
        *attack = Some(attackee_ref);
    }

    pub(crate) fn next_motion(&self, now: f64) -> Motion {
        let dx = if self.x != self.target_x {
            dir(self.x(), self.target_x())
        } else {
            0.0
        };
        let dy = if self.y != self.target_y {
            dir(self.y(), self.target_y())
        } else {
            0.0
        };
        let start_t = self.motion().and_then(|m| Some(m.end_t)).unwrap_or(now);
        let speed = self.speed() as f64;
        Motion {
            start_t,
            end_t: start_t + speed,
            x0: self.x(),
            y0: self.y(),
            x1: self.x() + dx,
            y1: self.y() + dy,
        }
    }

    pub fn clear_walk_goal(&self) {
        self.motion.set(None);
        self.target_x.set(self.x());
        self.target_y.set(self.y());
        self.set_action_state(ActionState::Idle);
    }

    pub(crate) fn clear_attack(&self) {
        let mut attack = self.attack.borrow_mut();
        *attack = None;
    }

    pub(crate) fn clear_patrol(&self) {
        self.patrol_goal.set(None);
    }

    pub(crate) fn clear_talk(&self) {
        let mut talk_to = self.talk_to.borrow_mut();
        *talk_to = None;
    }

    pub(crate) fn set_motion(&self, motion: Motion) {
        self.motion.set(Some(motion));
    }

    pub(crate) fn update(&self, now: f64) {
        if self.health.get() == 0 {
            self.clear_attack();
            self.clear_walk_goal();
            self.clear_talk();

            if self.death_time() + 1000.0 > now {
                self.set_action_state(ActionState::Dying);
            } else {
                self.set_action_state(ActionState::Dead);
                return;
            }
        }
        if let Some(motion) = self.motion() {
            let (x, y) = motion.tween(now);
            self.set_x(x);
            self.set_y(y);
            if now >= motion.end_t {
                self.motion.set(None);
                self.set_action_state(ActionState::Idle);
            } else {
                self.set_action_state(ActionState::Walk);
            }
        } else if self.attack.borrow().is_none() && self.wanderer.borrow().is_some() {
            let mut wanderer = self.wanderer.borrow_mut();
            let (x, y) = wanderer
                .as_mut()
                .unwrap()
                .update(now, self.x().floor(), self.y().floor());

            self.walk_to(x, y);
        }
        let mut battle_events = self.battle_events.borrow_mut();
        battle_events.retain(|event| {
            // This one battle event is shown for just one frame
            if event.kind == BattleEventType::Condition1 {
                return event.time + 200.0 > now;
            }
            event.time + 600.0 > now
        });
        drop(battle_events);

        let mut conditions = self.conditions.borrow_mut();
        for condition in conditions.iter() {
            condition.update(self, now);
        }
        conditions.retain(|condition| {
            let finished = condition.finished(now);
            if finished {
                let cond: &str = condition.kind.into();
                aldon_log!("*{} has worn off.*", cond);
            }
            !finished
        });
        let mut pet = self.pet.borrow_mut();
        if pet.is_some() {
            if pet.as_ref().unwrap().get_health() <= 0 {
                *pet = None;
            }
        }
        let mut quest_pet = self.quest_pet.borrow_mut();
        if quest_pet.is_some() {
            if quest_pet.as_ref().unwrap().get_health() <= 0 {
                *quest_pet = None;
            }
        }
        let mut summoned_pet = self.summoned_pet.borrow_mut();
        if summoned_pet.is_some() {
            if summoned_pet.as_ref().unwrap().get_health() <= 0 {
                *summoned_pet = None;
            }
        }
        if let Some(revert) = self.revert_action_state.get() {
            if now >= revert.deadline {
                self.set_action_state(revert.state);
            }
        }
    }

    pub fn equip(&self, now: f64, body: Rc<Body>) -> bool {
        let inventory = self.inventory.borrow();

        let contains_body = inventory.iter().find(|b| Rc::ptr_eq(&body, b)).is_some();

        if !contains_body {
            return false;
        }
        let prop_id = body.prop_id;
        let prop = &PROPS[&prop_id.to_string()];

        let Some(mut equip_to) = prop.can_equip(self.class.get(), self.level()) else {
            return false;
        };
        let mut equiped = self.equiped.borrow_mut();

        // Hack to wear multiple rings
        if equip_to == EquipType::Ring1 && equiped.contains_key(&equip_to) {
            equip_to = EquipType::Ring2
        }
        if equiped.contains_key(&equip_to) {
            return false;
        }
        equiped.insert(equip_to, body.clone());

        if let Some(condition) = condition::for_item(now, body) {
            self.add_condition_no_log(condition);
        }
        true
    }

    /// Equip but ignore the rules, useful for monsters
    pub fn force_equip(&self, now: f64, body: Rc<Body>) {
        let prop_id = body.prop_id;
        let prop = &PROPS[&prop_id.to_string()];

        let Some(equip_to) = prop.equip_type() else {
            return;
        };
        let mut equiped = self.equiped.borrow_mut();
        equiped.insert(equip_to, body.clone());

        if let Some(condition) = condition::for_item(now, body) {
            self.add_condition(condition);
        }
    }

    pub fn unequip(&self, body: Rc<Body>) -> bool {
        let mut equiped = self.equiped.borrow_mut();
        let remove_key = equiped
            .iter()
            .find(|(_, b)| Rc::ptr_eq(&body, b))
            .map(|(key, _)| *key);

        let Some(key) = remove_key else {
            return false;
        };
        equiped.remove(&key);
        true
    }

    pub fn relinquish(&self, body: Rc<Body>) -> bool {
        let equiped = self.equiped.borrow();
        let equip_type = equiped.iter().find(|(_, b)| Rc::ptr_eq(&body, b));

        // can't drop an equiped item
        if equip_type.is_some() {
            return false;
        }
        // TODO: check if item can is droppable (some items cannot be dropped)
        let mut inventory = self.inventory.borrow_mut();
        let idx = inventory.iter().position(|b| Rc::ptr_eq(&body, b));
        if let Some(i) = idx {
            inventory.remove(i);
            js::log("droppable");
            true
        } else {
            js::log("cannot drop");
            false
        }
    }

    pub fn give_item(&self, prop_id: u16) -> Rc<Body> {
        let prop = &PROPS[&prop_id.to_string()];
        let inner_item = Body::new(
            prop.name.clone(),
            None, /* actor_id */
            prop_id,
            0.0,
            0.0,
        );
        let item = Rc::new(inner_item);
        self.give_body_item(item.clone());
        if self.is_player() {
            aldon_log!("(you receive a {}.)", prop.name);
        }
        item
    }

    pub fn give_gold(&self, gold: i32) {
        self.gold.set(self.gold.get() + gold);
        if self.is_player() {
            aldon_log!("(you receive {} gold.)", gold);
        }
    }

    pub fn give_exp(&self, exp: i32) {
        let mut exp = exp;
        let body = self.pet.borrow();
        if body.is_some() {
            let body = body.as_ref().unwrap();
            exp = exp / 2;
            body.give_exp(exp);
            let level = stats::max_level(body.exp());
            if body.level() < level {
                body.set_level(level);
                aldon_log!("*Pet {} gained a level!*", body.name);
            }
        }
        self.exp.set(self.exp.get() + exp);
        if self.is_player() {
            aldon_log!("(you receive {} exp.)", exp);
        }
    }

    pub(crate) fn is_player(&self) -> bool {
        matches!(self.actor_id, Some(0))
    }

    pub fn give_body_item(&self, body: Rc<Body>) {
        let mut inventory = self.inventory.borrow_mut();
        if body.groupable() {
            for item in inventory.iter() {
                js::log(&item.name);
                let item = item;
                if item.prop_id == body.prop_id {
                    item.quantity.set(min(item.quantity.get() + 1, 10));
                    return;
                }
            }
        }
        inventory.push(body);
    }

    pub fn item_quantity(&self, prop_id: u16) -> u8 {
        for item in self.inventory.borrow().iter() {
            if item.prop_id == prop_id {
                return item.quantity();
            }
        }
        return 0;
    }

    pub fn inventory_len(&self) -> usize {
        self.inventory.borrow().len()
    }

    pub fn take_item(&self, prop_id: u16) {
        let mut inventory = self.inventory.borrow_mut();
        let Some((idx, body)) = inventory
            .iter()
            .enumerate()
            .find(|(_i, b)| b.prop_id == prop_id)
        else {
            return;
        };
        if body.groupable() {
            body.quantity.set(body.quantity.get() - 1);
            if body.quantity() == 0 {
                inventory.remove(idx);
            }
        } else {
            self.unequip(body.clone());
            inventory.remove(idx);
        }
        if self.is_player() {
            let prop = &PROPS[&prop_id.to_string()];
            aldon_log!("(you lost your {}.)", prop.name);
        }
    }

    pub fn is_equiped(&self, body: Rc<Body>) -> Option<EquipType> {
        self.equiped
            .borrow()
            .iter()
            .find(|(_, b)| Rc::ptr_eq(&body, b))
            .map(|(key, _)| *key)
    }

    pub fn has_item(&self, prop_id: u16) -> bool {
        self.inventory
            .borrow()
            .iter()
            .find(|b| b.prop_id == prop_id)
            .is_some()
    }

    pub(crate) fn heal(&self, delta: i32) {
        let delta = self.heal_no_log(delta);
        if delta != 0 {
            aldon_log!("*{} gains {} health*", self.name, delta);
        }
    }

    pub(crate) fn heal_no_log(&self, delta: i32) -> i32 {
        // Don't heal if already dead
        if self.health.get() <= 0 {
            return 0;
        }
        let delta = min(self.max_health() - self.health.get(), delta);
        if delta == 0 {
            return 0;
        }
        self.health.set(self.health.get() + delta);
        delta
    }

    pub(crate) fn cure_poison(&self) {
        self.conditions
            .borrow_mut()
            .retain(|c| c.kind != save::ConditionType::POISON);

        aldon_log!("*Poison has worn off.*");
    }

    pub(crate) fn add_condition(&self, condition: Condition) {
        let kind = condition.kind.clone();
        let condition_str: String = (&condition).into();
        self.add_condition_no_log(condition);
        if matches!(kind, ConditionType::SNEAKING | ConditionType::HIDDEN) {
            return;
        }
        aldon_log!("*{} recieves {}*", self.name, condition_str);
    }

    pub(crate) fn add_condition_no_log(&self, condition: Condition) {
        let mut conditions = self.conditions.borrow_mut();
        conditions.retain(|c| {
            !(c.kind == condition.kind
                && c.source == condition.source
                // Potions and player effects are idempotent
                && matches!(c.source, save::ConditionSource::PLAYER | save::ConditionSource::POTION))
        });
        conditions.push(condition);
    }

    /// Returns true if conditions were removed
    pub(crate) fn remove_condition(&self, kind: save::ConditionType) -> bool {
        let mut conditions = self.conditions.borrow_mut();
        let len_before = conditions.len();
        conditions.retain(|c| c.kind != kind);

        len_before != conditions.len()
    }

    pub(crate) fn battle_event(&self, now: f64, event: BattleEventType) {
        self.battle_events
            .borrow_mut()
            .push(BattleEvent::new(now, event));
    }

    // TODO: maybe check on_death here so enemies that die from poison
    // will reward the player?
    pub(crate) fn take_attack(
        &self,
        now: f64,
        battle_event: BattleEventType,
        damage: i32,
        maybe_attacker: Option<Rc<Body>>,
    ) {
        let health = max(0, self.health.get() - damage);
        self.health.set(health);
        self.battle_event(now, battle_event);

        if self.health.get() <= 0 {
            self.death_time.set(now);
        }
        if battle_event == BattleEventType::Hit && damage > 0 {
            aldon_log!("-{} takes {} dmg-", self.name, damage);
        }
        if self.needs_attack_update().is_some() {
            // don't attack back if you're already attacking something
            return;
        }
        if !self.is_player() && self.attack.borrow().is_none() {
            if let Some(attacker) = maybe_attacker {
                self.attack(attacker);
            }
        }
    }

    pub fn follow(&self, body: Rc<Body>) {
        *self.follow.borrow_mut() = Some(body);
    }

    pub fn clear_follow(&self) {
        *self.follow.borrow_mut() = None;
    }

    // TODO: this should be in the static resource
    pub fn groupable(&self) -> bool {
        match self.prop_id {
            101 | 180 | 19 | 210 | 211 | 213 | 214 | 315 | 316 | 317 | 321 | 365 | 366 | 367
            | 368 | 405 | 89 | 91 | 92 | 93 => true,
            _ => false,
        }
    }

    pub fn max_health(&self) -> i32 {
        let base = match self.class.get() {
            save::ClassType::FIGHTER => 10,
            save::ClassType::SPELLCASTER => 5, // TODO: is this right?
            save::ClassType::PRIEST => 6,      // TODO: is this right?
            save::ClassType::THIEF => 6,       // TODO: is this right?
            save::ClassType::JOURNEYMAN => 6,  // TODO: is this right?
            _ => 0,
        };
        let bonus = stats::vitality_to_hit_points(self.vitality());
        let health = (base + bonus) * self.level();
        max(health, 1)
    }

    pub fn class(&self) -> save::ClassType {
        self.class.get()
    }

    pub fn set_class(&self, class: save::ClassType) {
        self.class.set(class);
        self.set_health(i32::MAX);
    }

    /// Reward a player (and maybe their pet) for killing a monster
    pub(crate) fn monster_reward(&self, prop_id: u16, level: i32) {
        let (mut exp, gp) = monster_reward(prop_id, level);

        let pet = self.pet();
        if pet.is_some() {
            let pet = pet.unwrap();
            exp = exp / 2;
            pet.give_exp(exp);
            let level = stats::max_level(pet.exp());

            if pet.level() < level {
                pet.set_level(level);
                aldon_log!("*Pet {} gained a level!*", pet.name);
            }
        }
        aldon_log!("(you receive {}xp and {}gp.)", exp, gp);
        self.exp.set(self.exp.get() + exp);
        self.gold.set(self.gold.get() + gp);
    }

    pub fn sneak(&self, now: f64) {
        let chance = stats::sneak_chance(self.dexterity(), self.level());
        let mut rng = rand::thread_rng();
        let roll: i32 = rng.gen_range(1..=100);
        aldon_log!("*Chance:{} Roll: {}*", chance, roll);

        if roll >= chance {
            aldon_log!("*You fail to sneak.*");
            return;
        }
        aldon_log!("*You begin sneaking.*");
        let c = condition::sneaking(now, chance);
        self.add_condition(c);
    }

    pub fn reveal(&self) {
        let removed = self.remove_condition(save::ConditionType::SNEAKING)
            || self.remove_condition(save::ConditionType::HIDDEN);

        if removed {
            aldon_log!("*You are revealed*");
        }
    }

    pub fn hide(&self) {
        let chance = stats::hide_chance(self.dexterity(), self.level());
        let mut rng = rand::thread_rng();
        let roll: i32 = rng.gen_range(1..=100);
        aldon_log!("*Chance:{} Roll: {}*", chance, roll);
        if roll >= chance {
            aldon_log!("*You fail to hide yourself.*");
            return;
        }
        aldon_log!("*You have hidden yourself*");
        let c = condition::hidden();
        self.add_condition(c);
    }

    /// The spells available to cast
    pub fn spells(&self) -> Vec<u16> {
        SPELLS
            .iter()
            .filter(|(_, s)| s.class() == self.class() && s.level <= self.level())
            .map(|(spell_id, _)| match spell_id.parse::<u16>() {
                Ok(n) => n,
                Err(e) => panic!("parse spell_id: {}", e),
            })
            .collect()
    }

    /// Attempts to cast a spell, returns true on success. This may fail if the spell hasn't cooled
    /// down yet, out of mana, or fizzles.
    pub(crate) fn try_spell(&self, now: f64, spell_id: u16) -> bool {
        if !self.can_cast_spell(now, spell_id) {
            return false;
        }
        let spell = &SPELLS[&spell_id.to_string()];
        if self.magic() < spell.cost {
            aldon_log!("-Out of mana.-");
            return false;
        }
        self.set_magic(self.magic() - spell.cost);
        let cast_success = min(
            stats::intelligence_to_chance_cast(self.inteligence())
                + stats::luck_to_modifier(self.luck()),
            99,
        );
        let mut rng = rand::thread_rng();
        let roll: i32 = rng.gen_range(0..=99);
        aldon_log!("-{} needs < {}, rolls {}-", self.name, cast_success, roll);

        if roll >= cast_success {
            aldon_log!("-{} fizzles-", self.name);
            return false;
        }
        self.set_action_state(ActionState::Attack);
        let revert = RevertActionState {
            deadline: now + 600.0,
            state: ActionState::Idle,
        };
        self.revert_action_state.set(Some(revert));
        true
    }

    /// Returns true if enough time has elapsed to cast a spell again
    fn can_cast_spell(&self, now: f64, spell_id: u16) -> bool {
        if let Some(spell) = self.last_spell.get() {
            if spell.spell_id == spell_id && spell.time + 1500.0 > now {
                return false;
            }
        }
        let last_spell = CastSpell {
            spell_id,
            time: now,
        };
        self.last_spell.set(Some(last_spell));
        true
    }

    pub fn max_magic(&self) -> i32 {
        if matches!(
            self.class(),
            save::ClassType::FIGHTER | save::ClassType::THIEF | save::ClassType::JOURNEYMAN
        ) {
            return 0;
        }
        stats::wisdom_to_mana(self.wisdom()) * self.level()
    }

    pub fn stats(&self) -> PlayerStats {
        let race = self
            .race
            .get()
            .map_or(PROPS[&self.prop_id.to_string()].name.clone(), |r| r.into());

        let class = if self.is_player() {
            self.class.get().into()
        } else {
            "NPC".to_string()
        };
        PlayerStats {
            name: self.name.clone(),
            class,
            race,
            level: self.level(),
            hp: self.health.get(),
            hp_max: self.max_health(),
            ac: self.armor_class(),
            exp: self.exp(),
            mp: self.magic.get(),
            mp_max: self.max_magic(),
            gp: self.gold.get(),
            str: self.strength(),
            int: self.inteligence(),
            dex: self.dexterity(),
            vit: self.vitality(),
            wis: self.wisdom(),
            luck: self.luck(),
            portrait: self.portrait_id.get().unwrap(),
        }
    }
}

/// An ActionState to revert to after the deadline
#[derive(Debug, Copy, Clone)]
struct RevertActionState {
    deadline: f64,
    state: ActionState,
}

/// TODO: I think the origional game had something like this but the way I ended up implementing
/// things here this could just be part of Body
#[derive(Debug)]
pub struct Intel {
    items: RefCell<Vec<Rc<Body>>>,
    msg_id: Cell<Option<u16>>,
    pub(crate) kind: Cell<save::IntelType>,
}

impl Intel {
    fn new(intel_type: save::IntelType) -> Self {
        Self {
            items: RefCell::new(Vec::new()),
            msg_id: Cell::new(None),
            kind: Cell::new(intel_type),
        }
    }

    pub fn set_message(&self, msg_id: u16) -> Result<(), ActorError> {
        self.msg_id.set(Some(msg_id));
        Ok(())
    }

    pub fn take_message(&self) -> Option<u16> {
        self.msg_id.take()
    }

    pub fn has_message(&self) -> bool {
        self.msg_id.get().is_some()
    }

    pub fn add_sell_item(&self, prop_id: u16) -> Result<(), ActorError> {
        let prop = &PROPS[&prop_id.to_string()];
        let inner_item = Body::new(
            prop.name.clone(),
            None, /* actor_id */
            prop_id,
            0.0,
            0.0,
        );
        let item = Rc::new(inner_item);
        self.items.borrow_mut().push(item);
        Ok(())
    }

    pub fn pop_transaction(&self) -> Result<Vec<Rc<Body>>, ActorError> {
        Ok(self.items.borrow_mut().drain(..).collect())
    }
}

/// Behavior for a body that wanders around the map
#[derive(Debug)]
struct Wanderer {
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    deadline: f64,
    rng: ThreadRng,
}

impl Wanderer {
    fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Self {
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            deadline: 0.0,
            rng: rand::thread_rng(),
        }
    }

    fn update(&mut self, now: f64, x: f64, y: f64) -> (f64, f64) {
        if now < self.deadline {
            return (x, y);
        }
        if self.rng.gen_ratio(1, 2) {
            self.deadline = now + self.rng.gen_range(1000.0..1500.0);
            return (x, y);
        }
        let mut dx = self.rng.gen_range(-1..=1) as f64;
        if x + dx > self.x_max || x + dx < self.x_min {
            dx = 0.0;
        }
        let mut dy = self.rng.gen_range(-1..=1) as f64;
        if y + dy > self.y_max || y + dy < self.y_min {
            dy = 0.0;
        }
        (x + dx, y + dy)
    }

    fn save(&self, now: f64) -> save::Wanderer {
        save::Wanderer::new(
            OrderedFloat::from(self.x_min),
            OrderedFloat::from(self.y_min),
            OrderedFloat::from(self.x_max),
            OrderedFloat::from(self.y_max),
            OrderedFloat::from(self.deadline - now),
        )
    }

    fn from_save(save: &save::Wanderer, now: f64) -> Result<Self, InvalidDataError> {
        let x_min = *save
            .x_min
            .ok_or(InvalidDataError::new("x_min field missing"))?;

        let y_min = *save
            .y_min
            .ok_or(InvalidDataError::new("y_min field missing"))?;

        let x_max = *save
            .x_max
            .ok_or(InvalidDataError::new("x_max field missing"))?;

        let y_max = *save
            .y_max
            .ok_or(InvalidDataError::new("y_max field missing"))?;

        let rest_time = *save
            .rest_time
            .ok_or(InvalidDataError::new("rest_time field missing"))?;

        let wanderer = Self {
            x_min,
            y_min,
            x_max,
            y_max,
            deadline: now + rest_time,
            rng: rand::thread_rng(),
        };
        Ok(wanderer)
    }
}

#[derive(Debug, Copy, Clone)]
struct CastSpell {
    spell_id: u16,
    time: f64,
}

impl CastSpell {
    fn from_save(save: &save::CastSpell, now: f64) -> Result<Self, InvalidDataError> {
        let spell_id: u16 = save
            .spell_id
            .ok_or(InvalidDataError::new("spell_id field missing"))?
            .try_into()
            .map_err(|_| InvalidDataError::new("spell_id field not u16"))?;

        let delay = *save
            .delay
            .ok_or(InvalidDataError::new("delay field missing"))?;

        let result = Self {
            spell_id,
            time: now + delay,
        };
        Ok(result)
    }

    fn save(&self, now: f64) -> save::CastSpell {
        save::CastSpell::new(self.spell_id as i32, OrderedFloat::from(self.time - now))
    }
}

/// A hint to the renderer how to draw the body
#[derive(Debug, Copy, Clone)]
pub enum ActionState {
    Attack,
    Idle,
    Dying,
    Dead,
    Walk,
}

impl Into<&'static str> for RaceType {
    fn into(self) -> &'static str {
        match self {
            RaceType::HUMAN => "Human",
            RaceType::ELF => "Elf",
            RaceType::DWARF => "Dwarf",
            _ => "Unknown",
        }
    }
}

impl Into<String> for RaceType {
    fn into(self) -> String {
        let result: &'static str = self.into();
        result.to_string()
    }
}

const PET_NAMES: [&str; 25] = [
    // Original names:
    "Chuckles",
    "Spencer",
    "Xyla",
    "Gracie",
    "Barney",
    "Fido",
    "Tobie",
    "Sterling",
    "Fluffy",
    // End original names
    "Tan the Man",
    "Nova",
    "Mr. Train",
    "Richard",
    "Bobert",
    "Nugget",
    "Doc",
    "Ray Mundo",
    "Big Dan",
    "Rach",
    "Vermanator",
    "Mrs. Corn",
    "Taurus",
    "Leo",
    "Nimby",
    "Riker",
];

pub fn pet_name() -> &'static str {
    let mut rng = rand::thread_rng();
    PET_NAMES.choose(&mut rng).unwrap()
}
