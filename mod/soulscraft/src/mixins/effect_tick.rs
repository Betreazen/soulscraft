// soulscraft:bleeding's hurt, inside the entity's own tick: queued for the level's next tick.
fn soulscraft_effect_tick(game: &Game<'_>, call: &mod_api::__private::ApplyEffectTickCall<'_>) {
    if soulscraft::bridge::is_bleeding(call.effect) {
        crate::game::soulscraft_server::bleed_tick(game, "soulscraft:bleeding", call.dimension, call.entity, call.amplifier);
    }
}
