// Trophies on the server (docs/design/09-trophies.md; soulscraft_rules::trophy): the block
// entities of heads and dioramas (so clients draw them), the state a placed head or diorama takes,
// a head put on a wall, and the dioramas' particles.
//
// - Block entities (`native_block_entity`, API 15): every `<mob>_head`, `<mob>_wall_head` and
//   `<mob>_trophy` has one; only dioramas tick (`native_block_entity_ticks`), and their tick only
//   notes where they are (the tick hook has no game: native_hooks.rs:3526-3538).
// - Placement (`native_state_for_placement`, API 76; called for every placement through vanilla's
//   `BlockItem.place` on an authoritative server, game.rs:1659-1669, no `mods()` check): a floor
//   head turns to the placer in 16 steps (vanilla's `SkullBlock`), a diorama turns its plaque to
//   the placer (`getHorizontalDirection().getOpposite()`).
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
use crate::game::Game;
use horizon_client::particles::ParticleOptions;
use horizon_world::nbt::Compound;
use horizon_world::registry::BlockStateId;
use soulscraft_rules::event::Rng;
use soulscraft_rules::trophy::{self, Kind, TrophyFile};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

const LOG_EVERY: u64 = 200;

#[derive(Default)]
struct Hub {
    files: HashMap<String, Option<TrophyFile>>,
    /// Wall heads to put up in the level's tick: (dimension, cell, state).
    walls: Vec<(String, [i32; 3], String)>,
    rng: Option<Rng>,
    /// Particles sent per diorama cell since the counter was last read (the self-test).
    sent: HashMap<[i32; 3], u64>,
    sent_since_log: u64,
}

static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(Mutex::default);
/// Dioramas whose block entity ticked since the level's last tick (the tick hook takes only this).
static TICKED: Mutex<Vec<([i32; 3], String)>> = Mutex::new(Vec::new());

fn hub() -> MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_block_entity`: heads and dioramas get one on every side, so clients draw them.
pub(super) fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && Kind::of(block).is_some() {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// `native_block_entity_ticks`: only dioramas tick (their particles).
pub(super) fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if matches!(Kind::of(block), Some(Kind::Diorama(_))) {
        *ticks = true;
    }
}

pub(super) fn on_block_entity_tick(pos: [i32; 3], entity: &horizon_client::containers::ModBlockEntity) {
    if matches!(Kind::of(&entity.id), Some(Kind::Diorama(_))) {
        TICKED.lock().unwrap_or_else(PoisonError::into_inner).push((pos, entity.id.to_string()));
    }
}

/// `native_state_for_placement` (GameLevel, read-only): the state a head or a diorama is placed in.
#[allow(clippy::option_option)]
pub(super) fn on_state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    if answer.is_some() {
        return;
    }
    let Some(block) = level.native_state_block(state) else { return };
    let (key, value) = match Kind::of(block) {
        Some(Kind::Head(_)) => ("rotation", trophy::floor_rotation(context.yaw).to_string()),
        Some(Kind::Diorama(_)) => ("facing", trophy::facing_toward(context.yaw).to_owned()),
        _ => return,
    };
    if let Some(s) = level.native_state_with_property(state, key, &value) {
        *answer = Some(Some(s));
    }
}

/// `native_right_click_block` with the clicked face: a head item on the side of a block goes up
/// as a wall head (in the level's tick). The player's stack changes now.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let Some((item, _, _)) = game.native_held() else { return };
    let Some(mob) = trophy::head_item_mob(&item) else { return };
    let Some(facing) = trophy::wall_facing(face) else { return };
    let Some(player) = game.native_acting_player() else { return };
    let cell = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
    let id = game.world.block(cell[0], cell[1], cell[2]);
    if !game.res.facts().get(id).replaceable {
        return;
    }
    *canceled = true;
    if player.game_mode != 1 && game.native_take_items(&item, 1) != 1 {
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
    let ticked: Vec<([i32; 3], String)> = std::mem::take(&mut *TICKED.lock().unwrap_or_else(PoisonError::into_inner));
    if ticked.is_empty() {
        return;
    }
    let mut sent = 0;
    for (pos, block) in &ticked {
        sent += emit(game, *pos, block);
    }
    let mut h = hub();
    h.sent_since_log += sent;
    if now % LOG_EVERY == 0 {
        log::info!("soulscraft: trophy particles dioramas={} sent={} tick={now}", ticked.len(), h.sent_since_log);
        h.sent_since_log = 0;
    }
}

/// One diorama's particles this tick; how many were queued.
fn emit(game: &mut Game<'_>, pos: [i32; 3], block: &str) -> u64 {
    let Some(Kind::Diorama(mob)) = Kind::of(block) else { return 0 };
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
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
