// The joint scenarios with RustCraft's turrets (soulscraft_selftest run turret; left out of the
// default run). RustCraft's test kit places a powered turret that fires at once:
//   rustcraft_testkit turret <x> <y> <z> [modules=bleed,poison,...] [owner=<uuid|name>]
// (server console, RustCraft 0.8.0). The turret shoots hostile mobs within 12 blocks, once a
// second, 4 damage, with its modules' effects; SoulsCraft's resistances (data/mobs.json
// `turret_effects`) are RustCraft's to apply. Without RustCraft in the build each scenario is
// skipped (passed, with the reason in the log): it is a soft dependency.
//
// RustCraft 0.10.0 on also runs SoulsCraft's `soulscraft-kill turret <owner> <id> <type> x y z` for
// each kill (docs/research/integration.md 4.2).
//
// Scenarios: the revenant takes no poison; the brute holds bleed and slowness half as long as a
// zombie and takes no poison; the bleed module gives `soulscraft:bleeding`; a turret's kill (what
// SoulsCraft can see of the owner's credit); the same kill in the owner's hunter's book, once; a mob
// takes cover from an attack turret and not from a support one.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_selftest_altar::{at, centre, clear_site};
use soulscraft::rules::geom::{self, V3};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

const DIM: &str = "minecraft:overworld";
const KIT: &str = "rustcraft_testkit";
/// The owner the kill scenario gives its turret (a fixed test UUID: no players on the bench).
pub(super) const OWNER: &str = "5eed50c1c0de4a11a9e000000000c0de";
/// Where the scenarios stand, 12 blocks north of the altar site's core.
const SPOT: [i32; 3] = [0, 0, -12];

pub(super) const CASES: &[(&str, u64)] = &[
    ("turret_poison_revenant", 140),
    ("turret_brute_half_durations", 320),
    ("turret_bleed_module", 140),
    ("turret_kill_credit", 160),
    ("turret_kill_book", 200),
    ("turret_cover_attack", 220),
    ("turret_cover_support", 160),
];

#[derive(Default)]
struct Case {
    mark: u64,
    phase: u32,
    mob: Option<u32>,
    turret: Option<[i32; 3]>,
    /// Effect id → ticks left when first seen, per phase.
    seen: Vec<HashMap<String, i32>>,
    start: Option<V3>,
    health: f64,
}

static CASE: Mutex<Option<Case>> = Mutex::new(None);

fn kit_present(game: &Game<'_>) -> bool {
    game.sc_command_info(KIT).is_some()
}

fn turret(game: &mut Game<'_>, c: [i32; 3], modules: &str) -> Option<[i32; 3]> {
    let p = at(c);
    let line = format!("{KIT} turret {} {} {} modules={modules} owner={OWNER}", p[0], p[1], p[2]);
    // A mod's console command: `native_run_command` reaches vanilla's only.
    if !game.sc_run_mod_command(&line) {
        log::warn!("soulscraft: selftest command refused: {line}");
        return None;
    }
    Some(p)
}

fn remove_turret(game: &mut Game<'_>, p: [i32; 3]) {
    game.sc_run_command(&format!("setblock {} {} {} air", p[0], p[1], p[2]));
}

fn summon(game: &mut Game<'_>, kind: &str, c: [i32; 3], extra: &str) -> Option<u32> {
    let tags = if kind.starts_with("soulscraft:") { "Tags:[\"soulscraft.p=0.00\",\"soulscraft.fresh\"]," } else { "" };
    let snbt = format!("{{{tags}{extra}PersistenceRequired:1b}}");
    let Ok((horizon_core::nbt::Tag::Compound(nbt), _)) = horizon_core::nbt::parse_snbt(&snbt) else { return None };
    game.sc_summon(DIM, kind, centre(at(c)), nbt)
}

fn mob<'a>(game: &'a Game<'_>, id: u32) -> Option<&'a horizon_client::mobs::Mob> {
    game.mobs.iter().find(|m| m.id == id && !m.removed)
}

fn effects(game: &Game<'_>, id: u32) -> Vec<(String, i32)> {
    mob(game, id).map(|m| m.extra.effects.iter().map(|(e, t, _)| ((*e).to_owned(), *t)).collect()).unwrap_or_default()
}

/// Records each effect's ticks left the first time it shows on the mob.
fn note_effects(game: &Game<'_>, id: u32, into: &mut HashMap<String, i32>) {
    for (e, t) in effects(game, id) {
        into.entry(e).or_insert(t);
    }
}

pub(super) fn cleanup(game: &mut Game<'_>) {
    let c = CASE.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(c) = c {
        if let Some(p) = c.turret {
            remove_turret(game, p);
        }
        if let Some(m) = c.mob {
            game.sc_remove_mob(DIM, m);
        }
    }
    // A broken turret drops its modules: the clear kills every item on the site.
    clear_site(game);
}

pub(super) fn step(game: &mut Game<'_>, now: u64, name: &str, first: bool) -> Option<Result<(), String>> {
    let mut guard = CASE.lock().unwrap_or_else(PoisonError::into_inner);
    if first {
        *guard = Some(Case { mark: now, seen: vec![HashMap::new(), HashMap::new()], ..Case::default() });
        drop(guard);
        clear_site(game);
        return None;
    }
    if !kit_present(game) {
        log::info!("soulscraft: selftest SKIP {name}: no {KIT} command (RustCraft is not in this build)");
        return Some(Ok(()));
    }
    let c = guard.as_mut()?;
    let t = now - c.mark;
    match name {
        "turret_poison_revenant" => poison_revenant(game, t, c),
        "turret_brute_half_durations" => brute_half(game, t, c),
        "turret_bleed_module" => bleed(game, t, c),
        "turret_kill_credit" => kill_credit(game, t, c),
        "turret_kill_book" => kill_book(game, t, c),
        "turret_cover_attack" => cover(game, t, c, true),
        "turret_cover_support" => cover(game, t, c, false),
        other => Some(Err(format!("no scenario {other}"))),
    }
}

/// A poison turret on a revenant (undead: `turret_effects.poison` 0): it is shot, never poisoned.
fn poison_revenant(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        2 => {
            c.mob = summon(game, "soulscraft:revenant", SPOT, "NoAI:1b,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], "poison");
            if c.mob.is_none() || c.turret.is_none() {
                return Some(Err("no revenant or no turret".into()));
            }
        }
        6 => c.health = f64::from(mob(game, c.mob?)?.health),
        _ if t > 6 => {
            let id = c.mob?;
            let hp = f64::from(mob(game, id)?.health);
            let poisoned = effects(game, id).iter().any(|(e, _)| e == "minecraft:poison");
            if poisoned {
                return Some(Err("the revenant is poisoned".into()));
            }
            // Shot twice (4 each, less its armour) with no poison: done.
            if hp <= c.health - 2.0 && t >= 60 {
                return Some(Ok(()));
            }
        }
        _ => {}
    }
    None
}

/// The same turret (bleed, poison, slowness) on a husk (a zombie that does not burn in daylight),
/// then on a brute: the brute's bleed and slowness last half as long (`turret_effects` 0.5), and it
/// is never poisoned (undead since round 11: `turret_effects.poison` 0, `#minecraft:undead`).
/// Husks stand in for zombies throughout: a still zombie burns in the sun and dies of it.
fn brute_half(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let modules = "bleed,poison,slow";
    match (c.phase, t) {
        (0, 2) => {
            c.mob = summon(game, "minecraft:husk", SPOT, "NoAI:1b,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], modules);
        }
        (0, _) if t > 2 => {
            let id = c.mob?;
            let mut seen = std::mem::take(&mut c.seen[0]);
            note_effects(game, id, &mut seen);
            c.seen[0] = seen;
            if c.seen[0].contains_key("soulscraft:bleeding") && c.seen[0].contains_key("minecraft:slowness") {
                game.sc_remove_mob(DIM, id);
                if let Some(p) = c.turret.take() {
                    remove_turret(game, p);
                }
                c.phase = 1;
                c.mark = c.mark.max(1);
                c.health = t as f64;
            } else if t > 120 {
                return Some(Err(format!("the zombie got {:?}", c.seen[0])));
            }
        }
        (1, _) if t as f64 == c.health + 3.0 => {
            c.mob = summon(game, "soulscraft:brute", SPOT, "NoAI:1b,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], modules);
        }
        (1, _) if t as f64 > c.health + 3.0 => {
            let id = c.mob?;
            let mut seen = std::mem::take(&mut c.seen[1]);
            note_effects(game, id, &mut seen);
            c.seen[1] = seen;
            let b = &c.seen[1];
            // No poison: the brute is undead (round 11), and a poison turret must not poison it.
            if b.contains_key("minecraft:poison") {
                return Some(Err(format!("the brute is poisoned: {b:?}")));
            }
            if b.contains_key("soulscraft:bleeding") && b.contains_key("minecraft:slowness") && t as f64 > c.health + 20.0 {
                let ratio = |e: &str| f64::from(b[e]) / f64::from(c.seen[0][e].max(1));
                let (rb, rs) = (ratio("soulscraft:bleeding"), ratio("minecraft:slowness"));
                let half = |r: f64| (0.4..=0.6).contains(&r);
                log::info!("soulscraft: selftest turret durations zombie={:?} brute={:?}", c.seen[0], c.seen[1]);
                return Some(if half(rb) && half(rs) { Ok(()) } else { Err(format!("brute/husk bleed {rb:.2}, slowness {rs:.2}: {:?} {:?}", c.seen[1], c.seen[0])) });
            }
            if t as f64 > c.health + 140.0 {
                return Some(Err(format!("the brute got {:?}, the husk {:?}", c.seen[1], c.seen[0])));
            }
        }
        _ => {}
    }
    None
}

/// A bleed turret on a zombie: `soulscraft:bleeding`, SoulsCraft's shared bleed, and it bleeds.
fn bleed(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        2 => {
            c.mob = summon(game, "minecraft:husk", SPOT, "NoAI:1b,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], "bleed");
        }
        _ if t > 2 => {
            let bleeding = effects(game, c.mob?).iter().any(|(e, _)| e == "soulscraft:bleeding");
            if bleeding {
                return Some(Ok(()));
            }
        }
        _ => {}
    }
    None
}

/// An attack turret with a fixed owner kills a zombie of 1 health. SoulsCraft sees the kill;
/// the owner's credit (experience, Path-of-craft-tree's `kill`) is RustCraft's and PCT's side:
/// the log is searched for the owner and what is found is logged, not checked.
fn kill_credit(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        2 => {
            c.mob = summon(game, "minecraft:husk", SPOT, "NoAI:1b,Health:1f,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], "slow");
            if c.mob.is_none() || c.turret.is_none() {
                return Some(Err("no husk or no turret".into()));
            }
        }
        _ if t > 2 => {
            let alive = mob(game, c.mob?).is_some_and(|m| m.alive());
            if !alive {
                let log = std::fs::read_to_string("logs/latest.log").unwrap_or_default();
                let lines: Vec<&str> = log.lines().filter(|l| l.contains(OWNER) && !l.contains("soulscraft:")).collect();
                log::info!(
                    "soulscraft: selftest turret kill: the zombie died; lines naming the owner in the log: {} {}",
                    lines.len(),
                    lines.last().map_or("(the credit is on RustCraft's and PCT's side only)", |l| *l)
                );
                return Some(Ok(()));
            }
        }
        _ => {}
    }
    None
}

/// A revenant in an attack turret's sight with a wall beside it goes behind the wall, out of the
/// turret's line of fire; next to a support turret it stays where it is.
fn cover(game: &mut Game<'_>, t: u64, c: &mut Case, attack: bool) -> Option<Result<(), String>> {
    match t {
        1 => {
            let w = at([SPOT[0] + 2, SPOT[1], SPOT[2] - 2]);
            game.sc_run_command(&format!("fill {} {} {} {} {} {} stone", w[0], w[1], w[2], w[0], w[1] + 2, w[2] + 4));
        }
        3 => {
            c.mob = summon(game, "soulscraft:revenant", SPOT, "");
            c.turret = turret(game, [SPOT[0] - 6, SPOT[1], SPOT[2]], if attack { "bleed" } else { "heal" });
            c.start = c.mob.and_then(|id| mob(game, id)).map(|m| [m.position.x, m.position.y, m.position.z]);
        }
        _ if t > 3 => {
            let id = c.mob?;
            let m = mob(game, id)?;
            let pos = [m.position.x, m.position.y, m.position.z];
            let moved = geom::flat_dist(pos, c.start?);
            let muzzle = { let p = c.turret?; [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.8, f64::from(p[2]) + 0.5] };
            let seen = crate::game::soulscraft_cover::sees(game, muzzle, geom::add(pos, [0.0, 1.8, 0.0]));
            if attack {
                if !seen && moved >= 1.5 && crate::game::soulscraft_cover::in_cover_mode(id) {
                    return Some(Ok(()));
                }
                if t > 200 {
                    return Some(Err(format!("moved {moved:.1}, still seen {seen}")));
                }
            } else {
                if crate::game::soulscraft_cover::in_cover_mode(id) || moved > 1.0 {
                    return Some(Err(format!("it took cover from a support turret (moved {moved:.1})")));
                }
                if t >= 100 {
                    return Some(Ok(()));
                }
            }
        }
        _ => {}
    }
    None
}

/// Round 14 (1): a turret's kill in the owner's hunter's book. The owner (a still golem standing in
/// for the test UUID) is 3 blocks from the turret; a husk of 1 health dies to it. RustCraft 0.10.0
/// reports it by `soulscraft-kill` (and by `pct-machine kill` while Path-of-craft-tree is in the
/// build): the book credits it once, to the zombie row.
fn kill_book(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let owner = u128::from_str_radix(OWNER, 16).ok()?;
    match t {
        2 => {
            crate::game::soulscraft_book::clear_stand_ins();
            let Some(g) = crate::game::soulscraft_selftest_altar::golem(game, [SPOT[0] + 5, SPOT[1], SPOT[2] + 3]) else {
                return Some(Err("no stand-in".into()));
            };
            crate::game::soulscraft_book::add_stand_in(owner, "turret_owner", g);
            c.health = f64::from(crate::game::soulscraft_book::reports_heard().1);
            c.mob = summon(game, "minecraft:husk", SPOT, "NoAI:1b,Health:1f,");
            c.turret = turret(game, [SPOT[0] + 5, SPOT[1], SPOT[2]], "slow");
            if c.mob.is_none() || c.turret.is_none() {
                return Some(Err("no husk or no turret".into()));
            }
            None
        }
        _ if t > 2 => {
            if mob(game, c.mob?).is_some_and(|m| m.alive()) {
                return None;
            }
            c.phase += 1;
            // The reports come after the death, within a few ticks.
            if c.phase < 20 {
                return None;
            }
            let (pct, own) = crate::game::soulscraft_book::reports_heard();
            let kills = crate::game::soulscraft_book::progress_of(owner).map_or(0.0, |p| p.mob("zombie").kills);
            crate::game::soulscraft_book::clear_stand_ins();
            let new_own = f64::from(own) - c.health;
            Some(if kills == 1.0 && new_own >= 1.0 {
                Ok(())
            } else {
                Err(format!("book kills {kills} (wanted 1), soulscraft-kill reports {new_own}, pct-machine reports {pct}"))
            })
        }
        _ => None,
    }
}
