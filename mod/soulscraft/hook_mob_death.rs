fn soulscraft_mob_death(game: &mut crate::game::Game<'_>, mob: &horizon_client::mobs::Mob) {
    crate::game::soulscraft_book::on_mob_death(game, mob);
    crate::game::soulscraft_server::on_mob_death(game, mob);
}
