fn soulscraft_block_changed(
    level: &crate::game::GameLevel<'_>,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    old: horizon_world::registry::BlockStateId,
    new: horizon_world::registry::BlockStateId,
) {
    crate::game::soulscraft_altar::on_block_changed(level.facts, dimension, [pos.x, pos.y, pos.z], old, new);
}
