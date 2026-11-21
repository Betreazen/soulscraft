// A right click on a block, before the block's own use: a head item on a block's side goes up as
// a wall head, the pedestal and the core open their menus. It fires once per hand, on the
// clicking client and on the server: only the server's main hand acts, and a cancel ends the
// click as used (`Pass` would go on to the other hand).
fn soulscraft_right_click_block(
    game: &mut Game<'_>,
    click: &BlockClick<'_>,
    cancellation_result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    if !matches!(click.side, mod_api::LogicalSide::Server) || !matches!(click.hand, mod_api::Hand::Main) {
        return;
    }
    crate::game::soulscraft_trophy::on_right_click_block(game, click.pos, click.hit.face, canceled);
    crate::game::soulscraft_altar::on_right_click_block(game, click.pos, canceled);
    if *canceled {
        *cancellation_result = mod_api::InteractionResult::Success;
    }
}
