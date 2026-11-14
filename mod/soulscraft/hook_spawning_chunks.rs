fn soulscraft_spawning_chunks(value: i32, game: &crate::game::Game<'_>, scratch: &mut crate::game::natural_spawning::SpawnScratch) -> i32 {
    crate::game::soulscraft_spawn_boost::on_spawning_chunks(game, &mut scratch.candidates);
    value
}
