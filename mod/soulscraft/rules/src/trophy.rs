//! Trophies in the world (docs/design/09-trophies.md; the file `assets/rigs/<mob>.trophy.json`,
//! format in assets/models/README.md, «Трофеи»): the head blocks on the floor (16 rotations) and
//! on a wall, the diorama block that faces the player, the figure's clip phase, and the particles
//! a diorama sends (one in a random point of each box with chance `1 / every` a tick).
//!
//! Frames: a box is in pixels of the block model facing north (x east, y up, z south, 0..16);
//! a blockstate's `y` turns the model clockwise seen from above, as mod/sync.ps1 `Turn-Box` does.

use crate::rig::Rig;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct TrophyFile {
    pub format: String,
    pub version: u32,
    /// The mob's short id: `revenant`.
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The mob's rig: `soulscraft:revenant`.
    pub rig: String,
    pub head: HeadDef,
    pub diorama: DioramaDef,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HeadDef {
    /// The rig texture, as in rig.json.
    pub texture: Value,
    /// The head's parts in rig.json's format.
    pub parts: Value,
    pub floor: Place,
    pub wall: Place,
}

/// Where a figure or a head stands: scale, offset in pixels, turn in degrees.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Place {
    pub scale: f32,
    pub offset: [f32; 3],
    pub yaw: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DioramaDef {
    /// `soulscraft:revenant_trophy`.
    pub block: String,
    pub clip: String,
    pub ticks: u32,
    #[serde(default, rename = "loop")]
    pub looped: bool,
    #[serde(default)]
    pub flags: Vec<String>,
    pub scale: f32,
    pub offset: [f32; 3],
    pub yaw: f32,
    pub light: u8,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub plaque: Option<PxBox>,
    #[serde(default)]
    pub particles: Vec<Emitter>,
}

/// A box in model pixels, facing north.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct PxBox {
    pub from: [f64; 3],
    pub to: [f64; 3],
}

/// One kind of particle a diorama sends: on average one each `every` ticks, in `area`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Emitter {
    /// `soulscraft:mist`.
    #[serde(rename = "type")]
    pub kind: String,
    pub every: u32,
    #[serde(rename = "box")]
    pub area: PxBox,
}

impl TrophyFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: Self = serde_json::from_str(text).map_err(|e| format!("trophy: {e}"))?;
        if f.format != "soulscraft-trophy" || f.version != 2 {
            return Err(format!("trophy {}: format {} version {}", f.id, f.format, f.version));
        }
        if f.diorama.block != format!("soulscraft:{}_trophy", f.id) {
            return Err(format!("trophy {}: diorama block {}", f.id, f.diorama.block));
        }
        if f.diorama.light > 15 {
            return Err(format!("trophy {}: light {} over 15", f.id, f.diorama.light));
        }
        if let Some(e) = f.diorama.particles.iter().find(|e| e.every == 0 || !inside_block(&e.area)) {
            return Err(format!("trophy {}: particles {} every 0 or out of the block", f.id, e.kind));
        }
        f.head_rig()?;
        Ok(f)
    }

    /// The head as a rig of its own (rest pose, no clips), drawn as a mob's parts are.
    pub fn head_rig(&self) -> Result<Rig, String> {
        let text = serde_json::json!({
            "format": "soulscraft-rig",
            "version": 2,
            "id": format!("{}_head", self.id),
            "texture": self.head.texture,
            "parts": self.head.parts,
        });
        Rig::parse(&text.to_string())
    }

    /// The rig's short id (`soulscraft:revenant` → `revenant`).
    pub fn rig_id(&self) -> &str {
        self.rig.strip_prefix("soulscraft:").unwrap_or(&self.rig)
    }
}

/// The crate's copy of a mob's trophy file (mod/sync.ps1 writes `data/trophy/<mob>.json`): the
/// server sends the diorama's particles from it.
pub fn embedded_text(mob: &str) -> Option<&'static str> {
    macro_rules! file {
        ($id:literal) => {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/trophy/", $id, ".json"))
        };
    }
    Some(match mob {
        "revenant" => file!("revenant"),
        "stalker" => file!("stalker"),
        "marksman" => file!("marksman"),
        "brute" => file!("brute"),
        "shrike" => file!("shrike"),
        "lurker" => file!("lurker"),
        "bog_slime" => file!("bog_slime"),
        _ => return None,
    })
}

pub fn embedded(mob: &str) -> Option<TrophyFile> {
    TrophyFile::parse(embedded_text(mob)?).ok()
}

fn inside_block(b: &PxBox) -> bool {
    (0..3).all(|i| (0.0..=16.0).contains(&b.from[i]) && (0.0..=16.0).contains(&b.to[i]) && b.from[i] <= b.to[i])
}

/// Which trophy block a block id is, and of which mob.
///
/// The head blocks are `<mob>_head_stand` and `<mob>_head_mount`, not `<mob>_head`: Horizon gives
/// any block whose name ends in `_head` or `_skull` vanilla's skull block entity, before a mod is
/// asked (horizon-client-render/src/containers.rs:265-279, 389). The item is `<mob>_head`
/// (`place_block` puts down the stand).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind<'a> {
    /// `soulscraft:<mob>_head_stand`, on the floor (`rotation` 0..15).
    Head(&'a str),
    /// `soulscraft:<mob>_head_mount`, on a wall (`facing`).
    WallHead(&'a str),
    /// `soulscraft:<mob>_trophy`, the diorama (`facing`).
    Diorama(&'a str),
}

impl<'a> Kind<'a> {
    pub fn of(block: &'a str) -> Option<Self> {
        let id = block.strip_prefix("soulscraft:")?;
        if let Some(m) = id.strip_suffix("_head_mount") {
            Some(Kind::WallHead(m))
        } else if let Some(m) = id.strip_suffix("_head_stand") {
            Some(Kind::Head(m))
        } else {
            id.strip_suffix("_trophy").map(Kind::Diorama)
        }
    }

    /// The block id of this kind for a mob.
    pub fn block(self) -> String {
        match self {
            Kind::Head(m) => format!("soulscraft:{m}_head_stand"),
            Kind::WallHead(m) => format!("soulscraft:{m}_head_mount"),
            Kind::Diorama(m) => format!("soulscraft:{m}_trophy"),
        }
    }

    pub fn mob(self) -> &'a str {
        match self {
            Kind::Head(m) | Kind::WallHead(m) | Kind::Diorama(m) => m,
        }
    }
}

/// The mob of a head item (soulscraft:<mob>_head).
pub fn head_item_mob(item: &str) -> Option<&str> {
    item.strip_prefix("soulscraft:")?.strip_suffix("_head").filter(|m| !m.is_empty())
}

/// A horizontal facing's turn about y, degrees clockwise seen from above (the blockstate's `y`).
pub fn facing_y(facing: &str) -> f32 {
    match facing {
        "east" => 90.0,
        "south" => 180.0,
        "west" => 270.0,
        _ => 0.0,
    }
}

/// The floor head's `rotation` for a placer looking along `yaw` (vanilla's `SkullBlock`:
/// `RotationSegment.convertToSegment(yaw + 180)`), so the face looks back at the placer.
pub fn floor_rotation(yaw: f32) -> u8 {
    let s = ((yaw + 180.0) * 16.0 / 360.0 + 0.5).floor() as i32;
    s.rem_euclid(16) as u8
}

/// The facing of a block that turns its front to the placer (vanilla's
/// `getHorizontalDirection().getOpposite()`): `Direction.fromYRot(yaw)` is south at 0, west at 90.
pub fn facing_toward(yaw: f32) -> &'static str {
    match ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    }
}

/// The wall head's facing on a clicked side face (`[1, 0, 0]` is east): the head looks out of the
/// wall. None for the top and bottom faces.
pub fn wall_facing(face: [i32; 3]) -> Option<&'static str> {
    match face {
        [0, 0, -1] => Some("north"),
        [0, 0, 1] => Some("south"),
        [1, 0, 0] => Some("east"),
        [-1, 0, 0] => Some("west"),
        _ => None,
    }
}

/// The turn the drawing chain starts with (README «Трофеи»): a floor head −22.5 × rotation, a wall
/// head and a diorama −facing_y.
pub fn turn(kind: Kind<'_>, rotation: u8, facing: &str) -> f32 {
    match kind {
        Kind::Head(_) => -22.5 * f32::from(rotation),
        Kind::WallHead(_) | Kind::Diorama(_) => -facing_y(facing),
    }
}

/// A box of the model facing north, turned as the blockstate turns it for `facing`, in blocks
/// (0..1) from the block's corner: each quarter turn takes (x, z) to (1 − z, x).
pub fn turn_box(b: &PxBox, facing: &str) -> ([f64; 3], [f64; 3]) {
    let mut a = [b.from[0] / 16.0, b.from[2] / 16.0];
    let mut c = [b.to[0] / 16.0, b.to[2] / 16.0];
    for _ in 0..(facing_y(facing) as i32 / 90) {
        a = [1.0 - a[1], a[0]];
        c = [1.0 - c[1], c[0]];
    }
    (
        [a[0].min(c[0]), b.from[1] / 16.0, a[1].min(c[1])],
        [a[0].max(c[0]), b.to[1] / 16.0, a[1].max(c[1])],
    )
}

/// One particle point in the world: the block at `pos`, the box turned by `facing`, and three
/// numbers from 0 up to 1.
pub fn particle_point(pos: [i32; 3], b: &PxBox, facing: &str, u: [f64; 3]) -> [f64; 3] {
    let (lo, hi) = turn_box(b, facing);
    std::array::from_fn(|i| f64::from(pos[i]) + lo[i] + (hi[i] - lo[i]) * u[i])
}

/// Whether an emitter sends this tick: `unit` from 0 up to 1 under `1 / every`.
pub fn due(unit: f64, every: u32) -> bool {
    every > 0 && unit < 1.0 / f64::from(every)
}

/// A figure's tick in its looping clip: `game_time` shifted by a phase from the block's place, so
/// a row of dioramas does not breathe in step (09-trophies.md, «Трофейные позы»).
pub fn clip_tick(pos: [i32; 3], game_time: i64, partial: f32, ticks: u32) -> f32 {
    let ticks = ticks.max(1);
    let h = (pos[0] as i64).wrapping_mul(73_856_093) ^ (pos[1] as i64).wrapping_mul(19_349_663) ^ (pos[2] as i64).wrapping_mul(83_492_791);
    let phase = h.rem_euclid(i64::from(ticks));
    (game_time + phase).rem_euclid(i64::from(ticks)) as f32 + partial
}

/// The plaque's lines (09-trophies.md, «Подставка»): the mob's name, then the kills of the player
/// looking at it from their hunter's book (a spawner's mob counts a part, so the count rounds down).
/// No second line until the client has its book.
pub fn plaque_lines(name: &str, kills: Option<f64>) -> Vec<String> {
    let mut out = vec![name.to_owned()];
    if let Some(k) = kills {
        out.push(format!("убито вами: {}", k.max(0.0).floor() as u64));
    }
    out
}

/// Model pixels per font pixel for a line `width` font pixels wide on a plaque `plaque` wide and
/// tall in model pixels, with `lines` lines: as large as the plaque lets the line be, with a margin
/// of a tenth of its width and a line of 9 font pixels (8 for the glyph, 1 between).
pub fn plaque_scale(width: f32, plaque: [f32; 2], lines: usize) -> f32 {
    let by_width = if width > 0.0 { plaque[0] * 0.9 / width } else { f32::INFINITY };
    let by_height = plaque[1] / (9.0 * lines.max(1) as f32);
    by_width.min(by_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plaque_shows_the_name_and_the_viewers_kills() {
        assert_eq!(plaque_lines("Громила", None), ["Громила"]);
        assert_eq!(plaque_lines("Громила", Some(12.75)), ["Громила", "убито вами: 12"]);
        assert_eq!(plaque_lines("Громила", Some(0.25)), ["Громила", "убито вами: 0"]);
    }

    #[test]
    fn plaque_text_fits_its_width_and_its_height() {
        // Two lines on the 8 × 1.5 pixel plaque: a short line is held by the height.
        assert_eq!(plaque_scale(20.0, [8.0, 1.5], 2), 1.5 / 18.0);
        // A long one by the width: 0.9 of the plaque.
        let s = plaque_scale(100.0, [8.0, 1.5], 2);
        assert!((s * 100.0 - 7.2).abs() < 1e-5);
        assert!(plaque_scale(0.0, [8.0, 1.5], 1).is_finite());
    }

    pub(crate) const MOBS: [&str; 7] = ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"];

    fn file(id: &str) -> TrophyFile {
        let path = format!("{}/../../../assets/rigs/{id}.trophy.json", env!("CARGO_MANIFEST_DIR"));
        TrophyFile::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_trophy_file_reads_and_its_head_is_a_rig() {
        for id in MOBS {
            let f = file(id);
            assert_eq!(f.rig_id(), id);
            assert_eq!(f.diorama.clip, "trophy");
            assert_eq!(f.diorama.ticks, 80);
            assert!(f.diorama.looped);
            assert!((1..=5).contains(&f.diorama.light), "{id}");
            assert!(!f.diorama.particles.is_empty(), "{id}");
            let head = f.head_rig().unwrap();
            assert!(!head.parts.is_empty());
            assert!(head.parents[0].is_none());
            assert!(f.diorama.particles.iter().all(|e| e.kind.starts_with("soulscraft:")));
        }
        let r = file("revenant");
        assert_eq!(r.diorama.particles[0].every, 6);
        assert_eq!(r.diorama.light, 4);
        assert_eq!(r.head.floor.scale, 0.9);
        assert_eq!(file("brute").diorama.flags, ["rage"]);
    }

    #[test]
    fn every_mob_has_an_embedded_trophy() {
        for m in crate::data::MobsFile::embedded().mobs {
            let f = embedded(&m.id).unwrap_or_else(|| panic!("{}", m.id));
            assert_eq!(f.id, m.id);
            assert_eq!(f.diorama.particles, file(&m.id).diorama.particles);
        }
        assert!(embedded("zombie").is_none());
    }

    #[test]
    fn bad_files_are_refused() {
        let path = format!("{}/../../../assets/rigs/revenant.trophy.json", env!("CARGO_MANIFEST_DIR"));
        let good = std::fs::read_to_string(path).unwrap();
        assert!(TrophyFile::parse(&good.replace("soulscraft:revenant_trophy", "soulscraft:x")).unwrap_err().contains("block"));
        assert!(TrophyFile::parse(&good.replace("\"light\": 4", "\"light\": 16")).unwrap_err().contains("light"));
        assert!(TrophyFile::parse(&good.replace("\"every\": 6", "\"every\": 0")).unwrap_err().contains("every"));
        assert!(TrophyFile::parse(&good.replace("\"version\": 2", "\"version\": 1")).unwrap_err().contains("version"));
        assert!(TrophyFile::parse("{").is_err());
    }

    #[test]
    fn kinds_by_block_id() {
        assert_eq!(Kind::of("soulscraft:revenant_head_stand"), Some(Kind::Head("revenant")));
        assert_eq!(Kind::of("soulscraft:bog_slime_head_mount"), Some(Kind::WallHead("bog_slime")));
        assert_eq!(Kind::of("soulscraft:bog_slime_trophy"), Some(Kind::Diorama("bog_slime")));
        assert_eq!(Kind::of("soulscraft:altar_core"), None);
        assert_eq!(Kind::of("minecraft:zombie_head"), None);
        // The item is no block: the engine would make a vanilla skull of a block named *_head.
        assert_eq!(Kind::of("soulscraft:revenant_head"), None);
        assert_eq!(Kind::of("soulscraft:brute_trophy").unwrap().mob(), "brute");
        for k in [Kind::Head("lurker"), Kind::WallHead("lurker"), Kind::Diorama("lurker")] {
            let b = k.block();
            assert_eq!(Kind::of(&b), Some(k));
            assert!(!b.ends_with("_head") && !b.ends_with("_skull"), "{b}");
        }
        assert_eq!(head_item_mob("soulscraft:brute_head"), Some("brute"));
        assert_eq!(head_item_mob("minecraft:zombie_head"), None);
        assert_eq!(head_item_mob("soulscraft:_head"), None);
    }

    #[test]
    fn placement_faces_the_placer() {
        // Looking south (yaw 0): the head's face looks north, at the placer (vanilla: rotation 8).
        assert_eq!(floor_rotation(0.0), 8);
        assert_eq!(floor_rotation(180.0), 0);
        assert_eq!(floor_rotation(-180.0), 0);
        assert_eq!(floor_rotation(90.0), 12);
        assert_eq!(floor_rotation(11.0), 8);
        assert_eq!(floor_rotation(12.0), 9);
        assert_eq!(floor_rotation(-90.0), 4);
        // All 16 come out.
        let all: std::collections::BTreeSet<u8> = (0..360).map(|y| floor_rotation(y as f32)).collect();
        assert_eq!(all.len(), 16);
        // A diorama turns its plaque (north in the model) to the placer.
        assert_eq!(facing_toward(0.0), "north");
        assert_eq!(facing_toward(90.0), "east");
        assert_eq!(facing_toward(180.0), "south");
        assert_eq!(facing_toward(-90.0), "west");
        assert_eq!(facing_toward(44.0), "north");
        assert_eq!(facing_toward(46.0), "east");
        assert_eq!(wall_facing([1, 0, 0]), Some("east"));
        assert_eq!(wall_facing([0, 0, -1]), Some("north"));
        assert_eq!(wall_facing([0, 0, 1]), Some("south"));
        assert_eq!(wall_facing([-1, 0, 0]), Some("west"));
        assert_eq!(wall_facing([0, 1, 0]), None);
        assert_eq!(turn(Kind::Head("x"), 4, "north"), -90.0);
        assert_eq!(turn(Kind::WallHead("x"), 0, "east"), -90.0);
        assert_eq!(turn(Kind::Diorama("x"), 0, "west"), -270.0);
    }

    #[test]
    fn particle_boxes_turn_with_the_block() {
        let b = PxBox { from: [2.0, 4.0, 1.0], to: [6.0, 5.0, 3.0] };
        let (lo, hi) = turn_box(&b, "north");
        assert_eq!((lo, hi), ([0.125, 0.25, 0.0625], [0.375, 0.3125, 0.1875]));
        // East: a quarter turn clockwise from above (x, z) → (1 − z, x).
        let (lo, hi) = turn_box(&b, "east");
        assert_eq!((lo, hi), ([0.8125, 0.25, 0.125], [0.9375, 0.3125, 0.375]));
        let (lo, hi) = turn_box(&b, "south");
        assert_eq!((lo, hi), ([0.625, 0.25, 0.8125], [0.875, 0.3125, 0.9375]));
        let (lo, _) = turn_box(&b, "west");
        assert_eq!(lo, [0.0625, 0.25, 0.625]);
        let p = particle_point([10, 64, -3], &b, "north", [0.0, 0.5, 1.0]);
        assert_eq!(p, [10.125, 64.28125, -3.0 + 0.1875]);
        // Every point of every file stays in its block at every facing.
        for id in MOBS {
            for e in file(id).diorama.particles {
                for f in ["north", "east", "south", "west"] {
                    let (lo, hi) = turn_box(&e.area, f);
                    assert!((0..3).all(|i| lo[i] >= 0.0 && hi[i] <= 1.0 && lo[i] <= hi[i]), "{id} {f}");
                }
            }
        }
    }

    #[test]
    fn particles_come_at_their_rate() {
        // A seeded stream: one in `every` ticks on average.
        let mut rng = crate::event::Rng::new(7);
        for every in [3u32, 6, 12, 20] {
            let n = 20_000;
            let sent = (0..n).filter(|_| due(rng.unit(), every)).count() as f64;
            let want = f64::from(n) / f64::from(every);
            assert!((sent - want).abs() < want * 0.08, "every {every}: {sent} for {want}");
        }
        assert!(!due(0.0, 0));
        assert!(due(0.0, 1) && due(0.999, 1));
    }

    #[test]
    fn the_clip_phase_differs_by_place_and_loops() {
        let a = clip_tick([0, 64, 0], 100, 0.25, 80);
        let b = clip_tick([1, 64, 0], 100, 0.25, 80);
        assert_ne!(a, b);
        assert!((0.0..80.25).contains(&a));
        assert_eq!(clip_tick([5, 6, 7], 10, 0.0, 80), clip_tick([5, 6, 7], 90, 0.0, 80));
        assert_eq!(clip_tick([5, 6, 7], 10, 0.5, 80) - clip_tick([5, 6, 7], 10, 0.0, 80), 0.5);
        assert!(clip_tick([-5, 6, -7], -3, 0.0, 0) >= 0.0);
    }
}
