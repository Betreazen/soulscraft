fn soulscraft_guest_die(host: &mut crate::game::lan::LanHost, gi: usize) {
    let Some(g) = host.hub.guests.get_mut(gi) else { return };
    if g.health <= 0.0 && !g.dead && crate::game::soulscraft_event::spare_death(g.uuid) {
        g.health = 1.0;
    }
}
