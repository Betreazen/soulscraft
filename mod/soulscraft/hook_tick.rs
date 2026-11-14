fn soulscraft_tick(game: &mut crate::game::Game<'_>) {
    crate::game::soulscraft_altar_client::on_tick(game);
    crate::game::soulscraft_book_client::on_tick(game);
}
