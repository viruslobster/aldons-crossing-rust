//! All things bitey and scratchy
use crate::{
    aldon_log,
    body::Body,
    condition::{self, Condition},
    data::{PropTypeRes, SpellTarget, PROPS, SPELLS},
    game::CONSOLE,
    js,
    stats::{intelligence_to_chance_cast, luck_to_modifier, strength_to_damage},
    thrift::save::ConditionType,
};
use rand::{seq::SliceRandom, Rng};
use std::{cmp::max, fmt::Write, rc::Rc, vec::Vec};

fn weapon_damage(
    prop_id: u16,
    attacker_level: i32,
    attacker_strength: i32,
    opponent_level: i32,
) -> i32 {
    let factor = max(1, (attacker_level - opponent_level) / 2);
    if matches!(prop_id, 244 | 314 | 392 | 237) {
        let mut rng = rand::thread_rng();
        let damage_min = 1;
        let damage_max = 2 + 3 * attacker_level;
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
        let dmg = rng.gen_range(damage_min..=damage_max) + strength_to_damage(attacker_strength);
        return factor * max(dmg, 1);
    }
    js::log(&format!(
        "{} was equiped as a weapon but is not a weapon",
        prop_id
    ));
    0
}

pub(crate) fn spell_damage(spell_id: u16, attacker_level: i32, opponent_level: i32) -> i32 {
    let factor = max(1, (attacker_level - opponent_level) / 2) * max(1, attacker_level / 2);
    let dmg_max = spell_base_damage(spell_id);
    if dmg_max == 0 {
        // gen_range(1..=0) panics
        return 0;
    }
    let mut rng = rand::thread_rng();
    rng.gen_range(1..=dmg_max) * factor
}

fn make_spell_attack(spell_id: u16, attacker: &Body, target: &Body) -> (i32, BattleEventType) {
    let chance_hit = intelligence_to_chance_cast(attacker.inteligence());
    let mut rng = rand::thread_rng();
    let roll: i32 = rng.gen_range(1..=100);
    aldon_log!("-{} needs < {}, rolls {}-", attacker.name, chance_hit, roll);

    if roll > chance_hit {
        aldon_log!("-{} fizzles", target.name);
        return (0, BattleEventType::Fizzle);
    }
    return (
        spell_damage(spell_id, attacker.level(), target.level()),
        BattleEventType::Hit,
    );
}

fn make_weapon_attack(prop_id: u16, attacker: &Body, target: &Body) -> (i32, BattleEventType) {
    let bonus = attacker.level() / 2;
    let chance_hit = clamp(
        54 + attacker.attack_hit_bonus() - target.armor_class()
            + 5 * bonus
            + luck_to_modifier(attacker.luck())
            - luck_to_modifier(target.luck()),
        5,
        99,
    );
    let mut rng = rand::thread_rng();
    let roll: i32 = rng.gen_range(1..=100);
    aldon_log!("-{} needs < {}, rolls {}-", attacker.name, chance_hit, roll);
    let crit_chance = max(1, 5 + luck_to_modifier(attacker.luck()));

    if roll <= crit_chance {
        aldon_log!("*{} CRITICALLY HITS {}*", attacker.name, target.name);
        let dmg = 2 * attacker.attack_damage(prop_id, target.level());
        (dmg, BattleEventType::Crit)
    } else if roll < chance_hit {
        let dmg = weapon_damage(
            prop_id,
            attacker.level(),
            attacker.strength(),
            target.level(),
        );
        (dmg, BattleEventType::Hit)
    } else {
        (0, BattleEventType::Miss)
    }
}

fn make_attack_impl(attacker: &Body, target: &Body) -> (i32, BattleEventType) {
    let Some(weapon) = attacker.equiped_weapon() else {
        return (0, BattleEventType::Miss);
    };
    if let Some(spell_id) = weapon_to_spell(weapon) {
        return make_spell_attack(spell_id, attacker, target);
    }
    return make_weapon_attack(weapon, attacker, target);
}

pub fn make_attack(now: f64, attacker: Rc<Body>, target: Rc<Body>) -> Vec<Attack> {
    let mut result = Vec::new();
    let Some(prop_id) = attacker.equiped_weapon() else {
        return result;
    };
    let (damage, event) = make_attack_impl(&attacker, &target);
    let strike = Strike {
        target: target.clone(),
        damage,
        event,
    };
    if attacker.is_attack_ranged() {
        let effect = MissileEffect::Strike(strike);
        let missile = Missile::from_combatants(now, attacker.clone(), target.clone(), vec![effect]);
        let attack = Attack::Range(missile);
        result.push(attack);
    } else {
        let attack = Attack::Melee(strike);
        result.push(attack);
    }
    if !matches!(event, BattleEventType::Hit | BattleEventType::Crit) {
        return result;
    }
    // On a hit some weapons have special missiles
    // blood blade
    if prop_id == 248 {
        let effect = MissileEffect::Heal {
            target: attacker.clone(),
            amount: damage / 2,
            show_animation: false,
        };
        let missile = Missile::from_combatants(now, attacker.clone(), target.clone(), vec![effect]);
        let attack = Attack::Range(missile);
        result.push(attack);
    }
    // storm sword, storm hammer
    if matches!(prop_id, 390 | 249) && target.get_health() <= damage {
        let effect = MissileEffect::Explosion {
            splash_damage: 21,
            splash_kind: MissileType::Fire,
            size: ExplosionSize::Medium,
        };
        let missile = Missile::from_combatants(now, attacker.clone(), target.clone(), vec![effect]);
        let attack = Attack::Range(missile);
        result.push(attack);
    }
    // Bite, Poison
    if prop_id == 198 {
        let effect = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::random_poison(now),
        };
        let missile = Missile::from_combatants(now, attacker.clone(), target.clone(), vec![effect]);
        let attack = Attack::Range(missile);
        result.push(attack);
    }
    result
}

// TODO: should be part of static resources
fn missile_type(prop_id: u16) -> MissileType {
    match prop_id {
        // ice short bow, storm bow, ice strike, blizzard staff
        256 | 247 | 314 | 392 => MissileType::Ice,

        // fire strike, fire short bow, storm sword, storm hammer
        237 | 245 | 390 | 249 => MissileType::Fire,

        // magic long bow, magic short bow, life strike, magic shot, blood blade
        102 | 107 | 244 | 397 | 248 => MissileType::Magic,

        198 => MissileType::Poison,

        _ => MissileType::Rock,
    }
}

#[derive(Clone)]
pub enum MissileType {
    Rock,
    Magic,
    Fire,
    Bonfire,
    Ice,
    Poison,
}

#[derive(Clone)]
pub enum ExplosionSize {
    Medium,
    Large,
}

pub enum MissileEffect {
    Strike(Strike),
    SplashDamage {
        amount: i32,
    },
    Explosion {
        splash_damage: i32,
        splash_kind: MissileType,
        size: ExplosionSize,
    },
    Heal {
        target: Rc<Body>,
        amount: i32,
        show_animation: bool,
    },
    // TODO: remove this
    Spell {
        spell_id: u16,
    },
    Condition {
        target: Rc<Body>,
        condition: Condition,
    },
    AnimateDead,
    CurePoison {
        target: Rc<Body>,
    },
    DetonateCorpse {
        size: ExplosionSize,
    },
}

pub enum Attack {
    Melee(Strike),
    Range(Missile),
}

pub struct Strike {
    pub damage: i32,
    pub event: BattleEventType,
    pub target: Rc<Body>,
}

pub struct Missile {
    motion: Motion,
    x: f64,
    y: f64,
    attacker: Rc<Body>,
    kind: MissileType,
    effects: Vec<MissileEffect>,
}

/// Stripped representation to hand over to rendering
pub struct MissileInfo {
    pub x: f64,
    pub y: f64,
    pub kind: MissileType,
}

impl Missile {
    pub(crate) fn from_combatants(
        now: f64,
        attacker: Rc<Body>,
        target: Rc<Body>,
        effects: Vec<MissileEffect>,
    ) -> Self {
        let prop_id = attacker.equiped_weapon().unwrap_or(0);
        Self::new(
            now,
            attacker.x(),
            attacker.y(),
            target.x(),
            target.y(),
            attacker.clone(),
            missile_type(prop_id),
            effects,
        )
    }

    pub fn new(
        now: f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        attacker: Rc<Body>,
        kind: MissileType,
        effects: Vec<MissileEffect>,
    ) -> Self {
        let dist = ((x0 - x1).powf(2.0) + (y0 - y1).powf(2.0)).sqrt();
        let speed = 0.0075;
        Self {
            motion: Motion {
                start_t: now,
                end_t: now + (dist / speed),
                x0,
                y0,
                x1,
                y1,
            },
            x: x0,
            y: y0,
            attacker,
            kind,
            effects,
        }
    }

    pub fn attacker(&self) -> Rc<Body> {
        self.attacker.clone()
    }

    pub fn effects(&self) -> &[MissileEffect] {
        &self.effects
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn finished(&self, now: f64) -> bool {
        self.motion.end_t <= now
    }

    pub fn update(&mut self, now: f64) {
        (self.x, self.y) = self.motion.tween(now);
    }

    pub fn info(&self) -> MissileInfo {
        MissileInfo {
            x: self.x,
            y: self.y,
            kind: self.kind.clone(),
        }
    }
}

/// How much experience the player gets for defeating a monster
pub(crate) fn monster_reward(prop_id: u16, level: i32) -> (i32, i32) {
    // * = observered directly, otherwise I'm just guessing
    // TODO: should be part of static resource
    let base = match prop_id {
        // troll*
        112 => 40,

        // spider*
        200 => 31,

        // lesser fire elemental*
        236 => 32,

        // lesser air elemental*
        239 => 32,

        // lesser ice elemental*
        228 => 35,

        // lesser earth elemental*
        229 => 36,

        // greater air elemental
        238 => 37,

        // greater fire elemental
        235 => 37,

        // greater ice elemental
        224 => 40,

        // greater earth elemental
        225 => 41,

        // large rat*
        30 => 30,

        // demon
        361 => 40,

        // dragon*
        362 => 37,

        // dragonling*
        364 => 34,

        // large dog
        61 => 30,

        // large cat
        63 => 30,

        // goblin*
        29 => 33,

        // goblin lobber*
        110 => 33,

        // goblin shaman*
        // lvl 1 gp: 63
        319 => 31,

        // goblin chieftan
        115 => 34,

        // orc*
        81 => 36,

        // orc warlord*
        113 => 37,

        // bandit*
        114 => 32,

        // ogre*
        118 => 36,

        // ogre lobber*
        119 => 36,

        // female human
        41 => 30,

        // female human fighter
        40 => 32,

        // female human thief
        43 => 32,

        // female human priest
        42 => 32,

        // female human mage
        31 => 32,

        // female dwarf
        35 => 30,

        // female dwarf fighter
        32 => 32,

        // female dwarf thief
        34 => 32,

        // femal dwarf priest
        33 => 32,

        // female elf
        38 => 30,

        // female elf fighter,
        37 => 32,

        // female elf mage,
        36 => 32,

        // female elf thief
        39 => 32,

        // female dark mage
        336 => 40,

        // male human
        55 => 30,

        // male human fighter
        54 => 32,

        // male human thief
        88 => 32,

        // male human priest*
        56 => 32,

        // male human mage
        45 => 32,

        // male dwarf
        49 => 30,

        // male dwarf fighter
        46 => 32,

        // male dwarf thief
        48 => 32,

        // male dwarf priest
        47 => 32,

        // male elf
        52 => 30,

        // male elf fighter
        51 => 32,

        // male elf thief
        53 => 32,

        // male elf mage
        50 => 32,

        // death knight
        395 => 40,

        // assassin
        402 => 40,

        // giant
        44 => 40,

        // skeleton*
        57 => 33,

        // zombie*
        59 => 33,

        _ => 2, // minimum that generates a valid range below
    };
    let exp = base * level;
    let mut rng = rand::thread_rng();
    let gp = rng.gen_range(1..=exp / 2);
    (exp, gp)
}

// TODO: should be part of static resource
fn spell_base_damage(spell_id: u16) -> i32 {
    match spell_id {
        // Fire Bolt
        0 => 6,

        // Fireball
        1 => 6,

        // Meteor
        2 => 8,

        // Ice Bolt
        3 => 8,

        // Ice Spear
        4 => 12,

        // Ice storm
        5 => 12,

        // Lupo's Poison Dart
        6 => 4,

        // Lupo's Poison Strike
        7 => 6,

        // Life Drain
        14 => 4,

        // Holy Smite
        51 => 8,

        _ => 0,
    }
}

fn weapon_to_spell(prop_id: u16) -> Option<u16> {
    let spell = match prop_id {
        // Life Strike
        244 => 14, // Life Drain

        // Ice Strike
        314 => 4, // Ice Spear

        // Blizzard Staff
        392 => 5, // Ice Storm

        // Fire Strike
        237 => 0, // Fire Bolt

        _ => return None,
    };
    Some(spell)
}

pub(crate) fn spell_to_missile_type(spell_id: u16) -> MissileType {
    match spell_id {
        // Fire Bolt, Fire Ball, Meteor
        0 | 1 | 2 => MissileType::Fire,

        // Life Drain,
        14 => MissileType::Magic,

        // Ice Bolt, Ice Speark, Ice Storm
        3 | 4 | 5 => MissileType::Ice,

        // Lupo's Poison Dart, Lupo's Poison Strike
        6 | 7 => MissileType::Poison, // Bite Poison

        _ => MissileType::Magic,
    }
}

/// Animations that are drawn over bodies during battle like: Hit! Miss!
#[derive(Debug)]
pub(crate) struct BattleEvent {
    pub(crate) time: f64,
    pub(crate) kind: BattleEventType,
}

impl BattleEvent {
    pub fn new(time: f64, kind: BattleEventType) -> Self {
        Self { time, kind }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum BattleEventType {
    Hit,
    Miss,
    Crit,
    Fizzle,
    Condition1,
    Condition2,
}

/// Behavior for anything that moves from point a to point b
#[derive(Debug, Copy, Clone)]
pub(crate) struct Motion {
    pub start_t: f64,
    pub end_t: f64,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Motion {
    pub fn tween(&self, now: f64) -> (f64, f64) {
        if now >= self.end_t {
            return (self.x1, self.y1);
        }
        let x = Self::tween_impl(now, self.start_t, self.end_t, self.x0, self.x1);
        let y = Self::tween_impl(now, self.start_t, self.end_t, self.y0, self.y1);
        (x, y)
    }

    fn tween_impl(x: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
        (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
    }
}

pub(crate) fn dir(start: f64, target: f64) -> f64 {
    if start < target {
        1.0
    } else if start > target {
        -1.0
    } else {
        0.0
    }
}

fn clamp(input: i32, min: i32, max: i32) -> i32 {
    if input > max {
        max
    } else if input < min {
        min
    } else {
        input
    }
}

pub(crate) fn spell(now: f64, spell_id: u16, caster: Rc<Body>, x: f64, y: f64) -> Missile {
    let spell = MissileEffect::Spell { spell_id };
    let mut effects = vec![spell];

    // TODO: merge this map with the function below
    if spell_id == 44 {
        // Animate Dead
        effects.push(MissileEffect::AnimateDead);
    } else if spell_id == 48 {
        // Detonate Corpse
        effects.push(MissileEffect::DetonateCorpse {
            size: ExplosionSize::Medium,
        });
    } else if spell_id == 49 {
        // Corpse Bomb
        effects.push(MissileEffect::DetonateCorpse {
            size: ExplosionSize::Large,
        });
    }
    Missile::new(
        now,
        caster.x(),
        caster.y(),
        x,
        y,
        caster.clone(),
        spell_to_missile_type(spell_id),
        effects,
    )
}

pub(crate) fn targeted_spell(
    now: f64,
    spell_id: u16,
    caster: Rc<Body>,
    target: Rc<Body>,
) -> Missile {
    let damage = spell_damage(spell_id, caster.level(), target.level());
    let spell = &SPELLS[&spell_id.to_string()];
    let mut effects = Vec::new();
    if matches!(spell.target, SpellTarget::Enemy) {
        let strike = MissileEffect::Strike(Strike {
            damage,
            event: BattleEventType::Hit,
            target: target.clone(),
        });
        effects.push(strike);
    }
    if spell_id == 14 {
        // Life Drain
        let heal = MissileEffect::Heal {
            target: caster.clone(),
            amount: damage / 2,
            show_animation: false,
        };
        effects.push(heal);
    } else if spell_id == 6 || spell_id == 7 {
        // Lupo's poison dart and strike
        let poison = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::poison(now),
        };
        effects.push(poison);
    } else if spell_id == 15 {
        // Damage Shield I
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 1),
        };
        effects.push(armor);
    } else if spell_id == 16 {
        // Damage Shield II
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 2),
        };
        effects.push(armor);
    } else if spell_id == 17 {
        // Damage Shield III
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 3),
        };
        effects.push(armor);
    } else if spell_id == 13 {
        // Invisibility
        let invisible = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::invisible(now),
        };
        effects.push(invisible);
    } else if spell_id == 1 {
        // Fireball
        let invisible = MissileEffect::Explosion {
            splash_damage: 24,
            splash_kind: MissileType::Fire,
            size: ExplosionSize::Medium,
        };
        effects.push(invisible);
    } else if spell_id == 5 {
        // Ice storm
        let invisible = MissileEffect::Explosion {
            splash_damage: 36,
            splash_kind: MissileType::Ice,
            size: ExplosionSize::Large,
        };
        effects.push(invisible);
    } else if spell_id == 2 {
        // Meteor
        let invisible = MissileEffect::Explosion {
            splash_damage: 48,
            splash_kind: MissileType::Fire,
            size: ExplosionSize::Large,
        };
        effects.push(invisible);
    } else if spell_id == 42 {
        // Bless
        let bless = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::LUCK, 2),
        };
        effects.push(bless);
    } else if spell_id == 43 {
        // Curse
        let curse = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::LUCK, -2),
        };
        effects.push(curse);
    } else if spell_id == 30 {
        // Minor Heal
        let heal = MissileEffect::Heal {
            target: target.clone(),
            amount: 10,
            show_animation: true,
        };
        effects.push(heal);
    } else if spell_id == 31 {
        // Normal Heal
        let heal = MissileEffect::Heal {
            target: target.clone(),
            amount: 25,
            show_animation: true,
        };
        effects.push(heal);
    } else if spell_id == 32 {
        // Major Heal
        let heal = MissileEffect::Heal {
            target: target.clone(),
            amount: 45,
            show_animation: true,
        };
        effects.push(heal);
    } else if spell_id == 33 {
        // Greater Heal
        let heal = MissileEffect::Heal {
            target: target.clone(),
            amount: 85,
            show_animation: true,
        };
        effects.push(heal);
    } else if spell_id == 34 {
        // Divine Heal
        let heal = MissileEffect::Heal {
            target: target.clone(),
            amount: 165,
            show_animation: true,
        };
        effects.push(heal);
    } else if spell_id == 35 {
        // Holy Armor
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 5),
        };
        effects.push(armor);
    } else if spell_id == 35 {
        // Spirit Armor
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 10),
        };
        effects.push(armor);
    } else if spell_id == 35 {
        // Spirit Armor
        let armor = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::armor(now, 20),
        };
        effects.push(armor);
    } else if spell_id == 41 {
        // Clumsy
        let clumsy = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::DEXTERITY, -2),
        };
        effects.push(clumsy);
    } else if spell_id == 40 {
        // Dexterity
        let dex = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::DEXTERITY, 2),
        };
        effects.push(dex);
    } else if spell_id == 38 {
        // Strength
        let strength = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::STRENGTH, 2),
        };
        effects.push(strength);
    } else if spell_id == 39 {
        // Weaken
        let weaken = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::STRENGTH, -2),
        };
        effects.push(weaken);
    } else if spell_id == 46 {
        // Cure Poision
        let cure = MissileEffect::CurePoison {
            target: target.clone(),
        };
        effects.push(cure);
    } else if spell_id == 47 {
        // TODO: Purify
    } else if spell_id == 50 {
        // Regeneration
        let regen = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::body_regen(now),
        };
        effects.push(regen);
    } else if spell_id == 72 {
        // Mana Regen
        let regen = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::body_mana_regen(now),
        };
        effects.push(regen);
    } else if spell_id == 52 {
        // Stupify
        let stupid = MissileEffect::Condition {
            target: target.clone(),
            condition: condition::stat(now, ConditionType::INTELIGENCE, -2),
        };
        effects.push(stupid);
    }
    Missile::new(
        now,
        caster.x(),
        caster.y(),
        target.x(),
        target.y(),
        caster.clone(),
        spell_to_missile_type(spell_id),
        effects,
    )
}

// TODO: is this right?
pub(crate) fn summon(spell_id: u16) -> u16 {
    let mut rng = rand::thread_rng();
    let prop_ids = match spell_id {
        // Second Summon
        9 => vec![29, 110, 81], // goblin, goblin lobber, orc

        // Third Summon
        10 => vec![81, 118, 119], // orc, ogre, ogre lobber

        // Fourth Summon
        // lesser air elem, lesser fire elem, lesser ice elem, lesser earch elem
        11 => vec![239, 236, 228, 229],

        // Fifth Summon
        // lesser air elem, lesser fire elem, lesser ice elem, lesser earch elem
        12 => vec![238, 235, 224, 225],

        // First Summon
        8 | _ => vec![30, 29], // rat, goblin
    };
    *prop_ids.choose(&mut rng).unwrap()
}
