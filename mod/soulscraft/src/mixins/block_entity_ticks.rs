fn soulscraft_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::soulscraft_altar::on_block_entity_ticks(block, ticks);
    crate::game::soulscraft_trophy::on_block_entity_ticks(block, ticks);
}
