// The survival slice in the world (docs/design/10-survival.md; soulscraft::rules::natural):
// SoulsCraft's mobs in the overworld's natural spawn lists by biome (hypothesis Г14), the lurker's
// own spawner in deep ocean water, and the soul shard a credited kill drops (Г15).
//
// - Spawn lists: `native_potential_spawns` (API 388, native_hooks.rs:6335-6356, asked by natural
//   spawning for each candidate position, game/natural_spawning.rs:458-470). The injection binds
//   the game (`&*self`) for `native_biome` (native_hooks.rs:11132, the zoomed biome
//   `Level.getBiome` picks). The engine then places the mob by its template's rules (zombie: solid
//   floor, dark enough; mobs/spawn_rules.rs) and counts it in the monster cap.
// - The lurker: zombie placement never puts a mob in water, so every `every_ticks` each player in
//   a deep ocean may get one lurker, with `chance`, in water below `max_y` between the radii,
//   if fewer than `near_cap` lurkers are near.
// - The tier gate (10-survival.md; data/mobs.json `natural.tier_gate_blocks`): a mob comes by
//   itself only where a player within the gate has P of its `tier_p` or more (`/sc p` stands in
//   for everyone's P). The spawn list hook has only `&Game` and P needs `&mut Game` (gear), so
//   each player's P is read on the level tick every `TIER_REFRESH` ticks and kept here. The
//   altar's spawn boost (soulscraft_spawn_boost, after this hook) and its events never ask it.
// - World drops (Г15, Г18; docs/design/11-summons-rewards.md): the book's deaths
//   (soulscraft_book::deaths) hand over each credited kill of a SoulsCraft mob; its shards, horn,
//   head and armour are summoned here, outside the book's lock.
//
// Log lines:
//   soulscraft: natural lurker at=x,y,z near=player tick=..
//   soulscraft: shard drop mob=revenant count=1 at=x,y,z
//   soulscraft: world drop mob=revenant item=soulscraft:grave_horn at=x,y,z
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use horizon_client::biome::SpawnEntry;
use soulscraft::rules::data::MobsFile;
use soulscraft::rules::event::Rng;
use std::sync::{LazyLock, Mutex, PoisonError};

/// The data as built: spawn lists are asked often, no copy of the live data each time.
static MOBS: LazyLock<MobsFile> = LazyLock::new(MobsFile::embedded);
/// The head's world chances (data/trophies.json).
static CHANCES: LazyLock<soulscraft::rules::event::TrophyChances> = LazyLock::new(soulscraft::rules::event::TrophyChances::embedded);
static RNG: LazyLock<Mutex<Rng>> = LazyLock::new(|| Mutex::new(Rng::new(0x5EED_50_11)));
/// Lists extended since the last read (the self-test and the cost line).
static EXTENDED: Mutex<u64> = Mutex::new(0);
/// The overworld's players (where, P) and the `/sc p` override, for the tier gate.
static POWERS: Mutex<(Vec<([f64; 3], f64)>, Option<f64>)> = Mutex::new((Vec::new(), None));
/// The players' P for the tier gate is read again this often.
const TIER_REFRESH: u64 = 20;

fn roll() -> f64 {
    RNG.lock().unwrap_or_else(PoisonError::into_inner).unit()
}

/// `native_potential_spawns`: the overworld's monster lists get SoulsCraft's mobs of the biome.
pub(super) fn on_potential_spawns(game: &Game<'_>, dimension: &str, category: &str, pos: [i32; 3], entries: &mut Vec<SpawnEntry>) {
    if category != "monster" || dimension != "minecraft:overworld" {
        return;
    }
    let Some(biome) = game.sc_biome(pos) else { return };
    let listed: Vec<&str> = entries.iter().map(|e| e.entity.as_str()).collect();
    let best = best_power_at([f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5]);
    let extras = soulscraft::rules::natural::extras(&MOBS.natural, &MOBS, &biome, pos[1], &listed, best);
    if extras.is_empty() {
        return;
    }
    for e in extras {
        entries.push(SpawnEntry { entity: e.key, weight: e.weight, min: e.min, max: e.max, constant: e.min == e.max });
    }
    *EXTENDED.lock().unwrap_or_else(PoisonError::into_inner) += 1;
}

/// The P the tier gate reads at `at`: the `/sc p` override, else the strongest player within
/// `natural.tier_gate_blocks` (None: nobody that near).
pub(super) fn best_power_at(at: [f64; 3]) -> Option<f64> {
    let p = POWERS.lock().unwrap_or_else(PoisonError::into_inner);
    p.1.or_else(|| soulscraft::rules::scale::best_power_near(at, &p.0, MOBS.natural.tier_gate_blocks))
}

/// Reads the overworld players' P and the override for the tier gate now (the level tick, and the
/// self-test after it changes `/sc p`).
pub(super) fn refresh_powers(game: &mut Game<'_>, dim: &str) {
    let read = crate::game::soulscraft_power::powers_in(game, dim);
    *POWERS.lock().unwrap_or_else(PoisonError::into_inner) = read;
}

/// Lists extended so far, and the counter cleared (the self-test).
pub(super) fn take_extended() -> u64 {
    std::mem::take(&mut *EXTENDED.lock().unwrap_or_else(PoisonError::into_inner))
}

/// The level's tick: the lurker's spawner.
pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    if dim != "minecraft:overworld" {
        return;
    }
    if now % TIER_REFRESH == 0 {
        refresh_powers(game, dim);
    }
    let l = &MOBS.natural.lurker;
    if l.every_ticks == 0 || now % l.every_ticks != 0 {
        return;
    }
    let Some(def) = MOBS.mob(&l.mob) else { return };
    let players = game.sc_server_players();
    for p in players.iter().filter(|p| p.dimension == dim && p.health > 0.0 && p.game_mode != 3) {
        if !l.due(now, roll()) {
            continue;
        }
        let at = p.position;
        let Some(biome) = game.sc_biome([at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32]) else { continue };
        if !l.biome_ok(&biome) {
            continue;
        }
        let [x, z] = l.spot(at, [roll(), roll()]);
        let Some(y) = water_column(game, x.floor() as i32, z.floor() as i32, l.max_y) else { continue };
        let spot = [x.floor() + 0.5, f64::from(y), z.floor() + 0.5];
        if !MOBS.natural.lurker_allowed(&MOBS, best_power_at(spot)) {
            continue;
        }
        let near = game.mobs.iter().filter(|m| !m.removed && m.alive() && m.kind.id == def.key && soulscraft::rules::geom::dist([m.position.x, m.position.y, m.position.z], spot) <= l.near_blocks).count();
        if near >= l.near_cap {
            continue;
        }
        // No P tag: a natural mob takes the power of the players near it (Г5), as the others do.
        let Ok((horizon_core::nbt::Tag::Compound(nbt), _)) = horizon_core::nbt::parse_snbt("{}") else { continue };
        if game.sc_summon(dim, &def.key, spot, nbt).is_some() {
            log::info!("soulscraft: natural lurker at={:.0},{y},{:.0} near={} tick={now}", spot[0], spot[2], p.name);
        }
    }
}

/// The highest cell at or below `max_y` with water in it and in the cell above (the lurker's box
/// is under 2 blocks tall); None where the column is not loaded or holds no such water.
fn water_column(game: &Game<'_>, x: i32, z: i32, max_y: i32) -> Option<i32> {
    let water = |y: i32| game.sc_block([x, y, z]) == "minecraft:water";
    (-60..=max_y).rev().find(|&y| water(y) && water(y + 1))
}

/// What credited kills drop in the world, from the book's deaths: (mob key, where). Shards, now
/// and then a horn, the head, armour pieces (soulscraft::rules::natural::world_drops).
pub(super) fn drop_loot(game: &mut Game<'_>, dim: &str, kills: &[(String, [f64; 3])]) {
    for (kind, at) in kills {
        let Some(def) = MOBS.mob(kind) else { continue };
        for (item, n) in soulscraft::rules::natural::world_drops(&MOBS, &CHANCES, def, roll) {
            let snbt = format!("{{Item:{{id:\"{item}\",count:{n}}},PickupDelay:10}}");
            let Ok((horizon_core::nbt::Tag::Compound(nbt), _)) = horizon_core::nbt::parse_snbt(&snbt) else { continue };
            game.sc_summon(dim, "minecraft:item", [at[0], at[1] + 0.3, at[2]], nbt);
            if item == MOBS.soul_shard.item {
                log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: shard drop mob={} count={n} at={:.0},{:.0},{:.0}", def.id, at[0], at[1], at[2]);
            } else {
                log::info!("soulscraft: world drop mob={} item={item} at={:.0},{:.0},{:.0}", def.id, at[0], at[1], at[2]);
            }
        }
    }
}
