// SoulsCraft's own time in its level ticks, whole and by part, for `/sc cost` (hypothesis Г8,
// prototypes/p5-cost.md, docs/design/12-playtest-tidewill.md).
//
// The level hook runs once per loaded level and game tick, so the time is divided by server ticks
// (distinct game times), not level ticks: with the Nether and the End loaded a per-level average
// reads a third of what a server tick pays.
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
}

/// The totals since the last reset: (nanoseconds, level ticks, server ticks, nanoseconds by part).
pub(super) fn totals() -> (u64, u64, u64, [u64; PARTS]) {
    let t = TOTALS.lock().unwrap_or_else(PoisonError::into_inner);
    (t.nanos, t.level_ticks, t.server_ticks, t.parts)
}
