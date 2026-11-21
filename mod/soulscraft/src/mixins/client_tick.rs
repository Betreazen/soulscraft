// The window's tick: the server's notes (payloads and, on a replay, custom payload packets, both
// queued by the mod crate), the menus and the book's window, and the boxes of new mob copies.
fn soulscraft_client_tick(game: &mut Game<'_>) {
    for (channel, data) in soulscraft_rules::bridge::take_client() {
        crate::game::soulscraft_client::on_net(game, &channel, &data);
        crate::game::soulscraft_altar_client::on_net(game, &channel, &data);
        crate::game::soulscraft_book_client::on_net(game, &channel, &data);
    }
    crate::game::soulscraft_goals::fit_all(game);
    crate::game::soulscraft_altar_client::on_tick(game);
    crate::game::soulscraft_book_client::on_tick(game);
}
