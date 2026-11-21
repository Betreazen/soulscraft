// The pedestal's item, the core's ghost copies, heads and dioramas, and the book's figure.
fn soulscraft_block_entity_submit(
    game: &Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    if crate::game::soulscraft_book_client::draw_figure(game, entity, draw) {
        return;
    }
    if !crate::game::soulscraft_trophy_client::draw_block_entity(game, pos, block, entity, draw) {
        crate::game::soulscraft_altar_client::draw_block_entity(game, pos, block, entity, draw);
    }
}
