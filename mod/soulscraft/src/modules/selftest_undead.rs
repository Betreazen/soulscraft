// Undead and living SoulsCraft types (round 11, data/mobs.json `undead`) against poison, instant
// harm and a thrown harm potion. A stalker (living: soulscraft_goals::fit clears the zombie
// template's `flags.undead`) and a revenant (undead) stand still side by side on the altar site:
//   undead_poison        poison hurts the stalker, the revenant never gets it (`flags.undead`)
//   undead_instant_harm  `/effect` instant damage hurts the stalker, heals the revenant (the flag)
//   undead_splash_harm   a splash potion of harming between them: the same, through the content
//                        mod's `#minecraft:inverted_healing_and_harm` (potions.rs reads the tag)
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_selftest_altar::{at, centre, clear_site};
use std::sync::{Mutex, PoisonError};

const DIM: &str = "minecraft:overworld";
/// The two mobs, 2 blocks apart, 12 blocks south of the altar site's core.
const LIVING: [i32; 3] = [-5, 0, 12];
const UNDEAD: [i32; 3] = [-3, 0, 12];
/// Ticks after the summon before anything is applied: the server adopts the mobs first (P, health).
const SETTLE: u64 = 10;

pub(super) const CASES: &[(&str, u64)] = &[("undead_poison", 120), ("undead_instant_harm", 60), ("undead_splash_harm", 100)];

#[derive(Default)]
struct Case {
    mark: u64,
    living: Option<u32>,
    undead: Option<u32>,
    /// Health of each when the effect was applied.
    before: (f32, f32),
    poisoned: bool,
}

static CASE: Mutex<Option<Case>> = Mutex::new(None);

fn summon(game: &mut Game<'_>, kind: &str, c: [i32; 3], tag: &str) -> Option<u32> {
    let snbt = format!("{{Tags:[\"soulscraft.p=0.00\",\"soulscraft.fresh\",\"{tag}\"],NoAI:1b,PersistenceRequired:1b}}");
    let Ok((horizon_core::nbt::Tag::Compound(nbt), _)) = horizon_core::nbt::parse_snbt(&snbt) else { return None };
    game.sc_summon(DIM, kind, centre(at(c)), nbt)
}

fn cmd(game: &mut Game<'_>, line: &str) -> bool {
    let ok = game.sc_run_command(line);
    if !ok {
        log::warn!("soulscraft: selftest command refused: {line}");
    }
    ok
}

fn health(game: &Game<'_>, id: u32) -> Option<f32> {
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| m.health)
}

fn has_effect(game: &Game<'_>, id: u32, effect: &str) -> bool {
    game.mobs.iter().find(|m| m.id == id).is_some_and(|m| m.extra.effects.iter().any(|e| e.id == effect))
}

/// Both at half health, so a heal shows.
fn halve(game: &mut Game<'_>, ids: [u32; 2]) {
    for m in game.mobs.iter_mut().filter(|m| ids.contains(&m.id)) {
        m.health = m.attrs.max_health / 2.0;
    }
}

pub(super) fn cleanup(game: &mut Game<'_>) {
    if let Some(c) = CASE.lock().unwrap_or_else(PoisonError::into_inner).take() {
        for id in [c.living, c.undead].into_iter().flatten() {
            game.sc_remove_mob(DIM, id);
        }
    }
    clear_site(game);
}

pub(super) fn step(game: &mut Game<'_>, now: u64, name: &str, first: bool) -> Option<Result<(), String>> {
    let mut guard = CASE.lock().unwrap_or_else(PoisonError::into_inner);
    if first {
        *guard = Some(Case { mark: now, ..Case::default() });
        drop(guard);
        clear_site(game);
        return None;
    }
    let c = guard.as_mut()?;
    let t = now - c.mark;
    if t == 2 {
        c.living = summon(game, "soulscraft:stalker", LIVING, "soulscraft.u_live");
        c.undead = summon(game, "soulscraft:revenant", UNDEAD, "soulscraft.u_dead");
        return (c.living.is_none() || c.undead.is_none()).then(|| Err("summon refused".into()));
    }
    if t < SETTLE {
        return None;
    }
    let (l, u) = (c.living?, c.undead?);
    let applied = t == SETTLE;
    if applied {
        if name != "undead_poison" {
            halve(game, [l, u]);
        }
        c.before = (health(game, l)?, health(game, u)?);
        let ok = match name {
            "undead_poison" => {
                // The engine refuses poison on the undead: that command may well report failure.
                game.sc_run_command("effect give @e[tag=soulscraft.u_dead] minecraft:poison 3 0");
                cmd(game, "effect give @e[tag=soulscraft.u_live] minecraft:poison 3 0")
            }
            "undead_instant_harm" => cmd(game, "effect give @e[tag=soulscraft.u_live] minecraft:instant_damage 1 0")
                && cmd(game, "effect give @e[tag=soulscraft.u_dead] minecraft:instant_damage 1 0"),
            "undead_splash_harm" => {
                let p = centre(at([-4, 3, 12]));
                cmd(game, &format!(
                    "summon minecraft:splash_potion {} {} {} {{Item:{{id:\"minecraft:splash_potion\",count:1,components:{{\"minecraft:potion_contents\":{{potion:\"minecraft:harming\"}}}}}}}}",
                    p[0], p[1], p[2]
                ))
            }
            other => return Some(Err(format!("no scenario {other}"))),
        };
        return (!ok).then(|| Err("command refused".into()));
    }
    let (hl, hu) = (health(game, l)?, health(game, u)?);
    if name == "undead_poison" {
        c.poisoned |= has_effect(game, l, "minecraft:poison");
        if has_effect(game, u, "minecraft:poison") {
            return Some(Err("the revenant is poisoned".into()));
        }
        if t < SETTLE + 70 {
            return None;
        }
        return Some(if c.poisoned && hl < c.before.0 && hu >= c.before.1 {
            Ok(())
        } else {
            Err(format!("stalker poisoned {} {:.1} -> {hl:.1}, revenant {:.1} -> {hu:.1}", c.poisoned, c.before.0, c.before.1))
        });
    }
    // Instant: hurt the living, heal the undead. Settled once both moved, or failed at the timeout.
    if hl < c.before.0 && hu > c.before.1 {
        log::info!("soulscraft: selftest {name}: stalker {:.1} -> {hl:.1}, revenant {:.1} -> {hu:.1}", c.before.0, c.before.1);
        return Some(Ok(()));
    }
    if hu < c.before.1 {
        return Some(Err(format!("the revenant was hurt: {:.1} -> {hu:.1}", c.before.1)));
    }
    if t > SETTLE + 40 {
        return Some(Err(format!("stalker {:.1} -> {hl:.1}, revenant {:.1} -> {hu:.1}", c.before.0, c.before.1)));
    }
    None
}
