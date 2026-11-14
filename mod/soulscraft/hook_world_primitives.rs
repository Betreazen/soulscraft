fn soulscraft_world_primitives(
    game: &crate::game::Game<'_>,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::soulscraft_client::debug_labels(game, primitives);
    crate::game::soulscraft_altar_client::primitives(game, primitives);
}
