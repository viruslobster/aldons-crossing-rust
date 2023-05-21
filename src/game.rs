//! The backend of aldon's crossing. The root of everything except drawing and dialogs
use crate::{
    aldon_log,
    body::{self, Body},
    buttons::{Button, Buttons},
    cast::Cast,
    condition,
    data::{PROPS, WORLD},
    fog::Fog,
    js,
    stage::{PetKind, Stage},
    stats::{self, PlayerStats},
    thrift::save::{self, ClassType, IntelType, RaceType, Team},
};
use once_cell::sync::Lazy;
use serde_derive::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Write as _},
    rc::Rc,
    sync::Mutex,
};
use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol, TOutputProtocol};
use thrift::transport::{TBufferChannel, TBufferedReadTransport, TBufferedWriteTransport};

const MAX_INVENTORY_LEN: usize = 50;
const CONSOLE_LEN: usize = 10;
pub(crate) static CONSOLE: Lazy<Mutex<Console>> = Lazy::new(|| Mutex::new(Console::new()));

pub struct AldonGame {
    pub(crate) stage: Rc<Stage>,
    pub(crate) buttons: Buttons,
    pub(crate) fog: Fog,
    pub(crate) dialog: Rc<dyn Dialog>,

    cast: Cast,
    loaded: bool,
    player_on_map_edge: bool,
    prevent_teleport: Option<(f64, f64)>,
    last_update: f64,
    game_over: bool,
    input_cooldown_deadline: f64,
}

impl AldonGame {
    pub fn new(dialog: Rc<dyn Dialog>) -> AldonGame {
        let stage = Rc::new(Stage::new(1, dialog.clone()));
        let cast = Cast::new(stage.clone(), dialog.clone());

        AldonGame {
            stage: stage.clone(),
            cast,
            dialog: dialog.clone(),
            buttons: Buttons::new(stage.clone(), dialog.clone()),
            loaded: false,
            player_on_map_edge: false,
            prevent_teleport: None,
            fog: Fog::new(),
            last_update: 0.0,
            game_over: false,
            input_cooldown_deadline: 0.0,
        }
    }

    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn game_over(&self) -> bool {
        self.game_over
    }

    pub fn new_game(
        &mut self,
        name: String,
        race: RaceType,
        portrait: u16,
        strength: i32,
        dexterity: i32,
        vitality: i32,
        intelligence: i32,
        wisdom: i32,
        luck: i32,
    ) {
        *self = Self::new(self.dialog.clone());

        let player_name = if name.len() > 0 {
            name
        } else {
            "Enter Name".to_string()
        };
        let player = self.stage.create_body(player_name, Some(0), 55, 12.0, 3.0);
        player.set_portrait(portrait);
        player.male.set(is_portrait_male(portrait));
        player.set_intel(IntelType::PLAYER);
        player.set_team(Team::PLAYER);
        player.race.set(Some(race));
        player.base_str.set(strength);
        player.base_dex.set(dexterity);
        player.base_vit.set(vitality);
        player.base_int.set(intelligence);
        player.base_wis.set(wisdom);
        player.base_luck.set(luck);
        player.class.set(ClassType::JOURNEYMAN);
        player.set_level(1);
        let c = condition::body_regen(self.last_update);
        player.add_condition_no_log(c);
        let c = condition::body_mana_regen(self.last_update);
        player.add_condition_no_log(c);
        /*
        player.give_exp(9999999);
        player.give_gold(99999);
        player.set_class(ClassType::FIGHTER);
        player.set_level(16);
        player.give_item(120);
        player.give_item(262);
        player.give_item(259);
        player.give_item(188);
        player.give_item(178);
        player.give_item(177);
        player.give_item(175);
        player.give_item(174);
        player.give_item(173);
        player.give_item(322);
        */

        self.fog = Fog::new();
        self.load_map(1, 12.0, 3.0);
    }

    pub fn load_map(&mut self, map_id: u16, x: f64, y: f64) {
        self.loaded = true;
        js::clear_logs();
        let player = self.stage.get_player();
        player.set_x(x);
        player.set_y(y);
        player.clear_walk_goal();
        self.player_on_map_edge =
            player.x() == 0.0 || player.x() == 23.0 || player.y() == 0.0 || player.y() == 23.0;

        // cast.load_map must be called before stage.load_map because the
        // old bodies on the stage need to be saved before they are destroyed
        // by loading another map
        self.cast.load_map(map_id, false);

        self.stage.load_map(map_id, false /*from_save*/);
        self.fog.load_map(map_id);
    }

    pub fn update(&mut self, now: f64) {
        self.last_update = now;
        if !self.loaded || self.game_over {
            return;
        }
        self.cast.act(now);
        self.stage.update(now);
        self.buttons.update();

        let player = self.stage.get_player();
        let (from_x, from_y) = player.moving_from();
        self.fog.look(from_x, from_y, &self.stage.sight());

        let teleporter = self.stage.teleporter_at(player.x(), player.y());

        if let Some((x, y)) = self.prevent_teleport {
            if player.x() != x || player.y() != y {
                self.prevent_teleport = None;
            }
        } else if let Some((map_id, x, y)) = teleporter {
            self.prevent_teleport = Some((x, y));
            if self.stage.map_id() == map_id {
                self.stage.set_actor_body_loc(0, x, y);
                let henchmen = player.henchmen();
                for body in henchmen {
                    body.set_x(x);
                    body.set_y(y);
                    body.clear_walk_goal();
                    body.clear_attack();
                }
            } else {
                self.load_map(map_id, x, y);
            }
            return;
        }

        if self.player_on_map_edge {
            if player.x() > 0.0 && player.x() < 23.0 && player.y() > 0.0 && player.y() < 23.0 {
                self.player_on_map_edge = false;
            }
        } else {
            let map = self.stage.map.get();
            let north = map.north;
            let south = map.south;
            let east = map.east;
            let west = map.west;

            if player.x() == 0.0 {
                if let Some(map_id) = west {
                    self.load_map(map_id, 23.0, player.y());
                }
            } else if player.x() == 23.0 {
                if let Some(map_id) = east {
                    self.load_map(map_id, 0.0, player.y());
                }
            } else if player.y() == 0.0 {
                if let Some(map_id) = north {
                    self.load_map(map_id, player.x(), 23.0);
                }
            } else if player.y() == 23.0 {
                if let Some(map_id) = south {
                    self.load_map(map_id, player.x(), 0.0);
                }
            }
        }

        let mut game_state = self.cast.state.borrow_mut();
        let map_change_request = game_state.map_change_request.take();
        drop(game_state);

        if let Some((map_id, x, y)) = map_change_request {
            self.prevent_teleport = Some((x, y));
            self.load_map(map_id, x, y);
        }
        if player.get_health() <= 0 && player.death_time() + 100.0 < now {
            self.game_over = true;
        }
    }

    /// Control input for anything that isn't the stage, i.e. the buttons for stats, inventory,
    /// etc. Uses game coordinates.
    pub fn input_buttons(&mut self, x: f64, y: f64, touch_up: bool) {
        self.buttons.input(x, y, touch_up);
    }

    /// Control input for the stage, i.e. moving the player around. Uses game coordinates.
    pub fn input_stage(&mut self, x: f64, y: f64) {
        if self.last_update < self.input_cooldown_deadline {
            return;
        }
        let player = self.stage.get_player();
        let mut interactable = player.henchmen();
        interactable.push(player.clone());

        for body in interactable {
            if rect_contains(body.x(), body.y(), 1.0, 1.0, x, y) {
                let body_prop = &PROPS[&body.prop_id.to_string()];

                match self.buttons.toggled_menu_button() {
                    Some(Button::Inventory) => {
                        if !body_prop.has_inventory() {
                            return;
                        }
                        if body.inventory_len() == 0 {
                            aldon_log!("*Nothing in inventory*");
                        } else {
                            js::log(&format!("actor_id: {:?}", body.actor_id));
                            self.dialog.execute_trade(
                                TransactionType::Inventory,
                                body.clone(),
                                body.inventory(),
                            );
                        }
                        self.buttons.untoggle_menu_button();
                        return;
                    }
                    Some(Button::PickUp) => {
                        if !body_prop.has_inventory() {
                            return;
                        }
                        let items = self.stage.pick_up_at(body.x(), body.y());
                        if items.len() == 0 {
                            aldon_log!("*Nothing to pickup*");
                        } else {
                            self.dialog.pickup(body.clone(), items);
                        }
                        self.buttons.untoggle_menu_button();
                        return;
                    }
                    Some(Button::Stats) => {
                        let stats = body.stats();
                        self.dialog.stats(&stats);
                        self.buttons.untoggle_menu_button();
                        return;
                    }
                    _ => {}
                }
            }
        }
        if let Some(spell_id) = self.buttons.active_spell() {
            let ok = self
                .stage
                .cast_spell(spell_id, player, x.floor(), y.floor());
            if ok {
                // after casting a spell a touch will still be held down
                // for a couple of milliseconds, which will cause the player
                // to move unless we stop taking input for a little bit
                self.input_cooldown_deadline = self.last_update + 200.0;
                self.buttons.maybe_untoggle_spell(spell_id);
                return;
            }
        }
        self.stage.input(x, y);
    }

    pub fn save(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        if !self.loaded() || self.game_over() {
            return Err(Box::new(GameNotLoadedError {}));
        }
        // shadows for each map
        // state of each actor
        // variables
        let cast = self.cast.save();
        let fog = self.fog.save();
        let buttons = self.buttons.save();
        let stage = self.stage.save(self.last_update);
        let save = save::AldonGame::new(stage, cast, fog, buttons);

        let mut channel = TBufferChannel::with_capacity(
            0,      // read_capacity
            500000, // write_capacity
        );
        let transport = TBufferedWriteTransport::new(&mut channel);
        let mut protocol = TCompactOutputProtocol::new(transport);
        save.write_to_out_protocol(&mut protocol)?;
        protocol.flush().unwrap();
        Ok(channel.write_bytes())
    }

    pub fn load_save(&mut self, save_bytes: Vec<u8>) -> Result<(), Box<dyn Error>> {
        js::log(&format!("load: {}", save_bytes.len().to_string()));

        let mut channel = TBufferChannel::with_capacity(
            500000, // read_capacity
            0,      // write_capacity
        );
        channel.set_readable_bytes(&save_bytes);
        let transport = TBufferedReadTransport::new(channel);
        let mut protocol = TCompactInputProtocol::new(transport);
        let save = save::AldonGame::read_from_in_protocol(&mut protocol)?;

        let save_stage = save
            .stage
            .ok_or(InvalidDataError::new("stage field missing"))?;

        let save_cast = save
            .cast
            .ok_or(InvalidDataError::new("cast field missing"))?;

        let save_fog = save.fog.ok_or(InvalidDataError::new("fog field missing"))?;

        let save_buttons = save
            .buttons
            .ok_or(InvalidDataError::new("buttons field missing"))?;

        let stage = Stage::from_save(self.last_update, &save_stage, self.dialog.clone())
            .map_err(|err| InvalidDataError::new(&format!("stage: {}", err)))?;

        let stage = Rc::new(stage);

        let cast = Cast::from_save(&save_cast, stage.clone(), self.dialog.clone())
            .map_err(|err| InvalidDataError::new(&format!("cast: {}", err)))?;

        let fog = Fog::from_save(&save_fog)
            .map_err(|err| InvalidDataError::new(&format!("fog: {}", err)))?;

        let buttons = Buttons::from_save(&save_buttons, stage.clone(), self.dialog.clone())
            .map_err(|err| InvalidDataError::new(&format!("buttons: {}", err)))?;

        self.buttons = buttons;
        self.stage = stage;
        self.cast = cast;
        self.fog = fog;
        self.loaded = true;
        self.game_over = false;
        Ok(())
    }

    pub fn send_response(&mut self, actor_id: u16, raw_response: u8) {
        self.cast.send_response(actor_id, raw_response.into());
    }

    /// Attempts to equip inventory item at idx to the player, returns success.
    pub fn equip(&mut self, body: &Body, index: usize) -> bool {
        let transaction = self.dialog.get_transaction();
        let item = transaction[index].clone();
        body.equip(self.last_update, item)
    }

    /// Attemps to sell inventory item at idx, returns success
    pub fn sell(&mut self, body: &Body, index: usize) -> bool {
        let transaction = self.dialog.get_transaction();
        if index >= transaction.len() {
            return false;
        }
        let item = transaction[index].clone();
        let prop_id = item.prop_id;

        if body.relinquish(item) {
            self.dialog.remove_item(index);
            let prop = &PROPS[&prop_id.to_string()];
            let cost = prop.sell_cost();
            body.give_gold(cost);
            true
        } else {
            false
        }
    }

    /// Attempts to buy the item at idx. Returns success
    pub fn buy(&mut self, body: Rc<Body>, index: usize) -> bool {
        let item = self.dialog.get_transaction()[index].clone();
        let prop_id = item.prop_id;
        let prop = &PROPS[&prop_id.to_string()];
        let cost = prop.buy_cost();

        if body.gold.get() < cost {
            return false;
        }
        let (handled, ok) = self.buy_special(body.clone(), prop_id);
        if handled {
            if ok {
                body.take_gold(cost);
            }
            return ok;
        }
        if body.inventory_len() >= MAX_INVENTORY_LEN {
            return false;
        }
        let quantity = body.item_quantity(prop_id);
        if quantity >= 10 {
            return false;
        }
        body.give_item(prop_id);
        body.take_gold(cost);
        true
    }

    /// These special items don't add something to your inventory but do
    /// something else, like give you a pet or level you up.
    /// Returns (handled: bool, ok: bool) where handled is true if prop_id
    /// is a type of prop that needs to be handled by buy_special and ok represents
    /// if the buy was succesful (implies handled is true).
    fn buy_special(&mut self, body: Rc<Body>, prop_id: u16) -> (bool, bool) {
        // Join Fighter's Guild
        if prop_id == 14 {
            if body.class.get() == ClassType::FIGHTER {
                return (true, false);
            }
            body.set_class(ClassType::FIGHTER);
            return (true, true);
        }
        // Train if ready
        if prop_id == 13 {
            let level = body.level();
            if level >= stats::max_level(body.exp()) {
                return (true, false);
            }
            body.set_level(level + 1);
            let stats = body.stats();
            self.dialog.stats(&stats);
            return (true, true);
        }
        // Rest at Inn
        if prop_id == 90 {
            body.set_health(i32::MAX);
            for pet in body.henchmen() {
                pet.set_health(i32::MAX);
            }
            return (true, true);
        }
        // Quit current guild
        if prop_id == 18 {
            if body.class.get() != ClassType::JOURNEYMAN {
                body.set_level(1);
            }
            self.buttons.clear_class_specific();
            body.set_class(ClassType::JOURNEYMAN);
            return (true, true);
        }
        // Join the Thieves' Guild
        if prop_id == 15 {
            if body.class.get() != ClassType::JOURNEYMAN {
                return (true, false);
            }
            self.buttons.set_tab_button(0, 6, Button::Sneak);
            self.buttons.set_tab_button(0, 8, Button::Hide);
            body.set_class(ClassType::THIEF);
            return (true, true);
        }
        // Join the Priesthood
        if prop_id == 16 {
            if body.race.get() == Some(RaceType::ELF) {
                return (true, false);
            }
            if body.class.get() != ClassType::JOURNEYMAN {
                return (true, false);
            }
            self.buttons
                .set_tab_button(0, 8, Button::Spellbook { spell_id: None });

            body.set_class(ClassType::PRIEST);
            return (true, true);
        }
        // Join the Mages' Guild
        if prop_id == 17 {
            if body.race.get() == Some(RaceType::DWARF) {
                return (true, false);
            }
            if body.class.get() != ClassType::JOURNEYMAN {
                return (true, false);
            }
            self.buttons
                .set_tab_button(0, 8, Button::Spellbook { spell_id: None });

            body.set_class(ClassType::SPELLCASTER);
            return (true, true);
        }
        // dog, cat
        if prop_id == 61 || prop_id == 63 {
            if body.pet.borrow().is_some() {
                return (true, false);
            }
            let name = body::pet_name();
            self.stage.create_pet(
                name,
                prop_id,
                body,
                PetKind::Normal,
                None, // actor_id
            );
            return (true, true);
        }
        return (false, false);
    }

    /// unequips the inventory item at idx
    pub fn unequip(&mut self, body: &Body, index: usize) -> bool {
        let item = self.dialog.get_transaction()[index].clone();
        body.unequip(item)
    }

    /// attempts to drop inventory item at idx, returns success.
    pub fn relinquish(&mut self, body: &Body, index: usize) -> bool {
        let transaction = self.dialog.get_transaction();
        let item = transaction[index].clone();
        let result = body.relinquish(item.clone());

        if result {
            let (x, y) = body.moving_from();
            item.set_position(x, y);
            item.persist();
            self.dialog.remove_item(index);
            self.stage.place_body(item);
            true
        } else {
            false
        }
    }

    /// attempts to pickup item at idx, returns success.
    pub fn pickup(&mut self, body: &Body, index: usize) -> bool {
        if body.inventory_len() < MAX_INVENTORY_LEN {
            let transaction = self.dialog.get_transaction();
            let item = &transaction[index];
            if item.groupable() {
                let quantity = body.item_quantity(item.prop_id);
                if quantity >= 10 {
                    return false;
                }
            }
            let item = self.dialog.remove_item(index);

            body.give_body_item(item.clone());
            self.stage.remove_body_ref(item);
            true
        } else {
            false
        }
    }

    /// returns the amount of gold available to the player
    pub fn gold(&self, body: &Body) -> i32 {
        body.gold.get()
    }

    pub fn is_equiped(&self, body: &Body, index: usize) -> Option<EquipType> {
        let len = self.dialog.get_transaction().len();
        if index >= len {
            return None;
        }
        let item = self.dialog.get_transaction()[index].clone();
        body.is_equiped(item)
    }

    pub fn name(&self) -> String {
        if !self.loaded {
            // There is no player to get
            return "".to_string();
        }
        let player = self.stage.get_player();
        player.name.clone()
    }

    pub fn log(&self, message: &str) {
        aldon_log!("{}", message);
    }

    pub fn set_button(&mut self, button_idx: usize, button: Button) {
        self.buttons.set_button(button_idx, button);
    }

    pub fn untoggle_menu_button(&mut self) {
        self.buttons.untoggle_menu_button();
    }

    pub fn use_transaction_item(&mut self, body: &Body, index: usize) {
        let transaction = self.dialog.get_transaction();
        if index >= transaction.len() {
            return;
        }
        let item = transaction[index].clone();
        let prop_id = item.prop_id;

        self.use_item(body, prop_id);

        // If we use up the item we have to remove it in the front end, from the transaction, and
        // the inventory. Should probably make this simpler
        let quant = body.item_quantity(prop_id);
        js::log(&format!("prop_id: {}, quantity: {}", prop_id, quant));
        if quant == 0 {
            self.dialog.remove_item(index);
        }
    }

    pub fn use_item(&mut self, body: &Body, prop_id: u16) {
        self.stage.use_item(&body, prop_id);
    }

    pub fn quests(&self) -> Vec<String> {
        let state = self.cast.state.borrow();
        state
            .quest_log
            .iter()
            .map(|str_id| WORLD.messages[&str_id.to_string()].value.clone())
            .collect()
    }

    /// Activates the first spellbook button and get ready to cast a spell
    pub fn set_spellbook_spell(&mut self, spell_id: u16) {
        self.buttons.set_spellbook_spell(spell_id);
    }
}

/// The different dialogs that must be implemented
pub trait Dialog {
    fn tell_message(&self, title: &str, portrait_id: u16, msg_id: u16, from_actor: u16);

    fn execute_trade(&self, kind: TransactionType, body: Rc<Body>, items: Vec<Rc<Body>>);

    fn pickup(&self, body: Rc<Body>, items: Vec<Rc<Body>>);

    fn buy_sell(&self, body: Rc<Body>, items: Vec<Rc<Body>>, kind: TransactionType);

    fn pick_button(&self, button_idx: usize, buttons: Vec<Button>);

    fn stats(&self, stats: &PlayerStats);

    fn spellbook(&self, spells: &[u16]);

    /// The "transaction" is the list of things bodies that exists in most dialogs
    fn get_transaction(&self) -> Vec<Rc<Body>>;
    fn remove_item(&self, index: usize) -> Rc<Body>;
}

// TODO: get rid of this
#[derive(Debug, Copy, Clone)]
pub enum TransactionType {
    Buy,
    Sell,
    PickUp,
    Inventory,
}

/// Get debugging logs, not just the aldon game logs
pub fn get_logs() -> String {
    let logs = js::DEBUG_LOGS.lock().unwrap();
    logs.join("\n")
}

/// Convenience macro to log to the aldon game console
mod macros {
    #[macro_export]
    macro_rules! aldon_log {
        ($($arg:tt)*) => {
            let mut console = CONSOLE.lock().unwrap();
            write!(console, $($arg)*).unwrap();
            write!(console, "\n").unwrap();
            js::log(&format!($($arg)*));
            drop(console);
        };
    }
}

// The aldon game console (a circular buffer)
#[derive(Debug)]
pub(crate) struct Console {
    lines: [Option<String>; CONSOLE_LEN],
    idx: usize,
}

impl Console {
    fn new() -> Self {
        Self {
            lines: [None, None, None, None, None, None, None, None, None, None],
            idx: 0,
        }
    }

    pub fn iter(&self) -> ConsoleIter {
        ConsoleIter {
            lines: &self.lines,
            idx: self.idx,
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.len() == 0 {
            return Ok(());
        }
        if let Some(line) = &mut self.lines[self.idx] {
            line.push_str(s);
        } else {
            self.lines[self.idx] = Some(String::from(s));
        }
        let last = s.chars().last().unwrap();
        if last == '\n' {
            self.idx = (self.idx + 1) % CONSOLE_LEN;
            self.lines[self.idx] = None;
        }
        Ok(())
    }
}

pub(crate) struct ConsoleIter<'a> {
    lines: &'a [Option<String>],
    idx: usize,
}

impl<'a> Iterator for ConsoleIter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<&'a String> {
        self.idx = if self.idx == 0 {
            self.lines.len() - 1
        } else {
            self.idx - 1
        };
        self.lines[self.idx].as_ref()
    }
}

#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum EquipType {
    Melee,
    Range,
    Head,
    Neck,
    Chest,
    Arm,
    Hand,
    Leg,
    Foot,
    Back,
    Shield,
    Ring1,
    Ring2,
    Suit,
}

impl EquipType {
    pub fn from_str(kind: &str) -> EquipType {
        match kind {
            "melee" => EquipType::Melee,
            "range" => EquipType::Range,
            "head" => EquipType::Head,
            "neck" => EquipType::Neck,
            "chest" => EquipType::Chest,
            "arm" => EquipType::Arm,
            "hand" => EquipType::Hand,
            "leg" => EquipType::Leg,
            "foot" => EquipType::Foot,
            "back" => EquipType::Back,
            "shield" => EquipType::Shield,
            "ring" => EquipType::Ring1,
            "suit" => EquipType::Suit,
            _ => panic!("uknown equip type '{}'", kind),
        }
    }
}

fn rect_contains(left: f64, top: f64, width: f64, height: f64, x: f64, y: f64) -> bool {
    (x >= left) && (x <= (left + width)) && (y >= top) && (y <= (top + height))
}

/// TODO: should be in static resource
fn is_portrait_male(portrait: u16) -> bool {
    match portrait {
        600 | 601 | 602 | 603 | 604 | 605 | 606 | 607 => true,
        650 | 651 | 652 | 653 | 654 | 655 | 656 | 657 => false,
        _ => panic!("Invalid portrait id '{}'", portrait),
    }
}

#[derive(Debug)]
pub struct InvalidDataError {
    message: String,
}

impl InvalidDataError {
    pub fn new(message: &str) -> Self {
        Self {
            message: String::from(message),
        }
    }
}

impl Error for InvalidDataError {}

impl fmt::Display for InvalidDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Debug)]
pub struct GameNotLoadedError {}

impl Error for GameNotLoadedError {}

impl fmt::Display for GameNotLoadedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "game not loaded")
    }
}
