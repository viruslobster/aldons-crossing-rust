//! Here lies all the drawing code
use crate::{
    body::{ActionState, Body},
    buttons::Button,
    combat::{BattleEvent, BattleEventType, MissileType},
    data::*,
    game::CONSOLE,
    js,
    stage::Stage,
    thrift::save::{ClassType, RaceType},
    AldonHtmlCanvasGame,
};
use wasm_bindgen::prelude::*;

// Maps a tile id to a frame id. For some reason in the original game
// these numbers are almost the same but not quite.
const FRAME_BY_TILE: [u16; 14] = [0, 1, 2, 3, 4, 7, 6, 5, 10, 11, 12, 13, 14, 15];

// Tile id's that are a wall or space
const WALL_OR_SPACE: [u8; 5] = [0, 1, 2, 13, 14];

// All frame ids for walls in the game
#[rustfmt::skip]
const WALL_FRAME_IDS: [u16; 40] = [
     0, 18, 19, 20, 21, 22, 23, 24, 25, 50,
     1, 26, 27, 28, 29, 30, 31, 32, 33, 51,
     2, 34, 35, 36, 37, 38, 39, 40, 41, 52,
    14, 42, 43, 44, 45, 46, 47, 48, 49, 53,
];
const PROP_DOOR: [u16; 14] = [
    144, 145, 162, 163, 193, 194, 406, 407, 408, 409, 410, 411, 161, 309,
];
const PROP_WINDOW: [u16; 7] = [143, 146, 147, 148, 164, 165, 166];
const PROP_VERTICAL: [u16; 10] = [161, 162, 163, 164, 165, 166, 408, 410, 370, 406];

#[wasm_bindgen]
impl AldonHtmlCanvasGame {
    fn render_tiles(&mut self, animation_idx: u16) {
        let stage = &self.game.stage;
        let ctx = self.tiles_ctx();
        let render_new_map = self.rendered_map != Some(stage.map_id());

        if !render_new_map && self.last_animation_idx == animation_idx {
            return;
        }
        for y in 0..24 {
            for x in 0..24 {
                let tile_id = stage.tile_at(x, y).unwrap() as usize;

                // If we're not rendering a new map, water is the only thing that changes
                if tile_id != 5 && !render_new_map {
                    continue;
                }
                let frame_id = match tile_id {
                    // Render walls so they look "3D"
                    0 | 1 | 2 | 14 => wall_frame_id(&stage, x, y),

                    // Animation water
                    5 => FRAME_BY_TILE[5] + animation_idx,

                    // Draw the tile beneath a door or window
                    13 => resolve_door_or_window(&stage, x, y),
                    _ => FRAME_BY_TILE[tile_id],
                };
                self.draw(&ctx, frame_id, (x as f64) * 16.0, (y as f64) * 16.0);
            }
        }
        self.last_animation_idx = animation_idx;
        self.rendered_map = Some(stage.map_id());
    }

    fn render_fog(&mut self) {
        let current_fog = self.game.fog.current();
        if self.last_fog == current_fog {
            return;
        }
        let fog_ctx = self.fog_ctx();
        fog_ctx.clear_rect(
            0.0,
            0.0,
            self.fog_canvas.width().into(),
            self.fog_canvas.height().into(),
        );
        for y in 0..24 {
            for x in 0..24 {
                let x = x as f64;
                let y = y as f64;
                if self.game.fog.occluded(x, y) {
                    self.draw(&fog_ctx, 186, x * 16.0, y * 16.0);
                }
            }
        }
        self.last_fog = current_fog;
    }

    #[wasm_bindgen]
    pub fn render(&mut self, now_js: js_sys::BigInt) {
        let now = now_js.as_f64().unwrap();
        let animation_idx = ((now / 200.0).floor() % 3.0) as u16;
        let (viewport_width, viewport_height) = self.viewport_size();
        let ctx = self.ctx();
        let stage_ctx = self.stage_ctx();
        // TODO: big speed up, only draw if animation frame has changed or something has moved

        if !self.game.loaded() {
            let scale = viewport_width.min(viewport_height) / SPRITES.frames["2200"].frame.w;
            self.draw_scale(&ctx, 2200, 0.0, 0.0, scale);
            return;
        }

        if self.game.game_over() && self.drawn_once.get() {
            return;
        }
        ctx.clear_rect(
            0.0,
            0.0,
            self.canvas.width().into(),
            self.canvas.height().into(),
        );
        self.render_tiles(animation_idx);
        stage_ctx
            .draw_image_with_html_canvas_element(&self.tile_canvas, 0.0, 0.0)
            .unwrap();

        let traps = self.game.stage.traps();
        for trap in traps {
            let trap_frame_id = 7700;
            self.draw(
                &stage_ctx,
                trap_frame_id + animation_idx,
                (trap.x as f64) * 16.0,
                (trap.y as f64) * 16.0,
            );
        }
        let mut bodies = self.game.stage.bodies();

        bodies.sort_by(|b0, b1| b0.health.cmp(&b1.health));
        bodies.sort_by(|b0, b1| {
            let p0 = &PROPS[&b0.prop_id.to_string()];
            let p1 = &PROPS[&b1.prop_id.to_string()];
            p1.draw_depth.cmp(&p0.draw_depth)
        });

        for body in bodies {
            if body.is_player() {
                // Player is drawn directly to the screen, see below
                continue;
            }
            let frame_id = body.frame(now);
            if frame_id == 0 {
                continue;
            }
            if !body.hidden() {
                self.draw(&stage_ctx, frame_id, body.x() * 16.0, body.y() * 16.0);

                for frame_id in body.battle_event_frames(now) {
                    self.draw(&stage_ctx, frame_id, body.x() * 16.0, body.y() * 16.0);
                }
            }
        }
        for missile in self.game.stage.missiles() {
            let frame_id = missile.kind.frame_id(animation_idx);
            self.draw(&stage_ctx, frame_id, missile.x * 16.0, missile.y * 16.0);
        }
        self.render_fog();
        stage_ctx
            .draw_image_with_html_canvas_element(&self.fog_canvas, 0.0, 0.0)
            .unwrap();

        // Offset the stage so the player is always in the center
        let (mut offset_x, mut offset_y) = self.stage_offset();
        let player = self.game.stage.get_player();
        if matches!(player.action_state(), ActionState::Idle) {
            // scaling combined with a fractional offset can lead to distorted
            // images. This solves that problem, but the camera does visibly
            // jump a little. Maybe I should just scale the images up natively
            // instead.
            offset_x = round(offset_x);
            offset_y = round(offset_y);
        }
        if player.moving_from() == player.moving_to() {}

        ctx.draw_image_with_html_canvas_element(&self.stage_canvas, -offset_x, -offset_y)
            .unwrap();

        // The player is drawn directly to the screen instead of to
        // stage_canvas because otherwise there is a slight inconsistency
        // between where the player's position and the camera such that the
        // player sometimes appears slightly blurry when both the camera
        // and the player are moving
        if !player.hidden() {
            self.draw(
                &ctx,
                player.frame(now),
                player.x() * 16.0 - offset_x,
                player.y() * 16.0 - offset_y,
            );
            for frame_id in player.battle_event_frames(now) {
                self.draw(
                    &ctx,
                    frame_id,
                    player.x() * 16.0 - offset_x,
                    player.y() * 16.0 - offset_y,
                );
            }
        }

        self.draw_sidebar(now);
        self.drawn_once.set(true);
    }

    fn draw_sidebar(&self, _now: f64) {
        // full game is (384 + 46) x (384 + 46)
        let (viewport_width, viewport_height) = self.viewport_size();
        let (stage_width, stage_height) = self.stage_size();
        let ctx = self.ctx();

        // tile the console background
        let bg_sprite_width: f64 = 157.0; //160.0;
        let bg_sprite_height: f64 = 12.0;
        let n_width = (viewport_width / bg_sprite_width).ceil() as i32;
        let n_height = 1 + ((viewport_height - stage_height) / bg_sprite_height.ceil()) as i32;
        for i in 0..n_height {
            for j in 0..n_width {
                self.draw(
                    &ctx,
                    2207,
                    (j as f64) * bg_sprite_width,
                    (i as f64) * bg_sprite_height + stage_height,
                );
            }
        }

        // tile the buttons background
        let n_height = (viewport_height / bg_sprite_height).ceil() as i32;
        for i in 0..n_height {
            self.draw(&ctx, 2207, stage_width, (i as f64) * bg_sprite_height);
        }

        // side bar
        self.draw(&ctx, 2201, stage_width, 0.0);

        let stage = &self.game.stage;
        let player = stage.get_player();

        // health stat
        self.draw(&ctx, 2202, stage_width + 4.0, 3.0);
        let ctx = self.ctx();
        ctx.set_font("16px PalmOS");
        ctx.set_fill_style(&JsValue::from("#ff0000"));
        ctx.fill_text(&player.health.get().to_string(), stage_width + 16.0, 12.0)
            .unwrap();

        // mp stat
        self.draw(&ctx, 2203, stage_width + 4.0, 14.0);
        ctx.set_fill_style(&JsValue::from("#0099cc"));
        ctx.fill_text(&player.magic().to_string(), stage_width + 16.0, 23.0)
            .unwrap();

        let (stage_width, _stage_height) = self.stage_size();
        // TODO: this is duplicated in lib.rs
        for (i, button) in self.game.buttons.iter().enumerate() {
            let frame_id = match button {
                Button::Picker => Some(2010),
                Button::Inventory => Some(2006),
                Button::Melee => Some(2002),
                Button::PickUp => Some(2004),
                Button::Ranged => Some(2003),
                Button::Stats => Some(2008),
                Button::Item { prop_id, .. } => {
                    let prop = &PROPS[&prop_id.to_string()];
                    Some(prop.frame())
                }
                Button::Empty => None,
                Button::Sneak => Some(2118),
                Button::Hide => Some(2116),
                Button::Spellbook { .. } => Some(2100),
                Button::Spell { spell_id, .. } => {
                    let spell = &SPELLS[&spell_id.to_string()];
                    Some(spell.frames[0])
                }
            };
            let toggled = if self.game.buttons.idx_toggled(i) {
                1
            } else {
                0
            };
            if let Some(id) = frame_id {
                let (x, y) = self.game.buttons.position(i);

                if matches!(
                    button,
                    Button::Sneak | Button::Hide | Button::Spellbook { .. } | Button::Spell { .. }
                ) {
                    // Most button images come with the background baked in,
                    // but not these.
                    let button_frame = 2000;
                    self.draw(
                        &ctx,
                        button_frame + toggled,
                        stage_width + x * 16.0,
                        y * 16.0,
                    );
                }
                self.draw(&ctx, id + toggled, stage_width + x * 16.0, y * 16.0);
            }
        }

        self.draw(
            &ctx,
            2210, // button tab
            stage_width + 38.0,
            28.0 + 27.0 * self.game.buttons.tab() as f64,
        );

        // console
        // You can use ctx.measure(text).(font|actual)_bounding_box_(ascent|decent)
        // to meausre text but it isn't actually consistent across platforms.
        // I get better results just with 11.0
        ctx.set_font("18px PalmOS");
        ctx.set_fill_style(&JsValue::from("white"));
        let text_height = 11.0;
        let console = CONSOLE.lock().unwrap();
        let mut y = viewport_height - 2.0;
        for line in console.iter() {
            if y - text_height < stage_height {
                break;
            }
            let color = text_color(line);
            ctx.set_fill_style(&JsValue::from(color));
            ctx.fill_text(line, 0.0, y).unwrap();
            y -= text_height;
        }
    }

    fn draw(&self, ctx: &web_sys::CanvasRenderingContext2d, frame_id: u16, x: f64, y: f64) {
        self.draw_scale(ctx, frame_id, x, y, 1.0);
    }

    fn draw_scale(
        &self,
        ctx: &web_sys::CanvasRenderingContext2d,
        frame_id: u16,
        x: f64,
        y: f64,
        scale: f64,
    ) {
        let frame = &SPRITES.frames[&frame_id.to_string()].frame;
        ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &self.spritesheet,
            frame.x,
            frame.y,
            frame.w,
            frame.h,
            x,
            y,
            frame.w * scale,
            frame.h * scale,
        )
        .unwrap();
    }

    /// Returns (width, height)
    pub(crate) fn viewport_size(&self) -> (f64, f64) {
        let width = f64::min((self.canvas.width() as f64) / self.scale, 430.0);
        let height = f64::min((self.canvas.height() as f64) / self.scale, 430.0);
        (width, height)
    }

    /// Returns (width, height)
    pub(crate) fn stage_size(&self) -> (f64, f64) {
        let (viewport_width, viewport_height) = self.viewport_size();

        let stage_width = f64::min(viewport_width - 46.0, 430.0);
        let stage_height = f64::min(viewport_height - 46.0, 430.0);
        (stage_width, stage_height)
    }

    pub fn ctx(&self) -> web_sys::CanvasRenderingContext2d {
        new_ctx(
            &self.canvas,
            false,      // alpha
            self.scale, // scale
        )
    }

    pub fn stage_ctx(&self) -> web_sys::CanvasRenderingContext2d {
        new_ctx(
            &self.stage_canvas,
            false, // alpha
            1.0,   // scale
        )
    }

    pub fn tiles_ctx(&self) -> web_sys::CanvasRenderingContext2d {
        new_ctx(
            &self.tile_canvas,
            false, // alpha
            1.0,   // scale
        )
    }

    pub fn fog_ctx(&self) -> web_sys::CanvasRenderingContext2d {
        new_ctx(
            &self.fog_canvas,
            true, // alpha
            1.0,  // scale
        )
    }
}

fn player_prop_id(male: bool, race: RaceType, class: ClassType) -> u16 {
    match (male, race, class) {
        (true, RaceType::HUMAN, ClassType::JOURNEYMAN) => 55,
        (true, RaceType::HUMAN, ClassType::FIGHTER) => 54,
        (true, RaceType::HUMAN, ClassType::THIEF) => 88,
        (true, RaceType::HUMAN, ClassType::PRIEST) => 56,
        (true, RaceType::HUMAN, ClassType::SPELLCASTER) => 45,
        (true, RaceType::ELF, ClassType::JOURNEYMAN) => 52,
        (true, RaceType::ELF, ClassType::FIGHTER) => 51,
        (true, RaceType::ELF, ClassType::THIEF) => 53,
        (true, RaceType::ELF, ClassType::SPELLCASTER) => 50,
        (true, RaceType::DWARF, ClassType::JOURNEYMAN) => 49,
        (true, RaceType::DWARF, ClassType::FIGHTER) => 46,
        (true, RaceType::DWARF, ClassType::THIEF) => 48,
        (true, RaceType::DWARF, ClassType::PRIEST) => 47,
        (false, RaceType::HUMAN, ClassType::JOURNEYMAN) => 41,
        (false, RaceType::HUMAN, ClassType::FIGHTER) => 40,
        (false, RaceType::HUMAN, ClassType::THIEF) => 43,
        (false, RaceType::HUMAN, ClassType::PRIEST) => 42,
        (false, RaceType::HUMAN, ClassType::SPELLCASTER) => 31,
        (false, RaceType::ELF, ClassType::JOURNEYMAN) => 38,
        (false, RaceType::ELF, ClassType::FIGHTER) => 37,
        (false, RaceType::ELF, ClassType::THIEF) => 39,
        (false, RaceType::ELF, ClassType::SPELLCASTER) => 36,
        (false, RaceType::DWARF, ClassType::JOURNEYMAN) => 35,
        (false, RaceType::DWARF, ClassType::FIGHTER) => 32,
        (false, RaceType::DWARF, ClassType::THIEF) => 34,
        (false, RaceType::DWARF, ClassType::PRIEST) => 33,
        _ => panic!("invalid player, race='{:?}', class='{:?}'", race, class),
    }
}

impl MissileType {
    fn frame_id(&self, animation_idx: u16) -> u16 {
        match self {
            MissileType::Rock => 1650,
            MissileType::Magic => 1670 + animation_idx,
            MissileType::Fire => 1630 + animation_idx,
            MissileType::Bonfire => 1640 + animation_idx,
            MissileType::Ice => 1660,
            MissileType::Poison => 1690 + animation_idx, // TODO
        }
    }
}

fn wall_frame_id(stage: &Stage, x: i64, y: i64) -> u16 {
    let up = WALL_OR_SPACE.contains(&stage.tile_at(x, y - 1).unwrap_or(u8::MAX));
    let down = WALL_OR_SPACE.contains(&stage.tile_at(x, y + 1).unwrap_or(u8::MAX));
    let left = WALL_OR_SPACE.contains(&stage.tile_at(x - 1, y).unwrap_or(u8::MAX));
    let right = WALL_OR_SPACE.contains(&stage.tile_at(x + 1, y).unwrap_or(u8::MAX));
    let diag = WALL_OR_SPACE.contains(&stage.tile_at(x + 1, y + 1).unwrap_or(u8::MAX));

    let tile_id = stage.tile_at(x, y).unwrap() as usize;
    let offset: usize = match tile_id {
        0 | 1 | 2 => tile_id * 10,
        14 => 3 * 10,
        _ => panic!("none wall frame passed to wall_frame_id"),
    };
    if right && down && diag {
        WALL_FRAME_IDS[offset] // block
    } else if down && right {
        WALL_FRAME_IDS[offset + 2] // top left
    } else if left && right {
        WALL_FRAME_IDS[offset + 4] // horizontal
    } else if up && right {
        WALL_FRAME_IDS[offset + 5] // bottom left
    } else if down && up {
        WALL_FRAME_IDS[offset + 3] // vertical
    } else if left && up {
        WALL_FRAME_IDS[offset + 1] // bottom right
    } else if down && !up {
        WALL_FRAME_IDS[offset + 6] // top right
    } else if left && !right {
        WALL_FRAME_IDS[offset + 8] // horizontal cap
    } else if up && !down {
        WALL_FRAME_IDS[offset + 7] // vertical cap
    } else if right && !left {
        WALL_FRAME_IDS[offset + 4] // horizontal
    } else {
        WALL_FRAME_IDS[offset + 9] // spike
    }
}

fn is_wall(maybe_tile_id: Option<u8>) -> bool {
    let Some(tile_id) = maybe_tile_id else {
        return false;
    };
    WALL_OR_SPACE.contains(&tile_id)
}

/// Maps have a special "door or window" tile. The idea is that the
/// person making the map does not have to worry about drawing the right
/// tile underneath a door or window because the engine will look at the
/// surrounding tiles and do the right thing.
fn resolve_door_or_window(stage: &Stage, x: i64, y: i64) -> u16 {
    let maybe_prop_id = stage.prop_at(x, y);
    if let None = maybe_prop_id {
        // If there no door or window prop at (x, y) just pick a tile
        // perpendicular to the direction of the wall
        let north = stage.tile_at(x, y - 1);
        let south = stage.tile_at(x, y + 1);
        let east = stage.tile_at(x + 1, y);
        let west = stage.tile_at(x - 1, y);

        let tile_id = if is_wall(north) && is_wall(south) {
            stage
                .tile_at(x - 1, y)
                .or_else(|| stage.tile_at(x + 1, y))
                .unwrap_or(9)
        } else if is_wall(east) && is_wall(west) {
            stage
                .tile_at(x, y - 1)
                .or_else(|| stage.tile_at(x, y + 1))
                .unwrap_or(9)
        } else {
            9
        };
        return FRAME_BY_TILE[tile_id as usize];
    }
    let prop_id = maybe_prop_id.unwrap();
    let window = is_window(prop_id);
    let door = is_door(prop_id);
    let vertical = is_vertical(prop_id);

    let tile_id = if window && vertical {
        stage.tile_at(x - 1, y).unwrap_or(9)
    } else if window && !vertical {
        stage.tile_at(x, y - 1).unwrap_or(9)
    } else if door && vertical {
        stage
            .tile_at(x + 1, y)
            .unwrap_or(stage.tile_at(x - 1, y).unwrap_or(9))
    } else if door && !vertical {
        stage
            .tile_at(x, y + 1)
            .unwrap_or(stage.tile_at(x, y - 1).unwrap_or(9))
    } else {
        js::log(&format!("id {} is not a door or window", prop_id));
        0
    };
    FRAME_BY_TILE[tile_id as usize]
}

/// Player, Enemy, NPC props can be in a several different
/// states that have different animations associated with them.
/// All the frames for these animations are stores in prop.frames
/// At different offsets
impl ActionState {
    fn frame_offset(&self) -> usize {
        match self {
            ActionState::Attack => 6,
            ActionState::Idle => 0,
            ActionState::Dying => 8,
            ActionState::Dead => 9,
            ActionState::Walk => 3,
        }
    }

    fn frame_count(&self) -> usize {
        match self {
            ActionState::Attack => 2,
            ActionState::Idle => 3,
            ActionState::Dying => 1,
            ActionState::Dead => 1,
            ActionState::Walk => 3,
        }
    }
}

impl BattleEvent {
    fn frame_id(&self) -> Option<u16> {
        match self.kind {
            BattleEventType::Hit => Some(1600),
            BattleEventType::Miss => Some(1610),
            BattleEventType::Crit => Some(1620),
            BattleEventType::Fizzle => None,
            BattleEventType::Condition1 => Some(1670),
            BattleEventType::Condition2 => Some(1680),
        }
    }
}

fn is_door(id: u16) -> bool {
    return PROP_DOOR.contains(&id);
}

fn is_window(id: u16) -> bool {
    return PROP_WINDOW.contains(&id);
}

fn is_vertical(id: u16) -> bool {
    return PROP_VERTICAL.contains(&id);
}

fn text_color(txt: &str) -> &str {
    match txt.chars().next() {
        Some('(') => "#FFFF00",
        Some('*') => "#33FF00",
        Some('-') => "#FF3366",
        _ => "white",
    }
}

impl Body {
    /// The "frame" for a body is the sprite actually drawn
    fn frame(&self, now: f64) -> u16 {
        let prop_id = if self.is_player() {
            player_prop_id(self.male.get(), self.race.get().unwrap(), self.class.get())
        } else {
            self.prop_id
        };
        let prop = &PROPS[&prop_id.to_string()];
        let offset = self.action_state().frame_offset();
        let len = self.action_state().frame_count() as f64;
        let animation_idx = ((now / 400.0).floor() % len) as usize;

        match &prop.kind {
            PropTypeRes::Item { frame }
            | PropTypeRes::Usable { frame, .. }
            | PropTypeRes::Armor { frame, .. }
            | PropTypeRes::Weapon { frame, .. }
            | PropTypeRes::Physical { frame } => *frame,

            PropTypeRes::User { frames, .. }
            | PropTypeRes::Creature { frames, .. }
            | PropTypeRes::Animprop { frames, .. } => frames[offset + animation_idx],
        }
    }

    fn battle_event_frames(&self, now: f64) -> Vec<u16> {
        let mut result = Vec::new();

        for battle_event in self.battle_events.borrow().iter() {
            let frame_id = battle_event.frame_id();
            if frame_id.is_none() {
                continue;
            }
            let animation_idx = (((now - battle_event.time) / 200.0).floor() % 3.0) as u16;
            result.push(frame_id.unwrap() + animation_idx);
        }
        result
    }
}

fn round(x: f64) -> f64 {
    let whole = x.floor();
    let decimal = x - whole;
    let decimal = (decimal * 5.0).round() / 5.0;
    whole + decimal
}

fn new_ctx(
    canvas: &web_sys::HtmlCanvasElement,
    alpha: bool,
    scale: f64,
) -> web_sys::CanvasRenderingContext2d {
    let mut options = web_sys::ContextAttributes2d::new();
    options.alpha(alpha);

    let ctx = canvas
        .get_context_with_context_options("2d", &options)
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    ctx.set_image_smoothing_enabled(false);
    ctx.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0).unwrap();
    ctx
}
