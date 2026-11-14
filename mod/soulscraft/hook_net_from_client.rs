fn soulscraft_net_from_client(player: u128, channel: &str, data: &[u8]) {
    if channel == soulscraft_rules::view::PRESS {
        crate::game::soulscraft_altar::on_press(player, data);
    }
}
