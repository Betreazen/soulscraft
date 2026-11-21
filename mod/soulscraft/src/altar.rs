//! The summoning altar (docs/design/06-altar.md, data/altar.json): which block stands on which
//! cell of each ring, the level a structure reaches, the banners on its sockets and what they add
//! up to (hypothesis Г10), the tall pieces made of several blocks, and the facing of a banner.
//!
//! Cells are in blocks from the core, x east and z south; the floor (slabs, sockets, the core)
//! is y = 0 and the pieces stand on it from y = 1 (data/altar.json `frame`). The server reads the
//! world into [`Seen`] cells and asks [`evaluate`]; it never scans more than the 9 × 9 × 4 cells
//! of one altar, and only when a block inside it changed.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The copy of data/altar.json the crate was built with (mod/sync.ps1 writes it).
pub const EMBEDDED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/soulscraft/altar.json"));

pub const CORE: &str = "soulscraft:altar_core";
pub const PEDESTAL: &str = "soulscraft:altar_pedestal";
pub const SLAB: &str = "soulscraft:altar_slab";
pub const SOCKET: &str = "soulscraft:altar_socket";
pub const PILLAR: &str = "soulscraft:altar_pillar";
pub const BRAZIER: &str = "soulscraft:altar_brazier";
pub const OBELISK: &str = "soulscraft:altar_obelisk";
pub const BANNER_PREFIX: &str = "soulscraft:banner_";
pub const AIR: &str = "minecraft:air";
/// The highest ring (level 4, 9 × 9).
pub const MAX_RING: u32 = 4;
/// The layers a structure has: the floor and three above it (the obelisk is 3 tall).
pub const LAYERS: i32 = 4;

#[derive(Clone, Debug, Deserialize)]
pub struct AltarFile {
    pub format: String,
    pub levels: Vec<LevelDef>,
    pub pedestal_item: PedestalItem,
    pub modifiers: Vec<ModifierDef>,
    /// Banners on the sockets of the sample altars (Blender, the showcase, `/sc altar N banners`).
    #[serde(default)]
    pub sample_banners: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LevelDef {
    pub level: u32,
    pub size: i32,
    /// The piece on the ring's four corners (`altar_pillar`), none on ring 1.
    pub corner: Option<String>,
    /// Socket cells of this ring, `[x, z]`.
    #[serde(default)]
    pub sockets: Vec<[i32; 2]>,
    #[serde(default)]
    pub unlocks: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct PedestalItem {
    pub height_px: f64,
    pub scale: f64,
    pub spin_deg_per_tick: f64,
    pub bob_px: f64,
    pub bob_ticks: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ModifierDef {
    pub id: String,
    /// The banner block's short id (`banner_weight`).
    pub banner: String,
    pub name: String,
    #[serde(default)]
    pub short: String,
    pub effect: Map<String, Value>,
    #[serde(default)]
    pub text: String,
    /// `more`, `less` or `same` reward.
    pub reward: String,
    #[serde(default)]
    pub opposite: Option<String>,
}

impl AltarFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: Self = serde_json::from_str(text).map_err(|e| format!("altar.json: {e}"))?;
        if f.format != "soulscraft-altar" {
            return Err(format!("altar.json: format {:?}, not soulscraft-altar", f.format));
        }
        if f.levels.len() != MAX_RING as usize {
            return Err(format!("altar.json: {} levels, the structure has {MAX_RING}", f.levels.len()));
        }
        for (i, l) in f.levels.iter().enumerate() {
            if l.level != i as u32 + 1 || l.size != 2 * l.level as i32 + 1 {
                return Err(format!("altar.json: level {} of size {} out of order", l.level, l.size));
            }
            for s in &l.sockets {
                if s[0].abs().max(s[1].abs()) != l.level as i32 || s[0].abs() == s[1].abs() {
                    return Err(format!("altar.json: socket {s:?} is not an edge cell of ring {}", l.level));
                }
            }
        }
        Ok(f)
    }

    pub fn embedded() -> Self {
        Self::parse(EMBEDDED).expect("the embedded altar.json reads (crate tests check it)")
    }

    /// The modifier a banner block stands for (`soulscraft:banner_weight` → weight).
    pub fn modifier_of(&self, block: &str) -> Option<&ModifierDef> {
        let short = block.strip_prefix("soulscraft:")?;
        self.modifiers.iter().find(|m| m.banner == short)
    }

    /// Every banner block id (`soulscraft:banner_weight`, ...), in the file's order.
    pub fn banner_blocks(&self) -> Vec<String> {
        self.modifiers.iter().map(|m| format!("soulscraft:{}", m.banner)).collect()
    }
}

/// A block as the altar sees it: its id and its `part` (a tall piece's segment).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub block: String,
    pub part: Option<String>,
}

impl Seen {
    pub fn new(block: &str, part: Option<&str>) -> Self {
        Self { block: block.to_owned(), part: part.map(str::to_owned) }
    }
    pub fn air() -> Self {
        Self::new(AIR, None)
    }
    pub fn is_air(&self) -> bool {
        self.block == AIR || self.block == "minecraft:cave_air" || self.block == "minecraft:void_air"
    }
}

/// One cell a ring needs filled.
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub cell: [i32; 3],
    pub block: &'static str,
    pub part: Option<&'static str>,
    pub ring: u32,
}

impl Need {
    pub fn met_by(&self, seen: &Seen) -> bool {
        seen.block == self.block && (self.part.is_none() || seen.part.as_deref() == self.part)
    }
}

/// The segments of a piece several blocks tall, bottom first; empty for a one-block piece.
pub fn tall_parts(block: &str) -> &'static [&'static str] {
    if block == PILLAR || block.starts_with(BANNER_PREFIX) {
        &["bottom", "top"]
    } else if block == OBELISK {
        &["bottom", "middle", "top"]
    } else {
        &[]
    }
}

/// Whether the block stands on a slab when it is on the altar: the pieces with `on_slab`.
pub fn has_on_slab(block: &str) -> bool {
    block == PILLAR || block == OBELISK || block == BRAZIER || block.starts_with(BANNER_PREFIX)
}

/// A SoulsCraft altar block (the core, the floor, the pieces, the banners).
pub fn is_altar_block(block: &str) -> bool {
    block.starts_with("soulscraft:altar_") || block.starts_with(BANNER_PREFIX)
}

fn corner_block(name: &str) -> Option<&'static str> {
    match name {
        "altar_pillar" => Some(PILLAR),
        "altar_brazier" => Some(BRAZIER),
        "altar_obelisk" => Some(OBELISK),
        _ => None,
    }
}

/// The cells of ring `r` on the floor (Chebyshev distance `r` from the core), in a fixed order.
pub fn ring_cells(r: i32) -> Vec<[i32; 2]> {
    let mut out = Vec::new();
    for x in -r..=r {
        for z in -r..=r {
            if x.abs().max(z.abs()) == r {
                out.push([x, z]);
            }
        }
    }
    out
}

/// What ring `ring` (1 to 4) needs: ring 1 is the core, the pedestal on it and 8 slabs; the
/// others are their floor (sockets where data/altar.json puts them, slabs elsewhere) and their
/// corner piece standing on the corner slabs.
pub fn ring_needs(file: &AltarFile, ring: u32) -> Vec<Need> {
    let mut out = Vec::new();
    if ring == 1 {
        out.push(Need { cell: [0, 0, 0], block: CORE, part: None, ring });
        out.push(Need { cell: [0, 1, 0], block: PEDESTAL, part: None, ring });
    }
    let Some(def) = file.levels.get(ring as usize - 1) else { return out };
    let r = ring as i32;
    for [x, z] in ring_cells(r) {
        let socket = def.sockets.contains(&[x, z]);
        out.push(Need { cell: [x, 0, z], block: if socket { SOCKET } else { SLAB }, part: None, ring });
    }
    if let Some(corner) = def.corner.as_deref().and_then(corner_block) {
        for [x, z] in [[-r, -r], [r, -r], [-r, r], [r, r]] {
            let parts = tall_parts(corner);
            if parts.is_empty() {
                out.push(Need { cell: [x, 1, z], block: corner, part: None, ring });
            }
            for (i, p) in parts.iter().enumerate() {
                out.push(Need { cell: [x, 1 + i as i32, z], block: corner, part: Some(p), ring });
            }
        }
    }
    out
}

/// What a structure reaches, read cell by cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// The highest ring complete with every ring inside it (0: not even the core's ring).
    pub built: u32,
    /// What the next ring still needs (empty at level 4).
    pub missing: Vec<Need>,
    /// Cells of the next ring with a wrong block on them (drawn red).
    pub wrong: Vec<[i32; 3]>,
    /// Banners on the sockets of the complete rings: socket cell and modifier id.
    pub banners: Vec<([i32; 3], String)>,
}

impl Report {
    /// The level that counts: none under a roof (round 5: the core needs the open sky).
    pub fn level(&self, open_sky: bool) -> u32 {
        if open_sky { self.built } else { 0 }
    }
}

/// Reads the structure round the core with `read(cell)` and works out its level.
pub fn evaluate(file: &AltarFile, read: impl Fn([i32; 3]) -> Seen) -> Report {
    let mut report = Report::default();
    for ring in 1..=MAX_RING {
        let needs = ring_needs(file, ring);
        let mut missing = Vec::new();
        for n in needs {
            let seen = read(n.cell);
            if !n.met_by(&seen) {
                if !seen.is_air() {
                    report.wrong.push(n.cell);
                }
                missing.push(n);
            }
        }
        if !missing.is_empty() {
            report.missing = missing;
            break;
        }
        report.built = ring;
    }
    for def in file.levels.iter().take(report.built as usize) {
        for [x, z] in &def.sockets {
            let cell = [*x, 1, *z];
            let seen = read(cell);
            if seen.part.as_deref().is_none_or(|p| p == "bottom")
                && let Some(m) = file.modifier_of(&seen.block)
            {
                report.banners.push((cell, m.id.clone()));
            }
        }
    }
    report
}

/// The way a banner on socket `[x, z]` faces: outward from the altar, along the longer axis.
pub fn facing_out(x: i32, z: i32) -> &'static str {
    if x.abs() >= z.abs() {
        if x >= 0 { "east" } else { "west" }
    } else if z >= 0 {
        "south"
    } else {
        "north"
    }
}

/// What the banners add up to (hypothesis Г10): same banners stack, opposite ones cancel because
/// their numbers have opposite signs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    /// Added to the power P the event's mobs are scaled by.
    pub power: f64,
    /// The share more (or fewer) mobs in each wave: 0.25 is a quarter more.
    pub wave_size: f64,
    /// Seconds added to each rest between waves.
    pub rest_seconds: f64,
    /// The share more altar health: 0.5 is half as much again.
    pub altar_health: f64,
    /// Kin banners: kin to count.
    pub kin: BTreeMap<String, u32>,
    /// The share of each wave's slots the kin banners turn into their kin (0.25 a banner).
    pub kin_share: f64,
    /// Banners that make the event harder (more reward) and easier (less).
    pub more: u32,
    pub less: u32,
}

impl Effects {
    pub fn is_empty(&self) -> bool {
        *self == Effects::default()
    }

    /// The sum as lines for the altar's menu.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let signed = |v: f64| if v >= 0.0 { format!("+{}", trim(v)) } else { format!("−{}", trim(-v)) };
        if self.power.abs() > 1e-9 {
            out.push(format!("сила врагов P {}", signed(self.power)));
        }
        if self.wave_size.abs() > 1e-9 {
            out.push(format!("врагов в волне {}%", signed(self.wave_size * 100.0)));
        }
        if self.rest_seconds.abs() > 1e-9 {
            out.push(format!("передышка {} с", signed(self.rest_seconds)));
        }
        if self.altar_health.abs() > 1e-9 {
            out.push(format!("здоровье алтаря {}%", signed(self.altar_health * 100.0)));
        }
        if !self.kin.is_empty() {
            let kins: Vec<String> = self.kin.iter().map(|(k, n)| if *n > 1 { format!("{} ×{n}", kin_name(k)) } else { kin_name(k).to_owned() }).collect();
            out.push(format!("род: {} — {}% волны и чаще в спавне", kins.join(", "), trim(self.kin_share.min(1.0) * 100.0)));
        }
        if self.more > 0 || self.less > 0 {
            let heads = i64::from(self.more) - i64::from(self.less);
            out.push(format!("трофеи: сложнее {}, проще {} (голов {})", self.more, self.less, if heads >= 0 { format!("+{heads}") } else { format!("−{}", -heads) }));
        }
        out
    }
}

/// A kin's name in the menu (data/altar.json `kin_names`).
pub fn kin_name(kin: &str) -> &str {
    match kin {
        "undead" => "нежить",
        "beasts" => "звери",
        "slimes" => "слизни",
        "flyers" => "летучие",
        "water" => "водные",
        other => other,
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// The sum of the modifiers with these ids.
pub fn effects(file: &AltarFile, ids: &[String]) -> Effects {
    let mut e = Effects::default();
    for id in ids {
        let Some(m) = file.modifiers.iter().find(|m| &m.id == id) else { continue };
        let num = |k: &str| m.effect.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        e.power += num("power");
        e.wave_size += num("wave_size");
        e.rest_seconds += num("rest_seconds");
        e.altar_health += num("altar_health");
        if let Some(k) = m.effect.get("kin").and_then(Value::as_str) {
            *e.kin.entry(k.to_owned()).or_default() += 1;
            e.kin_share += num("kin_share");
        }
        match m.reward.as_str() {
            "more" => e.more += 1,
            "less" => e.less += 1,
            _ => {}
        }
    }
    e
}

/// The block state for `setblock`: `soulscraft:altar_pillar[on_slab=true,part=top]`.
pub fn state_string(block: &str, props: &[(&str, &str)]) -> String {
    if props.is_empty() {
        return block.to_owned();
    }
    let inner: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{block}[{}]", inner.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A world of cells relative to the core.
    #[derive(Default)]
    struct World(HashMap<[i32; 3], Seen>);

    impl World {
        fn put(&mut self, n: &Need) {
            self.0.insert(n.cell, Seen::new(n.block, n.part));
        }
        fn build(file: &AltarFile, level: u32) -> Self {
            let mut w = World::default();
            for r in 1..=level {
                for n in ring_needs(file, r) {
                    w.put(&n);
                }
            }
            w
        }
        fn read(&self) -> impl Fn([i32; 3]) -> Seen + '_ {
            |c| self.0.get(&c).cloned().unwrap_or_else(Seen::air)
        }
    }

    #[test]
    fn embedded_file_reads() {
        let f = AltarFile::embedded();
        assert_eq!(f.levels.len(), 4);
        assert_eq!(f.modifiers.len(), 12);
        assert_eq!(f.pedestal_item.height_px, 17.0);
        assert_eq!(f.pedestal_item.scale, 0.625);
        assert_eq!(f.banner_blocks().len(), 12);
        assert!(AltarFile::parse("{}").is_err());
    }

    #[test]
    fn rings_need_what_the_spec_says() {
        let f = AltarFile::embedded();
        let count = |r, b: &str| ring_needs(&f, r).iter().filter(|n| n.block == b).count();
        // Ring 1: core, pedestal, 8 slabs.
        assert_eq!((count(1, CORE), count(1, PEDESTAL), count(1, SLAB)), (1, 1, 8));
        // Ring 2: 16 floor cells of which 2 sockets, 4 pillars of 2 blocks.
        assert_eq!((count(2, SLAB), count(2, SOCKET), count(2, PILLAR)), (14, 2, 8));
        // Ring 3: 24 cells, 2 sockets, 4 braziers.
        assert_eq!((count(3, SLAB), count(3, SOCKET), count(3, BRAZIER)), (22, 2, 4));
        // Ring 4: 32 cells, 4 sockets, 4 obelisks of 3.
        assert_eq!((count(4, SLAB), count(4, SOCKET), count(4, OBELISK)), (28, 4, 12));
        // Sockets on the whole altar of each level: 0, 2, 4, 8.
        let sockets: Vec<usize> = (1..=4).map(|l| (1..=l).map(|r| count(r, SOCKET)).sum()).collect();
        assert_eq!(sockets, [0, 2, 4, 8]);
    }

    #[test]
    fn each_level_is_computed_from_complete_rings() {
        let f = AltarFile::embedded();
        assert_eq!(evaluate(&f, World::default().read()).built, 0);
        for level in 1..=4 {
            let w = World::build(&f, level);
            let r = evaluate(&f, w.read());
            assert_eq!(r.built, level, "level {level}");
            assert!(r.wrong.is_empty());
            if level < 4 {
                assert_eq!(r.missing.len(), ring_needs(&f, level + 1).len());
            } else {
                assert!(r.missing.is_empty());
            }
        }
    }

    #[test]
    fn an_outer_ring_does_not_count_without_the_inner_one() {
        let f = AltarFile::embedded();
        let mut w = World::build(&f, 4);
        // A slab of ring 2 gone: rings 3 and 4 stand but the altar is level 1.
        w.0.remove(&[1, 0, -2]);
        let r = evaluate(&f, w.read());
        assert_eq!(r.built, 1);
        assert_eq!(r.missing.len(), 1);
        assert_eq!(r.missing[0].cell, [1, 0, -2]);
    }

    #[test]
    fn a_wrong_block_is_reported_and_a_socket_counts_as_a_ring_cell() {
        let f = AltarFile::embedded();
        let mut w = World::build(&f, 2);
        // A slab where ring 2's socket belongs: wrong, and the ring is not complete.
        w.0.insert([2, 0, 0], Seen::new(SLAB, None));
        let r = evaluate(&f, w.read());
        assert_eq!(r.built, 1);
        assert_eq!(r.wrong, vec![[2, 0, 0]]);
        // A pillar's top missing: not complete either.
        let mut w = World::build(&f, 2);
        w.0.remove(&[2, 2, 2]);
        assert_eq!(evaluate(&f, w.read()).built, 1);
    }

    #[test]
    fn roof_takes_the_level_away() {
        let f = AltarFile::embedded();
        let r = evaluate(&f, World::build(&f, 3).read());
        assert_eq!(r.level(true), 3);
        assert_eq!(r.level(false), 0);
    }

    #[test]
    fn banners_on_sockets_of_complete_rings_count() {
        let f = AltarFile::embedded();
        let mut w = World::build(&f, 2);
        w.0.insert([-2, 1, 0], Seen::new("soulscraft:banner_weight", Some("bottom")));
        w.0.insert([-2, 2, 0], Seen::new("soulscraft:banner_weight", Some("top")));
        w.0.insert([2, 1, 0], Seen::new("soulscraft:banner_horde", Some("bottom")));
        // On ring 3's socket, which is not built: not counted.
        w.0.insert([0, 1, -3], Seen::new("soulscraft:banner_mercy", Some("bottom")));
        let r = evaluate(&f, w.read());
        let ids: Vec<&str> = r.banners.iter().map(|b| b.1.as_str()).collect();
        assert_eq!(ids, ["weight", "horde"]);
    }

    #[test]
    fn banner_effects_stack_and_cancel() {
        let f = AltarFile::embedded();
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let e = effects(&f, &ids(&["weight", "weight", "mercy", "horde", "rest", "resilience", "kin_undead"]));
        assert!((e.power - 0.15).abs() < 1e-9);
        assert!((e.wave_size - 0.25).abs() < 1e-9);
        assert_eq!(e.rest_seconds, 15.0);
        assert_eq!(e.altar_health, 0.5);
        assert_eq!(e.kin.get("undead"), Some(&1));
        assert_eq!((e.more, e.less), (3, 3));
        let lines = e.lines();
        assert!(lines.iter().any(|l| l == "сила врагов P +0,15"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "передышка +15 с"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "род: нежить — 25% волны и чаще в спавне"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "трофеи: сложнее 3, проще 3 (голов +0)"), "{lines:?}");
        let kin = effects(&f, &ids(&["kin_undead", "kin_undead", "kin_flyers", "mercy"]));
        assert!((kin.kin_share - 0.75).abs() < 1e-9);
        assert_eq!(kin.kin.get("undead"), Some(&2));
        let lines = kin.lines();
        assert!(lines.iter().any(|l| l == "род: летучие, нежить ×2 — 75% волны и чаще в спавне"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "трофеи: сложнее 0, проще 1 (голов −1)"), "{lines:?}");
        // Five kin banners still turn at most every slot.
        let all = effects(&f, &ids(&["kin_undead", "kin_beasts", "kin_slimes", "kin_flyers", "kin_water"]));
        assert!(all.lines().iter().any(|l| l.ends_with("— 100% волны и чаще в спавне")));
        assert!(effects(&f, &ids(&["weight", "mercy"])).power.abs() < 1e-9);
        assert!(effects(&f, &[]).is_empty());
        assert!(effects(&f, &ids(&["nothing"])).is_empty());
    }

    #[test]
    fn banners_face_outward() {
        assert_eq!(facing_out(-2, 0), "west");
        assert_eq!(facing_out(2, 0), "east");
        assert_eq!(facing_out(0, -3), "north");
        assert_eq!(facing_out(0, 3), "south");
        assert_eq!(facing_out(2, -4), "north");
        assert_eq!(facing_out(4, 2), "east");
        assert_eq!(facing_out(-2, 4), "south");
        assert_eq!(facing_out(-4, -2), "west");
    }

    #[test]
    fn tall_pieces_and_states() {
        assert_eq!(tall_parts(PILLAR), ["bottom", "top"]);
        assert_eq!(tall_parts(OBELISK).len(), 3);
        assert_eq!(tall_parts("soulscraft:banner_calm"), ["bottom", "top"]);
        assert!(tall_parts(BRAZIER).is_empty());
        assert!(has_on_slab(BRAZIER) && !has_on_slab(SLAB));
        assert!(is_altar_block(CORE) && is_altar_block("soulscraft:banner_rest") && !is_altar_block("soulscraft:brute_head"));
        assert_eq!(state_string(PILLAR, &[("on_slab", "true"), ("part", "top")]), "soulscraft:altar_pillar[on_slab=true,part=top]");
        assert_eq!(state_string(CORE, &[]), CORE);
        assert!(Seen::air().is_air());
    }
}
