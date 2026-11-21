// Natural spawning's candidate chunks: an altar sector's chunks come again.
fn soulscraft_spawning_chunks(value: i32, game: &Game<'_>, scratch: &mut SpawnScratch) -> i32 {
    crate::game::soulscraft_spawn_boost::on_spawning_chunks(game, &mut scratch.candidates);
    value
}
