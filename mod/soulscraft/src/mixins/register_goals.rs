// Compiled into horizon-client-lib (mobs/ai.rs), which has no `crate::game` and no rules crate:
// SoulsCraft's types keep only Float, so nothing but the server half's brain moves or strikes.
// Asked for every mob Horizon makes, a client's copies of a server's mobs included (ai.rs:451-463),
// so the box set here is the one the server collides and paths with (path.rs:430-432) and the one
// a client picks and draws as its hitbox.
fn soulscraft_register_goals(mob: &mut crate::mobs::Mob) {
    if !mob.kind.id.starts_with("soulscraft:") {
        return;
    }
    mob.ai.goals.clear();
    mob.ai.targets.clear();
    mob.ai.goals.add(0, crate::mobs::goals::Float);
    // The zombie class burns in daylight; SoulsCraft's mobs are out by day too (bestiary).
    if mob.kind.flags.burns_in_day {
        std::sync::Arc::make_mut(&mut mob.kind).flags.burns_in_day = false;
    }
    // The zombie template marks every type undead (poison and regeneration do nothing, instant
    // harm heals: mobs/ai.rs:3056-3059, mobs.rs:4272-4278). Round 11: these types are living.
    // The rules crate's test `the_goal_hook_undead_matches_the_data` keeps the list equal to
    // data/mobs.json `undead: false`.
    const LIVING: [&str; 3] = ["soulscraft:stalker", "soulscraft:lurker", "soulscraft:bog_slime"];
    if mob.kind.flags.undead && LIVING.contains(&&*mob.kind.id) {
        std::sync::Arc::make_mut(&mut mob.kind).flags.undead = false;
    }
    // Each type's own box (data/mobs.json `hitbox`: width, height, eyes in blocks) in place of
    // the zombie template's 0.6 × 1.95: a living type's size is its template's row
    // (mod_content.rs:829-833), and a mob's box is its type's `width`/`height` (mobs/living.rs:489).
    // The rules crate's test `the_goal_hook_boxes_match_the_data` keeps this table equal to the
    // data; the server sets a smaller slime's box again from its size (server.rs, `fit_hitbox`).
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
