//! The survival slice's world rules (docs/design/10-survival.md): SoulsCraft's mobs in the natural
//! spawn lists by biome (hypothesis Г14), the lurker's own water spawner, and the soul shard a
//! credited kill drops (Г15). Data: `natural` and `soul_shard` in data/mobs.json.

use serde::Deserialize;

/// One mob's place in the monster spawn lists.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct NaturalMob {
    /// The mob's id in `mobs` (`revenant`).
    pub mob: String,
    /// Parts of a biome's name (`forest` takes `birch_forest`); empty: see `biomes_with_zombie`.
    pub biomes: Vec<String>,
    /// Every biome whose list already has the zombie (the overworld's ordinary monster biomes).
    pub biomes_with_zombie: bool,
    pub weight: u32,
    pub min: u32,
    pub max: u32,
    pub min_y: Option<i32>,
    pub max_y: Option<i32>,
}

/// The lurker's own spawner: in deep ocean water, near players.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct LurkerSpawn {
    pub mob: String,
    /// Whole biome names (`minecraft:` left out).
    pub biomes: Vec<String>,
    pub every_ticks: u64,
    pub chance: f64,
    /// Nearest and farthest horizontal distance from the player.
    pub radius_blocks: [f64; 2],
    pub max_y: i32,
    /// At most this many lurkers within `near_blocks` of the spot.
    pub near_cap: usize,
    pub near_blocks: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct NaturalSection {
    pub mobs: Vec<NaturalMob>,
    pub lurker: LurkerSpawn,
}

/// The soul shard (Г15).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct ShardDef {
    pub item: String,
    pub chance: f64,
    pub count: u32,
    pub elite_chance: f64,
    pub elite_count: u32,
}

impl Default for ShardDef {
    fn default() -> Self {
        Self { item: "soulscraft:soul_shard".into(), chance: 0.35, count: 1, elite_chance: 1.0, elite_count: 2 }
    }
}

/// An entry to add to a position's monster list: the mob's key, weight and group size.
#[derive(Clone, Debug, PartialEq)]
pub struct Extra {
    pub key: String,
    pub weight: u32,
    pub min: u32,
    pub max: u32,
}

/// The biome's own name without its namespace (`minecraft:birch_forest` → `birch_forest`).
fn bare(biome: &str) -> &str {
    biome.rsplit(':').next().unwrap_or(biome)
}

impl NaturalMob {
    /// Whether this row adds its mob at `y` in `biome`, whose list holds the zombie or not.
    pub fn applies(&self, biome: &str, y: i32, has_zombie: bool) -> bool {
        let name = bare(biome);
        let by_biome = if self.biomes.is_empty() { self.biomes_with_zombie && has_zombie } else { self.biomes.iter().any(|b| name.contains(b.as_str())) };
        by_biome && self.min_y.is_none_or(|m| y >= m) && self.max_y.is_none_or(|m| y <= m) && self.weight > 0
    }
}

/// What SoulsCraft adds to the monster list of a position in `biome` at height `y` (the overworld
/// only: the caller checks the dimension). A mob already in the list (a spawn boost put it there)
/// is not added twice; two rows of one mob that both apply count once, the first.
pub fn extras(section: &NaturalSection, mobs: &crate::data::MobsFile, biome: &str, y: i32, listed: &[&str]) -> Vec<Extra> {
    let has_zombie = listed.contains(&"minecraft:zombie");
    let mut out: Vec<Extra> = Vec::new();
    for row in &section.mobs {
        let Some(def) = mobs.mob(&row.mob) else { continue };
        if !row.applies(biome, y, has_zombie) || listed.contains(&def.key.as_str()) || out.iter().any(|e| e.key == def.key) {
            continue;
        }
        out.push(Extra { key: def.key.clone(), weight: row.weight, min: row.min.max(1), max: row.max.max(row.min.max(1)) });
    }
    out
}

impl LurkerSpawn {
    /// Whether a player in `biome` may get a lurker near them.
    pub fn biome_ok(&self, biome: &str) -> bool {
        let name = bare(biome);
        self.biomes.iter().any(|b| b == name)
    }

    /// Whether this tick tries a lurker for a player (every `every_ticks`, with `chance`).
    pub fn due(&self, now: u64, roll: f64) -> bool {
        self.every_ticks > 0 && now % self.every_ticks == 0 && roll < self.chance
    }

    /// The spot `roll` (two numbers in 0..1) picks round the player at `at`: an angle and a
    /// distance between the radii; the height is the caller's to find (water below `max_y`).
    pub fn spot(&self, at: [f64; 3], roll: [f64; 2]) -> [f64; 2] {
        let [near, far] = self.radius_blocks;
        let angle = roll[0] * std::f64::consts::TAU;
        let d = near + (far - near).max(0.0) * roll[1];
        [at[0] + d * angle.cos(), at[2] + d * angle.sin()]
    }
}

impl ShardDef {
    /// How many shards a credited kill of `elite` (or not) drops for `roll` in 0..1.
    pub fn drop(&self, elite: bool, roll: f64) -> u32 {
        let (chance, count) = if elite { (self.elite_chance, self.elite_count) } else { (self.chance, self.count) };
        if roll < chance { count } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MobsFile;

    fn file() -> MobsFile {
        MobsFile::embedded()
    }

    fn keys(e: &[Extra]) -> Vec<&str> {
        e.iter().map(|x| x.key.as_str()).collect()
    }

    #[test]
    fn plains_at_night_get_the_revenant_and_the_marksman() {
        let f = file();
        let e = extras(&f.natural, &f, "minecraft:plains", 64, &["minecraft:zombie", "minecraft:skeleton"]);
        assert_eq!(keys(&e), ["soulscraft:revenant", "soulscraft:marksman"]);
        assert_eq!(e[0].weight, 20);
        assert_eq!((e[1].min, e[1].max), (1, 2));
    }

    #[test]
    fn the_revenant_keeps_to_the_surface_and_the_brute_to_caves() {
        let f = file();
        let deep = extras(&f.natural, &f, "minecraft:plains", 12, &["minecraft:zombie"]);
        assert_eq!(keys(&deep), ["soulscraft:marksman", "soulscraft:brute"]);
        // A biome without the zombie (a mushroom field) gets neither.
        let none = extras(&f.natural, &f, "minecraft:mushroom_fields", 64, &[]);
        assert!(none.is_empty());
    }

    #[test]
    fn biome_parts_pick_their_mobs() {
        let f = file();
        let forest = extras(&f.natural, &f, "minecraft:birch_forest", 70, &["minecraft:zombie"]);
        assert_eq!(keys(&forest), ["soulscraft:revenant", "soulscraft:stalker"]);
        let swamp = extras(&f.natural, &f, "minecraft:mangrove_swamp", 63, &["minecraft:zombie"]);
        assert!(keys(&swamp).contains(&"soulscraft:bog_slime"));
        // The shrike only high up: a peak at 120 has it, at 70 not.
        let high = extras(&f.natural, &f, "minecraft:jagged_peaks", 120, &[]);
        assert_eq!(keys(&high), ["soulscraft:marksman", "soulscraft:brute", "soulscraft:shrike"]);
        let low = extras(&f.natural, &f, "minecraft:jagged_peaks", 70, &[]);
        assert!(!keys(&low).contains(&"soulscraft:shrike"));
    }

    #[test]
    fn a_mob_already_listed_is_not_added_again() {
        let f = file();
        let e = extras(&f.natural, &f, "minecraft:plains", 64, &["minecraft:zombie", "soulscraft:revenant"]);
        assert_eq!(keys(&e), ["soulscraft:marksman"]);
    }

    #[test]
    fn the_lurker_comes_in_deep_oceans_only_sometimes() {
        let l = file().natural.lurker;
        assert!(l.biome_ok("minecraft:deep_cold_ocean"));
        assert!(!l.biome_ok("minecraft:ocean"));
        assert!(!l.biome_ok("minecraft:deep_dark"));
        assert!(l.due(400, 0.1));
        assert!(!l.due(400, 0.9));
        assert!(!l.due(401, 0.1));
        let s = l.spot([0.0, 30.0, 0.0], [0.25, 0.0]);
        assert!((s[0]).abs() < 1e-9 && (s[1] - 16.0).abs() < 1e-9, "{s:?}");
        let far = l.spot([10.0, 30.0, 0.0], [0.0, 1.0]);
        assert!((far[0] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_credited_kill_drops_shards_by_its_odds() {
        let s = file().soul_shard;
        assert_eq!(s.item, "soulscraft:soul_shard");
        assert_eq!(s.drop(false, 0.2), 1);
        assert_eq!(s.drop(false, 0.5), 0);
        assert_eq!(s.drop(true, 0.99), 2);
    }

    #[test]
    fn the_tier_events_need_their_horns_and_levels() {
        let f = file();
        let tiers: Vec<(u32, &str, usize)> = f.events.iter().map(|e| (e.tier, e.item.as_str(), e.waves.len())).collect();
        assert_eq!(
            tiers,
            [(1, "soulscraft:rotten_horn", 5), (2, "soulscraft:grave_horn", 6), (3, "soulscraft:storm_horn", 7), (4, "soulscraft:abyss_horn", 8)]
        );
        // Every wave mob is a known type, and no water-only mob walks to an altar.
        for e in &f.events {
            for w in &e.waves {
                for m in &w.mobs {
                    if m.kind.starts_with("minecraft:") || m.kind == "banner_zombie" {
                        continue;
                    }
                    let def = f.mob(&m.kind).unwrap_or_else(|| panic!("{}: unknown {}", e.id, m.kind));
                    assert!(!def.stats.water_only, "{}: {} lives in water", e.id, m.kind);
                }
            }
        }
    }
}
