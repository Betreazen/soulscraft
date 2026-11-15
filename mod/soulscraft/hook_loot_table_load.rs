fn soulscraft_loot_table_load(id: &str, table: &mut serde_json::Value) {
    soulscraft_loot::apply(id, table);
}
