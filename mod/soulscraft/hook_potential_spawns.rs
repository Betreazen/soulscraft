fn soulscraft_potential_spawns(
    dimension: &str,
    category: &str,
    pos: [i32; 3],
    entries: &mut Vec<horizon_client::biome::SpawnEntry>,
) {
    crate::game::soulscraft_spawn_boost::on_potential_spawns(dimension, category, pos, entries);
}