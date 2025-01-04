//! The frontend of aldon's crossing. Implements drawing and dialogs.
//! TODO: this should really be a separate module from the backend
use body::Body;
use data::{PropTypeRes, RectRes, SpellRes, PROPS, SPELLS, SPRITES, WORLD};
use game::{AldonGame, Dialog, EquipType, TransactionType};
use js_sys;
use std::{
    cell::{Cell, RefCell},
    panic,
    rc::Rc,
};
use thrift::save::{ClassType, RaceType};
use wasm_bindgen::prelude::*;
use web_sys::*;

pub mod game;
pub mod stage;

mod actor;
mod body;
mod buttons;
mod cast;
mod combat;
mod condition;
pub mod data;
mod draw;
mod fog;
mod js;
mod search;
mod stats;
mod thrift;

#[wasm_bindgen]
pub struct AldonHtmlCanvasGame {
    game: AldonGame,

    // The part of the game you can currently see
    canvas: HtmlCanvasElement,

    // The full screen actual game
    stage_canvas: HtmlCanvasElement,

    // The canvas containing fog of war
    fog_canvas: HtmlCanvasElement,

    // The background tiles of the stage that do not change very much
    tile_canvas: HtmlCanvasElement,
    spritesheet: HtmlImageElement,
    scale: f64,
    drawn_once: Cell<bool>,
    rendered_map: Option<u16>,
    last_animation_idx: u16,
    last_fog: [bool; 576],
}

#[wasm_bindgen]
extern "C" {
    pub type AldonDialog;

    #[wasm_bindgen(method)]
    fn tell(
        this: &AldonDialog,
        title: &str,
        portrait_x: f64,
        portrait_y: f64,
        portrait_w: f64,
        portrait_h: f64,
        msg: &str,
        choiceA: Option<&str>,
        choiceB: Option<&str>,
        choiceC: Option<&str>,
        fromActor: u16,
    );

    #[wasm_bindgen(method)]
    fn inventory(
        this: &AldonDialog,
        body: BodyWrapper,
        items: Vec<TransactionItem>,
        buttons: Vec<String>,
    );

    #[wasm_bindgen(method)]
    fn pickup(this: &AldonDialog, body: BodyWrapper, items: Vec<TransactionItem>);

    #[wasm_bindgen(method)]
    fn buySell(this: &AldonDialog, body: BodyWrapper, items: Vec<TransactionItem>, kind: &str);

    #[wasm_bindgen(method)]
    fn pickButton(this: &AldonDialog, button_idx: usize, buttons: Vec<Button>);

    #[wasm_bindgen(method)]
    fn stats(this: &AldonDialog, stats: PlayerStats);

    #[wasm_bindgen(method)]
    fn spellbook(this: &AldonDialog, spells: Vec<Spell>);
}

#[wasm_bindgen]
impl AldonHtmlCanvasGame {
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas: &HtmlCanvasElement,
        spritesheet: &HtmlImageElement,
        aldon_dialog: AldonDialog,
    ) -> Self {
        panic::set_hook(Box::new(panic_hook));

        let dialog = Rc::new(HtmlDialog::new(aldon_dialog));
        Self {
            canvas: canvas.clone(),
            stage_canvas: new_canvas(384 * 4, 384 * 4).unwrap(),
            tile_canvas: new_canvas(384 * 4, 384 * 4).unwrap(),
            fog_canvas: new_canvas(384 * 4, 384 * 4).unwrap(),
            spritesheet: spritesheet.clone(),
            game: AldonGame::new(dialog),
            drawn_once: Cell::new(false),
            scale: 1.0,
            rendered_map: None,
            last_animation_idx: 0,
            last_fog: [false; 576],
        }
    }

    #[wasm_bindgen]
    pub fn load_map(&mut self, map_id: js_sys::BigInt) {
        let id = map_id.as_f64().unwrap() as u16;
        self.game.load_map(id, 10.0, 10.0);
    }

    #[wasm_bindgen]
    pub fn new_game(
        &mut self,
        name: js_sys::JsString,
        race: js_sys::JsString,
        portrait: js_sys::BigInt,
        strength: js_sys::BigInt,
        dexterity: js_sys::BigInt,
        vitality: js_sys::BigInt,
        intelligence: js_sys::BigInt,
        wisdom: js_sys::BigInt,
        luck: js_sys::BigInt,
    ) {
        self.game.new_game(
            name.as_string().unwrap(),
            RaceType::from_str(&race.as_string().unwrap()),
            portrait.as_f64().unwrap() as u16,
            strength.as_f64().unwrap() as i32,
            dexterity.as_f64().unwrap() as i32,
            vitality.as_f64().unwrap() as i32,
            intelligence.as_f64().unwrap() as i32,
            wisdom.as_f64().unwrap() as i32,
            luck.as_f64().unwrap() as i32,
        );
    }

    #[wasm_bindgen]
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    #[wasm_bindgen]
    pub fn save(&self) -> Result<Vec<u8>, JsValue> {
        let bytes = self
            .game
            .save()
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        js::log(&format!("save: {}", bytes.len().to_string()));
        Ok(bytes)
    }

    #[wasm_bindgen]
    pub fn name(&self) -> String {
        self.game.name()
    }

    #[wasm_bindgen]
    pub fn log(&self, message: &str) {
        self.game.log(message);
    }

    #[wasm_bindgen]
    pub fn load_save(&mut self, array_buffer: js_sys::ArrayBuffer) {
        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();

        if let Err(error) = self.game.load_save(bytes) {
            js::log(&format!("Failed to load save: {}", error));
        }
    }

    #[wasm_bindgen]
    pub fn show_stats(&self, body: &BodyWrapper) {
        let stats = body.0.stats();
        self.game.dialog.stats(&stats);
    }

    fn stage_offset(&self) -> (f64, f64) {
        let (stage_width, stage_height) = self.stage_size();
        let player = self.game.stage.get_player();
        let mut offset_x = 16.0 * player.x() - stage_width / 2.0;
        offset_x = offset_x.max(0.0);
        offset_x = offset_x.min(24.0 * 16.0 - stage_width);
        let mut offset_y = player.y() * 16.0 - stage_height / 2.0;
        offset_y = offset_y.max(0.0);
        offset_y = offset_y.min(24.0 * 16.0 - stage_height);

        (offset_x, offset_y)
    }

    #[wasm_bindgen]
    pub fn update(&mut self, now_js: js_sys::BigInt) {
        let now = now_js.as_f64().unwrap();
        self.game.update(now);
    }

    fn input(&mut self, x: f64, y: f64, touch_up: bool) {
        let (stage_width, stage_height) = self.stage_size();

        if x < stage_width && y < stage_height {
            let (offset_x, offset_y) = self.stage_offset();
            let x = x + offset_x;
            let y = y + offset_y;
            self.game.input_stage(x / 16.0, y / 16.0);
        } else {
            let x = x - stage_width;
            self.game.input_buttons(x / 16.0, y / 16.0, touch_up);
        }
    }

    #[wasm_bindgen]
    pub fn input_up(&mut self, raw_x: js_sys::BigInt, raw_y: js_sys::BigInt) {
        if !self.game.loaded() {
            return;
        }
        let x = raw_x.as_f64().unwrap();
        let y = raw_y.as_f64().unwrap();
        self.input(x, y, true /* touch_up */);
    }

    #[wasm_bindgen]
    pub fn input_down(&mut self, raw_x: js_sys::BigInt, raw_y: js_sys::BigInt) {
        if !self.game.loaded() {
            return;
        }
        let x = raw_x.as_f64().unwrap();
        let y = raw_y.as_f64().unwrap();
        self.input(x, y, false /* touch_down */);
    }

    #[wasm_bindgen]
    pub fn send_response(&mut self, actor_id: js_sys::BigInt, response: js_sys::BigInt) {
        self.game.send_response(
            actor_id.as_f64().unwrap() as u16,
            response.as_f64().unwrap() as u8,
        );
    }

    #[wasm_bindgen]
    pub fn equip(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.equip(&body.0, index)
    }

    #[wasm_bindgen]
    pub fn sell(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.sell(&body.0, index)
    }

    #[wasm_bindgen]
    pub fn buy(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.buy(body.0.clone(), index)
    }

    #[wasm_bindgen]
    pub fn use_item(&mut self, body: &BodyWrapper, index: usize) {
        self.game.use_transaction_item(&body.0, index);
    }

    #[wasm_bindgen]
    pub fn unequip(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.unequip(&body.0, index)
    }

    #[wasm_bindgen]
    pub fn drop(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.relinquish(&body.0, index)
    }

    #[wasm_bindgen]
    pub fn pickup(&mut self, body: &BodyWrapper, index: usize) -> bool {
        self.game.pickup(&body.0, index)
    }

    #[wasm_bindgen]
    pub fn gold(&self, body: &BodyWrapper) -> i32 {
        self.game.gold(&body.0)
    }

    #[wasm_bindgen]
    pub fn is_equiped(&self, body: &BodyWrapper, index: usize) -> Option<String> {
        match self.game.is_equiped(&body.0, index) {
            None => None,
            Some(EquipType::Melee) => Some(String::from("MELEE")),
            Some(EquipType::Range) => Some(String::from("RANGE")),
            Some(EquipType::Head) => Some(String::from("HEAD")),
            Some(EquipType::Neck) => Some(String::from("NECK")),
            Some(EquipType::Chest) => Some(String::from("CHEST")),
            Some(EquipType::Arm) => Some(String::from("ARM")),
            Some(EquipType::Hand) => Some(String::from("HAND")),
            Some(EquipType::Leg) => Some(String::from("LEG")),
            Some(EquipType::Foot) => Some(String::from("FOOT")),
            Some(EquipType::Back) => Some(String::from("BACK")),
            Some(EquipType::Shield) => Some(String::from("SHIELD")),
            Some(EquipType::Ring1) => Some(String::from("RING")),
            Some(EquipType::Ring2) => Some(String::from("RING")),
            Some(EquipType::Suit) => Some(String::from("*")),
        }
    }

    #[wasm_bindgen]
    pub fn set_button(&mut self, button_idx: usize, button: Button) {
        self.game.set_button(button_idx, button.0);
    }

    #[wasm_bindgen]
    pub fn untoggle_menu_button(&mut self) {
        self.game.untoggle_menu_button();
    }

    #[wasm_bindgen]
    pub fn quests(&self) -> Vec<JsValue> {
        return self
            .game
            .quests()
            .iter()
            .map(|q| JsValue::from(q))
            .collect();
    }

    #[wasm_bindgen]
    pub fn set_spellbook_spell(&mut self, spell_id: u16) {
        self.game.set_spellbook_spell(spell_id);
    }

    #[wasm_bindgen]
    pub fn playing(&mut self) -> bool {
        self.game.loaded()
    }
}

pub struct HtmlDialog {
    dialog: AldonDialog,
    transaction: RefCell<Vec<Rc<Body>>>,
    transaction_type: Cell<Option<TransactionType>>,
}

impl HtmlDialog {
    fn new(dialog: AldonDialog) -> Self {
        Self {
            dialog,
            transaction: RefCell::new(Vec::new()),
            transaction_type: Cell::new(None),
        }
    }
}

impl Dialog for HtmlDialog {
    fn tell_message(&self, title: &str, portrait_id: u16, msg_id: u16, from_actor: u16) {
        let msg = &WORLD.messages[&msg_id.to_string()];
        let frame = &SPRITES.frames[&portrait_id.to_string()].frame;
        self.dialog.tell(
            title,
            frame.x,
            frame.y,
            frame.w,
            frame.h,
            &msg.value,
            msg.choice_a.as_deref(),
            msg.choice_b.as_deref(),
            msg.choice_c.as_deref(),
            from_actor,
        );
    }

    fn execute_trade(&self, kind: TransactionType, body: Rc<Body>, new_transaction: Vec<Rc<Body>>) {
        let mut transaction = self.transaction.borrow_mut();
        *transaction = new_transaction;
        self.transaction_type.set(Some(kind));

        let items = transaction
            .iter()
            .enumerate()
            .map(|(i, item)| TransactionItem::new(i, item.clone(), None /* equiped */))
            .collect();

        self.dialog.inventory(
            BodyWrapper(body),
            items,
            vec!["Drop".to_string(), "Equip".to_string(), "Stats".to_string()],
        );
    }

    fn pickup(&self, body: Rc<Body>, new_transaction: Vec<Rc<Body>>) {
        let mut transaction = self.transaction.borrow_mut();
        *transaction = new_transaction;
        let items = transaction
            .iter()
            .enumerate()
            .map(|(i, item)| TransactionItem::new(i, item.clone(), None /* equiped */))
            .collect();

        self.dialog.pickup(BodyWrapper(body), items);
    }

    fn buy_sell(&self, body: Rc<Body>, items: Vec<Rc<Body>>, kind: TransactionType) {
        js::log("buysell");

        let mut transaction = self.transaction.borrow_mut();
        *transaction = items;

        let kind_str = match kind {
            TransactionType::Buy => "buy",
            TransactionType::Sell => "sell",

            // TODO: make panic not possible
            _ => panic!("Unsupported transaction type in buy_sell"),
        };
        let items = transaction
            .iter()
            .enumerate()
            .map(|(i, item)| TransactionItem::new(i, item.clone(), None /* equiped */))
            .collect();

        self.dialog.buySell(BodyWrapper(body), items, kind_str);
    }

    fn stats(&self, stats: &stats::PlayerStats) {
        self.dialog.stats(PlayerStats(stats.clone()));
    }

    fn pick_button(&self, button_idx: usize, buttons: Vec<buttons::Button>) {
        let js_buttons = buttons.iter().map(|b| Button(*b)).collect();
        self.dialog.pickButton(button_idx, js_buttons);
    }

    fn spellbook(&self, spells: &[u16]) {
        let js_spells = spells.iter().map(|s| Spell(*s)).collect();
        self.dialog.spellbook(js_spells);
    }

    fn get_transaction(&self) -> Vec<Rc<Body>> {
        self.transaction.borrow().clone()
    }

    fn remove_item(&self, index: usize) -> Rc<Body> {
        let mut transaction = self.transaction.borrow_mut();
        transaction.remove(index)
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);

    #[wasm_bindgen(js_namespace = window)]
    fn js_panic(msg: String);

    type Error;

    #[wasm_bindgen(constructor)]
    fn new() -> Error;

    #[wasm_bindgen(structural, method, getter)]
    fn stack(error: &Error) -> String;
}

/// Log debug information if we panic
fn panic_hook(info: &panic::PanicInfo) {
    let mut msg = info.to_string();

    msg.push_str("\n\nStack:\n\n");
    let e = Error::new();
    let stack = e.stack();
    msg.push_str(&stack);
    msg.push_str("\n\n");
    error(msg.clone());
    js_panic(msg.clone());

    // TODO: pretty hacky
    let mut logs = js::DEBUG_LOGS.lock().unwrap();
    logs.push(msg);
}

#[wasm_bindgen]
pub struct TransactionItem {
    index: usize,
    body: Rc<Body>,
}

#[wasm_bindgen]
impl TransactionItem {
    fn new(index: usize, body: Rc<Body>, _equiped: Option<EquipType>) -> Self {
        Self { index, body }
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> usize {
        self.index
    }

    #[wasm_bindgen(getter)]
    pub fn frame(&self) -> RectResWrapped {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        let frame_id = prop.frame();
        let rect = &SPRITES.frames[&frame_id.to_string()].frame;
        RectResWrapped(*rect)
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        if self.body.groupable() {
            format!("{} ({})", self.body.name, self.body.quantity())
        } else {
            self.body.name.to_string()
        }
    }

    #[wasm_bindgen(getter)]
    pub fn info(&self) -> String {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        if matches!(
            prop.kind,
            PropTypeRes::Weapon { .. } | PropTypeRes::Armor { .. }
        ) {
            return prop.info_str();
        }
        self.name()
    }

    #[wasm_bindgen(getter)]
    pub fn restriction(&self) -> String {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        prop.restriction_str()
    }

    #[wasm_bindgen(getter)]
    pub fn buy_cost(&self) -> i32 {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        prop.buy_cost()
    }

    #[wasm_bindgen(getter)]
    pub fn sell_cost(&self) -> i32 {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        prop.sell_cost()
    }

    #[wasm_bindgen(getter)]
    pub fn usable(&self) -> bool {
        let prop = &PROPS[&self.body.prop_id.to_string()];
        matches!(prop.kind, PropTypeRes::Usable { .. })
    }

    #[wasm_bindgen(getter)]
    pub fn quantity(&self) -> u8 {
        self.body.quantity()
    }
}

#[wasm_bindgen]
pub fn aldon_debug_logs() -> String {
    game::get_logs()
}

#[wasm_bindgen]
pub struct RectResWrapped(RectRes);

#[wasm_bindgen]
impl RectResWrapped {
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> f64 {
        self.0.x
    }

    #[wasm_bindgen(getter)]
    pub fn y(&self) -> f64 {
        self.0.y
    }

    #[wasm_bindgen(getter)]
    pub fn w(&self) -> f64 {
        self.0.w
    }

    #[wasm_bindgen(getter)]
    pub fn h(&self) -> f64 {
        self.0.h
    }
}

#[wasm_bindgen]
pub struct PlayerStats(stats::PlayerStats);

#[wasm_bindgen]
impl PlayerStats {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.0.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn klass(&self) -> String {
        self.0.class.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn race(&self) -> String {
        self.0.race.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn level(&self) -> i32 {
        self.0.level
    }

    #[wasm_bindgen(getter)]
    pub fn hp(&self) -> i32 {
        self.0.hp
    }

    #[wasm_bindgen(getter)]
    pub fn hp_max(&self) -> i32 {
        self.0.hp_max
    }

    #[wasm_bindgen(getter)]
    pub fn ac(&self) -> i32 {
        self.0.ac
    }

    #[wasm_bindgen(getter)]
    pub fn exp(&self) -> i32 {
        self.0.exp
    }

    #[wasm_bindgen(getter)]
    pub fn mp(&self) -> i32 {
        self.0.mp
    }

    #[wasm_bindgen(getter)]
    pub fn mp_max(&self) -> i32 {
        self.0.mp_max
    }

    #[wasm_bindgen(getter)]
    pub fn gp(&self) -> i32 {
        self.0.gp
    }

    #[wasm_bindgen(getter)]
    pub fn str(&self) -> i32 {
        self.0.str
    }

    #[wasm_bindgen(getter)]
    pub fn dex(&self) -> i32 {
        self.0.dex
    }

    #[wasm_bindgen(getter)]
    pub fn vit(&self) -> i32 {
        self.0.vit
    }

    #[wasm_bindgen(getter)]
    pub fn int(&self) -> i32 {
        self.0.int
    }

    #[wasm_bindgen(getter)]
    pub fn wis(&self) -> i32 {
        self.0.wis
    }

    #[wasm_bindgen(getter)]
    pub fn luck(&self) -> i32 {
        self.0.luck
    }

    #[wasm_bindgen(getter)]
    pub fn portrait(&self) -> u16 {
        self.0.portrait
    }
}

#[wasm_bindgen]
pub struct Button(buttons::Button);

#[wasm_bindgen]
impl Button {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        match self.0 {
            buttons::Button::Picker => "Visual: Picker".to_string(),
            buttons::Button::Empty => "Clear Button".to_string(),
            buttons::Button::Inventory => "Visual: Inventory".to_string(),
            buttons::Button::Melee => "Combat: Melee".to_string(),
            buttons::Button::PickUp => "Visual: Pickup Item".to_string(),
            buttons::Button::Ranged => "Combat: Ranged".to_string(),
            buttons::Button::Stats => "Visual: Stats".to_string(),
            buttons::Button::Item { prop_id, quantity } => {
                let prop = &PROPS[&prop_id.to_string()];
                format!("Item: {} ({})", prop.name, quantity)
            }
            buttons::Button::Sneak => "Sneaky Movement".to_string(),
            buttons::Button::Hide => "Hide in Shadows".to_string(),
            buttons::Button::Spellbook { .. } => "Spell Book".to_string(),
            buttons::Button::Spell { spell_id, .. } => {
                let spell = &SPELLS[&spell_id.to_string()];
                spell.name.clone()
            }
        }
    }

    // TODO: this shouldn't need to exist
    #[wasm_bindgen(getter)]
    pub fn frame(&self) -> RectResWrapped {
        let frame_id = match self.0 {
            buttons::Button::Picker => 2010,
            buttons::Button::Inventory => 2006,
            buttons::Button::Melee => 2002,
            buttons::Button::PickUp => 2004,
            buttons::Button::Ranged => 2003,
            buttons::Button::Stats => 2008,
            buttons::Button::Item { prop_id, .. } => {
                let prop = &PROPS[&prop_id.to_string()];
                prop.frame()
            }
            buttons::Button::Empty => 2000,
            buttons::Button::Sneak => 2118,
            buttons::Button::Hide => 2116,
            buttons::Button::Spellbook { .. } => 2100,
            buttons::Button::Spell { spell_id, .. } => {
                let spell = &SPELLS[&spell_id.to_string()];
                spell.frames[0]
            }
        };
        let rect = &SPRITES.frames[&frame_id.to_string()].frame;
        RectResWrapped(*rect)
    }
}

#[wasm_bindgen]
pub struct Spell(u16);

#[wasm_bindgen]
impl Spell {
    fn spell(&self) -> &SpellRes {
        let spell_id = self.0;
        &SPELLS[&spell_id.to_string()]
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.spell().name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn level(&self) -> i32 {
        self.spell().level
    }

    #[wasm_bindgen(getter)]
    pub fn cost(&self) -> i32 {
        self.spell().cost
    }

    #[wasm_bindgen(getter)]
    pub fn frame(&self) -> RectResWrapped {
        let frame_id = self.spell().frames[0];
        let rect = &SPRITES.frames[&frame_id.to_string()].frame;
        RectResWrapped(*rect)
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u16 {
        self.0
    }
}

impl Into<&'static str> for ClassType {
    fn into(self) -> &'static str {
        match self {
            ClassType::FIGHTER => "Fighter",
            ClassType::SPELLCASTER => "Spellcaster",
            ClassType::PRIEST => "Priest",
            ClassType::THIEF => "Thief",
            ClassType::JOURNEYMAN => "Journeyman",
            _ => "Unknown",
        }
    }
}

impl Into<String> for ClassType {
    fn into(self) -> String {
        let result: &'static str = self.into();
        result.to_string()
    }
}

#[wasm_bindgen]
pub struct Stats {}

#[wasm_bindgen]
impl Stats {
    #[wasm_bindgen]
    pub fn strength_to_hit_bonus(strength: i32) -> i32 {
        stats::strength_to_hit_bonus(strength)
    }

    #[wasm_bindgen]
    pub fn strength_to_damage(strength: i32) -> i32 {
        stats::strength_to_damage(strength)
    }

    #[wasm_bindgen]
    pub fn dexterity_to_hit_bonus(dexterity: i32) -> i32 {
        stats::dexterity_to_hit_bonus(dexterity)
    }

    #[wasm_bindgen]
    pub fn dexterity_to_armor_class(dexterity: i32) -> i32 {
        stats::dexterity_to_armor_class(dexterity)
    }

    #[wasm_bindgen]
    pub fn vitality_to_hit_points(vitality: i32) -> i32 {
        stats::vitality_to_hit_points(vitality)
    }

    #[wasm_bindgen]
    pub fn intelligence_to_chance_cast(intelligence: i32) -> i32 {
        stats::intelligence_to_chance_cast(intelligence)
    }

    #[wasm_bindgen]
    pub fn wisdom_to_mana(wisdom: i32) -> i32 {
        stats::wisdom_to_mana(wisdom)
    }

    #[wasm_bindgen]
    pub fn luck_to_modifier(intelligence: i32) -> i32 {
        stats::luck_to_modifier(intelligence)
    }

    #[wasm_bindgen]
    pub fn max_level(exp: i32) -> i32 {
        stats::max_level(exp)
    }
}

#[wasm_bindgen]
pub struct BodyWrapper(Rc<Body>);

/// Returns a canvas to draw on. Use real canvases for everyting instead of offscreen canvases
/// for better support
fn new_canvas(width: u32, height: u32) -> Result<HtmlCanvasElement, JsValue> {
    let document = web_sys::window()
        .ok_or("expected window")?
        .document()
        .ok_or("expected document")?;

    let canvas = document
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()?;

    canvas.set_width(width);
    canvas.set_height(height);
    canvas.style().set_property("display", "none")?;

    // Append the canvas to the document body
    document
        .body()
        .ok_or("document should have a body")?
        .append_child(&canvas)?;

    Ok(canvas)
}
