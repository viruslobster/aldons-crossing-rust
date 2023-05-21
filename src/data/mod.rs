use crate::game::EquipType;
use crate::thrift::save::ClassType;
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use serde::{self, de, Deserializer};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct SpriteSheetRes {
    pub frames: HashMap<String, SpriteRes>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SpriteRes {
    pub frame: RectRes,
    pub rotated: bool,
    pub trimmed: bool,
    pub sprite_source_size: RectRes,
    pub source_size: SizeRes,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct RectRes {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SizeRes {
    pub w: f64,
    pub h: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldRes {
    pub maps: HashMap<String, MapRes>,
    pub messages: HashMap<String, MessageRes>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MapRes {
    pub name: String,
    pub north: Option<u16>,
    pub south: Option<u16>,
    pub east: Option<u16>,
    pub west: Option<u16>,
    pub actors: Vec<ActorRes>,

    #[serde(deserialize_with = "from_base64")]
    pub tiles: Vec<u8>,
    pub props: Vec<PropPlacementRes>,
    pub teleports: Vec<TeleportRes>,
    pub spawners: Vec<SpawnerRes>,
}

fn from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = serde::Deserialize::deserialize(deserializer)?;
    let array_u8 = general_purpose::STANDARD
        .decode(s)
        .map_err(de::Error::custom)?;

    let mut result: Vec<u8> = Vec::new();
    for num in array_u8 {
        result.push(num >> 4);
        result.push(num & 0x0F);
    }
    Ok(result)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SpawnerRes {
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
    pub delay: f64,
    pub max_creatures: usize,
    pub monster_team: u8,
    pub monster_target: u8,
    pub level: i32,
    pub creatures: Vec<u16>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActorRes {
    pub id: u16,
    pub x: f64,
    pub y: f64,
    pub health: f64,
    pub name: String,
    pub bmp_offset: u16,
    pub is_mapped: bool,
    pub actions: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PropPlacementRes {
    pub id: u16,
    pub x: i64,
    pub y: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TeleportRes {
    pub id: u16,
    pub from_x: f64,
    pub from_y: f64,
    pub to_x: f64,
    pub to_y: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageRes {
    pub value: String,
    pub choice_a: Option<String>,
    pub choice_b: Option<String>,
    pub choice_c: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SpellTarget {
    Enemy,
    Friend,
    Corpse,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpellRes {
    pub name: String,
    pub class: String,
    pub cost: i32,
    pub level: i32,
    pub frames: Vec<u16>,

    #[serde(deserialize_with = "to_spell_target")]
    pub target: SpellTarget,
}

fn to_spell_target<'de, D>(deserializer: D) -> Result<SpellTarget, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = serde::Deserialize::deserialize(deserializer)?;
    match s {
        "friend" => Ok(SpellTarget::Friend),
        "enemy" => Ok(SpellTarget::Enemy),
        "corpse" => Ok(SpellTarget::Corpse),
        "none" => Ok(SpellTarget::None),
        _ => Err(de::Error::custom(
            "target must be one of friend | enemy | corpse | none",
        )),
    }
}

impl SpellRes {
    pub fn class(&self) -> ClassType {
        match self.class.as_str() {
            "mage" => ClassType::SPELLCASTER,
            "priest" => ClassType::PRIEST,
            _ => panic!("Invalid class: {}", self.class),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PropRes {
    pub str_id: String,
    pub name: String,
    pub blocker: bool,
    pub sight_blocker: bool,
    pub draw_depth: u8,

    #[serde(flatten)]
    pub kind: PropTypeRes,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PropTypeRes {
    Item {
        frame: u16,
    },
    Usable {
        frame: u16,
        level: i32,
        fighter: bool,
        thief: bool,
        priest: bool,
        mage: bool,
        journeyman: bool,
        buy_cost: i32,
        sell_cost: i32,
    },
    Weapon {
        frame: u16,
        damage_min: i32,
        damage_max: i32,
        delay: i32,
        level: i32,
        fighter: bool,
        thief: bool,
        priest: bool,
        mage: bool,
        journeyman: bool,
        buy_cost: i32,
        sell_cost: i32,
        equip_to: String,
    },
    Armor {
        frame: u16,
        level: i32,
        armor_value: u32,
        fighter: bool,
        thief: bool,
        priest: bool,
        mage: bool,
        journeyman: bool,
        buy_cost: i32,
        sell_cost: i32,
        equip_to: String,
    },
    Creature {
        frames: Vec<u16>,
        portrait: u16,
        weapon: u16,
        armor: u16,
        strength: i32,
        inteligence: i32,
        dexterity: i32,
        wisdom: i32,
        vitality: i32,
        luck: i32,
    },
    User {
        frames: Vec<u16>,
        portrait: u16,
        weapon: u16,
        armor: u16,
        strength: i32,
        inteligence: i32,
        dexterity: i32,
        wisdom: i32,
        vitality: i32,
        luck: i32,
    },
    Physical {
        frame: u16,
    },
    Animprop {
        frames: Vec<u16>,
    },
}

impl PropRes {
    pub fn frame(&self) -> u16 {
        match &self.kind {
            PropTypeRes::Item { frame }
            | PropTypeRes::Usable { frame, .. }
            | PropTypeRes::Weapon { frame, .. }
            | PropTypeRes::Armor { frame, .. }
            | PropTypeRes::Physical { frame } => *frame,

            PropTypeRes::User { frames, .. }
            | PropTypeRes::Creature { frames, .. }
            | PropTypeRes::Animprop { frames, .. } => frames[0],
        }
    }

    pub fn armor_value(&self) -> i32 {
        if let PropTypeRes::Armor { armor_value, .. } = &self.kind {
            *armor_value as i32
        } else {
            panic!("{:?} was used as armor but is not armor", self.kind);
        }
    }

    pub fn buy_cost(&self) -> i32 {
        // special cases
        match self.str_id.as_str() {
            // join / quit guilds
            "gldf" | "gldt" | "gldp" | "gldm" | "gldr" => return 5,

            // train if ready
            "lvl" => return 100,

            // rest at the inn
            "rest" => return 10,
            _ => {}
        };
        match &self.kind {
            PropTypeRes::Usable { buy_cost, .. }
            | PropTypeRes::Weapon { buy_cost, .. }
            | PropTypeRes::Armor { buy_cost, .. } => *buy_cost,

            PropTypeRes::User { .. }
            | PropTypeRes::Item { .. }
            | PropTypeRes::Physical { .. }
            | PropTypeRes::Creature { .. }
            | PropTypeRes::Animprop { .. } => 0,
        }
    }

    pub fn sell_cost(&self) -> i32 {
        match &self.kind {
            PropTypeRes::Usable { sell_cost, .. }
            | PropTypeRes::Weapon { sell_cost, .. }
            | PropTypeRes::Armor { sell_cost, .. } => *sell_cost,

            PropTypeRes::User { .. }
            | PropTypeRes::Item { .. }
            | PropTypeRes::Physical { .. }
            | PropTypeRes::Creature { .. }
            | PropTypeRes::Animprop { .. } => 0,
        }
    }

    pub fn restriction_str(&self) -> String {
        match &self.kind {
            PropTypeRes::Usable {
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                level,
                ..
            }
            | PropTypeRes::Weapon {
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                level,
                ..
            }
            | PropTypeRes::Armor {
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                level,
                ..
            } => {
                let class_str = if *fighter && *thief && *priest && *mage && *journeyman {
                    "All".to_string()
                } else {
                    let mut classes = vec![];
                    if *fighter {
                        classes.push("Ftr");
                    }
                    if *thief {
                        classes.push("Thf");
                    }
                    if *priest {
                        classes.push("Pst");
                    }
                    if *mage {
                        classes.push("Mage");
                    }
                    if *journeyman {
                        classes.push("Jman");
                    }
                    classes.join(" ").to_string()
                };
                format!("Class: {} Lvl: {}+", class_str, level)
            }

            PropTypeRes::Item { .. }
            | PropTypeRes::Physical { .. }
            | PropTypeRes::User { .. }
            | PropTypeRes::Creature { .. }
            | PropTypeRes::Animprop { .. } => "Class: All Lvl: 1+".to_string(),
        }
    }

    pub fn info_str(&self) -> String {
        match &self.kind {
            PropTypeRes::Weapon {
                damage_min,
                damage_max,
                delay,
                ..
            } => format!("Damage: {}-{}, Delay: {}", damage_min, damage_max, delay),

            PropTypeRes::Armor { armor_value, .. } => format!("Armor Value: {}", armor_value),

            PropTypeRes::Usable { .. }
            | PropTypeRes::User { .. }
            | PropTypeRes::Item { .. }
            | PropTypeRes::Physical { .. }
            | PropTypeRes::Creature { .. }
            | PropTypeRes::Animprop { .. } => "".to_string(),
        }
    }

    pub fn equip_type(&self) -> Option<EquipType> {
        let equip_to = match &self.kind {
            PropTypeRes::Weapon { equip_to, .. } | PropTypeRes::Armor { equip_to, .. } => equip_to,
            _ => return None,
        };
        Some(EquipType::from_str(equip_to))
    }

    pub fn can_use(&self, player_class: ClassType, player_level: i32) -> bool {
        let (level, fighter, thief, priest, mage, journeyman) = match &self.kind {
            PropTypeRes::Usable {
                level,
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                ..
            } => (level, fighter, thief, priest, mage, journeyman),
            _ => return false,
        };
        match player_class {
            ClassType::THIEF => {
                if !thief {
                    return false;
                }
            }
            ClassType::FIGHTER => {
                if !fighter {
                    return false;
                }
            }
            ClassType::PRIEST => {
                if !priest {
                    return false;
                }
            }
            ClassType::JOURNEYMAN => {
                if !journeyman {
                    return false;
                }
            }
            ClassType::SPELLCASTER => {
                if !mage {
                    return false;
                }
            }
            _ => {
                return false;
            }
        };
        if player_level < *level {
            return false;
        }
        true
    }

    pub fn can_equip(&self, player_class: ClassType, player_level: i32) -> Option<EquipType> {
        let (equip_to, level, fighter, thief, priest, mage, journeyman) = match &self.kind {
            PropTypeRes::Weapon {
                equip_to,
                level,
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                ..
            }
            | PropTypeRes::Armor {
                equip_to,
                level,
                fighter,
                thief,
                priest,
                mage,
                journeyman,
                ..
            } => (equip_to, level, fighter, thief, priest, mage, journeyman),
            _ => return None,
        };
        if player_level < *level {
            return None;
        }
        match player_class {
            ClassType::THIEF => {
                if !thief {
                    return None;
                }
            }
            ClassType::FIGHTER => {
                if !fighter {
                    return None;
                }
            }
            ClassType::PRIEST => {
                if !priest {
                    return None;
                }
            }
            ClassType::JOURNEYMAN => {
                if !journeyman {
                    return None;
                }
            }
            ClassType::SPELLCASTER => {
                if !mage {
                    return None;
                }
            }
            _ => {
                return None;
            }
        }
        Some(EquipType::from_str(equip_to))
    }

    pub fn has_inventory(&self) -> bool {
        matches!(self.kind, PropTypeRes::User { .. })
    }
}

pub static WORLD: Lazy<WorldRes> = Lazy::new(|| {
    let json = include_str!("maps.json");
    return serde_json::from_str(json).unwrap();
});

pub static SPRITES: Lazy<SpriteSheetRes> = Lazy::new(|| {
    let json = include_str!("spritesheet.json");
    return serde_json::from_str(json).unwrap();
});

pub static PROPS: Lazy<HashMap<String, PropRes>> = Lazy::new(|| {
    let json = include_str!("props.json");
    return serde_json::from_str(json).unwrap();
});

pub static SPELLS: Lazy<HashMap<String, SpellRes>> = Lazy::new(|| {
    let json = include_str!("spells.json");
    return serde_json::from_str(json).unwrap();
});
