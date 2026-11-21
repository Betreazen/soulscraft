// The altar menu and the book's window, drawn over the mod screen.
fn soulscraft_screen_render_post(game: &Game<'_>, batch: &mut HudBatch, screen: &ScreenView<'_>) {
    crate::game::soulscraft_altar_client::draw_screen(game, batch, screen);
    crate::game::soulscraft_book_client::draw_screen(game, batch, screen);
}
