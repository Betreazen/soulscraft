//! Room for a mob's box (owner's decision, round 9): before a SoulsCraft mob appears — `/sc spawn`,
//! a wave, an adopted mob — its box must fit where it stands. When it does not, the nearest spot
//! within a small radius (data/mobs.json `spawn_fit.search_blocks`) that fits and holds it up is
//! taken; when none does, the spawn is skipped.
//!
//! Blocks are read through `boxes(cell)`: the collision boxes of the block in that cell, in the
//! cell's own coordinates (0 to 1), as `native_block_collision` gives them.

use crate::geom::V3;

/// A collision box in a cell's coordinates: min x, y, z, max x, y, z.
pub type CellBox = [f64; 6];

const EPS: f64 = 1e-4;

/// The mob's box at `at` (its feet's centre): `width` across, `height` up.
fn aabb(at: V3, width: f64, height: f64) -> [f64; 6] {
    let h = width / 2.0;
    [at[0] - h, at[1], at[2] - h, at[0] + h, at[1] + height, at[2] + h]
}

fn overlaps(a: [f64; 6], b: [f64; 6]) -> bool {
    a[0] < b[3] - EPS && a[3] > b[0] + EPS && a[1] < b[4] - EPS && a[4] > b[1] + EPS && a[2] < b[5] - EPS && a[5] > b[2] + EPS
}

/// Whether no block's collision box cuts into the mob's box at `at`.
pub fn fits(at: V3, width: f64, height: f64, boxes: &impl Fn([i32; 3]) -> Vec<CellBox>) -> bool {
    let b = aabb(at, width, height);
    for x in b[0].floor() as i32..=(b[3] - EPS).floor() as i32 {
        for y in b[1].floor() as i32..=(b[4] - EPS).floor() as i32 {
            for z in b[2].floor() as i32..=(b[5] - EPS).floor() as i32 {
                let o = [f64::from(x), f64::from(y), f64::from(z)];
                for c in boxes([x, y, z]) {
                    let w = [c[0] + o[0], c[1] + o[1], c[2] + o[2], c[3] + o[0], c[4] + o[1], c[5] + o[2]];
                    if overlaps(b, w) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Whether something under the mob's feet holds it up: a box whose top is at its feet, under its
/// footprint.
pub fn supported(at: V3, width: f64, boxes: &impl Fn([i32; 3]) -> Vec<CellBox>) -> bool {
    let b = aabb(at, width, 0.0);
    let y = (at[1] - 0.01).floor() as i32;
    for x in b[0].floor() as i32..=(b[3] - EPS).floor() as i32 {
        for z in b[2].floor() as i32..=(b[5] - EPS).floor() as i32 {
            let o = [f64::from(x), f64::from(y), f64::from(z)];
            for c in boxes([x, y, z]) {
                let top = c[4] + o[1];
                let foot = [c[0] + o[0], c[2] + o[2], c[3] + o[0], c[5] + o[2]];
                let under = foot[0] < b[3] && foot[2] > b[0] && foot[1] < b[5] && foot[3] > b[2];
                if under && (top - at[1]).abs() < 0.05 {
                    return true;
                }
            }
        }
    }
    false
}

/// The spot for a mob of this box near `at`: `at` itself when it fits, else the nearest block
/// centre within `radius` blocks (a block up or down too) where the box fits and, unless it
/// flies or swims (`grounded` false), stands on something. None when no spot within the radius
/// does.
pub fn nearest(at: V3, width: f64, height: f64, radius: i32, grounded: bool, boxes: &impl Fn([i32; 3]) -> Vec<CellBox>) -> Option<V3> {
    let ok = |p: V3| fits(p, width, height, boxes) && (!grounded || supported(p, width, boxes));
    if fits(at, width, height, boxes) && (!grounded || supported(at, width, boxes) || !supported_anywhere(at, boxes)) {
        return Some(at);
    }
    let base = [at[0].floor(), at[1].floor(), at[2].floor()];
    let mut spots = Vec::new();
    for dy in [0, 1, -1, 2, -2] {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let p = [base[0] + 0.5 + f64::from(dx), base[1] + f64::from(dy), base[2] + 0.5 + f64::from(dz)];
                // A step up or down counts as far as four sideways: a mob under a roof goes out, not on top.
                let d = (p[0] - at[0]).powi(2) + (p[2] - at[2]).powi(2) + 16.0 * (p[1] - at[1]).powi(2);
                spots.push((d, p));
            }
        }
    }
    spots.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, p) in spots {
        // A slab or a step under the feet: stand on its top.
        let mut q = p;
        let below = boxes([p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
        if let Some(top) = below.iter().map(|c| c[4]).filter(|t| *t <= 0.5 + EPS).fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t)))) {
            q[1] = p[1].floor() + top;
        }
        if ok(q) {
            return Some(q);
        }
    }
    None
}

/// A spot in mid-air (a `/sc spawn` from a flying player): grounding is not asked of it.
fn supported_anywhere(at: V3, boxes: &impl Fn([i32; 3]) -> Vec<CellBox>) -> bool {
    let c = [at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32];
    (1..=3).any(|d| !boxes([c[0], c[1] - d, c[2]]).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const FULL: CellBox = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];

    /// Stone at y 0 everywhere in x, z −20..20, and `extra` cells.
    fn world(extra: &[[i32; 3]]) -> impl Fn([i32; 3]) -> Vec<CellBox> {
        let set: HashSet<[i32; 3]> = extra.iter().copied().collect();
        move |c: [i32; 3]| {
            if (c[1] == 0 && c[0].abs() <= 20 && c[2].abs() <= 20) || set.contains(&c) {
                vec![FULL]
            } else {
                Vec::new()
            }
        }
    }

    #[test]
    fn a_box_fits_in_the_open_and_not_under_a_low_ceiling() {
        let w = world(&[]);
        assert!(fits([0.5, 1.0, 0.5], 1.6, 3.2, &w));
        assert!(supported([0.5, 1.0, 0.5], 1.6, &w));
        assert!(!supported([0.5, 3.0, 0.5], 1.6, &w));
        // A ceiling 3 blocks over the floor: a zombie (1.95) fits, a brute (3.2) does not.
        let roof: Vec<[i32; 3]> = (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 4, z])).collect();
        let w = world(&roof);
        assert!(fits([0.5, 1.0, 0.5], 0.6, 1.95, &w));
        assert!(!fits([0.5, 1.0, 0.5], 1.6, 3.2, &w));
        // A wall beside: a 1.6 wide box at a block's centre reaches 0.3 into the next blocks.
        let w = world(&[[1, 1, 0]]);
        assert!(!fits([0.5, 1.0, 0.5], 1.6, 3.2, &w));
        assert!(fits([0.5, 1.0, 0.5], 0.6, 1.95, &w));
    }

    #[test]
    fn the_nearest_spot_that_fits_is_taken() {
        let roof: Vec<[i32; 3]> = (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 4, z])).collect();
        let w = world(&roof);
        // Under the roof's middle: the nearest spot out from under it.
        let p = nearest([0.5, 1.0, 0.5], 1.6, 3.2, 4, true, &w).unwrap();
        assert!(fits(p, 1.6, 3.2, &w) && supported(p, 1.6, &w));
        let d = (p[0] - 0.5).abs().max((p[2] - 0.5).abs());
        assert!((3.0..=4.0).contains(&d), "{p:?}");
        // Fits already: unchanged.
        assert_eq!(nearest([5.3, 1.0, 5.7], 1.6, 3.2, 4, true, &w), Some([5.3, 1.0, 5.7]));
        // A radius too small: none.
        assert_eq!(nearest([0.5, 1.0, 0.5], 1.6, 3.2, 1, true, &w), None);
    }

    #[test]
    fn a_spot_inside_a_block_moves_up_or_aside_and_a_flyer_needs_no_floor() {
        let w = world(&[[0, 1, 0]]);
        let p = nearest([0.5, 1.0, 0.5], 0.6, 1.95, 2, true, &w).unwrap();
        assert!(fits(p, 0.6, 1.95, &w) && supported(p, 0.6, &w), "{p:?}");
        // In mid-air with nothing under for blocks: taken as it is.
        let w = world(&[]);
        assert_eq!(nearest([0.5, 30.0, 0.5], 0.5, 0.9, 2, true, &w), Some([0.5, 30.0, 0.5]));
        assert_eq!(nearest([0.5, 3.0, 0.5], 0.5, 0.9, 2, false, &w), Some([0.5, 3.0, 0.5]));
        // Over the floor in the air a grounded mob comes down to it.
        let p = nearest([0.5, 2.0, 0.5], 0.6, 1.95, 2, true, &w).unwrap();
        assert_eq!(p[1], 1.0);
    }

    #[test]
    fn slabs_hold_a_mob_on_their_top() {
        let w = |c: [i32; 3]| -> Vec<CellBox> {
            if c[1] == 0 || (c[1] == 1 && c[0] == 0 && c[2] == 0) {
                if c[1] == 1 { vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]] } else { vec![FULL] }
            } else {
                Vec::new()
            }
        };
        assert!(supported([0.5, 1.5, 0.5], 0.6, &w));
        assert!(!fits([0.5, 1.0, 0.5], 0.6, 1.95, &w));
        let p = nearest([0.5, 1.0, 0.5], 0.6, 1.95, 2, true, &w).unwrap();
        assert!(fits(p, 0.6, 1.95, &w) && supported(p, 0.6, &w), "{p:?}");
    }
}
