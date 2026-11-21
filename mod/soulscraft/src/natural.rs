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

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct NaturalSection {
    pub mobs: Vec<NaturalMob>,
    pub lurker: LurkerSpawn,
    /// A mob comes by itself only if a player this near has P of its `tier_p` or more.
    pub tier_gate_blocks: f64,
}

impl Default for NaturalSection {
    fn default() -> Self {
        Self { mobs: Vec::new(), lurker: LurkerSpawn::default(), tier_gate_blocks: 128.0 }
    }
}

/// Whether a mob of `tier_p` may come by itself where the strongest player near has `best`
/// (None: nobody near). The altar's events and spawn boost do not ask: their power is the group's.
pub fn tier_allows(tier_p: f64, best: Option<f64>) -> bool {
    best.is_some_and(|p| tier_p <= p + 1e-9)
}

impl NaturalSection {
    /// Whether the lurker's spawner may bring one where the strongest player near has `best`.
    pub fn lurker_allowed(&self, mobs: &crate::data::MobsFile, best: Option<f64>) -> bool {
        mobs.mob(&self.lurker.mob).is_some_and(|d| tier_allows(d.tier_p, best))
    }
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
/// only: the caller checks the dimension), where the strongest player within
/// [`NaturalSection::tier_gate_blocks`] has P `best` (None: nobody that near). A mob above that P
/// (`tier_p`) does not come; a mob with `params.group_size` comes in groups of that size. A mob
/// already in the list (a spawn boost put it there) is not added twice; two rows of one mob that
/// both apply count once, the first.
pub fn extras(section: &NaturalSection, mobs: &crate::data::MobsFile, biome: &str, y: i32, listed: &[&str], best: Option<f64>) -> Vec<Extra> {
    let has_zombie = listed.contains(&"minecraft:zombie");
    let mut out: Vec<Extra> = Vec::new();
    for row in &section.mobs {
        let Some(def) = mobs.mob(&row.mob) else { continue };
        if !row.applies(biome, y, has_zombie) || listed.contains(&def.key.as_str()) || out.iter().any(|e| e.key == def.key) {
            continue;
        }
        if !tier_allows(def.tier_p, best) {
            continue;
        }
        let (min, max) = group_size(def).unwrap_or((row.min, row.max));
        out.push(Extra { key: def.key.clone(), weight: row.weight, min: min.max(1), max: max.max(min.max(1)) });
    }
    out
}

/// A mob's `params.group_size` [min, max], if it has one (the shrike: pairs and threes).
fn group_size(def: &crate::data::MobDef) -> Option<(u32, u32)> {
    let g = def.params.get("group_size")?.as_array()?;
    Some((g.first()?.as_u64()? as u32, g.get(1)?.as_u64()? as u32))
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

/// The summoning horn a credited kill drops now and then: an ordinary mob's of tier 2, an elite's
/// of tier 3 (11-summons-rewards.md, «Добыча с врагов»; round 3 in 06-altar.md).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct HornDropDef {
    pub item: String,
    pub chance: f64,
    pub elite_item: String,
    pub elite_chance: f64,
}

impl Default for HornDropDef {
    fn default() -> Self {
        Self { item: "soulscraft:grave_horn".into(), chance: 0.005, elite_item: "soulscraft:storm_horn".into(), elite_chance: 0.03 }
    }
}

/// What a credited kill of the SoulsCraft mob `def` drops in the world (11-summons-rewards.md,
/// Г15 and Г18), each stack once: its shards, now and then a horn, its head by the trophies'
/// world chance, and each armour piece by its own bestiary chance (`loot`). `roll` gives a number
/// in 0..1 for each chance, in that order, so a test can feed it.
pub fn world_drops(file: &crate::data::MobsFile, chances: &crate::event::TrophyChances, def: &crate::data::MobDef, mut roll: impl FnMut() -> f64) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let shards = file.soul_shard.drop(def.elite, roll());
    if shards > 0 {
        out.push((file.soul_shard.item.clone(), shards));
    }
    let h = &file.horn_drop;
    let (horn, horn_chance) = if def.elite { (&h.elite_item, h.elite_chance) } else { (&h.item, h.chance) };
    if roll() < horn_chance && !horn.is_empty() {
        out.push((horn.clone(), 1));
    }
    let head_chance = if def.elite { chances.head_world_elite } else { chances.head_world };
    if roll() < head_chance {
        let id = def.key.strip_prefix("soulscraft:").unwrap_or(&def.id);
        out.push((format!("soulscraft:{id}_head"), 1));
    }
    for piece in &def.loot {
        if roll() < piece.chance {
            out.push((piece.item.clone(), 1));
        }
    }
    out
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
        let e = extras_at(&f, "minecraft:plains", 64, &["minecraft:zombie", "minecraft:skeleton"]);
        assert_eq!(keys(&e), ["soulscraft:revenant", "soulscraft:marksman"]);
        assert_eq!(e[0].weight, 20);
        assert_eq!((e[1].min, e[1].max), (1, 2));
    }

    #[test]
    fn the_revenant_keeps_to_the_surface_and_the_brute_to_caves() {
        let f = file();
        let deep = extras_at(&f, "minecraft:plains", 12, &["minecraft:zombie"]);
        assert_eq!(keys(&deep), ["soulscraft:marksman", "soulscraft:brute"]);
        // A biome without the zombie (a mushroom field) gets neither.
        let none = extras_at(&f, "minecraft:mushroom_fields", 64, &[]);
        assert!(none.is_empty());
    }

    #[test]
    fn biome_parts_pick_their_mobs() {
        let f = file();
        let forest = extras_at(&f, "minecraft:birch_forest", 70, &["minecraft:zombie"]);
        assert_eq!(keys(&forest), ["soulscraft:revenant", "soulscraft:stalker"]);
        let swamp = extras_at(&f, "minecraft:mangrove_swamp", 63, &["minecraft:zombie"]);
        assert!(keys(&swamp).contains(&"soulscraft:bog_slime"));
        // The shrike only high up: a peak at 120 has it, at 70 not.
        let high = extras_at(&f, "minecraft:jagged_peaks", 120, &[]);
        assert_eq!(keys(&high), ["soulscraft:marksman", "soulscraft:brute", "soulscraft:shrike"]);
        let low = extras_at(&f, "minecraft:jagged_peaks", 70, &[]);
        assert!(!keys(&low).contains(&"soulscraft:shrike"));
    }

    /// The rows as before the tier gate: a player of P 1 near.
    fn extras_at(f: &MobsFile, biome: &str, y: i32, listed: &[&str]) -> Vec<Extra> {
        extras(&f.natural, f, biome, y, listed, Some(1.0))
    }

    #[test]
    fn natural_respects_tier_p() {
        let f = file();
        assert_eq!(f.natural.tier_gate_blocks, 128.0);
        let plains = |p: Option<f64>| keys(&extras(&f.natural, &f, "minecraft:plains", 64, &["minecraft:zombie"], p)).into_iter().map(str::to_owned).collect::<Vec<_>>();
        // P 0: nobody strong enough for the revenant (0.2) or the marksman (0.3).
        assert!(plains(Some(0.0)).is_empty());
        // P 0.3: both (tier_p not above P).
        assert_eq!(plains(Some(0.3)), ["soulscraft:revenant", "soulscraft:marksman"]);
        assert_eq!(plains(Some(0.2)), ["soulscraft:revenant"]);
        // Nobody within the gate's reach: nothing.
        assert!(plains(None).is_empty());
        // The brute (0.5) in a cave needs P 0.5.
        let cave = |p| keys(&extras(&f.natural, &f, "minecraft:plains", 12, &["minecraft:zombie"], Some(p))).contains(&"soulscraft:brute");
        assert!(!cave(0.45) && cave(0.5));
        // The lurker's own spawner keeps the same gate.
        assert!(!f.natural.lurker_allowed(&f, Some(0.3)) && f.natural.lurker_allowed(&f, Some(0.35)));
        assert!(!f.natural.lurker_allowed(&f, None));
    }

    #[test]
    fn shrikes_come_in_their_group_size() {
        let f = file();
        let high = extras_at(&f, "minecraft:jagged_peaks", 120, &[]);
        let shrike = high.iter().find(|e| e.key == "soulscraft:shrike").unwrap();
        assert_eq!((shrike.min, shrike.max), (2, 3));
        // The row agrees with the mob's own group_size.
        let row = f.natural.mobs.iter().find(|m| m.mob == "shrike").unwrap();
        assert_eq!((row.min, row.max), (2, 3));
        // A mob with no group_size keeps its row's numbers.
        let marksman = high.iter().find(|e| e.key == "soulscraft:marksman").unwrap();
        assert_eq!((marksman.min, marksman.max), (1, 2));
    }

    #[test]
    fn a_mob_already_listed_is_not_added_again() {
        let f = file();
        let e = extras_at(&f, "minecraft:plains", 64, &["minecraft:zombie", "soulscraft:revenant"]);
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

    #[test]
    fn a_credited_kill_drops_shards_horn_head_and_armour_by_their_chances() {
        let f = file();
        let c = crate::event::TrophyChances::embedded();
        let revenant = f.mob("revenant").unwrap();
        let brute = f.mob("brute").unwrap();
        // Every roll a hit: shards, the tier-2 horn, the head, each armour piece.
        let all = world_drops(&f, &c, revenant, || 0.0);
        let items: Vec<&str> = all.iter().map(|d| d.0.as_str()).collect();
        assert_eq!(items[..3], ["soulscraft:soul_shard", "soulscraft:grave_horn", "soulscraft:revenant_head"]);
        assert_eq!(items.len(), 3 + revenant.loot.len());
        // The elite's horn is the tier-3 one, its shards two.
        let elite = world_drops(&f, &c, brute, || 0.0);
        assert_eq!(elite[0], ("soulscraft:soul_shard".into(), 2));
        assert_eq!(elite[1].0, "soulscraft:storm_horn");
        // Every roll a miss: nothing (the elite's shards are certain).
        assert!(world_drops(&f, &c, revenant, || 0.999).is_empty());
        assert_eq!(world_drops(&f, &c, brute, || 0.999), [("soulscraft:soul_shard".into(), 2)]);
    }

    #[test]
    fn world_drop_rates_follow_the_data() {
        let f = file();
        let c = crate::event::TrophyChances::embedded();
        let revenant = f.mob("revenant").unwrap();
        let mut seed = 0x9e3779b97f4a7c15u64;
        let mut roll = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let n = 200_000;
        let mut count = |item: &str| -> f64 {
            let mut hits = 0u32;
            for _ in 0..n {
                hits += world_drops(&f, &c, revenant, &mut roll).iter().filter(|d| d.0 == item).count() as u32;
            }
            f64::from(hits) / f64::from(n)
        };
        assert!((count("soulscraft:grave_horn") - 0.005).abs() < 0.001);
        assert!((count("soulscraft:revenant_head") - 0.01).abs() < 0.0015);
        let piece = &revenant.loot[0];
        assert!((count(&piece.item) - piece.chance).abs() < 0.003, "{}", piece.item);
    }
}
