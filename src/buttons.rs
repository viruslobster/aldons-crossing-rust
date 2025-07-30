//! The interface on the right hand side of the screen allowing the player to cast spell, switch
//! weapons, use potions, etc.
use crate::{
    body::Body,
    data::{PropTypeRes, PROPS},
    game::{Dialog, InvalidDataError},
    js,
    stage::Stage,
    thrift::save::{self, ClassType},
};
use serde_derive::{Deserialize, Serialize};
use std::rc::Rc;

const BUTTON_WIDTH: f64 = 1.0;
const BUTTON_HEIGHT: f64 = 1.0;

/// The three tabs each with 10 buttons on the right hand side of the screen
pub(crate) struct Buttons {
    pub origin_x: f64,
    pub origin_y: f64,

    stage: Rc<Stage>,
    buttons: [[Button; 10]; 3],
    tab: usize,
    dialog: Rc<dyn Dialog>,
    touch_down: bool,
}

impl Buttons {
    pub fn new(stage: Rc<Stage>, dialog: Rc<dyn Dialog>) -> Self {
        Self {
            stage,
            dialog,
            buttons: default_buttons(),
            tab: 0,
            origin_x: 4.0 / 16.0,
            origin_y: 28.0 / 16.0,
            touch_down: false,
        }
    }

    pub fn from_save(
        save: &save::Buttons,
        stage: Rc<Stage>,
        dialog: Rc<dyn Dialog>,
    ) -> Result<Self, InvalidDataError> {
        let mut buttons = default_buttons();

        let player = stage.get_player();
        if let Some(save_buttons) = &save.buttons {
            for (tab_idx, tab) in save_buttons.iter().enumerate() {
                for (button_idx, save) in tab.iter().enumerate() {
                    buttons[tab_idx][button_idx] = Button::from_save(save, player.clone());
                }
            }
        }

        let tab: usize = save
            .tab
            .unwrap_or(1)
            .try_into()
            .map_err(|err| InvalidDataError::new(&format!("tab field: {}", err)))?;

        let result = Self {
            stage,
            dialog,
            buttons,
            tab,
            origin_x: 4.0 / 16.0,
            origin_y: 28.0 / 16.0,
            touch_down: false,
        };
        Ok(result)
    }

    pub fn save(&self) -> save::Buttons {
        let buttons: Vec<Vec<save::Button>> = self
            .buttons
            .iter()
            .map(|tab| tab.iter().map(|&button| button.into()).collect())
            .collect();

        save::Buttons::new(buttons, self.tab as i32)
    }

    pub fn update(&mut self) {
        let buttons = &mut self.buttons[self.tab];
        for button in buttons.iter_mut() {
            match button {
                Button::Sneak { toggled: true } => {
                    if !self.stage.get_player().sneaking() {
                        *button = Button::Sneak { toggled: false };
                    }
                }
                Button::Hide { toggled: true } => {
                    js::log("hidden down");
                    if !self.stage.get_player().hidden() {
                        js::log("unhide button");
                        *button = Button::Hide { toggled: false };
                    }
                }
                _ => {}
            }
        }
    }

    pub fn toggled(&self, kind: ButtonKind) -> bool {
        for button in self.iter() {
            if button.matches(kind) {
                return button.toggled();
            }
        }
        return false;
    }

    pub fn untoggle(&mut self, kind: ButtonKind) {
        self.untoggle_many(vec![kind]);
    }

    pub fn untoggle_many(&mut self, kinds: Vec<ButtonKind>) {
        let player = self.stage.get_player();
        let buttons = &mut self.buttons[self.tab];
        for button in buttons.iter_mut() {
            for kind in kinds.iter() {
                if button.matches(*kind) {
                    button.untoggle(player.clone());
                }
            }
        }
    }

    pub fn tab(&self) -> usize {
        self.tab
    }

    pub fn idx_toggled(&self, idx: usize) -> bool {
        self.buttons[self.tab][idx].toggled()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Button> + '_ {
        self.buttons[self.tab].iter()
    }

    /// The (x, y) of the top left corner of button idx
    pub fn position(&self, idx: usize) -> (f64, f64) {
        let column = idx % 2;
        let row = idx / 2;
        let spacer_x = column as f64 / 16.0;
        let spacer_y = row as f64 / 16.0;
        let button_x = (idx % 2) as f64 * BUTTON_WIDTH + spacer_x;
        let button_y = (idx / 2) as f64 * BUTTON_HEIGHT + spacer_y;
        (self.origin_x + button_x, self.origin_y + button_y)
    }

    pub fn input(&mut self, x: f64, y: f64, touch_up: bool) {
        if touch_up {
            self.touch_down = false;
            return;
        }
        if self.touch_down {
            return;
        }
        if rect_contains(2.4375, 1.8125, 0.3125, 1.5625, x, y) && self.tab != 0 {
            js::log("tab1");
            self.untoggle_all();
            self.tab = 0;
            return;
        }
        if rect_contains(2.4375, 3.5, 0.3125, 1.5625, x, y) && self.tab != 1 {
            js::log("tab2");
            self.untoggle_all();
            self.tab = 1;
            return;
        }
        if rect_contains(2.4375, 5.1875, 0.3125, 1.5625, x, y) && self.tab != 2 {
            js::log("tab3");
            self.untoggle_all();
            self.tab = 2;
            return;
        }
        for i in 0..self.buttons[self.tab].len() {
            let (button_x, button_y) = self.position(i);

            if rect_contains(button_x, button_y, BUTTON_WIDTH, BUTTON_HEIGHT, x, y) {
                self.touch_down = true;
                self.click(i);
                break;
            }
        }
    }

    pub fn set_button(&mut self, button_idx: usize, button: Button) {
        self.buttons[self.tab][button_idx] = button;
        self.untoggle_all();
    }

    pub fn set_tab_button(&mut self, tab: usize, button_idx: usize, button: Button) {
        self.buttons[tab][button_idx] = button;
    }

    fn untoggle_all(&mut self) {
        let player = self.stage.get_player();
        let buttons = &mut self.buttons[self.tab];
        for button in buttons.iter_mut() {
            button.untoggle(player.clone());
        }
    }

    /// The list of buttons a player can assign to the side bar
    fn assignable_buttons(&self) -> Vec<Button> {
        let toggled = false;
        let mut buttons = vec![
            Button::Empty,
            Button::Inventory { toggled },
            Button::Melee,
            Button::PickUp { toggled },
            Button::Stats { toggled },
        ];
        let player = self.stage.get_player();

        for item in player.inventory.borrow().iter() {
            let prop = &PROPS[&item.prop_id.to_string()];
            if matches!(prop.kind, PropTypeRes::Usable { .. }) {
                let button = Button::Item {
                    prop_id: item.prop_id,
                    quantity: item.quantity(),
                };
                buttons.push(button);
            }
        }
        if player.class() == ClassType::THIEF {
            buttons.push(Button::Sneak { toggled });
            buttons.push(Button::Hide { toggled });
        }
        if matches!(player.class(), ClassType::SPELLCASTER | ClassType::PRIEST) {
            buttons.push(Button::Spellbook {
                spell_id: None,
                toggled,
            });
            let spells: Vec<Button> = player
                .spells()
                .iter()
                .map(|spell_id| Button::Spell {
                    spell_id: *spell_id,
                    toggled,
                })
                .collect();

            buttons.extend(spells);
        }
        buttons
    }

    fn click(&mut self, idx: usize) {
        if self.toggled(ButtonKind::Picker) {
            let button = &mut self.buttons[self.tab][idx];
            if button.matches(ButtonKind::Picker) {
                button.untoggle(self.stage.get_player());
                return;
            }
            let buttons = self.assignable_buttons();
            self.dialog.pick_button(idx, buttons);
            self.touch_down = false;
            return;
        }
        let now = self.stage.now();
        let mut to_untoggle = vec![];
        match &mut self.buttons[self.tab][idx] {
            Button::Picker { toggled } => {
                to_untoggle.extend(vec![
                    ButtonKind::Inventory,
                    ButtonKind::PickUp,
                    ButtonKind::Stats,
                    ButtonKind::Spell,
                ]);
                *toggled = !*toggled;
            }

            Button::Inventory { toggled } => {
                to_untoggle.extend(vec![
                    ButtonKind::Picker,
                    ButtonKind::PickUp,
                    ButtonKind::Stats,
                    ButtonKind::Spell,
                ]);
                *toggled = !*toggled;
            }
            Button::PickUp { toggled } => {
                to_untoggle.extend(vec![
                    ButtonKind::Picker,
                    ButtonKind::Inventory,
                    ButtonKind::Stats,
                    ButtonKind::Spell,
                    // Important, otherwise the thief can sneak into areas and get all the loot for free
                    ButtonKind::Sneak,
                    ButtonKind::Hide,
                ]);
                *toggled = !*toggled;
            }

            Button::Stats { toggled } => {
                to_untoggle.extend(vec![
                    ButtonKind::Picker,
                    ButtonKind::Inventory,
                    ButtonKind::PickUp,
                    ButtonKind::Spell,
                ]);
                *toggled = !*toggled;
            }

            Button::Spell { toggled, .. } => {
                to_untoggle.extend(vec![
                    ButtonKind::Picker,
                    ButtonKind::Inventory,
                    ButtonKind::PickUp,
                    ButtonKind::Stats,
                ]);
                *toggled = !*toggled;
            }

            Button::Sneak { toggled } => {
                let player = self.stage.get_player();

                if *toggled {
                    player.reveal();
                } else {
                    player.sneak(now);
                }
                *toggled = !*toggled;
            }
            Button::Hide { toggled } => {
                let player = self.stage.get_player();
                if *toggled {
                    player.reveal();
                } else {
                    player.hide();
                }
                *toggled = !*toggled;
            }
            btn @ Button::Melee => {
                *btn = {
                    let player = self.stage.get_player();
                    player.set_prefer_melee(false);
                    Button::Ranged
                }
            }
            btn @ Button::Ranged => {
                *btn = {
                    let player = self.stage.get_player();
                    player.set_prefer_melee(true);
                    Button::Melee
                }
            }
            Button::Item { prop_id, .. } => {
                let player = self.stage.get_player();
                self.stage.use_item(&player, *prop_id);
                if !player.has_item(*prop_id) {
                    self.buttons[self.tab][idx] = Button::Empty;
                }
            }
            Button::Empty => self.untoggle_all(),
            Button::Spellbook { spell_id: None, .. } => {
                let spells = self.stage.get_player().spells();
                self.dialog.spellbook(&spells);
            }
            btn @ Button::Spellbook {
                spell_id: Some(_), ..
            } => {
                *btn = Button::Spellbook {
                    spell_id: None,
                    toggled: true,
                };
            }
        }
        self.untoggle_many(to_untoggle);
    }

    /// Activates the first spellbook button and get ready to cast a spell
    pub fn set_spellbook_spell(&mut self, spell_id: u16) {
        for button in self.buttons[self.tab].iter_mut() {
            if matches!(button, Button::Spellbook { .. }) {
                *button = Button::Spellbook {
                    spell_id: Some(spell_id),
                    toggled: true,
                };
                break;
            }
        }
    }

    /// Returns the spell the player has selected to cast, if it exists
    pub fn active_spell(&self) -> Option<u16> {
        for button in self.iter() {
            match button {
                Button::Spellbook {
                    spell_id: Some(spell_id),
                    ..
                } => return Some(*spell_id),

                Button::Spell { spell_id, .. } => return Some(*spell_id),
                _ => {}
            }
        }
        return None;
    }

    /// Some spells like fire strike should remain active once cast so
    /// its easy for the player to recast it, but other spells like summon
    /// monster should be untoggled automatically after cast
    pub fn maybe_untoggle_spell(&mut self, spell_id: u16) {
        match spell_id {
            8 | 9 | 10 | 11 | 12 | 13 | 15 | 16 | 17 | 44 | 43 | 42 | 30 | 35 | 36 | 37 | 38
            | 39 => self.untoggle(ButtonKind::Spell),

            _ => {}
        }
    }

    pub fn clear_class_specific(&mut self) {
        for tab in &mut self.buttons {
            for button in tab {
                match button {
                    Button::Sneak { .. }
                    | Button::Hide { .. }
                    | Button::Spellbook { .. }
                    | Button::Spell { .. } => *button = Button::Empty,

                    _ => {}
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ButtonKind {
    Picker,
    Empty,
    Inventory,
    Melee,
    PickUp,
    Ranged,
    Stats,
    Item,
    Sneak,
    Hide,
    Spellbook,
    Spell,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Button {
    Picker {
        toggled: bool,
    },
    Empty,
    Inventory {
        toggled: bool,
    },
    Melee,
    PickUp {
        toggled: bool,
    },
    Ranged,
    Stats {
        toggled: bool,
    },
    Item {
        prop_id: u16,
        // only used to display name in button picker
        quantity: u8,
    },
    Sneak {
        toggled: bool,
    },
    Hide {
        toggled: bool,
    },
    Spellbook {
        toggled: bool,
        spell_id: Option<u16>,
    },
    Spell {
        toggled: bool,
        spell_id: u16,
    },
}

impl Button {
    fn toggled(self) -> bool {
        match self {
            Button::Picker { toggled }
            | Button::Inventory { toggled }
            | Button::PickUp { toggled }
            | Button::Stats { toggled }
            | Button::Sneak { toggled }
            | Button::Hide { toggled }
            | Button::Spellbook { toggled, .. }
            | Button::Spell { toggled, .. } => toggled,

            _ => false,
        }
    }

    fn untoggle(&mut self, player: Rc<Body>) {
        match self {
            Button::Picker { toggled }
            | Button::Inventory { toggled }
            | Button::PickUp { toggled }
            | Button::Stats { toggled }
            | Button::Spellbook { toggled, .. }
            | Button::Spell { toggled, .. } => *toggled = false,

            Button::Sneak { toggled } | Button::Hide { toggled } => {
                player.reveal();
                *toggled = false;
            }

            _ => {}
        }
    }

    fn matches(self, kind: ButtonKind) -> bool {
        match (self, kind) {
            (Button::Picker { .. }, ButtonKind::Picker) => true,
            (Button::Empty { .. }, ButtonKind::Empty) => true,
            (Button::Inventory { .. }, ButtonKind::Inventory) => true,
            (Button::Melee { .. }, ButtonKind::Melee) => true,
            (Button::PickUp { .. }, ButtonKind::PickUp) => true,
            (Button::Ranged { .. }, ButtonKind::Ranged) => true,
            (Button::Stats { .. }, ButtonKind::Stats) => true,
            (Button::Item { .. }, ButtonKind::Item) => true,
            (Button::Sneak { .. }, ButtonKind::Sneak) => true,
            (Button::Hide { .. }, ButtonKind::Hide) => true,
            (Button::Spellbook { .. }, ButtonKind::Spellbook) => true,
            (Button::Spell { .. }, ButtonKind::Spell) => true,
            _ => false,
        }
    }

    fn from_save(button: &save::Button, player: Rc<Body>) -> Button {
        let toggled = false;
        match button {
            save::Button::Picker(_) => Button::Picker { toggled },
            save::Button::Empty(_) => Button::Empty,
            save::Button::Inventory(_) => Button::Inventory { toggled },
            save::Button::Melee(_) => Button::Melee,
            save::Button::Pickup(_) => Button::PickUp { toggled },
            save::Button::Ranged(_) => Button::Ranged,
            save::Button::Stats(_) => Button::Stats { toggled },
            save::Button::Item(item) => {
                let Some(prop_id) = item.prop_id else {
                    js::log("warning: item button without prop_id field");
                    return Button::Empty;
                };
                let Some(quantity) = item.quantity else {
                    js::log("warning: item button without quantity field");
                    return Button::Empty;
                };
                Button::Item {
                    prop_id: prop_id as u16,
                    quantity: quantity as u8,
                }
            }
            save::Button::Sneak(_) => Button::Sneak {
                toggled: player.sneaking(),
            },
            save::Button::Hide(_) => Button::Hide {
                toggled: player.hidden(),
            },
            save::Button::Spellbook(_) => Button::Spellbook {
                spell_id: None,
                toggled,
            },
            save::Button::Spell(button) => {
                let Some(spell_id) = button.spell_id else {
                    js::log("warning: spell button without spell_id field");
                    return Button::Empty;
                };
                let Ok(spell_id) = spell_id.try_into() else {
                    js::log(&format!(
                        "warning: spell button invalid spell_id field: {}",
                        spell_id
                    ));
                    return Button::Empty;
                };
                Button::Spell { spell_id, toggled }
            }
        }
    }
}

impl Into<save::Button> for Button {
    fn into(self) -> save::Button {
        match self {
            Button::Picker { .. } => save::Button::Picker(save::ButtonPicker {}),
            Button::Empty { .. } => save::Button::Empty(save::ButtonEmpty {}),
            Button::Inventory { .. } => save::Button::Inventory(save::ButtonInventory {}),
            Button::Melee { .. } => save::Button::Melee(save::ButtonMelee {}),
            Button::PickUp { .. } => save::Button::Pickup(save::ButtonPickUp {}),
            Button::Ranged { .. } => save::Button::Ranged(save::ButtonRanged {}),
            Button::Stats { .. } => save::Button::Stats(save::ButtonStats {}),
            Button::Item { prop_id, quantity } => {
                let item = save::ButtonItem::new(prop_id as i32, quantity as i32);
                save::Button::Item(item)
            }
            Button::Sneak { .. } => save::Button::Sneak(save::ButtonSneak {}),
            Button::Hide { .. } => save::Button::Hide(save::ButtonHide {}),
            Button::Spellbook { .. } => save::Button::Spellbook(save::ButtonSpellbook {}),
            Button::Spell { spell_id, .. } => {
                save::Button::Spell(save::ButtonSpell::new(spell_id as i32))
            }
        }
    }
}

fn rect_contains(left: f64, top: f64, width: f64, height: f64, x: f64, y: f64) -> bool {
    (x >= left) && (x <= (left + width)) && (y >= top) && (y <= (top + height))
}

fn default_buttons() -> [[Button; 10]; 3] {
    let toggled = false;
    let buttons0 = [
        Button::Stats { toggled },
        Button::Inventory { toggled },
        Button::PickUp { toggled },
        Button::Melee,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Picker { toggled },
    ];
    let buttons1 = [
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Picker { toggled },
    ];
    let buttons2 = [
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Empty,
        Button::Picker { toggled },
    ];
    [buttons0, buttons1, buttons2]
}
