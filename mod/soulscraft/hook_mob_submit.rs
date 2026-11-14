fn soulscraft_mob_submit(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
    drawn: &mut bool,
) {
    crate::game::soulscraft_client::draw_mob(game, mob, draw, drawn);
}
