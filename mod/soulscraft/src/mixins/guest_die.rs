// The hardcore guard: `ServerPlayer.die` for a guest. A participant of a running event whose
// health ran out is kept at 1 health, so the body does nothing; soulscraft_event heals them and
// puts them outside the zone on the next tick.
fn soulscraft_guest_die(game: &mut Game<'_>, gi: usize) {
    let Some(host) = game.lan.as_mut() else { return };
    let Some(g) = host.hub.guests.get(gi) else { return };
    if g.health <= 0.0 && !g.dead && crate::game::soulscraft_event::spare_death(g.uuid) {
        host.hub.guests.update(gi, |g| g.health = 1.0);
    }
}
