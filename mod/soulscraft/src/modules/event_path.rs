// The altar event's path map and the brute's breach (06-altar.md, «Путь к алтарю», «Пролом
// громилы»; soulscraft::rules::flow): the zone's cells read from the world into a flow field, a
// look through it, and the brute breaking an opening in a wall on its way.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_altar as altar_srv;
use crate::game::soulscraft_event::{Breaching, EventRun, WaveMob, cell_of, centre_of, core_centre, halt, nav_to, record};
use crate::game::soulscraft_server as server;
use horizon_world::registry::BlockStateId;
use soulscraft::rules::{
    altar as ra,
    defense,
    flow::{Cell, Field, Grid},
    geom::{self, V3},
};
use std::collections::HashMap;

/// The zone's cells as the path map sees them, from the world: air, low blocks (slabs), walls
/// the brute breaks in some hits, and blocks nothing breaks (altar blocks, machines, bedrock).
pub(super) fn build_field(game: &mut Game<'_>, r: &mut EventRun, now: u64) {
    let started = std::time::Instant::now();
    let (min, max) = r.flow.zone(r.core);
    let y0 = r.core[1] - r.flow.below_blocks;
    let dims = [max[0] - min[0] + 1, r.flow.below_blocks + r.flow.above_blocks + 1, max[1] - min[1] + 1];
    let mut grid = Grid::new([min[0], y0, min[1]], dims, Cell::Hard);
    let mut cache: HashMap<BlockStateId, Cell> = HashMap::new();
    for y in y0..y0 + dims[1] {
        for z in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                let id = game.world.block(x, y, z);
                let cell = match cache.get(&id) {
                    Some(c) => *c,
                    None => {
                        let c = classify(game, id, [x, y, z], &r.numbers.breach);
                        cache.insert(id, c);
                        c
                    }
                };
                grid.set([x, y, z], cell);
            }
        }
    }
    let reach = r.flow.drain_blocks;
    let mut seeds = Vec::new();
    let span = reach.ceil() as i32;
    for dx in -span..=span {
        for dz in -span..=span {
            if f64::from(dx * dx + dz * dz).sqrt() > reach {
                continue;
            }
            for dy in 0..=2 {
                seeds.push([r.core[0] + dx, r.core[1] + dy, r.core[2] + dz]);
            }
        }
    }
    let n = grid.len();
    let field = Field::build(grid, &seeds);
    r.field = Some(field);
    r.dirty_at = None;
    r.built_at = now;
    r.builds += 1;
    log::info!(
        "soulscraft: event field cells={n} seeds={} ms={:.2} build={} tick={now}",
        r.field.as_ref().map_or(0, |f| f.seeds.len()),
        started.elapsed().as_secs_f64() * 1000.0,
        r.builds
    );
}

/// The breach field's cost at `c`. Its first question after a build works the field out (the
/// costly part, rules::flow::Field), logged:
///   soulscraft: event field breach ms=10.95 build=3
pub(super) fn breach_at(r: &EventRun, c: [i32; 3]) -> Option<u32> {
    let f = r.field.as_ref()?;
    if f.breach_ready() {
        return f.breach_at(c);
    }
    let started = std::time::Instant::now();
    let d = f.breach_at(c);
    log::info!("soulscraft: event field breach ms={:.2} build={}", started.elapsed().as_secs_f64() * 1000.0, r.builds);
    d
}

/// The cell a block makes on the path map. The zone's changes are told apart by it too: a change
/// that leaves the cell as it was never rebuilds the map.
pub(super) fn classify(game: &Game<'_>, id: BlockStateId, pos: [i32; 3], rules: &defense::BreachRules) -> Cell {
    let facts = game.res.facts().get(id);
    let name: &str = &facts.name;
    if matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
        return Cell::Open;
    }
    if name == ra::SLAB || name == ra::SOCKET {
        return Cell::Low;
    }
    if ra::is_altar_block(name) {
        return Cell::Hard;
    }
    let boxes = game.sc_block_collision(pos);
    if boxes.is_empty() {
        return Cell::Open;
    }
    let top = boxes.iter().map(|b| b[4]).fold(0.0, f64::max);
    if top <= 0.5 + 1e-6 {
        return Cell::Low;
    }
    match rules.hits(name, facts.hardness, facts.block_entity) {
        Some(h) => Cell::Wall(h.min(255) as u8),
        None => Cell::Hard,
    }
}

/// A straight look through the zone's cells from `from` to `to`, short of the last block.
pub(super) fn sight(field: Option<&Field>, from: V3, to: V3) -> bool {
    let Some(f) = field else { return true };
    let d = geom::sub(to, from);
    let l = geom::len(d);
    let steps = ((l - 1.0) / 0.5).floor() as i32;
    for i in 1..steps.max(1) {
        let p = geom::add(from, geom::scale(d, f64::from(i) * 0.5 / l));
        if matches!(f.grid.get(cell_of(p)), Cell::Wall(_) | Cell::Hard) {
            return false;
        }
    }
    true
}

/// The brute walled out: it walks the breach map and, at a wall, hits the opening in front of it
/// (two columns wide and four high, its own size) with its overhead blow until it breaks. Each
/// swing is one hit on every block of the opening; a block goes after the hits its hardness
/// needs (Г12). Broken blocks drop nothing and go in the core's book.
pub(super) fn breach(game: &mut Game<'_>, r: &mut EventRun, id: u32, w: &mut WaveMob, cell: Option<[i32; 3]>, now: u64) -> bool {
    let dim = r.dim.clone();
    let (hit_at, cycle) = brute_timing();
    if w.breach.is_none() {
        let Some(field) = r.field.as_ref() else { return false };
        let Some(c) = cell else { return false };
        let Some((next, walls)) = field.next_breach(c) else { return false };
        if walls.is_empty() {
            nav_to(game, &dim, id, w, next, now);
            return true;
        }
        let blocks = opening(game, r, w.pos, c, next);
        if blocks.is_empty() {
            return false;
        }
        record(format!("breach mob={id} at={},{},{} blocks={} tick={now}", next[0], next[1], next[2], blocks.len()));
        w.breach = Some(Breaching { blocks, swing_start: now });
        server::show_clip(game, &dim, id, "attack_slam", now);
    }
    let Some(b) = w.breach.as_mut() else { return false };
    let target = b.blocks.first().map_or(core_centre(r.core), |(p, _)| centre_of(*p));
    halt(game, &dim, id, w, target);
    let Some(b) = w.breach.as_mut() else { return false };
    let t = now - b.swing_start;
    if t == hit_at {
        let mut broke = Vec::new();
        for (pos, hits) in b.blocks.iter_mut() {
            *hits = hits.saturating_sub(1);
            if *hits == 0 {
                broke.push(*pos);
            }
        }
        b.blocks.retain(|(_, h)| *h > 0);
        for pos in broke {
            let state = altar_srv::state_string_at(game, pos);
            r.breaches.record(pos, &state);
            altar_srv::setblock(game, &dim, pos, "minecraft:air", "replace");
            altar_srv::run(game, &dim, &format!("playsound minecraft:block.stone.break hostile @a {} {} {} 1 0.7", pos[0], pos[1], pos[2]));
            record(format!("broke mob={id} at={},{},{} block={state} tick={now}", pos[0], pos[1], pos[2]));
        }
        let lines = r.breaches.to_lines();
        altar_srv::set_core_lines(game, r.core, altar_srv::BREACHED_KEY, &lines);
        r.dirty_at.get_or_insert(now);
    }
    if t >= cycle {
        if w.breach.as_ref().is_some_and(|b| b.blocks.is_empty()) {
            w.breach = None;
        } else if let Some(b) = w.breach.as_mut() {
            b.swing_start = now;
            server::show_clip(game, &dim, id, "attack_slam", now);
        }
    }
    true
}

/// The brute's overhead blow: the tick its hit lands and its whole length (data/mobs.json).
fn brute_timing() -> (u64, u64) {
    let s = server::state();
    let a = s.data.mob("brute").and_then(|m| m.attack("overhead_wave"));
    match a {
        Some(a) => (u64::from(a.windup + a.hold), u64::from(soulscraft::rules::attack::Timeline::of(a).end()).max(2)),
        None => (16, 38),
    }
}

/// The blocks in the way of a brute stepping from `from` to `next`: the two columns its width
/// takes (the side its body leans to first) and four cells up. Only breakable ones; none when a
/// block nothing breaks stands there.
fn opening(game: &Game<'_>, r: &EventRun, pos: V3, from: [i32; 3], next: [i32; 3]) -> Vec<([i32; 3], u32)> {
    let Some(field) = r.field.as_ref() else { return Vec::new() };
    let dx = next[0] - from[0];
    let dz = next[2] - from[2];
    // Across the step: the side the brute's centre leans to.
    let (ax, az) = if dx != 0 { (0, 1) } else { (1, 0) };
    let lean = if dx != 0 { pos[2] - pos[2].floor() } else { pos[0] - pos[0].floor() };
    let sides: [i32; 2] = if lean >= 0.5 { [1, -1] } else { [-1, 1] };
    for s in sides {
        let mut out = Vec::new();
        let mut blocked = false;
        for col in [0, s] {
            for up in 0..4 {
                let p = [next[0] + ax * col, next[1] + up, next[2] + az * col];
                match field.grid.get(p) {
                    Cell::Wall(_) => {
                        let id = game.world.block(p[0], p[1], p[2]);
                        let facts = game.res.facts().get(id);
                        // The map is a second old: a block already gone is no wall.
                        if facts.replaceable {
                            continue;
                        }
                        if let Some(h) = r.numbers.breach.hits(&facts.name, facts.hardness, facts.block_entity) {
                            out.push((p, h));
                        }
                    }
                    Cell::Hard if up < 2 || col == 0 => blocked = true,
                    _ => {}
                }
            }
        }
        if !blocked && !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

