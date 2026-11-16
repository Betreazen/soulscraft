//! An attack's phases in ticks, as data/README.md draws them:
//!
//! ```text
//! tick 0         windup         windup+hold        +hit             +recover
//! │── windup ─────│── hold ───────│── hit ───│── player's window ─│ end
//! ```
//!
//! The clip's markers are the same ticks (assets/models/build.py fails when they differ), so the
//! server and the client agree on the pose by counting ticks from the attack's start.

use crate::data::{AttackDef, WaveDef};
use crate::geom::{self, V3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Windup,
    Hold,
    Hit,
    Recover,
    Done,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Windup => "windup",
            Phase::Hold => "hold",
            Phase::Hit => "hit",
            Phase::Recover => "recover",
            Phase::Done => "end",
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Option<Self> {
        [Phase::Windup, Phase::Hold, Phase::Hit, Phase::Recover, Phase::Done]
            .get(usize::from(code))
            .copied()
    }
}

/// The phase lengths of one attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    pub windup: u32,
    pub hold: u32,
    pub hit: u32,
    pub recover: u32,
}

impl Timeline {
    pub fn of(a: &AttackDef) -> Self {
        Self {
            windup: a.windup,
            hold: a.hold,
            hit: a.hit,
            recover: a.recover,
        }
    }

    /// The tick the hit phase starts: the contact frame of the clip (`markers.hit`).
    pub fn hit_start(&self) -> u32 {
        self.windup + self.hold
    }

    pub fn recover_start(&self) -> u32 {
        self.hit_start() + self.hit
    }

    /// The clip's length: `markers.end`.
    pub fn end(&self) -> u32 {
        self.recover_start() + self.recover
    }

    pub fn phase_at(&self, t: u32) -> Phase {
        if t < self.windup {
            Phase::Windup
        } else if t < self.hit_start() {
            Phase::Hold
        } else if t < self.recover_start() {
            Phase::Hit
        } else if t < self.end() {
            Phase::Recover
        } else {
            Phase::Done
        }
    }

    /// The markers as the rig writes them: windup 0, hold (only with a hold), hit, recover, end.
    pub fn markers(&self) -> Vec<(&'static str, u32)> {
        let mut m = vec![("windup", 0)];
        if self.hold > 0 {
            m.push(("hold", self.windup));
        }
        m.extend([("hit", self.hit_start()), ("recover", self.recover_start()), ("end", self.end())]);
        m
    }
}

/// When an attack deals damage: on the hit tick, at each listed strike, or for a dash on the
/// first tick of the dash that the target is within reach.
#[derive(Clone, Debug, PartialEq)]
pub enum Strikes {
    At(Vec<u32>),
    Dash { from: u32, to: u32 },
}

impl Strikes {
    pub fn of(a: &AttackDef) -> Self {
        let t = Timeline::of(a);
        let start = t.hit_start();
        if a.dash_blocks.is_some() {
            return Strikes::Dash {
                from: start,
                to: t.recover_start(),
            };
        }
        match &a.strikes_at {
            Some(list) if !list.is_empty() => Strikes::At(list.iter().map(|s| start + s).collect()),
            _ => Strikes::At(vec![start]),
        }
    }

    /// Whether tick `t` of the attack may deal a blow, `landed` blows having landed already.
    pub fn strikes_on(&self, t: u32, landed: u32) -> bool {
        match self {
            Strikes::At(list) => list.contains(&t),
            Strikes::Dash { from, to } => landed == 0 && (*from..*to).contains(&t),
        }
    }
}

/// A jumpable wave passes under feet this high over the ground under them, in blocks (the
/// brief's 0,5; hypothesis, 03-hypotheses.md Г6).
pub const JUMP_CLEAR: f64 = 0.5;

/// Where a wave starts: `impact_ahead_blocks` in front of a mob at `pos` facing `yaw`.
pub fn wave_center(pos: V3, yaw: f64, wave: &WaveDef) -> V3 {
    geom::add(pos, geom::scale(geom::yaw_dir(yaw), wave.impact_ahead_blocks))
}

/// Whether a wave from `center` catches a target whose feet are at `target_feet`, the ground
/// under them at `ground_y`: within `radius`, and for a `jumpable` wave not over it (feet more
/// than [`JUMP_CLEAR`] over the ground).
pub fn wave_hits(center: V3, radius: f64, jumpable: bool, target_feet: V3, ground_y: f64) -> bool {
    if jumpable && target_feet[1] - ground_y > JUMP_CLEAR {
        return false;
    }
    geom::dist(center, target_feet) <= radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MobsFile;

    fn overhead() -> AttackDef {
        MobsFile::embedded().mob("revenant").unwrap().attack("overhead").unwrap().clone()
    }

    #[test]
    fn delayed_overhead_has_its_hold_before_the_hit() {
        let t = Timeline::of(&overhead());
        assert_eq!(t.phase_at(0), Phase::Windup);
        assert_eq!(t.phase_at(9), Phase::Windup);
        assert_eq!(t.phase_at(10), Phase::Hold);
        assert_eq!(t.phase_at(17), Phase::Hold);
        assert_eq!(t.phase_at(18), Phase::Hit);
        assert_eq!(t.phase_at(19), Phase::Hit);
        assert_eq!(t.phase_at(20), Phase::Recover);
        assert_eq!(t.phase_at(35), Phase::Recover);
        assert_eq!(t.phase_at(36), Phase::Done);
        assert_eq!(t.end(), 36);
        assert_eq!(
            t.markers(),
            vec![("windup", 0), ("hold", 10), ("hit", 18), ("recover", 20), ("end", 36)]
        );
    }

    #[test]
    fn an_attack_without_hold_has_no_hold_marker() {
        let f = MobsFile::embedded();
        let t = Timeline::of(f.mob("revenant").unwrap().attack("thrust").unwrap());
        assert_eq!(t.markers(), vec![("windup", 0), ("hit", 6), ("recover", 8), ("end", 18)]);
        assert_eq!(t.phase_at(6), Phase::Hit);
    }

    #[test]
    fn strikes_land_on_the_hit_tick_or_listed_ticks() {
        let f = MobsFile::embedded();
        let s = Strikes::of(&overhead());
        assert!(s.strikes_on(18, 0));
        assert!(!s.strikes_on(17, 0));
        assert!(!s.strikes_on(19, 0));
        let double = Strikes::of(f.mob("stalker").unwrap().attack("double_slash").unwrap());
        assert_eq!(double, Strikes::At(vec![6, 9]));
        assert!(double.strikes_on(9, 1));
    }

    #[test]
    fn a_dash_strikes_once_while_it_runs() {
        let f = MobsFile::embedded();
        let s = Strikes::of(f.mob("brute").unwrap().attack("charge").unwrap());
        assert_eq!(s, Strikes::Dash { from: 14, to: 22 });
        assert!(!s.strikes_on(13, 0));
        assert!(s.strikes_on(14, 0));
        assert!(s.strikes_on(21, 0));
        assert!(!s.strikes_on(22, 0));
        assert!(!s.strikes_on(15, 1));
    }

    #[test]
    fn every_attack_of_every_mob_has_consistent_phases() {
        for mob in MobsFile::embedded().mobs {
            for (_, a) in mob.phased_attacks() {
                let t = Timeline::of(a);
                assert!(t.hit > 0, "{} {}", mob.id, a.id);
                let mut last = Phase::Windup;
                for tick in 0..=t.end() {
                    let p = t.phase_at(tick);
                    assert!(p.code() >= last.code(), "{} {} goes back at {tick}", mob.id, a.id);
                    last = p;
                }
                assert_eq!(last, Phase::Done);
            }
        }
    }

    fn slam_wave() -> crate::data::WaveDef {
        MobsFile::embedded().mob("brute").unwrap().attack("overhead_wave").unwrap().wave.clone().unwrap()
    }

    #[test]
    fn wave_from_impact_point() {
        let w = slam_wave();
        // The brute at the origin facing south (+z): the hammer lands impact_ahead_blocks (2) ahead.
        let center = wave_center([0.0; 3], 0.0, &w);
        assert!(crate::geom::dist(center, [0.0, 0.0, 2.0]) < 1e-9, "{center:?}");
        let r = w.radius_blocks;
        // 4.9 blocks in front of the brute is 2.9 from the impact: hit.
        assert!(wave_hits(center, r, w.jumpable, [0.0, 0.0, 4.9], 0.0));
        // 2 blocks behind the brute is 4 from the impact: missed.
        assert!(!wave_hits(center, r, w.jumpable, [0.0, 0.0, -2.0], 0.0));
        // Facing east (yaw −90): the impact is at +x.
        let east = wave_center([10.0, 64.0, 0.0], -90.0, &w);
        assert!(crate::geom::dist(east, [12.0, 64.0, 0.0]) < 1e-9, "{east:?}");
        // A wave without impact_ahead_blocks (the slime's landing) is centred on the mob.
        let slime = MobsFile::embedded().mob("bog_slime").unwrap().attack("high_jump").unwrap().wave.clone().unwrap();
        assert_eq!(wave_center([1.0, 2.0, 3.0], 45.0, &slime), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn wave_misses_jumping_player() {
        let w = slam_wave();
        let center = wave_center([0.0; 3], 0.0, &w);
        assert!(w.jumpable);
        // Feet 0.6 over the ground under them: over the wave.
        assert!(!wave_hits(center, w.radius_blocks, true, [0.0, 0.6, 2.0], 0.0));
        // Feet 0.4 over it: still caught.
        assert!(wave_hits(center, w.radius_blocks, true, [0.0, 0.4, 2.0], 0.0));
        // On a step a block up the player stands on the ground there: caught.
        assert!(wave_hits(center, w.radius_blocks, true, [0.0, 1.0, 2.0], 1.0));
        // A wave that cannot be jumped catches the jumper.
        assert!(wave_hits(center, w.radius_blocks, false, [0.0, 0.6, 2.0], 0.0));
    }

    #[test]
    fn phase_codes_round_trip() {
        for p in [Phase::Windup, Phase::Hold, Phase::Hit, Phase::Recover, Phase::Done] {
            assert_eq!(Phase::from_code(p.code()), Some(p));
        }
        assert_eq!(Phase::from_code(9), None);
        assert_eq!(Phase::Done.label(), "end");
    }
}
