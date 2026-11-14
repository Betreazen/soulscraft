// The wave event on an altar, on the server (06-altar.md, «Ивент на алтаре» and «Оборона алтаря»;
// the decisions are soulscraft_rules::{event, defense, flow}): the run's phases, the spawn points
// shown before each wave, the participants in the 3 × 3-chunk zone, the altar's health, the path
// map of the zone and the wave mobs driven along it, their drain, curse and the brute's breach,
// the boss bar with its warning, the reward of a win and the walls put back at the end.
//
// Wave mobs are vanilla zombies (the test event) whose own goals are dropped when the event first
// sees them (`Mob.ai.goals`, `targets`), so this file walks them, and SoulsCraft's mobs, whose
// brain still fights a player they go for (soulscraft_server asks `steering`) while this file
// walks them to the altar. The path map is built from the world once at the start and again
// when a block in the zone changes (the block_changed hook, through soulscraft_altar), at most
// once a `rebuild_min_ticks`.
//
// Log lines:
//   soulscraft: event start core=-700,101,700 event=test_zombies level=1 power=0.00 banners=[] waves=5 hp=200
//   soulscraft: event telegraph wave=1 points=4 tick=..   soulscraft: event spawn wave=1 mobs=6 tick=..
//   soulscraft: event field cells=39168 open=1234 breach=2345 ms=3.1 tick=..
//   soulscraft: event drain mob=12 hp=199.95   soulscraft: event curse mob=12   soulscraft: event breach mob=12 at=x,y,z
//   soulscraft: event won|lost=altar|nobody|stopped core=.. wave=3 hp=..   soulscraft: event reward items=[..]
//   soulscraft: event restored blocks=4 kept=1
use crate::game::Game;
use crate::game::soulscraft_altar::{self as altar_srv, Altar};
use crate::game::soulscraft_server as server;
use soulscraft_rules::{
    altar as ra,
    data::Stats,
    defense::{self, AltarHealth, Breaches, Drain, Goal, Harm, Mind, Numbers, Role},
    event::{self as rev, Flow, Plan, Rng, Run, SpawnMob, Step},
    flow::Field,
    geom::{self, V3},
    scale::{self, Scaled},
    view::BeamKind,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
pub(super) use crate::game::soulscraft_event_ui::{Snapshot, menu_lines, snapshot, view, wave_ids};
use crate::game::soulscraft_event_ui::{show_bar, tell_zone, zone_selector};
use crate::game::soulscraft_event_path::{breach, build_field, sight};

/// The vanilla zombie's numbers (data/mobs.json `vanilla_zombie`).
const ZOMBIE: Stats = Stats { health: 20.0, damage: 3.0, armor: 2.0, speed: Some(0.23), knockback_resistance: 0.0, flying: false, water_only: false };
/// Wave mobs carry this tag.
pub(super) const TAG_WAVE: &str = "soulscraft.wave";
/// A walk is asked again this often.
const REPATH: u64 = 20;
/// A melee blow of a zombie this often.
const ZOMBIE_SWING: u64 = 20;
const MELEE_REACH: f64 = 1.7;
/// A ranged mob drains from this far.
const RANGED_DRAIN: f64 = 12.0;
/// How many cells along the way a walk aims at.
const LOOK_AHEAD: usize = 6;
/// Spawn points a wave comes from.
const SPAWN_POINTS: usize = 4;
const LOG_KEEP: usize = 256;

/// What the self-test changes in a run: shorter waits, a smaller altar, stand-in participants.
#[derive(Clone, Debug, Default)]
pub(super) struct Overrides {
    pub waves: Option<Vec<Vec<SpawnMob>>>,
    pub telegraph_ticks: Option<u64>,
    pub rest_ticks: Option<u64>,
    pub altar_health: Option<f64>,
    pub curse_after_ticks: Option<u64>,
    pub grace_ticks: Option<u64>,
    pub power: Option<f64>,
    /// The reward's random stream (a seeded run drops what soulscraft_rules::event::rewards rolls).
    pub seed: Option<u64>,
    /// Mobs (iron golems) that stand in for players: participants, and targets of interception.
    pub stand_ins: Vec<u32>,
}

/// How soulscraft_server drives one of SoulsCraft's mobs in a wave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Steering {
    /// The event walks it (to the altar, its drain, its curse, the brute's breach).
    Own,
    /// Its brain fights this entity (a participant).
    Fight(u32),
}

#[derive(Clone, Debug)]
pub(super) struct Breaching {
    pub(super) blocks: Vec<([i32; 3], u32)>,
    pub(super) swing_start: u64,
}

#[derive(Clone, Debug)]
pub(super) struct WaveMob {
    pub(super) kind: String,
    pub(super) role: Role,
    pub(super) soul: bool,
    pub(super) health_mult: f64,
    pub(super) mind: Mind,
    pub(super) adopted: bool,
    pub(super) nav: Option<([i32; 3], u64)>,
    pub(super) moving: bool,
    pub(super) swing: u64,
    pub(super) breach: Option<Breaching>,
    pub(super) beam: Option<BeamKind>,
    pub(super) pos: V3,
    pub(super) goal: Goal,
    pub(super) cursed_logged: bool,
    /// A short hop the path finder refused (a wide brute beside a wall): walked by velocity.
    pub(super) steer: Option<[i32; 3]>,
}

/// A participant this tick: a player or a stand-in.
#[derive(Clone, Debug)]
pub(super) struct Part {
    pub(super) entity: u32,
    pub(super) pos: V3,
    pub(super) player: Option<u128>,
}

pub(super) struct EventRun {
    pub dim: String,
    pub core: [i32; 3],
    pub event: String,
    pub(super) plan: Plan,
    pub(super) run: Run,
    pub(super) hp: AltarHealth,
    pub(super) numbers: Numbers,
    pub(super) flow: Flow,
    pub(super) mobs: BTreeMap<u32, WaveMob>,
    pub(super) field: Option<Field>,
    pub(super) dirty_at: Option<u64>,
    pub(super) built_at: u64,
    pub builds: u32,
    pub(super) spawns: Vec<[i32; 3]>,
    pub(super) joined: BTreeMap<u128, String>,
    pub(super) out: BTreeSet<u128>,
    pub(super) breaches: Breaches,
    pub(super) rng: Rng,
    /// The reward's own random stream, apart from the spawn points' (Overrides::seed).
    pub(super) reward_seed: u64,
    pub(super) stand_ins: Vec<u32>,
    pub(super) bar: String,
    pub(super) kinds: Vec<String>,
    pub(super) level: u32,
    pub(super) bar_shown: (i64, i64, String),
    pub(super) flash: bool,
    pub(super) parts_now: Vec<Part>,
}

pub(super) static EVENTS: LazyLock<Mutex<HashMap<(String, [i32; 3]), EventRun>>> = LazyLock::new(Mutex::default);
/// Wave mobs' ids, for the damage hook (no game there).
static WAVE_IDS: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Mutex::default);
/// Hits on wave mobs: (mob, attacker or 0).
static HITS: Mutex<Vec<(u32, i32)>> = Mutex::new(Vec::new());
static STEER: LazyLock<Mutex<HashMap<u32, Steering>>> = LazyLock::new(Mutex::default);
/// What the runs did, for the self-test and the log.
static RECORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static BARS: Mutex<u32> = Mutex::new(0);

pub(super) fn events() -> MutexGuard<'static, HashMap<(String, [i32; 3]), EventRun>> {
    EVENTS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn record(line: String) {
    log::info!("soulscraft: event {line}");
    let mut r = RECORDS.lock().unwrap_or_else(PoisonError::into_inner);
    r.push(line);
    let n = r.len();
    if n > LOG_KEEP {
        r.drain(..n - LOG_KEEP);
    }
}

pub(super) fn records() -> Vec<String> {
    RECORDS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub(super) fn clear_records() {
    RECORDS.lock().unwrap_or_else(PoisonError::into_inner).clear();
}

pub(super) fn running(dim: &str, core: [i32; 3]) -> bool {
    events().contains_key(&(dim.to_owned(), core))
}

pub(super) fn steering(mob: u32) -> Option<Steering> {
    STEER.lock().unwrap_or_else(PoisonError::into_inner).get(&mob).copied()
}

/// The damage hook: a hit on a wave mob (its attacker when the engine names it).
pub(super) fn on_damage(entity: i32, causing: i32) {
    if entity < 0 || !WAVE_IDS.lock().unwrap_or_else(PoisonError::into_inner).contains(&(entity as u32)) {
        return;
    }
    HITS.lock().unwrap_or_else(PoisonError::into_inner).push((entity as u32, causing));
}

pub(super) fn core_centre(core: [i32; 3]) -> V3 {
    [f64::from(core[0]) + 0.5, f64::from(core[1]) + 1.0, f64::from(core[2]) + 0.5]
}

pub(super) fn cell_of(p: V3) -> [i32; 3] {
    [p[0].floor() as i32, (p[1] + 0.01).floor() as i32, p[2].floor() as i32]
}

pub(super) fn centre_of(c: [i32; 3]) -> V3 {
    [f64::from(c[0]) + 0.5, f64::from(c[1]), f64::from(c[2]) + 0.5]
}

/// Starts `events[index]` on the altar. The pedestal's item is the caller's to take.
pub(super) fn start(game: &mut Game<'_>, dim: &str, a: &Altar, index: usize, now: u64, ov: Option<Overrides>) -> Result<(), String> {
    if running(dim, a.core) {
        return Err("an event runs here already".into());
    }
    let data = server::state().data.clone();
    let ev = data.events.get(index).ok_or("no such event")?.clone();
    let ov = ov.unwrap_or_default();
    let file = altar_srv::file();
    let banners = a.banner_ids();
    let effects = ra::effects(&file, &banners);
    let mut flow = Flow::of(&ev);
    if let Some(t) = ov.telegraph_ticks {
        flow.telegraph_ticks = t;
    }
    if let Some(t) = ov.grace_ticks {
        flow.participant_grace_ticks = t;
    }
    let mut numbers = Numbers::of(&ev);
    if let Some(t) = ov.curse_after_ticks {
        numbers.curse_after_ticks = t;
    }
    let parts = players_in_zone(game, dim, a.core, &flow, &BTreeSet::new());
    let power = match ov.power {
        Some(p) => p,
        None => {
            let near: Vec<(V3, u128)> = parts.iter().filter_map(|p| p.player.map(|u| (p.pos, u))).collect();
            if near.is_empty() { 0.0 } else { crate::game::soulscraft_power::power_here(game, core_centre(a.core), &near) }
        }
    };
    let mut plan = rev::plan(&ev, &data, &effects, power);
    if let Some(w) = ov.waves {
        plan.waves = w;
    }
    if let Some(r) = ov.rest_ticks {
        plan.rest_ticks = r;
    }
    if let Some(h) = ov.altar_health {
        plan.altar_health = h;
    }
    let (run, step) = Run::start(&plan, &flow, now);
    let bar = {
        let mut n = BARS.lock().unwrap_or_else(PoisonError::into_inner);
        *n += 1;
        format!("soulscraft:altar_{}", *n)
    };
    let kinds: Vec<String> = plan.waves.iter().flatten().map(|m| m.kind.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    let mut r = EventRun {
        dim: dim.to_owned(),
        core: a.core,
        event: ev.id.clone(),
        hp: AltarHealth::new(plan.altar_health),
        plan,
        run,
        numbers,
        flow,
        mobs: BTreeMap::new(),
        field: None,
        dirty_at: None,
        built_at: now,
        builds: 0,
        spawns: Vec::new(),
        joined: BTreeMap::new(),
        out: BTreeSet::new(),
        breaches: Breaches::default(),
        rng: Rng::new(now ^ (a.core[0] as u64).wrapping_mul(31) ^ (a.core[2] as u64).wrapping_mul(17)),
        reward_seed: ov.seed.unwrap_or(now.wrapping_mul(0x9E37_79B9) ^ (a.core[1] as u64)),
        stand_ins: ov.stand_ins,
        bar: bar.clone(),
        kinds,
        level: a.level,
        bar_shown: (-1, -1, String::new()),
        flash: false,
        parts_now: Vec::new(),
    };
    for p in &parts {
        if let Some(u) = p.player {
            r.joined.insert(u, String::new());
        }
    }
    build_field(game, &mut r, now);
    altar_srv::run(game, dim, &format!("bossbar add {bar} \"Оборона алтаря\""));
    altar_srv::run(game, dim, &format!("bossbar set {bar} color yellow"));
    altar_srv::light_braziers(game, a.core, true);
    record(format!(
        "start core={},{},{} event={} level={} power={:.2} banners=[{}] waves={} hp={} tick={now}",
        a.core[0],
        a.core[1],
        a.core[2],
        ev.id,
        a.level,
        r.plan.power,
        banners.join(","),
        r.plan.waves.len(),
        r.plan.altar_health
    ));
    act(game, &mut r, step, now);
    events().insert((dim.to_owned(), a.core), r);
    Ok(())
}

/// Stops a run (an operator, the self-test): lost, walls back.
pub(super) fn stop(game: &mut Game<'_>, dim: &str, core: [i32; 3], now: u64) -> bool {
    let Some(mut r) = events().remove(&(dim.to_owned(), core)) else { return false };
    if let Some(step) = r.run.stop() {
        act(game, &mut r, step, now);
    }
    true
}

/// The core was broken: its event is lost.
pub(super) fn core_gone(game: &mut Game<'_>, dim: &str, core: [i32; 3], now: u64) {
    let Some(mut r) = events().remove(&(dim.to_owned(), core)) else { return };
    r.hp.hp = 0.0;
    if let Some(step) = r.run.step(now, 0, 1, true) {
        act(game, &mut r, step, now);
    }
}

/// A block in a zone changed: the path map is built again soon, and a place the brute broke
/// that a player built on stays the player's.
pub(super) fn on_block_change(dim: &str, pos: [i32; 3], now_air: bool, now: u64) {
    let mut ev = events();
    for r in ev.values_mut().filter(|r| r.dim == dim) {
        if !r.flow.in_zone(r.core, f64::from(pos[0]) + 0.5, f64::from(pos[2]) + 0.5) {
            continue;
        }
        if r.dirty_at.is_none() {
            r.dirty_at = Some(now);
        }
        r.breaches.changed(pos, now_air);
    }
}

/// A core seen again after a restart whose event was cut off: its broken walls come back.
pub(super) fn restore_orphans(game: &mut Game<'_>, dim: &str, core: [i32; 3]) {
    if running(dim, core) {
        return;
    }
    let lines = altar_srv::core_lines(game, core, altar_srv::BREACHED_KEY);
    if lines.is_empty() {
        return;
    }
    let mut b = Breaches::from_lines(lines.iter().map(String::as_str));
    restore(game, dim, &mut b);
    altar_srv::set_core_lines(game, core, altar_srv::BREACHED_KEY, &[]);
}

pub(super) fn tick(game: &mut Game<'_>, dim: &str, now: u64) {
    let keys: Vec<(String, [i32; 3])> = events().keys().filter(|k| k.0 == dim).cloned().collect();
    let hits: Vec<(u32, i32)> = std::mem::take(&mut *HITS.lock().unwrap_or_else(PoisonError::into_inner));
    for key in keys {
        let Some(mut r) = events().remove(&key) else { continue };
        tick_run(game, &mut r, now, &hits);
        if !r.run.over() {
            events().insert(key, r);
        }
    }
}

fn tick_run(game: &mut Game<'_>, r: &mut EventRun, now: u64, hits: &[(u32, i32)]) {
    let dim = r.dim.clone();
    // Participants: living players in the zone who have not died in this event, and stand-ins.
    let players = game.native_server_players();
    for p in &players {
        if p.health <= 0.0 && r.joined.contains_key(&p.uuid) && r.out.insert(p.uuid) {
            record(format!("out player={} tick={now}", p.name));
        }
    }
    let mut parts = players_in_zone(game, &dim, r.core, &r.flow, &r.out);
    for p in &parts {
        if let Some(u) = p.player
            && let Some(pl) = players.iter().find(|x| x.uuid == u)
        {
            r.joined.insert(u, pl.name.clone());
        }
    }
    for id in &r.stand_ins {
        if let Some(m) = game.mobs.iter().find(|m| m.id == *id && !m.removed && m.alive()) {
            let pos = [m.position.x, m.position.y, m.position.z];
            if r.flow.in_zone(r.core, pos[0], pos[2]) {
                parts.push(Part { entity: *id, pos, player: None });
            }
        }
    }
    r.parts_now = parts.clone();
    if r.dirty_at.is_some_and(|_| now >= r.built_at + r.flow.rebuild_min_ticks) {
        build_field(game, r, now);
    }
    for (mob, by) in hits {
        if let Some(w) = r.mobs.get_mut(mob) {
            let pos = w.pos;
            let from = if *by > 0 && parts.iter().any(|p| p.entity == *by as u32) {
                Some(*by as u32)
            } else {
                parts
                    .iter()
                    .filter(|p| geom::dist(p.pos, pos) <= r.numbers.intercept_blocks)
                    .min_by(|a, b| geom::dist(a.pos, pos).total_cmp(&geom::dist(b.pos, pos)))
                    .map(|p| p.entity)
            };
            match from {
                Some(e) => defense::on_hit(&mut w.mind, e, now),
                None => w.mind.drain_start = None,
            }
        }
    }
    drive_mobs(game, r, &parts, now);
    let alive = r.mobs.len();
    let fallen = r.hp.fallen();
    if let Some(step) = r.run.step(now, alive, parts.len(), fallen) {
        act(game, r, step, now);
    }
    if !r.run.over() {
        show_bar(game, r, now);
    }
}

fn players_in_zone(game: &Game<'_>, dim: &str, core: [i32; 3], flow: &Flow, out: &BTreeSet<u128>) -> Vec<Part> {
    game.native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dim && p.health > 0.0 && p.game_mode != 3 && !out.contains(&p.uuid))
        .filter(|p| flow.in_zone(core, p.position[0], p.position[2]) && (p.position[1] - f64::from(core[1])).abs() <= 48.0)
        .map(|p| Part { entity: p.entity_id as u32, pos: p.position, player: Some(p.uuid) })
        .collect()
}

fn drive_mobs(game: &mut Game<'_>, r: &mut EventRun, parts: &[Part], now: u64) {
    let dim = r.dim.clone();
    let ids: Vec<u32> = r.mobs.keys().copied().collect();
    let core_c = core_centre(r.core);
    for id in ids {
        let Some((pos, alive)) = game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| ([m.position.x, m.position.y, m.position.z], m.alive())) else {
            gone(r, id);
            continue;
        };
        if !alive {
            gone(r, id);
            continue;
        }
        let Some(mut w) = r.mobs.remove(&id) else { continue };
        w.pos = pos;
        if !w.adopted {
            adopt_zombie(game, id, &w, r.plan.power);
            w.adopted = true;
        }
        let eye = geom::add(pos, [0.0, 1.6, 0.0]);
        let seen: Vec<defense::Seen> = parts
            .iter()
            .map(|p| defense::Seen {
                entity: p.entity,
                pos: p.pos,
                visible: sight(r.field.as_ref(), eye, geom::add(p.pos, [0.0, 1.5, 0.0])),
                reachable: walkable(r.field.as_ref(), pos) && walkable(r.field.as_ref(), p.pos),
            })
            .collect();
        let altar_visible = sight(r.field.as_ref(), eye, core_c);
        let goal = if w.mind.cursed {
            Goal::Altar
        } else {
            defense::choose(w.role, &r.numbers, pos, core_c, altar_visible, &seen, &mut w.mind, now)
        };
        if goal != w.goal {
            w.nav = None;
            w.goal = goal;
        }
        match goal {
            Goal::Player(e) => {
                w.beam = None;
                w.breach = None;
                w.mind.drain_start = None;
                if w.soul {
                    STEER.lock().unwrap_or_else(PoisonError::into_inner).insert(id, Steering::Fight(e));
                } else if let Some(p) = parts.iter().find(|p| p.entity == e) {
                    chase(game, &dim, id, &mut w, p, r.plan.power, now);
                }
            }
            Goal::Altar => {
                if w.soul {
                    STEER.lock().unwrap_or_else(PoisonError::into_inner).insert(id, Steering::Own);
                }
                to_altar(game, r, id, &mut w, altar_visible, now);
            }
        }
        r.mobs.insert(id, w);
    }
}

fn gone(r: &mut EventRun, id: u32) {
    r.mobs.remove(&id);
    WAVE_IDS.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
    STEER.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
}

/// A vanilla zombie of the wave the first time it is seen: its own goals dropped (the event walks
/// and swings it), no burning in daylight, its numbers from the event's P, and Path-of-craft-tree's
/// experience tag (Г3).
fn adopt_zombie(game: &mut Game<'_>, id: u32, w: &WaveMob, power: f64) {
    if w.soul {
        return;
    }
    let sc = Scaled::at(&ZOMBIE, power);
    let max = sc.max_health * w.health_mult;
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
        m.ai.goals.clear();
        m.ai.targets.clear();
        m.ai.goals.add(0, horizon_client::mobs::goals::Float);
        if m.kind.flags.burns_in_day {
            std::sync::Arc::make_mut(&mut m.kind).flags.burns_in_day = false;
        }
        m.attrs.max_health = max as f32;
        m.health = m.attrs.max_health;
        m.attrs.attack_damage = sc.damage as f32;
        m.needs_sync = true;
    }
    server::set_xp_tag(game, id, ZOMBIE.health, max);
}

pub(super) fn nav_to(game: &mut Game<'_>, dim: &str, id: u32, w: &mut WaveMob, to: [i32; 3], now: u64) {
    let due = w.nav.is_none_or(|(c, t)| c != to || now - t >= REPATH);
    if due {
        let ok = game.native_mob_navigate(dim, id, centre_of(to), 1, 1.0);
        if now % 100 < REPATH {
            log::debug!("soulscraft: event nav mob={id} at={:.1},{:.1},{:.1} to={},{},{} ok={ok} tick={now}", w.pos[0], w.pos[1], w.pos[2], to[0], to[1], to[2]);
        }
        w.steer = (!ok).then_some(to);
        w.nav = Some((to, now));
        w.moving = true;
    }
    if let Some(t) = w.steer {
        push(game, dim, id, w.pos, centre_of(t));
    }
}

/// The engine's path finder gives no path for a hop shorter than its reach, or for a mob wider than
/// one block beside a wall: such a hop is walked by setting the mob's velocity, as the server half
/// steers its flyers.
fn push(game: &mut Game<'_>, dim: &str, id: u32, from: V3, to: V3) {
    const STEP: f64 = 0.12;
    if geom::flat_dist(from, to) < 0.3 {
        return;
    }
    let d = geom::flat_dir(from, to);
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
        m.velocity.x = d[0] * STEP;
        m.velocity.z = d[2] * STEP;
        m.needs_sync = true;
    }
    game.native_mob_look_at(dim, id, [to[0], from[1] + 1.5, to[2]]);
}

pub(super) fn halt(game: &mut Game<'_>, dim: &str, id: u32, w: &mut WaveMob, look: V3) {
    w.steer = None;
    if w.moving {
        game.native_mob_stop(dim, id);
        w.moving = false;
        w.nav = None;
    }
    game.native_mob_look_at(dim, id, look);
}

/// A zombie after a participant: it walks to them and swings once a second in reach.
fn chase(game: &mut Game<'_>, dim: &str, id: u32, w: &mut WaveMob, p: &Part, power: f64, now: u64) {
    w.beam = None;
    let d = geom::flat_dist(w.pos, p.pos);
    if d <= MELEE_REACH && (w.pos[1] - p.pos[1]).abs() < 2.0 {
        halt(game, dim, id, w, geom::add(p.pos, [0.0, 1.5, 0.0]));
        if now >= w.swing {
            let dmg = Scaled::at(&ZOMBIE, power).damage;
            game.native_hurt_entity(dim, p.entity, dmg as f32, "minecraft:mob_attack", Some(id));
            w.swing = now + ZOMBIE_SWING;
        }
    } else {
        nav_to(game, dim, id, w, cell_of(p.pos), now);
    }
}

/// A mob going for the altar: along the path map, then the drain with its windup; walled out
/// for long it curses the altar from where it stands; the brute breaks a way through.
fn to_altar(game: &mut Game<'_>, r: &mut EventRun, id: u32, w: &mut WaveMob, altar_visible: bool, now: u64) {
    let dim = r.dim.clone();
    let core_c = core_centre(r.core);
    let flat = geom::flat_dist(w.pos, core_c);
    let dy = w.pos[1] - f64::from(r.core[1]);
    // A mob's centre is up to a block from the seed cell it stands in: a cell next to the altar
    // (step 0 of the map) counts too.
    let range = if w.role == Role::Ranged { RANGED_DRAIN } else { r.flow.drain_blocks + 1.0 };
    let cell = r.field.as_ref().and_then(|f| f.locate(cell_of(w.pos)));
    let open = cell.and_then(|c| r.field.as_ref().and_then(|f| f.open_at(c)));
    let at_altar = (flat <= range && dy.abs() <= 3.0 && (w.role != Role::Ranged || altar_visible))
        || (w.role != Role::Ranged && open == Some(0));
    let breaker = w.kind == "soulscraft:brute" && cell.and_then(|c| r.field.as_ref().and_then(|f| f.breach_at(c))).is_some();
    let has_path = at_altar || open.is_some() || r.field.is_none() || w.breach.is_some() || breaker;
    if now % 100 == 0 {
        log::debug!("soulscraft: event mob={id} at={:.1},{:.1},{:.1} cell={cell:?} open={open:?} breaker={breaker} at_altar={at_altar} tick={now}", w.pos[0], w.pos[1], w.pos[2]);
    }
    if defense::track_path(&mut w.mind, has_path, &r.numbers, now) {
        if !w.cursed_logged {
            w.cursed_logged = true;
            record(format!("curse mob={id} tick={now}"));
        }
        halt(game, &dim, id, w, core_c);
        w.beam = Some(BeamKind::Curse);
        r.hp.hurt(r.numbers.curse_per_second / 20.0, Harm::Curse, r.numbers.cap_per_second, now);
        return;
    }
    match defense::drain(&mut w.mind, at_altar, &r.numbers, now) {
        Drain::Windup(_) => {
            halt(game, &dim, id, w, core_c);
            w.beam = Some(BeamKind::Windup);
        }
        Drain::Draining => {
            halt(game, &dim, id, w, core_c);
            if w.beam != Some(BeamKind::Drain) {
                record(format!("drain mob={id} tick={now}"));
            }
            w.beam = Some(BeamKind::Drain);
            r.hp.hurt(r.numbers.drain_per_second / 20.0, Harm::Drain, r.numbers.cap_per_second, now);
        }
        Drain::Away => {
            w.beam = None;
            let Some(field) = r.field.as_ref() else {
                nav_to(game, &dim, id, w, cell_of(core_c), now);
                return;
            };
            if let (Some(c), Some(_)) = (cell, open) {
                w.breach = None;
                let to = field.ahead(c, LOOK_AHEAD).last().copied().unwrap_or(c);
                nav_to(game, &dim, id, w, to, now);
                return;
            }
            if w.kind == "soulscraft:brute" && breach(game, r, id, w, cell, now) {
                return;
            }
            nav_to(game, &dim, id, w, cell_of(core_c), now);
        }
    }
}

fn act(game: &mut Game<'_>, r: &mut EventRun, step: Step, now: u64) {
    match step {
        Step::Telegraph(w) => telegraph(game, r, w, now),
        Step::Spawn(w) => spawn(game, r, w, now),
        Step::Rest(next) => {
            record(format!("rest next={} ticks={} tick={now}", next + 1, r.plan.rest_ticks));
            tell_zone(game, r, "Передышка", &format!("Волна {} из {} скоро", next + 1, r.plan.waves.len()));
        }
        Step::Won => finish(game, r, true, "won", now),
        Step::Lost(l) => finish(game, r, false, l.name(), now),
    }
}

/// Where wave `w` comes from: up to four points spread round the altar, reachable on the path
/// map, with room for the wave's tallest mob (another point when a brute's has no headroom).
fn telegraph(game: &mut Game<'_>, r: &mut EventRun, w: usize, now: u64) {
    if r.field.is_none() || r.dirty_at.is_some() {
        build_field(game, r, now);
    }
    let height = r.plan.waves.get(w).map_or(2, |m| m.iter().map(|x| x.height).max().unwrap_or(2));
    let count = r.plan.waves.get(w).map_or(0, Vec::len).clamp(1, SPAWN_POINTS);
    let mut points = Vec::new();
    if let Some(f) = r.field.as_ref() {
        let cells: Vec<[i32; 3]> = f
            .spawn_cells(r.core, r.flow.spawn_near, r.flow.spawn_far)
            .into_iter()
            .filter(|c| f.grid.headroom(*c, height))
            .collect();
        let seed = r.rng.next_u64();
        points = soulscraft_rules::flow::pick_spread(&cells, r.core, count, seed);
    }
    if points.is_empty() {
        // No reachable ground in the band (a walled altar): the ring at the near edge.
        for i in 0..count {
            let a = i as f64 * std::f64::consts::TAU / count as f64;
            let p = geom::ring_point(core_centre(r.core), r.flow.spawn_near, a);
            points.push([p[0].floor() as i32, r.core[1], p[2].floor() as i32]);
        }
    }
    record(format!("telegraph wave={} points={} tick={now}", w + 1, points.len()));
    r.spawns = points;
    tell_zone(game, r, &format!("Волна {}", w + 1), "Враги идут к алтарю");
    altar_srv::run(game, &r.dim, &format!("playsound minecraft:event.raid.horn hostile {} 1 0.8", zone_selector(r)));
}

fn spawn(game: &mut Game<'_>, r: &mut EventRun, w: usize, now: u64) {
    let dim = r.dim.clone();
    let wave = r.plan.waves.get(w).cloned().unwrap_or_default();
    let points = if r.spawns.is_empty() { vec![r.core] } else { r.spawns.clone() };
    let mut n = 0;
    for (i, m) in wave.iter().enumerate() {
        let p = points[i % points.len()];
        let k = (i / points.len()) as f64;
        let jitter = [0.5 + 0.3 * (k * 2.1).sin(), 0.0, 0.5 + 0.3 * (k * 2.1).cos()];
        let at = [f64::from(p[0]) + jitter[0], f64::from(p[1]), f64::from(p[2]) + jitter[2]];
        let soul = m.kind.starts_with("soulscraft:");
        let id = spawn_one(game, &dim, m, at, r.plan.power);
        let Some(id) = id else { continue };
        n += 1;
        WAVE_IDS.lock().unwrap_or_else(PoisonError::into_inner).insert(id);
        r.mobs.insert(
            id,
            WaveMob {
                kind: m.kind.clone(),
                role: m.role,
                soul,
                health_mult: m.health_mult,
                mind: Mind::default(),
                adopted: soul,
                nav: None,
                moving: false,
                swing: 0,
                breach: None,
                beam: None,
                pos: at,
                goal: Goal::Altar,
                cursed_logged: false,
                steer: None,
            },
        );
    }
    r.spawns.clear();
    record(format!("spawn wave={} mobs={n} tick={now}", w + 1));
}

/// One wave mob at `at`, its spot moved to where its box fits (round 9: silently; none when
/// nothing within the search radius fits).
pub(super) fn spawn_one(game: &mut Game<'_>, dim: &str, m: &SpawnMob, at: V3, power: f64) -> Option<u32> {
    if let Some(short) = m.kind.strip_prefix("soulscraft:") {
        let id = crate::game::soulscraft_tools::summon(game, dim, short, at, power, 0)?;
        if let Some(e) = game.native_entity(id) {
            game.native_entity_add_tag(e, TAG_WAVE);
        }
        return Some(id);
    }
    let Some((spot, _)) = crate::game::soulscraft_tools::fit_spot(game, &m.kind, 0, at) else {
        log::info!("soulscraft: event spawn skipped kind={} at={:.1},{:.1},{:.1}: no room", m.kind, at[0], at[1], at[2]);
        return None;
    };
    summon_zombie(game, dim, m, spot)
}

fn summon_zombie(game: &mut Game<'_>, dim: &str, m: &SpawnMob, at: V3) -> Option<u32> {
    let mut gear = String::new();
    if m.armored {
        gear = "equipment:{head:{id:\"minecraft:iron_helmet\",count:1},chest:{id:\"minecraft:iron_chestplate\",count:1}},drop_chances:{head:0f,chest:0f},".into();
    }
    if m.banner {
        gear = "equipment:{head:{id:\"minecraft:black_banner\",count:1}},drop_chances:{head:0f},".into();
    }
    let snbt = format!("{{{gear}Tags:[\"{TAG_WAVE}\"],PersistenceRequired:1b,CanPickUpLoot:0b}}");
    let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&snbt) else { return None };
    game.native_summon(dim, &m.kind, at, nbt)
}

/// The end, won or lost: the reward of a win, the wave's mobs gone, the walls back, the
/// braziers out, the bar away.
fn finish(game: &mut Game<'_>, r: &mut EventRun, won: bool, how: &str, now: u64) {
    let dim = r.dim.clone();
    record(format!(
        "{} core={},{},{} wave={} hp={:.2} drained={:.2} cursed={:.2} parts={} tick={now}",
        if won { "won".to_owned() } else { format!("lost={how}") },
        r.core[0],
        r.core[1],
        r.core[2],
        r.run.wave().min(r.plan.waves.len()),
        r.hp.hp,
        r.hp.by_drain,
        r.hp.by_curse,
        r.joined.len() + r.stand_ins.len()
    ));
    if won {
        reward(game, r, now);
        tell_zone(game, r, "Алтарь выстоял", "Победа");
    } else {
        let why = match how {
            "altar" => "Алтарь пал",
            "nobody" => "В зоне никого не осталось",
            _ => "Ивент остановлен",
        };
        tell_zone(game, r, "Поражение", why);
    }
    let ids: Vec<u32> = r.mobs.keys().copied().collect();
    for id in ids {
        game.native_remove_mob(&dim, id);
        gone(r, id);
    }
    restore(game, &dim, &mut r.breaches);
    altar_srv::set_core_lines(game, r.core, altar_srv::BREACHED_KEY, &[]);
    altar_srv::light_braziers(game, r.core, false);
    altar_srv::run(game, &dim, &format!("bossbar remove {}", r.bar));
}

/// The broken blocks back where nothing stands now; places a player built on keep theirs.
fn restore(game: &mut Game<'_>, dim: &str, b: &mut Breaches) {
    let kept = b.list.iter().filter(|x| x.overwritten).count();
    let back = b.take_restore();
    let mut put = 0;
    for x in &back {
        let now_there = game.native_block(x.pos);
        if now_there == "minecraft:air" || now_there == "minecraft:cave_air" {
            if altar_srv::setblock(game, dim, x.pos, &x.state, "replace") {
                put += 1;
            }
        }
    }
    if !back.is_empty() || kept > 0 {
        record(format!("restored blocks={put} kept={kept}"));
    }
}

/// The win's reward (09-trophies.md, data/trophies.json; soulscraft_rules::event::rewards with the
/// banners the run started with), dropped over the core: heads, armour pieces, a diorama.
fn reward(game: &mut Game<'_>, r: &mut EventRun, now: u64) {
    let chances = rev::TrophyChances::embedded();
    let data = server::state().data.clone();
    let trophies = rev::rewards(&r.kinds, &data, &r.plan.effects, r.level, &chances, &mut Rng::new(r.reward_seed));
    let mut items = Vec::new();
    for t in &trophies {
        let Some(item) = t.item() else { continue };
        let c = core_centre(r.core);
        let at = [c[0], c[1] + 1.2, c[2]];
        let snbt = format!("{{Item:{{id:\"{item}\",count:1}},PickupDelay:10}}");
        if let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&snbt) {
            game.native_summon(&r.dim, "minecraft:item", at, nbt);
        }
        items.push(item);
    }
    record(format!(
        "reward items=[{}] mult={:.2} trophies={trophies:?} tick={now}",
        items.join(","),
        chances.multiplier(&r.plan.effects)
    ));
}


/// On the zone's path map with a walkable way to the altar (no map yet: taken as walkable). Two
/// such places are joined through the altar: the round-10 test for «a path to the participant».
fn walkable(field: Option<&Field>, at: V3) -> bool {
    field.is_none_or(|f| f.locate(cell_of(at)).and_then(|c| f.open_at(c)).is_some())
}
