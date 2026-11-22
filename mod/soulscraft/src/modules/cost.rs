// SoulsCraft's own time in its level ticks, whole and by part, for `/sc cost` (hypothesis Г8,
// prototypes/p5-cost.md, docs/design/12-playtest-tidewill.md).
//
// The level hook runs once per loaded level and game tick, so the time is divided by server ticks
// (distinct game times), not level ticks: with the Nether and the End loaded a per-level average
// reads a third of what a server tick pays.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The parts of the level tick timed on their own, in the order the tick runs them.
#[derive(Clone, Copy)]
pub(super) enum Part {
    /// The level's mob index (find.rs).
    Index,
    /// Queued `/sc` tasks and test waves.
    Tasks,
    /// Altars and their events, the path map included.
    Altar,
    Trophy,
    /// The spawn boost and natural spawns.
    Spawning,
    /// The Abyss Heart's off-hand check.
    Relic,
    /// The players' power summary (overworld).
    Power,
    /// SoulsCraft's own mobs: brains, blows, clip notes, bleeding.
    Own,
    /// The adopted vanilla mobs.
    Vanilla,
    Book,
}

/// How many parts.
pub(super) const PARTS: usize = 10;

pub(super) const PART_NAMES: [&str; PARTS] = ["index", "tasks", "altar", "trophy", "spawning", "relic", "power", "own_mobs", "vanilla", "book"];

struct Totals {
    nanos: u64,
    level_ticks: u64,
    server_ticks: u64,
    last_time: Option<u64>,
    parts: [u64; PARTS],
}

static TOTALS: Mutex<Totals> = Mutex::new(Totals { nanos: 0, level_ticks: 0, server_ticks: 0, last_time: None, parts: [0; PARTS] });

/// One level tick's clock: `lap` charges the time since the last lap to a part.
pub(super) struct Clock {
    started: std::time::Instant,
    last: std::time::Instant,
    parts: [u64; PARTS],
}

impl Clock {
    pub(super) fn start() -> Self {
        let now = std::time::Instant::now();
        Self { started: now, last: now, parts: [0; PARTS] }
    }

    pub(super) fn lap(&mut self, part: Part) {
        let now = std::time::Instant::now();
        self.parts[part as usize] += now.duration_since(self.last).as_nanos() as u64;
        self.last = now;
    }

    /// Adds this level tick, at game time `time`, to the totals.
    pub(super) fn finish(self, time: u64) {
        let spent = self.started.elapsed().as_nanos() as u64;
        let mut t = TOTALS.lock().unwrap_or_else(PoisonError::into_inner);
        t.nanos += spent;
        t.level_ticks += 1;
        if t.last_time != Some(time) {
            t.server_ticks += 1;
            t.last_time = Some(time);
        }
        for (sum, part) in t.parts.iter_mut().zip(self.parts) {
            *sum += part;
        }
    }
}

pub(super) fn reset() {
    *TOTALS.lock().unwrap_or_else(PoisonError::into_inner) = Totals { nanos: 0, level_ticks: 0, server_ticks: 0, last_time: None, parts: [0; PARTS] };
    for c in &CALLS {
        c.0.store(0, Ordering::Relaxed);
        c.1.store(0, Ordering::Relaxed);
    }
}

/// The engine calls timed inside the parts (the compat layer's mob calls, nested in `own_mobs` and
/// `vanilla`): how many and how long, for `/sc cost`.
#[derive(Clone, Copy)]
pub(super) enum Call {
    Look,
    Navigate,
    Stop,
    Navigating,
    Attack,
    Hurt,
    /// The steps of the own mobs' part (server.rs on_level_tick).
    Bleed,
    Players,
    Seen,
    TickMob,
    Notes,
    /// Inside one own mob's tick (server.rs tick_mob).
    Target,
    Brain,
    DefClone,
    Act,
    After,
    Drowned,
    Cover,
    Blows,
    BookSaw,
}

pub(super) const CALLS_N: usize = 20;

pub(super) const CALL_NAMES: [&str; CALLS_N] = [
    "look_at", "navigate", "stop", "navigating", "mob_attack", "hurt_entity", "own_bleed", "own_players", "own_seen", "own_tick_mob", "own_notes",
    "mob_target", "mob_brain", "mob_def_clone", "mob_act", "mob_after", "mob_drowned", "mob_cover", "mob_blows", "mob_book_saw",
];

static CALLS: [(AtomicU64, AtomicU64); CALLS_N] = [const { (AtomicU64::new(0), AtomicU64::new(0)) }; CALLS_N];

/// Runs `f`, charging its time to `call`.
pub(super) fn timed<R>(call: Call, f: impl FnOnce() -> R) -> R {
    let started = std::time::Instant::now();
    let out = f();
    let c = &CALLS[call as usize];
    c.0.fetch_add(1, Ordering::Relaxed);
    c.1.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    out
}

/// Charges `nanos` to `call` (a span `timed` can't wrap).
pub(super) fn add(call: Call, nanos: u64) {
    let c = &CALLS[call as usize];
    c.0.fetch_add(1, Ordering::Relaxed);
    c.1.fetch_add(nanos, Ordering::Relaxed);
}

/// (calls, nanoseconds) by [`CALL_NAMES`] since the last reset.
pub(super) fn calls() -> [(u64, u64); CALLS_N] {
    std::array::from_fn(|i| (CALLS[i].0.load(Ordering::Relaxed), CALLS[i].1.load(Ordering::Relaxed)))
}

/// The totals since the last reset: (nanoseconds, level ticks, server ticks, nanoseconds by part).
pub(super) fn totals() -> (u64, u64, u64, [u64; PARTS]) {
    let t = TOTALS.lock().unwrap_or_else(PoisonError::into_inner);
    (t.nanos, t.level_ticks, t.server_ticks, t.parts)
}
