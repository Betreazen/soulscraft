// The altar's spawn boost on the server (06-altar.md, the altar's second role; prototype П4;
// prototypes/p4-spawn.md; the rules are soulscraft::rules::boost, numbers data/altar.json
// `spawn_boost`). No trait mod: three injections on natural spawning reach it from the bundle.
//
// - `spawning_chunks` (RETURN; game/natural_spawning.rs:739-748, `ChunkMap.collectSpawningChunks`
//   for `spawn_natural`, :764): each candidate chunk an active sector holds is appended again
//   `passes` times. `spawn_natural` then shuffles and drains the list (:814-842), so a sector
//   chunk gets `1 + passes` spawn passes a tick under vanilla's own rules, light and caps. The
//   global cap is untouched: the union's size (the return value) is the same. Side effect: the
//   chunk's inhabited time grows `1 + passes` times as fast (:766-768), so its local difficulty
//   rises sooner.
// - `spawn_hooks` (`!mods().is_empty()`, game/mobs/natural_spawning.rs:882): SoulsCraft has a
//   Hooks impl on the Mod API, so natural spawning asks `native_potential_spawns` everywhere and
//   the bundle era's RETURN on it is gone.
// - `potential_spawns` (HEAD, catalog hook v1, API 388; native_hooks.rs:6335-6356): a monster
//   position in a sector gets the other biomes' monsters and SoulsCraft's land mobs, the kin
//   banners' kin weighted up (soulscraft::rules::boost::extend).
//   The tier gate of natural spawns (soulscraft_natural, `tier_p` against the players' P) does
//   not apply here: in a sector the altar calls its mobs, and an event's power is its group's.
//
// The sectors are worked out in the level's tick from the known altars (soulscraft_altar: level,
// banners, the menu's switch, `since`) into a table the injections read; each takes only its own
// lock.
//
// Log lines:
//   soulscraft: boost core=-700,101,700 level=2 passes=1 centre=-38,49 age=1200 kin=[]   (on change)
//   soulscraft: boost spawn passes=+18 lists=40 tick=..   (every 200 ticks while a sector is active)
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use horizon_client::biome::SpawnEntry;
use soulscraft::rules::altar as ra;
use soulscraft::rules::boost::{self, Boost, BoostDef, Entry};
use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

const LOG_EVERY: u64 = 200;

#[derive(Default)]
struct Table {
    /// Active sectors by dimension.
    sectors: HashMap<String, Vec<Boost>>,
    /// What each core's boost was last tick (logged on change).
    last: HashMap<(String, [i32; 3]), Boost>,
    /// Counters since the last log line, and in total (the self-test).
    passes: u64,
    lists: u64,
    /// Time our two spawning injections took since the last log line (the П4 cost).
    nanos: u64,
    total_passes: u64,
    total_lists: u64,
}

static TABLE: LazyLock<Mutex<Table>> = LazyLock::new(Mutex::default);
static DEF: LazyLock<BoostDef> = LazyLock::new(BoostDef::embedded);
/// The mobs' kin and keys, as built (a spawn list is asked often: no copy of the live data each time).
static MOBS: LazyLock<soulscraft::rules::data::MobsFile> = LazyLock::new(soulscraft::rules::data::MobsFile::embedded);

fn table() -> MutexGuard<'static, Table> {
    TABLE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The level's tick: the dimension's sectors from its altars.
pub(super) fn on_level_tick(dim: &str, now: u64) {
    let mut active = Vec::new();
    let mut logs = Vec::new();
    let altars = crate::game::soulscraft_altar::altars_in(dim);
    let file = crate::game::soulscraft_altar::file();
    let mut t = table();
    for a in &altars {
        let effects = ra::effects(&file, &a.banner_ids());
        let b = boost::boost(&DEF, a.core, a.level, a.boost, crate::game::soulscraft_altar::age(a, now), &effects);
        let key = (dim.to_owned(), a.core);
        match &b {
            Some(b) if t.last.get(&key) != Some(b) => {
                logs.push(format!(
                    "core={},{},{} level={} passes={} centre={},{} age={} kin={:?}",
                    a.core[0],
                    a.core[1],
                    a.core[2],
                    a.level,
                    b.passes,
                    b.centre.0,
                    b.centre.1,
                    crate::game::soulscraft_altar::age(a, now),
                    b.kin
                ));
                t.last.insert(key, b.clone());
            }
            None => {
                t.last.remove(&key);
            }
            _ => {}
        }
        if let Some(b) = b {
            active.push(b);
        }
    }
    t.last.retain(|(d, core), _| d != dim || altars.iter().any(|a| a.core == *core));
    let any = !active.is_empty();
    t.sectors.insert(dim.to_owned(), active);
    if any && now % LOG_EVERY == 0 {
        logs.push(format!("spawn passes=+{} lists={} us={:.1} tick={now}", t.passes, t.lists, t.nanos as f64 / 1000.0));
        t.passes = 0;
        t.lists = 0;
        t.nanos = 0;
    }
    drop(t);
    for l in logs {
        log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: boost {l}");
    }
}

/// The active sectors of a dimension now.
pub(super) fn sectors(dim: &str) -> Vec<Boost> {
    table().sectors.get(dim).cloned().unwrap_or_default()
}

/// Passes and lists added since the server started (the self-test reads the difference).
pub(super) fn totals() -> (u64, u64) {
    let t = table();
    (t.total_passes, t.total_lists)
}

/// `spawning_chunks` RETURN: the sectors' chunks among the candidates come again.
pub(super) fn on_spawning_chunks(game: &Game<'_>, candidates: &mut Vec<(i32, i32)>) {
    let began = std::time::Instant::now();
    let dim = game.dim.id();
    let extra = {
        let t = table();
        let Some(list) = t.sectors.get(dim).filter(|l| !l.is_empty()) else { return };
        boost::extra_candidates(candidates, list)
    };
    if extra.is_empty() {
        return;
    }
    let n = extra.len() as u64;
    candidates.extend(extra);
    let mut t = table();
    t.passes += n;
    t.total_passes += n;
    t.nanos += began.elapsed().as_nanos() as u64;
}

/// The dimension's extra candidates for these chunks (the self-test asks without a player).
pub(super) fn extra_for(dim: &str, candidates: &[(i32, i32)]) -> Vec<(i32, i32)> {
    boost::extra_candidates(candidates, &sectors(dim))
}

/// `potential_spawns` HEAD: a monster position in a sector gets its extra entries.
pub(super) fn on_potential_spawns(dimension: &str, category: &str, pos: [i32; 3], entries: &mut Vec<SpawnEntry>) {
    if category != "monster" {
        return;
    }
    let began = std::time::Instant::now();
    let kin: Option<BTreeMap<String, u32>> = {
        let t = table();
        let Some(list) = t.sectors.get(dimension) else { return };
        let mut hit = list.iter().filter(|b| b.holds_block(pos[0], pos[2])).peekable();
        if hit.peek().is_none() {
            None
        } else {
            let mut kin = BTreeMap::new();
            for b in hit {
                for (k, n) in &b.kin {
                    *kin.entry(k.clone()).or_insert(0) += n;
                }
            }
            Some(kin)
        }
    };
    let Some(kin) = kin else { return };
    let mobs = &*MOBS;
    let mut list: Vec<Entry> = entries.iter().map(|e| Entry { kind: e.entity.clone(), weight: e.weight, min: e.min, max: e.max }).collect();
    boost::extend(&mut list, &DEF, mobs, &kin);
    for (i, e) in list.into_iter().enumerate() {
        match entries.get_mut(i) {
            Some(old) => old.weight = e.weight,
            None => entries.push(SpawnEntry { entity: e.kind, weight: e.weight, min: e.min, max: e.max, constant: e.min == e.max }),
        }
    }
    let mut t = table();
    t.lists += 1;
    t.total_lists += 1;
    t.nanos += began.elapsed().as_nanos() as u64;
}

/// The self-test's call of the engine's own hook point with a list, as natural spawning asks it.
pub(super) fn ask_engine(game: &Game<'_>, dim: &str, pos: [i32; 3], entries: &mut Vec<SpawnEntry>) {
    let mut canceled = false;
    game.sc_potential_spawns(dim, "monster", pos, entries, &mut canceled);
}
