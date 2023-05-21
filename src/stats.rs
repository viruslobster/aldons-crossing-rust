//! Player statistics like strength and dexterity
#[derive(Clone)]
pub struct PlayerStats {
    pub name: String,
    pub class: String,
    pub race: String,
    pub level: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub ac: i32,
    pub exp: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub gp: i32,
    pub str: i32,
    pub int: i32,
    pub dex: i32,
    pub vit: i32,
    pub wis: i32,
    pub luck: i32,
    pub portrait: u16,
}

pub fn strength_to_hit_bonus(strength: i32) -> i32 {
    match strength {
        i32::MIN..=2 => -20,
        3 => -15,
        4 => -10,
        5 => -5,
        6 => -3,
        7 => -1,
        8 => 0,
        9 => 1,
        10 => 3,
        11 => 5,
        12 => 8,
        13 => 10,
        14 => 12,
        n @ 15..=43 => 12 + 3 * (n - 14),
        44..=i32::MAX => 99,
    }
}

pub fn strength_to_damage(strength: i32) -> i32 {
    match strength {
        n @ 7..=8 => n - 8,
        n @ i32::MIN..=6 => (n - 9) / 2,
        n @ 9..=i32::MAX => (n - 7) / 2,
    }
}

pub fn dexterity_to_hit_bonus(dexterity: i32) -> i32 {
    match dexterity {
        i32::MIN..=2 => -15,
        3 => -10,
        4 => -5,
        5 => -5,
        6 => -5,
        7 => -2,
        8 => 0,
        9 => 2,
        10 => 5,
        11 => 5,
        12 => 8,
        13 => 8,
        14 => 10,
        15 => 12,
        16 => 15,
        n @ 17..=99 => n,
        100..=i32::MAX => 99,
    }
}

pub fn dexterity_to_armor_class(dexterity: i32) -> i32 {
    2 * (dexterity - 8)
}

pub fn hide_chance(dexterity: i32, level: i32) -> i32 {
    let base = if dexterity == 8 {
        46
    } else if dexterity > 8 {
        50 + 3 * (dexterity - 9)
    } else {
        42 + 3 * (dexterity - 7)
    };
    clamp(base + level * 4, 1, 99)
}

pub fn sneak_chance(dexterity: i32, level: i32) -> i32 {
    let base = if dexterity == 8 {
        21
    } else if dexterity > 8 {
        25 + 3 * (dexterity - 9)
    } else {
        17 + 3 * (dexterity - 7)
    };
    clamp(base + level * 4, 1, 99)
}

pub fn vitality_to_hit_points(vitality: i32) -> i32 {
    ((vitality as f64 - 8.0) / 2.0).ceil() as i32
}

pub fn intelligence_to_chance_cast(intelligence: i32) -> i32 {
    // TODO: is this right?
    match intelligence {
        i32::MIN..=1 => 10,
        2 => 20,
        3 => 30,
        4 => 35,
        5 => 45,
        6 => 50,
        7 => 53,
        8 => 56,
        9 => 60,
        10 => 65,
        11 => 70,
        12 => 75,
        13 => 80,
        14 => 85,
        15 => 90,
        16 => 95,
        17 => 98,
        18..=i32::MAX => 99,
    }
}

pub fn wisdom_to_mana(wisdom: i32) -> i32 {
    wisdom
}

pub fn luck_to_modifier(luck: i32) -> i32 {
    luck - 8
}

/// The maximum possible level a player could train to with the given exp
pub fn max_level(exp: i32) -> i32 {
    let mut level = 1;
    let mut level_exp = 0;
    let mut delta = 1000;
    while level_exp + delta <= exp {
        level_exp += delta;
        delta += 1000;
        level += 1;
    }
    level
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
