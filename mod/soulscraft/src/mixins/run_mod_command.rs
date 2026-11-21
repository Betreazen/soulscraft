fn soulscraft_run_mod_command(
    game: &mut Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::soulscraft_tools::on_mod_command(game, source, command, ran);
}
