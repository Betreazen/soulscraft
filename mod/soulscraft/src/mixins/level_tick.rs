fn soulscraft_level_tick(game: &mut Game<'_>, level: mod_api::LevelRef) {
    // The hook fires on clients too now (network/remote/client_level_tick.rs): the server half only.
    if !crate::game::soulscraft_compat::is_server(game) {
        return;
    }
    soulscraft_rules::bridge::install_incoming_damage(crate::game::soulscraft_server::on_incoming_damage);
    soulscraft_rules::bridge::set_bleed_period(crate::game::soulscraft_server::bleed_numbers().1);
    for (player, channel, data) in soulscraft_rules::bridge::take_server() {
        if channel == soulscraft_rules::view::PRESS {
            crate::game::soulscraft_altar::on_press(player, &data);
        }
    }
    crate::game::soulscraft_goals::fit_all(game);
    crate::game::soulscraft_server::on_level_tick(game, level.dimension_id());
}
