fn soulscraft_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::soulscraft_tools::on_command(game, Some(player), command, ran);
}
