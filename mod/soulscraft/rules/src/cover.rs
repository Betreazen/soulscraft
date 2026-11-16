//! Cover from threats that shoot (round 1: smart mobs hide from shooting turrets; RustCraft's
//! `rustcraft:turret` with `role` `attack`, support turrets ignored). A threat sees a mob when the
//! mob's eyes are within its range and nothing solid stands between; a spot is cover when no
//! threat sees a mob standing there. The server half reads the turrets and the blocks, this
//! decides.
//!
//! The marksman reloads behind cover from its target the same way ([`reload_cover`]): the target
//! is the threat. Shooting from behind cover (a spot hidden from turrets that still sees its
//! target) is not done.

use crate::geom::{self, V3};

/// Something that shoots at mobs: where its muzzle is and how far it reaches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    pub pos: V3,
    pub range: f64,
}

/// The cover numbers (data/mobs.json `cover`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverRules {
    /// A turret's reach (RustCraft: 12 blocks).
    pub turret_range: f64,
    /// How far a mob looks for cover.
    pub search_blocks: i32,
    /// How long it stays in cover before its own mind takes over again.
    pub hold_ticks: u64,
    /// How often an exposed mob is checked.
    pub check_every_ticks: u64,
}

impl Default for CoverRules {
    fn default() -> Self {
        Self { turret_range: 12.0, search_blocks: 6, hold_ticks: 60, check_every_ticks: 10 }
    }
}

impl CoverRules {
    pub fn from_value(v: &serde_json::Value) -> Self {
        let d = Self::default();
        let n = |k: &str, def: f64| v.get(k).and_then(serde_json::Value::as_f64).unwrap_or(def);
        Self {
            turret_range: n("turret_range_blocks", d.turret_range),
            search_blocks: n("search_blocks", f64::from(d.search_blocks)) as i32,
            hold_ticks: n("hold_ticks", d.hold_ticks as f64) as u64,
            check_every_ticks: n("check_every_ticks", d.check_every_ticks as f64) as u64,
        }
    }
}

/// The threats that see eyes at `eye`.
pub fn seen_by<'a>(eye: V3, threats: &'a [Threat], sight: &'a impl Fn(V3, V3) -> bool) -> impl Iterator<Item = &'a Threat> + 'a {
    threats.iter().filter(move |t| geom::dist(t.pos, eye) <= t.range && sight(t.pos, eye))
}

pub fn exposed(eye: V3, threats: &[Threat], sight: &impl Fn(V3, V3) -> bool) -> bool {
    seen_by(eye, threats, sight).next().is_some()
}

/// The nearest of `spots` (where the mob's feet would stand) that no threat sees, the mob's eyes
/// `eye_height` over its feet. None when every spot is seen.
pub fn nearest_cover(from: V3, spots: &[V3], eye_height: f64, threats: &[Threat], sight: &impl Fn(V3, V3) -> bool) -> Option<V3> {
    nearest_cover_at(from, spots, &[[0.0, eye_height, 0.0]], threats, sight)
}

/// [`nearest_cover`] that hides every one of `points` (offsets from the mob's feet), not only its
/// eyes.
pub fn nearest_cover_at(from: V3, spots: &[V3], points: &[V3], threats: &[Threat], sight: &impl Fn(V3, V3) -> bool) -> Option<V3> {
    let mut sorted: Vec<V3> = spots.to_vec();
    sorted.sort_by(|a, b| geom::dist(*a, from).total_cmp(&geom::dist(*b, from)));
    sorted
        .into_iter()
        .find(|s| points.iter().all(|p| !exposed(geom::add(*s, *p), threats, sight)))
}

/// A mob walks to cover at this times its speed (from turrets, and the marksman to its reload).
pub const COVER_SPEED: f64 = 1.2;

/// A walk ends this far from where it was sent at most: vanilla's path is done within
/// `maxDistanceToWaypoint` (0.75 − width / 2 for a box under 0.75 wide) of its last node.
pub const ARRIVAL_SLACK: f64 = 0.45;

/// The marksman's reload (04-bestiary.md): the nearest of `spots` its target, whose eyes are at
/// `target_eye`, cannot see, at every one of `heights` (its eyes, `cover_min_height_blocks`), and
/// wherever within [`ARRIVAL_SLACK`] of the spot its walk ends (the corners of that square: a
/// wall's shadow is convex). The target is the threat here, at any range. None: no cover, it steps
/// back instead.
pub fn reload_cover(from: V3, spots: &[V3], heights: &[f64], target_eye: V3, sight: &impl Fn(V3, V3) -> bool) -> Option<V3> {
    let threat = [Threat { pos: target_eye, range: f64::INFINITY }];
    let s = ARRIVAL_SLACK;
    let points: Vec<V3> = heights
        .iter()
        .flat_map(|h| [[-s, *h, -s], [s, *h, -s], [-s, *h, s], [s, *h, s]])
        .collect();
    nearest_cover_at(from, spots, &points, &threat, sight)
}

/// A straight look from `from` to `to` that no solid cell cuts, sampled every quarter block and
/// stopping short of both ends (the turret's own block, the mob's own cell).
pub fn clear_line(from: V3, to: V3, solid: &impl Fn([i32; 3]) -> bool) -> bool {
    let d = geom::sub(to, from);
    let l = geom::len(d);
    if l < 1e-6 {
        return true;
    }
    let steps = (l / 0.25).ceil() as i32;
    let start = [from[0].floor() as i32, from[1].floor() as i32, from[2].floor() as i32];
    let end = [to[0].floor() as i32, to[1].floor() as i32, to[2].floor() as i32];
    for i in 1..steps {
        let p = geom::add(from, geom::scale(d, f64::from(i) / f64::from(steps)));
        let c = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        if c != start && c != end && solid(c) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall at x = 2, z −1..1, y 0..2.
    fn wall(c: [i32; 3]) -> bool {
        c[0] == 2 && (-1..=1).contains(&c[2]) && (0..=2).contains(&c[1])
    }

    #[test]
    fn a_wall_blocks_the_line_and_range_ends_it() {
        let turret = [-4.5, 1.5, 0.5];
        let t = [Threat { pos: turret, range: 12.0 }];
        let sight = |a: V3, b: V3| clear_line(a, b, &wall);
        // In the open, seen; behind the wall, not; beyond the range, not.
        assert!(exposed([0.5, 1.6, 0.5], &t, &sight));
        assert!(!exposed([4.5, 1.6, 0.5], &t, &sight));
        assert!(!exposed([0.5, 1.6, 13.5], &t, &sight));
        assert_eq!(seen_by([0.5, 1.6, 0.5], &t, &sight).count(), 1);
        assert!(clear_line([0.5, 0.5, 0.5], [0.5, 0.5, 0.5], &wall));
    }

    #[test]
    fn the_nearest_unseen_spot_is_the_cover() {
        let t = [Threat { pos: [-4.5, 1.5, 0.5], range: 12.0 }];
        let sight = |a: V3, b: V3| clear_line(a, b, &wall);
        let spots: Vec<V3> = (-3..=6).flat_map(|x| (-3..=3).map(move |z| [f64::from(x) + 0.5, 0.0, f64::from(z) + 0.5])).collect();
        let c = nearest_cover([0.5, 0.0, 0.5], &spots, 1.6, &t, &sight).unwrap();
        assert!(c[0] > 2.0 && c[2].abs() < 1.5, "{c:?}");
        // No threat: the nearest spot (where it stands) is cover.
        assert_eq!(nearest_cover([0.5, 0.0, 0.5], &spots, 1.6, &[], &sight), Some([0.5, 0.0, 0.5]));
        // Everything seen: none.
        let open = |a: V3, b: V3| clear_line(a, b, &|_| false);
        assert_eq!(nearest_cover([0.5, 0.0, 0.5], &spots, 1.6, &t, &open), None);
    }

    #[test]
    fn rules_read_from_data_with_defaults() {
        let r = CoverRules::from_value(&serde_json::json!({ "turret_range_blocks": 10, "hold_ticks": 40 }));
        assert_eq!((r.turret_range, r.search_blocks, r.hold_ticks, r.check_every_ticks), (10.0, 6, 40, 10));
        assert_eq!(CoverRules::from_value(&serde_json::Value::Null), CoverRules::default());
    }
}

#[cfg(test)]
mod data_tests {
    #[test]
    fn the_data_has_the_turret_range() {
        let f = crate::data::MobsFile::embedded();
        let r = super::CoverRules::from_value(&f.cover);
        assert_eq!((r.turret_range, r.search_blocks, r.hold_ticks), (12.0, 6, 60));
    }
}
