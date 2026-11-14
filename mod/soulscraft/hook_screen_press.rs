fn soulscraft_screen_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::soulscraft_altar_client::on_screen_press(game, screen, button, canceled);
}
