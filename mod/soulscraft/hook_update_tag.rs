fn soulscraft_update_tag(
    entity: &horizon_client::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::soulscraft_altar::on_update_tag(entity, tag);
}
