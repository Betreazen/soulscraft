// SoulsCraft's living types on their template: the bundle era's `register_goals` hook, which ran
// at every mob's construction. 8f83e5c builds mobs in horizon_client_lib, where nothing takes a
// mixin, so a new mob is fitted as it joins a server level (src/mixins/entity_join.rs) and, for a
// client's copies, on the client tick (src/mixins/client_tick.rs); both sides also sweep their
// mob lists for one not fitted yet (`fit_all`, on the level and client ticks).
//
// Compiled as `game::soulscraft_goals`. The rules crate's tests `the_goal_hook_boxes_match_the_data`
// and `the_goal_hook_undead_matches_the_data` keep the two tables below equal to data/mobs.json.

/// The zombie class burns in daylight; SoulsCraft's mobs are out by day too (bestiary). A fitted
/// mob's type has it cleared, so a type that still burns is one not fitted yet.
fn fitted(mob: &horizon_client::mobs::Mob) -> bool {
    !mob.kind.id.starts_with("soulscraft:") || !mob.kind.flags.burns_in_day
}

/// Every mob of the level in place not fitted yet (a loaded chunk's, another level's arrival, a
/// client's new copy).
pub(super) fn fit_all(game: &mut super::Game<'_>) {
    for mob in game.mobs.iter_mut() {
        if !fitted(mob) {
            fit(mob);
        }
    }
}

/// SoulsCraft's types keep only Float, so nothing but the server half's brain moves or strikes.
/// The box set here is the one the server collides and paths with and the one a client picks
/// and draws as its hitbox.
pub(super) fn fit(mob: &mut horizon_client::mobs::Mob) {
    if !mob.kind.id.starts_with("soulscraft:") {
        return;
    }
    mob.ai.goals.clear();
    mob.ai.targets.clear();
    mob.ai.goals.add(0, horizon_client::mobs::goals::Float);
    if mob.kind.flags.burns_in_day {
        std::sync::Arc::make_mut(&mut mob.kind).flags.burns_in_day = false;
    }
    // The zombie template marks every type undead (poison and regeneration do nothing, instant
    // harm heals). Round 11: these types are living.
    const LIVING: [&str; 3] = ["soulscraft:stalker", "soulscraft:lurker", "soulscraft:bog_slime"];
    if mob.kind.flags.undead && LIVING.contains(&&*mob.kind.id) {
        std::sync::Arc::make_mut(&mut mob.kind).flags.undead = false;
    }
    // Each type's own box (data/mobs.json `hitbox`: width, height, eyes in blocks) in place of
    // the zombie template's 0.6 × 1.95: a living type's size is its template's row, and a mob's
    // box is its type's `width`/`height`. The server sets a smaller slime's box again from its
    // size (server.rs, `fit_hitbox`).
    const HITBOXES: [(&str, f64, f64, f64); 7] = [
        ("soulscraft:revenant", 0.6, 1.95, 1.8),
        ("soulscraft:stalker", 0.6, 1.95, 1.6),
        ("soulscraft:marksman", 0.65, 1.95, 1.8),
        ("soulscraft:brute", 1.6, 3.2, 2.65),
        ("soulscraft:shrike", 0.5, 0.9, 0.7),
        ("soulscraft:lurker", 0.75, 1.0, 0.6),
        ("soulscraft:bog_slime", 0.55, 0.75, 0.5),
    ];
    if let Some(&(_, w, h, e)) = HITBOXES.iter().find(|b| b.0 == mob.kind.id) {
        let kind = std::sync::Arc::make_mut(&mut mob.kind);
        (kind.width, kind.height, kind.eye_height) = (w, h, e);
    }
}
