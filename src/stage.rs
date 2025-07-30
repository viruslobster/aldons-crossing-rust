//! The loaded map where bodies interact with eachother
use crate::{
    aldon_log,
    body::{ActionState, Body},
    combat::{
        self, make_attack, Attack, BattleEventType, Missile, MissileEffect, MissileInfo,
        MissileType, Motion, Strike,
    },
    condition::{self},
    data::{self, PropTypeRes, SpawnerRes, SpellTarget, PROPS, SPELLS, WORLD},
    game::{Dialog, InvalidDataError, CONSOLE},
    js,
    search::search_path,
    thrift::{
        save::{self, RaceType, Team, TrapKind},
        util::{box_vec, unbox_vec},
    },
};
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    fmt::Write,
    rc::Rc,
    vec::Vec,
};
use thrift::OrderedFloat;

const IMPASSIBLE_TILES: [u8; 7] = [3, 0, 4, 1, 5, 2, 14];
const SIGHT_BLOCKER_TILES: [u8; 5] = [3, 0, 1, 2, 14];

pub(crate) struct Stage {
    map_id: Cell<u16>,
    pub(crate) map: Cell<&'static data::MapRes>,

    bodies: RefCell<Vec<Rc<Body>>>,
    spawners: RefCell<Vec<Spawner>>,
    missiles: RefCell<Vec<Missile>>,
    traps: RefCell<Vec<Trap>>,
    occupancy: RefCell<Occupancy>,
    dialog: Rc<dyn Dialog>,
    player_has_moved: Cell<bool>,
    player_start_position: Cell<(f64, f64)>,
    now: Cell<f64>,

    // Actors that die in the current session are added here.  Does not
    // contain all actors to die ever.
    dead_actors: RefCell<HashSet<u16>>,
}

impl Stage {
    pub fn new(map_id: u16, dialog: Rc<dyn Dialog>) -> Self {
        let map = &data::WORLD.maps[&map_id.to_string()];
        Self {
            map_id: Cell::new(map_id),
            map: Cell::new(map),
            bodies: RefCell::new(Vec::new()),
            spawners: RefCell::new(Vec::new()),
            occupancy: RefCell::new(Occupancy::new()),
            missiles: RefCell::new(Vec::new()),
            traps: RefCell::new(Vec::new()),
            dialog,
            dead_actors: RefCell::new(HashSet::new()),
            player_has_moved: Cell::new(false),
            player_start_position: Cell::new((0.0, 0.0)),
            now: Cell::new(0.0),
        }
    }

    pub fn now(&self) -> f64 {
        self.now.get()
    }

    pub fn bodies(&self) -> Vec<Rc<Body>> {
        self.bodies.borrow().clone()
    }

    pub fn traps(&self) -> Vec<Trap> {
        self.traps.borrow().clone()
    }

    pub fn from_save(
        now: f64,
        stage_save: &save::Stage,
        dialog: Rc<dyn Dialog>,
    ) -> Result<Self, InvalidDataError> {
        let map_id: u16 = stage_save
            .map_id
            .ok_or(InvalidDataError::new("map_id field missing"))?
            .try_into()
            .map_err(|_| InvalidDataError::new("map_id not valid u16"))?;

        let player_save = stage_save
            .player
            .as_ref()
            .ok_or(InvalidDataError::new("player field missing"))?;

        let inventory_by_id = stage_save
            .inventory_by_id
            .as_ref()
            .ok_or(InvalidDataError::new("inventory_by_id field missing"))?;

        let traps: Vec<Trap> = stage_save
            .traps
            .iter()
            .flatten()
            .map(|t| Trap::try_from(*t.clone()))
            .filter_map(|trap| match trap {
                Ok(t) => Some(t),
                Err(e) => {
                    js::log(&format!("failed to load trap, ignoring: {}", e));
                    None
                }
            })
            .collect();

        let map = &data::WORLD.maps[&map_id.to_string()];
        let mut bodies = Vec::new();
        let player = Rc::new(Body::from_save(now, &player_save)?);
        bodies.push(player.clone());
        let inventory = inventory_by_id
            .get(&0) // player inventory saved with id 0
            .expect("Player has no inventory");

        let inventory = unbox_vec(inventory);
        player.give_inventory(now, &inventory)?;

        if let Some(pet_save) = &stage_save.pet {
            let pet = Body::from_save(now, pet_save)?;
            let inventory = inventory_by_id
                .get(&1) // pet inventory saved with id 1
                .expect("Pet has no inventory");

            let inventory = unbox_vec(inventory);
            pet.give_inventory(now, &inventory)?;
            pet.follow(player.clone());
            let pet = Rc::new(pet);
            player.give_pet(pet.clone());
            bodies.push(pet);
        }
        if let Some(quest_save) = &stage_save.quest_pet {
            let inventory = inventory_by_id
                .get(&2) // quest pet saved with id 2
                .expect("Quest pet has no inventory");
            let pet = Body::from_save(now, quest_save)?;

            let inventory = unbox_vec(inventory);
            pet.give_inventory(now, &inventory)?;
            pet.follow(player.clone());
            let pet = Rc::new(pet);
            player.give_quest_pet(pet.clone());
            bodies.push(pet);
        }
        if let Some(save) = &stage_save.summoned_pet {
            let pet = Body::from_save(now, save)?;
            let inventory = inventory_by_id
                .get(&3) // summoned pet saved with id 3
                .expect("Summoned pet has no inventory");

            let inventory = unbox_vec(inventory);
            pet.give_inventory(now, &inventory)?;
            pet.follow(player.clone());
            let pet = Rc::new(pet);
            player.give_summoned_pet(pet.clone());
            bodies.push(pet);
        }
        let stage = Self {
            map_id: Cell::new(map_id),
            map: Cell::new(map),
            bodies: RefCell::new(bodies),
            spawners: RefCell::new(Vec::new()),
            occupancy: RefCell::new(Occupancy::new()),
            missiles: RefCell::new(Vec::new()),
            dialog,
            dead_actors: RefCell::new(HashSet::new()),
            player_has_moved: Cell::new(false),
            player_start_position: Cell::new((player.x(), player.y())),
            now: Cell::new(now),
            traps: RefCell::new(traps),
        };

        stage.load_map(map_id, true /*from_save*/);
        if let Some(save_bodies) = &stage_save.bodies {
            for save in save_bodies {
                let body = Body::from_save(now, &save)?;
                body.equip_default(now);
                let body = Rc::new(body);
                stage.place_body(body.clone());
                js::log(&format!("loaded body: {} {:?}", body.name, body.actor_id));

                if !save.from_spawner.unwrap_or(false) {
                    continue;
                }
                for spawner in stage.spawners.borrow_mut().iter_mut() {
                    if !spawner.is_match(body.clone()) {
                        continue;
                    }
                    spawner.spawned.push(body.clone());
                }
            }
        } else {
            js::log("There are no bodies!!!");
        }
        Ok(stage)
    }

    pub fn sight(&self) -> [bool; 576] {
        let mut result = [false; 576];
        let map = self.map.get();
        for y in 0..24 {
            for x in 0..24 {
                let i = y * 24 + x;
                let tile = map.tiles[i];
                result[i] |= sight_blocking(tile);
            }
        }
        for body in self.bodies.borrow().iter() {
            let (x, y) = body.moving_to();
            let sight_blocking = PROPS[&body.prop_id.to_string()].sight_blocker;
            let i = y.floor() * 24.0 + x.floor();
            result[i as usize] |= sight_blocking;
        }
        result
    }

    pub fn map_id(&self) -> u16 {
        self.map_id.get()
    }

    pub fn player_has_moved(&self) -> bool {
        self.player_has_moved.get()
    }

    pub fn died(&self, actor_id: u16) -> bool {
        self.dead_actors.borrow().contains(&actor_id)
    }

    pub fn save(&self, now: f64) -> save::Stage {
        let player = self.get_player();
        let pet = player.pet();
        let quest_pet = player.quest_pet();
        let summoned_pet = player.summoned_pet();

        let mut inventory_by_id = BTreeMap::new();
        inventory_by_id.insert(0, player.save_inventory(now));
        if let Some(p) = &pet {
            inventory_by_id.insert(1, p.save_inventory(now));
        }
        if let Some(p) = &quest_pet {
            inventory_by_id.insert(2, p.save_inventory(now));
        }
        if let Some(p) = &summoned_pet {
            inventory_by_id.insert(3, p.save_inventory(now));
        }
        let pets = player.henchmen();

        let bodies: Vec<Box<save::Body>> = self
            .bodies
            .borrow()
            .iter()
            .filter(|b| b.persist.get())
            .filter(|b| !b.is_player())
            .filter(|b| !pets.iter().any(|p| Rc::ptr_eq(b, p)))
            .map(|b| Box::new(b.save(now)))
            .collect();

        let traps: Vec<save::Trap> = self.traps.borrow().iter().map(|&t| t.into()).collect();

        save::Stage::new(
            self.map_id.get() as i32,
            Box::new(player.save(now)),
            inventory_by_id,
            pet.map(|p| Box::new(p.save(now))),
            quest_pet.map(|p| Box::new(p.save(now))),
            summoned_pet.map(|p| Box::new(p.save(now))),
            bodies,
            box_vec(&traps),
        )
    }

    pub fn missiles(&self) -> Vec<MissileInfo> {
        self.missiles.borrow().iter().map(|m| m.info()).collect()
    }

    pub fn pick_up_at(&self, x: f64, y: f64) -> Vec<Rc<Body>> {
        let mut bodies = Vec::new();
        for body in self.bodies.borrow().iter() {
            let prop = &PROPS[&body.prop_id.to_string()];
            match prop.kind {
                PropTypeRes::Creature { .. }
                | PropTypeRes::User { .. }
                | PropTypeRes::Physical { .. }
                | PropTypeRes::Animprop { .. } => {
                    continue;
                }
                _ => {}
            }
            if body.x() == x && body.y() == y {
                bodies.push(body.clone());
            }
        }
        bodies
    }

    pub fn enemy_at(&self, x: f64, y: f64) -> Option<Rc<Body>> {
        for body in self.bodies.borrow().iter() {
            let prop = &PROPS[&body.prop_id.to_string()];
            if !matches!(
                prop.kind,
                PropTypeRes::Creature { .. } | PropTypeRes::User { .. }
            ) {
                continue;
            }
            let (body_x, body_y) = body.moving_to();
            if body_x == x
                && body_y == y
                && body.team() == Some(save::Team::ENEMY)
                && body.get_health() > 0
            {
                return Some(body.clone());
            }
        }
        None
    }

    pub fn corpse_at(&self, x: f64, y: f64) -> Option<Rc<Body>> {
        for body in self.bodies.borrow().iter() {
            let prop = &PROPS[&body.prop_id.to_string()];
            if !matches!(
                prop.kind,
                PropTypeRes::Creature { .. } | PropTypeRes::User { .. }
            ) {
                continue;
            }
            let (body_x, body_y) = body.moving_to();
            if body_x == x && body_y == y {
                return Some(body.clone());
            }
        }
        None
    }

    pub fn friend_at(&self, x: f64, y: f64) -> Option<Rc<Body>> {
        let mut friends = self.get_player().henchmen();
        friends.push(self.get_player());
        js::log(&format!("finding friend at {}, {}", x, y));

        for body in self.bodies.borrow().iter() {
            let (body_x, body_y) = body.moving_to();
            if body_x != x || body_y != y || body.get_health() <= 0 {
                continue;
            }
            let friend = friends.iter().any(|b| Rc::ptr_eq(body, b));
            if friend {
                js::log(&format!("found {} at {}, {}", body.name, x, y));
                return Some(body.clone());
            }
        }
        None
    }

    pub fn input(&self, x: f64, y: f64) {
        let player = self.get_player();
        if player.frozen() {
            return;
        }
        player.clear_attack();
        player.clear_talk();

        for body in self.bodies.borrow().iter() {
            if rect_contains(body.x(), body.y(), 1.0, 1.0, x, y) {
                let interacted = self.player_interaction(body.clone());
                if interacted {
                    return;
                }
            }
        }

        player.walk_to(x.floor(), y.floor());
    }

    fn player_interaction(&self, body2: Rc<Body>) -> bool {
        if body2.intel.borrow().is_none() {
            return false;
        }
        if body2.health.get() == 0 {
            return false;
        }
        let player = self.get_player();
        let intel = body2.intel.borrow();
        let intel = intel.as_ref().unwrap();

        match (intel.kind.get(), &body2.team()) {
            (_, Some(save::Team::ENEMY)) => {
                js::log("attack!");
                player.attack(body2.clone());
                true
            }
            (
                save::IntelType::GUILD_MASTER
                | save::IntelType::NPC
                | save::IntelType::MESSAGE_BEARER,
                _,
            ) => {
                js::log("talk!");
                if intel.has_message() {
                    player.talk_to(body2.clone());
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn body_distance(&self, body1: &Body, body2: &Body) -> f64 {
        (body1.x() - body2.x()).powf(2.0) + (body1.y() - body2.y()).powf(2.0)
    }

    fn nearest_enemy(&self, x: f64, y: f64, enemy_team: save::Team) -> Option<Rc<Body>> {
        let mut nearest_ref: Option<Rc<Body>> = None;
        let mut nearest_dist: Option<f64> = None;

        for body in self.bodies.borrow().iter() {
            let skip = body.health.get() == 0
                || body.team().is_none()
                || enemy_team != body.team().unwrap()
                || body.hidden()
                || body.sneaking();

            if skip {
                continue;
            }
            let dist = distance(x, y, body.x(), body.y());
            if dist > 2.0 {
                continue;
            }
            if let Some(nearest) = nearest_dist {
                if dist < nearest {
                    nearest_dist = Some(dist);
                    nearest_ref = Some(body.clone());
                }
            } else {
                nearest_dist = Some(dist);
                nearest_ref = Some(body.clone());
            }
        }
        nearest_ref
    }

    fn nearby_teammates(&self, body1: &Body) -> Vec<Rc<Body>> {
        let mut result = Vec::new();

        for body2 in self.bodies.borrow().iter() {
            let skip =
                body2.health.get() == 0 || body2.team().is_none() || body2.team() != body1.team();

            if skip {
                continue;
            }
            let dist = distance(body1.x(), body1.y(), body2.x(), body2.y());
            if dist > 2.0 {
                continue;
            }
            result.push(body2.clone());
        }
        result
    }

    pub fn set_actor_body_loc(&self, actor_id: u16, x: f64, y: f64) {
        let body = self.get_body(actor_id).unwrap();
        let (_old_x, _old_y) = if let Some(motion) = &body.motion() {
            (motion.x1, motion.y1)
        } else {
            (body.x(), body.y())
        };
        body.set_x(x);
        body.set_y(y);
        body.clear_walk_goal();
        body.clear_attack();
    }

    pub fn teleporter_at(&self, x: f64, y: f64) -> Option<(u16, f64, f64)> {
        let map = self.map.get();
        for teleport in &map.teleports {
            if x == teleport.from_x && y == teleport.from_y {
                return Some((teleport.id, teleport.to_x, teleport.to_y));
            }
        }
        None
    }

    fn on_death(&self, attacker: &Body, attackee: &Body) {
        aldon_log!("-{} dies...-", attackee.name);
        attacker.set_action_state(ActionState::Idle);
        attacker.clear_attack();

        if attacker.is_player() {
            attacker.monster_reward(attackee.prop_id, attackee.level());
        } else if attacker.is_pet() {
            let attackee_prop_id = attackee.prop_id;
            self.get_player()
                .monster_reward(attackee_prop_id, attackee.level());
        }
        if attackee.is_player() {
            aldon_log!("*Game Over! press menu to continue*");
        }
    }

    pub fn update(&self, now: f64) {
        self.now.set(now);

        if !self.player_has_moved.get() {
            let pos = self.get_player().moving_from();
            if pos != self.player_start_position.get() {
                self.player_has_moved.set(true);
            }
        }

        let mut bodies = self.bodies.borrow_mut();
        bodies.retain(|body| !(body.health.get() == 0 && body.death_time() + 10000.0 < now));

        let mut occupancy = self.occupancy.borrow_mut();
        occupancy.reset();

        for body in bodies.iter() {
            let prop = &PROPS[&body.prop_id.to_string()];
            if prop.blocker && body.health.get() != 0 {
                let (x, y) = body.moving_to();
                occupancy.occupy(x, y);
            }
        }
        for spawner in self.spawners.borrow_mut().iter_mut() {
            if let Some(body) = spawner.update(now, &occupancy) {
                bodies.push(body);
            }
        }
        // other methods used here need to borrow
        drop(bodies);

        for missile in self.missiles.borrow_mut().iter_mut() {
            missile.update(now);
        }
        let mut new_missiles = vec![];

        for missile in self.missiles.borrow().iter() {
            if !missile.finished(now) {
                continue;
            }
            let attacker = missile.attacker();
            attacker.reveal();

            for effect in missile.effects() {
                match effect {
                    MissileEffect::Strike(Strike {
                        damage,
                        event,
                        target,
                    }) => {
                        target.take_attack(now, *event, *damage, Some(attacker.clone()));
                        // TODO: check this automatically for all targets
                        if target.get_health() == 0 {
                            self.on_death(&attacker, target);
                        }
                    }
                    MissileEffect::SplashDamage { amount } => {
                        let Some(target) = self.enemy_at(missile.x().floor(), missile.y().floor())
                        else {
                            continue;
                        };
                        target.take_attack(
                            now,
                            BattleEventType::Hit,
                            *amount,
                            Some(attacker.clone()),
                        );
                        // TODO: check this automatically for all targets
                        if target.get_health() == 0 {
                            self.on_death(&attacker, &target);
                        }
                    }
                    MissileEffect::Explosion {
                        splash_damage,
                        splash_kind,
                        size,
                    } => {
                        let boom = explosion(
                            now,
                            missile.x(),
                            missile.y(),
                            attacker.clone(),
                            *splash_damage,
                            splash_kind.clone(),
                            size.clone(),
                        );
                        new_missiles.extend(boom);
                    }
                    MissileEffect::Heal {
                        target,
                        amount,
                        show_animation,
                    } => {
                        target.heal(*amount);
                        if *show_animation {
                            target.battle_event(now, BattleEventType::Condition2);
                        }
                    }
                    MissileEffect::Spell { spell_id } => {
                        self.effect_spell(*spell_id, attacker.clone(), missile.x(), missile.y());
                    }
                    MissileEffect::Condition { target, condition } => {
                        target.add_condition(condition.clone());
                        target.battle_event(now, BattleEventType::Condition2);
                    }
                    MissileEffect::AnimateDead => {
                        let player = self.get_player();
                        player.take_summoned_pet();
                        let Some(body) = self.corpse_at(missile.x().floor(), missile.y().floor())
                        else {
                            continue;
                        };
                        // TODO: pretty sure the game makes animated enemies weaker
                        // but not really sure what it does
                        body.set_health(body.max_health() / 2);
                        body.set_team(Team::PLAYER);
                        body.set_enemy(Team::ENEMY);
                        body.clear_wander();
                        body.clear_attack();
                        body.clear_patrol();
                        body.set_action_state(ActionState::Idle);
                        body.follow(player.clone());
                        body.battle_event(now, BattleEventType::Condition2);
                        player.take_summoned_pet().map(|p| self.remove_body_ref(p));
                        player.give_summoned_pet(body);
                    }
                    MissileEffect::CurePoison { target } => {
                        target.cure_poison();
                    }
                    MissileEffect::DetonateCorpse { size } => {
                        let Some(body) = self.corpse_at(missile.x().floor(), missile.y().floor())
                        else {
                            continue;
                        };
                        let (x, y) = body.moving_to();
                        let missiles = explosion(
                            now,
                            x,
                            y,
                            attacker.clone(),
                            24, /* damage */
                            MissileType::Fire,
                            size.clone(),
                        );
                        new_missiles.extend(missiles);
                    }
                }
            }
        }
        self.missiles
            .borrow_mut()
            .retain(|missile| !missile.finished(now));

        self.missiles.borrow_mut().append(&mut new_missiles);
        let player = self.get_player();

        // TODO: This is the area of the code that needs the most attention. Most of this logic should be
        // moved into Body::update but in some previous iteration that was not possible because I
        // was still figuring out rust's ownership and borrowing rules.
        for body in self.bodies.borrow().iter() {
            let mut iter = 0;
            loop {
                iter += 1;
                if iter > 10 {
                    // This logic is not sound and will sometimes loop forever. Just exit if we
                    // detect that is happening. This should be fixed when moving most of the
                    // implementation to Body::update
                    js::log("HACKY BREAK!");
                    break; // super hacky hack but better than crashing
                }
                if body.health.get() == 0 {
                    if let Some(actor_id) = body.actor_id {
                        if !self.dead_actors.borrow().contains(&actor_id) {
                            js::log(&format!("actor {} has died by health", actor_id));
                        }
                        self.dead_actors.borrow_mut().insert(actor_id);
                    }
                }
                body.update(now);

                if body.team() == Some(save::Team::ENEMY) {
                    self.maybe_trigger_trap(player.clone(), body);
                }

                if let Some((_x, _y)) = body.needs_walk_update(now) {
                    if let Some(talkee) = body.needs_talk_update(now) {
                        if body_distance(&body, &talkee) <= 1.0 {
                            // stop before refuse piles when searching them
                            body.clear_walk_goal();
                            break;
                        }
                    }
                    let motion = body.next_motion(now);
                    if occupancy.occupied(motion.x1, motion.y1) {
                        let henchmen = player.henchmen();
                        let body_is_henchmen = henchmen.iter().any(|b| Rc::ptr_eq(b, body));
                        if body_is_henchmen {
                            // Don't let pets switch back and forth in a loop
                            body.clear_walk_goal();
                            break;
                        }
                        if let Some(friend) = self.friend_at(motion.x1, motion.y1) {
                            // Move friendly bodies out of the way. Only move the player
                            // if they are frozen.
                            if (!friend.is_player() || friend.frozen())
                                && !matches!(body.team(), Some(Team::ENEMY))
                            {
                                // If a pet is in the way, trade places so they aren't annoying
                                let inverse = Motion {
                                    x0: motion.x1,
                                    y0: motion.y1,
                                    x1: motion.x0,
                                    y1: motion.y0,
                                    start_t: motion.start_t,
                                    end_t: motion.end_t,
                                };
                                body.set_motion(motion);
                                friend.set_motion(inverse);
                                break;
                            }
                        }
                        body.clear_walk_goal();
                        break;
                    } else {
                        occupancy.vacate(motion.x0, motion.y0);
                        occupancy.occupy(motion.x1, motion.y1);
                        body.set_motion(motion);
                    }
                    if let Some(_) = self.teleporter_at(body.x(), body.y()) {
                        return; // bail so we can teleport farther up in the stack
                    }
                    continue;
                }
                // some actors move the player while frozen
                if body.frozen() {
                    break;
                }
                if body.needs_attack_update().is_none() && body.get_health() > 0 {
                    if let Some(enemy_team) = body.hostile_to.get() {
                        let maybe_enemy = self.nearest_enemy(body.x(), body.y(), enemy_team);
                        if let Some(enemy) = maybe_enemy {
                            body.attack(enemy.clone());
                        }
                    }
                }
                if let Some((x1, y1, x2, y2)) = body.needs_patrol_update(now) {
                    if !body.needs_attack_update().is_some() {
                        if body.x() == x2 && body.y() == y2 {
                            body.patrol(x2, y2, x1, y1);
                        } else {
                            body.walk_to(x2, y2);
                        }
                    }
                }
                if let Some(follow_body) = body.needs_follow_update(now) {
                    let (x0, y0) = body.moving_to();
                    let (x1, y1) = follow_body.moving_from();
                    let dist = distance(x0, y0, x1, y1);
                    let follow = (body.needs_attack_update().is_some() && dist > 2.0)
                        || (body.needs_attack_update().is_none() && dist > 1.0);

                    if follow {
                        let mut special_occupancy = occupancy.clone();
                        for pet in follow_body.henchmen() {
                            let (x, y) = pet.moving_to();
                            special_occupancy.vacate(x, y);
                        }
                        let (x, y, _success) = search_path(&special_occupancy, x0, y0, x1, y1);
                        body.walk_to(x, y);
                        continue;
                    }
                }
                if let Some(talkee) = body.needs_talk_update(now) {
                    if body_distance(&body, &talkee) <= 1.0 {
                        let intel = talkee.intel.borrow();
                        let intel = intel.as_ref().unwrap();
                        let msg_id = intel.take_message();
                        if let Some(id) = msg_id {
                            self.dialog.tell_message(
                                &talkee.name,
                                talkee.portrait_id.get().unwrap(),
                                id,
                                talkee.actor_id.unwrap(),
                            );
                        }
                        body.clear_talk();
                    } else {
                        let x = talkee.x().floor();
                        let y = talkee.y().floor();
                        body.walk_to(x, y);
                    }
                }
                if let Some(attackee) = body.needs_attack_update() {
                    if let Some(follow_body) = body.following() {
                        let (x0, y0) = body.moving_to();
                        let (x1, y1) = follow_body.moving_from();
                        if distance(x0, y0, x1, y1) > 2.0 {
                            break;
                        }
                    }
                    if attackee.get_health() <= 0 {
                        body.set_action_state(ActionState::Idle);
                        body.clear_attack();
                        break;
                    }
                    if body.equiped_weapon().is_none() {
                        body.set_action_state(ActionState::Idle);
                        body.clear_attack();
                        break;
                    }
                    for teammate in self.nearby_teammates(&body) {
                        if teammate.needs_attack_update().is_none() && !teammate.is_player() {
                            teammate.attack(attackee.clone());
                        }
                    }
                    let in_range = if body.is_attack_ranged() {
                        self.body_distance(&body, &attackee) <= 16.0
                    } else {
                        body_distance(&body, &attackee) <= 1.0
                    };
                    if !in_range {
                        let (x1, y1) = body.moving_to();
                        let (x2, y2) = attackee.moving_to();
                        let (x, y, success) = search_path(&occupancy, x1, y1, x2, y2);
                        body.walk_to(x, y);
                        if !success {
                            let maybe_enemy = if body.hostile_to.get().is_some() {
                                self.nearest_enemy(
                                    body.x(),
                                    body.y(),
                                    body.hostile_to.get().unwrap_or(Team::ANIMAL),
                                )
                            } else {
                                None
                            };
                            if let Some(enemy) = maybe_enemy {
                                body.attack(enemy.clone());
                            }
                        }
                        break;
                    }

                    // finish up walking to whatever square and then stop
                    let (x, y) = body.moving_to();
                    body.walk_to(x, y);

                    body.set_action_state(ActionState::Attack);
                    let delay = 2000.0 / 30.0 * body.attack_delay().unwrap();
                    if attackee.get_health() <= 0 {
                        body.set_action_state(ActionState::Idle);
                        break;
                    }
                    if body.last_attack_time() + delay < now {
                        body.set_last_attack_time(now);
                        let attacks = make_attack(now, body.clone(), attackee.clone());
                        for attack in attacks {
                            match attack {
                                Attack::Melee(Strike {
                                    mut damage,
                                    event,
                                    target,
                                }) => {
                                    let sneak_attack = body.sneaking();
                                    body.reveal();
                                    if sneak_attack
                                        && (event == BattleEventType::Hit
                                            || event == BattleEventType::Crit)
                                    {
                                        aldon_log!("*Sneak attack! x4 damage!*");
                                        damage = damage * 4;
                                    }
                                    target.take_attack(now, event, damage, Some(body.clone()));
                                }
                                Attack::Range(missile) => self.missiles.borrow_mut().push(missile),
                            }
                        }
                    }
                    if attackee.health.get() == 0 {
                        self.on_death(&body, &attackee);
                    }
                }
                break;
            }
        }
    }

    pub fn load_map(&self, map_id: u16, from_save: bool) {
        let now = self.now.get();
        let mut occupancy = self.occupancy.borrow_mut();
        *occupancy = Occupancy::new();
        let player = self.get_player();
        if !from_save {
            // game takes your summoned pet every new map
            player.take_summoned_pet();
        }
        let henchmen = player.henchmen();

        for body in &henchmen {
            body.set_x(player.x());
            body.set_y(player.y());
            body.clear_walk_goal();
            body.clear_attack();
        }

        let player_start_position = player.moving_to();
        self.player_has_moved.set(false);
        self.player_start_position.set(player_start_position);

        self.bodies.borrow_mut().retain(|body| {
            let is_henchmen = henchmen.iter().any(|b| Rc::ptr_eq(body, b));
            let is_player = body.actor_id == Some(0);
            is_henchmen || is_player
        });
        player.clear_talk();

        self.map_id.set(map_id);
        let map = &WORLD.maps[&map_id.to_string()];
        self.map.set(map);
        for y in 0..24 {
            for x in 0..24 {
                let tile_id = self.tile_at(x, y).unwrap();
                if impassible(tile_id) {
                    occupancy.occupy_permanent(x as f64, y as f64);
                }
            }
        }
        for placement in &map.props {
            let prop = &PROPS[&placement.id.to_string()];

            let body = self.create_body(
                prop.name.clone(),
                None,
                placement.id,
                placement.x as f64,
                placement.y as f64,
            );
            body.equip_default(now);
        }
        self.traps.borrow_mut().clear();

        *self.spawners.borrow_mut() = map.spawners.iter().map(|res| Spawner::new(res)).collect();

        if !from_save {
            // only spawn monsters if monster bodies haven't already
            // been loaded from a save file

            for spawner in self.spawners.borrow_mut().iter_mut() {
                let mut bodies = spawner.spawn_all(now, &occupancy);
                self.bodies.borrow_mut().append(&mut bodies);
            }
        }
    }

    pub fn tile_at(&self, x: i64, y: i64) -> Option<u8> {
        if x < 0 || y < 0 || x > 23 || y > 23 {
            return None;
        }
        let map = self.map.get();
        let i = y * 24 + x;
        Some(map.tiles[i as usize])
    }

    pub fn prop_at(&self, x: i64, y: i64) -> Option<u16> {
        if x < 0 || y < 0 || x > 24 || y > 24 {
            return None;
        }
        let map = self.map.get();
        for prop in map.props.iter() {
            if prop.x == x && prop.y == y {
                return Some(prop.id);
            }
        }
        None
    }

    pub fn create_body(
        &self,
        name: String,
        actor_id: Option<u16>,
        prop_id: u16,
        x: f64,
        y: f64,
    ) -> Rc<Body> {
        let body = Body::new(name, actor_id, prop_id, x, y);
        body.set_health(i32::MAX);
        let wrapper = Rc::new(body);
        let mut bodies = self.bodies.borrow_mut();
        bodies.push(wrapper);
        bodies.last().unwrap().clone()
    }

    pub fn create_pet(
        &self,
        name: &str,
        prop_id: u16,
        owner: Rc<Body>,
        pet_kind: PetKind,
        actor_id: Option<u16>,
    ) -> Rc<Body> {
        let (x, y) = self.closest_available_space(owner.x(), owner.y());
        let henchmen = self.create_body(name.to_string(), actor_id, prop_id, x, y);
        henchmen.follow(owner.clone());
        henchmen.set_enemy(save::Team::ENEMY);
        henchmen.set_team(save::Team::PLAYER);
        henchmen.equip_default(self.now.get());
        henchmen.set_is_pet(true);

        let c = condition::body_regen(self.now.get());
        henchmen.add_condition_no_log(c);

        match pet_kind {
            PetKind::Normal => {
                owner.take_pet().map(|p| self.remove_body_ref(p));
                owner.give_pet(henchmen.clone());
            }
            PetKind::Quest => {
                owner.take_quest_pet().map(|p| self.remove_body_ref(p));
                owner.give_quest_pet(henchmen.clone());
            }
            PetKind::Summoned => {
                owner.take_summoned_pet().map(|p| self.remove_body_ref(p));
                owner.give_summoned_pet(henchmen.clone());
            }
        };
        let body_kind = &PROPS[&prop_id.to_string()].name;
        aldon_log!("(you now have a {} follower)", body_kind);
        henchmen
    }

    pub fn place_body(&self, body: Rc<Body>) {
        self.bodies.borrow_mut().push(body);
    }

    pub fn get_body(&self, actor_id: u16) -> Option<Rc<Body>> {
        for body in self.bodies.borrow().iter().rev() {
            if body.actor_id == Some(actor_id) {
                return Some(body.clone());
            }
        }
        /*
        // Actor sometimes search for thier bodies offset by 1000
        // because of a hack used when writting the state machine

        // Actually this was causing problems with removing spawns removing their actor

        if actor_id > 1000 {
            self.get_body(actor_id - 1000)
        } else {
            None
        }
        */
        None
    }

    pub fn get_player(&self) -> Rc<Body> {
        if let Some(body) = self.get_body(0) {
            return body;
        }
        // This should never happen
        js::log("get_player called but no player found! Creating a player...");
        let player = self.create_body("Enter Name".to_string(), Some(0), 55, 12.0, 3.0);
        player.set_health(0);
        player
    }

    pub fn remove_body(&self, actor_id: u16) {
        let Some(body) = self.get_body(actor_id) else {
            js::log("remove body got no body");
            return;
        };
        js::log("remove body is working");
        let foo: Vec<Option<u16>> = self
            .bodies
            .borrow()
            .iter()
            .map(|body| body.actor_id)
            .filter(|x| x.is_some())
            .collect();

        js::log(&format!("bodies before: {:?}", foo));

        self.remove_body_ref(body);

        let foo: Vec<Option<u16>> = self
            .bodies
            .borrow()
            .iter()
            .map(|body| body.actor_id)
            .filter(|x| x.is_some())
            .collect();

        js::log(&format!("bodies before: {:?}", foo));
    }

    pub fn remove_body_ref(&self, body: Rc<Body>) {
        let mut bodies = self.bodies.borrow_mut();
        let idx = bodies.iter().position(|b| Rc::ptr_eq(&body, b));
        if let Some(i) = idx {
            if let Some(actor_id) = bodies[i].actor_id {
                if !self.dead_actors.borrow().contains(&actor_id) {
                    js::log(&format!("actor {} has died by remove", actor_id));
                }
                self.dead_actors.borrow_mut().insert(actor_id);
            }
            bodies.remove(i);
        }
    }

    pub fn closest_available_space(&self, x: f64, y: f64) -> (f64, f64) {
        (x, y)
    }

    pub fn place_trap(&self, x: f64, y: f64, kind: TrapKind) {
        self.traps.borrow_mut().push(Trap { x, y, kind });
    }

    fn maybe_trigger_trap(&self, player: Rc<Body>, body: &Body) {
        let mut traps = self.traps.borrow_mut();
        let (x, y) = body.moving_from();
        let now = self.now.get();

        traps.retain(|trap| {
            if trap.x != x || trap.y != y {
                return true;
            }
            match trap.kind {
                save::TrapKind::SPARK1 => {
                    let attacker = self.get_player();
                    body.take_attack(now, BattleEventType::Hit, 15, Some(attacker));
                    body.battle_event(now, BattleEventType::Condition1);
                }
                save::TrapKind::SPARK2 => {
                    let attacker = self.get_player();
                    body.take_attack(now, BattleEventType::Hit, 30, Some(attacker));
                    body.battle_event(now, BattleEventType::Condition1);
                }
                save::TrapKind::SPARK3 => {
                    let attacker = self.get_player();
                    body.take_attack(now, BattleEventType::Hit, 45, Some(attacker));
                    body.battle_event(now, BattleEventType::Condition1);
                }
                save::TrapKind::FLAME1 => {
                    let attacker = self.get_player();
                    body.take_attack(now, BattleEventType::Hit, 30, Some(attacker));
                    let missiles = explosion(
                        now,
                        x,
                        y,
                        player.clone(),
                        24, /* damage */
                        MissileType::Fire,
                        combat::ExplosionSize::Medium,
                    );
                    self.missiles.borrow_mut().extend(missiles);
                }
                save::TrapKind::FLAME2 => {
                    let attacker = self.get_player();
                    body.take_attack(now, BattleEventType::Hit, 30, Some(attacker));
                    let missiles = explosion(
                        now,
                        x,
                        y,
                        player.clone(),
                        24, /* damage */
                        MissileType::Bonfire,
                        combat::ExplosionSize::Large,
                    );
                    self.missiles.borrow_mut().extend(missiles);
                }
                save::TrapKind::WEAKNESS => {
                    let c = condition::trap(now, save::ConditionType::STRENGTH);
                    body.add_condition(c);
                    body.battle_event(now, BattleEventType::Condition2);
                }
                save::TrapKind::SLOWNESS => {
                    let c = condition::trap(now, save::ConditionType::SPEED);
                    body.add_condition(c);
                    body.battle_event(now, BattleEventType::Condition2);
                }
                save::TrapKind::POISON => {
                    let c = condition::trap(now, save::ConditionType::POISON);
                    body.add_condition(c);
                    body.battle_event(now, BattleEventType::Condition2);
                }
                save::TrapKind(i) => {
                    js::log(&format!("Warning, unknown trap kind {}", i));
                }
            }
            false
        });
    }

    pub fn use_item(&self, body: &Body, prop_id: u16) {
        let prop = &PROPS[&prop_id.to_string()];
        if !prop.can_use(body.class(), body.level()) {
            return;
        }
        let now = self.now.get();
        match prop_id {
            // Trap, Spark I
            101 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::SPARK1);
            }

            // Trap, Slowness I
            180 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::SLOWNESS);
            }

            // potion, minor heal
            19 => {
                body.heal(10);
                body.battle_event(now, BattleEventType::Condition1);
            }

            // potion, cure poison
            210 => {
                body.cure_poison();
                body.battle_event(now, BattleEventType::Condition1);
            }

            // potion, purify
            211 => {
                body.cure_poison();
                body.battle_event(now, BattleEventType::Condition1);
            }

            // Trap, Weakness I
            213 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::WEAKNESS);
            }

            // Trap, Poison
            214 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::POISON);
            }

            // bag, medicine
            223 => {
                body.heal(15);
                body.battle_event(now, BattleEventType::Condition1);
            }

            // potion, quicksilver
            315 => {
                let c = condition::potion(now, save::ConditionType::DEXTERITY);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved positive Dex.*", body.name);
            }

            // potion, stone skin
            316 => {
                let c = condition::potion(now, save::ConditionType::ARMOR);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved Armor.*", body.name);
            }

            // potion, iron skin
            317 => {
                let c = condition::potion(now, save::ConditionType::ARMOR);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved Armor.*", body.name);
            }

            // potion, troll's blood
            321 => {
                let c = condition::potion_regen(now);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved Regen*", body.name);
            }

            // Trap, Spark II
            365 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::SPARK2);
            }

            // Trap, Spark III
            366 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::SPARK3);
            }

            // Trap, Flame I
            367 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::FLAME1);
            }

            // Trap, Flame II
            368 => {
                let (x, y) = body.moving_from();
                self.place_trap(x, y, save::TrapKind::FLAME2);
            }

            // potion, mana renewal
            405 => {}

            // potion, wood skin
            89 => {
                let c = condition::potion(now, save::ConditionType::ARMOR);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved Armor.*", body.name);
            }

            // potion, heal
            91 => {
                body.heal(25);
                body.battle_event(now, BattleEventType::Condition1);
            }

            // potion, major heal
            92 => {
                body.heal(45);
                body.battle_event(now, BattleEventType::Condition1);
            }

            // potion, iron arm
            93 => {
                let c = condition::potion(now, save::ConditionType::STRENGTH);
                body.add_condition(c);
                body.battle_event(now, BattleEventType::Condition1);
                aldon_log!("*{} recieved positive Str.*", body.name);
            }

            _ => return,
        }
        body.take_item(prop_id)
    }

    // Effecting a spell means it was already cast and now it is time to have some effect
    fn effect_spell(&self, spell_id: u16, caster: Rc<Body>, x: f64, y: f64) {
        match spell_id {
            // First, Second, Third, Fourth, Fifth Summon
            8 | 9 | 10 | 11 | 12 => {
                let prop_id = combat::summon(spell_id);
                let name = &PROPS[&prop_id.to_string()].name;
                let pet = self.create_pet(
                    name,
                    prop_id,
                    caster.clone(),
                    PetKind::Summoned,
                    None, /* actor_id */
                );
                pet.set_level(caster.level());
                pet.set_x(x);
                pet.set_y(y);
            }
            _ => {}
        }
    }

    // Casting a spell may fail (e.g. not enough mana, fizzles, no enemy). If successful a missile
    // will be created. Returns true if a spell was cast
    pub fn cast_spell(&self, spell_id: u16, caster: Rc<Body>, x: f64, y: f64) -> bool {
        js::log(&format!(
            "casting spell: {}, {}, {}, {}",
            spell_id, caster.name, x, y
        ));
        let spell = &SPELLS[&spell_id.to_string()];

        match spell.target {
            SpellTarget::None | SpellTarget::Corpse => {
                if !caster.try_spell(self.now(), spell_id) {
                    return true;
                }
                let missile = combat::spell(self.now.get(), spell_id, caster.clone(), x, y);
                self.missiles.borrow_mut().push(missile);
                return true;
            }

            SpellTarget::Enemy | SpellTarget::Friend => {
                let maybe_target = if matches!(spell.target, SpellTarget::Enemy) {
                    self.enemy_at(x, y)
                } else {
                    self.friend_at(x, y)
                };
                let Some(target) = maybe_target else {
                    return false;
                };
                if !caster.try_spell(self.now(), spell_id) {
                    return true;
                }
                let missile = combat::targeted_spell(
                    self.now.get(),
                    spell_id,
                    caster.clone(),
                    target.clone(),
                );
                self.missiles.borrow_mut().push(missile);
                return true;
            }
        }
    }
}

// Behavior for spawning random monsters on the map
struct Spawner {
    res: &'static SpawnerRes,
    tick_deadline: Option<f64>,
    rng: ThreadRng,
    spawned: Vec<Rc<Body>>,
}

impl Spawner {
    fn new(res: &'static SpawnerRes) -> Self {
        Self {
            res,
            spawned: Vec::new(),
            tick_deadline: None,
            rng: rand::thread_rng(),
        }
    }

    fn prob_spawn(&self, n: usize) -> f64 {
        if n == 0 {
            1.0
        } else if n == self.res.max_creatures {
            0.0
        } else {
            1.0 / (2.0 * n as f64)
        }
    }

    /// Returns true if the given body matches a type of creature
    /// spawned by the spawner
    fn is_match(&self, body: Rc<Body>) -> bool {
        let spawner_team = self.res.monster_team.try_into().unwrap();
        let spawner_target = self.res.monster_target.try_into().unwrap();
        self.res.creatures.contains(&body.prop_id)
            && body.team() == Some(spawner_team)
            && body.hostile_to.get() == Some(spawner_target)
            && body.level() == self.res.level
    }

    fn spawn_all(&mut self, now: f64, occupancy: &Occupancy) -> Vec<Rc<Body>> {
        let mut bodies = Vec::new();

        while self.spawned.len() < self.res.max_creatures {
            let Some(body) = self.spawn(now, occupancy) else {
                continue;
            };
            self.spawned.push(body.clone());
            bodies.push(body);
        }
        bodies
    }

    fn spawn(&mut self, now: f64, occupancy: &Occupancy) -> Option<Rc<Body>> {
        let x_min = self.res.x;
        let x_max = self.res.x + self.res.width;
        let y_min = self.res.y;
        let y_max = self.res.y + self.res.height;
        let mut spaces: Vec<(u8, u8)> = Vec::new();

        for x in x_min..x_max {
            for y in y_min..y_max {
                if !occupancy.occupied(x as f64, y as f64) {
                    spaces.push((x, y));
                }
            }
        }
        let Some(&(x, y)) = spaces.choose(&mut self.rng) else {
            return None;
        };
        let prop_id = *self
            .res
            .creatures
            .choose(&mut self.rng)
            .expect("Spawner should always have at least one creature type");

        let prop = &PROPS[&prop_id.to_string()];
        let body = Body::new(prop.name.clone(), None, prop_id, x as f64, y as f64);
        body.equip_default(now);
        body.wander(x_min as f64, y_min as f64, x_max as f64, y_max as f64);
        body.set_team(self.res.monster_team.try_into().unwrap());
        body.set_enemy(self.res.monster_target.try_into().unwrap());
        body.set_intel(save::IntelType::HUNTER);
        body.set_level(self.res.level);
        body.persist();
        body.set_from_spawner(true);
        js::log(&format!(
            "prop {} has health {}, level {}",
            prop_id,
            body.health.get(),
            body.level(),
        ));

        return Some(Rc::new(body));
    }

    fn update(&mut self, now: f64, occupancy: &Occupancy) -> Option<Rc<Body>> {
        self.spawned.retain(|body| body.get_health() > 0);

        if self.spawned.len() == self.res.max_creatures {
            self.tick_deadline = None;
            return None;
        }
        match self.tick_deadline {
            Some(deadline) if now > deadline => {
                self.tick_deadline = Some(now + self.res.delay * 50.0);
                let rand: f64 = self.rng.gen();

                if rand < self.prob_spawn(self.spawned.len()) {
                    let maybe_body = self.spawn(now, occupancy);

                    if let Some(body) = &maybe_body {
                        self.spawned.push(body.clone());
                    };
                    return maybe_body;
                }
            }
            None if self.spawned.len() < self.res.max_creatures => {
                self.tick_deadline = Some(now + self.res.delay * 50.0);
            }
            _ => {}
        };
        None
    }
}

impl TryFrom<u16> for RaceType {
    type Error = String;

    fn try_from(x: u16) -> Result<RaceType, Self::Error> {
        let race = match x {
            0 => save::RaceType::HUMAN,
            1 => save::RaceType::ELF,
            2 => save::RaceType::DWARF,
            _ => Err(format!("Unknown race type {}", x))?,
        };
        Ok(race)
    }
}

impl TryFrom<&str> for save::RaceType {
    type Error = String;

    fn try_from(name: &str) -> Result<save::RaceType, Self::Error> {
        let race = match name {
            "human" => save::RaceType::HUMAN,
            "elf" => save::RaceType::ELF,
            "dwarf" => save::RaceType::DWARF,
            _ => Err(format!("Unknown race type '{}'", name))?,
        };
        Ok(race)
    }
}

impl save::RaceType {
    pub fn from_u16(x: u16) -> save::RaceType {
        match x {
            0 => save::RaceType::HUMAN,
            1 => save::RaceType::ELF,
            2 => save::RaceType::DWARF,
            _ => panic!("Unknown race type {}", x),
        }
    }

    pub fn from_str(name: &str) -> save::RaceType {
        match name {
            "human" => save::RaceType::HUMAN,
            "elf" => save::RaceType::ELF,
            "dwarf" => save::RaceType::DWARF,
            _ => panic!("Unknown race type '{}'", name),
        }
    }

    pub fn to_str(&self) -> &str {
        match *self {
            save::RaceType::HUMAN => "Human",
            save::RaceType::ELF => "Elf",
            save::RaceType::DWARF => "Dwarf",
            _ => "Unknown",
        }
    }
}

impl TryFrom<u8> for save::IntelType {
    type Error = String;

    fn try_from(x: u8) -> Result<save::IntelType, Self::Error> {
        let intel = match x {
            1 => save::IntelType::HUNTER,
            2 => save::IntelType::GUILD_MASTER,
            3 => save::IntelType::NPC,
            4 => save::IntelType::MESSAGE_BEARER,
            5 => save::IntelType::PLAYER,
            _ => Err(format!("Invalid IntelType {}", x))?,
        };
        Ok(intel)
    }
}

impl TryFrom<u8> for save::Team {
    type Error = String;

    fn try_from(x: u8) -> Result<save::Team, Self::Error> {
        let team = match x {
            1 => save::Team::PLAYER,
            2 => save::Team::ENEMY,
            3 => save::Team::ANIMAL,
            4 => save::Team::NPC,
            _ => Err(format!("Invalid Team {}", x))?,
        };
        Ok(team)
    }
}

fn explosion(
    now: f64,
    x0: f64,
    y0: f64,
    attacker: Rc<Body>,
    damage: i32,
    kind: MissileType,
    size: combat::ExplosionSize,
) -> Vec<Missile> {
    let s = match size {
        combat::ExplosionSize::Medium => 1,
        combat::ExplosionSize::Large => 2,
    };
    let mut missiles: Vec<Missile> = Vec::new();
    for i in -s..=s {
        for j in -s..=s {
            if i == 0 && j == 0 {
                continue;
            }
            let missile = Missile::new(
                now,
                x0,
                y0,
                x0 + i as f64,
                y0 + j as f64,
                attacker.clone(),
                kind.clone(),
                vec![MissileEffect::SplashDamage { amount: damage }],
            );
            missiles.push(missile);
        }
    }
    missiles
}

fn impassible(tile_id: u8) -> bool {
    IMPASSIBLE_TILES.contains(&tile_id)
}

fn sight_blocking(tile_id: u8) -> bool {
    SIGHT_BLOCKER_TILES.contains(&tile_id)
}

#[derive(Clone, Copy)]
pub struct Trap {
    pub x: f64,
    pub y: f64,
    kind: save::TrapKind,
}

impl Into<save::Trap> for Trap {
    fn into(self) -> save::Trap {
        save::Trap::new(
            OrderedFloat::from(self.x),
            OrderedFloat::from(self.y),
            self.kind,
        )
    }
}

impl TryFrom<save::Trap> for Trap {
    type Error = InvalidDataError;

    fn try_from(save: save::Trap) -> Result<Trap, Self::Error> {
        let x = *save.x.ok_or(InvalidDataError::new("x field missing"))?;
        let y = *save.y.ok_or(InvalidDataError::new("y field missing"))?;

        let kind = save
            .kind
            .ok_or(InvalidDataError::new("kind field missing"))?;

        Ok(Trap { x, y, kind })
    }
}

pub(crate) enum PetKind {
    Normal,
    Quest,
    Summoned,
}

/// Track which tiles are occupied by something impassable
#[derive(Debug, Clone)]
pub(crate) struct Occupancy {
    perm: [bool; 576],
    temp: [bool; 576],
}

impl Occupancy {
    fn new() -> Self {
        Self {
            perm: [false; 576],
            temp: [false; 576],
        }
    }

    pub fn occupancy_idx(x: f64, y: f64) -> usize {
        let xp = x.floor() as usize;
        let yp = y.floor() as usize;
        yp * 24 + xp
    }

    pub fn reset(&mut self) {
        self.temp = self.perm.clone();
    }

    pub fn occupy_permanent(&mut self, x: f64, y: f64) {
        let i = Self::occupancy_idx(x, y);
        self.perm[i] = true;
        self.temp[i] = true;
    }

    pub fn occupy(&mut self, x: f64, y: f64) {
        let i = Self::occupancy_idx(x, y);
        self.temp[i] = true;
    }

    pub fn vacate(&mut self, x: f64, y: f64) {
        let i = Self::occupancy_idx(x, y);
        self.temp[i] = false;
    }

    pub fn occupied(&self, x: f64, y: f64) -> bool {
        if x > 23.0 || x < 0.0 || y > 23.0 || y < 0.0 {
            return true;
        }
        let i = Self::occupancy_idx(x, y);
        self.temp[i]
    }
}

fn rect_contains(left: f64, top: f64, width: f64, height: f64, x: f64, y: f64) -> bool {
    (x >= left) && (x <= (left + width)) && (y >= top) && (y <= (top + height))
}

fn body_distance(body0: &Body, body1: &Body) -> f64 {
    let (x0, y0) = body0.moving_from();
    let (x1, y1) = body1.moving_from();
    distance(x0, y0, x1, y1)
}

/// Distance between two points in a world where sqrt(2) = 1
fn distance(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    f64::max((x0 - x1).abs(), (y0 - y1).abs())
}
