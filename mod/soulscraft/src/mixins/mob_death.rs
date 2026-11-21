// A mob's health ran out (LivingEntity.die): the killer from its last damage and
// `last_hurt_by_player`, before the death is told (native_hooks/living_death.rs). A dead shrike
// gets its gravity back (soulscraft_server::on_mob_death).
fn soulscraft_mob_death(game: &mut Game<'_>, mob: &horizon_client::mobs::Mob) {
    crate::game::soulscraft_book::on_mob_death(game, mob);
    crate::game::soulscraft_server::on_mob_death(game, mob);
}
