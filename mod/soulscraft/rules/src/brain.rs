//! What a SoulsCraft mob does each tick, from what it senses: where to go, where to look, which
//! attack to start, and on which tick a blow lands. The server half turns a [`Tick`] into the
//! engine's calls (navigation, damage, velocity) and tells the clients the clip; nothing here
//! knows the game, so every rule is tested with `cargo test`.
//!
//! Decisions (start an attack, start circling) are taken every `Scaled::decide_every` ticks
//! (hypothesis Г8: 20 − 10·P); a running attack advances every tick, so its phases keep the
//! exact tick lengths of data/mobs.json.

use crate::attack::{Phase, Strikes, Timeline};
use crate::data::{AttackDef, MobDef};
use crate::geom::{self, V3};
use crate::scale::Scaled;

/// Reach of a melee blow in blocks, centre to centre, when the data names none
/// (`params.reach_blocks`). An assumption of this prototype, not a number from the bestiary.
pub const DEFAULT_REACH: f64 = 2.6;
/// The brute's maul is longer.
pub const HEAVY_REACH: f64 = 3.6;
/// A dash or a dive lands when the mob comes this close.
pub const DASH_CONTACT: f64 = 1.8;
/// A melee blow lands on what is within this many degrees of where the mob faces.
pub const MELEE_ARC: f64 = 100.0;
/// Contact with the bog slime sticks this often at most.
pub const STICK_EVERY: u64 = 20;
/// A charge that hits no wall ends this many ticks into its recovery (the long window is the
/// wall's only: `recover_if: "wall"`).
pub const CHARGE_OPEN_RECOVER: u32 = 10;
/// A jump that never lands counts as landed after this many ticks.
pub const LANDING_TIMEOUT: u64 = 40;
/// A walking mob stops this close to where it walks (its target, its post).
pub const CLOSE: f64 = 1.6;

/// How a mob fights, from its id. Every one runs its attacks from mobs.json.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Revenant: shield, waits out a raised shield, backs off after a series.
    Duelist,
    /// Stalker: circles, lunges, hops back, flees when hurt.
    Skirmisher,
    /// Marksman: keeps its distance, aimed shots, butt strike up close.
    Shooter,
    /// Brute: slow, wide swing, slam, charge from afar, rage.
    Heavy,
    /// Shrike: circles above, dives.
    Diver,
    /// Lurker: creeps up when unseen, lunges from the water.
    Ambusher,
    /// Bog slime: high jumps that land on an area, sticks on contact.
    Jumper,
}

impl Style {
    pub fn of(mob_id: &str) -> Self {
        match mob_id {
            "stalker" => Style::Skirmisher,
            "marksman" => Style::Shooter,
            "brute" => Style::Heavy,
            "shrike" => Style::Diver,
            "lurker" => Style::Ambusher,
            "bog_slime" => Style::Jumper,
            _ => Style::Duelist,
        }
    }

    /// Flyers and swimmers are steered by velocity, not walked by the navigation.
    pub fn steered(self) -> bool {
        matches!(self, Style::Diver | Style::Ambusher)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetSense {
    pub pos: V3,
    /// Where the target faces (`Entity.getYRot`): the stalker flanks it.
    pub yaw: f64,
    /// A shield raised and held long enough to block.
    pub shield_up: bool,
    /// The target's view points at the mob (within 60°): the plan's `sees_me`. The lurker creeps
    /// and the stalker strikes while it is false.
    pub looking_at_me: bool,
    /// A solid block right behind the target, away from the mob.
    pub at_wall: bool,
    /// A roof over the target (the shrike waits).
    pub under_roof: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Senses {
    pub now: u64,
    pub pos: V3,
    pub yaw: f64,
    pub health_frac: f64,
    pub on_ground: bool,
    pub in_water: bool,
    /// The mob ran into a wall this tick (`horizontalCollision`).
    pub wall_hit: bool,
    /// Another shrike holds the dive turn on this target ([`DiveTurns`]): this one circles.
    pub wait_turn: bool,
    /// Blows of the grabbed target that landed on the mob since the last tick (the server tells them
    /// apart from other damage).
    pub blows: u32,
    pub target: Option<TargetSense>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    /// Stand (stop walking).
    Stay,
    /// Walk there with the engine's pathfinding, at `speed` times the mob's speed.
    Walk { to: V3, speed: f64 },
    /// Fly or swim straight there (velocity steering), `speed` blocks a tick.
    Steer { to: V3, speed: f64 },
    /// Move `per_tick` blocks a tick along `dir` (a dash, a dive).
    Dash { dir: V3, per_tick: f64 },
    /// Jump towards a point (the slime's high jump).
    Leap { to: V3 },
}

/// A blow due this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub attack: usize,
    /// The single target is within reach and in front.
    pub hits_target: bool,
    /// Everything within this radius of the mob is hit (a slam's wave, a slime's landing).
    pub area: Option<f64>,
    /// Shoot a projectile at the target instead of a blow.
    pub ranged: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    AttackStart { attack: usize },
    Phase { attack: usize, phase: Phase, t: u32 },
    AttackEnd { attack: usize, t: u32 },
    Mode(&'static str),
    /// The slime touched its target: it sticks (`passives.stick`).
    Contact,
}

/// The clip the clients play: empty for the mob's walk or idle, chosen by the client from its
/// movement; `start` is the game tick it started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clip {
    pub name: String,
    pub start: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub order: Order,
    /// Ask the navigation for a new path (a decision tick or a new order).
    pub repath: bool,
    pub look_at: Option<V3>,
    pub events: Vec<Event>,
    pub strikes: Vec<Strike>,
    /// The revenant's shield is up (frontal blows are cut).
    pub guarding: bool,
    pub rage: bool,
    /// On the mob's speed (the brute's rage).
    pub speed_mult: f64,
    /// A label for the debug view: `overhead:hold 12`, `circle`.
    pub label: String,
    /// The grab: where its target is held this tick (the mouth).
    pub hold: Option<V3>,
    /// The slime in the air: its shadow shows at the landing point this tick.
    pub shadow: Option<V3>,
    /// The marksman's reload starts: the server looks for cover from the target
    /// (`cover::reload_cover`) and hands it to [`Brain::set_cover`].
    pub wants_cover: bool,
    /// The marksman needs a spot in its kept distance: the server grounds [`post_candidates`],
    /// picks one ([`pick_post`]) and hands it to [`Brain::set_post`].
    pub wants_post: bool,
    /// The walk ends on `to` itself (navigation reach 0), not within a block of it: the
    /// marksman's cover.
    pub exact_walk: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Engage,
    Circle,
    BackOff { until: u64, from: V3 },
    Pause { until: u64 },
    Flee { until: u64 },
    /// The brute's roar as its rage starts (`passives.rage.clip_ticks`): it stands and roars.
    Roar { until: u64 },
    /// The lurker holds its target (the attack `grab`, after `ambush` landed): since when, until
    /// when at most, the blows taken meanwhile.
    Grab { attack: usize, since: u64, until: u64, hits: u32 },
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Engage => "engage",
            Mode::Circle => "circle",
            Mode::BackOff { .. } => "backoff",
            Mode::Pause { .. } => "pause",
            Mode::Flee { .. } => "flee",
            Mode::Roar { .. } => "rage",
            Mode::Grab { .. } => "grab",
        }
    }
}

/// After its attack the lurker backs off into the dark this long, and after a grab too.
const AMBUSH_BACKOFF: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    idx: usize,
    start: u64,
    landed: u32,
    phase: Phase,
    /// A dash's way, fixed when the hit phase starts.
    dir: V3,
    per_tick: f64,
    /// The tick the attack ends at when it ends before its clip (an open charge).
    ends_at: Option<u32>,
    airborne_since: Option<u64>,
    landed_jump: bool,
    /// Where it falls on: the shrike's dive point, fixed as its wings fold (the windup's last
    /// tick); the slime's landing.
    point: Option<V3>,
    /// The jump's tick of landing: its window (the recovery) runs from there.
    landed_at: Option<u32>,
}

/// One mob's mind; its fields live as long as the mob does in memory.
#[derive(Clone, Debug)]
pub struct Brain {
    pub style: Style,
    mode: Mode,
    mode_since: u64,
    run: Option<Run>,
    series: u32,
    next_decision: u64,
    rng: u64,
    forced: Option<usize>,
    clip: Clip,
    circle_dir: f64,
    bash_due: bool,
    fled: bool,
    last_stick: u64,
    /// The brute roared once its rage started.
    raged: bool,
    /// The stalker's flank began then (it strikes unseen, or once `flank_max_ticks` passed).
    flank_since: Option<u64>,
    /// The marksman's cover for this reload, and the tick it gives it up when it never gets there.
    cover: Option<V3>,
    cover_until: u64,
    /// The marksman's spot in its kept distance.
    post: Option<V3>,
}

impl Brain {
    pub fn new(style: Style, seed: u64, now: u64) -> Self {
        Self {
            style,
            mode: Mode::Engage,
            mode_since: now,
            run: None,
            series: 0,
            next_decision: now,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            forced: None,
            clip: Clip { name: String::new(), start: now },
            circle_dir: if seed % 2 == 0 { 1.0 } else { -1.0 },
            bash_due: false,
            fled: false,
            last_stick: 0,
            raged: false,
            flank_since: None,
            cover: None,
            cover_until: 0,
            post: None,
        }
    }

    /// Start this attack on the next tick, whatever the range (`/sc force`, the self-test).
    pub fn force(&mut self, attack: usize) {
        self.forced = Some(attack);
    }

    /// The cover the server found for this reload ([`Tick::wants_cover`]); none: it steps back.
    pub fn set_cover(&mut self, spot: Option<V3>) {
        self.cover = spot;
    }

    /// The spot the server picked in the kept distance ([`Tick::wants_post`]).
    pub fn set_post(&mut self, spot: Option<V3>) {
        self.post = spot;
    }

    /// The marksman's cover it is still on its way to, after the reload: not yet within
    /// `cover::ARRIVAL_SLACK` of it on both ground axes (the square that is hidden), and not given up.
    fn cover_walk(&self, s: &Senses) -> Option<V3> {
        let near = |a: f64, b: f64| (a - b).abs() <= crate::cover::ARRIVAL_SLACK;
        self.cover
            .filter(|_| s.now < self.cover_until)
            .filter(|c| !(near(c[0], s.pos[0]) && near(c[2], s.pos[2])))
    }

    pub fn clip(&self) -> &Clip {
        &self.clip
    }

    /// The attack running and how many ticks into it.
    pub fn attack(&self, now: u64) -> Option<(usize, u32)> {
        self.run.map(|r| (r.idx, (now - r.start) as u32))
    }

    fn roll(&mut self) -> f64 {
        // xorshift64*: deterministic per mob, so a test replays a fight.
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn set_mode(&mut self, mode: Mode, now: u64, events: &mut Vec<Event>) {
        if std::mem::discriminant(&mode) != std::mem::discriminant(&self.mode) {
            events.push(Event::Mode(mode.name()));
        }
        self.mode = mode;
        self.mode_since = now;
    }

    fn set_clip(&mut self, name: &str, now: u64) {
        if self.clip.name != name {
            self.clip = Clip { name: name.to_owned(), start: now };
        }
    }

    pub fn tick(&mut self, mob: &MobDef, sc: &Scaled, s: &Senses) -> Tick {
        let rage = self.style == Style::Heavy
            && mob
                .passive("rage")
                .and_then(|r| r.get("below_health")?.as_f64())
                .is_some_and(|below| s.health_frac < below);
        let mut out = Tick {
            order: Order::Stay,
            repath: false,
            look_at: s.target.map(|t| geom::add(t.pos, [0.0, 1.5, 0.0])),
            events: Vec::new(),
            strikes: Vec::new(),
            guarding: false,
            rage,
            speed_mult: if rage { rage_mult(mob) } else { 1.0 },
            label: String::new(),
            hold: None,
            shadow: None,
            wants_cover: false,
            wants_post: false,
            exact_walk: false,
        };
        let decide = s.now >= self.next_decision;
        if decide {
            self.next_decision = s.now + u64::from(sc.decide_every.max(1));
        }
        if let Some(idx) = self.forced.take() {
            self.begin(idx, s.now, &mut out.events);
        }
        if self.run.is_some() {
            self.advance(mob, s, &mut out);
        }
        if self.run.is_none() {
            self.mind(mob, s, decide, &mut out);
            if self.run.is_some() {
                // An attack started this tick: its first tick is tick 0.
                self.advance(mob, s, &mut out);
            }
        }
        // A walk to the marksman's cover goes onto the spot itself: a path done within a block
        // of it stops in sight (`marksman_reload_cover` on the bench).
        out.exact_walk = matches!(out.order, Order::Walk { to, .. } if Some(to) == self.cover);
        out
    }

    fn begin(&mut self, idx: usize, now: u64, events: &mut Vec<Event>) {
        // A new attack ends the walk to the last reload's cover.
        self.cover = None;
        self.run = Some(Run {
            idx,
            start: now,
            landed: 0,
            phase: Phase::Windup,
            dir: [0.0, 0.0, 1.0],
            per_tick: 0.0,
            ends_at: None,
            airborne_since: None,
            landed_jump: false,
            point: None,
            landed_at: None,
        });
        events.push(Event::AttackStart { attack: idx });
        events.push(Event::Phase { attack: idx, phase: Phase::Windup, t: 0 });
    }

    /// One tick of the running attack: its phase, its blows, its movement.
    fn advance(&mut self, mob: &MobDef, s: &Senses, out: &mut Tick) {
        let Some(mut run) = self.run else { return };
        let Some(a) = mob.attacks.get(run.idx) else {
            self.run = None;
            return;
        };
        let line = Timeline::of(a);
        let t = (s.now - run.start) as u32;
        let mut phase = line.phase_at(t);
        if run.ends_at.is_some_and(|end| t >= end) {
            phase = Phase::Done;
        }
        if let Some(p) = jump_phase(&run, &line, t) {
            phase = p;
        }
        let entered = phase != run.phase;
        if entered {
            run.phase = phase;
            out.events.push(Event::Phase { attack: run.idx, phase, t });
        }
        if t == 0 {
            self.set_clip(&a.clip, run.start);
        }
        // The reload starts on the phase change to recover, not on its exact tick: a tick the
        // server skips (the mob in turret cover, an event steering it) would lose the cover ask.
        if entered && phase == Phase::Recover && self.style == Style::Shooter && a.projectile.is_some() {
            // The reload: its clip (the `hide` move's) all through, and cover asked for. Not there
            // by the reload's end, it keeps going for one more reload's length.
            if let Some(clip) = mob.move_clip("hide") {
                self.set_clip(clip, s.now);
            }
            self.cover = None;
            self.cover_until = s.now + 2 * u64::from(a.recover);
            out.wants_cover = true;
        }
        let reach = reach_of(mob, a, self.style);
        match phase {
            Phase::Windup => {
                if self.style == Style::Diver && t + 1 == line.windup {
                    // The wings fold: the dive's point is fixed here and does not follow after.
                    run.point = s.target.map(|tg| geom::add(tg.pos, [0.0, 0.5, 0.0]));
                }
            }
            Phase::Hold => match (mob.param("hold_step_speed"), s.target) {
                // The revenant's delayed blow follows: it faces the target and steps after it, so
                // stepping back in the hold is caught up with and only the hit tick's dodge counts.
                (Some(speed), Some(tg)) => out.order = walk_up(s.pos, tg.pos, speed),
                // Without the number a delayed blow commits: it stops tracking the target.
                _ => out.look_at = None,
            },
            Phase::Hit => {
                if t == line.hit_start() {
                    self.start_hit(&mut run, a, &line, s, out);
                } else if run.per_tick > 0.0 {
                    out.order = Order::Dash { dir: run.dir, per_tick: run.per_tick };
                }
                if run.per_tick > 0.0 {
                    out.look_at = None;
                    if a.recover_if.as_deref() == Some("wall") && s.wall_hit {
                        // Into a wall: the dash stops and the long window opens.
                        run.per_tick = 0.0;
                        out.order = Order::Stay;
                    }
                }
            }
            Phase::Recover => {
                if run.ends_at.is_none() && a.recover_if.as_deref() == Some("wall") && run.per_tick > 0.0 {
                    // The charge ran its length into nothing: a short window, not the wall's.
                    run.ends_at = Some(line.recover_start() + CHARGE_OPEN_RECOVER);
                }
                out.order = self.recover_order(mob, s);
            }
            Phase::Done => {}
        }
        if self.style != Style::Jumper {
            self.strike(&mut run, a, reach, t, s, out);
        }
        if self.style == Style::Jumper {
            self.track_landing(&mut run, mob, a, s, out);
            if run.landed_at == Some(t) && phase == Phase::Hit {
                // Down: the window opens now.
                phase = Phase::Recover;
                run.phase = phase;
                out.events.push(Event::Phase { attack: run.idx, phase, t });
            }
        }
        // The dive's clip holds until it touches the target or the ground, or its window ends.
        if self.style == Style::Diver
            && self.clip.name == "dive"
            && let Strikes::Dash { to, .. } = strike_window(self.style, a)
            && (run.landed > 0 || (s.on_ground && t > line.hit_start()) || t >= to)
        {
            self.set_clip("climb", s.now);
        }
        out.label = format!("{}:{} {}", a.id, phase.label(), t);
        if let Some(grab) = self.grab_after(mob, a, &run, out) {
            // The ambush bit: it ends on its blow and the grab holds the target from this tick.
            out.events.push(Event::Phase { attack: run.idx, phase: Phase::Done, t });
            out.events.push(Event::AttackEnd { attack: run.idx, t });
            self.run = None;
            self.series += 1;
            self.begin_grab(mob, grab, s, out);
            return;
        }
        if phase == Phase::Done {
            out.events.push(Event::AttackEnd { attack: run.idx, t });
            self.run = None;
            self.after_attack(mob, a, s, &mut out.events);
            self.set_clip(self.mode_clip(mob), s.now);
        } else {
            self.run = Some(run);
        }
    }

    /// The blow due on tick `t`, if any: whether it lands is decided here from where the target
    /// stands (reach and arc for a blow, contact for a dash or a dive).
    fn strike(&mut self, run: &mut Run, a: &AttackDef, reach: f64, t: u32, s: &Senses, out: &mut Tick) {
        let strikes = strike_window(self.style, a);
        if !strikes.strikes_on(t, run.landed) {
            return;
        }
        let hit = s.target.is_some_and(|tg| match strikes {
            Strikes::Dash { .. } if self.style == Style::Diver => geom::dist(s.pos, tg.pos) <= DASH_CONTACT,
            Strikes::Dash { .. } => geom::flat_dist(s.pos, tg.pos) <= DASH_CONTACT,
            Strikes::At(_) => {
                geom::flat_dist(s.pos, tg.pos) <= reach
                    && geom::off_facing(s.pos, s.yaw, tg.pos) <= a.arc_degrees.unwrap_or(MELEE_ARC) / 2.0
            }
        });
        let dash = matches!(strikes, Strikes::Dash { .. });
        if dash && !hit {
            return;
        }
        run.landed += 1;
        out.strikes.push(Strike {
            attack: run.idx,
            hits_target: hit && a.projectile.is_none(),
            area: a.wave.as_ref().map(|w| w.radius_blocks),
            ranged: a.projectile.is_some(),
        });
    }

    fn start_hit(&mut self, run: &mut Run, a: &AttackDef, line: &Timeline, s: &Senses, out: &mut Tick) {
        let Some(tg) = s.target else { return };
        if self.style == Style::Diver {
            // The dive falls on the point fixed as the wings folded (where the target stands now
            // for a windup too short to fold), in the hit phase's ticks.
            let to = run.point.unwrap_or_else(|| geom::add(tg.pos, [0.0, 0.5, 0.0]));
            self.set_clip("dive", s.now);
            let d = geom::sub(to, s.pos);
            let l = geom::len(d).max(1e-6);
            run.dir = geom::scale(d, 1.0 / l);
            run.per_tick = l / f64::from(line.hit.max(1));
            out.order = Order::Dash { dir: run.dir, per_tick: run.per_tick };
        } else if let Some(blocks) = a.dash_blocks {
            run.dir = geom::flat_dir(s.pos, tg.pos);
            run.per_tick = blocks / f64::from(line.hit.max(1));
            out.order = Order::Dash { dir: run.dir, per_tick: run.per_tick };
        } else if self.style == Style::Jumper {
            out.order = Order::Leap { to: tg.pos };
            run.airborne_since = Some(s.now);
            run.point = Some(tg.pos);
            self.set_clip("jump_air", s.now);
        }
    }

    /// The slime's blow is its landing, an area, when it is down again after the jump. In the air
    /// it shows `jump_air` and, every `params.shadow_every_ticks`, its shadow where it lands.
    fn track_landing(&mut self, run: &mut Run, mob: &MobDef, a: &AttackDef, s: &Senses, out: &mut Tick) {
        let Some(since) = run.airborne_since else { return };
        if run.landed_jump {
            return;
        }
        let up = s.now.saturating_sub(since);
        // Fallen into water counts as down: a slime afloat is never on the ground.
        if (up >= 3 && (s.on_ground || s.in_water)) || up >= LANDING_TIMEOUT {
            run.landed_jump = true;
            run.landed_at = Some((s.now - run.start) as u32);
            run.landed += 1;
            let hit = s
                .target
                .is_some_and(|t| geom::dist(s.pos, t.pos) <= a.wave.as_ref().map_or(DEFAULT_REACH, |w| w.radius_blocks));
            out.strikes.push(Strike {
                attack: run.idx,
                hits_target: hit,
                area: a.wave.as_ref().map(|w| w.radius_blocks),
                ranged: false,
            });
            // Down again: the attack's own clip from its recovery, which starts now.
            let recover = u64::from(Timeline::of(a).recover_start());
            self.clip = Clip { name: a.clip.clone(), start: s.now.saturating_sub(recover) };
            return;
        }
        let every = mob.param("shadow_every_ticks").map_or(0, |e| e as u64);
        if every > 0 && up.is_multiple_of(every) {
            out.shadow = run.point;
        }
    }

    /// The grab that follows attack `a` when its blow just landed on the target this tick.
    fn grab_after(&self, mob: &MobDef, a: &AttackDef, run: &Run, out: &Tick) -> Option<usize> {
        if self.style != Style::Ambusher || !out.strikes.iter().any(|st| st.attack == run.idx && st.hits_target) {
            return None;
        }
        mob.attacks.iter().position(|g| g.follows.as_deref() == Some(a.id.as_str()))
    }

    fn begin_grab(&mut self, mob: &MobDef, attack: usize, s: &Senses, out: &mut Tick) {
        let g = &mob.attacks[attack];
        let until = s.now + u64::from(g.grab_max_ticks.unwrap_or(0));
        self.set_mode(Mode::Grab { attack, since: s.now, until, hits: 0 }, s.now, &mut out.events);
        self.set_clip(&g.clip, s.now);
    }

    /// One tick of the grab: the target held at the mouth (drawn down with `pulls_down`), a bite
    /// every `grab_hit_every_ticks`. `break_hits` blows of the grabbed target (`Senses::blows`), the
    /// end of `grab_max_ticks` or the grabbed target gone (the server keeps the target on it) free
    /// it and the mob backs off: false then.
    fn grab(&mut self, mob: &MobDef, s: &Senses, out: &mut Tick) -> bool {
        let Mode::Grab { attack, since, until, mut hits } = self.mode else { return false };
        let Some(g) = mob.attacks.get(attack) else {
            self.set_mode(Mode::Engage, s.now, &mut out.events);
            return false;
        };
        // Only the grabbed target's own blows count (the server tells them from poison or a turret).
        hits += s.blows;
        let broken = g.break_hits.is_some_and(|n| hits >= n);
        if broken || s.now >= until || s.target.is_none() {
            let from = s.target.map_or(s.pos, |t| t.pos);
            self.set_mode(Mode::BackOff { until: s.now + AMBUSH_BACKOFF, from }, s.now, &mut out.events);
            return false;
        }
        self.mode = Mode::Grab { attack, since, until, hits };
        out.look_at = None;
        out.hold = Some(geom::add(s.pos, geom::scale(geom::yaw_dir(s.yaw), g.mouth_ahead_blocks.unwrap_or(0.0))));
        out.order = match g.pull_down_per_tick {
            Some(per_tick) if g.pulls_down => Order::Dash { dir: [0.0, -1.0, 0.0], per_tick },
            _ => Order::Stay,
        };
        let held = s.now - since;
        if g.grab_hit_every_ticks.is_some_and(|every| every > 0 && held > 0 && held.is_multiple_of(u64::from(every))) {
            out.strikes.push(Strike { attack, hits_target: true, area: None, ranged: false });
        }
        out.label = format!("{}:hold {held}", g.id);
        true
    }

    fn recover_order(&self, mob: &MobDef, s: &Senses) -> Order {
        let Some(tg) = s.target else { return Order::Stay };
        match self.style {
            // The shooter reloads behind the cover the server found, else walking away.
            Style::Shooter => match self.cover {
                Some(spot) => Order::Walk { to: spot, speed: crate::cover::COVER_SPEED },
                None => Order::Walk { to: away(s.pos, tg.pos, 4.0), speed: 1.0 },
            },
            // The shrike climbs low over the ground.
            Style::Diver => Order::Steer {
                to: geom::add(s.pos, [0.0, 0.25, 0.0]),
                speed: 0.25,
            },
            _ => {
                let _ = mob;
                Order::Stay
            }
        }
    }

    fn after_attack(&mut self, mob: &MobDef, a: &AttackDef, s: &Senses, events: &mut Vec<Event>) {
        self.series += 1;
        let from = s.target.map_or(s.pos, |t| t.pos);
        match self.style {
            Style::Duelist => {
                if a.id == "shield_bash" {
                    self.bash_due = false;
                }
                let n = mob.param("attacks_before_backstep").unwrap_or(2.0) as u32;
                if self.series >= n {
                    self.series = 0;
                    self.set_mode(Mode::BackOff { until: s.now + 16, from }, s.now, events);
                } else {
                    self.set_mode(Mode::Pause { until: s.now + 6 }, s.now, events);
                }
            }
            Style::Skirmisher | Style::Ambusher => {
                self.flank_since = None;
                let until = s.now + if self.style == Style::Ambusher { AMBUSH_BACKOFF } else { 10 };
                self.set_mode(Mode::BackOff { until, from }, s.now, events);
            }
            Style::Shooter if a.projectile.is_none() => {
                self.set_mode(Mode::BackOff { until: s.now + 12, from }, s.now, events);
            }
            _ => self.set_mode(Mode::Pause { until: s.now + 8 }, s.now, events),
        }
    }

    /// The clip of a mode that has one (circling, backing off); empty for walk and idle.
    fn mode_clip(&self, mob: &MobDef) -> &'static str {
        let has = |id: &str| mob.moves.iter().any(|m| m.id == id);
        match (self.style, self.mode) {
            (Style::Duelist | Style::Skirmisher, Mode::Circle) => "circle_strafe",
            (_, Mode::BackOff { .. }) if has("backstep") || has("hop_back") => "backstep",
            (Style::Ambusher, Mode::BackOff { .. }) => "retreat",
            (Style::Skirmisher, Mode::Flee { .. }) => "run",
            (_, Mode::Roar { .. }) => "rage",
            _ => "",
        }
    }

    /// Between attacks: the mode, the order, and on decision ticks the next attack.
    fn mind(&mut self, mob: &MobDef, s: &Senses, decide: bool, out: &mut Tick) {
        let before = self.mode;
        match self.mode {
            Mode::BackOff { until, .. } | Mode::Pause { until } | Mode::Flee { until } | Mode::Roar { until }
                if s.now >= until =>
            {
                self.set_mode(Mode::Engage, s.now, &mut out.events);
            }
            _ => {}
        }
        if matches!(self.mode, Mode::Grab { .. }) && self.grab(mob, s, out) {
            return;
        }
        let Some(tg) = s.target else {
            self.set_clip("", s.now);
            out.label = "idle".into();
            self.flank_since = None;
            self.post = None;
            self.cover = None;
            return;
        };
        if out.rage && !self.raged {
            // The rage starts: once, the brute stands and roars (`passives.rage.clip_ticks`).
            self.raged = true;
            if let Some(ticks) = mob.passive("rage").and_then(|r| r.get("clip_ticks")?.as_u64()) {
                self.set_mode(Mode::Roar { until: s.now + ticks }, s.now, &mut out.events);
            }
        }
        let d = geom::flat_dist(s.pos, tg.pos);
        if decide && matches!(self.mode, Mode::Engage | Mode::Circle) {
            self.decide(mob, s, &tg, d, &mut out.events);
        }
        if self.run.is_some() {
            return;
        }
        let to_cover = if self.style == Style::Shooter { self.cover_walk(s) } else { None };
        out.wants_post = decide
            && self.style == Style::Shooter
            && self.mode == Mode::Engage
            && to_cover.is_none()
            && self.held_post(mob, &tg).is_none();
        out.order = match to_cover {
            // After the reload, still on its way to cover: it keeps going (soulscraft_selftest
            // `marksman_reload_cover`: the walk took longer than the reload on the bench).
            Some(spot) => Order::Walk { to: spot, speed: crate::cover::COVER_SPEED },
            None => self.order(mob, s, &tg, d),
        };
        out.guarding = self.style == Style::Duelist;
        out.repath = decide || before != self.mode;
        self.set_clip(self.mode_clip(mob), s.now);
        out.label = self.mode.name().into();
    }

    fn decide(&mut self, mob: &MobDef, s: &Senses, tg: &TargetSense, d: f64, events: &mut Vec<Event>) {
        let pick = |id: &str| mob.attack_index(id);
        let start = match self.style {
            Style::Duelist => self.decide_duelist(mob, s, tg, d, events),
            Style::Skirmisher => self.decide_skirmisher(mob, s, tg, d, events),
            Style::Shooter => {
                let (lo, hi) = mob.param_range("keep_distance_blocks").unwrap_or((10.0, 16.0));
                let melee = mob.param("melee_below_blocks").unwrap_or(4.0);
                let butt = mob.attack("butt_strike");
                if d < melee && butt.is_some_and(|a| d <= reach_of(mob, a, self.style)) {
                    pick("butt_strike")
                } else if (lo..=hi).contains(&d) && self.cover_walk(s).is_none() {
                    pick("aimed_shot")
                } else {
                    None
                }
            }
            Style::Heavy => {
                let far = mob.param("charge_if_farther_than_blocks").unwrap_or(6.0);
                if d > far && d < 16.0 {
                    pick("charge")
                } else if mob.attack("wide_swing").is_some_and(|a| d <= reach_of(mob, a, self.style)) {
                    if self.roll() < 0.6 { pick("wide_swing") } else { pick("overhead_wave") }
                } else {
                    None
                }
            }
            Style::Diver => {
                let hovering = s.now.saturating_sub(self.mode_since) >= 40;
                let (lo, hi) = mob.param_range("circle_height_blocks").unwrap_or((8.0, 12.0));
                let above = s.pos[1] - tg.pos[1];
                // Another shrike's dive at this target: this one keeps circling (DiveTurns).
                if hovering && !s.wait_turn && !tg.under_roof && above >= lo - 2.0 && above <= hi + 2.0 && d < 8.0 {
                    pick("dive")
                } else {
                    None
                }
            }
            Style::Ambusher => {
                let dash = mob.attack("ambush").and_then(|a| a.dash_blocks).unwrap_or(6.0);
                if s.in_water && geom::dist(s.pos, tg.pos) <= dash { pick("ambush") } else { None }
            }
            Style::Jumper => {
                if d <= 8.0 && s.on_ground { pick("high_jump") } else { None }
            }
        };
        if let Some(idx) = start {
            self.begin(idx, s.now, events);
        }
    }

    fn decide_duelist(
        &mut self,
        mob: &MobDef,
        s: &Senses,
        tg: &TargetSense,
        d: f64,
        events: &mut Vec<Event>,
    ) -> Option<usize> {
        let (_, ring_max) = mob.param_range("circle_radius_blocks").unwrap_or((2.0, 3.0));
        let wait = mob.param("circle_wait_max_ticks").unwrap_or(60.0) as u64;
        let reach = mob.attack("slash").map_or(DEFAULT_REACH, |a| reach_of(mob, a, self.style));
        if self.mode == Mode::Circle {
            if !tg.shield_up {
                self.set_mode(Mode::Engage, s.now, events);
            } else if s.now - self.mode_since >= wait {
                // Waited long enough: the shield bash opens the guard.
                self.bash_due = true;
                self.set_mode(Mode::Engage, s.now, events);
            } else {
                return None;
            }
        }
        if self.bash_due {
            return (d <= reach).then(|| mob.attack_index("shield_bash")).flatten();
        }
        if tg.shield_up && d <= ring_max + 1.5 {
            self.set_mode(Mode::Circle, s.now, events);
            return None;
        }
        if d > reach {
            return None;
        }
        let r = self.roll();
        let id = if r < 0.375 {
            "thrust"
        } else if r < 0.75 {
            "slash"
        } else {
            "overhead"
        };
        mob.attack_index(id)
    }

    fn decide_skirmisher(
        &mut self,
        mob: &MobDef,
        s: &Senses,
        tg: &TargetSense,
        d: f64,
        events: &mut Vec<Event>,
    ) -> Option<usize> {
        let flee_below = mob.param("flee_below_health").unwrap_or(0.25);
        if !self.fled && s.health_frac < flee_below {
            self.fled = true;
            let back = mob.param("return_after_ticks").unwrap_or(200.0) as u64;
            self.set_mode(Mode::Flee { until: s.now + back }, s.now, events);
            // Back from fleeing, it flanks anew: the flight is no time spent flanking.
            self.flank_since = None;
            return None;
        }
        // The flank: it circles to the target's side or back while the target looks at it, and
        // strikes once it does not, or when the flank has taken `flank_max_ticks`.
        let since = *self.flank_since.get_or_insert(s.now);
        let max = mob.param("flank_max_ticks").unwrap_or(100.0) as u64;
        if tg.looking_at_me && s.now.saturating_sub(since) < max {
            self.set_mode(Mode::Circle, s.now, events);
            return None;
        }
        self.set_mode(Mode::Engage, s.now, events);
        let double = mob.attack("double_slash")?;
        if d <= reach_of(mob, double, self.style) {
            return mob.attack_index("double_slash");
        }
        let dash = mob.attack("lunge").and_then(|a| a.dash_blocks).unwrap_or(6.0);
        let chance = if tg.at_wall && mob.param_bool("lunge_more_often_if_player_at_wall") { 0.7 } else { 0.35 };
        if (3.0..=dash).contains(&d) && self.roll() < chance {
            return mob.attack_index("lunge");
        }
        None
    }

    /// The stalker's flank point: on its ring (`circle_radius_blocks`), `flank_angle_degrees` from
    /// where the target faces, on the side the stalker is already on.
    fn flank_point(&self, mob: &MobDef, s: &Senses, tg: &TargetSense) -> V3 {
        let (lo, hi) = mob.param_range("circle_radius_blocks").unwrap_or((4.0, 6.0));
        let off = mob.param("flank_angle_degrees").unwrap_or(135.0).to_radians();
        // `ring_point` counts round from south as a yaw does, the other way: the target faces
        // the ring angle −yaw.
        let view = -tg.yaw.to_radians();
        let side = wrap_angle(geom::ring_angle(tg.pos, s.pos) - view);
        let sign = if side.abs() < 1e-6 { self.circle_dir } else { side.signum() };
        geom::ring_point(tg.pos, (lo + hi) / 2.0, view + sign * off)
    }

    /// The marksman's post, while it stands in its kept distance from the target.
    fn held_post(&self, mob: &MobDef, tg: &TargetSense) -> Option<V3> {
        let (lo, hi) = mob.param_range("keep_distance_blocks").unwrap_or((10.0, 16.0));
        self.post.filter(|p| (lo..=hi).contains(&geom::flat_dist(*p, tg.pos)))
    }

    fn order(&mut self, mob: &MobDef, s: &Senses, tg: &TargetSense, d: f64) -> Order {
        match self.mode {
            Mode::BackOff { from, .. } => {
                let blocks = mob.param("backstep_blocks").unwrap_or(3.0);
                if self.style.steered() {
                    Order::Steer { to: away(s.pos, from, blocks * 2.0), speed: 0.25 }
                } else {
                    Order::Walk { to: away(s.pos, from, blocks), speed: 1.0 }
                }
            }
            Mode::Flee { .. } => Order::Walk { to: away(s.pos, tg.pos, 12.0), speed: 1.4 },
            Mode::Pause { .. } | Mode::Roar { .. } | Mode::Grab { .. } => Order::Stay,
            Mode::Circle if self.style == Style::Skirmisher => {
                Order::Walk { to: self.flank_point(mob, s, tg), speed: mob.param("circle_speed").unwrap_or(1.0) }
            }
            Mode::Circle => {
                let (lo, hi) = mob
                    .param_range("circle_radius_blocks")
                    .unwrap_or((2.0, 3.0));
                let angle = geom::ring_angle(tg.pos, s.pos) + self.circle_dir * 0.6;
                Order::Walk { to: geom::ring_point(tg.pos, (lo + hi) / 2.0, angle), speed: 0.8 }
            }
            Mode::Engage => self.engage_order(mob, s, tg, d),
        }
    }

    fn engage_order(&mut self, mob: &MobDef, s: &Senses, tg: &TargetSense, d: f64) -> Order {
        match self.style {
            Style::Shooter => {
                if let Some(post) = self.held_post(mob, tg) {
                    return walk_up(s.pos, post, 1.0);
                }
                let (lo, hi) = mob.param_range("keep_distance_blocks").unwrap_or((10.0, 16.0));
                let mid = (lo + hi) / 2.0;
                if d < lo {
                    Order::Walk { to: away(s.pos, tg.pos, mid - d), speed: 1.0 }
                } else if d > hi {
                    Order::Walk { to: tg.pos, speed: 1.0 }
                } else {
                    Order::Stay
                }
            }
            Style::Heavy => {
                let (lo, _) = mob.param_range("keep_distance_blocks").unwrap_or((3.0, 5.0));
                if d > lo { Order::Walk { to: tg.pos, speed: 1.0 } } else { Order::Stay }
            }
            Style::Diver => {
                let (lo, hi) = mob.param_range("circle_height_blocks").unwrap_or((8.0, 12.0));
                let angle = geom::ring_angle(tg.pos, s.pos) + self.circle_dir * 0.35;
                let ring = geom::ring_point(tg.pos, 5.0, angle);
                Order::Steer { to: geom::add(ring, [0.0, (lo + hi) / 2.0, 0.0]), speed: 0.35 }
            }
            Style::Ambusher => {
                if !s.in_water || tg.looking_at_me {
                    Order::Stay
                } else {
                    Order::Steer { to: tg.pos, speed: 0.12 }
                }
            }
            Style::Jumper => Order::Walk { to: tg.pos, speed: 1.0 },
            Style::Duelist | Style::Skirmisher => walk_up(s.pos, tg.pos, 1.0),
        }
    }

    /// The slime's contact: whether it sticks to its target this tick.
    pub fn contact(&mut self, s: &Senses) -> bool {
        let Some(tg) = s.target else { return false };
        if self.style != Style::Jumper || geom::dist(s.pos, tg.pos) > 1.3 {
            return false;
        }
        if s.now.saturating_sub(self.last_stick) < STICK_EVERY && self.last_stick != 0 {
            return false;
        }
        self.last_stick = s.now.max(1);
        true
    }
}

/// The point `blocks` away from `from`, past `pos`.
pub fn away(pos: V3, from: V3, blocks: f64) -> V3 {
    let dir = geom::flat_dir(from, pos);
    geom::add(pos, geom::scale(dir, blocks.max(0.0)))
}

/// Walk to `to` at `speed`, or stand once within [`CLOSE`] of it.
fn walk_up(pos: V3, to: V3, speed: f64) -> Order {
    if geom::flat_dist(pos, to) > CLOSE { Order::Walk { to, speed } } else { Order::Stay }
}

/// An angle in radians brought into −π..π.
fn wrap_angle(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

/// Bearings round the target, from the way to the marksman, where it may take its post.
const POST_TURNS: [f64; 5] = [-60.0, -30.0, 0.0, 30.0, 60.0];

/// The marksman's possible posts in its kept distance `lo..hi` from `target`: points on three
/// rings within the band, on bearings near its own, at the target's height (the server finds the
/// ground under each), and where it stands when that is in the band already.
pub fn post_candidates(target: V3, from: V3, lo: f64, hi: f64) -> Vec<V3> {
    let bearing = geom::ring_angle(target, from);
    let span = hi - lo;
    let mut out: Vec<V3> = [lo + span / 4.0, lo + span / 2.0, hi - span / 4.0]
        .into_iter()
        .flat_map(|r| POST_TURNS.map(|turn| geom::ring_point(target, r, bearing + turn.to_radians())))
        .collect();
    if (lo..=hi).contains(&geom::flat_dist(from, target)) {
        out.insert(0, from);
    }
    out
}

/// The post among grounded candidates (`prefers_high_ground`: the highest, the nearest of equal
/// height; else the nearest to `from`).
pub fn pick_post(spots: &[V3], from: V3, prefers_high: bool) -> Option<V3> {
    spots.iter().copied().min_by(|a, b| {
        let higher = if prefers_high { b[1].total_cmp(&a[1]) } else { std::cmp::Ordering::Equal };
        higher.then(geom::flat_dist(*a, from).total_cmp(&geom::flat_dist(*b, from)))
    })
}

/// When an attack of this style can land: a dive is a dash that lands until the bird is down
/// (the hit phase's 2 ticks are the fall, plus 3 for the last of it: the open question of
/// data/README.md, «Падение сорокопута в пике», answered this way for the prototype).
pub fn strike_window(style: Style, a: &AttackDef) -> Strikes {
    if style == Style::Diver {
        let t = Timeline::of(a);
        return Strikes::Dash { from: t.hit_start(), to: t.recover_start() + 3 };
    }
    Strikes::of(a)
}

/// The phase of a jump once the slime is off the ground: its hit phase lasts the flight, its
/// recovery is the `recover` ticks after the landing (`window_note`: «15 тиков после приземления»),
/// so a big slime's higher jump keeps its blow and its window. None before the jump.
fn jump_phase(run: &Run, line: &Timeline, t: u32) -> Option<Phase> {
    run.airborne_since?;
    Some(match run.landed_at {
        None => Phase::Hit,
        Some(at) if t < at + line.recover => Phase::Recover,
        Some(_) => Phase::Done,
    })
}

/// The brute's rage speed multiplier (`passives.rage.speed_mult`).
pub fn rage_mult(mob: &MobDef) -> f64 {
    mob.passive("rage")
        .and_then(|r| r.get("speed_mult")?.as_f64())
        .unwrap_or(1.0)
}

/// How far a blow of `a` reaches: `params.reach_blocks` when the data has it, else the defaults
/// above (an assumption of this prototype).
pub fn reach_of(mob: &MobDef, a: &AttackDef, style: Style) -> f64 {
    if let Some(r) = mob.param("reach_blocks") {
        return r;
    }
    if a.dash_blocks.is_some() {
        return DASH_CONTACT;
    }
    if style == Style::Heavy { HEAVY_REACH } else { DEFAULT_REACH }
}

/// The shrikes' dive turns: one shrike at a time dives at a target. The first to start its
/// windup takes the target's turn and gives it back when its dive ends; the others circle
/// ([`Senses::wait_turn`]). The server keeps one for all its shrikes.
#[derive(Clone, Debug, Default)]
pub struct DiveTurns {
    /// Target entity to (holder, the tick the turn lapses at).
    held: std::collections::HashMap<u32, (u32, u64)>,
}

impl DiveTurns {
    /// Whether `mob` must wait: another shrike holds `target`'s turn.
    pub fn waits(&self, target: u32, mob: u32, now: u64) -> bool {
        self.held.get(&target).is_some_and(|&(holder, until)| holder != mob && now < until)
    }

    /// After `mob`'s tick: a dive started takes the turn (for at most `dive_ticks`, should its end
    /// never come), a dive ended gives it back.
    pub fn note(&mut self, target: u32, mob: u32, now: u64, tick: &Tick, dive_ticks: u64) {
        for e in &tick.events {
            match e {
                Event::AttackStart { .. } if !self.waits(target, mob, now) => {
                    self.held.insert(target, (mob, now + dive_ticks));
                }
                Event::AttackEnd { .. } => self.release(mob),
                _ => {}
            }
        }
    }

    /// `mob` died or left, or its dive ended: its turn is free.
    pub fn release(&mut self, mob: u32) {
        self.held.retain(|_, (holder, _)| *holder != mob);
    }
}

#[cfg(test)]
#[path = "brain_tests.rs"]
mod tests;
