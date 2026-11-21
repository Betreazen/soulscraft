fn soulscraft_incoming_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    amount: &mut f32,
    canceled: &mut bool,
) {
    let _ = canceled;
    crate::game::soulscraft_server::on_incoming_damage(entity, kind, damage_type, causing, amount);
}
