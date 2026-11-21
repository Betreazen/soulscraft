// The joint bench's self-test (horizon-joint-dev/README.md, «Быстрые проверки»): the console's
// `soulscraft_selftest run [filter]` queues the scenarios; each runs in the overworld's ticks on a
// stone platform at y 100 in SoulsCraft's area (x −1000..0, z 0..1000), held loaded with
// /forceload, against a still iron golem (NoAI) as the target. The result goes to
// selftest/soulscraft.json in the server's folder, then the area is cleared.
//
// Every scenario reads what the server half recorded (soulscraft_server's phase, mode and strike
// records), so it checks the same code the game runs, at the ticks of data/mobs.json.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_server as server;
use soulscraft_rules::{
    attack::{Phase, Timeline},
    data::MobsFile,
};
use std::sync::{Mutex, PoisonError};

const BASE: [f64; 3] = [-599.5, 101.0, 600.5];
const FAR: [f64; 3] = [-899.5, 101.0, 900.5];
/// The golem stands this far south of the mob, which faces south.
const GAP: f64 = 1.5;
const RANGED_GAP: f64 = 11.0;
const OUT: &str = "selftest/soulscraft.json";

#[derive(Clone, Debug)]
enum Kind {
    Spawn,
    PScale,
    Timing { mob: String, attack: String },
    Miss,
    Backoff,
    ShieldWait,
    Reload,
    Survive,
    Bleed,
    Waves,
    /// A dead shrike falls (its no-gravity taken off in the death hook).
    ShrikeFalls,
    /// A size-4 bog slime splits into 2–3 of size 2.
    SlimeSplits,
    /// The lurker's grab: `holds` its target at its mouth, `breaks` on 3 blows of it, `keeps` the
    /// one it grabbed when another is nearer.
    Grab(&'static str),
    /// The mob's box against data/mobs.json `hitbox` (a slime of this size).
    Hitbox { mob: String, size: u8 },
    /// The altar's and the event's scenarios (soulscraft_selftest_altar).
    Altar(&'static str),
    /// The joint scenarios with RustCraft's turrets (soulscraft_selftest_turret), only when the
    /// filter names them.
    Turret(&'static str),
    /// Undead and living types against effects and potions (soulscraft_selftest_undead).
    Undead(&'static str),
    /// The vanilla changes (soulscraft_selftest_vanilla); `vslow_` ones only when the filter
    /// names them.
    Vanilla(String),
    /// The revenant's, the stalker's, the marksman's and the brute's fight ([`DUEL_CASES`]).
    Duel(&'static str),
}

#[derive(Clone, Debug)]
struct Scenario {
    name: String,
    kind: Kind,
    timeout: u64,
}

#[derive(Default)]
struct Live {
    step: u32,
    start: u64,
    mob: Option<u32>,
    golem: Option<u32>,
    golem_health: f64,
    mark: u64,
    note: String,
    saved: Option<Option<String>>,
    /// A falling body's height at the start and the lowest seen since (shrike_falls_on_death).
    y0: f64,
    low: f64,
    /// A second golem (lurker_grab_keeps_its_prey).
    other: Option<u32>,
}

struct Run {
    queue: Vec<Scenario>,
    current: Option<(Scenario, Live)>,
    passed: Vec<String>,
    failed: Vec<(String, String)>,
    prepared: bool,
    peaceful: bool,
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);

fn scenarios(data: &MobsFile, filter: Option<&str>) -> Vec<Scenario> {
    let mut all = vec![
        Scenario { name: "spawn_revenant".into(), kind: Kind::Spawn, timeout: 20 },
        Scenario { name: "p_scaling".into(), kind: Kind::PScale, timeout: 20 },
    ];
    for m in &data.mobs {
        for (_, a) in m.phased_attacks() {
            all.push(Scenario {
                name: format!("timing_{}_{}", m.id, a.id),
                kind: Kind::Timing { mob: m.id.clone(), attack: a.id.clone() },
                timeout: u64::from(Timeline::of(a).end()) + 60,
            });
        }
    }
    for m in &data.mobs {
        let size = m.full_size();
        all.push(Scenario { name: format!("hitbox_{}", m.id), kind: Kind::Hitbox { mob: m.id.clone(), size }, timeout: 20 });
        if size > 1 {
            let half = size / 2;
            all.push(Scenario { name: format!("hitbox_{}_size{half}", m.id), kind: Kind::Hitbox { mob: m.id.clone(), size: half }, timeout: 20 });
        }
    }
    all.extend([
        Scenario { name: "miss_when_away".into(), kind: Kind::Miss, timeout: 80 },
        Scenario { name: "backoff_after_series".into(), kind: Kind::Backoff, timeout: 300 },
        Scenario { name: "shield_wait".into(), kind: Kind::ShieldWait, timeout: 200 },
        Scenario { name: "reload_numbers".into(), kind: Kind::Reload, timeout: 80 },
        Scenario { name: "survive_unload".into(), kind: Kind::Survive, timeout: 600 },
        Scenario { name: "bleeding_ticks".into(), kind: Kind::Bleed, timeout: 120 },
        Scenario { name: "wave_stub".into(), kind: Kind::Waves, timeout: 60 },
        Scenario { name: "shrike_falls_on_death".into(), kind: Kind::ShrikeFalls, timeout: 80 },
        Scenario { name: "bog_slime_splits".into(), kind: Kind::SlimeSplits, timeout: 80 },
        Scenario { name: "lurker_grab_holds".into(), kind: Kind::Grab("holds"), timeout: 120 },
        Scenario { name: "lurker_grab_breaks".into(), kind: Kind::Grab("breaks"), timeout: 120 },
        Scenario { name: "lurker_grab_keeps_its_prey".into(), kind: Kind::Grab("keeps"), timeout: 120 },
    ]);
    for (name, timeout) in DUEL_CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Duel(*name), timeout: *timeout });
    }
    for (name, timeout) in crate::game::soulscraft_selftest_altar::CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Altar(*name), timeout: *timeout });
    }
    for (name, timeout) in crate::game::soulscraft_selftest_trophy::CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Altar(*name), timeout: *timeout });
    }    // The hunter's book, on the altar's site too (soulscraft_selftest_book, through the trophy step).
    for (name, timeout) in crate::game::soulscraft_selftest_book::CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Altar(*name), timeout: *timeout });
    }
    for (name, timeout) in crate::game::soulscraft_selftest_undead::CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Undead(*name), timeout: *timeout });
    }
    for (name, timeout) in crate::game::soulscraft_selftest_vanilla::cases(data) {
        all.push(Scenario { name: name.clone(), kind: Kind::Vanilla(name), timeout });
    }
    for (name, timeout) in crate::game::soulscraft_selftest_turret::CASES {
        all.push(Scenario { name: (*name).to_owned(), kind: Kind::Turret(*name), timeout: *timeout });
    }
    all.retain(|s| filter.is_none_or(|f| s.name.contains(f)));
    // The turret group needs RustCraft and takes a while: only when asked for (`run turret`).
    if !filter.is_some_and(|f| f.contains("turret")) {
        all.retain(|s| !matches!(s.kind, Kind::Turret(_)));
    }
    // The slow vanilla group (real arrows, walks round a wall): only when asked for (`run vslow`).
    if !filter.is_some_and(|f| f.contains("vslow")) {
        all.retain(|s| !s.name.starts_with("vslow_"));
    }
    all
}

pub(super) fn start(filter: Option<String>) {
    let list = scenarios(&server::state().data, filter.as_deref());
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = Some(Run {
        queue: list.into_iter().rev().collect(),
        current: None,
        passed: Vec::new(),
        failed: Vec::new(),
        prepared: false,
        peaceful: false,
    });
    let _ = std::fs::create_dir_all("selftest");
    let _ = std::fs::write(OUT, r#"{"mod":"soulscraft","done":false,"passed":[],"failed":[]}"#);
}

fn run_cmd(game: &mut Game<'_>, line: &str) {
    if !game.sc_run_command(line) {
        log::warn!("soulscraft: selftest command refused: {line}");
    }
}

fn prepare(game: &mut Game<'_>) -> bool {
    crate::game::soulscraft_selftest_altar::forceload(game, true);
    let peaceful = game.ui.difficulty == 0;
    if peaceful {
        // Monsters vanish on peaceful; this world is the check's own (joint.ps1 check makes it fresh).
        run_cmd(game, "difficulty normal");
    }
    for c in [BASE, FAR] {
        let (x, z) = (c[0] as i32, c[2] as i32);
        run_cmd(game, &format!("forceload add {} {} {} {}", x - 12, z - 12, x + 12, z + 12));
        run_cmd(game, &format!("fill {} 100 {} {} 100 {} stone", x - 10, z - 10, x + 10, z + 10));
        // Refused when there is nothing to clear (vanilla's fill fails on no change): not logged.
        game.sc_run_command(&format!("fill {} 101 {} {} 106 {} air", x - 10, z - 10, x + 10, z + 10));
    }
    peaceful
}

fn clean_area(game: &mut Game<'_>, peaceful: bool) {
    crate::game::soulscraft_selftest_altar::forceload(game, false);
    for c in [BASE, FAR] {
        let (x, z) = (c[0] as i32, c[2] as i32);
        // Leftovers only (each scenario removes its own mobs): no target is no failure.
        game.sc_run_command(&format!("kill @e[x={},y=95,z={},dx=24,dy=16,dz=24,type=!player]", x - 12, z - 12));
        run_cmd(game, &format!("fill {} 100 {} {} 106 {} air", x - 10, z - 10, x + 10, z + 10));
        run_cmd(game, &format!("forceload remove {} {} {} {}", x - 12, z - 12, x + 12, z + 12));
    }
    if peaceful {
        run_cmd(game, "difficulty peaceful");
    }
}

pub(super) fn on_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    if dim != "minecraft:overworld" {
        return;
    }
    let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(run) = guard.as_mut() else { return };
    if !run.prepared {
        run.peaceful = prepare(game);
        run.prepared = true;
        return;
    }
    if run.current.is_none() {
        match run.queue.pop() {
            Some(s) => {
                log::info!("soulscraft: selftest start {} tick={now}", s.name);
                run.current = Some((s, Live { start: now, ..Live::default() }));
            }
            None => {
                let done = guard.take().unwrap();
                drop(guard);
                finish(game, done);
                return;
            }
        }
    }
    let (sc, live) = run.current.as_mut().unwrap();
    let verdict = if now - live.start > sc.timeout {
        Some(Err(format!("timed out after {} ticks{}", sc.timeout, if live.note.is_empty() { String::new() } else { format!(" ({})", live.note) })))
    } else {
        step(game, dim, now, sc, live)
    };
    if let Some(v) = verdict {
        let (sc, live) = run.current.take().unwrap();
        for id in [live.mob, live.golem, live.other].into_iter().flatten() {
            game.sc_remove_mob(dim, id);
        }
        if let Some(saved) = live.saved {
            restore_reload_file(saved);
        }
        if matches!(sc.kind, Kind::Altar(_)) {
            crate::game::soulscraft_selftest_altar::cleanup(game, now);
        }
        if matches!(sc.kind, Kind::Turret(_)) {
            crate::game::soulscraft_selftest_turret::cleanup(game);
        }
        if matches!(sc.kind, Kind::Undead(_)) {
            crate::game::soulscraft_selftest_undead::cleanup(game);
        }
        if matches!(sc.kind, Kind::Vanilla(_)) {
            crate::game::soulscraft_selftest_vanilla::cleanup(game);
        }
        if matches!(sc.kind, Kind::Duel(_)) {
            duel_cleanup(game);
        }
        match v {
            Ok(()) => {
                log::info!("soulscraft: selftest PASS {}", sc.name);
                run.passed.push(sc.name);
            }
            Err(why) => {
                log::warn!("soulscraft: selftest FAIL {}: {why}", sc.name);
                run.failed.push((sc.name, why));
            }
        }
    }
}

fn finish(game: &mut Game<'_>, run: Run) {
    clean_area(game, run.peaceful);
    let json = serde_json::json!({
        "mod": "soulscraft",
        "done": true,
        "passed": run.passed,
        "failed": run.failed.iter().map(|(n, w)| serde_json::json!({"name": n, "why": w})).collect::<Vec<_>>(),
    });
    let _ = std::fs::create_dir_all("selftest");
    if let Err(e) = std::fs::write(OUT, json.to_string()) {
        log::warn!("soulscraft: {OUT} not written: {e}");
    }
    log::info!("soulscraft: selftest done passed={} failed={}", run.passed.len(), run.failed.len());
}

/// The iron golem's box is this wide (vanilla).
const GOLEM_WIDTH: f64 = 1.4;

/// Spawns a mob of ours facing south at `at` and a still iron golem `GAP` south of it (farther for
/// a mob whose box would touch the golem's: the brute's).
fn arrange(game: &mut Game<'_>, dim: &str, mob: &str, at: [f64; 3], p: f64, live: &mut Live) -> Result<(), String> {
    let (key, width) = server::state().data.mob(mob).map(|m| (m.key.clone(), m.hitbox_at(m.full_size()).width)).ok_or(format!("no mob {mob}"))?;
    let size = if mob == "bog_slime" { ",\"soulscraft.size=4\"" } else { "" };
    let snbt = format!("{{Rotation:[0f,0f],Tags:[\"soulscraft.p={p:.2}\",\"{}\"{size}],PersistenceRequired:1b}}", server::TAG_FRESH);
    let id = summon(game, dim, &key, at, &snbt).ok_or(format!("{key} not spawned"))?;
    live.mob = Some(id);
    // A bolt flies: its target stands in the marksman's kept distance (live.note names the attack).
    let gap = if live.note == "aimed_shot" { RANGED_GAP } else { GAP.max((width + GOLEM_WIDTH) / 2.0 + 0.3) };
    let golem_at = [at[0], at[1], at[2] + gap];
    let g = summon(game, dim, "minecraft:iron_golem", golem_at, "{NoAI:1b,PersistenceRequired:1b}").ok_or("no golem")?;
    live.golem = Some(g);
    Ok(())
}

fn summon(game: &mut Game<'_>, dim: &str, kind: &str, at: [f64; 3], snbt: &str) -> Option<u32> {
    let Ok((horizon_core::nbt::Tag::Compound(nbt), _)) = horizon_core::nbt::parse_snbt(snbt) else { return None };
    game.sc_summon(dim, kind, at, nbt)
}

fn health(game: &Game<'_>, id: u32) -> Option<f64> {
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| f64::from(m.health))
}

/// Points the tracked mob at the golem, once it is tracked.
fn aim(live: &Live, shield: bool) -> bool {
    let (Some(mob), Some(golem)) = (live.mob, live.golem) else { return false };
    let mut s = server::state();
    let Some(t) = s.mobs.get_mut(&mob) else { return false };
    t.target_override = Some(golem);
    t.test_shield = shield;
    true
}

fn force(live: &Live, attack: &str) -> Result<(), String> {
    let mob = live.mob.ok_or("no mob")?;
    let mut s = server::state();
    let server::State { mobs, data, .. } = &mut *s;
    let t = mobs.get_mut(&mob).ok_or("not tracked")?;
    let idx = data.mobs[t.kind].attack_index(attack).ok_or(format!("no attack {attack}"))?;
    t.brain.force(idx);
    Ok(())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

fn step(game: &mut Game<'_>, dim: &str, now: u64, sc: &Scenario, live: &mut Live) -> Option<Result<(), String>> {
    let first = live.step == 0;
    live.step += 1;
    match &sc.kind {
        Kind::Spawn | Kind::PScale => {
            let p = if matches!(sc.kind, Kind::PScale) { 1.0 } else { 0.0 };
            if first {
                return arrange(game, dim, "revenant", BASE, p, live).err().map(Err);
            }
            let mob = live.mob?;
            if !server::state().mobs.contains_key(&mob) {
                return None;
            }
            let m = game.mobs.iter().find(|m| m.id == mob)?;
            let armor = f64::from(horizon_client::mobs::armor::values(m).0);
            let (hp, max, dmg) = (f64::from(m.health), f64::from(m.attrs.max_health), f64::from(m.attrs.attack_damage));
            let want = if p > 0.0 { (57.0, 12.0, 6.4) } else { (30.0, 6.0, 4.0) };
            let kind_ok = m.kind.id == "soulscraft:revenant";
            // Path-of-craft-tree's experience tag (Г3): exactly one, ×1.00 at P 0 and ×1.45 at P 1.
            let tags = game.sc_entity(mob).and_then(|e| game.sc_entity_tags(e)).unwrap_or_default();
            let xp: Vec<&String> = tags.iter().filter(|t| t.starts_with("pct.xp")).collect();
            let want_xp = if p > 0.0 { "pct.xp145" } else { "pct.xp100" };
            if xp.len() != 1 || xp[0] != want_xp {
                return Some(Err(format!("experience tags {xp:?}, wanted [{want_xp}]")));
            }
            Some(if kind_ok && close(max, want.0) && close(hp, want.0) && close(armor, want.1) && close(dmg, want.2) {
                Ok(())
            } else {
                Err(format!("kind {} health {hp}/{max} armor {armor} damage {dmg}, wanted {want:?}", m.kind.id))
            })
        }
        Kind::Timing { mob, attack } => timing(game, dim, now, live, mob, attack, first),
        Kind::Miss => {
            if first {
                return arrange(game, dim, "revenant", BASE, 0.0, live).err().map(Err);
            }
            if live.step == 2 {
                if !aim(live, false) {
                    live.step = 1;
                    return None;
                }
                live.golem_health = health(game, live.golem?)?;
                return force(live, "overhead").err().map(Err).or_else(|| {
                    live.mark = now;
                    None
                });
            }
            if now == live.mark + 12 {
                let to = [BASE[0], BASE[1], BASE[2] + 6.0];
                game.sc_teleport_mob(dim, live.golem?, to);
            }
            let rec = server::state().strikes.iter().rev().find(|r| r.mob == live.mob.unwrap_or(0)).cloned();
            let rec = rec?;
            let after = health(game, live.golem?)?;
            Some(if !rec.hit && close(after, live.golem_health) && rec.t == 18 {
                Ok(())
            } else {
                Err(format!("strike t={} hit={} golem {} -> {after}", rec.t, rec.hit, live.golem_health))
            })
        }
        Kind::Backoff => {
            if first {
                return arrange(game, dim, "revenant", BASE, 0.0, live).err().map(Err);
            }
            if !aim(live, false) {
                return None;
            }
            let mob = live.mob?;
            let s = server::state();
            let ends: Vec<u64> = s
                .phases
                .iter()
                .filter(|r| r.mob == mob && r.phase == Phase::Done)
                .map(|r| r.tick)
                .collect();
            let back = s.modes.iter().find(|(id, m, _)| *id == mob && m == "backoff").map(|(_, _, t)| *t)?;
            Some(if ends.len() >= 2 && ends[1] <= back {
                Ok(())
            } else {
                Err(format!("backed off at {back} after {} attacks", ends.iter().filter(|t| **t <= back).count()))
            })
        }
        Kind::ShieldWait => {
            if first {
                return arrange(game, dim, "revenant", BASE, 0.0, live).err().map(Err);
            }
            if live.mark == 0 {
                if !aim(live, true) {
                    return None;
                }
                live.mark = now;
                return None;
            }
            let mob = live.mob?;
            let s = server::state();
            let first_attack = s.phases.iter().find(|r| r.mob == mob && r.phase == Phase::Windup).cloned()?;
            let waited = first_attack.tick - live.mark;
            let wait = s.data.mob("revenant").and_then(|m| m.param("circle_wait_max_ticks")).unwrap_or(60.0) as u64;
            let circled = s.modes.iter().any(|(id, m, _)| *id == mob && m == "circle");
            Some(if first_attack.attack == "shield_bash" && circled && waited >= wait && waited <= wait + 25 {
                Ok(())
            } else {
                Err(format!("first attack {} after {waited} ticks, circled {circled}", first_attack.attack))
            })
        }
        Kind::Reload => reload_scenario(game, dim, now, live, first),
        Kind::Survive => survive(game, dim, now, live),
        Kind::Bleed => bleed(game, dim, now, live, first),
        Kind::Waves => waves(game, dim, now, live),
        Kind::ShrikeFalls => shrike_falls(game, dim, now, live, first),
        Kind::SlimeSplits => slime_splits(game, dim, now, live, first),
        Kind::Grab(case) => grab(game, dim, now, live, case, first),
        Kind::Hitbox { mob, size } => hitbox(game, dim, live, mob, *size, first),
        Kind::Altar(name) => crate::game::soulscraft_selftest_altar::step(game, now, name, first),
        Kind::Turret(name) => crate::game::soulscraft_selftest_turret::step(game, now, name, first),
        Kind::Undead(name) => crate::game::soulscraft_selftest_undead::step(game, now, name, first),
        Kind::Vanilla(name) => crate::game::soulscraft_selftest_vanilla::step(game, now, name, first),
        Kind::Duel(name) => duel(game, dim, now, live, name, first),
    }
}

/// A mob's box once the server has it: its width, height and eyes from data/mobs.json (at a
/// slime's size), as the engine reports them (`native_entity_bounding_box`).
fn hitbox(game: &mut Game<'_>, dim: &str, live: &mut Live, mob: &str, size: u8, first: bool) -> Option<Result<(), String>> {
    if first {
        let p = 0.0;
        let id = crate::game::soulscraft_tools::summon(game, dim, mob, BASE, p, size);
        live.mob = id;
        return id.is_none().then(|| Err(format!("{mob} not spawned")));
    }
    let id = live.mob?;
    if !server::state().mobs.contains_key(&id) {
        return None;
    }
    let want = server::state().data.mob(mob)?.hitbox_at(size);
    let e = game.sc_entity(id)?;
    let [lo, hi] = game.sc_entity_bounding_box(e)?;
    let eye = game.mobs.iter().find(|m| m.id == id).map(|m| m.eye_height())?;
    let (w, d, h) = (hi[0] - lo[0], hi[2] - lo[2], hi[1] - lo[1]);
    log::info!("soulscraft: selftest hitbox mob={mob} size={size} box={w:.3}x{h:.3}x{d:.3} eye={eye:.3} want={want:?}");
    if !(close(w, want.width) && close(d, want.width) && close(h, want.height) && close(eye, want.eye)) {
        return Some(Err(format!("box {w:.3} × {h:.3} × {d:.3}, eyes {eye:.3}; data {want:?}")));
    }
    // PCT's experience tag at P 0 carries the size's share (scale::size_share): the full slime
    // and every other mob pct.xp100, a size-2 slime pct.xp25.
    let full = server::state().data.mob(mob)?.full_size();
    let want_xp = format!("{}{}", soulscraft_rules::scale::XP_TAG_PREFIX, (100.0 * soulscraft_rules::scale::size_share(size, full)).round());
    let tags = game.sc_entity_tags(e).unwrap_or_default();
    let xp: Vec<&String> = tags.iter().filter(|t| t.starts_with(soulscraft_rules::scale::XP_TAG_PREFIX)).collect();
    Some(if xp.len() == 1 && *xp[0] == want_xp { Ok(()) } else { Err(format!("experience tags {xp:?}, wanted [{want_xp}]")) })
}

/// One attack forced on the golem: its phases on the data's ticks, and its blow on its tick with
/// the scaled damage (melee blows on the ground; dashes, dives, jumps and bolts by their own rule).
fn timing(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, mob: &str, attack: &str, first: bool) -> Option<Result<(), String>> {
    if first {
        live.note = attack.to_owned();
        return arrange(game, dim, mob, BASE, 0.0, live).err().map(Err);
    }
    if live.mark == 0 {
        if !aim(live, false) {
            return None;
        }
        live.golem_health = health(game, live.golem?)?;
        if let Err(e) = force(live, attack) {
            return Some(Err(e));
        }
        live.mark = now;
        return None;
    }
    let id = live.mob?;
    let s = server::state();
    let def = s.data.mob(mob)?.clone();
    let a = def.attack(attack)?.clone();
    let recs: Vec<_> = s.phases.iter().filter(|r| r.mob == id && r.attack == attack && r.tick > live.mark).cloned().collect();
    let done = recs.iter().find(|r| r.phase == Phase::Done)?;
    let line = Timeline::of(&a);
    let t0 = recs.iter().find(|r| r.phase == Phase::Windup).map_or(0, |r| r.tick);
    let strikes: Vec<_> = s.strikes.iter().filter(|r| r.mob == id && r.attack == attack && r.tick >= t0).cloned().collect();
    let first_blow = strikes.first().map(|r| (r.tick - t0) as u32);
    // A jump's window is the `recover` ticks after it lands (its blow); an ambush that bit ends on
    // its blow, as the grab follows (soulscraft_rules::brain).
    let grabbed = def.attacks.iter().any(|g| g.follows.as_deref() == Some(attack)) && first_blow.is_some_and(|b| b < line.recover_start());
    let (recover_at, end_at) = match first_blow {
        Some(land) if mob == "bog_slime" => (Some(land), land + line.recover),
        Some(bit) if grabbed => (None, bit),
        _ => (Some(line.recover_start()), line.end()),
    };
    let mut problems = Vec::new();
    let mut want = vec![(Phase::Windup, 0)];
    if a.hold > 0 {
        want.push((Phase::Hold, line.windup));
    }
    want.push((Phase::Hit, line.hit_start()));
    want.extend(recover_at.map(|t| (Phase::Recover, t)));
    for (phase, t) in want {
        match recs.iter().find(|r| r.phase == phase) {
            Some(r) if r.tick - t0 == u64::from(t) => {}
            Some(r) => problems.push(format!("{} at {} not {t}", phase.label(), r.tick - t0)),
            None => problems.push(format!("no {}", phase.label())),
        }
    }
    let end = (done.tick - t0) as u32;
    let open_charge = a.recover_if.as_deref() == Some("wall") && end == line.recover_start() + soulscraft_rules::brain::CHARGE_OPEN_RECOVER;
    if end != end_at && !open_charge {
        problems.push(format!("end at {end} not {end_at}"));
    }
    drop(s);
    let melee = a.dash_blocks.is_none() && a.projectile.is_none() && !matches!(mob, "shrike" | "bog_slime");
    let after = health(game, live.golem?).unwrap_or(0.0);
    let lost = live.golem_health - after;
    let blow = server::scaled(&def, 0.0, if mob == "bog_slime" { 4 } else { 0 }).blow(a.damage);
    if melee {
        let first_tick = t0 + u64::from(line.hit_start());
        if !strikes.iter().any(|r| r.tick == first_tick && r.hit) {
            problems.push(format!("no blow landed on tick {}", line.hit_start()));
        }
        if !close(lost, blow * strikes.len() as f64) {
            problems.push(format!("golem lost {lost:.2}, {} blows of {blow:.2}", strikes.len()));
        }
    } else if strikes.is_empty() {
        problems.push("no blow at all".into());
    } else if lost <= 0.0 {
        problems.push(format!("the {} never hurt the golem", if a.projectile.is_some() { "bolt" } else { "blow" }));
    }
    log::info!(
        "soulscraft: selftest timing mob={mob} attack={attack} end={end} strikes={} lost={lost:.2} blow={blow:.2}",
        strikes.len()
    );
    Some(if problems.is_empty() { Ok(()) } else { Err(problems.join("; ")) })
}

fn reload_path() -> Option<std::path::PathBuf> {
    Game::sc_config_dir().ok().map(|d| d.join(server::RELOAD_FILE))
}

fn restore_reload_file(saved: Option<String>) {
    let Some(path) = reload_path() else { return };
    match saved {
        Some(text) => {
            let _ = std::fs::write(&path, text);
            let _ = server::reload();
        }
        None => {
            let _ = std::fs::remove_file(&path);
            server::reset_data();
        }
    }
}

/// `/sc reload` with the revenant's overhead windup raised by 4: the next overhead lands 4 ticks
/// later, without a rebuild. The config file is put back after.
fn reload_scenario(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        let path = reload_path()?;
        live.saved = Some(std::fs::read_to_string(&path).ok());
        let mut v: serde_json::Value = serde_json::from_str(soulscraft_rules::data::EMBEDDED).ok()?;
        let attacks = v["mobs"].as_array_mut()?.iter_mut().find(|m| m["id"] == "revenant")?["attacks"].as_array_mut()?;
        let o = attacks.iter_mut().find(|a| a["id"] == "overhead")?;
        o["windup"] = serde_json::json!(14);
        let _ = std::fs::create_dir_all(path.parent()?);
        if let Err(e) = std::fs::write(&path, v.to_string()) {
            return Some(Err(format!("{}: {e}", path.display())));
        }
        if let Err(e) = server::reload() {
            return Some(Err(e));
        }
        return arrange(game, dim, "revenant", BASE, 0.0, live).err().map(Err);
    }
    if live.mark == 0 {
        if !aim(live, false) {
            return None;
        }
        if let Err(e) = force(live, "overhead") {
            return Some(Err(e));
        }
        live.mark = now;
        return None;
    }
    let id = live.mob?;
    let s = server::state();
    let recs: Vec<_> = s.phases.iter().filter(|r| r.mob == id && r.tick > live.mark).cloned().collect();
    let t0 = recs.iter().find(|r| r.phase == Phase::Windup)?.tick;
    let hit = recs.iter().find(|r| r.phase == Phase::Hit)?.tick;
    Some(if hit - t0 == 22 { Ok(()) } else { Err(format!("hit at {} after the reload, wanted 22", hit - t0)) })
}

/// П3: a revenant at P 0.6 with 40 health (above its type's 30, below its scaled 46.2), its chunk let go and loaded again. Its P tag
/// should come back with it and its stats be set again.
fn survive(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live) -> Option<Result<(), String>> {
    let (x, z) = (FAR[0] as i32, FAR[2] as i32);
    let near = |game: &Game<'_>| {
        game.mobs
            .iter()
            .find(|m| !m.removed && m.alive() && m.kind.id == "soulscraft:revenant" && (m.position.x - FAR[0]).abs() < 3.0 && (m.position.z - FAR[2]).abs() < 3.0)
            .map(|m| (m.id, f64::from(m.health), f64::from(m.attrs.max_health)))
    };
    let range = format!("{} {} {} {}", x - 12, z - 12, x + 12, z + 12);
    match live.note.as_str() {
        "" => {
            let snbt = format!("{{Tags:[\"soulscraft.p=0.60\",\"{}\"],PersistenceRequired:1b}}", server::TAG_FRESH);
            if summon(game, dim, "soulscraft:revenant", FAR, &snbt).is_none() {
                return Some(Err("not spawned".into()));
            }
            live.note = "spawned".into();
            None
        }
        "spawned" => {
            let (id, _, _) = near(&*game)?;
            if !server::state().mobs.contains_key(&id) {
                return None;
            }
            game.sc_set_mob_health(dim, id, 40.0);
            live.note = "hurt".into();
            None
        }
        "hurt" => {
            run_cmd(game, &format!("forceload remove {range}"));
            live.mark = now;
            live.note = "chunk let go".into();
            None
        }
        "chunk let go" => {
            if near(&*game).is_some() {
                return None;
            }
            live.note = format!("unloaded after {} ticks", now - live.mark);
            run_cmd(game, &format!("forceload add {range}"));
            live.mark = now;
            None
        }
        _ => {
            let (id, hp, _) = near(&*game)?;
            if !server::state().mobs.contains_key(&id) {
                return None;
            }
            let max = game.mobs.iter().find(|m| m.id == id).map_or(0.0, |m| f64::from(m.attrs.max_health));
            let back = now - live.mark;
            log::info!("soulscraft: selftest survive {}, back after {back} ticks: health {hp}/{max}", live.note);
            Some(if close(max, 46.2) && close(hp, 40.0) {
                Ok(())
            } else {
                Err(format!("{}, back after {back} ticks with health {hp}/{max} (wanted 40/46.2)", live.note))
            })
        }
    }
}

/// The shared bleed, `soulscraft:bleeding`, on a golem for 3 s: a hit of `effects.bleeding.damage`
/// every `period_ticks` (20), so 3 hits at level I.
fn bleed(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        let at = [BASE[0] + 4.0, BASE[1], BASE[2]];
        let g = summon(game, dim, "minecraft:iron_golem", at, "{NoAI:1b,PersistenceRequired:1b,Tags:[\"soulscraft.bleed\"]}");
        live.golem = g;
        return g.is_none().then(|| Err("no golem".into()));
    }
    if live.mark == 0 {
        live.golem_health = health(game, live.golem?)?;
        run_cmd(game, "effect give @e[tag=soulscraft.bleed,limit=1] soulscraft:bleeding 3 0");
        live.mark = now;
        return None;
    }
    if now < live.mark + 70 {
        return None;
    }
    let lost = live.golem_health - health(game, live.golem?)?;
    let (damage, _) = server::bleed_numbers();
    Some(if close(lost, 3.0 * damage) {
        Ok(())
    } else {
        Err(format!("lost {lost:.2} in 3 s of bleeding, wanted {:.2}", 3.0 * damage))
    })
}

/// `/sc wave`: the test event's first wave (6 zombies) round the platform, cleared, rested,
/// the next one (8), then stopped. The altar's rules (Г12) are not in this stub.
fn waves(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live) -> Option<Result<(), String>> {
    use crate::game::soulscraft_tools::{wave, wave_state};
    use soulscraft_rules::command::WaveAction;
    let want = |n: usize| -> u32 {
        let s = server::state();
        s.data.events.first().and_then(|e| e.waves.get(n - 1)).map_or(0, |w| w.mobs.iter().map(|m| m.count).sum())
    };
    match live.note.as_str() {
        "" => {
            wave(game, dim, now, WaveAction::Start, BASE);
            live.note = "wave 1".into();
            let (n, ids, _) = wave_state()?;
            if n != 1 || ids.len() as u32 != want(1) {
                return Some(Err(format!("wave {n} with {} mobs, wanted 1 with {}", ids.len(), want(1))));
            }
            for id in ids {
                game.sc_kill_mob(dim, id);
            }
            None
        }
        "wave 1" => {
            let (_, _, resting) = wave_state()?;
            if !resting {
                return None;
            }
            wave(game, dim, now, WaveAction::Next, BASE);
            let (n, ids, _) = wave_state()?;
            wave(game, dim, now, WaveAction::Stop, BASE);
            Some(if n == 2 && ids.len() as u32 == want(2) && wave_state().is_none() {
                Ok(())
            } else {
                Err(format!("after the rest: wave {n} with {} mobs, wanted 2 with {}", ids.len(), want(2)))
            })
        }
        _ => None,
    }
}

/// The revenant's, the stalker's and the marksman's fight (plan 2026-11-16, task 1):
///   revenant_shield_bash_knocks  the shield bash (`knockback`) pushes a sheep that cannot walk a
///                                block farther than a thrust does, and does the data's blow with
///                                the shield-breaking axe in hand
///   revenant_shield_front        a blow from in front of its raised shield does at most a fifth of
///                                the same blow from behind (`shield_front_reduction`)
///   marksman_reload_cover        after its shot it reloads behind a 3×3 wall off to its side, out
///                                of the golem's sight (`reload_cover_blocks`)
const DUEL_CASES: &[(&str, u64)] = &[
    ("revenant_shield_bash_knocks", 140),
    ("revenant_shield_front", 60),
    ("marksman_reload_cover", 140),
    ("brute_wave_spares_jumper", 160),
];

/// The blow `revenant_shield_front` deals from each side.
const SHIELD_TEST_BLOW: f32 = 10.0;
/// Ticks after a blow before the push is measured: the sheep lands and stops.
const KNOCK_SETTLE: u64 = 15;
/// How much farther than an ordinary blow's the bash must push. The vanilla push of a hurt is 0.4
/// a tick; the bash adds `knockback_strength` / 2 = 1 on top of half of it, about 3 times as
/// fast, so the bash's push is about 3 times the thrust's (2 blocks or more). One block over the
/// thrust's measured push is well under that, and well over what the hurt's push varies by.
const KNOCK_MARGIN: f64 = 1.0;

/// What a duel scenario keeps between its ticks: where the pushed sheep stood, how far the
/// ordinary blow pushed it, the front blow's loss.
#[derive(Default)]
struct Duel {
    from: Option<[f64; 3]>,
    plain: f64,
    front: f32,
}

static DUEL: Mutex<Option<Duel>> = Mutex::new(None);

fn duel(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, name: &str, first: bool) -> Option<Result<(), String>> {
    if first {
        *DUEL.lock().unwrap_or_else(PoisonError::into_inner) = Some(Duel::default());
    }
    match name {
        "revenant_shield_bash_knocks" => shield_bash_knocks(game, dim, now, live, first),
        "revenant_shield_front" => shield_front(game, dim, live, first),
        "marksman_reload_cover" => marksman_reload_cover(game, dim, now, live, first),
        "brute_wave_spares_jumper" => brute_wave_spares_jumper(game, dim, now, live, first),
        other => Some(Err(format!("no scenario {other}"))),
    }
}

/// The wall and anything else a duel built on the platform; its state.
fn duel_cleanup(game: &mut Game<'_>) {
    *DUEL.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let (x, z) = (BASE[0] as i32, BASE[2] as i32);
    // Refused when there is nothing to clear (vanilla's fill fails on no change): not logged.
    game.sc_run_command(&format!("fill {} 101 {} {} 106 {} air", x - 10, z - 10, x + 10, z + 10));
}

/// A revenant at P 0 facing south at `at`, still (NoAI: the server's brain runs, the body stays) or not.
fn revenant(game: &mut Game<'_>, dim: &str, at: [f64; 3], still: bool) -> Option<u32> {
    let still = if still { ",NoAI:1b" } else { "" };
    let snbt = format!("{{Rotation:[0f,0f],Tags:[\"soulscraft.p=0.00\",\"{}\"],PersistenceRequired:1b{still}}}", server::TAG_FRESH);
    summon(game, dim, "soulscraft:revenant", at, &snbt)
}

fn pos_of(game: &Game<'_>, id: u32) -> Option<[f64; 3]> {
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z])
}

/// A still revenant (NoAI) strikes a sheep that cannot walk 1.5 blocks in front of it, twice from
/// the same spot: an ordinary thrust (the hurt's own push), then the shield bash (`knockback`,
/// `knockback_strength`). The bash must push it at least [`KNOCK_MARGIN`] farther than the thrust.
/// The revenant takes the sheep for a shield holder (`test_shield`), so the bash holds the
/// shield-breaking axe; the sheep has no shield, so the bash must still do the data's blow.
fn shield_bash_knocks(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        live.mob = revenant(game, dim, BASE, true);
        let at = [BASE[0], BASE[1], BASE[2] + GAP];
        live.golem = summon(game, dim, "minecraft:sheep", at, "{Tags:[\"soulscraft.selftest_sheep\"],PersistenceRequired:1b}");
        // It may not walk off (nor panic away when hurt): only the blows move it.
        run_cmd(game, "attribute @e[tag=soulscraft.selftest_sheep,limit=1] minecraft:movement_speed base set 0");
        return (live.mob.is_none() || live.golem.is_none()).then(|| Err("revenant or sheep not spawned".into()));
    }
    let (mob, sheep) = (live.mob?, live.golem?);
    // The last blow of `attack` since the mark, once the sheep has landed after it.
    let settled = |attack: &str, mark: u64| {
        server::state()
            .strikes
            .iter()
            .rev()
            .find(|r| r.mob == mob && r.attack == attack && r.tick > mark)
            .filter(|r| now >= r.tick + KNOCK_SETTLE)
            .cloned()
    };
    match live.note.as_str() {
        "" => {
            // Seen with a shield up, the revenant circles instead of picking its own attacks.
            if !aim(live, true) {
                return None;
            }
            if let Err(e) = force(live, "thrust") {
                return Some(Err(e));
            }
            DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_mut()?.from = pos_of(game, sheep);
            live.mark = now;
            live.note = "thrust".into();
            None
        }
        "thrust" => {
            let rec = settled("thrust", live.mark)?;
            let from = DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_ref()?.from?;
            let plain = soulscraft_rules::geom::flat_dist(from, pos_of(game, sheep)?);
            if !rec.hit {
                return Some(Err(format!("the thrust missed (the sheep moved {plain:.2})")));
            }
            DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_mut()?.plain = plain;
            game.sc_teleport_mob(dim, sheep, from);
            if let Some(m) = crate::game::soulscraft_find::mob_mut(game, sheep) {
                m.invulnerable = 0;
            }
            live.golem_health = health(game, sheep)?;
            if let Err(e) = force(live, "shield_bash") {
                return Some(Err(e));
            }
            live.mark = now;
            live.note = "bash".into();
            None
        }
        _ => {
            let rec = settled("shield_bash", live.mark)?;
            let (from, plain) = DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(|d| (d.from, d.plain))?;
            let Some(after) = health(game, sheep) else {
                return Some(Err("the sheep is gone after the bash (killed by an axe-sized blow?)".into()));
            };
            let moved = soulscraft_rules::geom::flat_dist(from?, pos_of(game, sheep)?);
            let lost = live.golem_health - after;
            let blow = {
                let s = server::state();
                let def = s.data.mob("revenant")?;
                server::scaled(def, 0.0, 0).blow(def.attack("shield_bash")?.damage)
            };
            log::info!("soulscraft: selftest shield_bash hit={} plain={plain:.2} moved={moved:.2} lost={lost:.2} blow={blow:.2}", rec.hit);
            let mut problems = Vec::new();
            if !rec.hit {
                problems.push("the bash missed".to_owned());
            }
            if moved < plain + KNOCK_MARGIN {
                problems.push(format!("the bash pushed {moved:.2} blocks, the thrust {plain:.2}: wanted {KNOCK_MARGIN} more"));
            }
            if !close(lost, blow) {
                problems.push(format!("the bash took {lost:.2} health, the blow is {blow:.2}"));
            }
            Some(if problems.is_empty() { Ok(()) } else { Err(problems.join("; ")) })
        }
    }
}

fn shield_front(game: &mut Game<'_>, dim: &str, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        live.mob = revenant(game, dim, BASE, true);
        // Its target, out of its reach in front: it guards and never swings.
        let at = [BASE[0], BASE[1], BASE[2] + 4.0];
        live.golem = summon(game, dim, "minecraft:iron_golem", at, "{NoAI:1b,PersistenceRequired:1b}");
        return (live.mob.is_none() || live.golem.is_none()).then(|| Err("revenant or golem not spawned".into()));
    }
    let id = live.mob?;
    let hp = health(game, id)? as f32;
    match live.note.as_str() {
        "" => {
            if !aim(live, false) || !server::state().mobs.get(&id).is_some_and(|t| t.guarding) {
                return None;
            }
            live.golem_health = f64::from(hp);
            live.note = "front".into();
            (!shield_test_hit(game, dim, id, 3.0)).then(|| Err("the front blow was refused".into()))
        }
        "front" => {
            DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_mut()?.front = live.golem_health as f32 - hp;
            live.golem_health = f64::from(hp);
            live.note = "back".into();
            (!shield_test_hit(game, dim, id, -3.0)).then(|| Err("the back blow was refused".into()))
        }
        _ => {
            let front = DUEL.lock().unwrap_or_else(PoisonError::into_inner).as_ref()?.front;
            let back = live.golem_health as f32 - hp;
            // Armour takes at most armour/25 of a blow: from behind, the rest is lost whole.
            let armor = server::state().data.mob("revenant").map_or(0.0, |m| m.stats.armor) as f32;
            let full = SHIELD_TEST_BLOW * (1.0 - armor / 25.0);
            log::info!("soulscraft: selftest shield_front front={front:.2} back={back:.2} full>={full:.2}");
            Some(if front <= 0.2 * back + 0.01 && back >= full - 0.01 {
                Ok(())
            } else {
                Err(format!("front lost {front:.2}, back {back:.2} (wanted front at most a fifth, back at least {full:.2})"))
            })
        }
    }
}

/// A player's blow on the revenant `id` from `dz` blocks south of the platform's centre (in front
/// of it for `dz` > 0): a stand-in player there for the damage hook, and the hurt cooldown cleared.
fn shield_test_hit(game: &mut Game<'_>, dim: &str, id: u32, dz: f64) -> bool {
    if let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) {
        m.invulnerable = 0;
    }
    server::stand_in_player([BASE[0], BASE[1], BASE[2] + dz]);
    game.sc_hurt_entity(dim, id, SHIELD_TEST_BLOW, "minecraft:mob_attack", None)
}

fn marksman_reload_cover(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        // The wall: x +4..+6, z +3, y 101..103 from the marksman's block; its shot south stays clear.
        let (x, z) = (BASE[0].floor() as i32, BASE[2].floor() as i32);
        run_cmd(game, &format!("fill {} 101 {} {} 103 {} stone", x + 4, z + 3, x + 6, z + 3));
        live.note = "aimed_shot".into();
        return arrange(game, dim, "marksman", BASE, 0.0, live).err().map(Err);
    }
    let (mob, golem) = (live.mob?, live.golem?);
    if live.mark == 0 {
        if !aim(live, false) {
            return None;
        }
        if let Err(e) = force(live, "aimed_shot") {
            return Some(Err(e));
        }
        live.mark = now;
        return None;
    }
    let shot = server::state().strikes.iter().any(|r| r.mob == mob && r.attack == "aimed_shot" && r.tick > live.mark);
    if !shot {
        return None;
    }
    let (eye, top) = {
        let s = server::state();
        let m = s.data.mob("marksman")?;
        (m.hitbox_at(0).eye, m.param("cover_min_height_blocks").unwrap_or(2.0))
    };
    let at = pos_of(game, mob)?;
    let g = game.mobs.iter().find(|m| m.id == golem)?;
    let seer = [g.position.x, g.position.y + g.eye_height(), g.position.z];
    let seen = [eye, top].iter().any(|h| crate::game::soulscraft_cover::sees(game, seer, [at[0], at[1] + h, at[2]]));
    live.note = format!("shot, still seen at {:.1},{:.1},{:.1}", at[0], at[1], at[2]);
    (!seen).then_some(Ok(()))
}

/// Where brute_wave_spares_jumper's golem stands: at the hammer's impact point (`impact_ahead_blocks`
/// 2 south of a brute facing south), on the platform, or a block up for the jump.
const WAVE_GOLEM_AT: [f64; 3] = [BASE[0], BASE[1], BASE[2] + 2.0];
/// Ticks after the first overhead_wave's blow before the second is forced: its recover (20) is over.
const WAVE_RECOVER: u64 = 25;

/// A still brute (NoAI: the brain runs, the body stays) swings `overhead_wave` at a still iron
/// golem at its impact point twice: the golem on the platform loses health; then held a block up
/// (no gravity, put back every tick), as a player mid-jump is over `jump_clear_blocks` 0.5, it
/// loses none and the blow does not land (soulscraft_rules::attack::clears_wave).
fn brute_wave_spares_jumper(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    let up = [WAVE_GOLEM_AT[0], WAVE_GOLEM_AT[1] + 1.0, WAVE_GOLEM_AT[2]];
    if first {
        let snbt = format!("{{Rotation:[0f,0f],Tags:[\"soulscraft.p=0.00\",\"{}\"],PersistenceRequired:1b,NoAI:1b}}", server::TAG_FRESH);
        live.mob = summon(game, dim, "soulscraft:brute", BASE, &snbt);
        live.golem = summon(game, dim, "minecraft:iron_golem", WAVE_GOLEM_AT, "{NoAI:1b,PersistenceRequired:1b}");
        return (live.mob.is_none() || live.golem.is_none()).then(|| Err("brute or golem not spawned".into()));
    }
    let (mob, golem) = (live.mob?, live.golem?);
    let blow_since = |mark: u64| {
        server::state().strikes.iter().rev().find(|r| r.mob == mob && r.attack == "overhead_wave" && r.tick > mark).cloned()
    };
    match live.note.as_str() {
        "" => {
            if !aim(live, false) {
                return None;
            }
            live.golem_health = health(game, golem)?;
            if let Err(e) = force(live, "overhead_wave") {
                return Some(Err(e));
            }
            live.mark = now;
            live.note = "ground".into();
            None
        }
        "ground" => {
            let rec = blow_since(live.mark)?;
            if now < rec.tick + WAVE_RECOVER {
                return None;
            }
            let lost = live.golem_health - health(game, golem)?;
            if !rec.hit || lost <= 0.0 {
                return Some(Err(format!("the grounded golem was not hit (hit={}, lost {lost:.2})", rec.hit)));
            }
            game.sc_teleport_mob(dim, golem, up);
            if let Some(g) = crate::game::soulscraft_find::mob_mut(game, golem) {
                g.no_gravity = true;
                g.velocity.x = 0.0;
                g.velocity.y = 0.0;
                g.velocity.z = 0.0;
                g.invulnerable = 0;
                g.needs_sync = true;
            }
            live.golem_health = health(game, golem)?;
            if let Err(e) = force(live, "overhead_wave") {
                return Some(Err(e));
            }
            live.mark = now;
            live.note = "air".into();
            None
        }
        _ => {
            let Some(rec) = blow_since(live.mark) else {
                // Held where a jump would have it until the blow falls.
                game.sc_teleport_mob(dim, golem, up);
                return None;
            };
            let y = pos_of(game, golem)?[1];
            let lost = live.golem_health - health(game, golem)?;
            log::info!("soulscraft: selftest brute_wave_spares_jumper hit={} lost={lost:.2} feet={:.2}", rec.hit, y - BASE[1]);
            Some(if !rec.hit && lost <= 0.0 && y - BASE[1] > 0.5 {
                Ok(())
            } else {
                Err(format!("the golem {:.2} over the platform: blow hit={}, lost {lost:.2}", y - BASE[1], rec.hit))
            })
        }
    }
}

/// A dead flyer's body is taken away at death time 20 (game/creatures.rs:1378): the fall is read
/// until then.
const BODY_TICKS: u64 = 20;

/// A shrike hovering 4 blocks over the platform (no target: it holds still, its gravity off),
/// killed: its body falls at least a block (the death hook gives it its gravity back). The lowest
/// height seen while the body lasts counts, at most 20 ticks after the kill.
fn shrike_falls(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        let at = [BASE[0], BASE[1] + 4.0, BASE[2]];
        let id = crate::game::soulscraft_tools::summon(game, dim, "shrike", at, 0.0, 0);
        live.mob = id;
        return id.is_none().then(|| Err("shrike not spawned".into()));
    }
    let id = live.mob?;
    let y = game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| m.position.y);
    if live.mark == 0 {
        // Adopted (its gravity off from then) and still for a few ticks: the kill.
        let flying = crate::game::soulscraft_find::mob(game, id).is_some_and(|m| m.no_gravity);
        if !server::state().mobs.contains_key(&id) || !flying || live.step < 6 {
            return None;
        }
        live.y0 = y?;
        live.low = live.y0;
        if !game.sc_kill_mob(dim, id) {
            return Some(Err("the shrike was not killed".into()));
        }
        live.mark = now;
        return None;
    }
    if let Some(y) = y {
        live.low = live.low.min(y);
    }
    let gone = y.is_none() || now >= live.mark + BODY_TICKS;
    if !gone {
        return None;
    }
    let fell = live.y0 - live.low;
    log::info!("soulscraft: selftest shrike_falls fell={fell:.2} in {} ticks", now - live.mark);
    Some(if fell >= 1.0 { Ok(()) } else { Err(format!("the dead shrike fell {fell:.2} blocks in {} ticks, wanted 1", now - live.mark)) })
}

/// A size-4 bog slime killed on the platform: 2–3 slimes of size 2 appear round it
/// (`passives.split`). They are taken away after.
fn slime_splits(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, first: bool) -> Option<Result<(), String>> {
    if first {
        let id = crate::game::soulscraft_tools::summon(game, dim, "bog_slime", BASE, 0.0, 4);
        live.mob = id;
        return id.is_none().then(|| Err("bog slime not spawned".into()));
    }
    let id = live.mob?;
    if live.mark == 0 {
        if !server::state().mobs.contains_key(&id) {
            return None;
        }
        if !game.sc_kill_mob(dim, id) {
            return Some(Err("the slime was not killed".into()));
        }
        live.mark = now;
        return None;
    }
    // Killed, split on its death tick, the children adopted the tick after: well within 10.
    if now < live.mark + 10 {
        return None;
    }
    let children: Vec<u32> = {
        let s = server::state();
        s.mobs
            .iter()
            .filter(|(c, t)| **c != id && t.alive && t.size == 2 && s.data.mobs[t.kind].id == "bog_slime")
            .filter(|(_, t)| soulscraft_rules::geom::dist(t.pos, BASE) <= 4.0)
            .map(|(c, _)| *c)
            .collect()
    };
    for c in &children {
        game.sc_remove_mob(dim, *c);
    }
    Some(if (2..=3).contains(&children.len()) {
        Ok(())
    } else {
        Err(format!("{} slimes of size 2 round the dead one, wanted 2 or 3", children.len()))
    })
}

/// Where the second golem of lurker_grab_keeps_its_prey stands: 3 blocks east, out of the grab.
const OTHER_AT: [f64; 3] = [BASE[0] + 3.0, BASE[1], BASE[2]];

/// The lurker's ambush on the golem lands and its grab follows.
/// - `holds`: for 20 ticks the golem stays within 1.5 blocks of the lurker (held at its mouth).
/// - `breaks`: a magic hit (no blow of the golem) frees nothing; then 3 blows of the golem (2 ticks
///   apart, the lurker's hurt cooldown cleared) free it long before grab_max_ticks, and the lurker
///   backs off.
/// - `keeps`: a tick into the grab its target is turned to a second golem (as a nearer player would
///   be chosen); the grab still holds and bites the first, and the second is never moved or hurt.
fn grab(game: &mut Game<'_>, dim: &str, now: u64, live: &mut Live, case: &str, first: bool) -> Option<Result<(), String>> {
    if first {
        live.note = "ambush".into();
        return arrange(game, dim, "lurker", BASE, 0.0, live).err().map(Err);
    }
    let (mob, golem) = (live.mob?, live.golem?);
    if live.mark == 0 {
        if !aim(live, false) {
            return None;
        }
        if let Err(e) = force(live, "ambush") {
            return Some(Err(e));
        }
        live.mark = now;
        return None;
    }
    let mode_since = |mode: &str, after: u64| {
        server::state().modes.iter().find(|(id, m, t)| *id == mob && m == mode && *t >= after).map(|(_, _, t)| *t)
    };
    let grabbed = mode_since("grab", live.mark)?;
    let since = now - grabbed;
    match case {
        "breaks" => grab_breaks(game, dim, mob, golem, grabbed, since, mode_since("backoff", grabbed + 1)),
        "keeps" if since == 1 => {
            let other = summon(game, dim, "minecraft:iron_golem", OTHER_AT, "{NoAI:1b,PersistenceRequired:1b}");
            live.other = other;
            let Some(other) = other else { return Some(Err("no second golem".into())) };
            server::state().mobs.get_mut(&mob)?.target_override = Some(other);
            None
        }
        _ => {
            let at = |id: u32| game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z]);
            let d = soulscraft_rules::geom::dist(at(mob)?, at(golem)?);
            if d > 1.5 {
                return Some(Err(format!("the golem is {d:.2} blocks from the lurker {since} ticks into the grab")));
            }
            if since < 20 {
                return None;
            }
            if case == "keeps" {
                return Some(kept_prey(game, live, golem));
            }
            Some(Ok(()))
        }
    }
}

/// lurker_grab_keeps_its_prey after 20 ticks: the first golem was bitten (at +10 and +20), the
/// second is where it was put and unhurt.
fn kept_prey(game: &Game<'_>, live: &Live, golem: u32) -> Result<(), String> {
    let other = live.other.ok_or("no second golem")?;
    let find = |id: u32| game.mobs.iter().find(|m| m.id == id && !m.removed);
    let g = find(golem).ok_or("the first golem is gone")?;
    let o = find(other).ok_or("the second golem is gone")?;
    let moved = soulscraft_rules::geom::dist([o.position.x, o.position.y, o.position.z], OTHER_AT);
    let bitten = g.health < g.attrs.max_health;
    let spared = o.health >= o.attrs.max_health;
    if bitten && spared && moved < 0.5 {
        Ok(())
    } else {
        Err(format!("first golem bitten {bitten}; second golem unhurt {spared}, moved {moved:.2}"))
    }
}

/// lurker_grab_breaks: a magic hit at +2 does not count; the golem's blows at +4, +6 and +8 free
/// it, so the lurker backs off after +8 and before grab_max_ticks.
#[allow(clippy::too_many_arguments)]
fn grab_breaks(game: &mut Game<'_>, dim: &str, mob: u32, golem: u32, grabbed: u64, since: u64, freed: Option<u64>) -> Option<Result<(), String>> {
    if matches!(since, 2 | 4 | 6 | 8) {
        if let Some(m) = crate::game::soulscraft_find::mob_mut(game, mob) {
            m.invulnerable = 0;
        }
        let landed = if since == 2 {
            game.sc_hurt_entity(dim, mob, 1.0, "minecraft:magic", None)
        } else {
            game.sc_hurt_entity(dim, mob, 1.0, "minecraft:mob_attack", Some(golem))
        };
        if !landed {
            return Some(Err(format!("the hit at +{since} on the lurker did not land")));
        }
        return None;
    }
    let max = server::state().data.mob("lurker").and_then(|m| m.attack("grab")?.grab_max_ticks).unwrap_or(40);
    match freed {
        Some(t) if t - grabbed > 8 && t - grabbed < u64::from(max) => Some(Ok(())),
        Some(t) => Some(Err(format!("freed {} ticks into the grab, wanted just after the golem's third blow (+8)", t - grabbed))),
        None if since > 14 => Some(Err("3 blows of the golem did not free it".into())),
        None => None,
    }
}
