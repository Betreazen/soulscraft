// The altar on the client (06-altar.md): the summon item floating over the pedestal and the ghost
// of the next ring as small turning copies (block entity drawing, `native_block_entity_submit`),
// the ghost's outlines, the event's spawn points and the beams of drains and curses (world
// primitives, `native_world_primitives`), and the altar's menu (a mod screen,
// `native_mod_screen_open`, drawn in `native_screen_render_post` as Path-of-craft-tree's tree
// window is). Everything drawn comes from the server's last message (soulscraft_rules::view); the
// client sends back only the menu's buttons.
use crate::game::Game;
use crate::game::native_block_entity_draw::BlockEntityDraw;
use crate::game::native_hooks::NativeScreenView;
use horizon_assets::item_model::DisplayContext;
use horizon_client::containers::ModBlockEntity;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::world_primitives::WorldPrimitives;
use horizon_world::nbt::{JavaString, Tag};
use soulscraft_rules::{
    altar::{self as ra, AltarFile},
    view::{self, BeamKind, Button, Menu, Press, View},
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The mod screen's id (`native_mod_screen_open`).
const SCREEN: &str = "soulscraft";
const FULL_LIGHT: u32 = 0xF0F0;
/// A view older than this is not drawn (the server sends one every 10 ticks near an altar).
const STALE: i64 = 40;
/// «Показать призрак» shows the ghost this long.
const GHOST_SHOW: i64 = 600;
const LINE: f32 = 11.0;
const PANEL_W: f32 = 300.0;
const BUTTON_H: f32 = 18.0;
const LEFT: i32 = 0;

#[derive(Default)]
struct AltarClient {
    views: HashMap<[i32; 3], (View, i64)>,
    menu: Option<Menu>,
    open_due: bool,
    was_open: bool,
    presses: Vec<Press>,
    ghost_until: i64,
    file: Option<AltarFile>,
}

static CLIENT: LazyLock<Mutex<AltarClient>> = LazyLock::new(Mutex::default);

fn client() -> MutexGuard<'static, AltarClient> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now(game: &Game<'_>) -> i64 {
    game.world.game_time
}

/// `native_net_from_server` on the altar's channels.
pub(super) fn on_net(game: &Game<'_>, channel: &str, data: &[u8]) {
    match channel {
        view::ALTAR => {
            if let Some(v) = view::decode::<View>(data) {
                client().views.insert(v.core, (v, now(game)));
            }
        }
        view::MENU => {
            if let Some(m) = view::decode::<Menu>(data) {
                let mut c = client();
                let fresh = c.menu.as_ref().is_none_or(|old| old.core != m.core) || !c.was_open;
                c.menu = Some(m);
                if fresh && !c.was_open {
                    c.open_due = true;
                }
            }
        }
        _ => {}
    }
}

/// The window's tick (`native_tick`): opens the menu the server sent, tells it when the menu
/// closed, and sends the buttons pressed.
pub(super) fn on_tick(game: &mut Game<'_>) {
    if game.remote.is_none() {
        return;
    }
    let open = game.native_mod_screen() == Some(SCREEN);
    let (due, was, core, presses) = {
        let mut c = client();
        let due = std::mem::take(&mut c.open_due);
        let was = c.was_open;
        c.was_open = open || due;
        (due, was, c.menu.as_ref().map(|m| m.core), std::mem::take(&mut c.presses))
    };
    if due && !open && game.native_playing() {
        game.native_mod_screen_open(SCREEN);
    }
    let mut presses = presses;
    if was && !open && !due && let Some(core) = core {
        presses.push(Press { core, button: Button::Closed });
        client().menu = None;
    }
    for p in presses {
        if let Err(e) = game.native_net_send_to_server(view::PRESS, &view::encode(&p)) {
            log::warn!("soulscraft: menu press not sent: {e}");
        }
    }
}

fn is_ours(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(SCREEN)
}

/// The panel and its three buttons, in GUI pixels: (panel, [boost on, boost off, ghost]).
fn layout(screen: &NativeScreenView<'_>, lines: usize) -> ([f32; 4], [[f32; 4]; 3]) {
    let [x, y, w, h] = screen.rect;
    let ph = 12.0 + lines as f32 * LINE + 8.0 + BUTTON_H + 10.0;
    let px = x + (w - PANEL_W) / 2.0;
    let py = y + ((h - ph) / 2.0).max(4.0);
    let by = py + ph - BUTTON_H - 8.0;
    let bw = (PANEL_W - 4.0 * 8.0) / 3.0;
    let b = |i: f32| [px + 8.0 + i * (bw + 8.0), by, bw, BUTTON_H];
    ([px, py, PANEL_W, ph], [b(0.0), b(1.0), b(2.0)])
}

/// The menu's lines wrapped to the panel's text width (a long line, the spawn boost's, breaks).
fn wrapped(game: &Game<'_>, menu: &view::Menu) -> Vec<String> {
    view::wrap(&view::menu_lines(menu), PANEL_W - 16.0, |s| game.native_text_width(s))
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] <= r[0] + r[2] && at[1] >= r[1] && at[1] <= r[1] + r[3]
}

/// `native_screen_render_post`: the menu as text lines and three buttons.
pub(super) fn draw_screen(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !is_ours(game, screen) {
        return;
    }
    let Some(menu) = client().menu.clone() else { return };
    let lines = wrapped(game, &menu);
    let (panel, buttons) = layout(screen, lines.len() + 1);
    game.native_fill(batch, panel, 0xE01A_1714);
    game.native_fill(batch, [panel[0], panel[1], panel[2], 1.0], 0xFF9C_F5E4);
    game.native_text(batch, "Алтарь призыва", [panel[0] + 8.0, panel[1] + 6.0], 0x9CF5E4, true);
    for (i, l) in lines.iter().enumerate() {
        let color = if l.starts_with("Алтарю нужно") { 0xFF6060 } else { 0xE8E0D0 };
        game.native_text(batch, l, [panel[0] + 8.0, panel[1] + 6.0 + LINE * (i as f32 + 1.0)], color, true);
    }
    let labels = [("Учащ. спавн: вкл", menu.boost), ("Учащ. спавн: выкл", !menu.boost), ("Показать призрак", false)];
    for (b, (label, active)) in buttons.iter().zip(labels) {
        let hover = inside(*b, screen.mouse);
        let fill = if active { 0xFF3C_6E5A } else if hover { 0xFF5A_5048 } else { 0xFF3A_3430 };
        game.native_fill(batch, *b, fill);
        let tw = game.native_text_width(label);
        game.native_text(batch, label, [b[0] + (b[2] - tw) / 2.0, b[1] + 5.0], 0xFFFFFF, true);
    }
}

/// `native_screen_mouse_button_pressed_pre`: a button of the menu.
pub(super) fn on_screen_press(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !is_ours(game, screen) || button != LEFT {
        return;
    }
    let mut c = client();
    let Some(menu) = c.menu.clone() else { return };
    let (_, buttons) = layout(screen, wrapped(game, &menu).len() + 1);
    let Some(i) = buttons.iter().position(|b| inside(*b, screen.mouse)) else { return };
    *canceled = true;
    match i {
        0 => c.presses.push(Press { core: menu.core, button: Button::BoostOn }),
        1 => c.presses.push(Press { core: menu.core, button: Button::BoostOff }),
        _ => c.ghost_until = now(game) + GHOST_SHOW,
    }
}

/// The ghost shows while an altar block is in hand, or for a while after «Показать призрак».
fn ghost_shown(game: &Game<'_>, c: &AltarClient) -> bool {
    if c.ghost_until > now(game) {
        return true;
    }
    game.native_held().is_some_and(|(item, _, _)| item.starts_with("soulscraft:altar_") || item.starts_with(ra::BANNER_PREFIX))
}

fn nbt_item(entity: &ModBlockEntity) -> Option<String> {
    match entity.nbt.get(&JavaString::from("item")) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// `native_block_entity_submit`: the pedestal's item, and the core's ghost copies.
pub(super) fn draw_block_entity(game: &Game<'_>, pos: [i32; 3], block: &str, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    if block == ra::PEDESTAL {
        draw_pedestal_item(game, pos, entity, draw);
    } else if block == ra::CORE {
        draw_ghost_copies(game, pos, draw);
    }
}

/// The summon item over the pedestal's bowl: its centre `height_px` over the pedestal's bottom,
/// at `scale`, turning `spin_deg_per_tick` and bobbing `bob_px` over `bob_ticks` (data/altar.json).
fn draw_pedestal_item(game: &Game<'_>, pos: [i32; 3], entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(item) = nbt_item(entity) else { return };
    let p = {
        let mut c = client();
        c.file.get_or_insert_with(AltarFile::embedded).pedestal_item
    };
    let t = draw.game_time as f64 + f64::from(draw.partial);
    let phase = f64::from(pos[0] * 7 + pos[2] * 13);
    let bob = p.bob_px * (std::f64::consts::TAU * (t + phase) / p.bob_ticks.max(1.0)).sin();
    let angle = (30.0 + p.spin_deg_per_tick * (t + phase)) % 360.0;
    draw.push();
    draw.translate([0.5, ((p.height_px + bob) / 16.0) as f32, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], angle as f32, [0.0; 3]);
    draw.scale([p.scale as f32; 3]);
    let light = draw.light;
    draw.light = FULL_LIGHT;
    game.native_draw_item(draw, &ItemStack::new(&item, 1), DisplayContext::Fixed, None, pos[0] ^ pos[2]);
    draw.light = light;
    draw.pop();
}

/// The ghost's half-size turning copies of the blocks the next ring needs, drawn by the core's
/// block entity relative to the core.
fn draw_ghost_copies(game: &Game<'_>, core: [i32; 3], draw: &mut BlockEntityDraw) {
    let cells = {
        let c = client();
        if !ghost_shown(game, &c) {
            return;
        }
        match c.views.get(&core) {
            Some((v, at)) if now(game) - at <= STALE => v.ghost.clone(),
            _ => return,
        }
    };
    let t = draw.game_time as f32 + draw.partial;
    let light = draw.light;
    draw.light = FULL_LIGHT;
    for g in cells {
        if g.block == ra::SOCKET {
            continue;
        }
        let rel = [(g.pos[0] - core[0]) as f32, (g.pos[1] - core[1]) as f32, (g.pos[2] - core[2]) as f32];
        draw.push();
        draw.translate([rel[0] + 0.25, rel[1] + 0.25, rel[2] + 0.25]);
        draw.rotate_about([0.0, 1.0, 0.0], (t * 2.0) % 360.0, [0.25, 0.0, 0.25]);
        draw.scale([0.5; 3]);
        let mut props: Vec<(&str, &str)> = Vec::new();
        if let Some(p) = g.part.as_deref() {
            props.push(("part", p));
        }
        game.native_draw_block(draw, &g.block, &props);
        draw.pop();
    }
    draw.light = light;
}

/// `native_block_entity_render_bounding_box`: the core's ghost reaches four blocks round it.
pub(super) fn bounds(block: &str, bounds: &mut [f32; 6]) {
    if block == ra::CORE {
        *bounds = [-4.0, -1.0, -4.0, 5.0, 4.0, 5.0];
    }
}

/// Each frame: the ghost's outlines, the coming wave's spawn points and the drain and curse beams.
pub(super) fn primitives(game: &Game<'_>, p: &mut WorldPrimitives) {
    let c = client();
    let t = now(game);
    let ghost = ghost_shown(game, &c);
    for (core, (v, at)) in &c.views {
        if t - at > STALE {
            continue;
        }
        let top = [f64::from(core[0]) + 0.5, f64::from(core[1]) + 1.6, f64::from(core[2]) + 0.5];
        if ghost {
            for g in &v.ghost {
                let color = if g.block == ra::SOCKET { 0xFFFF_A040 } else { 0xFF66_E0FF };
                let lo = [f64::from(g.pos[0]) + 0.03, f64::from(g.pos[1]) + 0.03, f64::from(g.pos[2]) + 0.03];
                let h = if g.block == ra::SLAB || g.block == ra::SOCKET { 0.5 } else { 1.0 };
                p.box_outline(lo, [lo[0] + 0.94, lo[1] + h - 0.06, lo[2] + 0.94], color, 2.0);
            }
            for w in &v.wrong {
                let lo = [f64::from(w[0]), f64::from(w[1]), f64::from(w[2])];
                p.box_outline(lo, [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0], 0xFFFF_3030, 3.0);
            }
        }
        let Some(e) = &v.event else { continue };
        for s in &e.spawns {
            let base = [f64::from(s[0]) + 0.5, f64::from(s[1]), f64::from(s[2]) + 0.5];
            p.beam(base, 24, 0xFFFF_5030);
            p.box_outline([base[0] - 0.5, base[1], base[2] - 0.5], [base[0] + 0.5, base[1] + 2.0, base[2] + 0.5], 0xFFFF_5030, 3.0)
                .through_walls();
        }
        for b in &e.beams {
            match b.kind {
                BeamKind::Windup => {
                    p.line(b.from, top, 0x90FF_FFFF, 1.5);
                }
                BeamKind::Drain => {
                    p.line(b.from, top, 0xFF9C_F5E4, 4.0);
                }
                BeamKind::Curse => {
                    p.line(b.from, top, 0xFFB0_40FF, 3.0).through_walls();
                }
            }
        }
        let color = if e.losing && (t / 5) % 2 == 0 { 0xFFFF_4040 } else { 0xFFE8_E0D0 };
        p.text([top[0], top[1] + 1.4, top[2]], format!("Алтарь {:.0}/{:.0}", e.hp.ceil(), e.max), color, 1.0)
            .background()
            .scale_with_distance();
    }
}
