fn soulscraft_block_entity_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    if !crate::game::soulscraft_trophy_client::draw_block_entity(game, pos, block, draw) {
        crate::game::soulscraft_altar_client::draw_block_entity(game, pos, block, entity, draw);
    }
}