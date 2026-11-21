// Their buttons.
fn soulscraft_screen_press(game: &Game<'_>, screen: &ScreenView<'_>, button: i32, canceled: &mut bool) {
    crate::game::soulscraft_altar_client::on_screen_press(game, screen, button, canceled);
    crate::game::soulscraft_book_client::on_screen_press(game, screen, button, canceled);
}
