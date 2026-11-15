//! The approved changes of vanilla mobs (docs/design/07-vanilla-mobs.md): data/mobs.json
//! `vanilla` read into types, and a small mind that gives a vanilla mob its one or two moves.
//!
//! A vanilla mob keeps its own model, goals and target (PCT's provocation still names it). The mind
//! only takes over while one of its moves runs (a telegraphed attack on the phase ticks of
//! crate::attack, the same machine as SoulsCraft's mobs) and for a few short modes (a step back, a
//! retreat from a close player, a pause after three arrows, the way to a higher spot or to cover).
//! While it holds `control`, the server half parks the vanilla goals that move and attack (a gate
//! goal over them), so nothing else strikes during a windup; between moves the vanilla mob is
//! vanilla. Witches and pillagers are the exception: the mind runs their whole fight within a
//! range, because vanilla's potions and bolts have no windup to add to.
//!
//! Nothing here knows the game; the server half (mod/soulscraft/vanilla.rs) reads the senses and
//! turns a [`VTick`] into the engine's calls.

use crate::attack::{Phase, Strikes, Timeline};
use crate::brain::{DASH_CONTACT, DEFAULT_REACH, Event, LANDING_TIMEOUT, MELEE_ARC, Order, Strike, away};
use crate::data::AttackDef;
use crate::geom::{self, V3};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Ticks a leap is reckoned to fly per unit of upward speed: vanilla's gravity of 0.08 a tick.
const GRAVITY: f64 = 0.08;
/// Vanilla's air drag on the ground plane, each tick (`LivingEntity.travel`: 0.91).
const AIR_DRAG: f64 = 0.91;
/// A spider's leap up when the data names none (vanilla `LeapAtTargetGoal`: 0.4).
const POUNCE_UP: f64 = 0.42;
/// The roles of a wave (06-altar.md, «Оборона алтаря»).
const ROLES: [&str; 3] = ["assault", "hunter", "ranged"];

/// data/mobs.json `vanilla`: which vanilla mobs change and how.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct VanillaSection {
    /// A mob that appears this close to a spawner block came from it (Г4: it grows, but no moves).
    #[serde(default)]
    pub spawner_radius_blocks: i32,
    #[serde(default)]
    pub groups: Vec<VanillaDef>,
    /// The vanilla mobs the design leaves as they are (creeper by the owner's rule, neutrals, the
    /// ones that already fit, bosses): never in a group.
    #[serde(default)]
    pub untouched: Vec<String>,
}

/// One row of 07-vanilla-mobs.md: the vanilla types that change the same way.
#[derive(Clone, Debug, Deserialize)]
pub struct VanillaDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Entity types: `minecraft:zombie`.
    pub types: Vec<String>,
    /// How the mind drives them ([`Behavior`]).
    pub behavior: String,
    /// The types that are undead in vanilla. Round 11: they stay so (nothing here changes a flag);
    /// the list documents it and is checked to name only the group's types.
    #[serde(default)]
    pub undead: Vec<String>,
    /// Their role in a wave of an altar's event: `assault`, `hunter`, `ranged`.
    #[serde(default)]
    pub wave_role: String,
    #[serde(default)]
    pub attacks: Vec<VanillaAttack>,
    #[serde(default)]
    pub params: Map<String, Value>,
}

/// A move of a vanilla mob: SoulsCraft's attack with phases, for some of the group's types only,
/// and how its windup shows without a rig.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct VanillaAttack {
    #[serde(flatten)]
    pub def: AttackDef,
    /// Only these types have it (the drowned's trident); empty for all of the group.
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub cue: Cue,
}

/// What a windup shows and sounds like on a vanilla model: a sound (`{name}` is the type's path,
/// so `minecraft:entity.{name}.angry` is the hoglin's or the zoglin's), particles around the mob,
/// the vanilla pose `aggressive` (raised arms, the drowned's raised trident) and a small hop (a
/// cube's squash on landing). `by_type` overrides it for one type.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Cue {
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub pitch: Option<f32>,
    #[serde(default)]
    pub particle: Option<String>,
    #[serde(default)]
    pub pose: Option<String>,
    #[serde(default)]
    pub hop: Option<f64>,
    #[serde(default)]
    pub by_type: BTreeMap<String, Cue>,
}

impl Cue {
    /// The cue of one type: its override over the group's, `{name}` filled in.
    pub fn for_type(&self, kind: &str) -> Cue {
        let o = self.by_type.get(kind);
        let name = kind.rsplit(':').next().unwrap_or(kind);
        let fill = |s: &Option<String>| s.as_ref().map(|s| s.replace("{name}", name));
        let pick = |own: &Option<String>, over: Option<&Option<String>>| fill(over.filter(|v| v.is_some()).unwrap_or(own));
        Cue {
            sound: pick(&self.sound, o.map(|o| &o.sound)),
            pitch: o.and_then(|o| o.pitch).or(self.pitch),
            particle: pick(&self.particle, o.map(|o| &o.particle)),
            pose: o.and_then(|o| o.pose.clone()).or_else(|| self.pose.clone()),
            hop: o.and_then(|o| o.hop).or(self.hop),
            by_type: BTreeMap::new(),
        }
    }

    pub fn aggressive(&self) -> bool {
        self.pose.as_deref() == Some("aggressive")
    }
}

/// How the mind drives a group (data `behavior`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// Zombies: lunge, a step back after a blow, flanks in a group; the drowned's trident.
    Brawler,
    /// Skeletons: back off from a close player, climb above it, pause after three arrows.
    Archer,
    /// Spiders: a crouched pounce; wait on a ledge above until the player passes under.
    Pouncer,
    /// Witch: keeps its distance, a telegraphed potion, hides to drink when hurt.
    Witch,
    /// Pillager: aimed bolts, reloads in cover.
    Crossbow,
    /// Vindicator: the delayed overhead axe.
    Axe,
    /// Piglin brute: every blow has a windup; a miss leaves it open.
    Heavy,
    /// Hoglin, zoglin: a ram after pawing the ground; a miss leaves it turning.
    Ram,
    /// Wither skeleton: two blows with a pause, then a step back.
    Fencer,
    /// Slime, magma cube: a big jump after squashing.
    Cube,
    /// Phantom: a cry ten ticks before its own dive.
    Phantom,
}

impl Behavior {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "brawler" => Self::Brawler,
            "archer" => Self::Archer,
            "pouncer" => Self::Pouncer,
            "witch" => Self::Witch,
            "crossbow" => Self::Crossbow,
            "axe" => Self::Axe,
            "heavy" => Self::Heavy,
            "ram" => Self::Ram,
            "fencer" => Self::Fencer,
            "cube" => Self::Cube,
            "phantom" => Self::Phantom,
            _ => return None,
        })
    }

    /// The attack every mob of this behaviour needs in the data.
    pub fn needs(self) -> Option<&'static str> {
        Some(match self {
            Self::Brawler => "lunge",
            Self::Archer => return None,
            Self::Pouncer => "pounce",
            Self::Witch => "potion_throw",
            Self::Crossbow => "crossbow_shot",
            Self::Axe => "overhead",
            Self::Heavy => "heavy_strike",
            Self::Ram => "ram",
            Self::Fencer => "series",
            Self::Cube => "high_jump",
            Self::Phantom => "cry",
        })
    }
}

impl VanillaSection {
    /// The group of a vanilla type, with its index.
    pub fn group_of(&self, kind: &str) -> Option<(usize, &VanillaDef)> {
        self.groups.iter().enumerate().find(|(_, g)| g.types.iter().any(|t| t == kind))
    }

    /// The wave role the data gives a vanilla type.
    pub fn role_of(&self, kind: &str) -> Option<&str> {
        self.group_of(kind).map(|(_, g)| g.wave_role.as_str()).filter(|r| !r.is_empty())
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for g in &self.groups {
            let at = |why: String| format!("vanilla group {}: {why}", g.id);
            let b = Behavior::parse(&g.behavior).ok_or_else(|| at(format!("behavior {:?} unknown", g.behavior)))?;
            if g.types.is_empty() {
                return Err(at("no types".into()));
            }
            for t in &g.types {
                if !t.starts_with("minecraft:") {
                    return Err(at(format!("{t} is not a vanilla type")));
                }
                if self.untouched.contains(t) {
                    return Err(at(format!("{t} is on the untouched list")));
                }
                if !seen.insert(t.clone()) {
                    return Err(at(format!("{t} is in two groups")));
                }
            }
            if let Some(t) = g.undead.iter().find(|t| !g.types.contains(t)) {
                return Err(at(format!("undead {t} is not one of its types")));
            }
            if !ROLES.contains(&g.wave_role.as_str()) {
                return Err(at(format!("wave_role {:?} is none of {ROLES:?}", g.wave_role)));
            }
            if let Some(need) = b.needs()
                && !g.attacks.iter().any(|a| a.def.id == need)
            {
                return Err(at(format!("a {} needs the attack {need}", g.behavior)));
            }
            for a in &g.attacks {
                if a.def.hit == 0 {
                    return Err(at(format!("attack {}: hit phase of 0 ticks", a.def.id)));
                }
                if a.def.strikes_at.as_ref().is_some_and(|s| s.iter().any(|x| *x >= a.def.hit)) {
                    return Err(at(format!("attack {}: a strike past the hit phase", a.def.id)));
                }
                if let Some(t) = a.only.iter().find(|t| !g.types.contains(t)) {
                    return Err(at(format!("attack {}: only {t}, not one of its types", a.def.id)));
                }
            }
        }
        if seen.contains("minecraft:creeper") {
            return Err("vanilla: the creeper stays as it is (owner's rule, round 3)".into());
        }
        Ok(())
    }
}

impl VanillaDef {
    pub fn behavior(&self) -> Behavior {
        Behavior::parse(&self.behavior).unwrap_or(Behavior::Axe)
    }

    pub fn attack_index(&self, id: &str) -> Option<usize> {
        self.attacks.iter().position(|a| a.def.id == id)
    }

    pub fn param(&self, name: &str) -> Option<f64> {
        self.params.get(name).and_then(Value::as_f64)
    }

    fn num(&self, name: &str, default: f64) -> f64 {
        self.param(name).unwrap_or(default)
    }

    /// A `[from, to]` parameter, or a single number as both ends.
    pub fn range(&self, name: &str, default: (f64, f64)) -> (f64, f64) {
        match self.params.get(name) {
            Some(Value::Array(v)) if v.len() == 2 => (v[0].as_f64().unwrap_or(default.0), v[1].as_f64().unwrap_or(default.1)),
            Some(v) => v.as_f64().map_or(default, |x| (x, x)),
            None => default,
        }
    }
}

/// Where the mob stands among the brawlers going for the same target: the first (`index` 0) leads
/// straight in, the others come round its sides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flank {
    pub index: usize,
    pub count: usize,
    pub leader: V3,
}

/// What the server half reads for one vanilla mob this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VSenses {
    pub now: u64,
    pub pos: V3,
    pub yaw: f64,
    pub on_ground: bool,
    pub in_water: bool,
    pub health_frac: f64,
    /// Where vanilla's own target stands (its feet).
    pub target: Option<V3>,
    /// Nothing solid between its eyes and the target's.
    pub sees_target: bool,
    /// Slowness on the mob (PCT's stun is slowness): no new move starts.
    pub slowed: bool,
    /// Its vanilla melee swung this tick.
    pub swung: bool,
    /// It draws a bow or loads a crossbow (vanilla's item use).
    pub using_item: bool,
    pub holding_trident: bool,
    /// The phantom's vanilla swoop was decided this tick.
    pub swoop_started: bool,
    pub flank: Option<Flank>,
    /// A slime's size (0 for others).
    pub size: i32,
    /// The server's answer to the last [`Want`]: the spot, or none found.
    pub answer: Option<Option<V3>>,
}

/// A spot the mind asks the server to find (the blocks are the server's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// A standing spot higher than the target (the skeleton).
    HighSpot,
    /// A spot the target cannot see (the witch drinking, the pillager reloading).
    Cover,
}

/// What a vanilla mob does this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct VTick {
    /// The mind holds the mob: its vanilla moving and attacking goals rest.
    pub control: bool,
    /// Vanilla's own leap rests (a spider near its target: the pounce has a windup instead).
    pub no_leap: bool,
    pub order: Order,
    /// A velocity set once now: a leap, a pounce, a cube's hop.
    pub launch: Option<V3>,
    pub look_at: Option<V3>,
    pub events: Vec<Event>,
    pub strikes: Vec<Strike>,
    /// The attack whose windup starts now: the server plays its cue.
    pub cue: Option<usize>,
    /// Vanilla's `aggressive` pose to set (raised arms or trident), when the mind says.
    pub aggressive: Option<bool>,
    pub want: Option<Want>,
    /// The phantom's vanilla swoop is held back this tick (it keeps circling).
    pub hold_dive: bool,
    /// The held swoop goes now.
    pub release_dive: bool,
    /// A label for `/sc debug`.
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Free,
    StepBack { until: u64, from: V3 },
    Retreat { until: u64 },
    Climb { to: V3, until: u64 },
    Pause { until: u64 },
    Hide { to: V3, until: u64 },
    Wait { until: u64 },
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Free => "free",
            Mode::StepBack { .. } => "step_back",
            Mode::Retreat { .. } => "retreat",
            Mode::Climb { .. } => "climb",
            Mode::Pause { .. } => "pause",
            Mode::Hide { .. } => "hide",
            Mode::Wait { .. } => "wait",
        }
    }

    fn until(self) -> Option<u64> {
        match self {
            Mode::Free => None,
            Mode::StepBack { until, .. }
            | Mode::Retreat { until }
            | Mode::Climb { until, .. }
            | Mode::Pause { until }
            | Mode::Hide { until, .. }
            | Mode::Wait { until } => Some(until),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    idx: usize,
    start: u64,
    landed: u32,
    hits: u32,
    phase: Phase,
    dir: V3,
    per_tick: f64,
    ends_at: Option<u32>,
    airborne_since: Option<u64>,
    landed_jump: bool,
    /// The tick of the attack a cube came down: its recovery counts from there.
    landed_at: Option<u32>,
}

/// One vanilla mob's mind, as long as the mob is in memory.
#[derive(Clone, Debug)]
pub struct VBrain {
    pub behavior: Behavior,
    /// Which of the group's attacks this type has (`only`).
    allowed: Vec<bool>,
    run: Option<Run>,
    mode: Mode,
    next_decision: u64,
    rng: u64,
    forced: Option<usize>,
    shots: u32,
    was_using: bool,
    /// The last tick held the mob: a draw that ended then was cut by the take-over, not loosed.
    held: bool,
    /// The skeleton may back off: at first and after each arrow. Taking it over restarts vanilla's
    /// bow goal, which walks in for 20 ticks before it shoots, so a retreat on every close tick
    /// kept it from ever shooting.
    may_retreat: bool,
    last_high: u64,
    last_hide: u64,
    pending: Option<Want>,
    /// The pillager's reload spot.
    cover: Option<V3>,
}

impl VBrain {
    pub fn new(def: &VanillaDef, kind: &str, seed: u64, now: u64) -> Self {
        Self {
            behavior: def.behavior(),
            allowed: def.attacks.iter().map(|a| a.only.is_empty() || a.only.iter().any(|t| t == kind)).collect(),
            run: None,
            mode: Mode::Free,
            next_decision: now,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            forced: None,
            shots: 0,
            was_using: false,
            held: false,
            may_retreat: true,
            last_high: now,
            last_hide: 0,
            pending: None,
            cover: None,
        }
    }

    /// Start this attack on the next tick (`/sc force`, the self-test).
    pub fn force(&mut self, attack: usize) {
        self.forced = Some(attack);
    }

    /// The attack running and how many ticks into it.
    pub fn attack(&self, now: u64) -> Option<(usize, u32)> {
        self.run.map(|r| (r.idx, (now - r.start) as u32))
    }

    pub fn mode_name(&self) -> &'static str {
        self.mode.name()
    }

    /// Arrows counted since the last pause (the skeleton).
    pub fn shots(&self) -> u32 {
        self.shots
    }

    fn has(&self, def: &VanillaDef, id: &str) -> Option<usize> {
        def.attack_index(id).filter(|i| self.allowed.get(*i).copied().unwrap_or(false))
    }

    fn roll(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn set_mode(&mut self, mode: Mode, out: &mut VTick) {
        if std::mem::discriminant(&mode) != std::mem::discriminant(&self.mode) {
            out.events.push(Event::Mode(mode.name()));
        }
        self.mode = mode;
    }

    /// One tick: `decide_every` is Г8's `20 − 10·P`.
    pub fn tick(&mut self, def: &VanillaDef, decide_every: u32, s: &VSenses) -> VTick {
        let mut out = VTick {
            control: false,
            no_leap: false,
            order: Order::Stay,
            launch: None,
            look_at: None,
            events: Vec::new(),
            strikes: Vec::new(),
            cue: None,
            aggressive: None,
            want: None,
            hold_dive: false,
            release_dive: false,
            label: String::new(),
        };
        let decide = s.now >= self.next_decision;
        if decide {
            self.next_decision = s.now + u64::from(decide_every.max(1));
        }
        if self.behavior == Behavior::Archer {
            if self.was_using && !s.using_item && !self.held {
                self.shots += 1;
                self.may_retreat = true;
            }
            self.was_using = s.using_item;
        }
        if let Some(idx) = self.forced.take()
            && idx < def.attacks.len()
        {
            self.begin(idx, s.now, &mut out);
        }
        if self.run.is_some() {
            self.advance(def, s, &mut out);
            if self.run.is_some() {
                self.held = out.control;
                return out;
            }
        }
        self.mind(def, s, decide, &mut out);
        if self.run.is_some() {
            self.advance(def, s, &mut out);
        }
        self.held = out.control;
        out
    }

    fn begin(&mut self, idx: usize, now: u64, out: &mut VTick) {
        self.run = Some(Run {
            idx,
            start: now,
            landed: 0,
            hits: 0,
            phase: Phase::Windup,
            dir: [0.0, 0.0, 1.0],
            per_tick: 0.0,
            ends_at: None,
            airborne_since: None,
            landed_jump: false,
            landed_at: None,
        });
        out.events.push(Event::AttackStart { attack: idx });
        out.events.push(Event::Phase { attack: idx, phase: Phase::Windup, t: 0 });
    }

    fn advance(&mut self, def: &VanillaDef, s: &VSenses, out: &mut VTick) {
        let Some(mut run) = self.run else { return };
        let Some(va) = def.attacks.get(run.idx) else {
            self.run = None;
            return;
        };
        let a = &va.def;
        let line = Timeline::of(a);
        let t = (s.now - run.start) as u32;
        let mut phase = line.phase_at(t);
        if run.ends_at.is_some_and(|end| t >= end) {
            phase = Phase::Done;
        }
        if self.behavior == Behavior::Cube && run.airborne_since.is_some() {
            // A cube's jump lasts as long as its flight: the hit phase holds until it is down, the
            // recovery counts from the landing.
            phase = match run.landed_at {
                None => Phase::Hit,
                Some(l) if t < l + a.recover => Phase::Recover,
                Some(_) => Phase::Done,
            };
        }
        if phase != run.phase {
            run.phase = phase;
            out.events.push(Event::Phase { attack: run.idx, phase, t });
        }
        // The phantom flies on its own controller: its cry only holds vanilla's swoop back.
        let phantom = self.behavior == Behavior::Phantom;
        out.control = !phantom;
        out.look_at = s.target.map(|p| geom::add(p, [0.0, 1.5, 0.0]));
        if t == 0 {
            out.cue = Some(run.idx);
        }
        let pose = va.cue.aggressive();
        match phase {
            Phase::Windup => {
                out.hold_dive = phantom;
                if pose {
                    out.aggressive = Some(true);
                }
            }
            Phase::Hold => {
                // A delayed blow commits: it stops tracking the target while it holds.
                out.look_at = None;
                if pose {
                    out.aggressive = Some(true);
                }
            }
            Phase::Hit => {
                if t == line.hit_start() {
                    out.release_dive = phantom;
                    self.start_hit(def, &mut run, a, &line, s, out);
                } else if run.per_tick > 0.0 {
                    out.order = Order::Dash { dir: run.dir, per_tick: run.per_tick };
                }
                if run.per_tick > 0.0 || run.airborne_since.is_some() {
                    out.look_at = None;
                }
            }
            Phase::Recover => {
                if t == line.recover_start() && a.recover_if.as_deref() == Some("miss") && run.hits > 0 {
                    // It landed: no opening for the player, only a short pause.
                    run.ends_at = Some(line.recover_start() + def.num("recover_on_hit_ticks", 4.0) as u32);
                }
                out.order = self.recover_order(a, s);
                if run.hits == 0 && a.recover_if.as_deref() == Some("miss") {
                    // The miss window: it turns round, slowly (no tracking).
                    out.look_at = None;
                }
                if pose {
                    out.aggressive = Some(false);
                }
            }
            Phase::Done => {}
        }
        match self.behavior {
            Behavior::Phantom => {}
            Behavior::Cube => self.track_landing(&mut run, a, s, out),
            _ => self.strike(def, &mut run, a, t, s, out),
        }
        out.label = format!("{}:{} {}", a.id, phase.label(), t);
        if phase == Phase::Done {
            out.events.push(Event::AttackEnd { attack: run.idx, t });
            self.run = None;
            self.after_attack(def, s, out);
        } else {
            self.run = Some(run);
        }
    }

    fn start_hit(&mut self, def: &VanillaDef, run: &mut Run, a: &AttackDef, line: &Timeline, s: &VSenses, out: &mut VTick) {
        let Some(tg) = s.target else { return };
        match self.behavior {
            Behavior::Pouncer => {
                let up = def.num("leap_up", POUNCE_UP);
                out.launch = Some(leap_velocity(s.pos, tg, up, a.dash_blocks.unwrap_or(4.0) + 1.0));
                run.airborne_since = Some(s.now);
            }
            Behavior::Cube => {
                let range = def.num("jump_range_blocks", 8.0);
                out.launch = Some(leap_velocity(s.pos, tg, def.num("leap_up", 0.75), range));
                run.airborne_since = Some(s.now);
            }
            Behavior::Crossbow if a.projectile.is_some() => {
                // The bolt is away: the reload is walked to cover (asked now, answered next tick).
                self.cover = None;
                self.pending = Some(Want::Cover);
                out.want = Some(Want::Cover);
            }
            _ => {
                if let Some(blocks) = a.dash_blocks {
                    run.dir = geom::flat_dir(s.pos, tg);
                    run.per_tick = blocks / f64::from(line.hit.max(1));
                    out.order = Order::Dash { dir: run.dir, per_tick: run.per_tick };
                }
            }
        }
    }

    /// The blow due on tick `t`: in reach and in front for a blow; contact for a dash or a pounce.
    fn strike(&mut self, def: &VanillaDef, run: &mut Run, a: &AttackDef, t: u32, s: &VSenses, out: &mut VTick) {
        let line = Timeline::of(a);
        let strikes = if self.behavior == Behavior::Pouncer {
            Strikes::Dash { from: line.hit_start(), to: line.recover_start() + 4 }
        } else {
            Strikes::of(a)
        };
        if !strikes.strikes_on(t, run.landed) {
            return;
        }
        let reach = def.num("reach_blocks", DEFAULT_REACH);
        let hit = s.target.is_some_and(|tg| match strikes {
            Strikes::Dash { .. } if self.behavior == Behavior::Pouncer => geom::dist(s.pos, tg) <= DASH_CONTACT,
            Strikes::Dash { .. } => geom::flat_dist(s.pos, tg) <= DASH_CONTACT,
            Strikes::At(_) => {
                a.projectile.is_some()
                    || geom::flat_dist(s.pos, tg) <= reach && geom::off_facing(s.pos, s.yaw, tg) <= a.arc_degrees.unwrap_or(MELEE_ARC) / 2.0
            }
        });
        let dash = matches!(strikes, Strikes::Dash { .. });
        if dash && !hit {
            return;
        }
        run.landed += 1;
        if hit && a.projectile.is_none() {
            run.hits += 1;
        }
        if dash {
            // A dash stops where it struck.
            run.per_tick = 0.0;
        }
        out.strikes.push(Strike {
            attack: run.idx,
            hits_target: hit && a.projectile.is_none(),
            area: a.wave.as_ref().map(|w| w.radius_blocks),
            ranged: a.projectile.is_some(),
        });
    }

    /// The cube's blow is its landing: everything near when it is down again.
    fn track_landing(&mut self, run: &mut Run, a: &AttackDef, s: &VSenses, out: &mut VTick) {
        let Some(since) = run.airborne_since else { return };
        if run.landed_jump {
            return;
        }
        let up = s.now.saturating_sub(since);
        if (up >= 3 && s.on_ground) || up >= LANDING_TIMEOUT {
            run.landed_jump = true;
            run.landed += 1;
            run.landed_at = Some((s.now - run.start) as u32);
            let radius = landing_radius(a, s.size);
            let hit = s.target.is_some_and(|tg| geom::dist(s.pos, tg) <= radius);
            if hit {
                run.hits += 1;
            }
            out.strikes.push(Strike { attack: run.idx, hits_target: hit, area: Some(radius), ranged: false });
        }
    }

    fn recover_order(&mut self, a: &AttackDef, s: &VSenses) -> Order {
        let Some(tg) = s.target else { return Order::Stay };
        if self.behavior == Behavior::Crossbow && a.projectile.is_some() {
            if self.pending == Some(Want::Cover)
                && let Some(answer) = s.answer
            {
                self.pending = None;
                self.cover = answer;
            }
            return match self.cover {
                Some(spot) if geom::flat_dist(s.pos, spot) > 0.7 => Order::Walk { to: spot, speed: 1.1 },
                Some(_) => Order::Stay,
                None => Order::Walk { to: away(s.pos, tg, 4.0), speed: 1.0 },
            };
        }
        Order::Stay
    }

    fn after_attack(&mut self, def: &VanillaDef, s: &VSenses, out: &mut VTick) {
        let from = s.target.unwrap_or(s.pos);
        match self.behavior {
            Behavior::Fencer => {
                let until = s.now + def.num("backstep_ticks", 16.0) as u64;
                self.set_mode(Mode::StepBack { until, from }, out);
            }
            Behavior::Crossbow => {
                self.cover = None;
                self.pending = None;
            }
            _ => {}
        }
    }

    /// Between moves: the modes, and on decision ticks the next move.
    fn mind(&mut self, def: &VanillaDef, s: &VSenses, decide: bool, out: &mut VTick) {
        if self.mode.until().is_some_and(|u| s.now >= u) {
            if matches!(self.mode, Mode::Climb { .. }) {
                self.last_high = s.now;
            }
            if matches!(self.mode, Mode::Hide { .. }) {
                self.last_hide = s.now;
            }
            self.set_mode(Mode::Free, out);
        }
        let Some(tg) = s.target else {
            if self.mode != Mode::Free {
                self.set_mode(Mode::Free, out);
            }
            self.pending = None;
            out.label = "idle".into();
            return;
        };
        let d = geom::flat_dist(s.pos, tg);
        match self.behavior {
            Behavior::Brawler => self.brawler(def, s, tg, d, decide, out),
            Behavior::Archer => self.archer(def, s, tg, d, decide, out),
            Behavior::Pouncer => self.pouncer(def, s, tg, d, decide, out),
            Behavior::Witch => self.witch(def, s, tg, d, decide, out),
            Behavior::Crossbow => self.crossbow(def, s, tg, d, decide, out),
            Behavior::Axe => {
                let reach = def.num("reach_blocks", DEFAULT_REACH);
                if decide && !s.slowed && d <= reach && self.roll() < def.num("overhead_chance", 0.5) {
                    self.start(def, "overhead", s.now, out);
                }
            }
            Behavior::Heavy => {
                if d <= def.num("engage_blocks", 4.0) {
                    out.control = true;
                    out.look_at = Some(geom::add(tg, [0.0, 1.5, 0.0]));
                    let reach = def.num("reach_blocks", DEFAULT_REACH);
                    if decide && !s.slowed && d <= reach {
                        self.start(def, "heavy_strike", s.now, out);
                    } else if d > reach - 0.6 {
                        out.order = Order::Walk { to: tg, speed: 1.0 };
                    }
                    out.label = "engage".into();
                }
            }
            Behavior::Ram => {
                let (lo, hi) = def.range("ram_range_blocks", (2.5, 7.0));
                if decide && !s.slowed && s.on_ground && (lo..=hi).contains(&d) && self.roll() < def.num("ram_chance", 0.5) {
                    self.start(def, "ram", s.now, out);
                }
            }
            Behavior::Fencer => {
                if let Mode::StepBack { from, .. } = self.mode {
                    self.step_back(def, s, from, "backstep_blocks", out);
                    return;
                }
                let reach = def.num("reach_blocks", DEFAULT_REACH);
                if decide && !s.slowed && d <= reach && self.roll() < def.num("series_chance", 0.5) {
                    self.start(def, "series", s.now, out);
                }
            }
            Behavior::Cube => {
                let min = def.num("min_size", 2.0) as i32;
                let range = def.num("jump_range_blocks", 8.0);
                if decide && !s.slowed && s.on_ground && s.size >= min && d <= range && self.roll() < def.num("jump_chance", 0.3) {
                    self.start(def, "high_jump", s.now, out);
                }
            }
            Behavior::Phantom => {
                if s.swoop_started {
                    self.start(def, "cry", s.now, out);
                }
            }
        }
        if out.label.is_empty() {
            out.label = self.mode.name().into();
        }
    }

    fn start(&mut self, def: &VanillaDef, id: &str, now: u64, out: &mut VTick) {
        if let Some(idx) = self.has(def, id) {
            self.begin(idx, now, out);
        }
    }

    fn step_back(&mut self, def: &VanillaDef, s: &VSenses, from: V3, blocks: &str, out: &mut VTick) {
        out.control = true;
        out.order = Order::Walk { to: away(s.pos, from, def.num(blocks, 2.0)), speed: 1.0 };
        out.look_at = Some(geom::add(from, [0.0, 1.5, 0.0]));
        out.label = "step_back".into();
    }

    fn brawler(&mut self, def: &VanillaDef, s: &VSenses, tg: V3, d: f64, decide: bool, out: &mut VTick) {
        if let Mode::StepBack { from, .. } = self.mode {
            self.step_back(def, s, from, "step_back_blocks", out);
            return;
        }
        let look = Some(geom::add(tg, [0.0, 1.5, 0.0]));
        let (t_lo, t_hi) = def.range("trident_range_blocks", (4.0, 15.0));
        if s.holding_trident && self.has(def, "trident_throw").is_some() && d <= t_hi {
            // A thrower keeps the fight: vanilla's instant throw would skip the windup.
            out.control = true;
            out.look_at = look;
            out.aggressive = Some(false);
            if decide && !s.slowed && d >= t_lo && s.sees_target {
                self.start(def, "trident_throw", s.now, out);
                return;
            }
            let keep = def.num("trident_keep_blocks", 4.0);
            if d < keep {
                out.order = Order::Walk { to: away(s.pos, tg, keep - d + 1.0), speed: 1.0 };
            }
            out.label = "thrower".into();
            return;
        }
        if s.swung && self.roll() < def.num("step_back_chance", 0.3) {
            let until = s.now + def.num("step_back_ticks", 10.0) as u64;
            self.set_mode(Mode::StepBack { until, from: tg }, out);
            self.step_back(def, s, tg, "step_back_blocks", out);
            return;
        }
        let (lo, hi) = def.range("lunge_range_blocks", (2.5, 5.0));
        if decide && !s.slowed && (lo..=hi).contains(&d) && self.roll() < def.num("lunge_chance", 0.35) {
            self.start(def, "lunge", s.now, out);
            return;
        }
        let min = def.num("flank_min_group", 3.0) as usize;
        if let Some(f) = s.flank
            && f.count >= min
            && f.index > 0
            && d > def.num("flank_until_blocks", 3.5)
        {
            // Every other one to each side, wider the further down the list.
            let side = if f.index % 2 == 1 { 1.0 } else { -1.0 };
            let step = f.index.div_ceil(2) as f64;
            let angle = geom::ring_angle(tg, f.leader) + side * step * def.num("flank_degrees", 70.0).to_radians();
            let to = geom::ring_point(tg, def.num("flank_radius_blocks", 3.0), angle);
            out.control = true;
            out.order = Order::Walk { to, speed: 1.0 };
            out.look_at = look;
            out.label = format!("flank {}/{}", f.index, f.count);
        }
    }

    fn archer(&mut self, def: &VanillaDef, s: &VSenses, tg: V3, d: f64, decide: bool, out: &mut VTick) {
        let look = Some(geom::add(tg, [0.0, 1.5, 0.0]));
        if self.shots >= def.num("shots_before_pause", 3.0) as u32 {
            self.shots = 0;
            let until = s.now + def.num("pause_ticks", 20.0) as u64;
            self.set_mode(Mode::Pause { until }, out);
        }
        if self.pending == Some(Want::HighSpot)
            && let Some(answer) = s.answer
        {
            self.pending = None;
            match answer {
                Some(to) => {
                    let until = s.now + def.num("high_max_ticks", 80.0) as u64;
                    self.set_mode(Mode::Climb { to, until }, out);
                }
                None => self.last_high = s.now,
            }
        }
        let below = def.num("retreat_below_blocks", 4.0);
        let back_to = def.num("retreat_to_blocks", 7.0);
        match self.mode {
            Mode::Pause { .. } => {
                out.control = true;
                out.look_at = look;
                return;
            }
            Mode::Retreat { .. } if d < back_to => {
                out.control = true;
                out.order = Order::Walk { to: away(s.pos, tg, back_to - d + 1.0), speed: 1.2 };
                out.look_at = look;
                return;
            }
            Mode::Retreat { .. } => self.set_mode(Mode::Free, out),
            Mode::Climb { to, .. } => {
                if geom::flat_dist(s.pos, to) < 0.8 && (s.pos[1] - to[1]).abs() < 1.2 {
                    self.last_high = s.now;
                    self.set_mode(Mode::Free, out);
                } else {
                    out.control = true;
                    out.order = Order::Walk { to, speed: 1.1 };
                    return;
                }
            }
            _ => {}
        }
        if d < below && self.may_retreat {
            self.may_retreat = false;
            let until = s.now + def.num("retreat_max_ticks", 30.0) as u64;
            self.set_mode(Mode::Retreat { until }, out);
            out.control = true;
            out.order = Order::Walk { to: away(s.pos, tg, back_to - d + 1.0), speed: 1.2 };
            out.look_at = look;
            return;
        }
        let every = def.num("high_every_ticks", 100.0) as u64;
        let above = def.num("high_min_above_blocks", 2.0);
        if decide && self.pending.is_none() && s.now.saturating_sub(self.last_high) >= every && s.pos[1] < tg[1] + above {
            self.pending = Some(Want::HighSpot);
            out.want = Some(Want::HighSpot);
        }
    }

    fn pouncer(&mut self, def: &VanillaDef, s: &VSenses, tg: V3, d: f64, decide: bool, out: &mut VTick) {
        out.no_leap = !s.in_water && d <= def.num("no_vanilla_leap_within_blocks", 6.0);
        let until_blocks = def.num("wait_until_blocks", 2.5);
        if let Mode::Wait { .. } = self.mode {
            if tg[1] >= s.pos[1] - 0.5 {
                self.set_mode(Mode::Free, out);
            } else if d <= until_blocks {
                self.set_mode(Mode::Free, out);
                self.start(def, "pounce", s.now, out);
                return;
            } else {
                out.control = true;
                out.look_at = Some(geom::add(tg, [0.0, 1.0, 0.0]));
                return;
            }
        }
        let above = s.pos[1] - tg[1];
        if s.on_ground && above >= def.num("wait_above_blocks", 2.0) && d > until_blocks && d <= def.num("wait_from_blocks", 10.0) {
            let until = s.now + def.num("wait_max_ticks", 200.0) as u64;
            self.set_mode(Mode::Wait { until }, out);
            out.control = true;
            return;
        }
        let (lo, hi) = def.range("pounce_range_blocks", (2.0, 6.0));
        if decide && !s.slowed && s.on_ground && (lo..=hi).contains(&d) && self.roll() < def.num("pounce_chance", 0.4) {
            self.start(def, "pounce", s.now, out);
        }
    }

    /// The witch's whole fight within its range: distance, potions, hiding to drink.
    fn witch(&mut self, def: &VanillaDef, s: &VSenses, tg: V3, d: f64, decide: bool, out: &mut VTick) {
        if d > def.num("control_within_blocks", 16.0) {
            return;
        }
        out.control = true;
        out.look_at = Some(geom::add(tg, [0.0, 1.5, 0.0]));
        if let Mode::Hide { to, .. } = self.mode {
            if geom::flat_dist(s.pos, to) > 0.8 {
                out.order = Order::Walk { to, speed: 1.2 };
            }
            out.look_at = None;
            return;
        }
        if self.pending == Some(Want::Cover)
            && let Some(answer) = s.answer
        {
            self.pending = None;
            match answer {
                Some(to) => {
                    let until = s.now + def.num("hide_ticks", 60.0) as u64 + 60;
                    self.set_mode(Mode::Hide { to, until }, out);
                    out.order = Order::Walk { to, speed: 1.2 };
                    return;
                }
                None => self.last_hide = s.now,
            }
        }
        let hurt = s.health_frac < def.num("heal_below_health", 0.5);
        let rested = self.last_hide == 0 || s.now.saturating_sub(self.last_hide) >= def.num("hide_cooldown_ticks", 200.0) as u64;
        if hurt && rested && self.pending.is_none() {
            self.pending = Some(Want::Cover);
            out.want = Some(Want::Cover);
        }
        let (lo, hi) = def.range("keep_distance_blocks", (6.0, 10.0));
        out.order = keep(s.pos, tg, d, lo, hi);
        if decide && !s.slowed && d <= hi + 2.0 && s.sees_target {
            self.start(def, "potion_throw", s.now, out);
        }
    }

    /// The pillager's fight within its range: distance, aimed bolts, reload in cover.
    fn crossbow(&mut self, def: &VanillaDef, s: &VSenses, tg: V3, d: f64, decide: bool, out: &mut VTick) {
        if d > def.num("control_within_blocks", 20.0) {
            return;
        }
        out.control = true;
        out.look_at = Some(geom::add(tg, [0.0, 1.5, 0.0]));
        let (lo, hi) = def.range("keep_distance_blocks", (8.0, 14.0));
        out.order = keep(s.pos, tg, d, lo, hi);
        if decide && !s.slowed && d >= lo - 2.0 && d <= hi + 2.0 && s.sees_target {
            self.start(def, "crossbow_shot", s.now, out);
        }
    }
}

/// Walk to keep `d` within `lo..=hi` of the target.
fn keep(pos: V3, tg: V3, d: f64, lo: f64, hi: f64) -> Order {
    if d < lo {
        Order::Walk { to: away(pos, tg, (lo + hi) / 2.0 - d), speed: 1.0 }
    } else if d > hi {
        Order::Walk { to: tg, speed: 1.0 }
    } else {
        Order::Stay
    }
}

/// The landing's reach for a cube of `size`: the data's radius for size 2, wider for size 4.
pub fn landing_radius(a: &AttackDef, size: i32) -> f64 {
    let base = a.wave.as_ref().map_or(1.5, |w| w.radius_blocks);
    base * (f64::from(size.max(2)) / 2.0)
}

/// The velocity that leaps from `from` up at `up` and comes down near `to` (at most `max` blocks
/// away): the time in the air from vanilla's gravity, the ground speed from its air drag.
pub fn leap_velocity(from: V3, to: V3, up: f64, max: f64) -> V3 {
    let ticks = (2.0 * up / GRAVITY).round().max(1.0) as i32;
    let carry: f64 = (0..ticks).map(|i| AIR_DRAG.powi(i)).sum();
    let dir = geom::flat_dir(from, to);
    let ground = geom::flat_dist(from, to).min(max) / carry;
    [dir[0] * ground, up, dir[2] * ground]
}

#[cfg(test)]
#[path = "vanilla_tests.rs"]
mod tests;
