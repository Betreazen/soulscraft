//! `assets/rigs/<mob>.rig.json` (format in assets/rigs/README.md): the model's parts, cubes and
//! sockets, and its clips by tick. The client builds its `ModelPart` tree from it once and asks
//! [`Rig::sample`] for every part's pose each frame, interpolated between ticks.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Vanilla's walk cycle: a leg swings `cos(walk_position · 0.6662)`, so one cycle of the walk
/// clip is `2π / 0.6662` of `walk_position`.
pub const WALK_PERIOD: f32 = std::f32::consts::TAU / 0.6662;

#[derive(Clone, Debug, Deserialize)]
pub struct Rig {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub texture: Texture,
    pub parts: Vec<PartDef>,
    #[serde(default)]
    pub sockets: Vec<SocketDef>,
    #[serde(default)]
    pub clips: BTreeMap<String, ClipDef>,
    /// Each part's parent's index, filled after reading.
    #[serde(skip)]
    pub parents: Vec<Option<usize>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Texture {
    pub sprite: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartDef {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub pivot: [f32; 3],
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default)]
    pub cubes: Vec<CubeDef>,
    /// Shown only while the mob's flag is on (`rage`), or off (`!lit`).
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CubeDef {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: [i32; 2],
    #[serde(default)]
    pub inflate: Value,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub glow: bool,
    /// The faces drawn when not all six: `front`, `back`, `left`, `right`, `top`, `bottom`.
    #[serde(default)]
    pub faces: Option<Vec<String>>,
}

impl CubeDef {
    /// `CubeDeformation` per axis: one number or three.
    pub fn grow(&self) -> [f32; 3] {
        match &self.inflate {
            Value::Number(n) => [n.as_f64().unwrap_or(0.0) as f32; 3],
            Value::Array(v) if v.len() == 3 => {
                let g = |i: usize| v[i].as_f64().unwrap_or(0.0) as f32;
                [g(0), g(1), g(2)]
            }
            _ => [0.0; 3],
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SocketDef {
    pub name: String,
    pub parent: String,
    pub offset: [f32; 3],
    /// Turns after the offset, in order: `["x", -90]`.
    #[serde(default)]
    pub rotate: Vec<(String, f32)>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub display: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClipDef {
    pub ticks: u32,
    #[serde(default, rename = "loop")]
    pub looped: bool,
    #[serde(default)]
    pub markers: BTreeMap<String, u32>,
    /// Per part: a row per tick, 6 numbers (place, turn) or 9 (and scale).
    #[serde(default)]
    pub tracks: BTreeMap<String, Vec<Vec<f32>>>,
}

/// A part's place in texels, turn in radians (applied Z, then Y, then X) and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    pub scale: [f32; 3],
}

impl Pose {
    fn of_row(row: &[f32]) -> Self {
        let g = |i: usize, d: f32| row.get(i).copied().unwrap_or(d);
        Self {
            pos: [g(0, 0.0), g(1, 0.0), g(2, 0.0)],
            rot: [g(3, 0.0), g(4, 0.0), g(5, 0.0)],
            scale: [g(6, 1.0), g(7, 1.0), g(8, 1.0)],
        }
    }

    fn lerp(a: Self, b: Self, f: f32) -> Self {
        let m = |x: [f32; 3], y: [f32; 3]| std::array::from_fn(|i| x[i] + (y[i] - x[i]) * f);
        Self {
            pos: m(a.pos, b.pos),
            rot: m(a.rot, b.rot),
            scale: m(a.scale, b.scale),
        }
    }
}

impl Rig {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut rig: Rig = serde_json::from_str(text).map_err(|e| format!("rig: {e}"))?;
        if rig.format != "soulscraft-rig" || !(1..=2).contains(&rig.version) {
            return Err(format!("rig {}: format {} version {}", rig.id, rig.format, rig.version));
        }
        let mut parents = Vec::with_capacity(rig.parts.len());
        for (i, p) in rig.parts.iter().enumerate() {
            let parent = match &p.parent {
                None => None,
                Some(name) => {
                    let at = rig.parts[..i].iter().position(|q| &q.name == name);
                    Some(at.ok_or_else(|| format!("rig {}: {} before its parent {name}", rig.id, p.name))?)
                }
            };
            parents.push(parent);
        }
        rig.parents = parents;
        for (name, clip) in &rig.clips {
            for (part, rows) in &clip.tracks {
                if rig.part(part).is_none() {
                    return Err(format!("rig {}: clip {name} moves no part {part}", rig.id));
                }
                if rows.len() != clip.ticks as usize + 1 {
                    return Err(format!("rig {}: clip {name} part {part}: {} rows", rig.id, rows.len()));
                }
            }
        }
        for s in &rig.sockets {
            if rig.part(&s.parent).is_none() {
                return Err(format!("rig {}: socket {} on no part {}", rig.id, s.name, s.parent));
            }
        }
        Ok(rig)
    }

    pub fn part(&self, name: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.name == name)
    }

    pub fn rest(&self, i: usize) -> Pose {
        let p = &self.parts[i];
        Pose { pos: p.pivot, rot: p.rotation, scale: [1.0; 3] }
    }

    /// The parts from the root down to `i`, `i` last.
    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut at = i;
        while let Some(p) = self.parents[at] {
            out.push(p);
            at = p;
        }
        out.reverse();
        out
    }

    /// Whether part `i` shows with these flags on (its own state; a hidden parent hides it too).
    pub fn visible(&self, i: usize, flags: &[&str]) -> bool {
        self.chain(i).into_iter().all(|j| match self.parts[j].state.as_deref() {
            None => true,
            Some(s) => match s.strip_prefix('!') {
                Some(off) => !flags.contains(&off),
                None => flags.contains(&s),
            },
        })
    }

    /// Every part's pose `t` ticks into `clip` (a looping clip wraps, another holds its last
    /// frame); the rest pose for a clip the rig lacks and for parts the clip leaves still.
    pub fn sample(&self, clip: &str, t: f32) -> Vec<Pose> {
        let mut out: Vec<Pose> = (0..self.parts.len()).map(|i| self.rest(i)).collect();
        let Some(c) = self.clips.get(clip) else { return out };
        let ticks = c.ticks.max(1) as f32;
        let t = if c.looped { t.rem_euclid(ticks) } else { t.clamp(0.0, ticks) };
        let i = (t.floor() as usize).min(c.ticks as usize);
        let f = t - i as f32;
        for (name, rows) in &c.tracks {
            let Some(p) = self.part(name) else { continue };
            let a = Pose::of_row(&rows[i]);
            let b = rows.get(i + 1).map_or(a, |r| Pose::of_row(r));
            out[p] = Pose::lerp(a, b, f);
        }
        out
    }

    /// Ticks into the walk clip for vanilla's `walk_position`.
    pub fn walk_ticks(&self, clip: &str, walk_position: f32) -> f32 {
        let ticks = self.clips.get(clip).map_or(1, |c| c.ticks) as f32;
        walk_position / WALK_PERIOD * ticks
    }

    pub fn has_clip(&self, clip: &str) -> bool {
        self.clips.contains_key(clip)
    }

    pub fn clip_ticks(&self, clip: &str) -> Option<u32> {
        self.clips.get(clip).map(|c| c.ticks)
    }
}

/// Where the model's origin is over the feet, in pixels (`translate(0, -1.501, 0)`).
pub const ORIGIN_ABOVE_FEET_PX: f32 = 24.016;

/// The box round a rig's cubes in the world frame, in blocks from the point between the feet:
/// `min`/`max` of x (the mob's left), y (up from the feet) and z (forward).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    pub fn size(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.max[i] - self.min[i])
    }
}

/// An affine map: a 3×3 matrix by rows, then the translation.
type Affine = ([[f32; 3]; 3], [f32; 3]);

fn mul(a: &Affine, b: &Affine) -> Affine {
    let m = std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a.0[r][k] * b.0[k][c]).sum()));
    let t = std::array::from_fn(|r| a.1[r] + (0..3).map(|k| a.0[r][k] * b.1[k]).sum::<f32>());
    (m, t)
}

fn apply(a: &Affine, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| a.1[r] + (0..3).map(|k| a.0[r][k] * p[k]).sum::<f32>())
}

/// `ModelPart.translateAndRotate` of one pose, in pixels: to the place, then Z, Y, X, then scale.
fn part_affine(p: &Pose) -> Affine {
    let (sx, cx) = p.rot[0].sin_cos();
    let (sy, cy) = p.rot[1].sin_cos();
    let (sz, cz) = p.rot[2].sin_cos();
    let rz: Affine = ([[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]], [0.0; 3]);
    let ry: Affine = ([[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]], [0.0; 3]);
    let rx: Affine = ([[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]], [0.0; 3]);
    let s = p.scale;
    let sc: Affine = ([[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]], [0.0; 3]);
    let t: Affine = ([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], p.pos);
    mul(&mul(&mul(&mul(&t, &rz), &ry), &rx), &sc)
}

impl Rig {
    /// The box round every cube shown with `flags` in these poses (a clip frame, or the rest
    /// pose from [`Rig::rest`]), inflation included, in blocks over the feet. Glowing and
    /// one-faced cubes count too: they are part of the silhouette.
    pub fn bounds(&self, poses: &[Pose], flags: &[&str]) -> Option<Bounds> {
        self.bounds_where(poses, |i| self.visible(i, flags))
    }

    /// The box round part `i`'s own cubes in the rest pose (the head's: where the eyes are).
    pub fn bounds_of_part(&self, i: usize) -> Option<Bounds> {
        self.bounds_where(&self.rest_poses(), |j| j == i)
    }

    fn bounds_where(&self, poses: &[Pose], counts: impl Fn(usize) -> bool) -> Option<Bounds> {
        let mut world: Vec<Affine> = Vec::with_capacity(self.parts.len());
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for (i, part) in self.parts.iter().enumerate() {
            let own = part_affine(&poses[i]);
            let m = match self.parents[i] {
                Some(p) => mul(&world[p], &own),
                None => own,
            };
            world.push(m);
            if !counts(i) {
                continue;
            }
            for c in &part.cubes {
                let g = c.grow();
                let a: [f32; 3] = std::array::from_fn(|k| c.origin[k] - g[k]);
                let b: [f32; 3] = std::array::from_fn(|k| c.origin[k] + c.size[k] + g[k]);
                for n in 0..8 {
                    let corner = [
                        if n & 1 == 0 { a[0] } else { b[0] },
                        if n & 2 == 0 { a[1] } else { b[1] },
                        if n & 4 == 0 { a[2] } else { b[2] },
                    ];
                    let q = apply(&world[i], corner);
                    // Model pixels (+x left, +y down, −z front) to blocks over the feet.
                    let w = [q[0] / 16.0, (ORIGIN_ABOVE_FEET_PX - q[1]) / 16.0, -q[2] / 16.0];
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
            }
        }
        lo[0].is_finite().then_some(Bounds { min: lo, max: hi })
    }

    /// The rest pose of every part, for [`Rig::bounds`].
    pub fn rest_poses(&self) -> Vec<Pose> {
        (0..self.parts.len()).map(|i| self.rest(i)).collect()
    }
}

/// Blends two pose sets: `b` over `a` by `f` (0 is `a`). For a short crossfade between clips.
pub fn blend(a: &[Pose], b: &[Pose], f: f32) -> Vec<Pose> {
    a.iter().zip(b).map(|(x, y)| Pose::lerp(*x, *y, f.clamp(0.0, 1.0))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(id: &str) -> Rig {
        let path = format!("{}/../../../assets/rigs/{id}.rig.json", env!("CARGO_MANIFEST_DIR"));
        Rig::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_rig_reads() {
        for id in ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"] {
            let r = read(id);
            assert_eq!(r.id, id);
            assert_eq!(r.texture.sprite, format!("soulscraft:entity/{id}"));
            assert!(r.parents[0].is_none());
        }
    }

    #[test]
    fn revenant_markers_match_mobs_json() {
        use crate::attack::Timeline;
        use crate::data::MobsFile;
        let f = MobsFile::embedded();
        for m in &f.mobs {
            let r = read(&m.id);
            for (_, a) in m.phased_attacks() {
                let clip = &r.clips[&a.clip];
                let want: BTreeMap<String, u32> =
                    Timeline::of(a).markers().into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
                assert_eq!(clip.markers, want, "{} {}", m.id, a.id);
                assert_eq!(clip.ticks, Timeline::of(a).end(), "{} {}", m.id, a.id);
            }
        }
    }

    #[test]
    fn sampling_interpolates_wraps_and_holds() {
        let r = read("revenant");
        let arm = r.part("arm_r").unwrap();
        let clip = &r.clips["attack_overhead"];
        let rows = &clip.tracks["arm_r"];
        let at18 = r.sample("attack_overhead", 18.0)[arm];
        assert_eq!(at18, Pose::of_row(&rows[18]));
        let mid = r.sample("attack_overhead", 18.5)[arm];
        let want = Pose::lerp(Pose::of_row(&rows[18]), Pose::of_row(&rows[19]), 0.5);
        assert_eq!(mid, want);
        assert_eq!(r.sample("attack_overhead", 99.0)[arm], Pose::of_row(&rows[36]));
        let walk = r.clips["walk"].ticks as f32;
        let a = r.sample("walk", 3.25);
        let b = r.sample("walk", 3.25 + walk);
        for (x, y) in a.iter().zip(&b) {
            for k in 0..3 {
                assert!((x.rot[k] - y.rot[k]).abs() < 1e-4);
            }
        }
        assert_eq!(r.sample("no_such_clip", 3.0)[arm], r.rest(arm));
        assert!((r.walk_ticks("walk", WALK_PERIOD) - walk).abs() < 1e-4);
    }

    #[test]
    fn chains_sockets_and_states() {
        let r = read("revenant");
        let arm = r.part("arm_r").unwrap();
        let names: Vec<&str> = r.chain(arm).iter().map(|&i| r.parts[i].name.as_str()).collect();
        assert_eq!(names, ["root", "body", "arm_r"]);
        let hand = r.sockets.iter().find(|s| s.name == "socket_hand_r").unwrap();
        assert_eq!(hand.item.as_deref(), Some("soulscraft:rusty_sword"));
        assert_eq!(hand.rotate, vec![("x".to_owned(), -90.0), ("y".to_owned(), 180.0)]);
        let b = read("brute");
        let eyes = b.parts.iter().position(|p| p.state.as_deref() == Some("rage")).unwrap();
        assert!(!b.visible(eyes, &[]));
        assert!(b.visible(eyes, &["rage"]));
        if let Some(calm) = b.parts.iter().position(|p| p.state.as_deref() == Some("!rage")) {
            assert!(b.visible(calm, &[]) && !b.visible(calm, &["rage"]));
        }
    }

    #[test]
    fn inflate_reads_one_or_three_numbers() {
        let c: CubeDef = serde_json::from_str(r#"{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0],"inflate":0.5}"#).unwrap();
        assert_eq!(c.grow(), [0.5; 3]);
        let c: CubeDef =
            serde_json::from_str(r#"{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0],"inflate":[0.1,0,0.2]}"#).unwrap();
        assert_eq!(c.grow(), [0.1, 0.0, 0.2]);
        let c: CubeDef = serde_json::from_str(r#"{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0]}"#).unwrap();
        assert_eq!(c.grow(), [0.0; 3]);
    }

    #[test]
    fn bad_rigs_are_refused() {
        let base = r#"{"format":"soulscraft-rig","version":2,"id":"x","texture":{"sprite":"s","width":64,"height":64},
            "parts":[{"name":"root","parent":null,"pivot":[0,24,0]},{"name":"a","parent":"root","pivot":[0,0,0]}]"#;
        assert!(Rig::parse(&format!("{base}}}")).is_ok());
        let orphan = base.replace("\"parent\":\"root\"", "\"parent\":\"later\"");
        assert!(Rig::parse(&format!("{orphan}}}")).unwrap_err().contains("parent"));
        let clip = format!(r#"{base},"clips":{{"c":{{"ticks":2,"tracks":{{"a":[[0,0,0,0,0,0]]}}}}}}}}"#);
        assert!(Rig::parse(&clip).unwrap_err().contains("rows"));
        let ghost = format!(r#"{base},"clips":{{"c":{{"ticks":0,"tracks":{{"b":[[0,0,0,0,0,0]]}}}}}}}}"#);
        assert!(Rig::parse(&ghost).unwrap_err().contains("no part"));
        let sock = format!(r#"{base},"sockets":[{{"name":"s","parent":"zz","offset":[0,0,0]}}]}}"#);
        assert!(Rig::parse(&sock).unwrap_err().contains("socket"));
        assert!(Rig::parse(&format!("{}}}", base.replace("soulscraft-rig", "x"))).unwrap_err().contains("format"));
        assert!(Rig::parse("[]").is_err());
    }

    #[test]
    fn bounds_of_one_cube_turned_and_inflated() {
        let text = r#"{"format":"soulscraft-rig","version":2,"id":"x","texture":{"sprite":"s","width":64,"height":64},
            "parts":[{"name":"root","parent":null,"pivot":[0,24,0]},
                     {"name":"a","parent":"root","pivot":[0,-8,0],"rotation":[0,1.5707964,0],
                      "cubes":[{"origin":[-4,-8,-2],"size":[8,8,4],"uv":[0,0],"inflate":0.5}]}]}"#;
        let r = Rig::parse(text).unwrap();
        let b = r.bounds(&r.rest_poses(), &[]).unwrap();
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        // A quarter turn about y swaps the cube's width (9 px with inflation) and depth (5 px).
        assert!(near(b.size()[0], 5.0 / 16.0) && near(b.size()[2], 9.0 / 16.0), "{b:?}");
        // From 16.5 px to 24.5 px up the model: 0.016 px over the feet is the origin's lift.
        assert!(near(b.max[1], (24.016 - 7.5) / 16.0) && near(b.min[1], (24.016 - 16.5) / 16.0), "{b:?}");
    }

    #[test]
    fn hidden_parts_leave_the_bounds() {
        let b = read("brute");
        let rest = b.rest_poses();
        let calm = b.bounds(&rest, &[]).unwrap();
        let rage = b.bounds(&rest, &["rage"]).unwrap();
        assert!(calm.size()[1] > 2.0 && rage.size()[1] >= calm.size()[1] - 0.01);
    }

    /// data/README.md, «Рамка столкновений»: each mob's box against its rig's rest pose.
    #[test]
    fn hitboxes_follow_the_rigs() {
        use crate::data::MobsFile;
        for m in &MobsFile::embedded().mobs {
            let h = m.hitbox.unwrap_or_else(|| panic!("{} has no hitbox", m.id));
            let r = read(&m.id);
            let b = r.bounds(&r.rest_poses(), &[]).unwrap();
            // The top of the parts at least 0.2 block across both ways: a lure's stalk or a
            // crest of a few pixels is not body.
            let top = (0..r.parts.len())
                .filter(|&i| r.visible(i, &[]))
                .filter_map(|i| r.bounds_of_part(i))
                .filter(|p| p.size()[0].min(p.size()[2]) >= 0.2)
                .map(|p| f64::from(p.max[1]))
                .fold(0.0, f64::max);
            let walker = m.env == "land" && !m.elite;
            // The box reaches the model's top within 0.05, or stops at 1.95 for a walker that
            // must pass a door 1 × 2.
            let capped = walker && h.height == 1.95 && top > 1.95;
            assert!(capped || (h.height - top).abs() <= 0.05, "{}: height {} vs rig top {top:.3}", m.id, h.height);
            assert!(h.width <= f64::from(b.size()[0].max(b.size()[2])) + 0.05, "{}: wider than the rig", m.id);
            if walker {
                assert!(h.width < 1.0, "{}: a walker under 1 block wide paths through a 1-wide gap", m.id);
            }
            assert!(h.eye < h.height && h.eye >= 0.4 * h.height, "{}: eyes at {}", m.id, h.eye);
        }
    }

    /// The goal hook (compiled where the rules crate is not) carries the same boxes as the data.
    #[test]
    fn the_goal_hook_boxes_match_the_data() {
        use crate::data::MobsFile;
        let path = format!("{}/../hook_register_goals.rs", env!("CARGO_MANIFEST_DIR"));
        let Ok(hook) = std::fs::read_to_string(path) else { return };
        for m in &MobsFile::embedded().mobs {
            let h = m.hitbox.unwrap();
            let row = format!("(\"{}\", {:?}, {:?}, {:?})", m.key, h.width, h.height, h.eye);
            assert!(hook.contains(&row), "hook_register_goals.rs lacks {row}");
        }
    }

    /// The goal hook clears `flags.undead` on exactly the data's living types.
    #[test]
    fn the_goal_hook_undead_matches_the_data() {
        use crate::data::MobsFile;
        let path = format!("{}/../hook_register_goals.rs", env!("CARGO_MANIFEST_DIR"));
        let Ok(hook) = std::fs::read_to_string(path) else { return };
        let living: Vec<String> =
            MobsFile::embedded().mobs.iter().filter(|m| !m.undead).map(|m| format!("\"{}\"", m.key)).collect();
        let row = format!("const LIVING: [&str; {}] = [{}];", living.len(), living.join(", "));
        assert!(hook.contains(&row), "hook_register_goals.rs lacks {row}");
    }

    /// `cargo test print_rig_bounds -- --ignored --nocapture`: the numbers data/README.md's
    /// `hitbox` rule is applied to.
    #[test]
    #[ignore]
    fn print_rig_bounds() {
        for id in ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"] {
            let r = read(id);
            let b = r.bounds(&r.rest_poses(), &[]).unwrap();
            let idle = r.has_clip("idle").then(|| r.bounds(&r.sample("idle", 0.0), &[]).unwrap());
            let head = r.part("head").map(|h| r.bounds_of_part(h));
            println!("{id}: rest {b:?} size {:?} idle {:?} head {head:?}", b.size(), idle.map(|i| i.size()));
            for (i, p) in r.parts.iter().enumerate() {
                if let Some(pb) = r.bounds_of_part(i) {
                    println!("    {} {:?} size {:?}", p.name, pb, pb.size());
                }
            }
        }
    }

    #[test]
    fn blend_mixes_poses() {
        let a = [Pose { pos: [0.0; 3], rot: [0.0; 3], scale: [1.0; 3] }];
        let b = [Pose { pos: [2.0; 3], rot: [1.0; 3], scale: [0.0; 3] }];
        let m = blend(&a, &b, 0.5);
        assert_eq!(m[0].pos, [1.0; 3]);
        assert_eq!(m[0].scale, [0.5; 3]);
        assert_eq!(blend(&a, &b, 7.0)[0], b[0]);
    }
}
