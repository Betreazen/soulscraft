fn soulscraft_block_entity_tick(pos: [i32; 3], entity: &horizon_client::containers::ModBlockEntity) {
    crate::game::soulscraft_altar::on_block_entity_tick(pos, entity);
    crate::game::soulscraft_trophy::on_block_entity_tick(pos, entity);
}
