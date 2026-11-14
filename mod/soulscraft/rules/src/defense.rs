//! The altar's defense (06-altar.md, «Оборона алтаря», round 7; numbers are hypothesis Г12 in
//! data/mobs.json `events[].defense`): what a wave mob goes for by its role, the players who
//! intercept it and when it goes back, the drain at the altar with its windup, the cap on what the
//! altar loses a second, the curse of a mob with no way in, which blocks the brute may break and in
//! how many hits, and the book of broken blocks the core puts back when the event ends.

use crate::data::EventDef;
use crate::geom::{self, V3};
use serde_json::{Map, Value};

/// Г12's numbers, with their defaults when the data leaves one out.
#[derive(Clone, Debug, PartialEq)]
pub struct Numbers {
    pub intercept_blocks: f64,
    pub return_blocks: f64,
    pub return_unseen_ticks: u64,
    pub drain_windup_ticks: u64,
    pub drain_per_second: f64,
    pub cap_per_second: f64,
    pub curse_after_ticks: u64,
    pub curse_per_second: f64,
    pub breach: BreachRules,
}

fn num(m: &Map<String, Value>, k: &str, default: f64) -> f64 {
    m.get(k).and_then(Value::as_f64).unwrap_or(default)
}

fn strings(m: &Map<String, Value>, k: &str) -> Vec<String> {
    m.get(k)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

impl Numbers {
    pub fn of(event: &EventDef) -> Self {
        Self::from_map(&event.defense)
    }

    pub fn from_map(d: &Map<String, Value>) -> Self {
        let b = d.get("breach").and_then(Value::as_object).cloned().unwrap_or_default();
        let thresholds = b
            .get("hits_by_hardness")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_u64()? as u32)))
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec![(2.0, 1), (5.0, 2), (50.0, 6)]);
        Self {
            intercept_blocks: num(d, "intercept_blocks", 6.0),
            return_blocks: num(d, "return_blocks", 16.0),
            return_unseen_ticks: (num(d, "return_unseen_seconds", 10.0) * 20.0) as u64,
            drain_windup_ticks: num(d, "drain_windup_ticks", 40.0) as u64,
            drain_per_second: num(d, "drain_per_second", 1.0),
            cap_per_second: num(d, "altar_damage_cap_per_second", 4.0),
            curse_after_ticks: (num(d, "curse_after_seconds", 30.0) * 20.0) as u64,
            curse_per_second: num(d, "curse_per_second", 0.25),
            breach: BreachRules {
                thresholds,
                attack: b.get("attack").and_then(Value::as_str).unwrap_or("overhead_wave").to_owned(),
                never_namespaces: strings(&b, "never_namespaces"),
                never_blocks: strings(&b, "never_blocks"),
                never_prefixes: strings(&b, "never_prefixes"),
            },
        }
    }
}

/// What the brute may break (06-altar.md, «Пролом громилы»).
#[derive(Clone, Debug, PartialEq)]
pub struct BreachRules {
    /// (hardness up to, hits): the first row whose hardness is not below the block's.
    pub thresholds: Vec<(f64, u32)>,
    /// The brute's attack it hits a wall with.
    pub attack: String,
    /// Namespaces never broken, `rustcraft:` (every RustCraft block: rails have no block entity).
    pub never_namespaces: Vec<String>,
    pub never_blocks: Vec<String>,
    /// Id prefixes never broken: the altar's blocks and banners.
    pub never_prefixes: Vec<String>,
}

impl BreachRules {
    /// The hits a block takes, or None when the brute never breaks it: air and fluids are no
    /// wall, a block entity (a chest, a furnace), an unbreakable block (hardness below 0), the
    /// protected namespaces and blocks, and anything harder than the last row.
    pub fn hits(&self, block: &str, hardness: f32, has_entity: bool) -> Option<u32> {
        if has_entity || hardness < 0.0 {
            return None;
        }
        if crate::altar::is_altar_block(block)
            || self.never_namespaces.iter().any(|n| block.starts_with(n.as_str()))
            || self.never_prefixes.iter().any(|n| block.starts_with(n.as_str()))
            || self.never_blocks.iter().any(|b| b == block)
        {
            return None;
        }
        self.thresholds.iter().find(|(h, _)| f64::from(hardness) <= *h).map(|(_, n)| (*n).max(1))
    }
}

/// A wave mob's role (data/mobs.json `wave_role`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Goes for the altar; a player only on interception.
    Assault,
    /// Goes for the nearest participant; the altar when none is in the zone.
    Hunter,
    /// Shoots the nearest target it sees: a player or the altar.
    Ranged,
}

impl Role {
    pub fn parse(s: &str) -> Self {
        match s {
            "hunter" => Role::Hunter,
            "ranged" => Role::Ranged,
            _ => Role::Assault,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Role::Assault => "assault",
            Role::Hunter => "hunter",
            Role::Ranged => "ranged",
        }
    }
}

/// What a wave mob goes for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Altar,
    /// A participant, by entity id.
    Player(u32),
}

/// A participant as one mob sees them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub entity: u32,
    pub pos: V3,
    /// In the mob's line of sight.
    pub visible: bool,
    /// The zone's path map joins the mob and them (both have a walkable way to the altar).
    pub reachable: bool,
}

/// A wave mob's memory of the defense.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mind {
    /// The player who intercepted it.
    pub intercept: Option<u32>,
    pub last_seen: u64,
    /// When its drain at the altar started (its windup first).
    pub drain_start: Option<u64>,
    /// When it last had no walkable way to the altar, from the first tick of the run.
    pub no_path_since: Option<u64>,
    /// It stands and beams the altar through blocks; it keeps it until it dies.
    pub cursed: bool,
}

fn nearest(ps: &[Seen], from: V3, only_visible: bool) -> Option<&Seen> {
    ps.iter()
        .filter(|p| !only_visible || p.visible)
        .min_by(|a, b| geom::dist(a.pos, from).total_cmp(&geom::dist(b.pos, from)))
}

/// What the mob goes for this tick, with its interception updated: a player who hit it or came
/// within `intercept_blocks` holds it until they are farther than `return_blocks` or unseen for
/// `return_unseen_ticks`; hunters take the nearest participant, ranged mobs the nearest target in
/// sight (the altar among them).
pub fn choose(role: Role, n: &Numbers, mob: V3, altar: V3, altar_visible: bool, ps: &[Seen], mind: &mut Mind, now: u64) -> Goal {
    if let Some(id) = mind.intercept {
        match ps.iter().find(|p| p.entity == id) {
            None => mind.intercept = None,
            Some(p) => {
                if p.visible {
                    mind.last_seen = now;
                }
                let far = geom::dist(p.pos, mob) > n.return_blocks;
                let unseen = now.saturating_sub(mind.last_seen) > n.return_unseen_ticks;
                if far || unseen {
                    mind.intercept = None;
                } else {
                    return Goal::Player(id);
                }
            }
        }
    }
    match role {
        // Round 10: a participant within the distance pulls the mob only when it sees them or
        // can walk to them: not through a wall.
        Role::Assault => match ps
            .iter()
            .filter(|p| p.visible || p.reachable)
            .filter(|p| geom::dist(p.pos, mob) <= n.intercept_blocks)
            .min_by(|a, b| geom::dist(a.pos, mob).total_cmp(&geom::dist(b.pos, mob)))
        {
            Some(p) => {
                mind.intercept = Some(p.entity);
                mind.last_seen = now;
                mind.drain_start = None;
                Goal::Player(p.entity)
            }
            None => Goal::Altar,
        },
        Role::Hunter => nearest(ps, mob, false).map_or(Goal::Altar, |p| Goal::Player(p.entity)),
        Role::Ranged => {
            let player = nearest(ps, mob, true);
            let altar_d = geom::dist(altar, mob);
            match player {
                Some(p) if !altar_visible || geom::dist(p.pos, mob) < altar_d => Goal::Player(p.entity),
                _ => Goal::Altar,
            }
        }
    }
}

/// A participant hit the mob: it turns on them, and a drain in progress is broken off (a hit
/// interrupts any attack with a windup: the «честный замах» pillar).
pub fn on_hit(mind: &mut Mind, by: u32, now: u64) {
    mind.intercept = Some(by);
    mind.last_seen = now;
    mind.drain_start = None;
}

/// Where a mob at the altar is in its drain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drain {
    /// Not at the altar.
    Away,
    /// Winding up: ticks done.
    Windup(u64),
    /// Taking the altar's health.
    Draining,
}

pub fn drain(mind: &mut Mind, at_altar: bool, n: &Numbers, now: u64) -> Drain {
    if !at_altar {
        mind.drain_start = None;
        return Drain::Away;
    }
    let start = *mind.drain_start.get_or_insert(now);
    let done = now - start;
    if done < n.drain_windup_ticks { Drain::Windup(done) } else { Drain::Draining }
}

/// Counts the ticks without a walkable way and turns the mob cursed after `curse_after_ticks`.
pub fn track_path(mind: &mut Mind, has_path: bool, n: &Numbers, now: u64) -> bool {
    if has_path {
        mind.no_path_since = None;
    } else {
        let since = *mind.no_path_since.get_or_insert(now);
        if now - since >= n.curse_after_ticks {
            mind.cursed = true;
        }
    }
    mind.cursed
}

/// What hurt the altar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harm {
    Drain,
    Curse,
}

/// The altar's health in an event, with the cap on what it loses in one second.
#[derive(Clone, Debug, PartialEq)]
pub struct AltarHealth {
    pub max: f64,
    pub hp: f64,
    second: u64,
    taken: f64,
    pub last_loss: Option<u64>,
    /// Lost to drains and to curses (telemetry `altar_damage_by`).
    pub by_drain: f64,
    pub by_curse: f64,
}

impl AltarHealth {
    pub fn new(max: f64) -> Self {
        Self { max, hp: max, second: u64::MAX, taken: 0.0, last_loss: None, by_drain: 0.0, by_curse: 0.0 }
    }

    /// Takes `amount` at tick `now`, no more than the cap leaves in this second. Returns what
    /// was taken.
    pub fn hurt(&mut self, amount: f64, harm: Harm, cap: f64, now: u64) -> f64 {
        let second = now / 20;
        if second != self.second {
            self.second = second;
            self.taken = 0.0;
        }
        let take = amount.min((cap - self.taken).max(0.0)).min(self.hp).max(0.0);
        if take <= 0.0 {
            return 0.0;
        }
        self.taken += take;
        self.hp -= take;
        self.last_loss = Some(now);
        match harm {
            Harm::Drain => self.by_drain += take,
            Harm::Curse => self.by_curse += take,
        }
        take
    }

    /// Lost health within the last second: the warning flashes.
    pub fn losing(&self, now: u64) -> bool {
        self.last_loss.is_some_and(|t| now.saturating_sub(t) < 20)
    }

    pub fn fallen(&self) -> bool {
        self.hp <= 1e-9
    }

    pub fn fraction(&self) -> f64 {
        if self.max <= 0.0 { 0.0 } else { (self.hp / self.max).clamp(0.0, 1.0) }
    }
}

/// A block the brute broke: where, and the state to put back (`minecraft:stone_bricks`).
#[derive(Clone, Debug, PartialEq)]
pub struct Broken {
    pub pos: [i32; 3],
    pub state: String,
    /// A player has put something there since: it stays, the old block does not come back.
    pub overwritten: bool,
}

/// The core's book of broken blocks (06-altar.md: walls come back after the event, won or lost,
/// unless the player built there; broken blocks drop nothing).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breaches {
    pub list: Vec<Broken>,
}

impl Breaches {
    /// Records a block about to be broken; a place broken twice keeps its first block.
    pub fn record(&mut self, pos: [i32; 3], state: &str) {
        if !self.list.iter().any(|b| b.pos == pos) {
            self.list.push(Broken { pos, state: state.to_owned(), overwritten: false });
        }
    }

    /// A block changed at `pos`: anything but air there marks the place as the player's.
    pub fn changed(&mut self, pos: [i32; 3], now_air: bool) {
        if now_air {
            return;
        }
        if let Some(b) = self.list.iter_mut().find(|b| b.pos == pos) {
            b.overwritten = true;
        }
    }

    pub fn contains(&self, pos: [i32; 3]) -> bool {
        self.list.iter().any(|b| b.pos == pos)
    }

    /// The blocks to put back, then the book is empty.
    pub fn take_restore(&mut self) -> Vec<Broken> {
        std::mem::take(&mut self.list).into_iter().filter(|b| !b.overwritten).collect()
    }

    /// One line a block for the core's saved data: `x y z overwritten state`.
    pub fn to_lines(&self) -> Vec<String> {
        self.list
            .iter()
            .map(|b| format!("{} {} {} {} {}", b.pos[0], b.pos[1], b.pos[2], u8::from(b.overwritten), b.state))
            .collect()
    }

    pub fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        let list = lines
            .into_iter()
            .filter_map(|l| {
                let mut w = l.splitn(5, ' ');
                let x = w.next()?.parse().ok()?;
                let y = w.next()?.parse().ok()?;
                let z = w.next()?.parse().ok()?;
                let over = w.next()? == "1";
                let state = w.next()?.to_owned();
                Some(Broken { pos: [x, y, z], state, overwritten: over })
            })
            .collect();
        Self { list }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MobsFile;

    fn numbers() -> Numbers {
        Numbers::of(&MobsFile::embedded().events[0])
    }

    fn p(entity: u32, x: f64, visible: bool) -> Seen {
        Seen { entity, pos: [x, 0.0, 0.0], visible, reachable: true }
    }

    #[test]
    fn numbers_come_from_the_data() {
        let n = numbers();
        assert_eq!(n.intercept_blocks, 6.0);
        assert_eq!(n.return_blocks, 16.0);
        assert_eq!(n.return_unseen_ticks, 200);
        assert_eq!(n.drain_windup_ticks, 40);
        assert_eq!(n.cap_per_second, 4.0);
        assert_eq!(n.curse_after_ticks, 600);
        assert_eq!(n.curse_per_second, 0.25);
        assert_eq!(n.breach.attack, "overhead_wave");
        assert!(n.breach.never_namespaces.contains(&"rustcraft:".to_string()));
        // Defaults when the data is empty.
        let d = Numbers::from_map(&Map::new());
        assert_eq!(d.breach.thresholds, vec![(2.0, 1), (5.0, 2), (50.0, 6)]);
    }

    #[test]
    fn breach_hits_by_hardness_and_what_is_never_broken() {
        let b = numbers().breach;
        assert_eq!(b.hits("minecraft:dirt", 0.5, false), Some(1));
        assert_eq!(b.hits("minecraft:cobblestone", 2.0, false), Some(1));
        assert_eq!(b.hits("minecraft:iron_block", 5.0, false), Some(2));
        assert_eq!(b.hits("minecraft:obsidian", 50.0, false), Some(6));
        assert_eq!(b.hits("minecraft:crying_obsidian", 50.0, false), Some(6));
        assert_eq!(b.hits("minecraft:bedrock", -1.0, false), None);
        assert_eq!(b.hits("minecraft:chest", 2.5, true), None);
        assert_eq!(b.hits("rustcraft:monorail", 1.0, false), None);
        assert_eq!(b.hits("rustcraft:turret", 3.0, true), None);
        assert_eq!(b.hits("soulscraft:altar_slab", 1.5, false), None);
        assert_eq!(b.hits("soulscraft:banner_weight", 1.0, false), None);
        assert_eq!(b.hits("minecraft:barrier", 0.0, false), None);
        assert_eq!(b.hits("minecraft:ancient_debris", 1200.0, false), None);
    }

    #[test]
    fn assault_goes_for_the_altar_until_a_player_comes_close() {
        let n = numbers();
        let mut m = Mind::default();
        let altar = [20.0, 0.0, 0.0];
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(7, 10.0, true)], &mut m, 0), Goal::Altar);
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(7, 5.5, true)], &mut m, 1), Goal::Player(7));
        // Held while near and seen.
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(7, 12.0, true)], &mut m, 50), Goal::Player(7));
        // Gone beyond 16 blocks: back to the altar.
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(7, 17.0, true)], &mut m, 60), Goal::Altar);
        assert_eq!(m.intercept, None);
    }

    #[test]
    fn interception_ends_after_ten_seconds_unseen_or_when_the_player_leaves() {
        let n = numbers();
        let mut m = Mind::default();
        on_hit(&mut m, 9, 100);
        let altar = [30.0, 0.0, 0.0];
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(9, 12.0, false)], &mut m, 200), Goal::Player(9));
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[p(9, 12.0, false)], &mut m, 301), Goal::Altar);
        on_hit(&mut m, 9, 400);
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[], &mut m, 401), Goal::Altar);
    }

    #[test]
    fn hunters_and_ranged_pick_their_own_targets() {
        let n = numbers();
        let altar = [8.0, 0.0, 0.0];
        let ps = [p(1, 30.0, true), p(2, -20.0, false)];
        let mut m = Mind::default();
        assert_eq!(choose(Role::Hunter, &n, [0.0; 3], altar, true, &ps, &mut m, 0), Goal::Player(2));
        assert_eq!(choose(Role::Hunter, &n, [0.0; 3], altar, true, &[], &mut m, 0), Goal::Altar);
        // Ranged: the visible player is farther than the altar it sees.
        assert_eq!(choose(Role::Ranged, &n, [0.0; 3], altar, true, &ps, &mut m, 0), Goal::Altar);
        // The altar out of sight: the visible player.
        assert_eq!(choose(Role::Ranged, &n, [0.0; 3], altar, false, &ps, &mut m, 0), Goal::Player(1));
        assert_eq!(choose(Role::Ranged, &n, [0.0; 3], [50.0, 0.0, 0.0], true, &ps, &mut m, 0), Goal::Player(1));
        assert_eq!(Role::parse("hunter"), Role::Hunter);
        assert_eq!(Role::parse("?"), Role::Assault);
        assert_eq!(Role::Ranged.name(), "ranged");
    }

    #[test]
    fn drain_winds_up_and_a_hit_breaks_it() {
        let n = numbers();
        let mut m = Mind::default();
        assert_eq!(drain(&mut m, true, &n, 100), Drain::Windup(0));
        assert_eq!(drain(&mut m, true, &n, 139), Drain::Windup(39));
        assert_eq!(drain(&mut m, true, &n, 140), Drain::Draining);
        on_hit(&mut m, 3, 141);
        assert_eq!(drain(&mut m, true, &n, 142), Drain::Windup(0));
        assert_eq!(drain(&mut m, false, &n, 150), Drain::Away);
        assert_eq!(m.drain_start, None);
    }

    #[test]
    fn altar_damage_is_capped_per_second() {
        let mut h = AltarHealth::new(200.0);
        // Ten mobs draining 1/s each in one tick: only the cap of 4 is taken this second.
        let mut taken = 0.0;
        for _ in 0..10 {
            taken += h.hurt(1.0, Harm::Drain, 4.0, 40);
        }
        assert_eq!(taken, 4.0);
        assert_eq!(h.hurt(1.0, Harm::Drain, 4.0, 59), 0.0);
        assert_eq!(h.hurt(1.0, Harm::Curse, 4.0, 60), 1.0);
        assert_eq!(h.hp, 195.0);
        assert_eq!((h.by_drain, h.by_curse), (4.0, 1.0));
        assert!(h.losing(70) && !h.losing(81));
        assert!((h.fraction() - 0.975).abs() < 1e-9);
        // It never goes below zero.
        let mut h = AltarHealth::new(2.0);
        assert_eq!(h.hurt(3.0, Harm::Drain, 4.0, 0), 2.0);
        assert!(h.fallen());
        assert_eq!(AltarHealth::new(0.0).fraction(), 0.0);
    }

    #[test]
    fn a_mob_without_a_way_for_thirty_seconds_is_cursed_for_good() {
        let n = numbers();
        let mut m = Mind::default();
        assert!(!track_path(&mut m, false, &n, 0));
        assert!(!track_path(&mut m, false, &n, 599));
        assert!(!track_path(&mut m, true, &n, 600));
        assert!(!track_path(&mut m, false, &n, 700));
        assert!(track_path(&mut m, false, &n, 1300));
        assert!(track_path(&mut m, true, &n, 1301));
    }

    #[test]
    fn the_book_of_broken_blocks_keeps_what_the_player_built() {
        let mut b = Breaches::default();
        b.record([1, 2, 3], "minecraft:stone_bricks");
        b.record([1, 3, 3], "minecraft:oak_planks[axis=y]");
        b.record([1, 2, 3], "minecraft:air");
        assert_eq!(b.list.len(), 2);
        assert!(b.contains([1, 3, 3]));
        // The brute's own break leaves air: still to restore.
        b.changed([1, 2, 3], true);
        // The player built on the other place.
        b.changed([1, 3, 3], false);
        b.changed([9, 9, 9], false);
        let lines = b.to_lines();
        let again = Breaches::from_lines(lines.iter().map(String::as_str));
        assert_eq!(again, b);
        let back = b.take_restore();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].state, "minecraft:stone_bricks");
        assert!(b.list.is_empty());
        assert!(Breaches::from_lines(["bad"]).list.is_empty());
    }
}

#[cfg(test)]
mod round10_tests {
    use super::*;
    use crate::data::MobsFile;

    #[test]
    fn a_participant_behind_a_wall_does_not_pull_the_mob() {
        let n = Numbers::of(&MobsFile::embedded().events[0]);
        let altar = [20.0, 0.0, 0.0];
        let walled = Seen { entity: 4, pos: [4.0, 0.0, 0.0], visible: false, reachable: false };
        let mut m = Mind::default();
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[walled], &mut m, 0), Goal::Altar);
        // Seen through a gap, or walkable round: pulled.
        let seen = Seen { visible: true, ..walled };
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[seen], &mut Mind::default(), 0), Goal::Player(4));
        let walkable = Seen { reachable: true, ..walled };
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[walkable], &mut Mind::default(), 0), Goal::Player(4));
        // A hit still turns it, wall or not.
        on_hit(&mut m, 4, 1);
        assert_eq!(choose(Role::Assault, &n, [0.0; 3], altar, true, &[walled], &mut m, 2), Goal::Player(4));
    }
}
