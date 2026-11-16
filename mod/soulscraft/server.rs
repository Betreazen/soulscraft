// SoulsCraft's server half: every SoulsCraft mob of a level is found after the level's tick
// (`native_level_tick_post`, game/levels.rs:559-561), given its numbers from P once, and driven
// from its brain (soulscraft_rules::brain): the brain's orders become the engine's navigation,
// look and velocity, its blows become damage (`native_hurt_entity`, game/native_entities.rs:375,
// armour, shields and the damage hooks included), and the clip each mob shows is sent to the
// players of the level as mod messages (soulscraft_rules::wire, API 40). The zombie template's
// own goals are dropped in `register_goals` (hook_register_goals.rs), so nothing else attacks.
//
// What survives: a mob's P (and the slime's size) is an entity tag, `soulscraft.p=0.40`, which is
// saved with the mob (`Entity.getTags`, game/native_entity_data.rs:55); everything else (its
// brain, its attack in progress) is memory and starts over when its chunk comes back, where the
// stats are set again from the tag (prototypes/p3-stats.md).
//
// Log lines (one per event, for the bot scripts: prototypes/lib/log.ps1 reads them):
//   soulscraft: phase mob=revenant id=12 attack=overhead phase=hold t=10 tick=4567
//   soulscraft: strike mob=revenant id=12 attack=overhead t=18 tick=4575 hit=true dmg=5.20 target=sc_a
use crate::game::Game;
use crate::game::native_player_state::NativeEffect;
use crate::mod_api::NativeServerPlayer;
use horizon_client::particles::ParticleOptions;
use soulscraft_rules::{
    attack::{Phase, Timeline, wave_center, wave_hits},
    brain::{self, Brain, DiveTurns, Event, Order, Senses, Strike, Style, TargetSense, Tick},
    data::{MobDef, MobsFile},
    geom::{self, V3},
    scale::{self, Scaled},
    vanilla::Cue,
    wire,
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The file `/sc reload` reads, in the server's `config/` (Game::native_config_dir).
pub(super) const RELOAD_FILE: &str = "soulscraft/mobs.json";
const TAG_P: &str = "soulscraft.p=";
const TAG_SIZE: &str = "soulscraft.size=";
/// A spawn by /sc or a wave: health starts full at the scaled maximum.
pub(super) const TAG_FRESH: &str = "soulscraft.fresh";
/// Players farther than this are not hunted.
const FOLLOW: f64 = 24.0;
/// A walk is asked a new path at most this often unless the brain decides.
const REPATH_EVERY: u64 = 10;
/// Every client gets every mob's clip again this often (new viewers, lost notes).
const RESEND_EVERY: u64 = 40;
/// A tracked mob not seen for this long left (its chunk unloaded): forgotten.
const FORGET_AFTER: u64 = 40;
const LOG_KEEP: usize = 512;
/// The shared bleed (integration.md 4.1).
pub(super) const BLEEDING: &str = "soulscraft:bleeding";
/// `SoundSource.HOSTILE`'s index, the source a cue sounds from.
const HOSTILE: usize = 5;
/// What a shield-breaking blow holds for its instant (`shield_disable_ticks`): the engine disables a
/// blocking player's shield only for a mob whose main hand item has `disable_blocking_for_seconds`
/// (shield.rs:200-208, 242-283), and an axe's 5 s are the data's 100 ticks.
const SHIELD_BREAKER: &str = "minecraft:iron_axe";

/// One SoulsCraft mob the server drives.
pub(super) struct Tracked {
    pub kind: usize,
    pub p: f64,
    pub size: u8,
    pub brain: Brain,
    pub dim: String,
    pub seen: u64,
    pub pos: V3,
    pub yaw: f64,
    pub guarding: bool,
    pub alive: bool,
    sent: Option<(String, i64, u8)>,
    path: Option<(V3, u64)>,
    stopped: bool,
    /// The self-test's target (a mob), instead of the nearest player.
    pub target_override: Option<u32>,
    /// The self-test's target holds a shield up.
    pub test_shield: bool,
}

/// A blow as the log and the self-test see it.
#[derive(Clone, Debug)]
pub(super) struct StrikeRecord {
    pub mob: u32,
    pub attack: String,
    pub t: u32,
    pub tick: u64,
    pub hit: bool,
    pub damage: f64,
}

#[derive(Clone, Debug)]
pub(super) struct PhaseRecord {
    pub mob: u32,
    pub attack: String,
    pub phase: Phase,
    pub t: u32,
    pub tick: u64,
}

pub(super) struct State {
    pub data: MobsFile,
    pub source: String,
    pub mobs: HashMap<u32, Tracked>,
    pub p_override: Option<f64>,
    pub debug: bool,
    pub strikes: Vec<StrikeRecord>,
    pub phases: Vec<PhaseRecord>,
    pub modes: Vec<(u32, String, u64)>,
    /// Bleed hits due, from the effect tick (which may not change the game): (dimension, mob
    /// or player, amplifier).
    pub bleeds: Vec<(String, Option<i32>, Option<u128>, i32)>,
    /// The shrikes' dive turns: one dives at a target at a time (soulscraft_rules::brain::DiveTurns).
    pub turns: DiveTurns,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| {
    let (data, source) = match read_reload_file() {
        Some(Ok(d)) => (d, "config".to_owned()),
        Some(Err(e)) => {
            log::warn!("soulscraft: {e}; using the built-in numbers");
            (MobsFile::embedded(), "built-in".to_owned())
        }
        None => (MobsFile::embedded(), "built-in".to_owned()),
    };
    log::info!("soulscraft: {} mobs from the {source} mobs.json", data.mobs.len());
    Mutex::new(State {
        data,
        source,
        mobs: HashMap::new(),
        p_override: None,
        debug: false,
        strikes: Vec::new(),
        phases: Vec::new(),
        modes: Vec::new(),
        bleeds: Vec::new(),
        turns: DiveTurns::default(),
    })
});

/// What the damage hook (no game there: native_hooks.rs:9237) needs: guarding revenants and
/// where the players stand, as of the last tick.
#[derive(Default)]
struct Guards {
    shields: HashMap<i32, (V3, f64, f64)>,
    players: Vec<(i32, V3)>,
}
static GUARDS: LazyLock<Mutex<Guards>> = LazyLock::new(Mutex::default);

/// A lurker's grab as the server keeps it: the one it grabbed (its target stays that one), where
/// it holds it, the blows that one dealt it since the lurker's last tick and its health then.
struct Grab {
    prey: u32,
    player: bool,
    at: V3,
    blows: u32,
    health: f64,
}

/// Grabbing lurkers by entity id, for the damage hook too (it has no game).
static GRABS: LazyLock<Mutex<HashMap<i32, Grab>>> = LazyLock::new(Mutex::default);

/// The lurker's tick: its grab begins (mode `grab`: the target is the one it bit), goes on (`hold`)
/// or ends.
fn note_grab(m: &Seen, tick: &Tick, target: Option<&Target>) {
    let mut grabs = GRABS.lock().unwrap_or_else(PoisonError::into_inner);
    let key = m.id as i32;
    match (tick.hold, target) {
        (Some(_), Some(tg)) if tick.events.contains(&Event::Mode("grab")) => {
            grabs.insert(key, Grab { prey: tg.entity, player: tg.player.is_some(), at: m.pos, blows: 0, health: m.health });
        }
        (Some(_), _) => {
            if let Some(g) = grabs.get_mut(&key) {
                g.at = m.pos;
            }
        }
        (None, _) => {
            grabs.remove(&key);
        }
    }
}

/// The grabbed one's blows on lurker `id` since its last tick, for its brain (`Senses::blows`): at
/// most one a tick, and only one that took health (a blow in the hurt cooldown is no blow).
fn take_blows(id: u32, health: f64) -> u32 {
    let mut grabs = GRABS.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(g) = grabs.get_mut(&(id as i32)) else { return 0 };
    let landed = g.blows > 0 && health < g.health;
    g.blows = 0;
    g.health = health;
    u32::from(landed)
}

/// The damage hook on a grabbing lurker: a blow of the one it holds. A mob's blow names its
/// attacker (`causing`); a player's does not in hook v1 (`mob_source`, game/native_hooks.rs:
/// 9768-9772: `Who::Player` gives 0; v3, which names players, is only called for trait mods,
/// 9676-9680), so a player's melee hit (`minecraft:player_attack`) counts when the grabbed player is
/// the nearest one, which a player in its mouth is. Poison, bleeding, turrets and arrows do not.
fn note_grab_blow(entity: i32, damage_type: &str, causing: i32) {
    let mut grabs = GRABS.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(g) = grabs.get_mut(&entity) else { return };
    let by_prey = if causing != 0 {
        causing as u32 == g.prey
    } else if g.player && damage_type == "minecraft:player_attack" {
        let guards = GUARDS.lock().unwrap_or_else(PoisonError::into_inner);
        guards
            .players
            .iter()
            .min_by(|a, b| geom::dist(a.1, g.at).total_cmp(&geom::dist(b.1, g.at)))
            .is_some_and(|(id, _)| *id as u32 == g.prey)
    } else {
        false
    };
    if by_prey {
        g.blows += 1;
    }
}

pub(super) fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `/sc log on`: the per-mob lines at `info`, for bench checks and bot scripts that read the log.
static CHATTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn set_chatty(on: bool) {
    CHATTY.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The level of a line logged per mob or per few ticks (adoption, phases, blows, notes,
/// particles): `debug` unless `/sc log on`. A live server spawns mobs by the second, and the owner
/// asked for a quiet log (tidewill, 2026-11-15).
pub(super) fn chatty() -> log::Level {
    if CHATTY.load(std::sync::atomic::Ordering::Relaxed) { log::Level::Info } else { log::Level::Debug }
}

fn read_reload_file() -> Option<Result<MobsFile, String>> {
    let dir = Game::native_config_dir().ok()?;
    let path = dir.join(RELOAD_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    Some(MobsFile::parse(&text).map_err(|e| format!("{}: {e}", path.display())))
}

/// `/sc reload`: the numbers and timings from config/soulscraft/mobs.json, live. Mobs keep their
/// brains; a new attack takes the new ticks.
pub(super) fn reload() -> Result<String, String> {
    match read_reload_file() {
        None => Err(format!("no config/{RELOAD_FILE} (prototypes/tune.ps1 puts data/mobs.json there)")),
        Some(Err(e)) => Err(e),
        Some(Ok(d)) => {
            let mut s = state();
            let same = s.data.mobs.iter().map(|m| &m.id).eq(d.mobs.iter().map(|m| &m.id));
            if !same {
                return Err("the mob list changed: restart the server for new mobs".into());
            }
            s.data = d;
            s.source = "config".into();
            DATA_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let respec: Vec<u32> = s.mobs.keys().copied().collect();
            drop(s);
            RESPEC.lock().unwrap_or_else(PoisonError::into_inner).extend(respec);
            Ok(format!("{} mobs reloaded", state().data.mobs.len()))
        }
    }
}

/// Back to the built-in numbers (the self-test, after it took its own file away).
pub(super) fn reset_data() {
    let mut s = state();
    s.data = MobsFile::embedded();
    s.source = "built-in".into();
    DATA_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Rises each time `state().data` is replaced: what is built from the data once (the vanilla
/// groups' type index) is built again when it moves.
static DATA_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn data_generation() -> u64 {
    DATA_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Mobs whose stats are set again on the next tick (after a reload).
static RESPEC: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// A mob of the level as the tick reads it.
struct Seen {
    id: u32,
    kind: String,
    pos: V3,
    yaw: f64,
    health: f64,
    max_health: f64,
    alive: bool,
    on_ground: bool,
    in_water: bool,
    wall: bool,
}

/// A target: a player, or (for the self-test) a mob.
#[derive(Clone)]
pub(super) struct Target {
    pub entity: u32,
    pub pos: V3,
    pub name: String,
    pub player: Option<u128>,
    pub sense: TargetSense,
}

pub(super) fn on_level_tick(game: &mut Game<'_>, dimension: &str) {
    use crate::game::soulscraft_cost::{Clock, Part};
    let mut clock = Clock::start();
    let now = game.world.game_time.max(0) as u64;
    crate::game::soulscraft_find::reindex(game);
    clock.lap(Part::Index);
    crate::game::soulscraft_tools::run_due(game, dimension, now);
    clock.lap(Part::Tasks);
    // A frozen world (`/tick freeze`) still runs the level hooks with its clock stopped: the mobs
    // advance once per game tick, or a blow due on a tick would land again and again.
    {
        let mut last = LAST_TICK.lock().unwrap_or_else(PoisonError::into_inner);
        if last.get(dimension) == Some(&now) {
            return;
        }
        last.insert(dimension.to_owned(), now);
    }
    hurt_bleeders(game, dimension);
    clock.lap(Part::Own);
    crate::game::soulscraft_altar::on_level_tick(game, dimension, now);
    clock.lap(Part::Altar);
    crate::game::soulscraft_trophy::on_level_tick(game, dimension, now);
    clock.lap(Part::Trophy);
    crate::game::soulscraft_spawn_boost::on_level_tick(dimension, now);
    crate::game::soulscraft_natural::on_level_tick(game, dimension, now);
    clock.lap(Part::Spawning);
    crate::game::soulscraft_relic::on_level_tick(game, dimension, now);
    clock.lap(Part::Relic);
    if dimension == "minecraft:overworld" {
        let online = game.native_server_players();
        crate::game::soulscraft_power::refresh(game, &online, now);
    }
    clock.lap(Part::Power);
    let players = players_in(game, dimension);
    let seen = seen_mobs(game);
    remember_players(&players, &seen);
    let mut notes = Vec::new();
    for m in &seen {
        tick_mob(game, dimension, now, m, &players, &mut notes);
    }
    forget_gone(dimension, now);
    send_notes(game, dimension, now, &players, notes);
    clock.lap(Part::Own);
    // The approved vanilla changes (07-vanilla-mobs.md): adopt and drive vanilla mobs.
    crate::game::soulscraft_vanilla::on_level_tick(game, dimension, now, &players);
    clock.lap(Part::Vanilla);
    crate::game::soulscraft_book::on_level_tick(game, dimension, now);
    clock.lap(Part::Book);
    clock.finish(now);
    crate::game::soulscraft_selftest::on_tick(game, dimension, now);
}

/// The game tick each level was last driven at.
static LAST_TICK: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Mutex::default);

fn seen_mobs(game: &Game<'_>) -> Vec<Seen> {
    game.mobs
        .iter()
        .filter(|m| !m.removed && m.kind.id.starts_with("soulscraft:"))
        .map(|m| Seen {
            id: m.id,
            kind: m.kind.id.clone(),
            pos: [m.position.x, m.position.y, m.position.z],
            yaw: f64::from(m.yaw),
            health: f64::from(m.health),
            max_health: f64::from(m.attrs.max_health),
            alive: m.alive(),
            on_ground: m.on_ground,
            in_water: m.fluids.in_water,
            wall: m.horizontal_collision,
        })
        .collect()
}

/// The players of this level the mobs may hunt: alive, not spectators; creative ones only while
/// debug is on (they take no damage, but the attacks show).
fn players_in(game: &Game<'_>, dimension: &str) -> Vec<(NativeServerPlayer, bool)> {
    let debug = state().debug;
    let shields = shields_up(game);
    game.native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dimension && p.health > 0.0 && p.game_mode != 3)
        .filter(|p| debug || p.game_mode != 1)
        .map(|p| {
            let up = shields.contains(&p.uuid);
            (p, up)
        })
        .collect()
}

/// Players holding a shield up: using an item (`living_flags` bit 1) that is a shield, in the
/// off hand (bit 2) or the selected slot (lan/guest.rs:156-158, 282-288).
fn shields_up(game: &Game<'_>) -> Vec<u128> {
    let Some(host) = game.lan.as_ref() else { return Vec::new() };
    let shield = |s: Option<&horizon_client::items::ItemStack>| s.is_some_and(|s| &*s.item == "minecraft:shield");
    host.hub
        .guests
        .iter()
        .filter(|g| g.placed && g.living_flags & 1 != 0)
        .filter(|g| {
            if g.living_flags & 2 != 0 {
                shield(g.offhand.as_ref())
            } else {
                shield(g.inventory.get(g.selected).and_then(Option::as_ref))
            }
        })
        .map(|g| g.uuid)
        .collect()
}

fn remember_players(players: &[(NativeServerPlayer, bool)], _seen: &[Seen]) {
    let mut g = GUARDS.lock().unwrap_or_else(PoisonError::into_inner);
    g.players = players.iter().map(|(p, _)| (p.entity_id, p.position)).collect();
}

fn tick_mob(
    game: &mut Game<'_>,
    dimension: &str,
    now: u64,
    m: &Seen,
    players: &[(NativeServerPlayer, bool)],
    notes: &mut Vec<wire::Note>,
) {
    let known = state().mobs.contains_key(&m.id);
    if !known {
        if !m.alive {
            return;
        }
        adopt(game, dimension, now, m, players);
    } else if RESPEC.lock().unwrap_or_else(PoisonError::into_inner).contains(&m.id) {
        RESPEC.lock().unwrap_or_else(PoisonError::into_inner).retain(|x| *x != m.id);
        let (kind, p, size) = {
            let s = state();
            let t = &s.mobs[&m.id];
            (t.kind, t.p, t.size)
        };
        apply_stats(game, m.id, kind, p, size, false);
        fit_hitbox(game, m.id, kind, size);
    }
    if !m.alive {
        on_death(game, dimension, now, m);
        return;
    }
    no_drowned_conversion(game, m.id);
    // A wave mob of an altar's event: the event walks it to the altar (its brain rests), or names
    // the participant its brain fights (soulscraft_event, «Оборона алтаря»).
    use crate::game::soulscraft_event::{Steering, steering};
    let steer = steering(m.id);
    if steer == Some(Steering::Own) {
        if let Some(t) = state().mobs.get_mut(&m.id) {
            t.seen = now;
            t.pos = m.pos;
            t.yaw = m.yaw;
            t.dim = dimension.to_owned();
        }
        return;
    }
    // Cover from attack turrets (soulscraft_cover): outside an event, a mob that a shooting
    // turret sees walks out of its line of fire and holds there; its brain rests meanwhile.
    if steer.is_none() {
        let fit = {
            let s = state();
            s.mobs.get(&m.id).map(|t| {
                let d = &s.data.mobs[t.kind];
                let hb = d.hitbox_at(t.size);
                (d.grounded(), (hb.width, hb.height, hb.eye))
            })
        };
        if let Some((true, size)) = fit
            && crate::game::soulscraft_cover::steer(game, dimension, now, m.id, m.pos, size).is_some()
        {
            if let Some(t) = state().mobs.get_mut(&m.id) {
                t.seen = now;
                t.pos = m.pos;
                t.yaw = m.yaw;
                t.dim = dimension.to_owned();
            }
            return;
        }
    }
    // A grabbing lurker keeps the one in its mouth: gone or dead, it has no target and lets go.
    let prey = GRABS.lock().unwrap_or_else(PoisonError::into_inner).get(&(m.id as i32)).map(|g| g.prey);
    let target = match (prey, steer) {
        (Some(e), _) => target_entity(game, m, e, players),
        (None, Some(Steering::Fight(e))) => target_entity(game, m, e, players),
        _ => choose_target(game, m, players),
    };
    let blows = take_blows(m.id, m.health);
    // The hunter's book: the mob saw its target («Встречен», soulscraft_book).
    crate::game::soulscraft_book::saw(&m.kind, m.pos, target.as_ref().map(|t| (t.entity, t.player, t.pos)));
    let mut s = state();
    let State { data, mobs, debug, turns, .. } = &mut *s;
    let Some(t) = mobs.get_mut(&m.id) else { return };
    let def = &data.mobs[t.kind];
    t.seen = now;
    t.pos = m.pos;
    t.yaw = m.yaw;
    t.dim = dimension.to_owned();
    let diver = t.brain.style == Style::Diver;
    let senses = Senses {
        now,
        pos: m.pos,
        yaw: m.yaw,
        health_frac: if m.max_health > 0.0 { m.health / m.max_health } else { 1.0 },
        on_ground: m.on_ground,
        in_water: m.in_water,
        wall_hit: m.wall,
        wait_turn: diver && target.as_ref().is_some_and(|tg| turns.waits(tg.entity, m.id, now)),
        blows,
        target: target.as_ref().map(|t| t.sense),
    };
    let sc = scaled(def, t.p, t.size);
    let tick = t.brain.tick(def, &sc, &senses);
    if diver && let Some(tg) = &target {
        // The first shrike into its windup takes the target's turn; its dive's end gives it back.
        let dive_ticks = def.attack("dive").map_or(0, |a| u64::from(Timeline::of(a).end()));
        turns.note(tg.entity, m.id, now, &tick, dive_ticks);
    }
    note_grab(m, &tick, target.as_ref());
    let contact = t.brain.contact(&senses);
    t.guarding = tick.guarding;
    let flags = (if tick.rage { wire::RAGE } else { 0 }) | (if tick.guarding { wire::GUARD } else { 0 });
    let clip = t.brain.clip().clone();
    let note_due = t.sent.as_ref() != Some(&(clip.name.clone(), clip.start as i64, flags));
    if note_due || *debug {
        t.sent = Some((clip.name.clone(), clip.start as i64, flags));
        notes.push(wire::Note {
            id: m.id,
            clip: clip.name,
            start: clip.start as i64,
            flags: flags | if *debug { wire::DEBUG } else { 0 },
            size: t.size,
            label: if *debug { tick.label.clone() } else { String::new() },
        });
    }
    let def = def.clone();
    let p = t.p;
    let size = t.size;
    let style = t.brain.style;
    let repath = tick.repath;
    let path = t.path;
    drop(s);
    guard_snapshot(m, &tick, style);
    log_events(&def, m.id, now, &tick);
    let leap_up = def.jump_velocity_at(size).unwrap_or(LEAP_UP);
    let path = drive(game, dimension, now, m, &tick, style, repath, path, &sc, leap_up);
    if let Some(t) = state().mobs.get_mut(&m.id) {
        t.path = path;
        t.stopped = matches!(tick.order, Order::Stay);
    }
    if let Some(at) = tick.hold
        && let Some(tg) = &target
    {
        hold_target(game, dimension, tg, at, players);
    }
    if let Some(at) = tick.shadow
        && let Some(kind) = def.params.get("shadow_particle").and_then(serde_json::Value::as_str)
    {
        shadow(game, &def, size, at, kind);
    }
    for strike in &tick.strikes {
        deal(game, dimension, now, m, &def, &sc, strike, target.as_ref(), players, p, size);
    }
    if contact && let Some(tg) = &target {
        stick(game, &def, tg, m.id);
    }
    play_cues(game, &def, m, &tick, size);
    roar(game, &def, m, &tick, size);
    if tick.wants_cover || tick.wants_post {
        answer_marksman(game, &def, m, &tick, target.as_ref(), size, now);
    }
}

/// The cues of this tick's attack phases (data/mobs.json `cue`, at its `on` phase) and of the
/// marksman's reload as it starts (its `hide` move's cue).
fn play_cues(game: &mut Game<'_>, def: &MobDef, m: &Seen, tick: &Tick, size: u8) {
    let at = geom::add(m.pos, [0.0, def.hitbox_at(size).height * 0.6, 0.0]);
    for e in &tick.events {
        let Event::Phase { attack, phase, .. } = e else { continue };
        if let Some(c) = def.attacks.get(*attack).and_then(|a| a.cue.as_ref()).filter(|c| c.on.phase() == *phase) {
            play_cue(game, m.id, at, &c.cue);
        }
    }
    if tick.wants_cover
        && let Some(c) = def.moves.iter().find(|x| x.id == "hide").and_then(|x| x.cue.as_ref())
    {
        play_cue(game, m.id, at, c);
    }
}

/// A cue (the vanilla groups' schema, soulscraft_rules::vanilla::Cue): its sound at `at` from the
/// hostile source (`GameLevel::native_play_sound`, native_hooks.rs:9906), its particles round the
/// mob's feet (`native_send_particles`, native_hooks.rs:9936) and a small hop while it stands. The
/// vanilla groups' windups (soulscraft_vanilla) and SoulsCraft's own mobs play it; the `pose` is
/// the caller's (a vanilla model's raised arms).
pub(super) fn play_cue(game: &mut Game<'_>, mob_id: u32, at: V3, cue: &Cue) {
    let Some((feet, width, height, on_ground)) = crate::game::soulscraft_find::mob(game, mob_id)
        .map(|m| ([m.position.x, m.position.y, m.position.z], m.kind.width, m.kind.height, m.on_ground))
    else {
        return;
    };
    if let Some(sound) = &cue.sound {
        let pitch = cue.pitch.unwrap_or(1.0);
        game.with_level(|_, level, _| level.native_play_sound(sound, at, HOSTILE, 1.0, pitch));
    }
    if let Some(kind) = &cue.particle {
        let options = ParticleOptions::named(kind.clone());
        let spread = [width * 0.6, height * 0.3, width * 0.6];
        let low = geom::add(feet, [0.0, 0.2, 0.0]);
        game.with_level(|_, level, _| level.native_send_particles(&options, None, low, spread, 0.02, 12));
    }
    if let Some(up) = cue.hop
        && on_ground
    {
        set_velocity(game, mob_id, [0.0, up, 0.0], true);
    }
}

/// A standing player's eye height (vanilla `Player`), when the target's own is not at hand.
const PLAYER_EYE: f64 = 1.62;

/// The marksman's asks of this tick (soulscraft_rules::brain, `Tick::wants_cover`, `wants_post`):
/// cover from its target for the reload that starts, a post in its kept distance.
#[allow(clippy::too_many_arguments)]
fn answer_marksman(game: &mut Game<'_>, def: &MobDef, m: &Seen, tick: &Tick, target: Option<&Target>, size: u8, now: u64) {
    let Some(tg) = target else { return };
    let hb = def.hitbox_at(size);
    let cover = if tick.wants_cover { reload_cover(game, def, m, tg, hb) } else { None };
    let post = if tick.wants_post { high_post(game, def, m, tg, hb) } else { None };
    let mut s = state();
    let Some(t) = s.mobs.get_mut(&m.id) else { return };
    if tick.wants_cover {
        t.brain.set_cover(cover);
        let to = cover.map_or("-".into(), |c| format!("{:.1},{:.1},{:.1}", c[0], c[1], c[2]));
        log::log!(chatty(), "soulscraft: reload_cover mob={} id={} to={to} tick={now}", def.id, m.id);
    }
    if tick.wants_post {
        t.brain.set_post(post);
    }
}

/// Where the marksman reloads: the nearest spot within `reload_cover_blocks` its target cannot see
/// at its eyes nor at `cover_min_height_blocks` over its feet (soulscraft_rules::cover::reload_cover,
/// the lines and spots of soulscraft_cover).
fn reload_cover(game: &Game<'_>, def: &MobDef, m: &Seen, tg: &Target, hb: soulscraft_rules::data::Hitbox) -> Option<V3> {
    let k = def.param("reload_cover_blocks").unwrap_or(8.0) as i32;
    let top = def.param("cover_min_height_blocks").unwrap_or(2.0);
    // A player's own (`Guest::eye_height`, lan/guest.rs:812: sneaking too), a mob's (mobs/living.rs:558).
    let eye_height = match tg.player {
        Some(uuid) => game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map_or(PLAYER_EYE, |g| g.eye_height()),
        None => crate::game::soulscraft_find::mob(game, tg.entity).map_or(PLAYER_EYE, |x| x.eye_height()),
    };
    let eye = geom::add(tg.pos, [0.0, eye_height, 0.0]);
    let spots = crate::game::soulscraft_cover::spots(game, m.pos, k, hb.width, hb.height);
    let sight = |a: V3, b: V3| crate::game::soulscraft_cover::sees(game, a, b);
    soulscraft_rules::cover::reload_cover(m.pos, &spots, &[hb.eye, top], eye, &sight)
}

/// The marksman's post: the candidates of soulscraft_rules::brain::post_candidates on the ground
/// (within `high_ground_scan_blocks` up or down; where it stands is kept as it is), the highest
/// with `prefers_high_ground` (`pick_post`).
fn high_post(game: &Game<'_>, def: &MobDef, m: &Seen, tg: &Target, hb: soulscraft_rules::data::Hitbox) -> Option<V3> {
    let (lo, hi) = def.param_range("keep_distance_blocks").unwrap_or((10.0, 16.0));
    let scan = def.param("high_ground_scan_blocks").unwrap_or(6.0) as i32;
    let grounded: Vec<V3> = brain::post_candidates(tg.pos, m.pos, lo, hi)
        .into_iter()
        .filter_map(|c| if c == m.pos { Some(c) } else { crate::game::soulscraft_cover::ground(game, c, scan, hb.width, hb.height) })
        .collect();
    brain::pick_post(&grounded, m.pos, def.param_bool("prefers_high_ground"))
}

/// The slime's jump without `sizes.*.jump_velocity`: the upward speed it always had.
const LEAP_UP: f64 = 0.62;

/// The numbers of this mob at its P (the slime's by its size, from `sizes`: health, damage and
/// its decision interval, `decide_mult`).
pub(super) fn scaled(def: &MobDef, p: f64, size: u8) -> Scaled {
    let mut stats = def.stats;
    if let Some(sz) = def.sizes.iter().find(|s| s.size == u32::from(size)) {
        stats.health = sz.health;
        stats.damage = sz.damage;
    }
    let mut sc = Scaled::at(&stats, p);
    sc.decide_every = def.decide_every_at(size, sc.decide_every);
    sc
}

/// The brute's roar as its rage starts (the mode `rage`): `passives.rage.cue`.
fn roar(game: &mut Game<'_>, def: &MobDef, m: &Seen, tick: &Tick, size: u8) {
    if !tick.events.contains(&Event::Mode("rage")) {
        return;
    }
    let Some(cue) = def.passive("rage").and_then(|r| serde_json::from_value::<Cue>(r.get("cue")?.clone()).ok()) else { return };
    let at = geom::add(m.pos, [0.0, def.hitbox_at(size).height * 0.6, 0.0]);
    play_cue(game, m.id, at, &cue);
}

/// The slime's shadow where it lands (`params.shadow_particle`): the cue's particle shape, sized by
/// its box at its size, round the landing point.
fn shadow(game: &mut Game<'_>, def: &MobDef, size: u8, at: V3, kind: &str) {
    let hb = def.hitbox_at(size);
    let options = ParticleOptions::named(kind.to_owned());
    let spread = [hb.width * 0.6, hb.height * 0.3, hb.width * 0.6];
    let low = geom::add(at, [0.0, 0.2, 0.0]);
    game.with_level(|_, level, _| level.native_send_particles(&options, None, low, spread, 0.02, 12));
}


/// The lurker's grab: its target put at its mouth this tick (a player keeps where it looks).
fn hold_target(game: &mut Game<'_>, dim: &str, tg: &Target, at: V3, players: &[(NativeServerPlayer, bool)]) {
    match tg.player {
        Some(uuid) => {
            if let Some((p, _)) = players.iter().find(|(p, _)| p.uuid == uuid) {
                game.native_teleport_player(uuid, dim, at, p.yaw, p.pitch);
            }
        }
        None => {
            game.native_teleport_mob(dim, tg.entity, at);
        }
    }
}

fn guard_snapshot(m: &Seen, tick: &Tick, style: Style) {
    if style != Style::Duelist {
        return;
    }
    let reduction = state()
        .data
        .mob("revenant")
        .and_then(|d| d.param("shield_front_reduction"))
        .unwrap_or(0.8);
    let mut g = GUARDS.lock().unwrap_or_else(PoisonError::into_inner);
    if tick.guarding {
        g.shields.insert(m.id as i32, (m.pos, m.yaw, reduction));
    } else {
        g.shields.remove(&(m.id as i32));
    }
}

/// A new SoulsCraft mob (spawned, or back with its chunk): its P and size from its tags, or from
/// the players near when it has none yet; then its stats.
fn adopt(game: &mut Game<'_>, dimension: &str, now: u64, m: &Seen, players: &[(NativeServerPlayer, bool)]) {
    let Some(kind) = state().data.by_key(&m.kind).map(|(i, _)| i) else {
        log::warn!("soulscraft: {} is not in mobs.json", m.kind);
        return;
    };
    let entity = game.native_entity(m.id);
    let tags = entity.and_then(|e| game.native_entity_tags(e)).unwrap_or_default();
    let tag = |prefix: &str| tags.iter().find_map(|t| t.strip_prefix(prefix)).and_then(|v| v.parse::<f64>().ok());
    let fresh = tags.iter().any(|t| t == TAG_FRESH);
    let p = match tag(TAG_P) {
        Some(p) => scale::clamp_p(p),
        None => {
            let near: Vec<_> = players.iter().map(|(p, _)| (p.position, p.uuid)).collect();
            let p = crate::game::soulscraft_power::power_here(game, m.pos, &near);
            if let Some(e) = entity {
                game.native_entity_add_tag(e, &format!("{TAG_P}{p:.2}"));
            }
            p
        }
    };
    let id_str = state().data.mobs[kind].id.clone();
    let size = tag(TAG_SIZE).map(|s| s as u8).unwrap_or(if id_str == "bog_slime" { 4 } else { 0 });
    let style = Style::of(&id_str);
    state().mobs.insert(
        m.id,
        Tracked {
            kind,
            p,
            size,
            brain: Brain::new(style, u64::from(m.id), now),
            dim: dimension.to_owned(),
            seen: now,
            pos: m.pos,
            yaw: m.yaw,
            guarding: false,
            alive: true,
            sent: None,
            path: None,
            stopped: true,
            target_override: None,
            test_shield: false,
        },
    );
    apply_stats(game, m.id, kind, p, size, fresh);
    fit_hitbox(game, m.id, kind, size);
    // Round 9: with its own box (bigger than the zombie template's for the brute) it must fit
    // where it stands; else it goes to the nearest spot that fits.
    match crate::game::soulscraft_tools::fit_spot(game, &id_str, size, m.pos) {
        Some((spot, true)) => {
            game.native_teleport_mob(dimension, m.id, spot);
            log::info!("soulscraft: adopt moved mob={id_str} id={} to={:.1},{:.1},{:.1}", m.id, spot[0], spot[1], spot[2]);
        }
        None => log::warn!("soulscraft: adopt mob={id_str} id={}: no room for its box within the search radius", m.id),
        _ => {}
    }
    if fresh && let Some(e) = entity {
        game.native_entity_remove_tag(e, TAG_FRESH);
    }
    if style.steered()
        && let Some(mob) = crate::game::soulscraft_find::mob_mut(game, m.id)
    {
        // Flyers and swimmers are steered by velocity: no gravity pulls them off their course.
        mob.no_gravity = true;
    }
    log::log!(
        chatty(),
        "soulscraft: adopt mob={id_str} id={} p={p:.2} size={size} fresh={fresh} health={:.1} tick={now}",
        m.id,
        m.health
    );
}

/// The mob's own collision box (data/mobs.json `hitbox`, a smaller slime's by its size) on its
/// copy of the type: the goal hook set the data's at its making (hook_register_goals.rs), and this
/// sets it again after `/sc reload` and for a slime whose size its tag gives. A box is the type's
/// width and height (mobs/living.rs:489); the path finder sizes its steps by it (mobs/path.rs:430).
pub(super) fn fit_hitbox(game: &mut Game<'_>, id: u32, kind: usize, size: u8) {
    let hb = state().data.mobs[kind].hitbox_at(size);
    let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) else { return };
    let same = |a: f64, b: f64| (a - b).abs() < 1e-6;
    if same(m.kind.width, hb.width) && same(m.kind.height, hb.height) && same(m.kind.eye_height, hb.eye) {
        return;
    }
    let k = std::sync::Arc::make_mut(&mut m.kind);
    (k.width, k.height, k.eye_height) = (hb.width, hb.height, hb.eye);
    m.needs_sync = true;
}
/// Sets the mob's attributes from its P: health, speed, damage and knockback resistance on the
/// mob itself (mobs.rs:1017-1027, read by its tick and its blows), armour by `/attribute`, which
/// keeps the command's own armour map (mobs/armor.rs:43-63).
pub(super) fn apply_stats(game: &mut Game<'_>, id: u32, kind: usize, p: f64, size: u8, fresh: bool) {
    let def = state().data.mobs[kind].clone();
    let sc = scaled(&def, p, size);
    let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) else { return };
    m.attrs.max_health = sc.max_health as f32;
    m.attrs.speed = sc.speed;
    m.attrs.attack_damage = sc.damage as f32;
    m.attrs.knockback_resistance = sc.knockback_resistance;
    // The zombie template's reinforcements (a hurt mob on Hard calls another of its own type,
    // game/zombie_reinforcements.rs:32-61) are not SoulsCraft's design: never.
    m.attrs.spawn_reinforcements = 0.0;
    if fresh || m.health > m.attrs.max_health {
        m.health = m.attrs.max_health;
    }
    m.needs_sync = true;
    set_armor(game, id, sc.armor);
    let base = scaled(&def, 0.0, size).max_health;
    set_xp_tag_sized(game, id, base, sc.max_health, scale::size_share(size, def.full_size()));
}

/// The mob's armour attribute. `/attribute` cannot do it for a mod's living type: the command makes
/// the mob's armour map from its own type id (game/commands/world_cmds.rs:1376-1381), and the
/// attribute catalog knows vanilla's types only, so it fails. The map is made here from the
/// template's, as the command would for a zombie, and read by the damage code
/// (mobs/armor.rs:43-63).
fn set_armor(game: &mut Game<'_>, id: u32, armor: f64) {
    let Some(i) = crate::game::soulscraft_find::index_of(game, id) else { return };
    if game.mobs[i].extra.armor_attributes.is_none() {
        game.mobs[i].extra.armor_attributes = game.motion.attributes.create("minecraft:zombie").map(Box::new);
    }
    let Some(map) = game.mobs[i].extra.armor_attributes.as_deref_mut() else {
        log::warn!("soulscraft: no armour map for {id}");
        return;
    };
    let Some(key) = map.catalog().id("minecraft:armor") else { return };
    map.edit(key, |value, observer| {
        let _ = value.set_base_value(armor, observer);
    });
}

fn choose_target(game: &Game<'_>, m: &Seen, players: &[(NativeServerPlayer, bool)]) -> Option<Target> {
    let forced = state().mobs.get(&m.id).and_then(|t| t.target_override.map(|o| (o, t.test_shield)));
    if let Some((id, shield_up)) = forced {
        let t = crate::game::soulscraft_find::mob(game, id).filter(|x| !x.removed && x.alive())?;
        let pos = [t.position.x, t.position.y, t.position.z];
        return Some(Target {
            entity: id,
            pos,
            name: format!("mob{id}"),
            player: None,
            sense: TargetSense { pos, yaw: f64::from(t.yaw), shield_up, ..TargetSense::default() },
        });
    }
    let (p, up) = players
        .iter()
        .filter(|(p, _)| geom::dist(p.position, m.pos) <= FOLLOW)
        .min_by(|a, b| geom::dist(a.0.position, m.pos).total_cmp(&geom::dist(b.0.position, m.pos)))?;
    let pos = p.position;
    let looking = geom::off_facing(pos, f64::from(p.yaw), m.pos) <= 60.0;
    Some(Target {
        entity: p.entity_id as u32,
        pos,
        name: p.name.clone(),
        player: Some(p.uuid),
        sense: TargetSense {
            pos,
            yaw: f64::from(p.yaw),
            shield_up: *up,
            looking_at_me: looking,
            at_wall: at_wall(game, m.pos, pos),
            under_roof: under_roof(game, pos),
        },
    })
}

/// The participant an event names for a wave mob: a player, or a mob standing in for one (the
/// self-test's golems).
fn target_entity(game: &Game<'_>, m: &Seen, entity: u32, players: &[(NativeServerPlayer, bool)]) -> Option<Target> {
    if let Some((p, up)) = players.iter().find(|(p, _)| p.entity_id as u32 == entity) {
        let pos = p.position;
        return Some(Target {
            entity,
            pos,
            name: p.name.clone(),
            player: Some(p.uuid),
            sense: TargetSense {
                pos,
                yaw: f64::from(p.yaw),
                shield_up: *up,
                looking_at_me: geom::off_facing(pos, f64::from(p.yaw), m.pos) <= 60.0,
                at_wall: at_wall(game, m.pos, pos),
                under_roof: under_roof(game, pos),
            },
        });
    }
    let t = crate::game::soulscraft_find::mob(game, entity).filter(|x| !x.removed && x.alive())?;
    let pos = [t.position.x, t.position.y, t.position.z];
    Some(Target {
        entity,
        pos,
        name: format!("mob{entity}"),
        player: None,
        sense: TargetSense { pos, yaw: f64::from(t.yaw), ..TargetSense::default() },
    })
}

/// Path-of-craft-tree's experience tag (Г3, variant A; PCT v0.4.6): exactly one `pct.xpNNN` on
/// the mob, NNN = round(100 · (1 + 0.5 · (health multiplier − 1))), 1 to 500. Older ones go
/// first, so a new P (`/sc reload`, re-adoption) leaves one.
pub(super) fn set_xp_tag(game: &mut Game<'_>, id: u32, base: f64, scaled: f64) {
    set_xp_tag_sized(game, id, base, scaled, 1.0);
}

/// [`set_xp_tag`] for a mob worth `share` of its kind: a smaller bog slime (`scale::size_share`;
/// PCT pays the slime its full size's health). A split slime's children get theirs at adoption.
pub(super) fn set_xp_tag_sized(game: &mut Game<'_>, id: u32, base: f64, scaled: f64, share: f64) {
    let Some(e) = game.native_entity(id) else { return };
    let tag = scale::xp_tag_sized(base, scaled, share);
    let tags = game.native_entity_tags(e).unwrap_or_default();
    if tags.iter().filter(|t| t.starts_with(scale::XP_TAG_PREFIX)).eq(std::iter::once(&tag)) {
        return;
    }
    for old in tags.iter().filter(|t| t.starts_with(scale::XP_TAG_PREFIX)) {
        game.native_entity_remove_tag(e, old);
    }
    game.native_entity_add_tag(e, &tag);
}

/// A clip shown now on a mob the event drives (its brain rests: nobody else sends its notes).
pub(super) fn show_clip(game: &mut Game<'_>, dim: &str, id: u32, clip: &str, now: u64) {
    let size = {
        let mut s = state();
        let Some(t) = s.mobs.get_mut(&id) else { return };
        t.sent = Some((clip.to_owned(), now as i64, 0));
        t.size
    };
    let note = wire::Note { id, clip: clip.to_owned(), start: now as i64, flags: 0, size, label: String::new() };
    let players: Vec<u128> = game.native_server_players().into_iter().filter(|p| p.dimension == dim).map(|p| p.uuid).collect();
    for bytes in wire::encode(&wire::Batch { now: now as i64, notes: vec![note] }) {
        for u in &players {
            let _ = game.native_net_send_to_player(*u, wire::STATE, &bytes);
        }
    }
}

/// A block with a collision box there, in the level in place (`native_block_collision`,
/// game/native_hooks.rs:11143).
fn solid(game: &Game<'_>, x: f64, y: f64, z: f64) -> bool {
    !game
        .native_block_collision([x.floor() as i32, y.floor() as i32, z.floor() as i32])
        .is_empty()
}

/// A block right behind the target, one block past it away from the mob, at its chest.
fn at_wall(game: &Game<'_>, mob: V3, target: V3) -> bool {
    let d = geom::flat_dir(mob, target);
    solid(game, target[0] + d[0], target[1] + 1.0, target[2] + d[2])
}

/// Any block in the eight above the target's head.
fn under_roof(game: &Game<'_>, target: V3) -> bool {
    (2..10).any(|dy| solid(game, target[0], target[1] + f64::from(dy), target[2]))
}

/// The brain's order as the engine's calls. Returns the path kept (where to and when).
#[allow(clippy::too_many_arguments)]
fn drive(
    game: &mut Game<'_>,
    dim: &str,
    now: u64,
    m: &Seen,
    tick: &Tick,
    style: Style,
    repath: bool,
    path: Option<(V3, u64)>,
    sc: &Scaled,
    leap_up: f64,
) -> Option<(V3, u64)> {
    if let Some(at) = tick.look_at {
        game.native_mob_look_at(dim, m.id, at);
    }
    let speed = sc.speed * tick.speed_mult;
    if let Some(mob) = crate::game::soulscraft_find::mob_mut(game, m.id) {
        mob.attrs.speed = speed;
    }
    match tick.order {
        Order::Stay => {
            if path.is_some() {
                game.native_mob_stop(dim, m.id);
            }
            if style.steered() {
                set_velocity(game, m.id, [0.0; 3], true);
            }
            None
        }
        Order::Walk { to, speed } => {
            let moved = path.is_none_or(|(at, when)| geom::dist(at, to) > 1.5 || now - when >= REPATH_EVERY);
            if repath || moved {
                game.native_mob_navigate(dim, m.id, to, 1, speed);
                Some((to, now))
            } else {
                path
            }
        }
        Order::Steer { to, speed } => {
            let d = geom::sub(to, m.pos);
            let l = geom::len(d);
            let v = if l < 1e-3 { [0.0; 3] } else { geom::scale(d, speed.min(l) / l) };
            set_velocity(game, m.id, v, true);
            if let Some(at) = tick.look_at.or(Some(to)) {
                game.native_mob_look_at(dim, m.id, at);
            }
            None
        }
        Order::Dash { dir, per_tick } => {
            if path.is_some() {
                game.native_mob_stop(dim, m.id);
            }
            set_velocity(game, m.id, geom::scale(dir, per_tick), style.steered());
            None
        }
        Order::Leap { to } => {
            let d = geom::flat_dir(m.pos, to);
            let reach = geom::flat_dist(m.pos, to).min(8.0) / 12.0;
            set_velocity(game, m.id, [d[0] * reach, leap_up, d[2] * reach], true);
            None
        }
    }
}

/// Sets a mob's velocity (`with_y`: all three, else only the ground plane).
fn set_velocity(game: &mut Game<'_>, id: u32, v: V3, with_y: bool) {
    if let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) {
        m.velocity.x = v[0];
        m.velocity.z = v[2];
        if with_y {
            m.velocity.y = v[1];
        }
        m.needs_sync = true;
    }
}

fn log_events(def: &MobDef, id: u32, now: u64, tick: &Tick) {
    let mut s = state();
    for e in &tick.events {
        match e {
            Event::Phase { attack, phase, t } => {
                let a = def.attacks.get(*attack).map_or("?", |a| a.id.as_str());
                log::log!(
                    chatty(),
                    "soulscraft: phase mob={} id={id} attack={a} phase={} t={t} tick={now}",
                    def.id,
                    phase.label()
                );
                s.phases.push(PhaseRecord { mob: id, attack: a.to_owned(), phase: *phase, t: *t, tick: now });
            }
            Event::Mode(mode) => {
                log::info!("soulscraft: mode mob={} id={id} mode={mode} tick={now}", def.id);
                s.modes.push((id, (*mode).to_owned(), now));
            }
            _ => {}
        }
    }
    let n = s.phases.len();
    if n > LOG_KEEP {
        s.phases.drain(..n - LOG_KEEP);
    }
    let n = s.modes.len();
    if n > LOG_KEEP {
        s.modes.drain(..n - LOG_KEEP);
    }
}

/// One blow: the target if it is in reach, everyone in the area, or a bolt.
#[allow(clippy::too_many_arguments)]
fn deal(
    game: &mut Game<'_>,
    dim: &str,
    now: u64,
    m: &Seen,
    def: &MobDef,
    sc: &Scaled,
    strike: &Strike,
    target: Option<&Target>,
    players: &[(NativeServerPlayer, bool)],
    _p: f64,
    size: u8,
) {
    let Some(a) = def.attacks.get(strike.attack) else { return };
    let t = state().mobs.get(&m.id).and_then(|x| x.brain.attack(now)).map_or(0, |(_, t)| t);
    let blow = sc.blow(a.damage);
    let mut landed = false;
    if strike.ranged {
        if let Some(tg) = target {
            landed = shoot(game, dim, m, tg, blow, def.hitbox_at(size).eye);
        }
    } else if strike.hits_target
        && let Some(tg) = target
    {
        // The grab's bites, 10 ticks apart, are meant to land too (its `follows` marks the grab).
        let later = a.strikes_at.as_ref().is_some_and(|s| s.len() > 1) && t > Timeline::of(a).hit_start();
        if later || a.follows.is_some() {
            clear_hurt_cooldown(game, tg);
        }
        landed = if a.knockback {
            let strength = def.param("knockback_strength").unwrap_or(0.0);
            let break_shield = tg.sense.shield_up && def.param("shield_disable_ticks").is_some_and(|t| t > 0.0);
            knock_blow(game, dim, m.id, tg.entity, blow, strength, break_shield)
        } else {
            game.native_hurt_entity(dim, tg.entity, blow as f32, "minecraft:mob_attack", Some(m.id))
        };
        if landed {
            crate::game::soulscraft_book::struck(&def.key, tg.entity, tg.player);
            give_effects(game, &a.effects, tg.player);
            if strike.area.is_some()
                && let Some(w) = &a.wave
            {
                // The one the slime landed on is in its mud too: the wave's slowness.
                give_effects(game, &w.effects, tg.player);
            }
        }
    }
    if let Some(radius) = strike.area {
        landed |= deal_wave(game, dim, m, def, sc, a, radius, blow, strike.hits_target, target, players);
    }
    let who = target.map_or("-", |t| t.name.as_str());
    log::log!(
        chatty(),
        "soulscraft: strike mob={} id={} attack={} t={t} tick={now} hit={} dmg={blow:.2} target={who}",
        def.id,
        m.id,
        a.id,
        landed
    );
    let mut s = state();
    s.strikes.push(StrikeRecord { mob: m.id, attack: a.id.clone(), t, tick: now, hit: landed, damage: blow });
    let n = s.strikes.len();
    if n > LOG_KEEP {
        s.strikes.drain(..n - LOG_KEEP);
    }
}

/// A blow with `knockback` (the revenant's shield bash, the marksman's butt strike): the engine's
/// own mob hit (`native_mob_attack`, native_npcs.rs:231, `Mob.doHurtTarget`) at the blow's damage,
/// with its extra push from the `attack_knockback` attribute (`knockback_strength`; half of it is
/// the push, mobs/ai.rs:2806-2821), and with `break_shield` an axe in hand for that instant, so a
/// blocking player's shield goes on cooldown as an axe's blow puts it (shield.rs:200-208,
/// 242-283). The axe's own `attack_damage` modifier is taken out of the mob's value for that blow
/// ([`base_for_blow`]), so the hit stays the data's blow. The mob's own numbers are put back after.
/// Whether the blow was dealt.
fn knock_blow(game: &mut Game<'_>, dim: &str, id: u32, target: u32, blow: f64, strength: f64, break_shield: bool) -> bool {
    let Some(i) = crate::game::soulscraft_find::index_of(game, id) else { return false };
    let m = &mut game.mobs[i];
    let was = (m.attrs.attack_damage, m.attrs.attack_knockback, m.extra.main_hand.clone());
    m.attrs.attack_knockback = strength;
    if break_shield {
        m.extra.main_hand = Some(std::sync::Arc::from(SHIELD_BREAKER));
    }
    let base = base_for_blow(game, i, blow);
    game.mobs[i].attrs.attack_damage = base;
    let landed = game.native_mob_attack(dim, id, target);
    if let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) {
        (m.attrs.attack_damage, m.attrs.attack_knockback, m.extra.main_hand) = was;
    }
    landed
}

/// The mob's own attack damage that makes its hit come to `blow` with what it holds now: the
/// engine's hit reads the value plus the main hand item's `attack_damage` modifiers from the item
/// data (`gear::attack_damage`, mobs/gear.rs:152-167; `attribute_with`, mobs/equip.rs:67-85). Those
/// are additions and multiplications of the value, so the hit is `a · value + b`: read at 0 and 1,
/// solved for `blow`. With nothing in hand it is `blow` itself. Leaves the value it last tried.
fn base_for_blow(game: &mut Game<'_>, i: usize, blow: f64) -> f32 {
    let b = hit_at(game, i, 0.0);
    let a = hit_at(game, i, 1.0) - b;
    if a.abs() < 1e-6 { blow as f32 } else { ((blow - b) / a) as f32 }
}

/// The hit of mob `i` (its value and what it holds) with its own attack damage set to `value`.
fn hit_at(game: &mut Game<'_>, i: usize, value: f32) -> f64 {
    game.mobs[i].attrs.attack_damage = value;
    f64::from(horizon_client::mobs::gear::attack_damage(&game.mobs[i], Some(game.items())))
}

/// The wave of a blow on the players it catches (the target the blow hit aside): from the impact
/// point (`wave.impact_ahead_blocks` in front of the mob), within `radius`, and for a `jumpable`
/// one only those not over it (soulscraft_rules::attack::wave_hits). True when one was hurt.
#[allow(clippy::too_many_arguments)]
fn deal_wave(
    game: &mut Game<'_>,
    dim: &str,
    m: &Seen,
    def: &MobDef,
    sc: &Scaled,
    a: &soulscraft_rules::data::AttackDef,
    radius: f64,
    blow: f64,
    hits_target: bool,
    target: Option<&Target>,
    players: &[(NativeServerPlayer, bool)],
) -> bool {
    let wave = a.wave.clone().unwrap_or_default();
    let wave_blow = wave.damage.map_or(blow, |k| sc.blow(k));
    let center = wave_center(m.pos, m.yaw, &wave);
    let mut landed = false;
    for (p, _) in players {
        let hit_already = target.is_some_and(|t| t.player == Some(p.uuid)) && hits_target;
        // The ground under the player matters only for a wave that can be jumped.
        let ground = if wave.clearance().is_some() { ground_under(game, p.position) } else { p.position[1] };
        if hit_already || !wave_hits(center, radius, wave.clearance(), p.position, ground) {
            continue;
        }
        if game.native_hurt_entity(dim, p.entity_id as u32, wave_blow as f32, "minecraft:mob_attack", Some(m.id)) {
            landed = true;
            crate::game::soulscraft_book::struck(&def.key, p.entity_id as u32, Some(p.uuid));
            give_effects(game, &wave.effects, Some(p.uuid));
        }
    }
    landed
}

/// The top of the ground under feet at `feet`: the highest collision box top at or below them in
/// their block or the one beneath. None there (feet a block or more over the ground) gives
/// minus infinity: they are over any wave.
fn ground_under(game: &Game<'_>, feet: V3) -> f64 {
    let (x, z) = (feet[0].floor() as i32, feet[2].floor() as i32);
    let top = feet[1].floor() as i32;
    for y in [top, top - 1] {
        let surface = game
            .native_block_collision([x, y, z])
            .iter()
            .map(|b| f64::from(y) + b[4])
            .filter(|s| *s <= feet[1] + 1e-3)
            .reduce(f64::max);
        if let Some(s) = surface {
            return s;
        }
    }
    f64::NEG_INFINITY
}

/// A later strike of a several-strike attack (the stalker's double slash, 3 ticks apart) would
/// fall in the target's hurt cooldown, where only what exceeds the last hit counts (players:
/// health.rs:406-446; mobs: `Mob.invulnerable`): the data means both blows to land, so the
/// cooldown is cleared first.
fn clear_hurt_cooldown(game: &mut Game<'_>, tg: &Target) {
    if tg.player.is_none() {
        if let Some(m) = crate::game::soulscraft_find::mob_mut(game, tg.entity) {
            m.invulnerable = 0;
        }
        return;
    }
    let Some(host) = game.lan.as_mut() else { return };
    let Some(g) = host.hub.guests.iter_mut().find(|g| g.entity_id as u32 == tg.entity) else { return };
    if let Some(motion) = g.motion.as_deref_mut() {
        motion.health.hurt.cooldown = 0;
    }
}

/// The marksman's bolt: an arrow from its eyes (its hitbox's eye height) at the target. An arrow deals its speed times
/// its `damage`, rounded up (vanilla `AbstractArrow`), so `damage` is the blow over the speed.
fn shoot(game: &mut Game<'_>, dim: &str, m: &Seen, tg: &Target, blow: f64, eye_height: f64) -> bool {
    const SPEED: f64 = 3.0;
    let eye = geom::add(m.pos, [0.0, eye_height, 0.0]);
    let aim = geom::add(tg.pos, [0.0, 1.2, 0.0]);
    let d = geom::sub(aim, eye);
    let l = geom::len(d).max(1e-3);
    let dir = geom::scale(d, 1.0 / l);
    // Gravity drops an arrow 0.05 a tick: aim a little high for the flight's ticks.
    let lift = 0.5 * 0.05 * (l / SPEED).powi(2) / l;
    let v = geom::scale([dir[0], dir[1] + lift, dir[2]], SPEED);
    let from = geom::add(eye, geom::scale(dir, 1.2));
    let snbt = format!(
        "{{Motion:[{:.4}d,{:.4}d,{:.4}d],damage:{:.4}d,pickup:0b}}",
        v[0],
        v[1],
        v[2],
        blow / SPEED
    );
    let nbt = match crate::mod_api::parse_snbt(&snbt) {
        Ok((crate::mod_api::Tag::Compound(c), _)) => c,
        _ => return false,
    };
    game.native_summon(dim, "minecraft:arrow", from, nbt).is_some()
}

/// The effects of a blow on a player: mobs.json names `bleed` (SoulsCraft's shared
/// `soulscraft:bleeding`) and vanilla effects by their path (`slowness`).
fn give_effects(game: &mut Game<'_>, effects: &[soulscraft_rules::data::EffectDef], player: Option<u128>) {
    let Some(uuid) = player else { return };
    for e in effects {
        let id = match e.id.as_str() {
            "bleed" => BLEEDING.to_owned(),
            other if other.contains(':') => other.to_owned(),
            other => format!("minecraft:{other}"),
        };
        let effect = NativeEffect {
            id,
            amplifier: e.level.unwrap_or(1).saturating_sub(1) as u8,
            duration: (e.seconds * 20.0).round() as i32,
            ambient: false,
            particles: true,
        };
        game.native_add_effect(uuid, &effect);
    }
}

/// The bog slime's contact: its `stick` passive's effects.
fn stick(game: &mut Game<'_>, def: &MobDef, tg: &Target, _id: u32) {
    let Some(p) = def.passive("stick") else { return };
    let effects: Vec<soulscraft_rules::data::EffectDef> = p
        .get("effects")
        .and_then(|v| serde_json_effects(v))
        .unwrap_or_default();
    give_effects(game, &effects, tg.player);
}

fn serde_json_effects(v: &serde_json::Value) -> Option<Vec<soulscraft_rules::data::EffectDef>> {
    v.as_array()?
        .iter()
        .map(|e| {
            Some(soulscraft_rules::data::EffectDef {
                id: e.get("id")?.as_str()?.to_owned(),
                seconds: e.get("seconds").and_then(serde_json::Value::as_f64).unwrap_or(0.0),
                level: e.get("level").and_then(serde_json::Value::as_u64).map(|l| l as u32),
            })
        })
        .collect()
}

/// A tracked mob died: logged once; the bog slime splits (`passives.split`).
fn on_death(game: &mut Game<'_>, dim: &str, now: u64, m: &Seen) {
    let (def, p, size) = {
        let mut s = state();
        let Some(t) = s.mobs.get_mut(&m.id) else { return };
        if !t.alive {
            return;
        }
        t.alive = false;
        let kind = t.kind;
        let (p, size) = (t.p, t.size);
        (s.data.mobs[kind].clone(), p, size)
    };
    GUARDS.lock().unwrap_or_else(PoisonError::into_inner).shields.remove(&(m.id as i32));
    crate::game::soulscraft_cover::forget(m.id);
    state().turns.release(m.id);
    GRABS.lock().unwrap_or_else(PoisonError::into_inner).remove(&(m.id as i32));
    if def.stats.flying {
        // Should the death hook have missed it: the body falls.
        let _ = drop_dead_flyer(game, m.id);
    }
    log::info!("soulscraft: death mob={} id={} tick={now}", def.id, m.id);
    let Some(split) = def.passive("split") else { return };
    let child = size / 2;
    if child == 0 {
        return;
    }
    let (lo, hi) = match split.get("spawn_count").and_then(serde_json::Value::as_array) {
        Some(v) if v.len() == 2 => (v[0].as_u64().unwrap_or(2), v[1].as_u64().unwrap_or(3)),
        _ => (2, 3),
    };
    let n = lo + u64::from(m.id) % (hi - lo + 1);
    for i in 0..n {
        let a = i as f64 * std::f64::consts::TAU / n as f64;
        let at = [m.pos[0] + 0.6 * a.sin(), m.pos[1] + 0.2, m.pos[2] + 0.6 * a.cos()];
        crate::game::soulscraft_tools::summon(game, dim, &def.id, at, p, child);
    }
}

/// `Game::mob_death_canceled` at its head (hook_mob_death.rs): a dead flyer of ours (the shrike,
/// `stats.flying`) loses the no-gravity its steering needed, so its body falls while it dies.
pub(super) fn on_mob_death(game: &mut Game<'_>, mob: &horizon_client::mobs::Mob) {
    if !mob.kind.id.starts_with("soulscraft:") {
        return;
    }
    // A death inside one of this module's own calls (the state held) is left to on_death, a tick
    // later, rather than waiting on the lock.
    let Ok(s) = STATE.try_lock() else { return };
    let flying = s.data.by_key(&mob.kind.id).is_some_and(|(_, d)| d.stats.flying);
    drop(s);
    if flying && drop_dead_flyer(game, mob.id) {
        log::log!(chatty(), "soulscraft: dead flyer falls mob={} id={}", mob.kind.id, mob.id);
    }
}

/// Gravity back on mob `id` (adopt turned it off for flyers). True when it was off.
fn drop_dead_flyer(game: &mut Game<'_>, id: u32) -> bool {
    let Some(m) = crate::game::soulscraft_find::mob_mut(game, id) else { return false };
    if !m.no_gravity {
        return false;
    }
    m.no_gravity = false;
    m.needs_sync = true;
    true
}

fn forget_gone(dim: &str, now: u64) {
    let mut s = state();
    let gone: Vec<u32> = s
        .mobs
        .iter()
        .filter(|(_, t)| t.dim == dim && now.saturating_sub(t.seen) >= FORGET_AFTER)
        .map(|(id, _)| *id)
        .collect();
    for id in gone {
        s.mobs.remove(&id);
        s.turns.release(id);
        GRABS.lock().unwrap_or_else(PoisonError::into_inner).remove(&(id as i32));
    }
}

fn send_notes(
    game: &mut Game<'_>,
    dim: &str,
    now: u64,
    players: &[(NativeServerPlayer, bool)],
    mut notes: Vec<wire::Note>,
) {
    if now % RESEND_EVERY == 0 {
        let s = state();
        let have: Vec<u32> = notes.iter().map(|n| n.id).collect();
        for (id, t) in s.mobs.iter().filter(|(id, t)| t.dim == dim && t.alive && !have.contains(id)) {
            if let Some((clip, start, flags)) = &t.sent {
                notes.push(wire::Note { id: *id, clip: clip.clone(), start: *start, flags: *flags, size: t.size, label: String::new() });
            }
        }
    }
    if notes.is_empty() {
        return;
    }
    let messages = wire::encode(&wire::Batch { now: now as i64, notes });
    // Every player of the server, not only of this level's hunters: creative watchers see too.
    let everyone: Vec<u128> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dim)
        .map(|p| p.uuid)
        .collect();
    let _ = players;
    let mut errors = Vec::new();
    for uuid in &everyone {
        for bytes in &messages {
            if let Err(e) = game.native_net_send_to_player(*uuid, wire::STATE, bytes) {
                errors.push(e);
            }
        }
    }
    if now % 200 == 0 || !errors.is_empty() && now % 20 == 0 {
        // Mod messages reach only players whose client says it is Horizon's (`brand`,
        // game/native_net.rs:124-135): the brands are logged with the count.
        let brands: Vec<String> = game
            .lan
            .as_ref()
            .map(|h| h.hub.guests.iter().map(|g| format!("{}:{}", g.name, g.brand)).collect())
            .unwrap_or_default();
        log::log!(
            chatty(),
            "soulscraft: notes tick={now} messages={} players={} errors={:?} brands={:?}",
            messages.len(),
            everyone.len(),
            errors.first(),
            brands
        );
    }
}

/// The damage hook: a blow on a guarding revenant from in front of it is cut by its shield
/// (`params.shield_front_reduction`, 0.8). The attacker is `causing` when the engine names it,
/// else the nearest player within 6 blocks (v1 may not know it: see horizon-mixin.toml).
pub(super) fn on_incoming_damage(entity: i32, kind: &str, damage_type: &str, causing: i32, amount: &mut f32) {
    // A hit on a wave mob breaks its drain and may turn it on the participant (Г12).
    crate::game::soulscraft_event::on_damage(entity, causing);
    note_grab_blow(entity, damage_type, causing);
    if kind != "soulscraft:revenant" || !matches!(damage_type, "minecraft:player_attack" | "minecraft:mob_attack" | "minecraft:arrow") {
        return;
    }
    let g = GUARDS.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(&(pos, yaw, reduction)) = g.shields.get(&entity) else { return };
    let from = g
        .players
        .iter()
        .find(|(id, _)| causing != 0 && *id == causing)
        .or_else(|| {
            g.players
                .iter()
                .filter(|(_, p)| geom::dist(*p, pos) <= 6.0)
                .min_by(|a, b| geom::dist(a.1, pos).total_cmp(&geom::dist(b.1, pos)))
        })
        .map(|(_, p)| *p);
    let Some(from) = from else { return };
    if geom::off_facing(pos, yaw, from) <= 60.0 {
        let before = *amount;
        *amount *= (1.0 - reduction) as f32;
        log::info!("soulscraft: blocked mob=revenant id={entity} dmg={before:.2}->{:.2}", *amount);
    }
}

/// The self-test's stand-in for a player who hits a guarding revenant (`revenant_shield_front`):
/// the damage hook's only player, at `pos`, until the next level tick names the real ones.
pub(super) fn stand_in_player(pos: V3) {
    GUARDS.lock().unwrap_or_else(PoisonError::into_inner).players = vec![(0, pos)];
}

/// soulscraft:bleeding hurts every `period_ticks` (data/mobs.json `effects.bleeding`).
pub(super) fn bleed_due(effect: &str, tick_count: i32, _amplifier: i32, apply: &mut bool) {
    if effect != BLEEDING {
        return;
    }
    let period = bleed_numbers().1.max(1);
    *apply = tick_count % period == 0;
}

/// (damage per hit, period in ticks) of the bleed.
pub(super) fn bleed_numbers() -> (f64, i32) {
    let s = state();
    let e = s.data.effects.get("bleeding");
    (
        e.and_then(|e| e.get("damage")).and_then(serde_json::Value::as_f64).unwrap_or(1.0),
        e.and_then(|e| e.get("period_ticks")).and_then(serde_json::Value::as_i64).unwrap_or(20) as i32,
    )
}

/// The bleed's tick, inside the entity's own (the game is read-only there, native_hooks.rs:
/// 9176-9182): the hit is queued for the level's next tick.
pub(super) fn bleed_tick(game: &Game<'_>, effect: &str, dimension: &str, entity: Option<i32>, amplifier: i32) {
    if effect != BLEEDING {
        return;
    }
    let player = if entity.is_none() { game.native_acting_player().map(|p| p.uuid) } else { None };
    state().bleeds.push((dimension.to_owned(), entity, player, amplifier));
}

fn hurt_bleeders(game: &mut Game<'_>, dim: &str) {
    let due: Vec<_> = {
        let mut s = state();
        let (mine, rest): (Vec<_>, Vec<_>) = s.bleeds.drain(..).partition(|b| b.0 == dim);
        s.bleeds = rest;
        mine
    };
    if due.is_empty() {
        return;
    }
    let (damage, _) = bleed_numbers();
    let players = game.native_server_players();
    for (_, entity, player, amplifier) in due {
        let id = match (entity, player) {
            (Some(e), _) => e as u32,
            (None, Some(u)) => match players.iter().find(|p| p.uuid == u) {
                Some(p) => p.entity_id as u32,
                None => continue,
            },
            _ => continue,
        };
        let amount = damage * f64::from(amplifier + 1);
        game.native_hurt_entity(dim, id, amount as f32, "minecraft:magic", None);
    }
}


/// The zombie template turns a mob whose eyes stay under water 600 + 300 ticks into a vanilla
/// drowned (mobs/undead.rs:610-646, `extra.counter` and `counter2` for the zombie species), which
/// would end a SoulsCraft mob: its type, rig and P. The counter is held at its "not in water" value
/// after each mob tick (docs/research/horizon-mob-models.md, 12).
fn no_drowned_conversion(game: &mut Game<'_>, id: u32) {
    if let Some(m) = crate::game::soulscraft_find::mob_mut(game, id)
        && (m.extra.counter >= 0 || m.extra.counter2 != 0)
    {
        m.extra.counter = -1;
        m.extra.counter2 = 0;
    }
}
