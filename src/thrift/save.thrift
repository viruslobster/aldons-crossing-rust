// Describe how game data is to be serialized/deserialized in a backwards compatible way
struct Fog {
    1: i32 current_map;
    2: map<i32, list<bool>> fog_by_map;
}

struct Actor {
    1: i32 state;
    2: double x;
    3: double y;
    4: bool dead;
}

struct ButtonPicker {}
struct ButtonEmpty {}
struct ButtonInventory {}
struct ButtonMelee {}
struct ButtonPickUp {}
struct ButtonRanged {}
struct ButtonStats {}
struct ButtonItem {
    1: i32 prop_id;
    2: i32 quantity;
}
struct ButtonSneak {}
struct ButtonHide {}
struct ButtonSpellbook {}
struct ButtonSpell {
    1: i32 spell_id,
}

union Button {
    1: ButtonPicker picker;
    2: ButtonEmpty empty;
    3: ButtonInventory inventory;
    4: ButtonMelee melee;
    5: ButtonPickUp pickup;
    6: ButtonRanged ranged;
    7: ButtonStats stats;
    8: ButtonItem item;
    9: ButtonSneak sneak;
    10: ButtonHide hide;
    11: ButtonSpellbook spellbook;
    12: ButtonSpell spell;
}

struct Buttons {
    1: list<list<Button>> buttons;
    2: i32 tab;
}

struct Cast {
    1: i32 map_id;
    2: set<i32> quest_log;
    3: map<i32, Actor> actor_save_by_id;
    4: list<i32> vars;
}

struct Stage {
    1: i32 map_id;
    2: Body player;
    3: map<i32, list<Body>> inventory_by_id;
    4: optional Body pet;
    5: optional Body quest_pet;
    6: optional Body summoned_pet;
    7: list<Body> bodies;
    8: list<Trap> traps;
}

enum ClassType {
    Journeyman = 0,
    Fighter = 1,
    Priest = 2,
    Thief = 3,
    Spellcaster = 4,
}
 
enum RaceType {
    Human = 0,
    Elf = 1,
    Dwarf = 2,
}

enum Team {
    Player = 1,
    Enemy = 2,
    Animal = 3,
    Npc = 4,
}

enum IntelType {
    Hunter = 1,
    GuildMaster = 2,
    Npc = 3,
    MessageBearer = 4,
    Player = 5,
}

struct CastSpell {
    1: i32 spell_id;
    2: double delay;
}

struct Body {
    1: ClassType klass;
    2: i32 health;
    // 3: i32 max_health; DEPRECATED
    4: i32 magic;
    // 5: i32 max_magic; DEPRECATED
    6: i32 level;
    7: optional RaceType race;
    8: optional i32 actor_id;
    9: optional Team team;
    10: double x;
    11: double y;
    12: i32 gold;
    13: i32 prop_id;
    14: i32 exp;
    15: optional i32 portrait_id;
    16: string name
    17: optional Team hostile_to;
    18: i32 base_str;
    19: i32 base_int;
    20: i32 base_dex;
    21: i32 base_wis;
    22: i32 base_vit;
    23: i32 base_luck;
    24: optional IntelType intel_type;
    25: i32 quantity;
    //26: list<Effect> effects; DEPRECATED

    // true if this body is part of an inventory and should be equiped
    27: bool equiped;
    28: bool male;
    29: bool persist;
    30: bool frozen;
    31: bool is_pet;
    32: bool from_spawner;
    33: optional Wanderer wanderer;
    34: list<Condition> conditions;
    35: optional CastSpell last_spell;
    36: bool prefer_melee;
}

struct Wanderer {
    1: double x_min;
    2: double y_min;
    3: double x_max;
    4: double y_max;
    5: double rest_time;
}

enum ConditionType {
    Health = 0,
    Armor = 1,
    Dexterity = 2,
    Strength = 3,
    Poison = 4,
    Inteligence = 5
    Luck = 6
    Speed = 7
    Hidden = 8
    Sneaking = 9
    Mana = 10
}

enum ConditionSource {
    Potion = 0,
    Player = 1,
    Item = 2,
    Enemy = 3,
}

struct Condition {
    1: ConditionType kind;
    2: ConditionSource source;
    3: Course course;
    4: i32 magnitude;
}

struct TimedCourse {
    1: double duration;
}
struct PeriodicCourse {
    1: double period;
    2: double ms_until_update;
}
struct PeriodicTimedCourse {
    1: TimedCourse timed;
    2: PeriodicCourse periodic;
}
struct MovementCanceledCourse {}
struct RandomCourse {
    1: PeriodicCourse periodic;
    2: i32 chance;
}
struct EquipedCourse {}
struct EquipedPeriodicCourse {}

union Course {
    1: TimedCourse timed;
    2: PeriodicCourse periodic;
    3: PeriodicTimedCourse periodic_timed;
    4: MovementCanceledCourse movement_canceled;
    5: RandomCourse random;
    6: EquipedCourse equiped;
    7: EquipedPeriodicCourse equiped_periodic;
}

struct AldonGame {
    1: Stage stage;
    2: Cast cast;
    3: Fog fog;
    4: Buttons buttons;
}

enum TrapKind {
    Spark1 = 1
    Spark2 = 2
    Spark3 = 3
    Flame1 = 4
    Flame2 = 5
    Slowness = 6
    Weakness = 7
    Poison = 8
}

struct Trap {
    1: double x;
    2: double y;
    3: TrapKind kind;
}
