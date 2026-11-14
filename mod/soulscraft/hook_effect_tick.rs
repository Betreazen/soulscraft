fn soulscraft_effect_tick(
    game: &crate::game::Game<'_>,
    effect: &str,
    dimension: &str,
    entity: Option<i32>,
    amplifier: i32,
) {
    crate::game::soulscraft_server::bleed_tick(game, effect, dimension, entity, amplifier);
}
