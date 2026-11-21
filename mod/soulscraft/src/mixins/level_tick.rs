fn soulscraft_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::soulscraft_server::on_level_tick(game, dimension);
}
