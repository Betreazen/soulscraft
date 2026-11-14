fn soulscraft_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::soulscraft_altar::on_block_entity(block, entity);
    crate::game::soulscraft_trophy::on_block_entity(block, entity);
}
