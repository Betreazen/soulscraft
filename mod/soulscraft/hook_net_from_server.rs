fn soulscraft_net_from_server(game: &crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::soulscraft_client::on_net(game, channel, data);
    crate::game::soulscraft_altar_client::on_net(game, channel, data);
}
