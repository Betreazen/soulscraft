// The summoning altar on the server (docs/design/06-altar.md; the rules are soulscraft_rules::altar):
// which cores stand where, their level, the tall pieces placed and broken together, the pedestal
// and its summon item, the menu, and what clients are told to draw.
//
// Cheap by design: nothing scans the world each tick. `native_block_changed` (block_changed hook,
// GameLevel::observe_block_changed, game.rs:1388-1392, every committed `Level.setBlock` of an
// authoritative server, no `mods()` check) queues only changes to SoulsCraft's own blocks and to
// blocks inside a known altar's event zone. The level tick then fixes tall pieces, marks the
// altar whose 9 × 9 × 4 cells or roof column changed, and evaluates only that altar: 324 cells and
// one heightmap column. Cores are found when placed (the queue) and when their chunk loads (their
// block entity ticks, `native_block_entity_tick`, and says where it is).
//
// What is saved: the pedestal's item and the core's spawn-boost switch and the event's broken
// blocks are block entity data (`native_with_mod_block_entity`, sent to clients by
// `native_block_entity_update_tag`); a level is worked out again whenever a core is seen.
//
// Log lines:
//   soulscraft: altar core=-700,101,700 level=2 built=2 sky=true banners=[weight] wrong=0
//   soulscraft: altar tall=soulscraft:altar_pillar at=-698,102,698 placed parts=2
//   soulscraft: pedestal at=-700,102,700 item=soulscraft:rotten_horn by=sc_a
use crate::game::Game;
use crate::game::soulscraft_event as event;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use soulscraft_rules::{
    altar::{self, AltarFile, Report, Seen},
    data::MobsFile,
    view,
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use crate::game::soulscraft_altar_ui::{menu_of, send_views};
pub(super) use crate::game::soulscraft_altar_ui::{build_here, event_here, item_name};

/// The pedestal's item in its block entity.
const ITEM_KEY: &str = "item";
/// The core's spawn-boost switch (1 on, 0 off; on by default).
const BOOST_KEY: &str = "boost";
/// The game time the core was first seen (the spawn boost's age; survives restarts).
const SINCE_KEY: &str = "since";
/// The core's book of broken blocks while an event runs (soulscraft_rules::defense::Breaches).
pub(super) const BREACHED_KEY: &str = "breached";
/// Views go out this often (soulscraft_altar_ui::send_views).
const VIEW_EVERY: u64 = 10;
/// A core whose block entity has not ticked for this long left with its chunk.
const FORGET_AFTER: u64 = 100;

/// One altar the server knows.
#[derive(Clone, Debug)]
pub(super) struct Altar {
    pub dim: String,
    pub core: [i32; 3],
    pub report: Report,
    pub sky: bool,
    pub level: u32,
    pub boost: bool,
    pub dirty: bool,
    pub seen: u64,
    /// The game time the core was first seen (block entity `since`): the spawn boost's sector
    /// drifts by the altar's age (soulscraft_spawn_boost).
    pub since: i64,
}

impl Altar {
    pub fn banner_ids(&self) -> Vec<String> {
        self.report.banners.iter().map(|b| b.1.clone()).collect()
    }
    pub fn pedestal(&self) -> [i32; 3] {
        [self.core[0], self.core[1] + 1, self.core[2]]
    }
}

/// A block change from the hook: where, and the state ids before and after.
#[derive(Clone, Copy, Debug)]
struct Change {
    pos: [i32; 3],
    old: BlockStateId,
    new: BlockStateId,
}

/// A request from a player's hook (their state swapped in), done in the level's tick.
#[derive(Clone, Debug)]
enum Action {
    /// Put this item on the pedestal (already taken from the player).
    Put { pos: [i32; 3], item: String, by: String },
    /// Take the pedestal's item off (already given to the player).
    Take { pos: [i32; 3] },
    OpenMenu { player: u128, core: [i32; 3] },
}

struct Hub {
    file: std::sync::Arc<AltarFile>,
    altars: HashMap<(String, [i32; 3]), Altar>,
    /// Pedestals' items as the server last wrote them: a player's hook decides from this.
    pedestals: HashMap<(String, [i32; 3]), Option<String>>,
    actions: Vec<(String, Action)>,
    /// Players with an altar's menu open: what they see is sent again each second.
    menus: HashMap<u128, (String, [i32; 3])>,
    /// Tall pieces a creative player broke: the rest goes without a drop.
    creative: Vec<(String, [i32; 3])>,
    /// The game time each dimension was last ticked (a frozen world does nothing twice).
    last: HashMap<String, u64>,
}

static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(|| {
    Mutex::new(Hub {
        file: std::sync::Arc::new(AltarFile::embedded()),
        altars: HashMap::new(),
        pedestals: HashMap::new(),
        actions: Vec::new(),
        menus: HashMap::new(),
        creative: Vec::new(),
        last: HashMap::new(),
    })
});

/// The hook's queue (dimension, change). Its own lock: the hook runs inside any `setBlock`,
/// including ours from the level tick, so nothing that locks [`HUB`] may be taken there.
static CHANGES: Mutex<Vec<(&'static str, Change)>> = Mutex::new(Vec::new());
/// The zones the hook watches: (dimension, min x, min z, max x, max z).
static ZONES: Mutex<Vec<(&'static str, [i32; 4])>> = Mutex::new(Vec::new());
/// Cores whose block entity ticked since the level's last tick.
static TICKED: Mutex<Vec<[i32; 3]>> = Mutex::new(Vec::new());

fn hub() -> MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The altar data, shared: the spawn boost asks for it every level tick (a copy of the whole file
/// each time cost 0.01 ms per tick, docs/design/12-playtest-tidewill.md).
pub(super) fn file() -> std::sync::Arc<AltarFile> {
    std::sync::Arc::clone(&hub().file)
}

/// A copy of the altars of a dimension (the event reads them).
pub(super) fn altar(dim: &str, core: [i32; 3]) -> Option<Altar> {
    hub().altars.get(&(dim.to_owned(), core)).cloned()
}

pub(super) fn altars_in(dim: &str) -> Vec<Altar> {
    hub().altars.values().filter(|a| a.dim == dim).cloned().collect()
}

fn dim_key(d: horizon_world::level::Dimension) -> &'static str {
    match d {
        horizon_world::level::Dimension::Overworld => "minecraft:overworld",
        horizon_world::level::Dimension::Nether => "minecraft:the_nether",
        horizon_world::level::Dimension::End => "minecraft:the_end",
    }
}

fn static_dim(dim: &str) -> &'static str {
    match dim {
        "minecraft:the_nether" => "minecraft:the_nether",
        "minecraft:the_end" => "minecraft:the_end",
        _ => "minecraft:overworld",
    }
}

/// The block_changed hook (GameLevel, read-only): keeps the change when it touches SoulsCraft's
/// blocks or a watched zone. Name lookups are the level's fact table (an index).
pub(super) fn on_block_changed(
    facts: &horizon_client::blocks::BlockFacts,
    dimension: horizon_world::level::Dimension,
    pos: [i32; 3],
    old: BlockStateId,
    new: BlockStateId,
) {
    let dim = dim_key(dimension);
    let ours = facts.get(old).name.starts_with("soulscraft:") || facts.get(new).name.starts_with("soulscraft:");
    let watched = ours
        || ZONES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|(d, z)| *d == dim && pos[0] >= z[0] && pos[2] >= z[1] && pos[0] <= z[2] && pos[2] <= z[3]);
    if watched {
        CHANGES.lock().unwrap_or_else(PoisonError::into_inner).push((dim, Change { pos, old, new }));
    }
}

/// `native_block_entity` (API 15): the core and the pedestal have block entities.
pub(super) fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && (block == altar::CORE || block == altar::PEDESTAL) {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// `native_block_entity_ticks`: the core ticks, so a loaded chunk tells where its core is.
pub(super) fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == altar::CORE {
        *ticks = true;
    }
}

pub(super) fn on_block_entity_tick(pos: [i32; 3], entity: &horizon_client::containers::ModBlockEntity) {
    if entity.id == altar::CORE {
        TICKED.lock().unwrap_or_else(PoisonError::into_inner).push(pos);
    }
}

/// `native_block_entity_update_tag` (API 45): clients get the pedestal's item and the core's
/// switch, which is all they draw.
pub(super) fn on_update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id == altar::PEDESTAL || entity.id == altar::CORE {
        let mut c = Compound::new();
        if let Some(v) = entity.nbt.get(&JavaString::from(ITEM_KEY)) {
            c.insert(JavaString::from(ITEM_KEY), v.clone());
        }
        *tag = Some(c);
    }
}

fn nbt_string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// The item on the pedestal at `pos`, from its block entity.
pub(super) fn pedestal_item(game: &Game<'_>, pos: [i32; 3]) -> Option<String> {
    game.native_mod_block_entity(pos).and_then(|e| nbt_string(&e.nbt, ITEM_KEY))
}

/// Writes the pedestal's item and tells clients.
pub(super) fn set_pedestal(game: &mut Game<'_>, dim: &str, pos: [i32; 3], item: Option<&str>) {
    game.native_with_mod_block_entity(pos, |e| match item {
        Some(i) => {
            e.nbt.insert(JavaString::from(ITEM_KEY), Tag::String(JavaString::from(i)));
        }
        None => {
            e.nbt.remove(&JavaString::from(ITEM_KEY));
        }
    });
    game.native_block_entity_sync(pos);
    hub().pedestals.insert((dim.to_owned(), pos), item.map(str::to_owned));
}

/// The core's saved lines under `key` (the book of broken blocks).
pub(super) fn core_lines(game: &Game<'_>, core: [i32; 3], key: &str) -> Vec<String> {
    match game.native_mod_block_entity(core).and_then(|e| e.nbt.get(&JavaString::from(key)).cloned()) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| if let Tag::String(s) = t { Some(s.to_string_lossy()) } else { None })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn set_core_lines(game: &mut Game<'_>, core: [i32; 3], key: &str, lines: &[String]) {
    game.native_with_mod_block_entity(core, |e| {
        if lines.is_empty() {
            e.nbt.remove(&JavaString::from(key));
        } else {
            let list = lines.iter().map(|l| Tag::String(JavaString::from(l.as_str()))).collect();
            e.nbt.insert(JavaString::from(key), Tag::List(list));
        }
    });
}

fn read_boost(game: &Game<'_>, core: [i32; 3]) -> bool {
    !matches!(
        game.native_mod_block_entity(core).and_then(|e| e.nbt.get(&JavaString::from(BOOST_KEY)).cloned()),
        Some(Tag::Byte(0))
    )
}

/// Turns a known altar's spawn boost on or off, as its menu's buttons do (the self-test).
pub(super) fn set_boost(dim: &str, core: [i32; 3], on: bool) {
    if let Some(a) = hub().altars.get_mut(&(dim.to_owned(), core)) {
        a.boost = on;
    }
}

/// How long the altar has stood (its spawn boost's age), from since.
pub(super) fn age(a: &Altar, now: u64) -> u64 {
    (now as i64).saturating_sub(a.since).max(0) as u64
}

/// When the core was first seen: its block entity's `since`, written now the first time.
fn read_since(game: &mut Game<'_>, core: [i32; 3], now: u64) -> i64 {
    match game.native_mod_block_entity(core).and_then(|e| e.nbt.get(&JavaString::from(SINCE_KEY)).cloned()) {
        Some(Tag::Long(v)) => v,
        _ => {
            game.native_with_mod_block_entity(core, |e| {
                e.nbt.insert(JavaString::from(SINCE_KEY), Tag::Long(now as i64));
            });
            now as i64
        }
    }
}

/// Sets a known core's first-seen time (the self-test's way to age an altar).
pub(super) fn set_since(game: &mut Game<'_>, dim: &str, core: [i32; 3], since: i64) {
    game.native_with_mod_block_entity(core, |e| {
        e.nbt.insert(JavaString::from(SINCE_KEY), Tag::Long(since));
    });
    if let Some(a) = hub().altars.get_mut(&(dim.to_owned(), core)) {
        a.since = since;
    }
}

/// The block at `pos` as the altar reads it: its id and its `part`.
pub(super) fn seen_at(game: &Game<'_>, pos: [i32; 3]) -> Seen {
    let id = game.world.block(pos[0], pos[1], pos[2]);
    match game.pack.assets.registry.state(id) {
        Some(st) => Seen { block: st.name.clone(), part: st.properties.get("part").cloned() },
        None => Seen::air(),
    }
}

/// The full state at `pos` for `setblock`: `minecraft:oak_stairs[facing=east,half=bottom,...]`.
pub(super) fn state_string_at(game: &Game<'_>, pos: [i32; 3]) -> String {
    let id = game.world.block(pos[0], pos[1], pos[2]);
    state_string_of(game, id)
}

fn state_string_of(game: &Game<'_>, id: BlockStateId) -> String {
    match game.pack.assets.registry.state(id) {
        Some(st) => {
            let props: Vec<(&str, &str)> =
                st.properties.keys().zip(st.properties.values()).map(|(k, v)| (k.as_str(), v.as_str())).collect();
            altar::state_string(&st.name, &props)
        }
        None => "minecraft:air".into(),
    }
}

fn replaceable(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let id = game.world.block(pos[0], pos[1], pos[2]);
    game.res.facts().get(id).replaceable
}

pub(super) fn run(game: &mut Game<'_>, dim: &str, line: &str) -> bool {
    let full = if dim == "minecraft:overworld" { line.to_owned() } else { format!("execute in {dim} run {line}") };
    game.native_run_command(&full)
}

pub(super) fn setblock(game: &mut Game<'_>, dim: &str, pos: [i32; 3], state: &str, mode: &str) -> bool {
    run(game, dim, &format!("setblock {} {} {} {state} {mode}", pos[0], pos[1], pos[2]))
}

/// `native_right_click_block` (a player's hook, their state swapped in): the pedestal takes or
/// gives its item, the core opens its menu to an empty hand. The world changes wait for the
/// level's tick; the player's items change now.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let block = game.native_block(pos);
    if block != altar::PEDESTAL && block != altar::CORE {
        return;
    }
    let Some(player) = game.native_acting_player() else { return };
    if player.sneaking {
        return;
    }
    let held = game.native_held().map(|h| h.0);
    let dim = player.dimension.clone();
    if block == altar::CORE {
        if held.is_none() {
            *canceled = true;
            let core = pos;
            hub().actions.push((dim, Action::OpenMenu { player: player.uuid, core }));
        }
        return;
    }
    *canceled = true;
    let key = (dim.clone(), pos);
    let known = hub().pedestals.get(&key).cloned();
    let on = match known {
        Some(v) => v,
        None => pedestal_item(game, pos),
    };
    let items: Vec<String> = summon_items();
    match (on, held) {
        (None, Some(item)) if items.contains(&item) => {
            if game.native_take_items(&item, 1) == 1 {
                hub().pedestals.insert(key, Some(item.clone()));
                hub().actions.push((dim, Action::Put { pos, item, by: player.name.clone() }));
            }
        }
        (Some(item), None) => {
            if game.native_give_items(&item, 1) == 1 {
                hub().pedestals.insert(key, None);
                hub().actions.push((dim, Action::Take { pos }));
            }
        }
        _ => {}
    }
}

/// The items that start an event from the pedestal (data/mobs.json `events[].item`).
pub(super) fn summon_items() -> Vec<String> {
    crate::game::soulscraft_server::state().data.events.iter().map(|e| e.item.clone()).filter(|i| !i.is_empty()).collect()
}

/// `native_destroy_block` (a player's hook): a creative player's break of a tall piece drops
/// nothing (as a door).
pub(super) fn on_destroy_block(game: &mut Game<'_>, pos: [i32; 3]) {
    let Some(player) = game.native_acting_player() else { return };
    if player.game_mode != 1 {
        return;
    }
    let block = game.native_block(pos);
    if !altar::tall_parts(&block).is_empty() {
        hub().creative.push((player.dimension.clone(), pos));
    }
}

/// The self-test's and `/sc altar`'s way to put an item on a pedestal (no player).
pub(super) fn put(dim: &str, pos: [i32; 3], item: &str) {
    hub().pedestals.insert((dim.to_owned(), pos), Some(item.to_owned()));
    hub().actions.push((dim.to_owned(), Action::Put { pos, item: item.to_owned(), by: "-".into() }));
}

/// `native_net_from_client` on the menu's channel: the spawn boost's buttons, and a closed menu.
pub(super) fn on_press(player: u128, data: &[u8]) {
    let Some(p) = view::decode::<view::Press>(data) else { return };
    let mut h = hub();
    let Some((dim, core)) = h.menus.get(&player).cloned() else { return };
    if core != p.core {
        return;
    }
    match p.button {
        view::Button::Closed => {
            h.menus.remove(&player);
        }
        view::Button::BoostOn | view::Button::BoostOff => {
            let on = p.button == view::Button::BoostOn;
            if let Some(a) = h.altars.get_mut(&(dim.clone(), core)) {
                a.boost = on;
            }
            h.actions.push((dim, Action::OpenMenu { player, core }));
        }
    }
}

/// The level's tick: cores found, changes handled, levels worked out, events run, views sent.
pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    {
        let mut h = hub();
        if h.last.get(dim) == Some(&now) {
            return;
        }
        h.last.insert(dim.to_owned(), now);
    }
    register_ticked(game, dim, now);
    let changes: Vec<Change> = {
        let mut q = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
        let (mine, rest): (Vec<_>, Vec<_>) = q.drain(..).partition(|(d, _)| *d == dim);
        *q = rest;
        mine.into_iter().map(|(_, c)| c).collect()
    };
    // New pieces first: a segment broken in the same tick as a new piece was set where it stood
    // (an operator's rebuild) must not take the new piece's other segments with it.
    for c in &changes {
        settle_new(game, dim, c);
    }
    for c in &changes {
        on_change(game, dim, now, c);
    }
    do_actions(game, dim, now);
    evaluate_dirty(game, dim, now);
    start_events(game, dim, now);
    event::tick(game, dim, now);
    if now % VIEW_EVERY == 0 {
        send_views(game, dim, now);
    }
    if now % 20 == 0 {
        refresh_menus(game, dim, now);
        forget_gone(dim, now);
    }
}

fn register_ticked(game: &mut Game<'_>, dim: &str, now: u64) {
    let cores: Vec<[i32; 3]> = std::mem::take(&mut *TICKED.lock().unwrap_or_else(PoisonError::into_inner));
    for core in cores {
        let known = {
            let mut h = hub();
            match h.altars.get_mut(&(dim.to_owned(), core)) {
                Some(a) => {
                    a.seen = now;
                    true
                }
                None => false,
            }
        };
        if !known && game.native_block(core) == altar::CORE {
            register(game, dim, core, now);
        }
    }
}

fn register(game: &mut Game<'_>, dim: &str, core: [i32; 3], now: u64) {
    let boost = read_boost(game, core);
    let since = read_since(game, core, now);
    hub().altars.insert(
        (dim.to_owned(), core),
        Altar { dim: dim.to_owned(), core, report: Report::default(), sky: false, level: 0, boost, dirty: true, seen: now, since },
    );
    // An event that was running when the server stopped left its broken blocks in the book.
    event::restore_orphans(game, dim, core);
    update_zones();
    log::info!("soulscraft: altar core={},{},{} registered tick={now}", core[0], core[1], core[2]);
}

fn update_zones() {
    let flow = crate::game::soulscraft_server::state().data.events.first().map(soulscraft_rules::event::Flow::of);
    let flow = flow.unwrap_or_else(|| soulscraft_rules::event::Flow::from_map(&Default::default()));
    let zones: Vec<(&'static str, [i32; 4])> = hub()
        .altars
        .values()
        .map(|a| {
            let (min, max) = flow.zone(a.core);
            (static_dim(&a.dim), [min[0], min[1], max[0], max[1]])
        })
        .collect();
    *ZONES.lock().unwrap_or_else(PoisonError::into_inner) = zones;
}

fn forget_gone(dim: &str, now: u64) {
    let gone: Vec<[i32; 3]> = hub()
        .altars
        .values()
        .filter(|a| a.dim == dim && now.saturating_sub(a.seen) > FORGET_AFTER && !event::running(dim, a.core))
        .map(|a| a.core)
        .collect();
    if gone.is_empty() {
        return;
    }
    let mut h = hub();
    for c in gone {
        h.altars.remove(&(dim.to_owned(), c));
    }
    drop(h);
    update_zones();
}

/// The cells an altar reads: its 9 × 9 floor and three layers above, and its core's column.
fn touches(a: &Altar, pos: [i32; 3]) -> bool {
    let d = [pos[0] - a.core[0], pos[1] - a.core[1], pos[2] - a.core[2]];
    let r = altar::MAX_RING as i32;
    (d[0].abs() <= r && d[2].abs() <= r && (0..altar::LAYERS).contains(&d[1])) || (d[0] == 0 && d[2] == 0 && d[1] >= 2)
}

fn names(game: &Game<'_>, c: &Change) -> ((String, Option<String>), (String, Option<String>)) {
    let reg = &game.pack.assets.registry;
    let get = |id: BlockStateId| {
        reg.state(id)
            .map(|s| (s.name.clone(), s.properties.get("part").cloned()))
            .unwrap_or_else(|| ("minecraft:air".to_owned(), None))
    };
    (get(c.old), get(c.new))
}

/// A piece set with its bottom (or a one-block piece) is put up and settled.
fn settle_new(game: &mut Game<'_>, dim: &str, c: &Change) {
    let (old, new) = names(game, c);
    let still = seen_at(game, c.pos).block == new.0;
    if still && old.0 != new.0 && altar::has_on_slab(&new.0) && new.1.as_deref().is_none_or(|p| p == "bottom") {
        settle_piece(game, dim, c.pos, &new.0);
    }
}

fn on_change(game: &mut Game<'_>, dim: &str, now: u64, c: &Change) {
    let (old, new) = {
        let reg = &game.pack.assets.registry;
        let get = |id: BlockStateId| {
            reg.state(id)
                .map(|s| (s.name.clone(), s.properties.get("part").cloned()))
                .unwrap_or_else(|| ("minecraft:air".to_owned(), None))
        };
        (get(c.old), get(c.new))
    };
    let pos = c.pos;
    if old.0 != new.0 {
        // The segment is gone, unless the same segment stands there again (a rebuild).
        if !altar::tall_parts(&old.0).is_empty() && seen_at(game, pos) != Seen::new(&old.0, old.1.as_deref()) {
            remove_siblings(game, dim, pos, &old.0, old.1.as_deref());
        }
        if new.0 == altar::CORE && !hub().altars.contains_key(&(dim.to_owned(), pos)) {
            register(game, dim, pos, now);
        }
        if old.0 == altar::CORE {
            event::core_gone(game, dim, pos, now);
            hub().altars.remove(&(dim.to_owned(), pos));
            update_zones();
            log::info!("soulscraft: altar core={},{},{} broken tick={now}", pos[0], pos[1], pos[2]);
        }
        if old.0 == altar::PEDESTAL {
            hub().pedestals.remove(&(dim.to_owned(), pos));
        }
    }
    {
        let mut h = hub();
        for a in h.altars.values_mut().filter(|a| a.dim == dim) {
            if touches(a, pos) {
                a.dirty = true;
            }
        }
    }
    let now_air = new.0 == "minecraft:air" || new.0 == "minecraft:cave_air";
    event::on_block_change(game, dim, pos, now_air, now);
}

/// A piece placed with its bottom (or a one-block piece): its other segments go up above it, it
/// stands lower on a slab, and a banner on a socket faces out from its altar. No room above: the
/// piece pops off as its item.
fn settle_piece(game: &mut Game<'_>, dim: &str, pos: [i32; 3], block: &str) {
    let parts = altar::tall_parts(block);
    let below = game.native_block([pos[0], pos[1] - 1, pos[2]]);
    let on_slab = if below == altar::SLAB || below == altar::SOCKET { "true" } else { "false" };
    let facing = if block.starts_with(altar::BANNER_PREFIX) {
        let core = hub()
            .altars
            .values()
            .filter(|a| a.dim == dim && pos[1] == a.core[1] + 1)
            .map(|a| a.core)
            .find(|c| {
                let (dx, dz) = (pos[0] - c[0], pos[2] - c[2]);
                dx.abs().max(dz.abs()) <= altar::MAX_RING as i32 && dx.abs() != dz.abs()
            });
        Some(match core {
            Some(c) => altar::facing_out(pos[0] - c[0], pos[2] - c[2]).to_owned(),
            None => game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into()),
        })
    } else {
        None
    };
    for i in 1..parts.len() {
        let cell = [pos[0], pos[1] + i as i32, pos[2]];
        if !replaceable(game, cell) {
            // No room: the piece comes off as its item, what was put up of it goes.
            for j in 1..i {
                setblock(game, dim, [pos[0], pos[1] + j as i32, pos[2]], "minecraft:air", "replace");
            }
            setblock(game, dim, pos, "minecraft:air", "destroy");
            log::info!("soulscraft: altar tall={block} at={},{},{} refused: no room", pos[0], pos[1], pos[2]);
            return;
        }
    }
    let mut props: Vec<(&str, &str)> = vec![("on_slab", on_slab)];
    if let Some(f) = facing.as_deref() {
        props.push(("facing", f));
    }
    for (k, v) in &props {
        game.native_set_block_property_with(pos, k, v, 3);
    }
    for (i, part) in parts.iter().enumerate().skip(1) {
        let mut p = props.clone();
        p.push(("part", *part));
        let cell = [pos[0], pos[1] + i as i32, pos[2]];
        setblock(game, dim, cell, &altar::state_string(block, &p), "replace");
    }
    if !parts.is_empty() {
        log::info!("soulscraft: altar tall={block} at={},{},{} placed parts={} on_slab={on_slab}", pos[0], pos[1], pos[2], parts.len());
    }
}

/// A segment of a tall piece went: the others go too, and the bottom drops the piece's item
/// unless a creative player broke it.
fn remove_siblings(game: &mut Game<'_>, dim: &str, pos: [i32; 3], block: &str, part: Option<&str>) {
    let parts = altar::tall_parts(block);
    let Some(i) = part.and_then(|p| parts.iter().position(|x| *x == p)) else { return };
    let creative = {
        let mut h = hub();
        let k = (dim.to_owned(), pos);
        let was = h.creative.contains(&k);
        h.creative.retain(|c| *c != k);
        was
    };
    for (j, want) in parts.iter().enumerate() {
        if j == i {
            continue;
        }
        let cell = [pos[0], pos[1] + j as i32 - i as i32, pos[2]];
        let seen = seen_at(game, cell);
        if seen.block != block || seen.part.as_deref() != Some(*want) {
            continue;
        }
        let mode = if j == 0 && !creative { "destroy" } else { "replace" };
        setblock(game, dim, cell, "minecraft:air", mode);
    }
}

fn do_actions(game: &mut Game<'_>, dim: &str, now: u64) {
    let actions: Vec<Action> = {
        let mut h = hub();
        let (mine, rest): (Vec<_>, Vec<_>) = h.actions.drain(..).partition(|(d, _)| d == dim);
        h.actions = rest;
        mine.into_iter().map(|(_, a)| a).collect()
    };
    for a in actions {
        match a {
            Action::Put { pos, item, by } => {
                set_pedestal(game, dim, pos, Some(&item));
                log::info!("soulscraft: pedestal at={},{},{} item={item} by={by} tick={now}", pos[0], pos[1], pos[2]);
                // The altar it stands on looks at it now.
                let core = [pos[0], pos[1] - 1, pos[2]];
                if let Some(a) = hub().altars.get_mut(&(dim.to_owned(), core)) {
                    a.dirty = true;
                }
            }
            Action::Take { pos } => {
                set_pedestal(game, dim, pos, None);
                log::info!("soulscraft: pedestal at={},{},{} item=- tick={now}", pos[0], pos[1], pos[2]);
            }
            Action::OpenMenu { player, core } => {
                if let Some(a) = altar(dim, core) {
                    game.native_with_mod_block_entity(core, |e| {
                        e.nbt.insert(JavaString::from(BOOST_KEY), Tag::Byte(i8::from(a.boost)));
                    });
                    hub().menus.insert(player, (dim.to_owned(), core));
                    send_menu(game, dim, player, &a, now);
                }
            }
        }
    }
}

/// Reads the structure of each altar a change touched.
fn evaluate_dirty(game: &mut Game<'_>, dim: &str, now: u64) {
    let dirty: Vec<[i32; 3]> = hub().altars.values().filter(|a| a.dim == dim && a.dirty).map(|a| a.core).collect();
    for core in dirty {
        evaluate(game, dim, core, now);
    }
}

/// Works out one altar's level now (the self-test asks for it too).
pub(super) fn evaluate(game: &mut Game<'_>, dim: &str, core: [i32; 3], now: u64) -> Option<Altar> {
    let file = file();
    let report = altar::evaluate(&file, |c| seen_at(game, [core[0] + c[0], core[1] + c[1], core[2] + c[2]]));
    let sky = open_sky(game, dim, core);
    let mut h = hub();
    let a = h.altars.get_mut(&(dim.to_owned(), core))?;
    let level = report.level(sky);
    let changed = level != a.level || report.banners != a.report.banners || sky != a.sky || report.wrong != a.report.wrong;
    a.report = report;
    a.sky = sky;
    a.level = level;
    a.dirty = false;
    let out = a.clone();
    drop(h);
    if changed {
        log::info!(
            "soulscraft: altar core={},{},{} level={level} built={} sky={sky} banners=[{}] wrong={} tick={now}",
            core[0],
            core[1],
            core[2],
            out.report.built,
            out.banner_ids().join(","),
            out.report.wrong.len()
        );
    }
    Some(out)
}

/// Nothing over the core but its pedestal: the world-surface height of its column is at most
/// two above it (round 5: the core needs the open sky).
fn open_sky(game: &Game<'_>, dim: &str, core: [i32; 3]) -> bool {
    use crate::game::native_world_edit::NativeHeightmap;
    match game.native_surface_heights(dim, &[(core[0], core[2])], NativeHeightmap::WorldSurface) {
        Ok(h) => h.first().copied().flatten().is_some_and(|top| top <= core[1] + 2),
        Err(_) => false,
    }
}

/// Altars with a summon item on the pedestal and the level its event needs start it.
fn start_events(game: &mut Game<'_>, dim: &str, now: u64) {
    let candidates: Vec<Altar> = altars_in(dim).into_iter().filter(|a| a.level > 0 && !event::running(dim, a.core)).collect();
    for a in candidates {
        let Some(item) = hub().pedestals.get(&(dim.to_owned(), a.pedestal())).cloned().unwrap_or_else(|| pedestal_item(game, a.pedestal())) else {
            continue;
        };
        let data: MobsFile = crate::game::soulscraft_server::state().data.clone();
        let Some((index, ev)) = data.events.iter().enumerate().find(|(_, e)| e.item == item) else { continue };
        if a.level < ev.tier.max(1) {
            continue;
        }
        match event::start(game, dim, &a, index, now, None) {
            Ok(()) => set_pedestal(game, dim, a.pedestal(), None),
            Err(e) => log::warn!("soulscraft: event not started at {:?}: {e}", a.core),
        }
    }
}

fn send_menu(game: &mut Game<'_>, dim: &str, player: u128, a: &Altar, now: u64) {
    let _ = dim;
    let bytes = view::encode(&menu_of(a, now));
    if let Err(e) = game.native_net_send_to_player(player, view::MENU, &bytes) {
        log::warn!("soulscraft: menu not sent: {e}");
    }
}

fn refresh_menus(game: &mut Game<'_>, dim: &str, now: u64) {
    let open: Vec<(u128, [i32; 3])> = hub().menus.iter().filter(|(_, (d, _))| d == dim).map(|(p, (_, c))| (*p, *c)).collect();
    let online: Vec<u128> = game.native_server_players().iter().map(|p| p.uuid).collect();
    for (player, core) in open {
        match altar(dim, core) {
            Some(a) if online.contains(&player) => send_menu(game, dim, player, &a, now),
            _ => {
                hub().menus.remove(&player);
            }
        }
    }
}

/// The braziers of an altar burn while its event runs (their `lit`).
pub(super) fn light_braziers(game: &mut Game<'_>, core: [i32; 3], lit: bool) {
    let r = 3;
    for [x, z] in [[-r, -r], [r, -r], [-r, r], [r, r]] {
        let pos = [core[0] + x, core[1] + 1, core[2] + z];
        if game.native_block(pos) == altar::BRAZIER {
            game.native_set_block_property_with(pos, "lit", if lit { "true" } else { "false" }, 3);
        }
    }
}

/// The altar's level after a fresh look (the self-test).
pub(super) fn level_now(game: &mut Game<'_>, dim: &str, core: [i32; 3], now: u64) -> Option<(u32, u32, bool, Vec<String>)> {
    if !hub().altars.contains_key(&(dim.to_owned(), core)) && game.native_block(core) == altar::CORE {
        register(game, dim, core, now);
    }
    evaluate(game, dim, core, now).map(|a| (a.level, a.report.built, a.sky, a.banner_ids()))
}

/// The self-test cleans up: forgets every altar of the dimension.
pub(super) fn forget_all(dim: &str) {
    hub().altars.retain(|k, _| k.0 != dim);
    hub().pedestals.retain(|k, _| k.0 != dim);
    update_zones();
}

