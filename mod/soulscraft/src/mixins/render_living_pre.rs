// Every mob in view each frame: SoulsCraft's are drawn from their rigs, and `canceled` keeps the
// zombie template's model out (the draw's geometry is kept: modding/native_mob_draw.rs).
fn soulscraft_render_living_pre(
    game: &Game<'_>,
    entity: &MobRender<'_>,
    draw: &mut BlockEntityDraw,
    canceled: &mut bool,
) {
    crate::game::soulscraft_client::draw_mob(game, entity, draw, canceled);
}
