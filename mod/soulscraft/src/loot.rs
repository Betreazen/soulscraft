//! SoulsCraft's changes to vanilla loot tables (docs/design/11-summons-rewards.md, Г17 and Г18):
//! summoning horns and soul shards in structure chests, and the Rotten Horn from zombies a player
//! killed. `apply` runs on every table as it loads (horizon_world::loot::native_loot_table_load),
//! on a server and a client alike, and makes the same change each time.

use serde_json::{Value, json};
use std::sync::OnceLock;

/// The numbers, `data/loot.json` (copied into the package as data/soulscraft/loot.json by mod/sync.ps1).
pub const LOOT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/soulscraft/loot.json"));

/// One structure chest's summoning horn.
#[derive(Clone, Debug, PartialEq)]
pub struct ChestHorn {
    pub table: String,
    pub item: String,
    pub chance: f64,
}

/// Soul shards in every chest that has a horn.
#[derive(Clone, Debug, PartialEq)]
pub struct ChestShards {
    pub item: String,
    pub chance: f64,
    pub count: [u32; 2],
}

/// An item that mobs of `tables` drop when a player killed them.
#[derive(Clone, Debug, PartialEq)]
pub struct KilledDrop {
    pub tables: Vec<String>,
    pub item: String,
    pub chance: f64,
}

/// `data/loot.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct LootFile {
    pub chests: Vec<ChestHorn>,
    pub chest_shards: Option<ChestShards>,
    pub killed_by_player: Vec<KilledDrop>,
}

impl LootFile {
    /// The compiled-in file, read once.
    pub fn embedded() -> &'static LootFile {
        static FILE: OnceLock<LootFile> = OnceLock::new();
        FILE.get_or_init(|| LootFile::parse(LOOT))
    }

    /// Reads the file; a malformed row is skipped, a malformed file gives no changes.
    pub fn parse(text: &str) -> LootFile {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let chance = |r: &Value| r["chance"].as_f64().filter(|c| (0.0..=1.0).contains(c));
        let chests = v["chests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(ChestHorn { table: r["table"].as_str()?.to_owned(), item: r["item"].as_str()?.to_owned(), chance: chance(r)? })
            })
            .collect();
        let s = &v["chest_shards"];
        let chest_shards = (|| {
            let min = s["count"][0].as_u64()? as u32;
            let max = s["count"][1].as_u64()? as u32;
            Some(ChestShards { item: s["item"].as_str()?.to_owned(), chance: chance(s)?, count: [min.max(1), max.max(min.max(1))] })
        })();
        let killed_by_player = v["killed_by_player"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let tables = r["tables"].as_array()?.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect();
                Some(KilledDrop { tables, item: r["item"].as_str()?.to_owned(), chance: chance(r)? })
            })
            .collect();
        LootFile { chests, chest_shards, killed_by_player }
    }

    /// The pools this file adds to the table `id`, in order: a chest's horn and its shards, a
    /// mob's drops when a player killed it. None for a table the file doesn't name.
    pub fn pools(&self, id: &str) -> Vec<Value> {
        let mut out = Vec::new();
        for c in self.chests.iter().filter(|c| c.table == id) {
            out.push(pool(&c.item, chance_condition(c.chance), None));
            if let Some(s) = &self.chest_shards {
                let count = json!({ "type": "minecraft:uniform", "min": s.count[0], "max": s.count[1] });
                out.push(pool(&s.item, chance_condition(s.chance), Some(count)));
            }
        }
        for k in self.killed_by_player.iter().filter(|k| k.tables.iter().any(|t| t == id)) {
            let conditions = json!([{ "type": "minecraft:killed_by_player" }, { "type": "minecraft:random_chance", "chance": k.chance }]);
            out.push(pool(&k.item, conditions, None));
        }
        out
    }
}

fn chance_condition(chance: f64) -> Value {
    json!([{ "type": "minecraft:random_chance", "chance": chance }])
}

/// One roll of one item under `conditions`, with a count when given. In the 26.3 shape Horizon's
/// loot reads (horizon-world/src/loot.rs `table_raw`, data/minecraft/loot_table): `condition` and
/// `modifier` with a `type` each, `rolls` a whole number (`1.0` would roll nothing).
fn pool(item: &str, conditions: Value, count: Option<Value>) -> Value {
    let mut entry = json!({ "type": "minecraft:item", "name": item });
    if let Some(count) = count {
        entry["modifier"] = json!([{ "type": "minecraft:set_count", "count": count }]);
    }
    json!({ "rolls": 1, "condition": conditions, "entries": [entry] })
}

/// Adds the file's pools to the table `id` as it loads. The table's own pools stay as they are;
/// a table without `pools` gets the list.
pub fn apply(id: &str, table: &mut Value) {
    let pools = LootFile::embedded().pools(id);
    if pools.is_empty() || !table.is_object() {
        return;
    }
    if !table["pools"].is_array() {
        table["pools"] = json!([]);
    }
    if let Some(list) = table["pools"].as_array_mut() {
        list.extend(pools);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_reads_every_row() {
        let f = LootFile::embedded();
        assert_eq!(f.chests.len(), 12);
        assert!(f.chest_shards.is_some());
        assert_eq!(f.killed_by_player.len(), 1);
        assert_eq!(f.killed_by_player[0].tables.len(), 4);
    }

    #[test]
    fn each_tier_has_its_own_structures() {
        let f = LootFile::embedded();
        let tables = |item: &str| f.chests.iter().filter(|c| c.item == item).map(|c| c.table.as_str()).collect::<Vec<_>>();
        assert_eq!(tables("soulscraft:rotten_horn"), ["minecraft:chests/simple_dungeon", "minecraft:chests/abandoned_mineshaft"]);
        assert!(tables("soulscraft:grave_horn").contains(&"minecraft:chests/desert_pyramid"));
        assert!(tables("soulscraft:storm_horn").contains(&"minecraft:chests/woodland_mansion"));
        assert!(tables("soulscraft:abyss_horn").contains(&"minecraft:chests/ancient_city"));
        // One horn per chest table: no structure gives two tiers.
        let mut all: Vec<&str> = f.chests.iter().map(|c| c.table.as_str()).collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), f.chests.len());
    }

    #[test]
    fn a_chest_gets_its_horn_and_shards_after_its_own_pools() {
        let mut table = json!({ "type": "minecraft:chest", "pools": [{ "rolls": 1.0, "entries": [] }], "random_sequence": "minecraft:chests/simple_dungeon" });
        apply("minecraft:chests/simple_dungeon", &mut table);
        let pools = table["pools"].as_array().unwrap();
        assert_eq!(pools.len(), 3);
        assert_eq!(pools[1]["entries"][0]["name"], "soulscraft:rotten_horn");
        assert_eq!(pools[1]["condition"][0]["chance"], 0.25);
        assert_eq!(pools[1]["rolls"], 1);
        assert_eq!(pools[2]["entries"][0]["name"], "soulscraft:soul_shard");
        assert_eq!(pools[2]["entries"][0]["modifier"][0]["type"], "minecraft:set_count");
        assert_eq!(pools[2]["entries"][0]["modifier"][0]["count"]["max"], 4);
        assert_eq!(table["random_sequence"], "minecraft:chests/simple_dungeon");
    }

    #[test]
    fn a_zombie_drops_the_rotten_horn_only_to_a_player() {
        let mut table = json!({ "type": "minecraft:entity", "pools": [] });
        apply("minecraft:entities/husk", &mut table);
        let p = &table["pools"][0];
        assert_eq!(p["entries"][0]["name"], "soulscraft:rotten_horn");
        assert_eq!(p["condition"][0]["type"], "minecraft:killed_by_player");
        assert_eq!(p["condition"][1]["type"], "minecraft:random_chance");
        assert_eq!(p["condition"][1]["chance"], 0.005);
    }

    #[test]
    fn other_tables_stay_as_they_are() {
        for id in ["minecraft:chests/village/village_weaponsmith", "minecraft:entities/skeleton", "minecraft:blocks/stone"] {
            let mut table = json!({ "pools": [{ "rolls": 1.0 }] });
            let before = table.clone();
            apply(id, &mut table);
            assert_eq!(table, before, "{id}");
        }
    }

    #[test]
    fn a_table_without_pools_gets_them_and_a_malformed_file_changes_nothing() {
        let mut table = json!({});
        apply("minecraft:chests/ancient_city", &mut table);
        assert_eq!(table["pools"].as_array().unwrap().len(), 2);
        let bad = LootFile::parse("not json");
        assert!(bad.pools("minecraft:chests/ancient_city").is_empty());
        let wrong = LootFile::parse(r#"{"chests":[{"table":"minecraft:chests/x","item":"a","chance":3.0}]}"#);
        assert!(wrong.chests.is_empty());
    }

    #[test]
    fn pools_use_the_26_3_shape_horizon_reads() {
        let f = LootFile::embedded();
        for id in ["minecraft:chests/woodland_mansion", "minecraft:entities/zombie"] {
            let text = serde_json::to_string(&f.pools(id)).unwrap();
            for old in ["\"conditions\"", "\"functions\"", "\"function\"", "\"condition\":\"minecraft:", "\"bonus_rolls\"", "1.0,\"entries\""] {
                assert!(!text.contains(old), "{id}: {old} in {text}");
            }
            for p in f.pools(id) {
                assert!(p["rolls"].is_u64(), "{id}: rolls {}", p["rolls"]);
            }
        }
    }
}
