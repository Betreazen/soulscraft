fn soulscraft_potential_spawns(
    game: &crate::game::Game<'_>,
    dimension: &str,
    category: &str,
    pos: [i32; 3],
    entries: &mut Vec<horizon_client::biome::SpawnEntry>,
) {
    crate::game::soulscraft_natural::on_potential_spawns(game, dimension, category, pos, entries);
    crate::game::soulscraft_spawn_boost::on_potential_spawns(dimension, category, pos, entries);
}
