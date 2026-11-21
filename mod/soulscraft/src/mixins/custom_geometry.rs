// Each frame's world geometry: the debug labels over the mobs (/sc debug on), the altar's beams
// and wave points.
fn soulscraft_custom_geometry(
    game: &Game<'_>,
    view: &mod_api::WorldRenderView,
    draw: &mut horizon_client::world_geometry::WorldGeometry,
) {
    crate::game::soulscraft_client::debug_labels(game, view, draw);
    crate::game::soulscraft_altar_client::primitives(game, view, draw);
}
