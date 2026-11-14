// The altar's and the event's self-test scenarios (soulscraft_selftest run [altar_|event_|defense_]),
// on their own site in SoulsCraft's area: a stone platform at y 100 round the core at
// [`CORE`], its 3 × 3-chunk event zone held loaded. Each scenario builds what it needs with
// commands, lets the server half do its work in the level's ticks, reads what it did
// (soulscraft_altar's levels, soulscraft_event's snapshots and records, the world) and clears the
// site after itself.
//
// The site and the event's test timing are constants here, for other mods' scenarios (RustCraft's
// turrets against a wave, `rustcraft_testkit turret`): a wave of `TEST_WAVE` zombies telegraphed
// `TEST_TELEGRAPH` ticks, rests of `TEST_REST`, the stand-in participant at `STAND_IN`.
use crate::game::Game;
use crate::game::soulscraft_altar as altar;
use crate::game::soulscraft_event::{self as event, Overrides};
use soulscraft_rules::{
    altar::{self as ra, AltarFile},
    defense::{Goal, Role},
    event::SpawnMob,
    geom::V3,
    view::BeamKind,
};
use std::sync::{Mutex, PoisonError};

/// The test altar's core; the floor (slabs) is at its y, the platform under it.
pub(super) const CORE: [i32; 3] = [-700, 101, 700];
/// The platform reaches this far round the core (the spawn band ends at 20).
const PLATFORM: i32 = 21;
/// Cleared this high over the floor.
const CLEAR_UP: i32 = 10;
/// Where the stand-in participant (a still, invulnerable iron golem) stands.
pub(super) const STAND_IN: [i32; 3] = [2, 0, 3];
/// A stand-in far out in the zone, beyond interception of the wave (spawns lie 12 to 20 blocks out).
const FAR_STAND_IN: [i32; 3] = [-20, 0, -20];
pub(super) const TEST_TELEGRAPH: u64 = 10;
pub(super) const TEST_REST: u64 = 20;
pub(super) const TEST_WAVE: usize = 2;
pub(super) const DIM: &str = "minecraft:overworld";

/// The scenarios and their timeouts in ticks.
pub(super) const CASES: &[(&str, u64)] = &[
    ("altar_levels", 120),
    ("altar_break_drops", 60),
    ("altar_roof", 40),
    ("altar_rebuild_keeps_pieces", 30),
    ("altar_pedestal_starts", 60),
    ("event_waves_win", 300),
    ("event_nobody_fails", 120),
    ("event_drain_fails", 700),
    ("event_path_rebuild", 120),
    ("event_wave_no_vanilla_moves", 200),
    ("defense_intercept", 400),
    ("defense_intercept_not_through_walls", 300),
    ("defense_curse", 400),
    ("defense_breach_restore", 900),
    ("fit_brute_under_ceiling", 40),
    ("fit_wave_spawn_under_roof", 40),
    ("template_lurker_stays_under_water", 1000),
    ("template_brute_no_reinforcements", 90),
];

#[derive(Default)]
pub(super) struct Case {
    pub(super) phase: u32,
    pub(super) mark: u64,
    /// When the current phase began (`mark` is the scenario's start).
    since: u64,
    pub(super) golem: Option<u32>,
    pub(super) note: String,
    probe: Vec<[i32; 3]>,
    pub(super) count: u32,
}

static CASE: Mutex<Option<Case>> = Mutex::new(None);

pub(super) fn at(c: [i32; 3]) -> [i32; 3] {
    [CORE[0] + c[0], CORE[1] + c[1], CORE[2] + c[2]]
}

pub(super) fn centre(c: [i32; 3]) -> V3 {
    [f64::from(c[0]) + 0.5, f64::from(c[1]), f64::from(c[2]) + 0.5]
}

fn cmd(game: &mut Game<'_>, line: &str) {
    if !game.native_run_command(line) {
        log::warn!("soulscraft: selftest command refused: {line}");
    }
}

pub(super) fn set(game: &mut Game<'_>, c: [i32; 3], state: &str) {
    let p = at(c);
    game.native_run_command(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]));
}

/// The event zone (3 × 3 chunks round the core's chunk) loaded for the self-test.
pub(super) fn forceload(game: &mut Game<'_>, on: bool) {
    let r = 24;
    let line = format!(
        "forceload {} {} {} {} {}",
        if on { "add" } else { "remove" },
        CORE[0] - r,
        CORE[2] - r,
        CORE[0] + r,
        CORE[2] + r
    );
    if on {
        cmd(game, &line);
    } else {
        game.native_run_command(&line);
    }
}

/// The platform and the air over it; mobs and items on it gone; no altar remembered.
pub(super) fn clear_site(game: &mut Game<'_>) {
    let (x0, z0, x1, z1) = (CORE[0] - PLATFORM, CORE[2] - PLATFORM, CORE[0] + PLATFORM, CORE[2] + PLATFORM);
    let y = CORE[1];
    game.native_run_command(&format!(
        "kill @e[x={},y={},z={},dx={},dy=24,dz={},type=!player]",
        x0 - 8,
        y - 6,
        z0 - 8,
        2 * PLATFORM + 16,
        2 * PLATFORM + 16
    ));
    game.native_run_command(&format!("fill {x0} {y} {z0} {x1} {} {z1} air", y + CLEAR_UP));
    game.native_run_command(&format!("fill {x0} {} {z0} {x1} {} {z1} stone", y - 1, y - 1));
}

fn site_ready(game: &mut Game<'_>) {
    clear_site(game);
    event::clear_records();
}

/// Ends a scenario: any run stopped, the site cleared.
pub(super) fn cleanup(game: &mut Game<'_>, now: u64) {
    event::stop(game, DIM, CORE, now);
    if let Some(c) = CASE.lock().unwrap_or_else(PoisonError::into_inner).take()
        && let Some(g) = c.golem
    {
        game.native_remove_mob(DIM, g);
    }
    clear_site(game);
    altar::forget_all(DIM);
}

/// Builds the altar up to `level` (bottoms of tall pieces only: the server puts up the rest).
pub(super) fn build(game: &mut Game<'_>, level: u32) {
    let file = AltarFile::embedded();
    for ring in 1..=level {
        for n in ra::ring_needs(&file, ring) {
            if n.part.is_some_and(|p| p != "bottom") {
                continue;
            }
            set(game, n.cell, n.block);
        }
    }
}

pub(super) fn level(game: &mut Game<'_>, now: u64) -> Option<(u32, u32, bool, Vec<String>)> {
    altar::level_now(game, DIM, CORE, now)
}

pub(super) fn items_near(game: &Game<'_>, c: [i32; 3], item: &str, r: f64) -> u32 {
    let p = centre(at(c));
    game.native_item_entities(DIM, [p[0] - r, p[1] - 2.0, p[2] - r], [p[0] + r, p[1] + 4.0, p[2] + r])
        .iter()
        .filter(|e| &*e.stack.item == item)
        .map(|e| e.stack.count)
        .sum()
}

pub(super) fn golem(game: &mut Game<'_>, c: [i32; 3]) -> Option<u32> {
    let Ok((crate::mod_api::Tag::Compound(nbt), _)) =
        crate::mod_api::parse_snbt("{NoAI:1b,Invulnerable:1b,PersistenceRequired:1b}")
    else {
        return None;
    };
    game.native_summon(DIM, "minecraft:iron_golem", centre(at(c)), nbt)
}

fn zombies(n: usize) -> Vec<SpawnMob> {
    (0..n)
        .map(|_| SpawnMob { kind: "minecraft:zombie".into(), role: Role::Assault, armored: false, banner: false, health_mult: 1.0, height: 2 })
        .collect()
}

pub(super) fn start(game: &mut Game<'_>, now: u64, ov: Overrides) -> Result<(), String> {
    let a = altar::altar(DIM, CORE).ok_or("no altar registered")?;
    event::start(game, DIM, &a, 0, now, Some(ov))
}

pub(super) fn has(rec: &[String], prefix: &str) -> bool {
    rec.iter().any(|r| r.starts_with(prefix))
}

pub(super) fn step(game: &mut Game<'_>, now: u64, name: &str, first: bool) -> Option<Result<(), String>> {
    let mut guard = CASE.lock().unwrap_or_else(PoisonError::into_inner);
    if first {
        *guard = Some(Case { mark: now, ..Case::default() });
        drop(guard);
        site_ready(game);
        return None;
    }
    let c = guard.as_mut()?;
    let t = now - c.mark;
    let out = match name {
        "altar_levels" => levels(game, now, t, c),
        "altar_break_drops" => break_drops(game, now, t, c),
        "altar_roof" => roof(game, now, t, c),
        "altar_rebuild_keeps_pieces" => rebuild_altar(game, now, t),
        "altar_pedestal_starts" => pedestal(game, now, t, c),
        "event_waves_win" => waves_win(game, now, t, c),
        "event_nobody_fails" => nobody(game, now, t, c),
        "event_drain_fails" => drain_fails(game, now, t, c),
        "event_path_rebuild" => rebuild(game, now, t, c),
        "event_wave_no_vanilla_moves" => wave_no_moves(game, now, t, c),
        "defense_intercept" => intercept(game, now, t, c),
        "defense_intercept_not_through_walls" => intercept_walled(game, now, t, c),
        "defense_curse" => curse(game, now, t, c),
        "defense_breach_restore" => breach(game, now, t, c),
        "fit_brute_under_ceiling" => fit_ceiling(game, t, c),
        "fit_wave_spawn_under_roof" => fit_roof(game, t, c),
        "template_lurker_stays_under_water" => stays_lurker(game, t, c),
        "template_brute_no_reinforcements" => no_reinforcements(game, t, c),
        other => crate::game::soulscraft_selftest_trophy::step(game, now, t, other, c),
    };
    out
}

/// Each level built ring by ring: the level counted, tall pieces put up whole and lowered on
/// their slabs, a banner on a socket facing out and counted.
fn levels(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    // Ring k is built at t = 1 + 6(k - 1) and checked 4 ticks later.
    let k = ((t.saturating_sub(1)) / 6 + 1) as u32;
    let phase = (t.saturating_sub(1)) % 6;
    if k <= 4 && phase == 0 && c.phase < k {
        c.phase = k;
        let file = AltarFile::embedded();
        for n in ra::ring_needs(&file, k) {
            if n.part.is_none_or(|p| p == "bottom") {
                set(game, n.cell, n.block);
            }
        }
        if k == 2 {
            set(game, [-2, 1, 0], "soulscraft:banner_weight");
        }
        return None;
    }
    if k <= 4 && phase == 4 {
        let (lv, built, sky, banners) = level(game, now)?;
        if lv != k || built != k || !sky {
            return Some(Err(format!("ring {k} built: level {lv}, built {built}, sky {sky}")));
        }
        if k == 2 {
            let corner = at([2, 2, 2]);
            let part = game.native_block_property(corner, "part");
            let on_slab = game.native_block_property(at([2, 1, 2]), "on_slab");
            let facing = game.native_block_property(at([-2, 1, 0]), "facing");
            let top = game.native_block(at([-2, 2, 0]));
            if part.as_deref() != Some("top") || on_slab.as_deref() != Some("true") {
                return Some(Err(format!("pillar top {part:?}, bottom on_slab {on_slab:?}")));
            }
            if facing.as_deref() != Some("west") || top != "soulscraft:banner_weight" || banners != ["weight"] {
                return Some(Err(format!("banner facing {facing:?}, top {top}, counted {banners:?}")));
            }
        }
        if k == 4 {
            let top = game.native_block_property(at([4, 3, 4]), "part");
            return Some(if top.as_deref() == Some("top") { Ok(()) } else { Err(format!("obelisk top {top:?}")) });
        }
    }
    None
}

/// Level 2 loses a slab (broken with drops): level 1 and the slab dropped; a pillar's top broken:
/// its bottom goes too and one pillar drops.
fn break_drops(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 2),
        5 => {
            let (lv, ..) = level(game, now)?;
            if lv != 2 {
                return Some(Err(format!("level {lv} before the break, wanted 2")));
            }
            set(game, [1, 0, -2], "air destroy");
            set(game, [2, 2, 2], "air destroy");
        }
        10 => {
            let (lv, ..) = level(game, now)?;
            let slabs = items_near(game, [1, 0, -2], "soulscraft:altar_slab", 2.0);
            let pillars = items_near(game, [2, 1, 2], "soulscraft:altar_pillar", 2.5);
            let bottom = game.native_block(at([2, 1, 2]));
            c.note = format!("level {lv}, slabs {slabs}, pillars {pillars}, bottom {bottom}");
            return Some(if lv == 1 && slabs == 1 && pillars == 1 && bottom == "minecraft:air" { Ok(()) } else { Err(c.note.clone()) });
        }
        _ => {}
    }
    None
}

/// A roof over the core takes the level away; gone, the level comes back.
fn roof(game: &mut Game<'_>, now: u64, t: u64, _c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 1),
        5 => {
            let (lv, ..) = level(game, now)?;
            if lv != 1 {
                return Some(Err(format!("level {lv} in the open, wanted 1")));
            }
            set(game, [0, 6, 0], "stone");
        }
        9 => {
            let (lv, built, sky, _) = level(game, now)?;
            if lv != 0 || built != 1 || sky {
                return Some(Err(format!("under a roof: level {lv}, built {built}, sky {sky}")));
            }
            set(game, [0, 6, 0], "air");
        }
        13 => {
            let (lv, _, sky, _) = level(game, now)?;
            return Some(if lv == 1 && sky { Ok(()) } else { Err(format!("roof gone: level {lv}, sky {sky}")) });
        }
        _ => {}
    }
    None
}

/// The horn on the pedestal of a level-1 altar starts the event and is spent.
fn pedestal(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 1),
        5 => {
            level(game, now)?;
            c.golem = golem(game, STAND_IN);
            altar::put(DIM, at([0, 1, 0]), "soulscraft:rotten_horn");
        }
        8..=30 => {
            if event::running(DIM, CORE) {
                let left = altar::pedestal_item(game, at([0, 1, 0]));
                let started = has(&event::records(), "start ");
                return Some(if left.is_none() && started { Ok(()) } else { Err(format!("running, pedestal {left:?}, started {started}")) });
            }
        }
        _ => {}
    }
    None
}

/// A short run with a stand-in: spawn points shown, then the wave (each zombie with one
/// `pct.xp100`), killed; the rest; the second wave killed; won, the reward dropped.
fn waves_win(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        build(game, 1);
        return None;
    }
    if t == 4 {
        level(game, now)?;
        c.golem = golem(game, STAND_IN);
        let ov = Overrides {
            waves: Some(vec![zombies(TEST_WAVE), zombies(TEST_WAVE)]),
            telegraph_ticks: Some(TEST_TELEGRAPH),
            rest_ticks: Some(TEST_REST),
            power: Some(0.0),
            stand_ins: c.golem.into_iter().collect(),
            ..Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    if t < 5 {
        return None;
    }
    let rec = event::records();
    if c.phase == 0 {
        let snap = event::snapshot(DIM, CORE, None)?;
        if snap.spawns.is_empty() {
            return Some(Err("no spawn points shown in the telegraph".into()));
        }
        c.phase = 1;
        return None;
    }
    if has(&rec, "won") {
        let heads = items_near(game, [0, 1, 0], "minecraft:zombie_head", 3.0);
        return Some(if heads >= 1 && has(&rec, "reward") && c.count == 2 * TEST_WAVE as u32 {
            Ok(())
        } else {
            Err(format!("won: heads {heads}, killed {}", c.count))
        });
    }
    let ids = event::wave_ids(DIM, CORE);
    if !ids.is_empty() && c.phase < 3 {
        // One tick after they appear the event has adopted them.
        if c.mark != 0 && c.note != format!("{ids:?}") {
            c.note = format!("{ids:?}");
            return None;
        }
        for id in &ids {
            let tags = game.native_entity(*id).and_then(|e| game.native_entity_tags(e)).unwrap_or_default();
            let xp: Vec<&String> = tags.iter().filter(|t| t.starts_with("pct.xp")).collect();
            if xp.len() != 1 || xp[0] != "pct.xp100" {
                return Some(Err(format!("zombie {id} experience tags {xp:?}")));
            }
        }
        for id in ids {
            game.native_kill_mob(DIM, id);
            c.count += 1;
        }
        c.phase += 1;
        c.note.clear();
    }
    None
}

/// Nobody in the zone: the run fails after the grace.
fn nobody(game: &mut Game<'_>, now: u64, t: u64, _c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 1),
        4 => {
            level(game, now)?;
            let ov = Overrides { waves: Some(vec![zombies(1)]), telegraph_ticks: Some(200), grace_ticks: Some(20), power: Some(0.0), ..Overrides::default() };
            return start(game, now, ov).err().map(Err);
        }
        _ if t > 4 => {
            let rec = event::records();
            if has(&rec, "lost=nobody") {
                return Some(if !event::running(DIM, CORE) && t >= 24 { Ok(()) } else { Err(format!("lost at t={t}")) });
            }
        }
        _ => {}
    }
    None
}

/// Zombies reach the altar along the path map, wind up 40 ticks and drain it; with no one to
/// stop them a small altar falls.
fn drain_fails(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 1),
        4 => {
            level(game, now)?;
            // The stand-in stays far out in the zone, beyond interception.
            c.golem = golem(game, [-20, 0, -20]);
            let ov = Overrides {
                waves: Some(vec![zombies(2)]),
                telegraph_ticks: Some(5),
                altar_health: Some(3.0),
                power: Some(0.0),
                stand_ins: c.golem.into_iter().collect(),
                ..Overrides::default()
            };
            return start(game, now, ov).err().map(Err);
        }
        _ if t > 4 => {
            if let Some(s) = event::snapshot(DIM, CORE, None) {
                let windup = s.mobs.iter().any(|m| m.4 == Some(BeamKind::Windup));
                if windup && c.probe.is_empty() {
                    c.probe.push([now as i32, 0, 0]);
                }
            }
            let rec = event::records();
            if has(&rec, "lost=altar") {
                let first_windup = c.probe.first().map(|p| p[0] as u64);
                let drain_at = rec
                    .iter()
                    .find(|r| r.starts_with("drain "))
                    .and_then(|r| r.rsplit("tick=").next())
                    .and_then(|v| v.parse::<u64>().ok());
                return Some(match (first_windup, drain_at) {
                    (Some(w), Some(d)) if d >= w + 39 => Ok(()),
                    other => Err(format!("windup and drain ticks {other:?}")),
                });
            }
        }
        _ => {}
    }
    None
}

/// A wall put up in the zone during a run makes the path map be built again, once, after the
/// debounce; and it reads the wall (the way round is longer).
fn rebuild(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let probe = at([10, 0, 0]);
    match t {
        1 => build(game, 1),
        4 => {
            level(game, now)?;
            c.golem = golem(game, STAND_IN);
            let ov = Overrides {
                waves: Some(vec![zombies(1)]),
                telegraph_ticks: Some(400),
                power: Some(0.0),
                stand_ins: c.golem.into_iter().collect(),
                ..Overrides::default()
            };
            return start(game, now, ov).err().map(Err);
        }
        6 => {
            let s = event::snapshot(DIM, CORE, Some(probe))?;
            c.count = s.builds;
            c.probe.push([s.open_from.map_or(-1, |d| d as i32), 0, 0]);
            // A wall across the way from the east, 9 wide, 3 high, put up over 3 ticks.
            let p = at([6, 0, -4]);
            game.native_run_command(&format!("fill {} {} {} {} {} {} cobblestone", p[0], p[1], p[2], p[0], p[1] + 2, p[2] + 2));
        }
        7 | 8 => {
            let p = at([6, 0, if t == 7 { -1 } else { 2 }]);
            game.native_run_command(&format!("fill {} {} {} {} {} {} cobblestone", p[0], p[1], p[2], p[0], p[1] + 2, p[2] + 2));
        }
        9..=60 => {
            let s = event::snapshot(DIM, CORE, Some(probe))?;
            if s.builds > c.count + 1 {
                return Some(Err(format!("built {} times for three changes within a second", s.builds - c.count)));
            }
            if s.builds == c.count + 1 {
                let before = c.probe[0][0];
                let after = s.open_from.map_or(-1, |d| d as i32);
                return Some(if after > before && before > 0 && t >= 24 {
                    Ok(())
                } else {
                    Err(format!("rebuilt at t={t}: steps from the probe {before} -> {after}"))
                });
            }
        }
        _ => {}
    }
    None
}

/// A zombie turns on a participant who comes within 6 blocks, goes back to the altar when they
/// are farther than 16, and turns on them again when hit by them.
fn intercept(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        build(game, 1);
        return None;
    }
    if t == 4 {
        level(game, now)?;
        c.golem = golem(game, [-20, 0, -20]);
        let ov = Overrides {
            waves: Some(vec![zombies(1)]),
            telegraph_ticks: Some(5),
            power: Some(0.0),
            curse_after_ticks: Some(10_000),
            stand_ins: c.golem.into_iter().collect(),
            ..Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    if t < 5 {
        return None;
    }
    let s = event::snapshot(DIM, CORE, None)?;
    let Some(&(zid, _, goal, _, _)) = s.mobs.first() else { return None };
    let g = c.golem?;
    let zpos = game.mobs.iter().find(|m| m.id == zid).map(|m| [m.position.x, m.position.y, m.position.z])?;
    match c.phase {
        0 => {
            if goal != Goal::Altar {
                return Some(Err(format!("before anyone came near: {goal:?}")));
            }
            game.native_teleport_mob("minecraft:overworld", g, toward(zpos, 3.0));
            c.phase = 1;
            c.since = now;
        }
        1 if now - c.since >= 3 => {
            if goal != Goal::Player(g) {
                return Some(Err(format!("a participant 3 blocks off: {goal:?}")));
            }
            let far = toward(zpos, 17.5);
            game.native_teleport_mob("minecraft:overworld", g, far);
            c.phase = 2;
            c.since = now;
        }
        2 if now - c.since >= 3 => {
            if goal != Goal::Altar {
                return Some(Err(format!("the participant 17.5 blocks off: {goal:?}")));
            }
            game.native_teleport_mob("minecraft:overworld", g, toward(zpos, 10.0));
            game.native_hurt_entity("minecraft:overworld", zid, 1.0, "minecraft:mob_attack", Some(g));
            c.phase = 3;
            c.since = now;
        }
        3 if now - c.since >= 3 => {
            return Some(if goal == Goal::Player(g) { Ok(()) } else { Err(format!("hit from 10 blocks: {goal:?}")) });
        }
        _ => {}
    }
    None
}

/// A wall nothing breaks round the altar: the zombie has no way, and after the (shortened) curse
/// time it stands and curses the altar through the wall, which loses health.
fn curse(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => {
            build(game, 1);
            ring_wall(game, 6, "bedrock");
        }
        4 => {
            level(game, now)?;
            c.golem = golem(game, FAR_STAND_IN);
            let ov = Overrides {
                waves: Some(vec![zombies(1)]),
                telegraph_ticks: Some(5),
                power: Some(0.0),
                curse_after_ticks: Some(40),
                stand_ins: c.golem.into_iter().collect(),
                ..Overrides::default()
            };
            return start(game, now, ov).err().map(Err);
        }
        _ if t > 4 => {
            let s = event::snapshot(DIM, CORE, None)?;
            if let Some(m) = s.mobs.first()
                && m.3
                && s.hp < s.max - 0.1
            {
                return Some(if m.4 == Some(BeamKind::Curse) && has(&event::records(), "curse") {
                    Ok(())
                } else {
                    Err(format!("cursed with beam {:?}", m.4))
                });
            }
        }
        _ => {}
    }
    None
}

/// A wall round the altar, 3 high, at Chebyshev distance `r`.
fn ring_wall(game: &mut Game<'_>, r: i32, block: &str) {
    let (x0, z0, x1, z1) = (CORE[0] - r, CORE[2] - r, CORE[0] + r, CORE[2] + r);
    let (y0, y1) = (CORE[1], CORE[1] + 2);
    for (a, b) in [((x0, z0), (x1, z0)), ((x0, z1), (x1, z1)), ((x0, z0), (x0, z1)), ((x1, z0), (x1, z1))] {
        game.native_run_command(&format!("fill {} {y0} {} {} {y1} {} {block}", a.0, a.1, b.0, b.1));
    }
}

/// The brute walled out breaks through a cobblestone wall (nothing else); the broken blocks drop
/// nothing; one the «player» built on stays; the rest come back when the run ends.
fn breach(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    match t {
        1 => {
            build(game, 1);
            ring_wall(game, 6, "cobblestone");
        }
        4 => {
            level(game, now)?;
            c.golem = golem(game, FAR_STAND_IN);
            let brute = SpawnMob { kind: "soulscraft:brute".into(), role: Role::Assault, armored: false, banner: false, health_mult: 1.0, height: 4 };
            let ov = Overrides {
                waves: Some(vec![vec![brute]]),
                telegraph_ticks: Some(5),
                power: Some(0.0),
                stand_ins: c.golem.into_iter().collect(),
                ..Overrides::default()
            };
            return start(game, now, ov).err().map(Err);
        }
        _ if t > 4 => {
            let rec = event::records();
            let broke: Vec<[i32; 3]> = rec
                .iter()
                .filter(|r| r.starts_with("broke "))
                .filter_map(|r| r.split("at=").nth(1)?.split(' ').next().map(str::to_owned))
                .filter_map(|p| {
                    let v: Vec<i32> = p.split(',').filter_map(|n| n.parse().ok()).collect();
                    (v.len() == 3).then(|| [v[0], v[1], v[2]])
                })
                .collect();
            if c.phase == 0 && !broke.is_empty() {
                // Some blocks broken: none dropped, and the «player» builds on the first place.
                let p = broke[0];
                let dropped = game
                    .native_item_entities(DIM, [f64::from(p[0]) - 3.0, f64::from(p[1]) - 1.0, f64::from(p[2]) - 3.0], [f64::from(p[0]) + 4.0, f64::from(p[1]) + 4.0, f64::from(p[2]) + 4.0])
                    .len();
                if dropped > 0 {
                    return Some(Err(format!("{dropped} items dropped by the breach")));
                }
                game.native_run_command(&format!("setblock {} {} {} oak_planks", p[0], p[1], p[2]));
                c.probe = broke.clone();
                c.phase = 1;
                c.since = now;
                return None;
            }
            if c.phase == 1 && now - c.since >= 3 {
                let all = broke;
                event::stop(game, DIM, CORE, now);
                let kept = game.native_block(all[0]);
                let back = all[1..].iter().filter(|p| game.native_block(**p) == "minecraft:cobblestone").count();
                let rec = event::records();
                return Some(if kept == "minecraft:oak_planks" && back == all.len() - 1 && has(&rec, "restored") {
                    Ok(())
                } else {
                    Err(format!("broken {}, kept {kept}, back {back}", all.len()))
                });
            }
        }
        _ => {}
    }
    None
}

/// `blocks` from `from` toward the core, on the ground.
fn toward(from: V3, blocks: f64) -> V3 {
    let c = centre(CORE);
    let d = soulscraft_rules::geom::flat_dir(from, c);
    [from[0] + d[0] * blocks, from[1], from[2] + d[2] * blocks]
}

fn mob_pos(game: &Game<'_>, id: u32) -> Option<V3> {
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z])
}

fn fits_here(game: &Game<'_>, at: V3, width: f64, height: f64) -> bool {
    let boxes = |c: [i32; 3]| game.native_block_collision(c);
    soulscraft_rules::fit::fits(at, width, height, &boxes)
}

/// Round 9: a brute asked for under a ceiling 3 blocks over the floor (its box is 3.2) appears
/// where its box fits, not inside the ceiling.
fn fit_ceiling(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let spot = at([8, 0, 0]);
    match t {
        1 => {
            game.native_run_command(&format!("fill {} {} {} {} {} {} stone", spot[0] - 2, spot[1] + 3, spot[2] - 2, spot[0] + 2, spot[1] + 3, spot[2] + 2));
        }
        3 => {
            c.golem = crate::game::soulscraft_tools::summon(game, DIM, "brute", centre(spot), 0.0, 0);
            if c.golem.is_none() {
                return Some(Err("no brute: no spot within the search radius".into()));
            }
        }
        6 => {
            let p = mob_pos(game, c.golem?)?;
            let moved = (p[0] - centre(spot)[0]).abs().max((p[2] - centre(spot)[2]).abs());
            return Some(if fits_here(game, p, 1.6, 3.2) && moved >= 3.0 {
                Ok(())
            } else {
                Err(format!("brute at {p:?}, moved {moved:.1}, fits {}", fits_here(game, p, 1.6, 3.2)))
            });
        }
        _ => {}
    }
    None
}

/// Round 9: a wave's zombie asked for under a roof one block over the floor moves out to open
/// space (silently).
fn fit_roof(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let spot = at([-8, 0, 0]);
    match t {
        1 => {
            game.native_run_command(&format!("fill {} {} {} {} {} {} stone", spot[0] - 2, spot[1] + 1, spot[2] - 2, spot[0] + 2, spot[1] + 1, spot[2] + 2));
        }
        3 => {
            c.golem = event::spawn_one(game, DIM, &zombies(1)[0], centre(spot), 0.0);
            if c.golem.is_none() {
                return Some(Err("no zombie: no spot within the search radius".into()));
            }
        }
        6 => {
            let p = mob_pos(game, c.golem?)?;
            let out = (p[0] - centre(spot)[0]).abs().max((p[2] - centre(spot)[2]).abs()) >= 2.5;
            return Some(if fits_here(game, p, 0.6, 1.95) && out { Ok(()) } else { Err(format!("zombie at {p:?}")) });
        }
        _ => {}
    }
    None
}

/// An operator's rebuild in one tick (the old altar cleared, the same one set again): the new
/// pieces stand whole and nothing drops (the old segments' removal must not take the new ones).
fn rebuild_altar(game: &mut Game<'_>, now: u64, t: u64) -> Option<Result<(), String>> {
    match t {
        1 => build(game, 2),
        5 => {
            let (x0, z0, x1, z1) = (CORE[0] - 3, CORE[2] - 3, CORE[0] + 3, CORE[2] + 3);
            game.native_run_command(&format!("fill {x0} {} {z0} {x1} {} {z1} air", CORE[1], CORE[1] + 3));
            build(game, 2);
        }
        10 => {
            let (lv, ..) = level(game, now)?;
            let dropped = items_near(game, [0, 0, 0], "soulscraft:altar_pillar", 5.0);
            return Some(if lv == 2 && dropped == 0 { Ok(()) } else { Err(format!("level {lv}, pillars dropped {dropped}")) });
        }
        _ => {}
    }
    None
}

/// A water column 3 deep under the platform, closed on every side and on top, at `c`.
fn water_column(game: &mut Game<'_>, c: [i32; 3]) {
    let p = at(c);
    game.native_run_command(&format!("fill {} {} {} {} {} {} stone", p[0] - 1, p[1] - 5, p[2] - 1, p[0] + 1, p[1] - 1, p[2] + 1));
    game.native_run_command(&format!("fill {} {} {} {} {} {} water", p[0], p[1] - 4, p[2], p[0], p[1] - 2, p[2]));
}

/// The zombie template turns a mob kept under water 900 ticks into a vanilla drowned
/// (docs/research/horizon-mob-models.md, 12): a lurker held in a closed water column stays a
/// lurker with its P, while a vanilla zombie in the next column, the control, does turn.
fn stays_lurker(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let (a, b) = ([-8, 0, 8], [-8, 0, 12]);
    match t {
        1 => {
            water_column(game, a);
            water_column(game, b);
        }
        3 => {
            let pa = at(a);
            let pb = at(b);
            let lurker = format!("{{Tags:[\"soulscraft.p=0.30\",\"soulscraft.fresh\"],PersistenceRequired:1b}}");
            let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&lurker) else { return Some(Err("snbt".into())) };
            c.golem = game.native_summon(DIM, "soulscraft:lurker", [f64::from(pa[0]) + 0.5, f64::from(pa[1]) - 4.0, f64::from(pa[2]) + 0.5], nbt);
            let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt("{PersistenceRequired:1b}") else { return Some(Err("snbt".into())) };
            let z = game.native_summon(DIM, "minecraft:zombie", [f64::from(pb[0]) + 0.5, f64::from(pb[1]) - 4.0, f64::from(pb[2]) + 0.5], nbt);
            c.probe = vec![[z.map_or(-1, |z| z as i32), 0, 0]];
            if c.golem.is_none() || z.is_none() {
                return Some(Err("no lurker or no zombie".into()));
            }
        }
        _ if t > 3 => {
            let id = c.golem?;
            let lurker = game.mobs.iter().find(|m| m.id == id && !m.removed);
            match lurker {
                Some(m) if m.fluids.eye_in_water => c.count += 1,
                Some(_) => c.count = 0,
                None => return Some(Err(format!("the lurker is gone after {t} ticks ({} under water)", c.count))),
            }
            if t >= 960 {
                let m = game.mobs.iter().find(|m| m.id == id && !m.removed)?;
                let kind = m.kind.id.clone();
                let tags = game.native_entity(id).and_then(|e| game.native_entity_tags(e)).unwrap_or_default();
                let p_ok = tags.iter().any(|t| t == "soulscraft.p=0.30");
                let pb = at(b);
                let drowned = game.mobs.iter().any(|m| {
                    !m.removed && m.kind.id == "minecraft:drowned" && (m.position.x - f64::from(pb[0]) - 0.5).abs() < 1.5 && (m.position.z - f64::from(pb[2]) - 0.5).abs() < 1.5
                });
                return Some(if kind == "soulscraft:lurker" && p_ok && c.count >= 900 && drowned {
                    Ok(())
                } else {
                    Err(format!("kind {kind}, P tag {p_ok}, {} ticks under water, the control zombie drowned {drowned}", c.count))
                });
            }
        }
        _ => {}
    }
    None
}

/// On Hard a hurt zombie may call another of its type (zombie_reinforcements.rs): SoulsCraft's
/// mobs have `spawn_reinforcements` 0, and a brute hit 40 times on Hard brings no second brute.
fn no_reinforcements(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let spot = [8, 0, 8];
    match t {
        1 => {
            c.note = match game.ui.difficulty {
                0 => "peaceful",
                1 => "easy",
                3 => "hard",
                _ => "normal",
            }
            .to_owned();
            game.native_run_command("difficulty hard");
        }
        3 => {
            c.golem = crate::game::soulscraft_tools::summon(game, DIM, "brute", centre(at(spot)), 0.0, 0);
            let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt("{NoAI:1b,Invulnerable:1b,PersistenceRequired:1b}") else { return None };
            let g = game.native_summon(DIM, "minecraft:iron_golem", centre(at([spot[0], spot[1], spot[2] + 3])), nbt);
            c.probe = vec![[g.map_or(-1, |g| g as i32), 0, 0]];
        }
        6 => {
            let reinf = game.mobs.iter().find(|m| Some(m.id) == c.golem)?.attrs.spawn_reinforcements;
            if reinf != 0.0 {
                return Some(Err(format!("spawn_reinforcements {reinf}")));
            }
        }
        7..=46 => {
            let id = c.golem?;
            let attacker = u32::try_from(c.probe.first()?[0]).ok();
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.invulnerable = 0;
            }
            game.native_hurt_entity(DIM, id, 0.5, "minecraft:mob_attack", attacker);
        }
        60 => {
            let brutes = game.mobs.iter().filter(|m| !m.removed && m.alive() && m.kind.id == "soulscraft:brute").count();
            let back = c.note.clone();
            game.native_run_command(&format!("difficulty {back}"));
            return Some(if brutes == 1 { Ok(()) } else { Err(format!("{brutes} brutes after 40 hits on Hard")) });
        }
        _ => {}
    }
    None
}

/// Round 10: a participant within 6 blocks but sealed in a stone box (no sight, no way round)
/// does not pull a zombie of the wave: it keeps going for the altar.
fn intercept_walled(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        build(game, 1);
        return None;
    }
    if t == 4 {
        level(game, now)?;
        c.golem = golem(game, [-20, 0, -20]);
        let ov = Overrides {
            waves: Some(vec![zombies(1)]),
            telegraph_ticks: Some(5),
            power: Some(0.0),
            curse_after_ticks: Some(10_000),
            stand_ins: c.golem.into_iter().collect(),
            ..Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    if t < 5 {
        return None;
    }
    let s = event::snapshot(DIM, CORE, None)?;
    let Some(&(zid, _, goal, _, _)) = s.mobs.first() else { return None };
    let g = c.golem?;
    let zpos = game.mobs.iter().find(|m| m.id == zid).map(|m| [m.position.x, m.position.y, m.position.z])?;
    match c.phase {
        0 => {
            // A sealed box 5 × 5 × 5 six blocks ahead of the zombie, on its way to the altar.
            let d = soulscraft_rules::geom::flat_dir(zpos, centre(CORE));
            let b = [(zpos[0] + d[0] * 6.0).floor() as i32, CORE[1], (zpos[2] + d[2] * 6.0).floor() as i32];
            game.native_run_command(&format!("fill {} {} {} {} {} {} stone hollow", b[0] - 2, b[1] - 1, b[2] - 2, b[0] + 2, b[1] + 3, b[2] + 2));
            c.probe = vec![b];
            c.count = s.builds;
            c.phase = 1;
            c.since = now;
        }
        // The participant goes in once the path map has the box (rebuilt at most a second after).
        1 if s.builds > c.count || now - c.since >= 60 => {
            let b = c.probe.first().copied()?;
            c.count = 0;
            game.native_teleport_mob(DIM, g, [f64::from(b[0]) + 0.5, f64::from(b[1]), f64::from(b[2]) + 0.5]);
            c.phase = 2;
            c.since = now;
        }
        2 => {
            let gpos = game.mobs.iter().find(|m| m.id == g).map(|m| [m.position.x, m.position.y, m.position.z])?;
            if goal == Goal::Player(g) {
                return Some(Err(format!("pulled through the wall from {:.1} blocks", soulscraft_rules::geom::dist(gpos, zpos))));
            }
            if soulscraft_rules::geom::dist(gpos, zpos) <= 6.0 {
                c.count += 1;
            }
            if now - c.since >= 40 {
                return Some(if c.count >= 10 { Ok(()) } else { Err(format!("within 6 blocks only {} ticks", c.count)) });
            }
        }
        _ => {}
    }
    None
}

/// Round 14 (4): the event leads its wave. Three wave zombies on a stand-in next to their spawns
/// for 60 ticks: none is adopted by the vanilla mind (no lunge, no flank, no cue), each is left alone.
fn wave_no_moves(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    const WATCH: u64 = 60;
    if t == 1 {
        build(game, 1);
        return None;
    }
    if t == 4 {
        level(game, now)?;
        c.golem = golem(game, STAND_IN);
        let ov = Overrides {
            waves: Some(vec![zombies(3)]),
            telegraph_ticks: Some(TEST_TELEGRAPH),
            rest_ticks: Some(TEST_REST),
            power: Some(0.0),
            stand_ins: c.golem.into_iter().collect(),
            ..Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    if t < 5 {
        return None;
    }
    let ids = event::wave_ids(DIM, CORE);
    if ids.is_empty() {
        return (c.since != 0).then(|| Err("the wave was gone before the watch ended".into()));
    }
    if c.since == 0 {
        c.since = now;
    }
    let v = crate::game::soulscraft_vanilla::state();
    let mine = |m: &u32| ids.contains(m);
    if let Some(id) = ids.iter().find(|id| v.mobs.contains_key(id)) {
        return Some(Err(format!("wave zombie {id} adopted by the vanilla mind")));
    }
    let moved = v.phases.iter().any(|r| mine(&r.mob)) || v.strikes.iter().any(|r| mine(&r.mob));
    let flanked = v.flanks.iter().any(|f| mine(&f.0)) || v.cues.iter().any(|q| mine(&q.0));
    if moved || flanked {
        return Some(Err(format!("wave zombies used vanilla moves: phases/strikes {moved}, flanks/cues {flanked}")));
    }
    if now - c.since < WATCH {
        return None;
    }
    let left = ids.iter().all(|id| v.skipped.contains(id));
    Some(if left && ids.len() == 3 { Ok(()) } else { Err(format!("{} zombies, all left alone {left}", ids.len())) })
}
