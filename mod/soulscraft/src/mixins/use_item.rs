fn soulscraft_use_item(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::soulscraft_book::on_use_item(game, stack, claimed);
}
