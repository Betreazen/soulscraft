// Trophies on the server (docs/design/09-trophies.md; soulscraft_rules::trophy): the block
// entities of heads and dioramas (so clients draw them), the state a placed head or diorama takes,
// a head put on a wall, and the dioramas' particles.
//
// - Block entities: every `<mob>_head_stand`, `<mob>_head_mount` and `<mob>_trophy` has a declared
//   type of its own (lib.rs); only dioramas tick, and their tick (lib.rs's BlockEntityPart) only
//   notes where they are, in `soulscraft_rules::bridge`. Their update tag (owner, kills) is lib.rs's too.
// - Placement: lib.rs's `Placement` BlockPart: a floor head turns to the placer in 16 steps
//   (vanilla's `SkullBlock`), a diorama turns its plaque to the placer
//   (`getHorizontalDirection().getOpposite()`).
// - A wall head (`native_right_click_block`, API 21, with the clicked face): a head item used on
//   the side of a block is put up as `<mob>_wall_head` facing out of the wall, in the level's
//   tick; the click is taken, so no floor head goes down there. One head comes off the stack
//   (none in creative).
// - Particles: each tick each diorama sends, for each of its file's rows, with chance `1 / every`,
//   one particle of the mod's own type (`ParticleOptions::named`, API 103) at a random point of
//   the row's box turned by the block's facing: `count` 0, `speed` 0, so the type's
//   `horizon:behaviour` moves it. `GameLevel::native_send_particles` (native_hooks.rs:9928-9963,
//   the level side of the stable `LevelWriteHost::send_particles`) queues them; players within
//   32 blocks get them (lan_particles.rs:668-676).
//
// Log lines:
//   soulscraft: trophy wall head=soulscraft:revenant_wall_head at=x,y,z facing=east by=sc_a
//   soulscraft: trophy particles dioramas=3 sent=12 tick=..   (every 200 ticks while any ticks)
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use horizon_client::particles::ParticleOptions;
use horizon_core::nbt::{JavaString, Tag};
use soulscraft_rules::event::Rng;
use soulscraft_rules::trophy::{self, Kind, TrophyFile};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

const LOG_EVERY: u64 = 200;
/// Level ticks a diorama click waits for its block to appear.
const PLACER_TTL: u32 = 20;
/// How often a diorama's owner's kills are read again from their book (while they are online).
const KILLS_EVERY: u64 = 40;

/// A player who right-clicked with a diorama: the block is looked for in `cells`.
struct Placer {
    dim: String,
    cells: [[i32; 3]; 2],
    uuid: u128,
    name: String,
    ttl: u32,
}

#[derive(Default)]
struct Hub {
    placers: Vec<Placer>,
    files: HashMap<String, Option<TrophyFile>>,
    /// Wall heads to put up in the level's tick: (dimension, cell, state).
    walls: Vec<(String, [i32; 3], String)>,
    rng: Option<Rng>,
    /// Particles sent per diorama cell since the counter was last read (the self-test).
    sent: HashMap<[i32; 3], u64>,
    sent_since_log: u64,
}

static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(Mutex::default);

fn hub() -> MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_right_click_block` with the clicked face: a head item on the side of a block goes up
/// as a wall head (in the level's tick). The player's stack changes now.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let Some((item, _, _)) = game.sc_held() else { return };
    if let Some(Kind::Diorama(_)) = Kind::of(&item) {
        // Vanilla places it after this hook: the level's tick finds it and names its owner.
        if let Some(player) = game.sc_acting_player() {
            note_placer(&player.dimension, pos, face, player.uuid, &player.name);
        }
        return;
    }
    let Some(mob) = trophy::head_item_mob(&item) else { return };
    let Some(facing) = trophy::wall_facing(face) else { return };
    let Some(player) = game.sc_acting_player() else { return };
    let cell = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
    let id = game.world.block(cell[0], cell[1], cell[2]);
    if !game.res.facts().get(id).replaceable {
        return;
    }
    *canceled = true;
    if player.game_mode != 1 && game.sc_take_items(&item, 1) != 1 {
        return;
    }
    let state = format!("{}[facing={facing}]", Kind::WallHead(mob).block());
    log::info!("soulscraft: trophy wall head={state} at={},{},{} by={}", cell[0], cell[1], cell[2], player.name);
    hub().walls.push((player.dimension.clone(), cell, state));
}

/// Puts a wall head up as a player's click would (the self-test's way, no player).
pub(super) fn put_wall_head(dim: &str, cell: [i32; 3], mob: &str, facing: &str) {
    hub().walls.push((dim.to_owned(), cell, format!("{}[facing={facing}]", Kind::WallHead(mob).block())));
}

/// The level's tick: wall heads go up, dioramas send their particles.
pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    let walls: Vec<(String, [i32; 3], String)> = {
        let mut h = hub();
        let (here, rest) = h.walls.drain(..).partition(|w| w.0 == dim);
        h.walls = rest;
        here
    };
    for (_, cell, state) in walls {
        crate::game::soulscraft_altar::setblock(game, dim, cell, &state, "keep");
    }
    claim_placed(game, dim);
    let ticked: Vec<([i32; 3], String)> = soulscraft_rules::bridge::take_dioramas_ticked();
    if ticked.is_empty() {
        return;
    }
    if now % KILLS_EVERY == 0 {
        for (pos, block) in &ticked {
            refresh_kills(game, *pos, block);
        }
    }
    let mut sent = 0;
    for (pos, block) in &ticked {
        sent += emit(game, *pos, block);
    }
    let mut h = hub();
    h.sent_since_log += sent;
    if now % LOG_EVERY == 0 {
        log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: trophy particles dioramas={} sent={} tick={now}", ticked.len(), h.sent_since_log);
        h.sent_since_log = 0;
    }
}

/// A click with a diorama on `hit`'s `face` by a player: the level's tick names them the owner of
/// the diorama that appears there (the self-test calls it as the click hook does).
pub(super) fn note_placer(dim: &str, hit: [i32; 3], face: [i32; 3], uuid: u128, name: &str) {
    hub().placers.push(Placer { dim: dim.to_owned(), cells: trophy::placement_cells(hit, face), uuid, name: name.to_owned(), ttl: PLACER_TTL });
}

/// Dioramas placed since a click: the new block in one of the clicked cells, with no owner yet,
/// takes the clicking player as its owner.
fn claim_placed(game: &mut Game<'_>, dim: &str) {
    let placers: Vec<Placer> = {
        let mut h = hub();
        let (here, rest) = h.placers.drain(..).partition(|p| p.dim == dim);
        h.placers = rest;
        here
    };
    for mut p in placers {
        let found = p.cells.iter().copied().find(|c| matches!(Kind::of(&game.sc_block(*c)), Some(Kind::Diorama(_))) && owner_uuid(game, *c).is_none());
        if let Some(cell) = found {
            set_owner(game, cell, p.uuid, &p.name);
            continue;
        }
        p.ttl = p.ttl.saturating_sub(1);
        if p.ttl > 0 {
            hub().placers.push(p);
        }
    }
}

/// Names a diorama's owner and writes their kills (the self-test's way too).
pub(super) fn set_owner(game: &mut Game<'_>, pos: [i32; 3], uuid: u128, name: &str) {
    let block = game.sc_block(pos);
    let Some(Kind::Diorama(mob)) = Kind::of(&block) else { return };
    let mob = mob.to_owned();
    let kills = crate::game::soulscraft_book::kills_of(uuid, &format!("soulscraft:{mob}")).unwrap_or(0.0);
    game.sc_with_mod_block_entity(pos, |e| {
        e.nbt.insert(JavaString::from(trophy::OWNER_KEY), Tag::String(JavaString::from(name)));
        e.nbt.insert(JavaString::from(trophy::OWNER_UUID_KEY), Tag::String(JavaString::from(format!("{uuid:032x}").as_str())));
        e.nbt.insert(JavaString::from(trophy::KILLS_KEY), Tag::Double(kills.to_bits()));
    });
    game.sc_block_entity_sync(pos);
    log::info!("soulscraft: trophy owner diorama={mob} at={},{},{} owner={name} kills={kills}", pos[0], pos[1], pos[2]);
}

/// The owner's UUID written in a diorama's block entity.
pub(super) fn owner_uuid(game: &Game<'_>, pos: [i32; 3]) -> Option<u128> {
    match game.sc_mod_block_entity(pos)?.nbt.get(&JavaString::from(trophy::OWNER_UUID_KEY)) {
        Some(Tag::String(s)) => u128::from_str_radix(&s.to_string_lossy(), 16).ok(),
        _ => None,
    }
}

/// The kills written in a diorama's block entity.
pub(super) fn stored_kills(game: &Game<'_>, pos: [i32; 3]) -> Option<f64> {
    match game.sc_mod_block_entity(pos)?.nbt.get(&JavaString::from(trophy::KILLS_KEY)) {
        Some(Tag::Double(k)) => Some(f64::from_bits(*k)),
        _ => None,
    }
}

/// While its owner is online, a diorama's kills follow their book; clients are told on a change.
fn refresh_kills(game: &mut Game<'_>, pos: [i32; 3], block: &str) {
    let Some(Kind::Diorama(mob)) = Kind::of(block) else { return };
    let Some(uuid) = owner_uuid(game, pos) else { return };
    let Some(kills) = crate::game::soulscraft_book::kills_of(uuid, &format!("soulscraft:{mob}")) else { return };
    if stored_kills(game, pos) == Some(kills) {
        return;
    }
    game.sc_with_mod_block_entity(pos, |e| {
        e.nbt.insert(JavaString::from(trophy::KILLS_KEY), Tag::Double(kills.to_bits()));
    });
    game.sc_block_entity_sync(pos);
}

/// One diorama's particles this tick; how many were queued.
fn emit(game: &mut Game<'_>, pos: [i32; 3], block: &str) -> u64 {
    let Some(Kind::Diorama(mob)) = Kind::of(block) else { return 0 };
    let facing = game.sc_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let mut points = Vec::new();
    {
        let mut h = hub();
        let Hub { files, rng, .. } = &mut *h;
        let Some(f) = files.entry(mob.to_owned()).or_insert_with(|| trophy::embedded(mob)).as_ref() else { return 0 };
        let rng = rng.get_or_insert_with(|| Rng::new(0x5EED_D10A));
        for e in &f.diorama.particles {
            if trophy::due(rng.unit(), e.every) {
                let u = [rng.unit(), rng.unit(), rng.unit()];
                points.push((e.kind.clone(), trophy::particle_point(pos, &e.area, &facing, u)));
            }
        }
    }
    let mut n = 0;
    for (kind, at) in points {
        let options = ParticleOptions::named(kind);
        if game.with_level(|_, level, _| level.native_send_particles(&options, None, at, [0.0; 3], 0.0, 0)) {
            n += 1;
        }
    }
    *hub().sent.entry(pos).or_default() += n;
    n
}

/// Particles sent at a diorama since the last call (the self-test), and the counter cleared.
pub(super) fn take_sent(pos: [i32; 3]) -> u64 {
    hub().sent.remove(&pos).unwrap_or(0)
}

/// Seeds the particle rolls (the self-test: a run it can compare with the rules).
pub(super) fn seed(seed: u64) {
    hub().rng = Some(Rng::new(seed));
}
