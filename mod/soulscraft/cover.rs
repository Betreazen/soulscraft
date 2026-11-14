// Smart mobs take cover from attack turrets (round 1; RustCraft's `rustcraft:turret`, whose block
// entity says `role` "attack" or "support" — only attack turrets shoot, so only they count;
// soulscraft_rules::cover decides). A SoulsCraft mob outside an altar's event that an attack
// turret within its range sees walks to the nearest spot within `cover.search_blocks` where its
// box fits and no turret sees it, stays there `cover.hold_ticks`, then its own mind takes over
// again. Turrets are found through the level's mod block entities near the mob
// (`BlockEntities::mods_near`), not by scanning blocks; the check runs every
// `cover.check_every_ticks`.
//
// Not yet (prototypes/p6-altar.md): the marksman's cover that still sees its target, and other
// threats (players with bows) in the same list.
//
// Log line: soulscraft: cover mob=revenant id=12 from=-706,101,700 to=-697.5,101.0,700.5 tick=..
use crate::game::Game;
use horizon_client::containers::BlockEntity;
use horizon_world::nbt::{JavaString, Tag};
use soulscraft_rules::{
    cover::{self, CoverRules, Threat},
    fit,
    geom::{self, V3},
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

pub(super) const TURRET: &str = "rustcraft:turret";
const ROLE_KEY: &str = "role";
/// The turret's muzzle over its block's floor.
const MUZZLE: f64 = 0.8;
/// A walk to cover is asked again this often.
const REPATH: u64 = 10;

#[derive(Clone, Copy, Debug, Default)]
struct State {
    spot: Option<V3>,
    /// Stays in cover until then (set when it gets there).
    until: Option<u64>,
    next_check: u64,
    walked: u64,
}

static STATES: LazyLock<Mutex<HashMap<u32, State>>> = LazyLock::new(Mutex::default);

fn states() -> MutexGuard<'static, HashMap<u32, State>> {
    STATES.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn rules() -> CoverRules {
    CoverRules::from_value(&crate::game::soulscraft_server::state().data.cover)
}

/// The attack turrets of the level in place within `reach` of `at`, as threats.
pub(super) fn threats(game: &Game<'_>, at: V3, reach: f64, range: f64) -> Vec<Threat> {
    let near: Vec<(i32, i32, i32)> = game.world.block_entities.mods_near(at[0], at[2], reach).copied().collect();
    near.into_iter()
        .filter_map(|p| match game.world.block_entities.get(&p) {
            Some(BlockEntity::Mod(m)) if m.id == TURRET && role(&m.nbt).as_deref() == Some("attack") => {
                Some(Threat { pos: [f64::from(p.0) + 0.5, f64::from(p.1) + MUZZLE, f64::from(p.2) + 0.5], range })
            }
            _ => None,
        })
        .filter(|t| geom::dist(t.pos, at) <= reach)
        .collect()
}

/// A turret's `role` (RustCraft slice 8: "attack" or "support").
pub(super) fn role(nbt: &horizon_world::nbt::Compound) -> Option<String> {
    match nbt.get(&JavaString::from(ROLE_KEY)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn solid(game: &Game<'_>, c: [i32; 3]) -> bool {
    !game.native_block_collision(c).is_empty()
}

pub(super) fn sees(game: &Game<'_>, from: V3, to: V3) -> bool {
    cover::clear_line(from, to, &|c| solid(game, c))
}

/// Where the mob goes this tick for cover, or None for its own mind. `width`, `height`, `eye`:
/// its box.
pub(super) fn steer(game: &mut Game<'_>, dim: &str, now: u64, id: u32, pos: V3, size: (f64, f64, f64)) -> Option<V3> {
    let r = rules();
    let mut s = states().get(&id).copied().unwrap_or_default();
    let (width, height, eye) = size;
    if let Some(spot) = s.spot {
        if s.until.is_none() && geom::flat_dist(pos, spot) < 0.6 {
            s.until = Some(now + r.hold_ticks);
            game.native_mob_stop(dim, id);
        }
        if s.until.is_some_and(|u| now >= u) {
            states().remove(&id);
            return None;
        }
        if now < s.next_check {
            if s.until.is_none() && now.saturating_sub(s.walked) >= REPATH {
                game.native_mob_navigate(dim, id, spot, 0, 1.2);
                s.walked = now;
            }
            states().insert(id, s);
            return Some(spot);
        }
    }
    if now < s.next_check {
        return None;
    }
    s.next_check = now + r.check_every_ticks;
    let reach = r.turret_range + f64::from(r.search_blocks) + 2.0;
    let threats = threats(game, pos, reach, r.turret_range);
    let sight = |a: V3, b: V3| sees(game, a, b);
    let head = geom::add(pos, [0.0, eye, 0.0]);
    if threats.is_empty() || !cover::exposed(head, &threats, &sight) {
        // Not seen: in cover already, or nothing shoots here.
        if s.spot.is_some() {
            states().insert(id, s);
            return s.spot;
        }
        states().insert(id, s);
        return None;
    }
    let boxes = |c: [i32; 3]| game.native_block_collision(c);
    let k = r.search_blocks;
    let mut spots = Vec::new();
    for dy in -1..=1 {
        for dx in -k..=k {
            for dz in -k..=k {
                let p = [pos[0].floor() + 0.5 + f64::from(dx), pos[1].floor() + f64::from(dy), pos[2].floor() + 0.5 + f64::from(dz)];
                if fit::fits(p, width, height, &boxes) && fit::supported(p, width, &boxes) {
                    spots.push(p);
                }
            }
        }
    }
    let found = cover::nearest_cover(pos, &spots, eye, &threats, &sight);
    match found {
        Some(spot) => {
            let from = threats.iter().find(|t| geom::dist(t.pos, head) <= t.range && sight(t.pos, head)).map(|t| t.pos);
            if s.spot != Some(spot) {
                log::info!(
                    "soulscraft: cover id={id} from={} to={:.1},{:.1},{:.1} tick={now}",
                    from.map_or("-".into(), |f| format!("{:.0},{:.0},{:.0}", f[0].floor(), f[1].floor(), f[2].floor())),
                    spot[0],
                    spot[1],
                    spot[2]
                );
            }
            game.native_mob_navigate(dim, id, spot, 0, 1.2);
            s.spot = Some(spot);
            s.until = None;
            s.walked = now;
            states().insert(id, s);
            Some(spot)
        }
        None => {
            states().insert(id, s);
            None
        }
    }
}

/// A mob gone: its cover state too.
pub(super) fn forget(id: u32) {
    states().remove(&id);
}

/// For the self-test: whether the mob is heading for or holding cover.
pub(super) fn in_cover_mode(id: u32) -> bool {
    states().get(&id).is_some_and(|s| s.spot.is_some())
}
