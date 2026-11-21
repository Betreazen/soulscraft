fn soulscraft_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::soulscraft_altar::on_destroy_block(game, pos);
}
