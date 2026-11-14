// The hunter's book on the client (08-guide-book.md): the window a right click on the book opens
// (a mod screen of its own, `native_mod_screen_open("soulscraft_book")`, drawn in
// `native_screen_render_post` as the altar's menu is), with the sections «Враги», «Алтарь»,
// «Ивенты» made from the player's progress the server last sent (soulscraft_rules::book_page).
//
// The enemy's model (path А): the mob's rig with its clips, drawn in the window by
// `native_draw_scene` (game/native_scene.rs:78-221, API scenes_in_screens). The scene is one
// invisible block (`minecraft:barrier`, a model with no faces) carrying a block entity of the
// book's own (`soulscraft:book_figure`, made here, never placed in a world); the scene calls
// `native_block_entity_submit` for it (native_scene.rs:155-166) with a `BlockEntityDraw` at the
// game time and the frame's partial tick, and the hook draws the rig there
// (`soulscraft_client::draw_rig_with`) in the clip the page plays, black for a locked entry.
// Under the model, each attack's phase strip (data/mobs.json) with a cursor that runs with the clip.
use crate::game::Game;
use crate::game::native_block_entity_draw::BlockEntityDraw;
use crate::game::native_hooks::NativeScreenView;
use crate::game::soulscraft_client as client;
use horizon_client::containers::{ModBlockEntity, ModBlockEntityFields};
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::{NativeSceneBlock, NativeSceneView};
use serde_json::Value;
use soulscraft_rules::{
    altar::AltarFile,
    book::{self, BookFile, Catalog, Progress},
    book_page::{self, Page, Seg, Tone},
    view,
};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The window's id (`native_mod_screen_open`): not the altar menu's `soulscraft`.
const SCREEN: &str = "soulscraft_book";
/// The figure's block entity id (only ever in the window's scene).
const FIGURE: &str = "soulscraft:book_figure";
const LEFT: i32 = 0;
const LINE: f32 = 10.0;
const ROW: f32 = 11.0;
const TAB_H: f32 = 14.0;
const LIST_W: f32 = 104.0;
const MODEL: f32 = 104.0;
const PAD: f32 = 6.0;
/// An attack clip plays, then the figure rests this long before it plays again.
const REST_TICKS: f32 = 14.0;
const TABS: [&str; 3] = ["Враги", "Алтарь", "Ивенты"];
const MOVES: [&str; 6] = ["walk", "fly", "swim", "hop", "glide", "drift"];

/// What the figure in the scene shows this frame.
#[derive(Clone, Debug, Default)]
struct Figure {
    mob: String,
    clip: String,
    /// The game tick the clip started.
    start: i64,
    /// Its ticks and the rest after (0: hold the clip as it loops itself).
    period: f32,
    silhouette: bool,
}

struct Data {
    book: BookFile,
    catalog: Catalog,
    altar: AltarFile,
    trophies: Value,
}

#[derive(Default)]
struct BookClient {
    progress: Option<Progress>,
    open_due: bool,
    tab: usize,
    sel: usize,
    clip: usize,
    scroll: usize,
    list_off: usize,
    started: i64,
    figure: Option<Figure>,
    logged: bool,
}

static DATA: LazyLock<Data> = LazyLock::new(|| Data {
    book: BookFile::embedded(),
    catalog: Catalog::embedded(),
    altar: AltarFile::embedded(),
    trophies: serde_json::from_str(soulscraft_rules::event::TROPHIES).unwrap_or(Value::Null),
});
static CLIENT: LazyLock<Mutex<BookClient>> = LazyLock::new(Mutex::default);
static ENTITY: LazyLock<ModBlockEntity> = LazyLock::new(|| {
    ModBlockEntity::new(ModBlockEntityFields { id: FIGURE.into(), ..ModBlockEntityFields::default() })
});

/// The figure the scene draws now (apart from the window's state, locked while the scene draws).
static SHOWN: Mutex<Option<Figure>> = Mutex::new(None);

fn shown() -> MutexGuard<'static, Option<Figure>> {
    SHOWN.lock().unwrap_or_else(PoisonError::into_inner)
}

fn client_state() -> MutexGuard<'static, BookClient> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_server`: the player's progress, and «open the book».
pub(super) fn on_net(game: &Game<'_>, channel: &str, data: &[u8]) {
    if channel != book::CHANNEL {
        return;
    }
    let Some(msg) = book::decode(data) else {
        log::warn!("soulscraft: a book message that does not read");
        return;
    };
    let mut c = client_state();
    let entries = msg.progress.mobs.len();
    c.progress = Some(msg.progress);
    if msg.open {
        c.open_due = true;
        c.started = game.world.game_time;
        c.clip = usize::MAX;
    }
    log::info!("soulscraft: book message open={} entries={entries}", msg.open);
}

/// The window's tick: opens the book the server said to.
pub(super) fn on_tick(game: &mut Game<'_>) {
    if game.remote.is_none() || !std::mem::take(&mut client_state().open_due) {
        return;
    }
    if game.native_mod_screen() != Some(SCREEN) && game.native_playing() {
        let opened = game.native_mod_screen_open(SCREEN);
        log::info!("soulscraft: book window opened={opened}");
    }
}

fn is_ours(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(SCREEN)
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] <= r[0] + r[2] && at[1] >= r[1] && at[1] <= r[1] + r[3]
}

/// Where everything is, in GUI pixels ([x, y, w, h]).
struct Layout {
    panel: [f32; 4],
    tabs: [[f32; 4]; 3],
    list: [f32; 4],
    model: [f32; 4],
    clips: [f32; 4],
    text: [f32; 4],
    up: [f32; 4],
    down: [f32; 4],
}

fn layout(screen: &NativeScreenView<'_>, tab: usize) -> Layout {
    let [x, y, w, h] = screen.rect;
    let pw = (w - 16.0).min(440.0);
    let ph = (h - 16.0).min(260.0);
    let (px, py) = (x + (w - pw) / 2.0, y + (h - ph) / 2.0);
    let tw = 64.0;
    let tab = |i: f32| [px + PAD + i * (tw + 4.0), py + PAD, tw, TAB_H];
    let top = py + PAD + TAB_H + PAD;
    let body_h = ph - (top - py) - PAD;
    let list = [px + PAD, top, LIST_W, body_h];
    let model = [list[0] + LIST_W + PAD, top, MODEL, MODEL];
    let clips = [model[0], top + MODEL + 4.0, MODEL, body_h - MODEL - 4.0];
    let text = if tab == 0 {
        let x0 = model[0] + MODEL + PAD;
        [x0, top, px + pw - PAD - x0, body_h - 14.0]
    } else {
        [px + PAD, top, pw - 2.0 * PAD, body_h - 14.0]
    };
    let arrows_y = top + body_h - 12.0;
    Layout {
        panel: [px, py, pw, ph],
        tabs: [tab(0.0), tab(1.0), tab(2.0)],
        list,
        model,
        clips,
        text,
        up: [px + pw - PAD - 34.0, arrows_y, 16.0, 12.0],
        down: [px + pw - PAD - 16.0, arrows_y, 16.0, 12.0],
    }
}

/// The page of the section and entry chosen.
fn page_of(c: &BookClient) -> Page {
    let d = &*DATA;
    let p = c.progress.clone().unwrap_or_default();
    match c.tab {
        1 => book_page::altar_page(&d.book, &d.altar, &p),
        2 => book_page::events_page(&d.book, &d.catalog, &d.trophies, &p),
        _ => match d.catalog.entries.get(c.sel) {
            Some((e, m)) => book_page::mob_page(&d.book, &d.catalog, &d.trophies, e, m, &p),
            None => Page::default(),
        },
    }
}

/// The clips the page lets the figure play: (label, clip), the default first-picked index.
fn clips_for(game: &Game<'_>, mob: &str, page: &Page) -> (Vec<(String, String)>, usize) {
    let rig = client::clips_of(game, mob);
    let has = |n: &str| rig.iter().any(|(c, _, _)| c == n);
    let mut out = Vec::new();
    if page.silhouette || page.stage < book::MET {
        return (out, 0);
    }
    out.push(("Покой".to_owned(), if has("idle") { "idle".to_owned() } else { String::new() }));
    if page.moves
        && let Some(m) = MOVES.iter().find(|m| has(m))
    {
        out.push(("Шаг".to_owned(), (*m).to_owned()));
    }
    let first_attack = out.len();
    out.extend(page.strips.iter().map(|s| (s.name.clone(), s.clip.clone())));
    let pick = if page.strips.is_empty() { 0 } else { first_attack };
    (out, pick)
}

fn tone_color(t: Tone) -> u32 {
    match t {
        Tone::Head => 0xE8C878,
        Tone::Text => 0xE8E0D0,
        Tone::Dim => 0x8C8478,
    }
}

fn stage_color(stage: usize) -> u32 {
    [0xFF55_5048, 0xFFC8_C0B0, 0xFF6C_C070, 0xFF6C_A8E8, 0xFFE8_C878][stage.min(4)]
}

fn seg_color(s: Seg) -> u32 {
    match s {
        Seg::Windup => 0xFFFF_E040,
        Seg::Hold => 0xFFFF_9020,
        Seg::Hit => 0xFFFF_3030,
        Seg::Window => 0xFF40_E060,
    }
}

/// `native_screen_render_post`: the book.
pub(super) fn draw_screen(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !is_ours(game, screen) {
        return;
    }
    let mut c = client_state();
    let l = layout(screen, c.tab);
    game.native_fill(batch, l.panel, 0xF02A_2118);
    game.native_fill(batch, [l.panel[0], l.panel[1], l.panel[2], 1.0], 0xFFE8_C878);
    for (i, r) in l.tabs.iter().enumerate() {
        let fill = if i == c.tab { 0xFF5A_4630 } else if inside(*r, screen.mouse) { 0xFF46_3A2C } else { 0xFF34_2A20 };
        game.native_fill(batch, *r, fill);
        let tw = game.native_text_width(TABS[i]);
        game.native_text(batch, TABS[i], [r[0] + (r[2] - tw) / 2.0, r[1] + 3.0], 0xFFFFFF, true);
    }
    let page = page_of(&c);
    let scene = if c.tab == 0 {
        draw_list(game, batch, &l, &c, screen);
        draw_enemy(game, batch, &l, &mut c, &page)
    } else {
        c.figure = None;
        None
    };
    *shown() = c.figure.clone();
    draw_text(game, batch, &l, &mut c, &page);
    if !c.logged {
        c.logged = true;
        log::info!("soulscraft: book window drawn tab={} title={}", c.tab, page.title);
    }
    // The scene draws the figure through the block entity hook, which reads SHOWN: no lock of
    // this window's state may be held then.
    drop(c);
    if let Some(m) = scene {
        draw_scene(game, batch, l.model, m);
    }
}

fn list_rows(l: &Layout) -> usize {
    ((l.list[3] - 14.0) / ROW).max(1.0) as usize
}

fn draw_list(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, c: &BookClient, screen: &NativeScreenView<'_>) {
    let d = &*DATA;
    let p = c.progress.clone().unwrap_or_default();
    game.native_fill(batch, l.list, 0x6018_120C);
    let rows = list_rows(l);
    for (row, i) in (c.list_off..d.catalog.entries.len()).take(rows).enumerate() {
        let (e, _) = &d.catalog.entries[i];
        let stage = p.stage(&d.book, e);
        let r = [l.list[0], l.list[1] + row as f32 * ROW, l.list[2], ROW];
        if i == c.sel {
            game.native_fill(batch, r, 0xFF4A_3A28);
        } else if inside(r, screen.mouse) {
            game.native_fill(batch, r, 0x803A_2E22);
        }
        game.native_fill(batch, [r[0] + 3.0, r[1] + 3.0, 4.0, 4.0], stage_color(stage));
        let name = if stage == book::UNKNOWN { "???" } else { e.name.as_str() };
        game.native_text(batch, name, [r[0] + 10.0, r[1] + 1.5], if stage == book::UNKNOWN { 0x8C8478 } else { 0xE8E0D0 }, true);
    }
    if d.catalog.entries.len() > rows {
        let (u, w) = list_arrows(l);
        for (r, s) in [(u, "▲"), (w, "▼")] {
            game.native_fill(batch, r, 0xFF34_2A20);
            game.native_text(batch, s, [r[0] + 4.0, r[1] + 2.0], 0xE8E0D0, true);
        }
    }
}

fn list_arrows(l: &Layout) -> ([f32; 4], [f32; 4]) {
    let y = l.list[1] + l.list[3] - 12.0;
    ([l.list[0], y, 16.0, 12.0], [l.list[0] + 18.0, y, 16.0, 12.0])
}

/// The model in its clip, the clip buttons and the phase strip of the attack playing.
fn draw_enemy(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, c: &mut BookClient, page: &Page) -> Option<&'static Value> {
    let d: &'static Data = &DATA;
    let (e, m) = d.catalog.entries.get(c.sel)?;
    game.native_fill(batch, l.model, 0x60_0C0A08);
    let now = game.world.game_time;
    if e.vanilla {
        c.figure = None;
        let at = [l.model[0] + l.model[2] / 2.0 - 16.0, l.model[1] + l.model[3] / 2.0 - 16.0];
        let egg = format!("{}_spawn_egg", e.keys[0]);
        if page.silhouette || !game.native_item_model(batch, &egg, &egg, at, 32.0) {
            game.native_text(batch, "?", [at[0] + 14.0, at[1] + 12.0], 0x8C8478, true);
        }
        None
    } else {
        let (clips, pick) = clips_for(game, &e.id, page);
        if c.clip == usize::MAX || c.clip >= clips.len().max(1) {
            c.clip = pick;
        }
        let clip = clips.get(c.clip).map_or(String::new(), |x| x.1.clone());
        let rig = client::clips_of(game, &e.id);
        let (ticks, looped) = rig.iter().find(|x| x.0 == clip).map_or((0.0, true), |x| (x.1, x.2));
        let strip = page.strips.iter().find(|s| s.clip == clip && !clip.is_empty());
        let len = ticks.max(strip.map_or(0.0, |s| s.ticks() as f32));
        let period = if looped && strip.is_none() { 0.0 } else { len + REST_TICKS };
        c.figure = Some(Figure { mob: e.id.clone(), clip: clip.clone(), start: c.started, period, silhouette: page.silhouette });
        for (i, (label, _)) in clips.iter().enumerate() {
            let r = clip_button(l, i);
            if r[1] + r[3] > l.clips[1] + l.clips[3] - 18.0 {
                break;
            }
            game.native_fill(batch, r, if i == c.clip { 0xFF5A_4630 } else { 0xFF34_2A20 });
            game.native_text(batch, label, [r[0] + 3.0, r[1] + 1.5], 0xE8E0D0, true);
        }
        if let Some(s) = strip {
            let t = (now - c.started) as f32 + game.clock.fraction() as f32;
            let at = if period > 0.0 { t % period } else { t };
            draw_strip(game, batch, [l.clips[0], l.clips[1] + l.clips[3] - 14.0, l.clips[2], 6.0], s, at);
        }
        Some(m)
    }
}

fn clip_button(l: &Layout, i: usize) -> [f32; 4] {
    [l.clips[0], l.clips[1] + i as f32 * (ROW + 1.0), l.clips[2], ROW]
}

/// The figure: one barrier block (no faces) with the book's figure entity, seen from the front.
fn draw_scene(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], m: &Value) {
    let Some(state) = game.pack.assets.registry.default_state("minecraft:barrier") else { return };
    let hb = &m["hitbox"];
    let height = hb["height"].as_f64().unwrap_or(2.0) as f32;
    let width = hb["width"].as_f64().unwrap_or(0.6) as f32;
    let span = height.max(width * 1.6).max(0.6);
    let view = NativeSceneView { center: [0.5, height / 2.0, 0.5], yaw: 160.0, pitch: 12.0, scale: rect[3] * 0.78 / span };
    let blocks = [NativeSceneBlock { pos: [0, 0, 0], state, entity: Some(&*ENTITY) }];
    game.native_draw_scene(batch, [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]], &blocks, view);
}

/// The phase strip: windup, hold, hit, the player's window, with a cursor at `t`.
fn draw_strip(game: &Game<'_>, batch: &mut HudBatch, r: [f32; 4], s: &book_page::Strip, t: f32) {
    let total = s.ticks().max(1) as f32;
    let mut x = r[0];
    for (seg, n) in &s.segs {
        let w = r[2] * *n as f32 / total;
        game.native_fill(batch, [x, r[1], w.max(1.0), r[3]], seg_color(*seg));
        x += w;
    }
    if t < total {
        let cx = r[0] + r[2] * t / total;
        game.native_fill(batch, [cx - 0.5, r[1] - 2.0, 1.5, r[3] + 4.0], 0xFFFF_FFFF);
        if let Some(seg) = s.seg_at(t as u32) {
            game.native_text(batch, seg.label(), [r[0], r[1] + r[3] + 1.0], seg_color(seg) & 0xFF_FFFF, true);
        }
    }
}

/// The page's text: title, lines wrapped to the column, scrolled; the attack's tell under it.
fn draw_text(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, c: &mut BookClient, page: &Page) {
    let lines = text_lines(game, l, c, page);
    let rows = (l.text[3] / LINE).max(1.0) as usize;
    c.scroll = c.scroll.min(lines.len().saturating_sub(rows));
    game.native_text(batch, &page.title, [l.text[0], l.text[1]], 0xE8C878, true);
    for (i, (text, color)) in lines.iter().skip(c.scroll).take(rows.saturating_sub(1)).enumerate() {
        game.native_text(batch, text, [l.text[0], l.text[1] + LINE * (i as f32 + 1.0)], *color, true);
    }
    if lines.len() + 1 > rows {
        for (r, s) in [(l.up, "▲"), (l.down, "▼")] {
            game.native_fill(batch, r, 0xFF34_2A20);
            game.native_text(batch, s, [r[0] + 4.0, r[1] + 2.0], 0xE8E0D0, true);
        }
    }
}

fn text_lines(game: &Game<'_>, l: &Layout, c: &BookClient, page: &Page) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut lines = page.lines.clone();
    if c.tab == 0
        && let Some(f) = &c.figure
        && let Some(s) = page.strips.iter().find(|s| s.clip == f.clip)
    {
        let phases: Vec<String> = s.segs.iter().map(|(g, n)| format!("{} {n}", g.label())).collect();
        lines.insert(0, book_page::Line { text: format!("{}: {}", s.name, phases.join(", ")), tone: Tone::Head });
        if !s.tell.is_empty() {
            lines.insert(1, book_page::Line { text: format!("Как читать замах: {}", s.tell), tone: Tone::Text });
        }
    }
    for line in lines {
        for w in view::wrap(&[line.text], l.text[2], |s| game.native_text_width(s)) {
            out.push((w, tone_color(line.tone)));
        }
    }
    out
}

/// `native_screen_mouse_button_pressed_pre`: tabs, entries, clips, scrolling.
pub(super) fn on_screen_press(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !is_ours(game, screen) || button != LEFT {
        return;
    }
    *canceled = true;
    let mut c = client_state();
    let l = layout(screen, c.tab);
    let at = screen.mouse;
    let now = game.world.game_time;
    if let Some(i) = l.tabs.iter().position(|r| inside(*r, at)) {
        c.tab = i;
        c.scroll = 0;
        return;
    }
    if inside(l.up, at) {
        c.scroll = c.scroll.saturating_sub(3);
        return;
    }
    if inside(l.down, at) {
        c.scroll += 3;
        return;
    }
    if c.tab != 0 {
        return;
    }
    let n = DATA.catalog.entries.len();
    let rows = list_rows(&l);
    let (lu, ld) = list_arrows(&l);
    if n > rows && inside(lu, at) {
        c.list_off = c.list_off.saturating_sub(rows);
        return;
    }
    if n > rows && inside(ld, at) {
        c.list_off = (c.list_off + rows).min(n.saturating_sub(1));
        return;
    }
    if inside(l.list, at) {
        let i = c.list_off + ((at[1] - l.list[1]) / ROW) as usize;
        if i < n && i < c.list_off + rows {
            c.sel = i;
            c.clip = usize::MAX;
            c.scroll = 0;
            c.started = now;
        }
        return;
    }
    if inside(l.clips, at) {
        let i = ((at[1] - l.clips[1]) / (ROW + 1.0)) as usize;
        c.clip = i;
        c.started = now;
        c.scroll = 0;
    }
}

/// `native_block_entity_submit` for the book's figure (in the window's scene only). False for
/// any other block entity.
pub(super) fn draw_figure(game: &Game<'_>, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    if entity.id != FIGURE {
        return false;
    }
    let Some(f) = shown().clone() else { return true };
    let t = (draw.game_time - f.start) as f32 + draw.partial;
    let t = if f.period > 0.0 { t % f.period } else { t };
    draw.push();
    draw.translate([0.5, 0.0, 0.5]);
    let (tint, items) = if f.silhouette { (0, false) } else { (0xFF_FFFF, true) };
    let flags: [&str; 0] = [];
    client::draw_rig_with(game, &f.mob, || client::load_rig(game, &f.mob), &f.clip, t, &flags, draw, 7, tint, items);
    draw.pop();
    true
}

