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
    /// A vanilla figure's attack playing (its phases pose the procedural figure).
    strip: Option<book_page::Strip>,
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
    let tab_rect = |i: f32| [px + PAD + i * (tw + 4.0), py + PAD, tw, TAB_H];
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
        tabs: [tab_rect(0.0), tab_rect(1.0), tab_rect(2.0)],
        list,
        model,
        clips,
        text,
        up: [px + pw - PAD - 34.0, arrows_y, 16.0, 12.0],
        down: [px + pw - PAD - 16.0, arrows_y, 16.0, 12.0],
    }
}

/// The page of the section and entry chosen.
fn page_of(game: &Game<'_>, c: &BookClient) -> Page {
    let d = &*DATA;
    let p = c.progress.clone().unwrap_or_default();
    match c.tab {
        1 => book_page::altar_page(&d.book, &d.altar, &p),
        2 => book_page::events_page(&d.book, &d.catalog, &d.trophies, &p),
        _ => match d.catalog.entries.get(c.sel) {
            Some((e, m)) => book_page::mob_page_with(&d.book, &d.catalog, &d.trophies, e, m, &p, &|k| crate::game::soulscraft_book::engine_stats(game, k)),
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
    let page = page_of(game, &c);
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
        log::info!("soulscraft: book window drawn tab={} title={} screen={:?} panel={:?}", c.tab, page.title, screen.rect, l.panel);
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
        let name = if stage == book::UNKNOWN { "???".to_owned() } else { fit(game, &e.name, r[2] - 12.0) };
        game.native_text(batch, &name, [r[0] + 10.0, r[1] + 1.5], if stage == book::UNKNOWN { 0x8C8478 } else { 0xE8E0D0 }, true);
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

/// The model in its clip, the clip buttons and the phase strip of the attack playing. The figure's
/// (height, width) in blocks for the scene's view, None for no scene.
fn draw_enemy(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, c: &mut BookClient, page: &Page) -> Option<(f32, f32)> {
    let d: &'static Data = &DATA;
    let (e, m) = d.catalog.entries.get(c.sel)?;
    game.native_fill(batch, l.model, 0x60_0C0A08);
    let now = game.world.game_time;
    if e.vanilla {
        draw_vanilla_enemy(game, batch, l, c, page, e, m, now)
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
        c.figure = Some(Figure { mob: e.id.clone(), clip: clip.clone(), start: c.started, period, silhouette: page.silhouette, strip: None });
        draw_clip_buttons(game, batch, l, clips.iter().map(|x| x.0.as_str()), c.clip);
        if let Some(s) = strip {
            let t = (now - c.started) as f32 + game.clock.fraction() as f32;
            let at = if period > 0.0 { t % period } else { t };
            draw_strip(game, batch, [l.clips[0], l.clips[1] + l.clips[3] - 14.0, l.clips[2], 6.0], s, at);
        }
        let hb = &m["hitbox"];
        Some((hb["height"].as_f64().unwrap_or(2.0) as f32, hb["width"].as_f64().unwrap_or(0.6) as f32))
    }
}

fn draw_clip_buttons<'a>(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, labels: impl Iterator<Item = &'a str>, sel: usize) {
    for (i, label) in labels.enumerate() {
        let r = clip_button(l, i);
        if r[1] + r[3] > l.clips[1] + l.clips[3] - 18.0 {
            break;
        }
        game.native_fill(batch, r, if i == sel { 0xFF5A_4630 } else { 0xFF34_2A20 });
        game.native_text(batch, label, [r[0] + 3.0, r[1] + 1.5], 0xE8E0D0, true);
    }
}

/// A changed vanilla mob's figure: its own vanilla model (`draw_vanilla_figure`), each type of
/// its row in turn, posed by hand (rest, a walk, an attack's phases), with the same buttons and
/// phase strips as ours. A type whose model would not draw shows its spawn egg instead.
#[allow(clippy::too_many_arguments)]
fn draw_vanilla_enemy(game: &Game<'_>, batch: &mut HudBatch, l: &Layout, c: &mut BookClient, page: &Page, e: &book::Entry, m: &Value, now: i64) -> Option<(f32, f32)> {
    let mut clips: Vec<(String, &str, Option<book_page::Strip>)> = Vec::new();
    if !page.silhouette && page.stage >= book::MET {
        clips.push(("Покой".into(), "idle", None));
        if page.moves {
            clips.push(("Шаг".into(), "walk", None));
        }
        clips.extend(page.strips.iter().map(|s| (s.name.clone(), "attack", Some(s.clone()))));
    }
    let pick = if page.strips.is_empty() { 0 } else { clips.len().saturating_sub(page.strips.len()) };
    if c.clip == usize::MAX || c.clip >= clips.len().max(1) {
        c.clip = pick;
    }
    let (clip, strip) = clips.get(c.clip).map_or(("idle", None), |x| (x.1, x.2.clone()));
    let period = strip.as_ref().map_or(0.0, |s| s.ticks() as f32 + REST_TICKS);
    let shown_for = ((now - c.started).max(0) / VANILLA_TYPE_TICKS) as usize;
    let kind = e.keys[shown_for % e.keys.len()].clone();
    draw_clip_buttons(game, batch, l, clips.iter().map(|x| x.0.as_str()), c.clip);
    if let Some(s) = &strip {
        let t = (now - c.started) as f32 + game.clock.fraction() as f32;
        draw_strip(game, batch, [l.clips[0], l.clips[1] + l.clips[3] - 14.0, l.clips[2], 6.0], s, t % period);
    }
    if vanilla_failed(&kind) {
        c.figure = None;
        let at = [l.model[0] + l.model[2] / 2.0 - 16.0, l.model[1] + l.model[3] / 2.0 - 16.0];
        let egg = format!("{kind}_spawn_egg");
        if page.silhouette || !game.native_item_model(batch, &egg, &egg, at, 32.0) {
            game.native_text(batch, "?", [at[0] + 14.0, at[1] + 12.0], 0x8C8478, true);
        }
        return None;
    }
    c.figure = Some(Figure { mob: kind.clone(), clip: clip.to_owned(), start: c.started, period, silhouette: page.silhouette, strip });
    if s_of(m, "behavior") == "cube" {
        // The model is a size-1 cube's (vanilla scales it by size when it draws it).
        return Some((0.52, 0.52));
    }
    vanilla_size(game, &kind).or(Some((2.0, 0.6)))
}

fn s_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn clip_button(l: &Layout, i: usize) -> [f32; 4] {
    [l.clips[0], l.clips[1] + i as f32 * (ROW + 1.0), l.clips[2], ROW]
}

/// The figure: one barrier block (no faces) with the book's figure entity, seen from the front.
fn draw_scene(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], (height, width): (f32, f32)) {
    let Some(state) = game.pack.assets.registry.default_state("minecraft:barrier") else { return };
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
    game.native_text(batch, &fit(game, &page.title, l.text[2]), [l.text[0], l.text[1]], 0xE8C878, true);
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
    if f.mob.starts_with("minecraft:") {
        draw_vanilla_figure(game, &f, t, tint, draw);
    } else {
        let flags: [&str; 0] = [];
        client::draw_rig_with(game, &f.mob, || client::load_rig(game, &f.mob), &f.clip, t, &flags, draw, 7, tint, items);
    }
    draw.pop();
    true
}


// ---- A changed vanilla mob's figure (round 14 (2)) ----
//
// Vanilla's renderer for an entity cannot be called from a screen or a scene: `native_draw_scene`
// draws blocks and calls `native_block_entity_submit` for a block entity (native_scene.rs:155-166),
// and the engine's mob drawing (`native_mob_submit`, game/native_mob_draw.rs:110-118) runs for mobs in the
// world only. What a block entity's draw can do is draw a model part tree with a texture
// (`native_draw_model_part`, native_block_entity_draw.rs:727) — and a vanilla type's own model is
// such a tree: its `MobType.model` and `MobType.texture` (mobs.rs:412-416, the same layers
// `native_bake_layer` bakes, :750-762). So the book draws the vanilla mob's real model and skin,
// posed here by hand (rest, a walk, an attack's phases): not vanilla's own animation or its extra
// layers (a drowned's or a stray's clothes, armour, held items).

/// A vanilla type's model tree and skin, made once.
struct VanillaFigure {
    tree: horizon_client::mobs::model_layers::ModelPart,
    /// A slime's outer cube, drawn over the inner one.
    outer: Option<horizon_client::mobs::model_layers::ModelPart>,
    texture: String,
}

/// Each type of a row shows this long in turn.
const VANILLA_TYPE_TICKS: i64 = 80;

static VANILLA: LazyLock<Mutex<std::collections::HashMap<String, Option<VanillaFigure>>>> = LazyLock::new(Mutex::default);

fn vanilla_figures() -> MutexGuard<'static, std::collections::HashMap<String, Option<VanillaFigure>>> {
    VANILLA.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The type's model would not draw (no type, or its skin is not in the atlas): the window shows
/// its spawn egg.
fn vanilla_failed(kind: &str) -> bool {
    vanilla_figures().get(kind).is_some_and(Option::is_none)
}

/// The type's (height, width) in blocks.
fn vanilla_size(game: &Game<'_>, kind: &str) -> Option<(f32, f32)> {
    let t = game.res.mob_types().get(kind)?;
    Some((t.height as f32, t.width as f32))
}

fn make_vanilla(game: &Game<'_>, kind: &str) -> Option<VanillaFigure> {
    use horizon_client::mobs::model_layers::ModelPart;
    let t = game.res.mob_types().get(kind)?;
    let path = kind.trim_start_matches("minecraft:");
    let outer = t.layers.get(&format!("{path}#outer")).filter(|_| path == "slime").map(|p| ModelPart::from_part(p));
    Some(VanillaFigure { tree: ModelPart::from_part(&t.model), outer, texture: t.texture.clone() })
}

/// Draws the vanilla type `f.mob` in its pose at `t` ticks, from the draw's frame at the feet.
fn draw_vanilla_figure(game: &Game<'_>, f: &Figure, t: f32, tint: u32, draw: &mut BlockEntityDraw) {
    let mut all = vanilla_figures();
    if !all.contains_key(&f.mob) {
        let made = make_vanilla(game, &f.mob);
        if made.is_none() {
            log::warn!("soulscraft: book: no vanilla model for {}", f.mob);
        }
        all.insert(f.mob.clone(), made);
    }
    let Some(v) = all.get_mut(&f.mob).and_then(Option::as_mut) else { return };
    pose_vanilla(&mut v.tree, &f.clip, t, f.strip.as_ref());
    draw.push();
    draw.scale([1.0, -1.0, -1.0]);
    draw.translate([0.0, -1.501, 0.0]);
    let drawn = game.native_draw_model_part(draw, &v.tree, &v.texture, tint);
    if let Some(outer) = &v.outer {
        game.native_draw_model_part(draw, outer, &v.texture, tint);
    }
    draw.pop();
    if !drawn {
        log::warn!("soulscraft: book: the skin {} of {} is not in the atlas", v.texture, f.mob);
        all.insert(f.mob.clone(), None);
    }
}

/// A hand-made pose on the model's top parts by their vanilla names: at rest the head looks round;
/// a walk swings legs (and arms against them); an attack raises arms or lowers the head in its
/// windup and hold, and strikes in its hit.
fn pose_vanilla(tree: &mut horizon_client::mobs::model_layers::ModelPart, clip: &str, t: f32, strip: Option<&book_page::Strip>) {
    use book_page::Seg;
    tree.reset_all_poses();
    let swing = (t * 0.45).sin();
    // How far the attack is: raised 0..1 in its windup, 1 in its hold, struck in its hit.
    let (raise, strike) = match strip.and_then(|s| seg_progress(s, t)) {
        Some((Seg::Windup, k)) => (k, 0.0),
        Some((Seg::Hold, _)) => (1.0, 0.0),
        Some((Seg::Hit, k)) => (1.0 - k, k),
        Some((Seg::Window, k)) => (0.0, 1.0 - k),
        None => (0.0, 0.0),
    };
    for (name, p) in tree.children_mut() {
        let left = name.starts_with("left");
        let side = if left ^ name.contains("hind") { 1.0 } else { -1.0 };
        match clip {
            "walk" if name.ends_with("_leg") => p.x_rot += swing * 0.7 * side,
            "walk" if name.ends_with("_arm") => p.x_rot -= swing * 0.6 * side,
            "idle" if name == "head" => p.y_rot += (t * 0.05).sin() * 0.35,
            "attack" if name.ends_with("_arm") || name == "arms" => p.x_rot += -2.4 * raise - 1.2 * strike,
            "attack" if name == "head" => p.x_rot += 0.45 * raise - 0.35 * strike,
            _ => {}
        }
    }
}

/// The phase at tick `t` of an attack and how far into it (0..1).
fn seg_progress(s: &book_page::Strip, t: f32) -> Option<(book_page::Seg, f32)> {
    let mut at = t.max(0.0);
    for (seg, n) in &s.segs {
        let n = *n as f32;
        if at < n {
            return Some((*seg, at / n.max(1.0)));
        }
        at -= n;
    }
    None
}

/// `text` cut to `width` GUI pixels with «…» (a vanilla row's long name in the list and the title).
fn fit(game: &Game<'_>, text: &str, width: f32) -> String {
    if game.native_text_width(text) <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        out.push(ch);
        if game.native_text_width(&format!("{out}…")) > width {
            out.pop();
            break;
        }
    }
    format!("{}…", out.trim_end_matches([' ', ',']))
}
