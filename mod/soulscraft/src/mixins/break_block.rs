// A player about to break a block: a creative player's tall altar piece drops nothing.
fn soulscraft_break_block(game: &mut Game<'_>, pos: [i32; 3]) {
    crate::game::soulscraft_altar::on_destroy_block(game, pos);
}
