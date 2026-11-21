// The hunter's book on the server (docs/design/08-guide-book.md; soulscraft_rules::book): each
// player's progress (met, weighted kills, altar and event entries) in their persistent data under
// `soulscraft:book`, beside the other mods' keys; kill credit; the action-bar line of a new stage;
// the progress sent to the player's client at login, on each new entry and when the book opens.
//
// What it hears, and where (paths under SRC/crates/horizon-client/src):
// - a mob's death: a mixin at `Game::mob_death_canceled` (game/native_hooks/living_death.rs:61,
//   before `last_hurt_by_player` is cleared and the death is told);
// - players' hits: each level tick, a mob of the book whose `last_damage` (mobs.rs:1543) changed,
//   by `Who::Player` and the hit's player in `Game::last_hurt_by_player` (a `PlayerCredit`: the uuid);
// - «Встречен»: the brain's target (soulscraft_server calls `saw` and `struck`), and for vanilla
//   mobs their AI target every VANILLA_EVERY ticks: no scan of players × mobs;
// - turret kills: RustCraft's `soulscraft-kill …` report, a command lib.rs registers at level 4,
//   reaches `on_console` through soulscraft_tools::on_mod_command;
// - spawners' mobs: `Game::add_planned`'s ids (game/mobs/spawner.rs:151), tagged `soulscraft.spawner`;
// - the book's use: lib.rs's `HunterBook` ItemPart queues the player in `soulscraft_rules::bridge`.
//
// Log lines: `soulscraft: book player=sc_a entry=revenant stage=Убит kills=1`,
// `soulscraft: book credit mob=revenant id=12 players=[sc_a] weight=1 how=kill`.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_altar as altar;
use horizon_core::nbt::Tag;
use horizon_client::text::Text;
use horizon_client::mobs::{Mob, Who};
use soulscraft_rules::{
    altar::AltarFile,
    book::{self, BookFile, BookMsg, Catalog, Held, HitLog, MachineKill, Progress, Unlock},
    geom::{self, V3},
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

/// «Встречен»: the mob was this close.
const MEET_BLOCKS: f64 = 16.0;
const VANILLA_EVERY: u64 = 10;
const HELD_EVERY: u64 = 20;
const ALTAR_EVERY: u64 = 20;
/// An altar first seen this young was built now (its core's `since`).
const YOUNG_ALTAR: i64 = 200;
/// Players near an altar whose level rose get its levels.
const ALTAR_NEAR: f64 = 16.0;
/// A death or a turret report waits this long for the other.
const DEATH_KEEP: u64 = 100;
/// After a login the progress goes again this much later (the client may still be configuring).
const RESYNC_AFTER: u64 = 40;
const LINES_KEEP: usize = 256;

/// One player's book on the server.
struct Player {
    name: String,
    progress: Progress,
    /// Read from the persistent data (never written before it was).
    loaded: bool,
    /// Its saved text read (an unreadable one is never written over).
    writable: bool,
    /// Changed since it was last written.
    dirty: bool,
    /// Changed since the client last got it.
    unsent: bool,
    /// New stages this tick, for the action bar.
    lines: Vec<String>,
    /// When it was first loaded (a second sync follows).
    since: u64,
    resent: bool,
    /// A self-test stand-in: memory only.
    stand_in: Option<u32>,
}

/// A death as the hook saw it.
#[derive(Clone, Debug)]
struct Death {
    id: u32,
    kind: String,
    pos: V3,
    killer: Option<u128>,
    spawner: bool,
    tick: u64,
    credited: Vec<u128>,
}

struct Book {
    file: BookFile,
    catalog: Arc<Catalog>,
    altar: AltarFile,
    keys: HashSet<String>,
    players: HashMap<u128, Player>,
    hits: HitLog,
    ages: HashMap<u32, i64>,
    deaths: HashMap<u32, Death>,
    /// Machine kill reports waiting for their death: (report, the tick it came).
    reports: Vec<(MachineKill, u64)>,
    /// (entity, owner) credited from a report without a death heard: a second report of it is not.
    machine_credited: HashSet<(u32, u128)>,
    /// Machine kill reports heard: [`pct-machine kill`, `soulscraft-kill`] (the self-test).
    heard: [u32; 2],
    spawner_ids: HashSet<u32>,
    untagged: Vec<u32>,
    meets: Vec<(String, u128)>,
    opens: Vec<u128>,
    levels: HashMap<(String, [i32; 3]), u32>,
    ended: Vec<(String, Vec<u128>, Vec<u32>, bool)>,
    /// Every unlock line, for the self-test and the bots.
    lines: Vec<String>,
}

static BOOK: LazyLock<Mutex<Book>> = LazyLock::new(|| {
    let catalog = Catalog::embedded();
    let keys = catalog.entries.iter().flat_map(|(e, _)| e.keys.iter().cloned()).collect();
    log::info!("soulscraft: book with {} enemy entries", catalog.entries.len());
    Mutex::new(Book {
        file: BookFile::embedded(),
        catalog: Arc::new(catalog),
        altar: AltarFile::embedded(),
        keys,
        players: HashMap::new(),
        hits: HitLog::default(),
        ages: HashMap::new(),
        deaths: HashMap::new(),
        reports: Vec::new(),
        machine_credited: HashSet::new(),
        heard: [0; 2],
        spawner_ids: HashSet::new(),
        untagged: Vec::new(),
        meets: Vec::new(),
        opens: Vec::new(),
        levels: HashMap::new(),
        ended: Vec::new(),
        lines: Vec::new(),
    })
});

/// Deaths from the hook: its own lock (the hook runs inside the game's mob code).
static DEATHS_IN: Mutex<Vec<Death>> = Mutex::new(Vec::new());

fn book() -> MutexGuard<'static, Book> {
    BOOK.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- Hooks ----

/// `Game::mob_death_canceled` at its head: who dealt the last blow, while the engine still knows.
pub(super) fn on_mob_death(game: &Game<'_>, m: &Mob) {
    let kind = m.kind.id.to_string();
    if !book().keys.contains(&kind) {
        return;
    }
    let attacker = m.last_damage.and_then(|(d, _)| d.attacker);
    let killer = match attacker {
        Some(Who::Player) => game.last_hurt_by_player.get(&m.id).map(|c| c.0),
        Some(Who::Mob(e)) => stand_in_of(e),
        None => None,
    };
    let spawner = book().spawner_ids.contains(&m.id)
        || game.sc_entity(m.id).and_then(|e| game.sc_entity_tags(e)).is_some_and(|t| t.iter().any(|t| t == book::SPAWNER_TAG));
    DEATHS_IN.lock().unwrap_or_else(PoisonError::into_inner).push(Death {
        id: m.id,
        kind,
        pos: [m.position.x, m.position.y, m.position.z],
        killer,
        spawner,
        tick: game.world.game_time.max(0) as u64,
        credited: Vec::new(),
    });
}

/// `Game::add_planned`'s ids: the mobs a spawner made.
pub(super) fn on_spawner_planned(ids: &HashMap<u32, u32>) {
    let mut b = book();
    for id in ids.values() {
        if b.spawner_ids.insert(*id) {
            b.untagged.push(*id);
        }
    }
    if b.spawner_ids.len() > 4096 {
        b.spawner_ids.clear();
    }
}

/// Every console line: RustCraft's machine kill reports, its turrets' and (later) its guards',
/// by `soulscraft-kill` or the older `pct-machine kill`. True when the line is this mod's own
/// command (the console's `ran`); a `pct-machine` line is Path-of-craft-tree's and stays its.
pub(super) fn on_console(line: &str) -> bool {
    let own = line.trim_start_matches('/').split_whitespace().next() == Some(book::KILL_COMMAND);
    match book::machine_kill(line) {
        Some(k) => {
            let mut b = book();
            b.heard[usize::from(k.at.is_some())] += 1;
            b.reports.push((k, 0));
        }
        None if own => log::warn!("soulscraft: book: unreadable report `{line}`"),
        None => {}
    }
    own
}

/// soulscraft_server: a mob's brain chose `target` (entity, player, where) this tick.
pub(super) fn saw(kind: &str, mob: V3, target: Option<(u32, Option<u128>, V3)>) {
    let Some((entity, player, at)) = target else { return };
    if geom::dist(mob, at) > MEET_BLOCKS {
        return;
    }
    let Some(uuid) = player.or_else(|| stand_in_of(entity)) else { return };
    meet(kind, uuid);
}

/// soulscraft_server: a mob's blow landed on `entity` (a player or a stand-in).
pub(super) fn struck(kind: &str, entity: u32, player: Option<u128>) {
    if let Some(uuid) = player.or_else(|| stand_in_of(entity)) {
        meet(kind, uuid);
    }
}

fn meet(kind: &str, uuid: u128) {
    let mut b = book();
    let known = b.players.get(&uuid).is_some_and(|p| {
        p.loaded && b.catalog.by_key(kind).is_some_and(|e| p.progress.mobs.get(&e.id).is_some_and(|m| m.met))
    });
    if !known {
        b.meets.push((kind.to_owned(), uuid));
    }
}

/// soulscraft_event: a run ended; its players and stand-ins.
pub(super) fn event_ended(event: &str, players: Vec<u128>, stand_ins: Vec<u32>, won: bool) {
    book().ended.push((event.to_owned(), players, stand_ins, won));
}

// ---- The level tick ----

pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    let overworld = dim == "minecraft:overworld";
    if overworld {
        logins(game, now);
        tag_spawner_mobs(game);
        if now % HELD_EVERY == 0 {
            held_items(game);
        }
    }
    sample_hits(game, dim, now);
    if now % VANILLA_EVERY == 0 {
        vanilla_met(game, dim);
    }
    deaths(game, dim, now);
    if overworld {
        reports(game, now);
    }
    if now % ALTAR_EVERY == 0 {
        altar_levels(game, dim, now);
    }
    events(dim);
    flush(game, now);
}

/// Players online and not loaded yet are read; players gone are dropped (written already).
fn logins(game: &mut Game<'_>, now: u64) {
    let online = game.sc_server_players();
    let due: Vec<(u128, String)> = {
        let mut b = book();
        b.players.retain(|u, p| p.stand_in.is_some() || online.iter().any(|o| o.uuid == *u));
        online.iter().filter(|o| b.players.get(&o.uuid).is_none_or(|p| !p.loaded)).map(|o| (o.uuid, o.name.clone())).collect()
    };
    for (uuid, name) in due {
        load(game, uuid, &name, now);
    }
    let mut b = book();
    for p in b.players.values_mut() {
        if p.loaded && p.stand_in.is_none() && !p.resent && now >= p.since + RESYNC_AFTER {
            p.resent = true;
            p.unsent = true;
        }
    }
}

/// Reads a player's book from their persistent data. An unreadable text is logged and left as it
/// is: the player starts empty in memory, and the saved text is not written over.
fn load(game: &mut Game<'_>, uuid: u128, name: &str, now: u64) {
    let Some(data) = game.sc_player_persistent_data(uuid) else { return };
    let (progress, writable) = match &data {
        Tag::Compound(c) => match c.get(&horizon_core::nbt::JavaString::from(book::KEY)) {
            None => (Progress::default(), true),
            Some(Tag::String(s)) => match Progress::from_json(&s.to_string_lossy()) {
                Ok(p) => (p, true),
                Err(e) => {
                    log::warn!("soulscraft: book player={name} unreadable, kept as it is: {e}");
                    (Progress::default(), false)
                }
            },
            Some(_) => (Progress::default(), false),
        },
        _ => (Progress::default(), false),
    };
    log::info!("soulscraft: book player={name} loaded entries={} writable={writable}", progress.mobs.len());
    let mut b = book();
    let p = b.players.entry(uuid).or_insert_with(|| Player {
        name: name.to_owned(),
        progress: Progress::default(),
        loaded: false,
        writable: false,
        dirty: false,
        unsent: false,
        lines: Vec::new(),
        since: now,
        resent: false,
        stand_in: None,
    });
    // Anything credited before the read (a stand-in never is) joins what was saved.
    p.progress = progress;
    p.loaded = true;
    p.writable = writable;
    p.unsent = true;
    p.since = now;
}

fn tag_spawner_mobs(game: &mut Game<'_>) {
    let ids = std::mem::take(&mut book().untagged);
    for id in ids {
        if let Some(e) = game.sc_entity(id) {
            game.sc_entity_add_tag(e, book::SPAWNER_TAG);
        }
    }
}

/// Each mob of the book whose last damage changed: a player's hit (or a stand-in's).
fn sample_hits(game: &Game<'_>, dim: &str, now: u64) {
    let _ = dim;
    let mut found: Vec<(u32, String, u128)> = Vec::new();
    let mut b = book();
    let Book { keys, ages, players, .. } = &mut *b;
    for m in game.mobs.iter().filter(|m| !m.removed && keys.contains(&*m.kind.id)) {
        let Some((d, age)) = m.last_damage else { continue };
        if ages.insert(m.id, age) == Some(age) {
            continue;
        }
        let who = match d.attacker {
            Some(Who::Player) => game.last_hurt_by_player.get(&m.id).map(|c| c.0),
            Some(Who::Mob(e)) => stand_in_in(players, e),
            None => None,
        };
        if let Some(u) = who {
            found.push((m.id, m.kind.id.to_string(), u));
        }
    }
    for (id, kind, u) in found {
        b.hits.add(id, u, now);
        b.meets.push((kind, u));
    }
    let window = b.file.window_ticks();
    b.hits.prune(now, window);
    if now % 200 == 0 {
        let alive: HashSet<u32> = game.mobs.iter().map(|m| m.id).collect();
        b.ages.retain(|id, _| alive.contains(id));
    }
}

/// Vanilla mobs of the book: their AI target, a player within 16 blocks.
fn vanilla_met(game: &Game<'_>, dim: &str) {
    let players: Vec<(u128, i32, V3)> =
        game.sc_server_players().into_iter().filter(|p| p.dimension == dim).map(|p| (p.uuid, p.entity_id, p.position)).collect();
    let mut b = book();
    let vanilla: HashSet<String> = b.catalog.entries.iter().filter(|(e, _)| e.vanilla).flat_map(|(e, _)| e.keys.iter().cloned()).collect();
    if vanilla.is_empty() {
        return;
    }
    let mut met = Vec::new();
    for m in game.mobs.iter().filter(|m| !m.removed && m.alive() && vanilla.contains(&*m.kind.id)) {
        let at = [m.position.x, m.position.y, m.position.z];
        let near = |u: &(u128, i32, V3)| geom::dist(u.2, at) <= MEET_BLOCKS;
        let who = match m.ai.target {
            Some(Who::Mob(e)) => players.iter().find(|p| p.1 as u32 == e).filter(|p| near(p)).map(|p| p.0).or_else(|| stand_in_in(&b.players, e)),
            Some(Who::Player) => players.iter().filter(|p| near(p)).min_by(|x, y| geom::dist(x.2, at).total_cmp(&geom::dist(y.2, at))).map(|p| p.0),
            None => None,
        };
        if let Some(u) = who {
            met.push((m.kind.id.to_string(), u));
        }
    }
    b.meets.extend(met);
}

/// Where each player (and stand-in) of `dim` stands now.
fn positions(game: &Game<'_>, dim: &str) -> HashMap<u128, V3> {
    let mut at: HashMap<u128, V3> = game.sc_server_players().into_iter().filter(|p| p.dimension == dim).map(|p| (p.uuid, p.position)).collect();
    let stand_ins: Vec<(u128, u32)> = book().players.iter().filter_map(|(u, p)| p.stand_in.map(|e| (*u, e))).collect();
    for (u, e) in stand_ins {
        if let Some(m) = game.mobs.iter().find(|m| m.id == e && !m.removed) {
            at.insert(u, [m.position.x, m.position.y, m.position.z]);
        }
    }
    at
}

fn deaths(game: &mut Game<'_>, dim: &str, now: u64) {
    let fresh: Vec<Death> = std::mem::take(&mut *DEATHS_IN.lock().unwrap_or_else(PoisonError::into_inner));
    if fresh.is_empty() && book().deaths.is_empty() {
        return;
    }
    let at = positions(game, dim);
    let mut b = book();
    // Credited kills of SoulsCraft's own mobs drop shards, horns, heads and armour (Г15, Г18).
    let mut shards: Vec<(String, V3)> = Vec::new();
    for mut d in fresh {
        let Some(entry) = b.catalog.by_key(&d.kind).cloned() else { continue };
        let hits = b.hits.take(d.id);
        d.credited = book::credited(&b.file, d.killer, &hits, now, d.pos, &at);
        let w = book::weight(&b.file, d.spawner);
        let names: Vec<String> = d.credited.iter().map(|u| b.players.get(u).map_or(format!("{u:032x}"), |p| p.name.clone())).collect();
        log::info!("soulscraft: book credit mob={} id={} players={names:?} weight={w} how=kill", entry.id, d.id);
        for u in d.credited.clone() {
            credit(&mut b, u, &entry, w);
        }
        if !d.credited.is_empty() && !d.spawner && d.kind.starts_with("soulscraft:") {
            shards.push((d.kind.clone(), d.pos));
        }
        b.deaths.insert(d.id, d);
    }
    b.deaths.retain(|_, d| now.saturating_sub(d.tick) <= DEATH_KEEP);
    drop(b);
    crate::game::soulscraft_natural::drop_loot(game, dim, &shards);
}

/// One kill for a player of the book (loaded, or a stand-in).
fn credit(b: &mut Book, uuid: u128, entry: &book::Entry, weight: f64) {
    let Book { players, file, lines, .. } = b;
    let Some(p) = players.get_mut(&uuid) else {
        log::warn!("soulscraft: book kill of {} for {uuid:032x}: not loaded yet, not counted", entry.id);
        return;
    };
    let unlock = p.progress.kill(file, entry, weight);
    p.dirty = true;
    p.unsent = true;
    log::info!("soulscraft: book player={} entry={} kills={}", p.name, entry.id, p.progress.mob(&entry.id).kills);
    if let Some(u) = unlock {
        note(p, file, lines, &u);
    }
}

fn note(p: &mut Player, file: &BookFile, lines: &mut Vec<String>, u: &Unlock) {
    let text = file.line(u);
    log::info!("soulscraft: book player={} entry={} stage={}", p.name, u.id, u.stage);
    lines.push(format!("{} {text}", p.name));
    if lines.len() > LINES_KEEP {
        lines.remove(0);
    }
    p.lines.push(text);
    p.dirty = true;
    p.unsent = true;
}

/// RustCraft's machine kills (turret or guard, either report): the owner within 32 blocks of the
/// death gets it, once per death whichever reports come. A report waits `DEATH_KEEP` ticks for its
/// death; a `soulscraft-kill` one whose death was never heard is then credited at its own block.
fn reports(game: &Game<'_>, now: u64) {
    let pending = std::mem::take(&mut book().reports);
    if pending.is_empty() {
        return;
    }
    let owners: HashMap<u128, V3> = game.sc_server_players().into_iter().map(|p| (p.uuid, p.position)).collect();
    let mut b = book();
    let mut keep = Vec::new();
    for (k, first) in pending {
        let first = if first == 0 { now } else { first };
        let Some(entry) = b.catalog.by_key(&k.kind).cloned() else { continue };
        let (died_at, spawner, from_death) = match b.deaths.get(&k.entity) {
            Some(d) if d.credited.contains(&k.owner) => continue,
            Some(d) => (d.pos, d.spawner, true),
            None if now - first < DEATH_KEEP => {
                keep.push((k, first));
                continue;
            }
            None => match k.at {
                Some(c) if !b.machine_credited.contains(&(k.entity, k.owner)) => {
                    ([f64::from(c[0]) + 0.5, f64::from(c[1]), f64::from(c[2]) + 0.5], b.spawner_ids.contains(&k.entity), false)
                }
                _ => continue,
            },
        };
        let owner_at = owners.get(&k.owner).copied().or_else(|| stand_in_pos(game, &b.players, k.owner));
        let near = book::owner_credited(&b.file, owner_at, died_at);
        log::info!(
            "soulscraft: book credit mob={} id={} owner={:032x} near={near} how={}",
            entry.id,
            k.entity,
            k.owner,
            k.machine.name()
        );
        if !near {
            continue;
        }
        let w = book::weight(&b.file, spawner);
        credit(&mut b, k.owner, &entry, w);
        if from_death {
            if let Some(d) = b.deaths.get_mut(&k.entity) {
                d.credited.push(k.owner);
            }
        } else {
            if b.machine_credited.len() > 4096 {
                b.machine_credited.clear();
            }
            b.machine_credited.insert((k.entity, k.owner));
        }
    }
    b.reports.extend(keep);
}

fn stand_in_pos(game: &Game<'_>, players: &HashMap<u128, Player>, uuid: u128) -> Option<V3> {
    let e = players.get(&uuid)?.stand_in?;
    game.mobs.iter().find(|m| m.id == e && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z])
}

/// Items in hand: an altar block or a summon item opens the base, a banner its modifier.
fn held_items(game: &Game<'_>) {
    let summon = altar::summon_items();
    let mut found: Vec<(u128, String)> = Vec::new();
    for p in game.sc_server_players() {
        for slot in [mod_api::EquipmentSlot::MainHand, mod_api::EquipmentSlot::OffHand] {
            if let Some(s) = game.sc_player_equipment(p.uuid, slot) {
                found.push((p.uuid, s.item.to_string()));
            }
        }
    }
    let mut b = book();
    for (uuid, item) in found {
        let Some(h) = book::held(&item, &b.altar, &summon) else { continue };
        open_altar(&mut b, uuid, |p, file, altar| match &h {
            Held::Base => p.altar_base(file).into_iter().collect(),
            Held::Banner(id) => p.banner(altar, id).into_iter().collect(),
        });
    }
}

/// Opens altar or event entries for a loaded player (or stand-in) with `f`.
fn open_altar(b: &mut Book, uuid: u128, f: impl FnOnce(&mut Progress, &BookFile, &AltarFile) -> Vec<Unlock>) {
    let Book { players, file, altar, lines, .. } = b;
    let Some(p) = players.get_mut(&uuid).filter(|p| p.loaded) else { return };
    for u in f(&mut p.progress, file, altar) {
        note(p, file, lines, &u);
    }
}

/// An altar whose level rose (or a young one first seen): players near it get its levels.
fn altar_levels(game: &Game<'_>, dim: &str, now: u64) {
    let altars = altar::altars_in(dim);
    let at = positions(game, dim);
    let mut b = book();
    for a in altars {
        let key = (dim.to_owned(), a.core);
        let before = match b.levels.get(&key) {
            Some(l) => *l,
            None if (now as i64) - a.since <= YOUNG_ALTAR => 0,
            None => a.level,
        };
        b.levels.insert(key, a.level);
        if a.level <= before {
            continue;
        }
        let centre = [f64::from(a.core[0]) + 0.5, f64::from(a.core[1]), f64::from(a.core[2]) + 0.5];
        let near: Vec<u128> = at.iter().filter(|(_, p)| geom::dist(**p, centre) <= ALTAR_NEAR).map(|(u, _)| *u).collect();
        log::info!("soulscraft: book altar core={:?} level={} near={}", a.core, a.level, near.len());
        for u in near {
            open_altar(&mut b, u, |p, _, _| p.altar_levels(a.level));
        }
    }
}

/// Runs in progress (their participants tried the event) and runs that ended.
fn events(dim: &str) {
    let running: Vec<(String, Vec<u128>, Vec<u32>)> = crate::game::soulscraft_event::events()
        .values()
        .filter(|r| r.dim == dim)
        .map(|r| (r.event.clone(), r.joined.keys().copied().collect(), r.stand_ins.clone()))
        .collect();
    let mut b = book();
    let ended = std::mem::take(&mut b.ended);
    for (event, players, stand_ins, won) in running.into_iter().map(|(e, p, s)| (e, p, s, false)).chain(ended) {
        let mut who = players;
        who.extend(stand_ins.iter().filter_map(|e| stand_in_in(&b.players, *e)));
        let catalog = b.catalog.clone();
        for u in who {
            open_altar(&mut b, u, |p, file, _| if won { p.event_won(file, &catalog, &event) } else { p.event_tried(file, &catalog, &event) });
        }
    }
}

/// Meetings, action-bar lines, the persistent data written, the clients told.
fn flush(game: &mut Game<'_>, now: u64) {
    let (lines, writes, sends) = {
        let mut b = book();
        let meets = std::mem::take(&mut b.meets);
        for (kind, uuid) in meets {
            let Some(entry) = b.catalog.by_key(&kind).cloned() else { continue };
            let Book { players, file, lines, .. } = &mut *b;
            let Some(p) = players.get_mut(&uuid).filter(|p| p.loaded) else { continue };
            if let Some(u) = p.progress.meet(file, &entry) {
                note(p, file, lines, &u);
            }
        }
        // The book's right clicks (lib.rs `HunterBook`, queued in the bridge) and `/sc book … open`.
        let opens: HashSet<u128> = std::mem::take(&mut b.opens).into_iter().chain(soulscraft_rules::bridge::take_book_opens()).collect();
        let mut lines = Vec::new();
        let mut writes = Vec::new();
        let mut sends = Vec::new();
        for (uuid, p) in b.players.iter_mut() {
            if p.stand_in.is_some() {
                p.lines.clear();
                p.dirty = false;
                p.unsent = false;
                continue;
            }
            if !p.lines.is_empty() {
                lines.push((*uuid, std::mem::take(&mut p.lines).join(" · ")));
            }
            if p.dirty && p.writable {
                writes.push((*uuid, p.progress.to_json()));
            }
            let open = opens.contains(uuid);
            if (p.unsent || open) && p.loaded {
                p.unsent = false;
                sends.push((*uuid, BookMsg { open, progress: p.progress.clone() }));
            }
        }
        (lines, writes, sends)
    };
    for (uuid, line) in lines {
        game.sc_action_bar(uuid, Text::literal(line));
    }
    for (uuid, json) in writes {
        if write(game, uuid, &json) {
            if let Some(p) = book().players.get_mut(&uuid) {
                p.dirty = false;
            }
        }
    }
    for (uuid, msg) in sends {
        if let Err(e) = game.sc_net_send_to_player(uuid, book::CHANNEL, &book::encode(&msg)) {
            log::warn!("soulscraft: book message to {uuid:032x} not sent: {e}");
        } else if msg.open {
            log::info!("soulscraft: book open player={uuid:032x} tick={now}");
        }
    }
}

/// The book's key written into the player's shared compound; the other mods' keys stay.
fn write(game: &mut Game<'_>, uuid: u128, json: &str) -> bool {
    let Some(mut data) = game.sc_player_persistent_data(uuid) else { return false };
    if !put(&mut data, json) {
        return false;
    }
    game.sc_set_player_persistent_data(uuid, data)
}

/// `json` under the book's key of a persistent-data compound (false for a tag that is not one).
pub(super) fn put(data: &mut Tag, json: &str) -> bool {
    let Tag::Compound(map) = data else { return false };
    map.insert(book::KEY.into(), Tag::String(json.into()));
    true
}

/// The book's progress read from a persistent-data compound.
pub(super) fn get(data: &Tag) -> Option<Result<Progress, String>> {
    let Tag::Compound(map) = data else { return None };
    match map.get(&horizon_core::nbt::JavaString::from(book::KEY))? {
        Tag::String(s) => Some(Progress::from_json(&s.to_string_lossy())),
        _ => Some(Err("not a string".into())),
    }
}

// ---- Stand-ins (the self-test has no players) ----

fn stand_in_in(players: &HashMap<u128, Player>, entity: u32) -> Option<u128> {
    players.iter().find(|(_, p)| p.stand_in == Some(entity)).map(|(u, _)| *u)
}

fn stand_in_of(entity: u32) -> Option<u128> {
    stand_in_in(&book().players, entity)
}

/// A mob (an iron golem) that stands in for a player `uuid` named `name`: its hits, its targets
/// and where it stands count as that player's; its book lives in memory only.
pub(super) fn add_stand_in(uuid: u128, name: &str, entity: u32) {
    book().players.insert(
        uuid,
        Player {
            name: name.to_owned(),
            progress: Progress::default(),
            loaded: true,
            writable: false,
            dirty: false,
            unsent: false,
            lines: Vec::new(),
            since: 0,
            resent: true,
            stand_in: Some(entity),
        },
    );
}

pub(super) fn clear_stand_ins() {
    let mut b = book();
    b.players.retain(|_, p| p.stand_in.is_none());
    b.lines.clear();
    b.levels.clear();
    b.deaths.clear();
    b.reports.clear();
    b.machine_credited.clear();
    b.meets.clear();
    b.ended.clear();
}

pub(super) fn progress_of(uuid: u128) -> Option<Progress> {
    book().players.get(&uuid).map(|p| p.progress.clone())
}

/// A player's kills on the book entry of `kind` (`soulscraft:brute`), while they are online and
/// their book is read: a diorama's plaque (trophy.rs).
pub(super) fn kills_of(uuid: u128, kind: &str) -> Option<f64> {
    let b = book();
    let p = b.players.get(&uuid).filter(|p| p.loaded)?;
    let entry = b.catalog.by_key(kind)?;
    Some(p.progress.mob(&entry.id).kills)
}

/// Machine kill reports heard so far: (`pct-machine kill`, `soulscraft-kill`).
pub(super) fn reports_heard() -> (u32, u32) {
    let h = book().heard;
    (h[0], h[1])
}


/// The unlock lines so far (`<name> <line>`).
pub(super) fn lines() -> Vec<String> {
    book().lines.clone()
}

/// Marks a mob as a spawner's, as `add_planned` would.
pub(super) fn mark_spawner(id: u32) {
    on_spawner_planned(&HashMap::from([(u32::MAX, id)]));
}

/// `/sc book <player>`: their progress as one line, and a sync sent.
pub(super) fn describe(name: &str) -> String {
    let mut b = book();
    let file = b.file.clone();
    let catalog = b.catalog.clone();
    let Some((_, p)) = b.players.iter_mut().find(|(_, p)| p.name == name) else { return format!("{name}: no book loaded") };
    p.unsent = true;
    let stages: Vec<String> = catalog
        .entries
        .iter()
        .map(|(e, _)| format!("{} {} ({})", e.id, file.stage_name(p.progress.stage(&file, e)), p.progress.mob(&e.id).kills))
        .collect();
    format!(
        "{name}: {}; altar base {} levels {:?} banners {:?} defense {}; events {:?}",
        stages.join(", "),
        p.progress.altar.base,
        p.progress.altar.levels,
        p.progress.altar.banners,
        p.progress.altar.defense,
        p.progress.events
    )
}

/// A player's book from the start (`/sc book <player> reset`), written at once.
pub(super) fn reset(name: &str) -> String {
    let mut b = book();
    let Some((_, p)) = b.players.iter_mut().find(|(_, p)| p.name == name && p.loaded) else { return format!("{name}: no book loaded") };
    p.progress = Progress::default();
    p.dirty = true;
    p.unsent = true;
    format!("{name}: book reset")
}

// ---- `/sc book` (operators: checks on a live server, the bots' round) ----

// The usages (`sc book <player:word> [action:text]` and the machine kills' level-4 command) are
// registered by lib.rs's `register_commands`.

/// `/sc book <player> [reset | open | meet <mob> | kill <mob> [n]]`: the player's book, a reset,
/// opening their window, or fixture meetings and kills (credited as real ones, action bar
/// included). True when the line was this command (the `/sc` parser is not asked then).
pub(super) fn on_command(game: &mut Game<'_>, line: &str, ran: &mut bool) -> bool {
    let w: Vec<&str> = line.trim_start_matches('/').split_whitespace().collect();
    if w.len() < 3 || w[0] != "sc" || w[1] != "book" {
        return false;
    }
    *ran = true;
    let name = w[2];
    let reply = match w.get(3..).unwrap_or_default() {
        [] => describe(name),
        ["reset"] => reset(name),
        ["open"] => match uuid_of(name) {
            Some(u) => {
                book().opens.push(u);
                format!("{name}: the book opens")
            }
            None => format!("{name}: no book loaded"),
        },
        ["meet", mob] => fixture(name, mob, None),
        ["kill", mob] => fixture(name, mob, Some(1.0)),
        ["kill", mob, n] => match n.parse::<f64>() {
            Ok(n) if n > 0.0 && n <= 100.0 => fixture(name, mob, Some(n)),
            _ => format!("kills {n}: a number from 0 to 100"),
        },
        _ => "sc book <player> [reset | open | meet <mob> | kill <mob> [n]]".to_owned(),
    };
    log::info!("soulscraft: {reply}");
    game.sc_feedback(Text::literal(format!("[SoulsCraft] {reply}")));
    true
}

fn uuid_of(name: &str) -> Option<u128> {
    book().players.iter().find(|(_, p)| p.name == name && p.loaded).map(|(u, _)| *u)
}

fn fixture(name: &str, mob: &str, kills: Option<f64>) -> String {
    let Some(uuid) = uuid_of(name) else { return format!("{name}: no book loaded") };
    let mut b = book();
    let Some((entry, _)) = b.catalog.get(mob).cloned() else { return format!("no entry {mob}") };
    match kills {
        Some(n) => credit(&mut b, uuid, &entry, n),
        None => b.meets.push((entry.keys[0].clone(), uuid)),
    }
    format!("{name}: {mob} {}", kills.map_or("met".to_owned(), |n| format!("+{n} kills")))
}

/// A vanilla type's own numbers as the engine has them (its `MobType`: max health, attack damage,
/// armour), for the book's pages of changed vanilla mobs (soulscraft_rules::book_page::VanillaStats)
/// on the client, and for the self-test on the server.
pub(super) fn engine_stats(game: &Game<'_>, kind: &str) -> Option<soulscraft_rules::data::Stats> {
    let t = game.res.mob_types().get(kind)?;
    Some(soulscraft_rules::data::Stats {
        health: f64::from(t.max_health),
        damage: f64::from(t.attack_damage),
        armor: f64::from(t.armor),
        ..Default::default()
    })
}
