//! The altar's second role (06-altar.md; round 2; prototype П4): while an altar stands and its
//! menu switch is on, natural spawning tries its sector of chunks more often, with enemies from
//! other biomes too, and the sector drifts towards the altar over time. Numbers are
//! data/altar.json `spawn_boost` (hypotheses of iteration 3).
//!
//! How the server uses it (mod/README.md, «Хуки»): natural spawning's candidate chunks
//! (`ChunkMap.collectSpawningChunks`) get each sector chunk that is already a candidate again,
//! [`Boost::passes`] more times — more spawn attempts there under vanilla's own rules and caps —
//! and the spawn list of a monster position in a sector ([`Boost::extend`]) gets the other
//! biomes' monsters and SoulsCraft's mobs, the kin banners' kin weighted up.

use crate::altar::Effects;
use crate::data::MobsFile;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BoostDef {
    /// Extra passes over each sector chunk a spawn tick, for altar levels 1 to 4.
    pub extra_passes: Vec<u32>,
    /// The sector is this many chunks round its centre (1: 3 × 3).
    pub sector_radius_chunks: i32,
    /// The sector's centre starts this many chunks from the altar's chunk ...
    pub start_chunks: i32,
    /// ... and comes one chunk closer each this many ticks the altar stands.
    pub drift_ticks: u64,
    /// Monsters of other biomes added to a sector position's list.
    #[serde(default)]
    pub foreign: Vec<Entry>,
    /// The weight of each SoulsCraft mob added (none walks on water only, none elite).
    pub soulscraft_weight: u32,
    /// A kin banner multiplies the weights of its kin by `1 + kin_weight · banners`.
    pub kin_weight: f64,
    /// The vanilla mobs of each kin (SoulsCraft's kin is in data/mobs.json).
    #[serde(default)]
    pub kin_vanilla: BTreeMap<String, Vec<String>>,
}

/// A spawn list entry as the engine's `SpawnEntry` has it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Entry {
    #[serde(rename = "type")]
    pub kind: String,
    pub weight: u32,
    pub min: u32,
    pub max: u32,
}

impl BoostDef {
    /// data/altar.json's `spawn_boost`.
    pub fn of(altar_json: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(altar_json).map_err(|e| format!("altar.json: {e}"))?;
        let b: Self = serde_json::from_value(v["spawn_boost"].clone()).map_err(|e| format!("altar.json spawn_boost: {e}"))?;
        if b.extra_passes.len() != crate::altar::MAX_RING as usize {
            return Err(format!("spawn_boost: {} extra_passes, the altar has 4 levels", b.extra_passes.len()));
        }
        if b.drift_ticks == 0 || b.sector_radius_chunks < 0 || b.start_chunks < 0 {
            return Err("spawn_boost: drift_ticks above 0, radius and start not negative".into());
        }
        Ok(b)
    }

    pub fn embedded() -> Self {
        Self::of(crate::altar::EMBEDDED).expect("the embedded spawn_boost reads (crate tests check it)")
    }
}

/// One altar's boost as the server sees it now.
#[derive(Clone, Debug, PartialEq)]
pub struct Boost {
    /// The altar's chunk.
    pub altar: (i32, i32),
    /// The sector's centre chunk now.
    pub centre: (i32, i32),
    pub radius: i32,
    /// Extra passes over each of its chunks.
    pub passes: u32,
    /// Kin banners and their count.
    pub kin: BTreeMap<String, u32>,
}

/// The way the sector comes from: one of eight, fixed by the core's place.
pub fn direction(core: [i32; 3]) -> (i32, i32) {
    const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
    let h = (core[0] as i64).wrapping_mul(341_873_128_712) ^ (core[2] as i64).wrapping_mul(132_897_987_541);
    DIRS[(h.rem_euclid(8)) as usize]
}

/// How many chunks the sector's centre is from the altar after `age` ticks: `start`, one closer
/// each `drift_ticks`, down to 0 (the sector round the altar).
pub fn distance(def: &BoostDef, age: u64) -> i32 {
    let steps = (age / def.drift_ticks).min(def.start_chunks as u64) as i32;
    def.start_chunks - steps
}

/// The boost of an altar at `core` of `level` that has stood `age` ticks with these banners;
/// None when it does nothing (level 0, switch off, or no passes left after the calm banners).
pub fn boost(def: &BoostDef, core: [i32; 3], level: u32, on: bool, age: u64, effects: &Effects) -> Option<Boost> {
    if !on || level == 0 {
        return None;
    }
    let base = f64::from(*def.extra_passes.get(level as usize - 1)?);
    // Horde and calm scale the boost as they scale a wave (06-altar.md: strength by banners).
    let passes = (base * (1.0 + effects.wave_size)).round().max(0.0) as u32;
    if passes == 0 {
        return None;
    }
    let altar = (core[0].div_euclid(16), core[2].div_euclid(16));
    let (dx, dz) = direction(core);
    let d = distance(def, age);
    Some(Boost { altar, centre: (altar.0 + dx * d, altar.1 + dz * d), radius: def.sector_radius_chunks, passes, kin: effects.kin.clone() })
}

/// The altar menu's line about the boost.
pub fn menu_line(def: &BoostDef, core: [i32; 3], level: u32, on: bool, age: u64, effects: &Effects) -> String {
    match boost(def, core, level, on, age, effects) {
        None if !on => "Учащённый спавн выключен".into(),
        None => "Учащённый спавн: нет (алтарь не достроен или затишье гасит его)".into(),
        Some(b) => {
            let side = 2 * b.radius + 1;
            let d = distance(def, age);
            let place = if d == 0 {
                "вокруг алтаря".to_owned()
            } else {
                format!("в {d} чанк. от алтаря, на чанк ближе каждые {} мин", def.drift_ticks / 1200)
            };
            format!("Учащённый спавн: ×{} попыток в секторе {side}×{side} чанков {place}", 1 + b.passes)
        }
    }
}

impl Boost {
    pub fn holds(&self, chunk: (i32, i32)) -> bool {
        (chunk.0 - self.centre.0).abs() <= self.radius && (chunk.1 - self.centre.1).abs() <= self.radius
    }

    pub fn holds_block(&self, x: i32, z: i32) -> bool {
        self.holds((x.div_euclid(16), z.div_euclid(16)))
    }
}

/// Natural spawning's candidate chunks with the boosts' extra passes: each candidate a sector
/// holds comes again `passes` more times (summed over sectors that overlap), appended after the
/// rest, so chunks outside sectors keep their passes. Chunks that are not candidates (no player
/// near, not ticking) get none.
pub fn extra_candidates(candidates: &[(i32, i32)], boosts: &[Boost]) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for &c in candidates {
        let n: u32 = boosts.iter().filter(|b| b.holds(c)).map(|b| b.passes).sum();
        out.extend(std::iter::repeat_n(c, n as usize));
    }
    out
}

/// A monster position's spawn list in a sector: the other biomes' monsters and SoulsCraft's land
/// mobs added (each once), then every entry of a banner's kin weighted up. Entries already there
/// keep their place; an added one already in the list is not added twice.
pub fn extend(entries: &mut Vec<Entry>, def: &BoostDef, mobs: &MobsFile, kin: &BTreeMap<String, u32>) {
    let mut add: Vec<Entry> = def.foreign.clone();
    for m in mobs.mobs.iter().filter(|m| !m.stats.water_only && !m.elite) {
        add.push(Entry { kind: m.key.clone(), weight: def.soulscraft_weight, min: 1, max: 1 });
    }
    for e in add {
        if !entries.iter().any(|x| x.kind == e.kind) {
            entries.push(e);
        }
    }
    for e in entries.iter_mut() {
        let n: u32 = kin.iter().filter(|(k, _)| of_kin(&e.kind, k, def, mobs)).map(|(_, n)| *n).sum();
        if n > 0 {
            e.weight = (f64::from(e.weight) * (1.0 + def.kin_weight * f64::from(n))).round() as u32;
        }
    }
}

/// Whether the entity type is of the kin: SoulsCraft's by data/mobs.json, vanilla's by
/// `kin_vanilla`.
pub fn of_kin(kind: &str, kin: &str, def: &BoostDef, mobs: &MobsFile) -> bool {
    match mobs.by_key(kind) {
        Some((_, m)) => m.has_kin(kin),
        None => def.kin_vanilla.get(kin).is_some_and(|l| l.iter().any(|v| v == kind)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::altar::{AltarFile, effects};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_data_reads() {
        let d = BoostDef::embedded();
        assert_eq!(d.extra_passes, [1, 1, 2, 3]);
        assert_eq!((d.sector_radius_chunks, d.start_chunks, d.drift_ticks), (1, 6, 6000));
        assert_eq!(d.foreign.len(), 5);
        assert!(d.kin_vanilla["undead"].contains(&"minecraft:zombie".to_owned()));
        assert!(BoostDef::of("{}").is_err());
        assert!(BoostDef::of(&crate::altar::EMBEDDED.replace("\"extra_passes\": [1, 1, 2, 3]", "\"extra_passes\": [1]")).unwrap_err().contains("levels"));
        assert!(BoostDef::of(&crate::altar::EMBEDDED.replace("\"drift_ticks\": 6000", "\"drift_ticks\": 0")).is_err());
        assert!(BoostDef::of("{").is_err());
    }

    #[test]
    fn the_sector_drifts_to_the_altar() {
        let d = BoostDef::embedded();
        let core = [-700, 101, 700];
        let dir = direction(core);
        assert_ne!(dir, (0, 0));
        let at = |age| boost(&d, core, 1, true, age, &Effects::default()).unwrap();
        let altar = (-44, 43);
        let b0 = at(0);
        assert_eq!(b0.altar, altar);
        assert_eq!(b0.centre, (altar.0 + 6 * dir.0, altar.1 + 6 * dir.1));
        assert!(!b0.holds(altar));
        assert_eq!(at(5999).centre, b0.centre);
        assert_eq!(at(6000).centre, (altar.0 + 5 * dir.0, altar.1 + 5 * dir.1));
        // After six steps it sits on the altar and stays.
        assert_eq!(at(36_000).centre, altar);
        assert_eq!(at(10_000_000).centre, altar);
        assert!(at(36_000).holds(altar) && at(36_000).holds((altar.0 + 1, altar.1 - 1)) && !at(36_000).holds((altar.0 + 2, altar.1)));
        assert!(at(36_000).holds_block(-700, 700));
        // Distances step down one at a time.
        let steps: Vec<i32> = (0..8).map(|k| distance(&d, k * 6000)).collect();
        assert_eq!(steps, [6, 5, 4, 3, 2, 1, 0, 0]);
        // Directions spread over all eight.
        let dirs: std::collections::BTreeSet<(i32, i32)> = (0..64).map(|i| direction([i * 37, 0, i * 11 - 300])).collect();
        assert_eq!(dirs.len(), 8);
    }

    #[test]
    fn strength_by_level_banners_and_switch() {
        let d = BoostDef::embedded();
        let f = AltarFile::embedded();
        let core = [0, 64, 0];
        let passes = |level, banners: &[&str]| boost(&d, core, level, true, 0, &effects(&f, &ids(banners))).map(|b| b.passes);
        assert_eq!([passes(1, &[]), passes(2, &[]), passes(3, &[]), passes(4, &[])], [Some(1), Some(1), Some(2), Some(3)]);
        assert_eq!(passes(0, &[]), None);
        assert_eq!(passes(4, &["horde"]), Some(4));
        assert_eq!(passes(4, &["horde", "horde"]), Some(5));
        assert_eq!(passes(4, &["calm"]), Some(2));
        assert_eq!(passes(4, &["horde", "calm"]), Some(3));
        assert_eq!(passes(1, &["calm", "calm"]), Some(1));
        assert_eq!(passes(1, &["calm", "calm", "calm", "calm"]), None);
        assert_eq!(boost(&d, core, 4, false, 0, &Effects::default()), None);
        let kin = boost(&d, core, 2, true, 0, &effects(&f, &ids(&["kin_slimes"]))).unwrap();
        assert_eq!(kin.kin.get("slimes"), Some(&1));
    }

    #[test]
    fn the_menu_says_what_the_boost_does() {
        let d = BoostDef::embedded();
        let f = AltarFile::embedded();
        let core = [0, 64, 0];
        assert_eq!(menu_line(&d, core, 2, true, 0, &Effects::default()), "Учащённый спавн: ×2 попыток в секторе 3×3 чанков в 6 чанк. от алтаря, на чанк ближе каждые 5 мин");
        assert_eq!(menu_line(&d, core, 4, true, 99_999, &Effects::default()), "Учащённый спавн: ×4 попыток в секторе 3×3 чанков вокруг алтаря");
        assert_eq!(menu_line(&d, core, 4, false, 0, &Effects::default()), "Учащённый спавн выключен");
        let calm = effects(&f, &ids(&["calm", "calm", "calm", "calm"]));
        assert!(menu_line(&d, core, 1, true, 0, &calm).contains("нет"));
    }

    #[test]
    fn sector_candidates_come_again() {
        let b = Boost { altar: (0, 0), centre: (0, 0), radius: 1, passes: 2, kin: BTreeMap::new() };
        let c = vec![(0, 0), (5, 5), (1, -1), (2, 0)];
        assert_eq!(extra_candidates(&c, std::slice::from_ref(&b)), [(0, 0), (0, 0), (1, -1), (1, -1)]);
        // Two sectors over one chunk add up; no boost, no extra.
        let b2 = Boost { centre: (1, 0), passes: 1, ..b.clone() };
        assert_eq!(extra_candidates(&[(1, 0)], &[b.clone(), b2]), [(1, 0); 3]);
        assert!(extra_candidates(&c, &[]).is_empty());
        // A chunk out of the candidates (no player near) gets nothing.
        assert!(extra_candidates(&[(9, 9)], &[b]).is_empty());
    }

    #[test]
    fn sector_spawn_lists_get_foreign_and_kin() {
        let d = BoostDef::embedded();
        let mobs = MobsFile::embedded();
        let base = vec![Entry { kind: "minecraft:zombie".into(), weight: 95, min: 4, max: 4 }, Entry { kind: "minecraft:spider".into(), weight: 100, min: 4, max: 4 }];
        let mut e = base.clone();
        extend(&mut e, &d, &mobs, &BTreeMap::new());
        let kinds: Vec<&str> = e.iter().map(|x| x.kind.as_str()).collect();
        assert_eq!(&kinds[..2], ["minecraft:zombie", "minecraft:spider"]);
        assert!(kinds.contains(&"minecraft:husk") && kinds.contains(&"minecraft:stray"));
        assert!(kinds.contains(&"soulscraft:revenant") && kinds.contains(&"soulscraft:shrike"));
        assert!(!kinds.contains(&"soulscraft:lurker") && !kinds.contains(&"soulscraft:brute"));
        assert_eq!(e[0].weight, 95);
        // Twice: nothing doubles.
        let n = e.len();
        extend(&mut e, &d, &mobs, &BTreeMap::new());
        assert_eq!(e.len(), n);
        // Two undead banners: zombies and revenants × 5; spiders stay.
        let mut e = base.clone();
        extend(&mut e, &d, &mobs, &BTreeMap::from([("undead".to_owned(), 2)]));
        let w = |k: &str| e.iter().find(|x| x.kind == k).unwrap().weight;
        assert_eq!(w("minecraft:zombie"), 475);
        assert_eq!(w("soulscraft:revenant"), 50);
        assert_eq!(w("minecraft:spider"), 100);
        assert_eq!(w("soulscraft:stalker"), 10);
        // Beasts: spiders and the stalker × 3.
        let mut e = base;
        extend(&mut e, &d, &mobs, &BTreeMap::from([("beasts".to_owned(), 1)]));
        assert_eq!(e.iter().find(|x| x.kind == "minecraft:spider").unwrap().weight, 300);
        assert_eq!(e.iter().find(|x| x.kind == "soulscraft:stalker").unwrap().weight, 30);
        assert!(of_kin("soulscraft:shrike", "flyers", &d, &mobs));
        assert!(!of_kin("minecraft:creeper", "undead", &d, &mobs));
    }
}
