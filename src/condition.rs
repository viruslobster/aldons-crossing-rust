//! Implementation for "conditions" that have some effect on a body for some duration
use crate::{
    aldon_log,
    body::Body,
    combat::BattleEventType,
    game::{InvalidDataError, CONSOLE},
    js,
    thrift::save::{self, ConditionSource, ConditionType},
};
use core::fmt::Debug;
use rand::Rng;
use std::{cell::Cell, fmt::Write, rc::Rc};
use thrift::OrderedFloat;

/// Behavior for how a condition changes over time, like the "course" of a disease
trait Course: CloneBox {
    fn finished(&self, now: f64) -> bool;
    fn update(&self, body: &Body, now: f64) -> bool;
    fn save(&self, now: f64) -> Option<save::Course>;
}

/// A Condition (e.g. strength, armor) applied from some source (e.g. potion, enemy) at some
/// magnitude (e.g. +1, -1) for some course (e.g. 2 min, until the player moves, some random
/// interval)
#[derive(Debug, Clone)]
pub(crate) struct Condition {
    pub kind: ConditionType,
    pub source: ConditionSource,
    pub magnitude: i32,
    course: Box<dyn Course>,
}

impl Condition {
    pub fn finished(&self, now: f64) -> bool {
        self.course.finished(now)
    }

    pub fn update(&self, body: &Body, now: f64) {
        if !self.course.update(body, now) {
            return;
        }
        match self.kind {
            ConditionType::HEALTH => {
                if self.source == ConditionSource::PLAYER {
                    body.heal_no_log(self.magnitude);
                } else {
                    body.heal(self.magnitude);
                }
            }
            ConditionType::POISON => {
                // TODO: If an enemy dies from poison here the player won't
                // get the reward
                body.take_attack(
                    now,
                    BattleEventType::Hit,
                    self.magnitude,
                    None, /* attacker */
                )
            }
            ConditionType::MANA => {
                let mana = body.magic();
                body.set_magic(mana + self.magnitude);
            }
            _ => {}
        }
    }

    pub fn save(&self, now: f64) -> Option<save::Condition> {
        let Some(save) = self.course.save(now) else {
            return None;
        };
        let save = Box::new(save);
        let result = save::Condition::new(self.kind, self.source, save, self.magnitude);
        Some(result)
    }

    pub fn from_save(save: &save::Condition, now: f64) -> Result<Condition, InvalidDataError> {
        let course = save
            .course
            .clone()
            .ok_or(InvalidDataError::new("course field missing"))?;

        let kind = save
            .kind
            .ok_or(InvalidDataError::new("kind field missing"))?;

        let source = save
            .source
            .ok_or(InvalidDataError::new("source field missing"))?;

        let magnitude = save
            .magnitude
            .ok_or(InvalidDataError::new("magnitude field missing"))?;

        let course: Box<dyn Course> = match *course {
            save::Course::Timed(timed) => Box::new(TimedCourse::from_save(&timed, now)?),
            save::Course::Periodic(periodic) => {
                Box::new(PeriodicCourse::from_save(&periodic, now)?)
            }
            save::Course::PeriodicTimed(periodic_timed) => {
                Box::new(PeriodicTimedCourse::from_save(&periodic_timed, now)?)
            }
            save::Course::MovementCanceled(movement_canceled) => {
                Box::new(MovementCanceledCourse::from_save(&movement_canceled)?)
            }
            save::Course::Random(random) => Box::new(RandomCourse::from_save(&random, now)?),
            save::Course::Equiped(_) => {
                return Err(InvalidDataError::new("Cannot load equiped save"))
            }
            save::Course::EquipedPeriodic(_) => {
                return Err(InvalidDataError::new("Cannot load equiped periodic save"))
            }
        };
        let result = Self {
            kind,
            source,
            course,
            magnitude,
        };
        Ok(result)
    }
}

impl PartialEq for Condition {
    fn eq(&self, other: &Condition) -> bool {
        self.kind == other.kind && self.source == other.source && self.magnitude == other.magnitude
    }
}

#[derive(Debug, Clone)]
struct TimedCourse {
    deadline: f64,
}

impl TimedCourse {
    fn new(now: f64, duration_sec: f64) -> Self {
        Self {
            deadline: now + duration_sec * 1000.0,
        }
    }

    fn from_save(save: &save::TimedCourse, now: f64) -> Result<Self, InvalidDataError> {
        let duration: f64 = save
            .duration
            .ok_or(InvalidDataError::new("duration field missing"))?
            .into();

        let result = Self {
            deadline: now + duration,
        };
        Ok(result)
    }
}

impl Course for TimedCourse {
    fn finished(&self, now: f64) -> bool {
        now >= self.deadline
    }

    fn update(&self, _body: &Body, _now: f64) -> bool {
        false
    }

    fn save(&self, now: f64) -> Option<save::Course> {
        let duration = f64::max(self.deadline - now, 0.0);
        let save = save::TimedCourse::new(OrderedFloat::from(duration));
        let result = save::Course::Timed(save);
        Some(result)
    }
}

#[derive(Debug, Clone)]
struct PeriodicCourse {
    period: f64,
    next_update: Cell<f64>,
}

impl PeriodicCourse {
    fn new(now: f64, period_sec: f64) -> Self {
        let period = period_sec * 1000.0;
        Self {
            period,
            next_update: Cell::new(now),
        }
    }

    fn from_save(save: &save::PeriodicCourse, now: f64) -> Result<Self, InvalidDataError> {
        let period: f64 = save
            .period
            .ok_or(InvalidDataError::new("period field missing"))?
            .into();

        let ms_until_update: f64 = save
            .ms_until_update
            .ok_or(InvalidDataError::new("ms_until_update field missing"))?
            .into();

        let result = Self {
            period: period.into(),
            next_update: Cell::new(now + ms_until_update),
        };
        Ok(result)
    }
}

impl Course for PeriodicCourse {
    fn finished(&self, _now: f64) -> bool {
        false
    }

    fn update(&self, _body: &Body, now: f64) -> bool {
        if now < self.next_update.get() {
            return false;
        }
        self.next_update.set(now + self.period);
        true
    }

    fn save(&self, now: f64) -> Option<save::Course> {
        let ms_until_update = f64::max(self.next_update.get() - now, 0.0);
        let save = save::PeriodicCourse::new(
            OrderedFloat::from(self.period),
            OrderedFloat::from(ms_until_update),
        );
        let result = save::Course::Periodic(save);
        Some(result)
    }
}

#[derive(Debug, Clone)]
struct PeriodicTimedCourse {
    timed: TimedCourse,
    periodic: PeriodicCourse,
}

impl PeriodicTimedCourse {
    fn new(now: f64, period_sec: f64, duration_sec: f64) -> Self {
        Self {
            timed: TimedCourse::new(now, duration_sec),
            periodic: PeriodicCourse::new(now, period_sec),
        }
    }

    fn from_save(save: &save::PeriodicTimedCourse, now: f64) -> Result<Self, InvalidDataError> {
        let timed = save
            .timed
            .as_ref()
            .ok_or(InvalidDataError::new("timed field missing"))?;

        let periodic = save
            .periodic
            .as_ref()
            .ok_or(InvalidDataError::new("periodic field missing"))?;

        let result = Self {
            timed: TimedCourse::from_save(&timed, now)?,
            periodic: PeriodicCourse::from_save(&periodic, now)?,
        };
        Ok(result)
    }
}

impl Course for PeriodicTimedCourse {
    fn finished(&self, now: f64) -> bool {
        self.timed.finished(now)
    }

    fn update(&self, body: &Body, now: f64) -> bool {
        self.periodic.update(body, now)
    }

    fn save(&self, now: f64) -> Option<save::Course> {
        let Some(save::Course::Timed(timed_save)) = self.timed.save(now) else {
            js::log("TimedCourse::save did not return CourseSave::Timed");
            return None;
        };
        let Some(save::Course::Periodic(periodic_save)) = self.periodic.save(now) else {
            js::log("PeriodicCourse::save did not return CourseSave::Periodic");
            return None;
        };
        let save = save::PeriodicTimedCourse::new(timed_save, periodic_save);
        let result = save::Course::PeriodicTimed(save);
        Some(result)
    }
}

#[derive(Debug, Clone)]
struct MovementCanceledCourse {
    moved: Cell<bool>,
}

impl MovementCanceledCourse {
    fn new() -> Self {
        Self {
            moved: Cell::new(false),
        }
    }

    fn from_save(_save: &save::MovementCanceledCourse) -> Result<Self, InvalidDataError> {
        Ok(Self::new())
    }
}

impl Course for MovementCanceledCourse {
    fn finished(&self, _now: f64) -> bool {
        self.moved.get()
    }

    fn update(&self, body: &Body, _now: f64) -> bool {
        if (body.x(), body.y()) != body.moving_to() {
            self.moved.set(true);
        }
        false
    }

    fn save(&self, _now: f64) -> Option<save::Course> {
        let save = save::MovementCanceledCourse::new();
        let result = save::Course::MovementCanceled(save);
        Some(result)
    }
}

#[derive(Debug, Clone)]
struct EquipedCourse {
    item: Rc<Body>,
    equiped: Cell<bool>,
}

impl EquipedCourse {
    fn new(item: Rc<Body>) -> Self {
        Self {
            item,
            equiped: Cell::new(true),
        }
    }
}

impl Course for EquipedCourse {
    fn finished(&self, _now: f64) -> bool {
        !self.equiped.get()
    }

    fn update(&self, body: &Body, _now: f64) -> bool {
        if body.is_equiped(self.item.clone()).is_none() {
            self.equiped.set(false);
            return false;
        }
        true
    }

    fn save(&self, _now: f64) -> Option<save::Course> {
        // We don't actually need to ever save an equiped effect.
        // Items are re-equiped on game load
        None
    }
}

#[derive(Debug, Clone)]
struct EquipedPeriodicCourse {
    equiped: EquipedCourse,
    periodic: PeriodicCourse,
}

impl EquipedPeriodicCourse {
    fn new(now: f64, item: Rc<Body>, period_sec: f64) -> Self {
        Self {
            equiped: EquipedCourse::new(item),
            periodic: PeriodicCourse::new(now, period_sec),
        }
    }
}

impl Course for EquipedPeriodicCourse {
    fn finished(&self, now: f64) -> bool {
        self.equiped.finished(now)
    }

    fn update(&self, body: &Body, now: f64) -> bool {
        self.equiped.update(body, now) && self.periodic.update(body, now)
    }

    fn save(&self, _now: f64) -> Option<save::Course> {
        // We don't actually need to ever save an equiped effect.
        // Items are re-equiped on game load
        None
    }
}

#[derive(Debug, Clone)]
struct RandomCourse {
    periodic: PeriodicCourse,
    finished: Cell<bool>,
    chance: i32,
}

impl RandomCourse {
    fn new(now: f64, chance: i32, period_sec: f64) -> Self {
        let periodic = PeriodicCourse::new(now, period_sec);
        // skip the first update
        periodic.next_update.set(now + period_sec * 1000.0);

        Self {
            periodic,
            chance,
            finished: Cell::new(false),
        }
    }

    fn from_save(save: &save::RandomCourse, now: f64) -> Result<Self, InvalidDataError> {
        let periodic = save
            .periodic
            .as_ref()
            .ok_or(InvalidDataError::new("periodic field missing"))?;

        let chance = save
            .chance
            .ok_or(InvalidDataError::new("chance field missing"))?;

        let result = Self {
            periodic: PeriodicCourse::from_save(&periodic, now)?,
            chance,
            finished: Cell::new(false),
        };
        Ok(result)
    }
}

impl Course for RandomCourse {
    fn finished(&self, _now: f64) -> bool {
        self.finished.get()
    }

    fn update(&self, body: &Body, now: f64) -> bool {
        if !self.periodic.update(body, now) {
            return false;
        }
        let mut rng = rand::thread_rng();
        let roll: i32 = rng.gen_range(1..=100);
        aldon_log!("*Chance:{} Roll: {}*", self.chance, roll);

        if roll >= self.chance {
            self.finished.set(true);
        }
        true
    }

    fn save(&self, now: f64) -> Option<save::Course> {
        let Some(save::Course::Periodic(periodic_save)) = self.periodic.save(now) else {
            js::log("PeriodicCourse::save did not return CourseSave::Periodic");
            return None;
        };
        let save = save::RandomCourse::new(periodic_save, self.chance);
        let result = save::Course::Random(save);
        Some(result)
    }
}

pub(crate) fn potion(now: f64, kind: ConditionType) -> Condition {
    let course = TimedCourse::new(now, 90.0 /* duration_sec */);
    Condition {
        kind,
        source: ConditionSource::POTION,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn potion_regen(now: f64) -> Condition {
    let course =
        PeriodicTimedCourse::new(now, 2.0 /* period_sec */, 90.0 /* duration_sec */);

    Condition {
        kind: ConditionType::HEALTH,
        source: ConditionSource::POTION,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn body_regen(now: f64) -> Condition {
    let course = PeriodicCourse::new(now, 18.0 /* period_sec */);
    Condition {
        kind: ConditionType::HEALTH,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn body_mana_regen(now: f64) -> Condition {
    let course = PeriodicCourse::new(now, 0.8 /* period_sec */);
    Condition {
        kind: ConditionType::MANA,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn item(kind: ConditionType, magnitude: i32, body: Rc<Body>) -> Condition {
    let course = EquipedCourse::new(body);
    Condition {
        kind,
        source: ConditionSource::ITEM,
        course: Box::new(course),
        magnitude,
    }
}

pub(crate) fn item_regen(now: f64, body: Rc<Body>) -> Condition {
    let course = EquipedPeriodicCourse::new(now, body, 2.0 /* period_sec */);
    Condition {
        kind: ConditionType::HEALTH,
        source: ConditionSource::ITEM,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn hidden() -> Condition {
    let course = MovementCanceledCourse::new();
    Condition {
        kind: ConditionType::HIDDEN,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn sneaking(now: f64, chance: i32) -> Condition {
    let course = RandomCourse::new(now, chance, 12.0 /* period_sec */);
    Condition {
        kind: ConditionType::SNEAKING,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn invisible(now: f64) -> Condition {
    let course = TimedCourse::new(now, 90.0 /* duration_sec */);
    Condition {
        kind: ConditionType::HIDDEN,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn trap(now: f64, kind: ConditionType) -> Condition {
    let course = TimedCourse::new(now, 90.0 /* duration_sec */);
    Condition {
        kind,
        source: ConditionSource::ENEMY,
        course: Box::new(course),
        magnitude: -1,
    }
}

pub(crate) fn poison(now: f64) -> Condition {
    let course =
        PeriodicTimedCourse::new(now, 2.0 /* period_sec */, 90.0 /* duration_sec */);

    Condition {
        kind: ConditionType::POISON,
        source: ConditionSource::ENEMY,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn random_poison(now: f64) -> Condition {
    let course = RandomCourse::new(now, 60 /* chance */, 2.0 /* period_sec */);

    Condition {
        kind: ConditionType::POISON,
        source: ConditionSource::ENEMY,
        course: Box::new(course),
        magnitude: 1,
    }
}

pub(crate) fn armor(now: f64, magnitude: i32) -> Condition {
    let course = TimedCourse::new(now, 90.0 /* duration_sec */);

    Condition {
        kind: ConditionType::ARMOR,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
        magnitude,
    }
}

pub(crate) fn stat(now: f64, kind: ConditionType, magnitude: i32) -> Condition {
    let course = TimedCourse::new(now, 90.0 /* duration_sec */);

    Condition {
        kind,
        magnitude,
        source: ConditionSource::PLAYER,
        course: Box::new(course),
    }
}
pub(crate) fn for_item(now: f64, body: Rc<Body>) -> Option<Condition> {
    let effect = match body.prop_id {
        // Ring of Regeneration
        322 => item_regen(now, body),

        // ring of force
        173 => item(save::ConditionType::STRENGTH, 1 /* manitude */, body),

        // ring of alacrity
        174 => item(save::ConditionType::DEXTERITY, 1 /* manitude */, body),

        // ring of intellect
        175 => item(
            save::ConditionType::INTELIGENCE,
            1, /* manitude */
            body,
        ),

        // Ring of Thieves
        177 => item(save::ConditionType::DEXTERITY, 2 /* manitude */, body),

        // Ring of Battle
        178 => item(save::ConditionType::STRENGTH, 2 /* manitude */, body),

        // Mystic Ring
        188 => item(
            save::ConditionType::INTELIGENCE,
            2, /* manitude */
            body,
        ),

        // Guantlets of power
        259 => item(save::ConditionType::STRENGTH, 1 /* manitude */, body),

        // Lucky charm
        262 => item(save::ConditionType::LUCK, 1 /* manitude */, body),

        // boots, speed
        120 => item(save::ConditionType::SPEED, 1 /* manitude */, body),

        // Prayer Beads
        375 => item(
            save::ConditionType::INTELIGENCE,
            2, // magnitude
            body,
        ),

        // Holy Breastplate
        253 => item(
            save::ConditionType::INTELIGENCE,
            1, // magnitude
            body,
        ),

        // Holy Ring
        189 => item(
            save::ConditionType::INTELIGENCE,
            1, // magnitude
            body,
        ),

        // TODO: Silvery Bracers
        // TODO: Spirit Cloak
        // TODO: Blessed Ring
        _ => return None,
    };
    Some(effect)
}

impl Into<&str> for ConditionType {
    fn into(self) -> &'static str {
        match self {
            ConditionType::INTELIGENCE => "Int",
            ConditionType::LUCK => "Luck",
            ConditionType::DEXTERITY => "Dex",
            ConditionType::STRENGTH => "Str",
            ConditionType::MANA => "Mana Regen",
            ConditionType::ARMOR => "Armor",
            ConditionType::SPEED => "Speed",
            ConditionType::HEALTH => "Regen",
            ConditionType::POISON => "Poison",
            _ => "UNKNOWN",
        }
    }
}

/// Just a weird rust thing I have to do in order to clone a Course
trait CloneBox {
    fn clone_box(&self) -> Box<dyn Course>;
}

impl<T> CloneBox for T
where
    T: 'static + Course + Clone,
{
    fn clone_box(&self) -> Box<dyn Course> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Course> {
    fn clone(&self) -> Box<dyn Course> {
        self.clone_box()
    }
}

impl Debug for dyn Course {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Course")
    }
}

impl Into<String> for &Condition {
    fn into(self) -> String {
        let sign_str = if self.magnitude >= 0 { "pos" } else { "neg" };
        let kind_str: &str = self.kind.into();
        format!("{} {}", sign_str, kind_str)
    }
}
