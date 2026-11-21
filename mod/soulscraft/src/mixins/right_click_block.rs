fn soulscraft_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    crate::game::soulscraft_trophy::on_right_click_block(game, pos, face, canceled);
    crate::game::soulscraft_altar::on_right_click_block(game, pos, canceled);
}
