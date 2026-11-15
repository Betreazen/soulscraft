// The approved vanilla changes on the server (docs/design/07-vanilla-mobs.md; the decisions are
// soulscraft_rules::vanilla). Every mob of a changed vanilla type (data/mobs.json `vanilla.groups`)
// is adopted the first time the level tick sees it:
//   - its P (Г1, Г5: the players near, or its tag `soulscraft.p=`), kept as that tag;
//   - Г2's growth as attribute modifiers `soulscraft:power` on its own vanilla base (health, damage,
//     speed) and armour on its type's (`armor_attributes`, as the server half does for its own);
//     health keeps its fraction of the maximum (a fresh mob is full); Path-of-craft-tree's
//     `pct.xpNNN` from its vanilla health, so PCT pays the vanilla
//     kind by its own table times Г3's bonus;
//   - a mob within `spawner_radius_blocks` of a spawner grows but gets no moves (Г4: farms keep
//     working), tag `soulscraft.spawner`;
//   - a wave mob of an altar's event (tag `soulscraft.wave`) is the event's and is left alone.
// Undead stay undead (round 11): no flag, tag or species is touched. Vanilla mobs keep their models,
// goals and target; a gate goal (`SoulscraftGate`, highest priority, not interruptable) holds the
// moving and attacking goals while the mind says `control`, and a second (`SoulscraftLeapGate`)
// holds only the jump slot (a spider's own leap) near its target. Their windups show with vanilla's
// own means: the `aggressive` pose, sounds, particles, a cube's hop; `/sc debug on` labels them.
//
// Log lines:
//   soulscraft: vadopt mob=minecraft:zombie id=12 group=zombie p=0.40 spawner=false health=27.2 tick=..
//   soulscraft: vphase mob=minecraft:zombie id=12 attack=lunge phase=hit t=10 tick=..
//   soulscraft: vstrike mob=minecraft:zombie id=12 attack=lunge t=10 tick=.. hit=true target=sc_a
use crate::game::Game;
use crate::mod_api::NativeServerPlayer;
use horizon_client::mobs::finalize::{self, Attr, Op};
use horizon_client::mobs::goals::{Cx, Goal, JUMP, LOOK, MOVE};
use horizon_client::mobs::{Mob, Who};
use horizon_client::particles::ParticleOptions;
use soulscraft_rules::{
    attack::Phase,
    brain::{Event, Order, Strike},
    cover::{self, Threat},
    fit,
    geom::{self, V3},
    scale::{self, Scaled},
    vanilla::{Behavior, Flank, VBrain, VSenses, VTick, VanillaDef, Want},
    wire,
};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

const TAG_P: &str = "soulscraft.p=";
const TAG_SPAWNER: &str = "soulscraft.spawner";
const TAG_WAVE: &str = "soulscraft.wave";
const TAG_FRESH: &str = "soulscraft.fresh";
/// The modifier id of Г2's growth on a vanilla mob's attributes.
const POWER_MOD: &str = "soulscraft:power";
/// A target farther than this is no target for the moves.
const FOLLOW: f64 = 32.0;
/// A walk is asked a new path at most this often unless it moved.
const REPATH_EVERY: u64 = 10;
/// A tracked mob not seen for this long left: forgotten.
const FORGET_AFTER: u64 = 40;
const LOG_KEEP: usize = 512;
/// Sound source `hostile` (`SoundSource` index).
const HOSTILE: usize = 5;
/// A thrown trident's speed (vanilla `Drowned.performRangedAttack`: 1.6).
const TRIDENT_SPEED: f64 = 1.6;
/// A thrown potion's speed (vanilla `Witch.performRangedAttack`: 0.75).
const POTION_SPEED: f64 = 0.75;
/// A bolt's speed (vanilla crossbow: 3.15).
const BOLT_SPEED: f64 = 3.15;
/// What the cover and the high spot search use as the target's eyes.
const EYE: f64 = 1.62;
/// Gate priorities: before every vanilla goal.
const GATE_PRIORITY: i32 = -1000;

/// One vanilla mob the server knows.
pub(super) struct VMob {
    pub group: usize,
    pub kind: String,
    pub p: f64,
    /// None for a spawner's mob (Г4: it grows, no moves).
    pub brain: Option<VBrain>,
    pub dim: String,
    pub seen: u64,
    was_swinging: bool,
    was_swoop: bool,
    answer: Option<Option<V3>>,
    path: Option<(V3, u64)>,
    held: u8,
    label: String,
}

#[derive(Clone, Debug)]
pub(super) struct VRecord {
    pub mob: u32,
    pub attack: String,
    pub phase: Option<Phase>,
    pub t: u32,
    pub tick: u64,
    pub hit: bool,
}

#[derive(Default)]
pub(super) struct VState {
    pub mobs: HashMap<u32, VMob>,
    /// Mobs seen and left alone (wave mobs).
    pub skipped: HashSet<u32>,
    /// The self-test's targets: mob → entity.
    pub targets: HashMap<u32, u32>,
    pub phases: Vec<VRecord>,
    pub strikes: Vec<VRecord>,
    /// (mob, attack, tick) of each cue played.
    pub cues: Vec<(u32, String, u64)>,
    pub modes: Vec<(u32, String, u64)>,
    /// (mob, flank label, tick) of each flank order.
    pub flanks: Vec<(u32, String, u64)>,
}

static STATE: LazyLock<Mutex<VState>> = LazyLock::new(Mutex::default);
/// The gate flags each mob's gates hold now (read inside the engine's goal tick).
static HELD: LazyLock<Mutex<HashMap<u32, u8>>> = LazyLock::new(Mutex::default);

pub(super) fn state() -> MutexGuard<'static, VState> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn held(id: u32) -> u8 {
    HELD.lock().unwrap_or_else(PoisonError::into_inner).get(&id).copied().unwrap_or(0)
}

/// The goal that parks a vanilla mob's own goals of `flags` while the mind holds the mob: it wins
/// their slots (priority before all, never interrupted) and gives them back when released.
#[derive(Clone, Debug)]
struct Gate {
    flags: u8,
}

impl Goal for Gate {
    fn flags(&self) -> u8 {
        self.flags
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        held(m.id) & self.flags == self.flags
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        if self.flags == JUMP { "SoulscraftLeapGate" } else { "SoulscraftGate" }
    }
}

/// A vanilla mob of a changed type, as the tick reads it.
struct Seen {
    id: u32,
    kind: String,
    group: usize,
    pos: V3,
    yaw: f64,
    health: f64,
    max_health: f64,
    on_ground: bool,
    in_water: bool,
    target: Option<Who>,
    swinging: bool,
    using_item: bool,
    trident: bool,
    swoop: bool,
    slowed: bool,
    size: i32,
    eye: f64,
    width: f64,
    height: f64,
}

/// A target resolved: its entity, feet and name.
#[derive(Clone)]
struct Target {
    entity: u32,
    pos: V3,
    name: String,
}

/// After SoulsCraft's own mobs on the level tick: adopt new vanilla mobs, drive the known ones.
pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64, hunters: &[(NativeServerPlayer, bool)]) {
    let players: Vec<&NativeServerPlayer> = hunters.iter().map(|(p, _)| p).collect();
    let groups = crate::game::soulscraft_server::state().data.vanilla.groups.clone();
    if groups.is_empty() {
        return;
    }
    let index: HashMap<&str, usize> = groups.iter().enumerate().flat_map(|(i, g)| g.types.iter().map(move |t| (t.as_str(), i))).collect();
    let seen: Vec<Seen> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.kind.id.starts_with("minecraft:"))
        .filter_map(|m| {
            let group = *index.get(m.kind.id.as_str())?;
            Some(Seen {
                id: m.id,
                kind: m.kind.id.to_string(),
                group,
                pos: [m.position.x, m.position.y, m.position.z],
                yaw: f64::from(m.yaw),
                health: f64::from(m.health),
                max_health: f64::from(m.attrs.max_health),
                on_ground: m.on_ground,
                in_water: m.fluids.in_water,
                target: m.ai.target,
                swinging: m.ai.swinging,
                using_item: m.extra.using_item > 0,
                trident: m.extra.main_hand.as_deref() == Some("minecraft:trident"),
                swoop: m.extra.flag,
                slowed: m.has_effect("minecraft:slowness"),
                size: m.extra.size,
                eye: m.kind.eye_height,
                width: m.kind.width,
                height: m.kind.height,
            })
        })
        .collect();
    for m in &seen {
        let known = {
            let s = state();
            s.mobs.contains_key(&m.id) || s.skipped.contains(&m.id)
        };
        if !known {
            adopt(game, dim, now, m, &groups[m.group], &players);
        }
    }
    let targets: HashMap<u32, Target> = seen.iter().filter_map(|m| Some((m.id, target_of(game, m, &players)?))).collect();
    let flanks = flanks(&seen, &targets, &groups);
    let debug = crate::game::soulscraft_server::state().debug;
    let mut notes = Vec::new();
    for m in &seen {
        tick_mob(game, dim, now, m, &groups[m.group], targets.get(&m.id), flanks.get(&m.id).copied(), debug, &mut notes);
    }
    forget_gone(dim, now);
    if !notes.is_empty() {
        send_labels(game, dim, now, notes);
    }
}

/// Wave mobs of an altar's event are the event's.
fn is_wave(id: u32, tags: &[String]) -> bool {
    tags.iter().any(|t| t == TAG_WAVE) || crate::game::soulscraft_event::is_wave(id)
}

fn adopt(game: &mut Game<'_>, dim: &str, now: u64, m: &Seen, g: &VanillaDef, players: &[&NativeServerPlayer]) {
    let entity = game.native_entity(m.id);
    let tags = entity.and_then(|e| game.native_entity_tags(e)).unwrap_or_default();
    if is_wave(m.id, &tags) {
        state().skipped.insert(m.id);
        return;
    }
    let tagged = tags.iter().find_map(|t| t.strip_prefix(TAG_P)).and_then(|v| v.parse::<f64>().ok());
    // First seen: no P yet, or a spawn by /sc or the self-test that names its P (`soulscraft.fresh`).
    let marked = tags.iter().any(|t| t == TAG_FRESH);
    let fresh = tagged.is_none() || marked;
    if marked && let Some(e) = entity {
        game.native_entity_remove_tag(e, TAG_FRESH);
    }
    let p = match tagged {
        Some(p) => scale::clamp_p(p),
        None => {
            let near: Vec<_> = players.iter().map(|p| (p.position, p.uuid)).collect();
            let p = crate::game::soulscraft_power::power_here(game, m.pos, &near);
            if let Some(e) = entity {
                game.native_entity_add_tag(e, &format!("{TAG_P}{p:.2}"));
            }
            p
        }
    };
    let spawner = if fresh {
        let radius = crate::game::soulscraft_server::state().data.vanilla.spawner_radius_blocks;
        let near = spawner_near(game, m.pos, radius);
        if near && let Some(e) = entity {
            game.native_entity_add_tag(e, TAG_SPAWNER);
        }
        near
    } else {
        tags.iter().any(|t| t == TAG_SPAWNER)
    };
    let base = apply_power(game, m.id, p);
    if let Some((base_health, max)) = base {
        crate::game::soulscraft_server::set_xp_tag(game, m.id, base_health, max);
    }
    if !spawner && let Some(mob) = game.mobs.iter_mut().find(|x| x.id == m.id) {
        if !mob.ai.goals.has("SoulscraftGate") {
            mob.ai.goals.add(GATE_PRIORITY, Gate { flags: MOVE | LOOK | JUMP });
            mob.ai.goals.add(GATE_PRIORITY + 1, Gate { flags: JUMP });
        }
    }
    let brain = (!spawner).then(|| VBrain::new(g, &m.kind, u64::from(m.id), now));
    state().mobs.insert(
        m.id,
        VMob {
            group: m.group,
            kind: m.kind.clone(),
            p,
            brain,
            dim: dim.to_owned(),
            seen: now,
            was_swinging: m.swinging,
            was_swoop: m.swoop,
            answer: None,
            path: None,
            held: 0,
            label: String::new(),
        },
    );
    log::log!(
        crate::game::soulscraft_server::chatty(),
        "soulscraft: vadopt mob={} id={} group={} p={p:.2} spawner={spawner} health={:.1} tick={now}",
        m.kind,
        m.id,
        g.id,
        base.map_or(m.health, |b| b.1)
    );
}

/// A spawner block (or a trial spawner) within `radius` blocks round and 2 up or down.
fn spawner_near(game: &Game<'_>, at: V3, radius: i32) -> bool {
    let c = [at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32];
    for dy in -2..=2 {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let b = game.native_block([c[0] + dx, c[1] + dy, c[2] + dz]);
                if b.ends_with("spawner") && b.contains("minecraft:") {
                    return true;
                }
            }
        }
    }
    false
}

/// Г2 on a vanilla mob: modifiers on its own base (a slime's size², a baby's speed stay its own),
/// armour on its type's. Returns (base health, scaled max health) for PCT's tag.
fn apply_power(game: &mut Game<'_>, id: u32, p: f64) -> Option<(f64, f64)> {
    let i = game.mobs.iter().position(|m| m.id == id)?;
    let base_armor = f64::from(game.mobs[i].kind.armor);
    let kind = game.mobs[i].kind.id.to_string();
    {
        let m = &mut game.mobs[i];
        let old_max = m.attrs.max_health;
        finalize::add_modifier(m, Attr::MaxHealth, POWER_MOD, 0.9 * p, Op::AddMultipliedBase);
        finalize::add_modifier(m, Attr::AttackDamage, POWER_MOD, 0.6 * p, Op::AddMultipliedBase);
        finalize::add_modifier(m, Attr::MovementSpeed, POWER_MOD, 0.10 * p, Op::AddMultipliedBase);
        // The same fraction of the grown maximum: a mob set low by another mod (RustCraft's test
        // spider at 2 health) stays low, a fresh full one is full; never above its proportion.
        m.health = scale::kept_health(m.health, old_max, m.attrs.max_health);
        m.needs_sync = true;
    }
    if p > 0.0 {
        if game.mobs[i].extra.armor_attributes.is_none() {
            game.mobs[i].extra.armor_attributes = game.motion.attributes.create(&kind).map(Box::new);
        }
        if let Some(map) = game.mobs[i].extra.armor_attributes.as_deref_mut()
            && let Some(key) = map.catalog().id("minecraft:armor")
        {
            map.edit(key, |value, observer| {
                let _ = value.set_base_value(base_armor + 6.0 * p, observer);
            });
        }
    }
    let m = &game.mobs[i];
    Some((finalize::base_value(m, Attr::MaxHealth), f64::from(m.attrs.max_health)))
}

/// Vanilla's target as an entity: a server's player (by its entity, or `Who::Player`, the guest the
/// mob tracks: the nearest), or a mob; the self-test's stand-in first.
fn target_of(game: &Game<'_>, m: &Seen, players: &[&NativeServerPlayer]) -> Option<Target> {
    let forced = state().targets.get(&m.id).copied();
    let who = match forced {
        Some(e) => Some(Who::Mob(e)),
        None => m.target,
    }?;
    let player = |p: &&NativeServerPlayer| Target { entity: p.entity_id as u32, pos: p.position, name: p.name.clone() };
    let t = match who {
        Who::Player => players
            .iter()
            .filter(|p| p.health > 0.0 && p.game_mode != 3)
            .min_by(|a, b| geom::dist(a.position, m.pos).total_cmp(&geom::dist(b.position, m.pos)))
            .map(player)?,
        Who::Mob(e) => match players.iter().find(|p| p.entity_id as u32 == e) {
            Some(p) => player(p),
            None => {
                let x = game.mobs.iter().find(|x| x.id == e && !x.removed && x.alive())?;
                Target { entity: e, pos: [x.position.x, x.position.y, x.position.z], name: format!("mob{e}") }
            }
        },
    };
    (geom::dist(t.pos, m.pos) <= FOLLOW).then_some(t)
}

/// The brawlers that go for the same target: the nearest leads, the others flank in turn.
fn flanks(seen: &[Seen], targets: &HashMap<u32, Target>, groups: &[VanillaDef]) -> HashMap<u32, Flank> {
    let mut by_target: HashMap<u32, Vec<(f64, u32, V3)>> = HashMap::new();
    let s = state();
    for m in seen {
        if groups[m.group].behavior() != Behavior::Brawler || !s.mobs.get(&m.id).is_some_and(|v| v.brain.is_some()) {
            continue;
        }
        let Some(t) = targets.get(&m.id) else { continue };
        let range = groups[m.group].param("flank_search_blocks").unwrap_or(16.0);
        let d = geom::dist(m.pos, t.pos);
        if d <= range {
            by_target.entry(t.entity).or_default().push((d, m.id, m.pos));
        }
    }
    let mut out = HashMap::new();
    for (_, mut list) in by_target {
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let leader = list[0].2;
        let count = list.len();
        for (index, (_, id, _)) in list.into_iter().enumerate() {
            out.insert(id, Flank { index, count, leader });
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn tick_mob(
    game: &mut Game<'_>,
    dim: &str,
    now: u64,
    m: &Seen,
    g: &VanillaDef,
    target: Option<&Target>,
    flank: Option<Flank>,
    debug: bool,
    notes: &mut Vec<wire::Note>,
) {
    let (sense, decide_every) = {
        let mut s = state();
        let Some(v) = s.mobs.get_mut(&m.id) else { return };
        v.seen = now;
        v.dim = dim.to_owned();
        if v.brain.is_none() {
            return;
        }
        let swung = m.swinging && !v.was_swinging;
        let swoop = m.swoop && !v.was_swoop;
        v.was_swinging = m.swinging;
        v.was_swoop = m.swoop;
        let sense = VSenses {
            now,
            pos: m.pos,
            yaw: m.yaw,
            on_ground: m.on_ground,
            in_water: m.in_water,
            health_frac: if m.max_health > 0.0 { m.health / m.max_health } else { 1.0 },
            target: target.map(|t| t.pos),
            sees_target: false,
            slowed: m.slowed,
            swung,
            using_item: m.using_item,
            holding_trident: m.trident,
            swoop_started: swoop && g.behavior() == Behavior::Phantom,
            flank,
            size: m.size,
            answer: v.answer.take(),
        };
        (sense, Scaled::at(&soulscraft_rules::data::Stats::default(), v.p).decide_every)
    };
    let mut sense = sense;
    if let Some(t) = target
        && matches!(g.behavior(), Behavior::Witch | Behavior::Crossbow | Behavior::Brawler)
        && (g.behavior() != Behavior::Brawler || m.trident)
    {
        let eye = geom::add(m.pos, [0.0, m.eye, 0.0]);
        sense.sees_target = crate::game::soulscraft_cover::sees(game, eye, geom::add(t.pos, [0.0, EYE, 0.0]));
    }
    let tick = {
        let mut s = state();
        let Some(v) = s.mobs.get_mut(&m.id) else { return };
        let Some(b) = v.brain.as_mut() else { return };
        b.tick(g, decide_every, &sense)
    };
    log_events(g, m, now, &tick);
    hold(game, dim, m.id, &tick);
    if let Some(want) = tick.want {
        let answer = target.and_then(|t| find_spot(game, m, g, t, want));
        if let Some(v) = state().mobs.get_mut(&m.id) {
            v.answer = Some(answer);
        }
    }
    if let Some(idx) = tick.cue {
        cue(game, dim, now, m, g, idx);
    }
    // A cube in its windup stands still: what is left of its own hop stops, on the ground or in the
    // air (its cue hop is straight up).
    let in_windup = || {
        let s = state();
        let run = s.mobs.get(&m.id).and_then(|v| v.brain.as_ref()?.attack(now));
        run.is_some_and(|(i, t)| g.attacks.get(i).is_some_and(|a| t < a.def.windup))
    };
    let slide = g.behavior() == Behavior::Cube && tick.launch.is_none() && in_windup();
    drive(game, dim, now, m, &tick, slide);
    for strike in &tick.strikes {
        deal(game, dim, now, m, g, strike, target);
    }
    if debug {
        let mut s = state();
        if let Some(v) = s.mobs.get_mut(&m.id)
            && v.label != tick.label
        {
            v.label = tick.label.clone();
            notes.push(wire::Note { id: m.id, clip: String::new(), start: now as i64, flags: wire::DEBUG, size: 0, label: tick.label.clone() });
        }
    }
}

/// The gates' flags, the pose, the phantom's held swoop.
fn hold(game: &mut Game<'_>, dim: &str, id: u32, tick: &VTick) {
    let flags = if tick.control { MOVE | LOOK | JUMP } else if tick.no_leap { JUMP } else { 0 };
    let was = {
        let mut s = state();
        let Some(v) = s.mobs.get_mut(&id) else { return };
        std::mem::replace(&mut v.held, flags)
    };
    {
        let mut h = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        if flags == 0 {
            h.remove(&id);
        } else {
            h.insert(id, flags);
        }
    }
    if tick.control && was & MOVE == 0 {
        // Taken over: vanilla's path ends now (its goals stop on their next tick).
        game.native_mob_stop(dim, id);
        if let Some(v) = state().mobs.get_mut(&id) {
            v.path = None;
        }
    }
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    if let Some(a) = tick.aggressive
        && m.ai.aggressive != a
    {
        m.ai.aggressive = a;
        m.needs_sync = true;
    }
    if tick.hold_dive && m.extra.flag {
        // The phantom keeps circling while it cries (its swoop is `extra.flag`, mobs/phantom.rs).
        m.extra.flag = false;
    }
    if tick.release_dive {
        m.extra.flag = true;
    }
}

/// The mind's order as the engine's calls, only while it holds the mob.
fn drive(game: &mut Game<'_>, dim: &str, now: u64, m: &Seen, tick: &VTick, slide: bool) {
    if let Some(v) = tick.launch {
        set_velocity(game, m.id, v, true);
    }
    if !tick.control {
        return;
    }
    if let Some(at) = tick.look_at {
        game.native_mob_look_at(dim, m.id, at);
    }
    let path = state().mobs.get(&m.id).and_then(|v| v.path);
    let path = match tick.order {
        Order::Stay => {
            stand(game, dim, m.id, slide);
            None
        }
        Order::Walk { to, speed } => {
            let stale = path.is_none_or(|(at, when)| geom::dist(at, to) > 1.5 || now - when >= REPATH_EVERY);
            if stale || !game.native_mob_navigating(dim, m.id) {
                game.native_mob_navigate(dim, m.id, to, 0, speed);
                Some((to, now))
            } else {
                path
            }
        }
        Order::Dash { dir, per_tick } => {
            stand(game, dim, m.id, false);
            set_velocity(game, m.id, geom::scale(dir, per_tick), false);
            None
        }
        Order::Steer { .. } | Order::Leap { .. } => None,
    };
    if let Some(v) = state().mobs.get_mut(&m.id) {
        v.path = path;
    }
}

/// Holds a mob where it is for this tick: its path ends and its move control waits (a vanilla goal
/// that ran before the gate took its slot may have set both; the gate holds from the next full
/// goal tick). Each tick, because vanilla's goals of the mob may still be ticking this one. `slide`
/// also stops what is left of a cube's own hop along the ground and in the air.
fn stand(game: &mut Game<'_>, dim: &str, id: u32, slide: bool) {
    game.native_mob_stop(dim, id);
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    m.ai.mov.operation = horizon_client::mobs::control::Operation::Wait;
    m.ai.zza = 0.0;
    m.ai.xxa = 0.0;
    if slide && (m.velocity.x != 0.0 || m.velocity.z != 0.0) {
        m.velocity.x = 0.0;
        m.velocity.z = 0.0;
        m.needs_sync = true;
    }
}

fn set_velocity(game: &mut Game<'_>, id: u32, v: V3, with_y: bool) {
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
        m.velocity.x = v[0];
        m.velocity.z = v[2];
        if with_y {
            m.velocity.y = v[1];
        }
        m.needs_sync = true;
    }
}

/// The windup's cue: its sound, particles round the mob and a cube's hop.
fn cue(game: &mut Game<'_>, dim: &str, now: u64, m: &Seen, g: &VanillaDef, idx: usize) {
    let Some(a) = g.attacks.get(idx) else { return };
    let c = a.cue.for_type(&m.kind);
    let at = geom::add(m.pos, [0.0, m.height * 0.6, 0.0]);
    if let Some(sound) = &c.sound {
        let pitch = c.pitch.unwrap_or(1.0);
        game.with_level(|_, level, _| level.native_play_sound(sound, at, HOSTILE, 1.0, pitch));
    }
    if let Some(kind) = &c.particle {
        let options = ParticleOptions::named(kind.clone());
        let spread = [m.width * 0.6, m.height * 0.3, m.width * 0.6];
        let feet = geom::add(m.pos, [0.0, 0.2, 0.0]);
        game.with_level(|_, level, _| level.native_send_particles(&options, None, feet, spread, 0.02, 12));
    }
    if let Some(up) = c.hop
        && m.on_ground
    {
        set_velocity(game, m.id, [0.0, up, 0.0], true);
    }
    let _ = dim;
    state().cues.push((m.id, a.def.id.clone(), now));
}

/// One blow: vanilla's own hit (`native_mob_attack`: its damage with Г2's growth, a husk's hunger,
/// a wither skeleton's wither, the knockback) at the attack's multiplier; a projectile; a landing.
fn deal(game: &mut Game<'_>, dim: &str, now: u64, m: &Seen, g: &VanillaDef, strike: &Strike, target: Option<&Target>) {
    let Some(a) = g.attacks.get(strike.attack) else { return };
    let a = &a.def;
    let t = state().mobs.get(&m.id).and_then(|v| v.brain.as_ref()?.attack(now)).map_or(0, |x| x.1);
    let mut landed = false;
    if strike.ranged {
        if let Some(tg) = target {
            let damage = game.mobs.iter().find(|x| x.id == m.id).map_or(0.0, |x| f64::from(x.attrs.attack_damage));
            landed = throw(game, dim, m, tg, a.projectile.as_deref().unwrap_or("bolt"), damage * a.damage);
        }
    } else if strike.hits_target
        && let Some(tg) = target
    {
        landed = blow(game, dim, m.id, tg.entity, a.damage, a.strikes_at.as_ref().is_some_and(|s| s.len() > 1));
    }
    if let Some(radius) = strike.area {
        let near: Vec<u32> = game
            .native_server_players()
            .into_iter()
            .filter(|p| p.dimension == dim && p.health > 0.0 && geom::dist(p.position, m.pos) <= radius)
            .map(|p| p.entity_id as u32)
            .filter(|e| !(strike.hits_target && target.is_some_and(|t| t.entity == *e)))
            .collect();
        for e in near {
            landed |= blow(game, dim, m.id, e, a.damage, false);
        }
    }
    let who = target.map_or("-", |t| t.name.as_str());
    log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: vstrike mob={} id={} attack={} t={t} tick={now} hit={landed} target={who}", m.kind, m.id, a.id);
    let mut s = state();
    s.strikes.push(VRecord { mob: m.id, attack: a.id.clone(), phase: None, t, tick: now, hit: landed });
    let n = s.strikes.len();
    if n > LOG_KEEP {
        s.strikes.drain(..n - LOG_KEEP);
    }
}

/// Vanilla's hit of `mob` on `target`, its damage times `mult` for this one blow. A later blow of a
/// series would fall in the target's hurt cooldown: it is cleared first (as the server half does).
fn blow(game: &mut Game<'_>, dim: &str, mob: u32, target: u32, mult: f64, series: bool) -> bool {
    if series {
        if let Some(t) = game.mobs.iter_mut().find(|m| m.id == target) {
            t.invulnerable = 0;
        }
        if let Some(host) = game.lan.as_mut()
            && let Some(g) = host.hub.guests.iter_mut().find(|g| g.entity_id as u32 == target)
            && let Some(motion) = g.motion.as_deref_mut()
        {
            motion.health.hurt.cooldown = 0;
        }
    }
    let Some(i) = game.mobs.iter().position(|m| m.id == mob) else { return false };
    let was = game.mobs[i].attrs.attack_damage;
    game.mobs[i].attrs.attack_damage = (f64::from(was) * mult) as f32;
    if let Some(m) = game.mobs.get_mut(i) {
        m.swing();
    }
    let ok = game.native_mob_attack(dim, mob, target);
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob) {
        m.attrs.attack_damage = was;
    }
    ok
}

/// A trident, a splash potion or a bolt from the mob's eyes at the target.
fn throw(game: &mut Game<'_>, dim: &str, m: &Seen, tg: &Target, kind: &str, damage: f64) -> bool {
    let eye = geom::add(m.pos, [0.0, m.eye, 0.0]);
    let aim = geom::add(tg.pos, [0.0, 1.1, 0.0]);
    let (entity, speed, gravity) = match kind {
        "trident" => ("minecraft:trident", TRIDENT_SPEED, 0.05),
        "potion" => ("minecraft:splash_potion", POTION_SPEED, 0.05),
        _ => ("minecraft:arrow", BOLT_SPEED, 0.05),
    };
    let d = geom::sub(aim, eye);
    let l = geom::len(d).max(1e-3);
    let dir = geom::scale(d, 1.0 / l);
    let lift = 0.5 * gravity * (l / speed).powi(2) / l;
    let v = geom::scale([dir[0], dir[1] + lift, dir[2]], speed);
    let from = geom::add(eye, geom::scale(dir, 0.8));
    let motion = format!("Motion:[{:.4}d,{:.4}d,{:.4}d]", v[0], v[1], v[2]);
    let snbt = match kind {
        "potion" => {
            let potion = potion_for(geom::flat_dist(m.pos, tg.pos));
            format!("{{{motion},Item:{{id:\"minecraft:splash_potion\",count:1,components:{{\"minecraft:potion_contents\":{{potion:\"{potion}\"}}}}}}}}")
        }
        "trident" => format!("{{{motion},pickup:0b}}"),
        _ => format!("{{{motion},damage:{:.4}d,pickup:0b}}", damage / speed),
    };
    let nbt = match crate::mod_api::parse_snbt(&snbt) {
        Ok((crate::mod_api::Tag::Compound(c), _)) => c,
        _ => return false,
    };
    if game.native_summon(dim, entity, from, nbt).is_some() {
        return true;
    }
    // The engine gives no id back for a projectile (`native_summon` finds its root among mobs and a
    // few kinds only, game/native_entities.rs:259-265): look for it where it was thrown.
    let probe = format!(
        "execute in {dim} if entity @e[type={entity},x={:.2},y={:.2},z={:.2},distance=..2.5]",
        from[0], from[1], from[2]
    );
    game.native_run_command_captured(&probe).result > 0
}

/// Vanilla's witch picks by the target (`Witch.performRangedAttack`); without the target's effects
/// at hand the pick goes by distance: slowness far away, poison in the middle, harm close.
fn potion_for(d: f64) -> &'static str {
    if d >= 8.0 {
        "minecraft:slowness"
    } else if d >= 4.0 {
        "minecraft:poison"
    } else {
        "minecraft:harming"
    }
}

/// The mind's question: a standing spot above the target (skeletons) or one the target cannot see
/// (cover), the nearest of those within the group's search radius.
fn find_spot(game: &Game<'_>, m: &Seen, g: &VanillaDef, t: &Target, want: Want) -> Option<V3> {
    let boxes = |c: [i32; 3]| game.native_block_collision(c);
    let base = [m.pos[0].floor() + 0.5, m.pos[1].floor(), m.pos[2].floor() + 0.5];
    let (k, ys): (i32, Vec<f64>) = match want {
        Want::HighSpot => {
            let above = g.param("high_min_above_blocks").unwrap_or(2.0);
            let from = (t.pos[1] + above).ceil();
            (g.param("high_search_blocks").unwrap_or(8.0) as i32, (0..5).map(|i| from + f64::from(i)).collect())
        }
        Want::Cover => (g.param("hide_search_blocks").unwrap_or(8.0) as i32, (-1..=1).map(|i| base[1] + f64::from(i)).collect()),
    };
    let mut spots = Vec::new();
    for y in ys {
        for dx in -k..=k {
            for dz in -k..=k {
                let p = [base[0] + f64::from(dx), y, base[2] + f64::from(dz)];
                if fit::fits(p, m.width, m.height, &boxes) && fit::supported(p, m.width, &boxes) {
                    spots.push(p);
                }
            }
        }
    }
    match want {
        Want::HighSpot => spots.into_iter().min_by(|a, b| geom::dist(*a, m.pos).total_cmp(&geom::dist(*b, m.pos))),
        Want::Cover => {
            let threat = [Threat { pos: geom::add(t.pos, [0.0, EYE, 0.0]), range: FOLLOW }];
            let sight = |a: V3, b: V3| crate::game::soulscraft_cover::sees(game, a, b);
            cover::nearest_cover(m.pos, &spots, m.eye, &threat, &sight)
        }
    }
}

fn log_events(g: &VanillaDef, m: &Seen, now: u64, tick: &VTick) {
    let mut s = state();
    for e in &tick.events {
        match e {
            Event::Phase { attack, phase, t } => {
                let a = g.attacks.get(*attack).map_or("?", |a| a.def.id.as_str());
                log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: vphase mob={} id={} attack={a} phase={} t={t} tick={now}", m.kind, m.id, phase.label());
                s.phases.push(VRecord { mob: m.id, attack: a.to_owned(), phase: Some(*phase), t: *t, tick: now, hit: false });
            }
            Event::Mode(mode) => {
                log::info!("soulscraft: vmode mob={} id={} mode={mode} tick={now}", m.kind, m.id);
                s.modes.push((m.id, (*mode).to_owned(), now));
            }
            _ => {}
        }
    }
    if tick.label.starts_with("flank") {
        s.flanks.push((m.id, tick.label.clone(), now));
    }
    let st = &mut *s;
    trim(&mut st.phases);
    trim(&mut st.strikes);
    trim(&mut st.modes);
    trim(&mut st.flanks);
    trim(&mut st.cues);
}

fn trim<T>(list: &mut Vec<T>) {
    let n = list.len();
    if n > LOG_KEEP {
        list.drain(..n - LOG_KEEP);
    }
}

fn forget_gone(dim: &str, now: u64) {
    let gone: Vec<u32> = {
        let mut s = state();
        let gone: Vec<u32> = s.mobs.iter().filter(|(_, v)| v.dim == dim && now.saturating_sub(v.seen) >= FORGET_AFTER).map(|(id, _)| *id).collect();
        for id in &gone {
            s.mobs.remove(id);
            s.targets.remove(id);
        }
        gone
    };
    if !gone.is_empty() {
        let mut h = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        for id in gone {
            h.remove(&id);
        }
    }
    if now % 200 == 0 {
        let mut s = state();
        if s.skipped.len() > 4096 {
            s.skipped.clear();
        }
    }
}

/// `/sc debug on`: the labels of vanilla mobs (no clip: vanilla draws them).
fn send_labels(game: &mut Game<'_>, dim: &str, now: u64, notes: Vec<wire::Note>) {
    let messages = wire::encode(&wire::Batch { now: now as i64, notes });
    let everyone: Vec<u128> = game.native_server_players().into_iter().filter(|p| p.dimension == dim).map(|p| p.uuid).collect();
    for uuid in everyone {
        for bytes in &messages {
            let _ = game.native_net_send_to_player(uuid, wire::STATE, bytes);
        }
    }
}

/// The self-test: this mob's target is this entity (a still golem), and start this move now.
pub(super) fn set_target(mob: u32, entity: u32) {
    state().targets.insert(mob, entity);
}

pub(super) fn force(mob: u32, attack: &str) -> bool {
    let groups = crate::game::soulscraft_server::state().data.vanilla.groups.clone();
    let mut s = state();
    let Some(v) = s.mobs.get_mut(&mob) else { return false };
    let Some(idx) = groups.get(v.group).and_then(|g| g.attack_index(attack)) else { return false };
    let Some(b) = v.brain.as_mut() else { return false };
    b.force(idx);
    true
}

/// `/sc info`: how many vanilla mobs are adopted, and how many of them are spawners'.
pub(super) fn summary() -> String {
    let s = state();
    let spawner = s.mobs.values().filter(|v| v.brain.is_none()).count();
    format!("vanilla: {} adopted ({spawner} from spawners), {} left to events", s.mobs.len(), s.skipped.len())
}
