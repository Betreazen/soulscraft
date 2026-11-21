// A position's possible natural spawns: the biome's SoulsCraft mobs and an altar sector's extras.
fn soulscraft_potential_spawns(
    game: &Game<'_>,
    dimension: &str,
    category: &str,
    pos: [i32; 3],
    entries: &mut Vec<SpawnEntry>,
) {
    crate::game::soulscraft_natural::on_potential_spawns(game, dimension, category, pos, entries);
    crate::game::soulscraft_spawn_boost::on_potential_spawns(dimension, category, pos, entries);
}
