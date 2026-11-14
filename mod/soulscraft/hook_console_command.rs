fn soulscraft_console_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::soulscraft_book::on_console(command);
    if !crate::game::soulscraft_book::on_command(game, command, ran) {
        crate::game::soulscraft_tools::on_command(game, None, command, ran);
    }
}
