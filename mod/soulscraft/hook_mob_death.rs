fn soulscraft_mob_death(game: &crate::game::Game<'_>, mob: &horizon_client::mobs::Mob) {
    crate::game::soulscraft_book::on_mob_death(game, mob);
}
