// A mob joining a server level, before its first tick: SoulsCraft's types drop the zombie
// template's goals and take their own box and flags (soulscraft_goals). A client's copies are
// fitted on the client tick (client_tick.rs).
fn soulscraft_entity_join(game: &mut Game<'_>, entity: u32, kind: &str) {
    if !kind.starts_with("soulscraft:") {
        return;
    }
    if let Some(mob) = game.mobs.iter_mut().find(|m| m.id == entity) {
        crate::game::soulscraft_goals::fit(mob);
    }
}
