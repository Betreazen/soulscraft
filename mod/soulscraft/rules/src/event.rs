//! The wave event on the altar (06-altar.md, «Ивент на алтаре»; pace hypothesis Г7): the plan
//! the banners shape (wave sizes, rests, altar health, the mobs' power), the run's phases
//! (spawn points shown, a wave fighting, a rest, won or lost), the zone's numbers and the reward
//! a win drops (data/trophies.json).

use crate::altar::Effects;
use crate::data::{EventDef, MobDef, MobsFile};
use crate::defense::Role;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The copy of data/trophies.json the crate was built with (mod/sync.ps1 writes it).
pub const TROPHIES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/trophies.json"));

/// The zone and path numbers of an event (data/mobs.json `events[].flow`).
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    /// The zone is this many chunks across, the altar's chunk in the middle (round 2: 3).
    pub zone_chunks: i32,
    pub below_blocks: i32,
    pub above_blocks: i32,
    /// The path map is built again at most this often.
    pub rebuild_min_ticks: u64,
    /// Spawn points lie this far from the core, on the ground.
    pub spawn_near: f64,
    pub spawn_far: f64,
    /// The spawn points show this long before the wave comes out of them.
    pub telegraph_ticks: u64,
    /// The event fails after this long with no living participant in the zone.
    pub participant_grace_ticks: u64,
    /// A mob this close to the core on the ground drains it.
    pub drain_blocks: f64,
}

impl Flow {
    pub fn of(event: &EventDef) -> Self {
        Self::from_map(&event.flow)
    }

    pub fn from_map(m: &Map<String, Value>) -> Self {
        let n = |k: &str, d: f64| m.get(k).and_then(Value::as_f64).unwrap_or(d);
        let spawn = m.get("spawn_blocks").and_then(Value::as_array);
        let at = |i: usize, d: f64| spawn.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(d);
        Self {
            zone_chunks: n("zone_chunks", 3.0) as i32,
            below_blocks: n("below_blocks", 8.0) as i32,
            above_blocks: n("above_blocks", 8.0) as i32,
            rebuild_min_ticks: n("rebuild_min_ticks", 20.0) as u64,
            spawn_near: at(0, 12.0),
            spawn_far: at(1, 20.0),
            telegraph_ticks: n("telegraph_ticks", 60.0) as u64,
            participant_grace_ticks: n("participant_grace_ticks", 60.0) as u64,
            drain_blocks: n("drain_blocks", 3.0),
        }
    }

    /// The zone's corners on the ground, min and max block (inclusive): `zone_chunks` chunks
    /// across with the core's chunk in the middle.
    pub fn zone(&self, core: [i32; 3]) -> ([i32; 2], [i32; 2]) {
        let half = self.zone_chunks / 2;
        let cx = core[0].div_euclid(16);
        let cz = core[2].div_euclid(16);
        let min = [(cx - half) * 16, (cz - half) * 16];
        let max = [(cx + half) * 16 + 15, (cz + half) * 16 + 15];
        (min, max)
    }

    pub fn in_zone(&self, core: [i32; 3], x: f64, z: f64) -> bool {
        let (min, max) = self.zone(core);
        x >= f64::from(min[0]) && x < f64::from(max[0] + 1) && z >= f64::from(min[1]) && z < f64::from(max[1] + 1)
    }
}

/// One mob a wave brings.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnMob {
    /// Its entity type: `minecraft:zombie`, `soulscraft:brute`.
    pub kind: String,
    pub role: Role,
    pub armored: bool,
    /// The event's banner-bearer: a zombie with a banner on its head.
    pub banner: bool,
    /// Its health over its type's (the banner-bearer's × 3), before P's growth.
    pub health_mult: f64,
    /// The cells it needs above its feet (a brute 4, a zombie 2).
    pub height: i32,
}

impl SpawnMob {
    /// A vanilla mob of a wave: an assault mob two cells tall.
    pub fn vanilla(kind: &str) -> Self {
        Self { kind: kind.to_owned(), role: Role::Assault, armored: false, banner: false, health_mult: 1.0, height: 2 }
    }

    /// One of SoulsCraft's mobs, with its wave role and its box's height.
    pub fn of(def: &MobDef) -> Self {
        Self {
            kind: def.key.clone(),
            role: Role::parse(&def.wave_role),
            armored: false,
            banner: false,
            health_mult: 1.0,
            height: def.hitbox.map_or(2, |h| h.height.ceil() as i32),
        }
    }
}

/// The mobs of a kin a kin banner brings into a wave: SoulsCraft's of that kin that walk on land
/// (no water-only one) and are not elite, the weaker (`tier_p`) first.
pub fn kin_wave_mobs<'a>(mobs: &'a MobsFile, kin: &str) -> Vec<&'a MobDef> {
    let mut out: Vec<&MobDef> = mobs.of_kin(kin).into_iter().filter(|m| !m.stats.water_only && !m.elite).collect();
    out.sort_by(|a, b| a.tier_p.total_cmp(&b.tier_p));
    out
}

/// The kin banners in a wave (data/altar.json `kin_rule`): `kin_share` of its slots (all but the
/// banner-bearer, rounded, at least one) become SoulsCraft mobs of the banners' kins, the last
/// slots first (the armoured ones stay), kins taken in turn by their banners' count and each kin's
/// mobs in turn. A slot that already holds a mob of the kin it would get is left as it is.
pub fn kin_bias(wave: &mut [SpawnMob], mobs: &MobsFile, effects: &Effects) {
    let share = effects.kin_share.clamp(0.0, 1.0);
    let slots: Vec<usize> = (0..wave.len()).rev().filter(|&i| !wave[i].banner).collect();
    if share <= 0.0 || slots.is_empty() {
        return;
    }
    let turn: Vec<(&str, Vec<&MobDef>)> = effects
        .kin
        .iter()
        .flat_map(|(k, n)| std::iter::repeat_n(k.as_str(), *n as usize))
        .map(|k| (k, kin_wave_mobs(mobs, k)))
        .filter(|(_, list)| !list.is_empty())
        .collect();
    if turn.is_empty() {
        return;
    }
    let n = ((slots.len() as f64 * share).round() as usize).clamp(1, slots.len());
    let mut next: BTreeMap<&str, usize> = BTreeMap::new();
    for (j, &slot) in slots.iter().take(n).enumerate() {
        let (kin, list) = &turn[j % turn.len()];
        if mobs.by_key(&wave[slot].kind).is_some_and(|(_, d)| d.has_kin(kin)) {
            continue;
        }
        let c = next.entry(kin).or_default();
        let def = list[*c % list.len()];
        *c += 1;
        wave[slot] = SpawnMob { armored: false, ..SpawnMob::of(def) };
    }
}

/// What a run of the event is, its banners counted.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub event: String,
    pub waves: Vec<Vec<SpawnMob>>,
    pub rest_ticks: u64,
    pub altar_health: f64,
    /// The power P the mobs are scaled by (participants' mean plus the weight banners).
    pub power: f64,
    pub effects: Effects,
}

/// The shortest rest the haste banners leave.
pub const MIN_REST_SECONDS: f64 = 5.0;

/// The plan of `event` with these banners, for participants of mean power `power`.
pub fn plan(event: &EventDef, mobs: &MobsFile, effects: &Effects, power: f64) -> Plan {
    let size = (1.0 + effects.wave_size).max(0.0);
    let waves = event
        .waves
        .iter()
        .map(|w| {
            let mut out = Vec::new();
            for g in &w.mobs {
                let count = ((f64::from(g.count) * size).round() as u32).max(1);
                let armored = (f64::from(g.armored) * size).round() as u32;
                let banner = g.kind == "banner_zombie";
                let base = if banner {
                    SpawnMob::vanilla("minecraft:zombie")
                } else if g.kind.starts_with("minecraft:") {
                    // A changed vanilla type takes its role from data/mobs.json `vanilla`.
                    let role = mobs.vanilla.role_of(&g.kind).map(Role::parse);
                    SpawnMob { role: role.unwrap_or(Role::Assault), ..SpawnMob::vanilla(&g.kind) }
                } else {
                    match mobs.mob(&g.kind) {
                        Some(d) => SpawnMob::of(d),
                        None => SpawnMob::vanilla(&g.kind),
                    }
                };
                for i in 0..count {
                    out.push(SpawnMob { armored: i < armored, banner, health_mult: g.health_mult.unwrap_or(1.0), ..base.clone() });
                }
            }
            kin_bias(&mut out, mobs, effects);
            out
        })
        .collect();
    let rest = if event.rest_seconds.is_empty() {
        30.0
    } else {
        event.rest_seconds.iter().sum::<f64>() / event.rest_seconds.len() as f64
    };
    let rest = (rest + effects.rest_seconds).max(MIN_REST_SECONDS);
    Plan {
        event: event.id.clone(),
        waves,
        rest_ticks: (rest * 20.0).round() as u64,
        altar_health: (if event.altar_health > 0.0 { event.altar_health } else { 200.0 }) * (1.0 + effects.altar_health).max(0.1),
        power: crate::scale::clamp_p(power + effects.power),
        effects: effects.clone(),
    }
}

/// Why a run ended without a win.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loss {
    /// The altar's health ran out.
    Altar,
    /// No living participant in the zone.
    Nobody,
    /// Stopped by an operator.
    Stopped,
}

impl Loss {
    pub fn name(self) -> &'static str {
        match self {
            Loss::Altar => "altar",
            Loss::Nobody => "nobody",
            Loss::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Wave `wave` (from 0) shows its spawn points until `until`.
    Telegraph { wave: usize, until: u64 },
    /// Wave `wave` is out.
    Fight { wave: usize },
    /// The rest before wave `next`.
    Rest { next: usize, until: u64 },
    Won,
    Lost(Loss),
}

/// What the server does on a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Show where wave `wave` comes from.
    Telegraph(usize),
    /// Bring wave `wave` out.
    Spawn(usize),
    Rest(usize),
    Won,
    Lost(Loss),
}

/// The phase machine of one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub phase: Phase,
    pub waves: usize,
    pub rest_ticks: u64,
    pub telegraph_ticks: u64,
    pub grace_ticks: u64,
    empty_since: Option<u64>,
}

impl Run {
    /// A run that starts telegraphing wave 1 at `now`.
    pub fn start(plan: &Plan, flow: &Flow, now: u64) -> (Self, Step) {
        let run = Self {
            phase: Phase::Telegraph { wave: 0, until: now + flow.telegraph_ticks },
            waves: plan.waves.len(),
            rest_ticks: plan.rest_ticks,
            telegraph_ticks: flow.telegraph_ticks,
            grace_ticks: flow.participant_grace_ticks,
            empty_since: None,
        };
        (run, Step::Telegraph(0))
    }

    pub fn over(&self) -> bool {
        matches!(self.phase, Phase::Won | Phase::Lost(_))
    }

    /// The wave in progress or coming, from 0.
    pub fn wave(&self) -> usize {
        match self.phase {
            Phase::Telegraph { wave, .. } | Phase::Fight { wave } => wave,
            Phase::Rest { next, .. } => next,
            Phase::Won | Phase::Lost(_) => self.waves,
        }
    }

    /// One tick: `alive` wave mobs left, `participants` living in the zone, the altar fallen.
    pub fn step(&mut self, now: u64, alive: usize, participants: usize, altar_fallen: bool) -> Option<Step> {
        if self.over() {
            return None;
        }
        if altar_fallen {
            return Some(self.end(Phase::Lost(Loss::Altar), Step::Lost(Loss::Altar)));
        }
        if participants == 0 {
            let since = *self.empty_since.get_or_insert(now);
            if now - since >= self.grace_ticks {
                return Some(self.end(Phase::Lost(Loss::Nobody), Step::Lost(Loss::Nobody)));
            }
        } else {
            self.empty_since = None;
        }
        match self.phase {
            Phase::Telegraph { wave, until } if now >= until => {
                self.phase = Phase::Fight { wave };
                Some(Step::Spawn(wave))
            }
            Phase::Fight { wave } if alive == 0 => {
                if wave + 1 >= self.waves {
                    Some(self.end(Phase::Won, Step::Won))
                } else {
                    self.phase = Phase::Rest { next: wave + 1, until: now + self.rest_ticks };
                    Some(Step::Rest(wave + 1))
                }
            }
            Phase::Rest { next, until } if now >= until => {
                self.phase = Phase::Telegraph { wave: next, until: now + self.telegraph_ticks };
                Some(Step::Telegraph(next))
            }
            _ => None,
        }
    }

    pub fn stop(&mut self) -> Option<Step> {
        if self.over() {
            return None;
        }
        Some(self.end(Phase::Lost(Loss::Stopped), Step::Lost(Loss::Stopped)))
    }

    fn end(&mut self, phase: Phase, step: Step) -> Step {
        self.phase = phase;
        step
    }
}

/// A small deterministic random stream (SplitMix64): rolls that tests and logs can replay.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// From 0 up to 1.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }
}

/// data/trophies.json's event chances.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyChances {
    /// The chance of each armour piece of the event's mobs.
    pub armor: f64,
    pub diorama_min_level: u32,
    /// How much one «harder» banner raises (an «easier» one lowers) the armour chance: × (1 +
    /// step · (harder − easier)), never under 0 (`drops.banner_step`).
    pub banner_step: f64,
    /// A credited kill's head in the world, an ordinary mob's and an elite's (`drops.head.world`,
    /// `world_elite`; 11-summons-rewards.md, Г18).
    pub head_world: f64,
    pub head_world_elite: f64,
    /// Each event tier's own reward (`tiers`, Г19).
    pub tiers: Vec<TierReward>,
}

/// What a win of an event tier gives on top of its trophies: shards for sure, the tier's item by
/// a chance the banners multiply.
#[derive(Clone, Debug, PartialEq)]
pub struct TierReward {
    pub tier: u32,
    pub shards: u32,
    pub item: Option<String>,
    pub chance: f64,
}

/// The item shards drop as (the altar's currency, 10-survival.md).
pub const SOUL_SHARD: &str = "soulscraft:soul_shard";

impl TrophyChances {
    pub fn embedded() -> Self {
        Self::parse(TROPHIES)
    }
    pub fn parse(text: &str) -> Self {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let drops = &v["drops"];
        let tiers = v["tiers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| {
                Some(TierReward {
                    tier: t["tier"].as_u64()? as u32,
                    shards: t["shards"].as_u64().unwrap_or(0) as u32,
                    item: t["item"].as_str().map(str::to_owned),
                    chance: t["chance"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0),
                })
            })
            .collect();
        Self {
            armor: drops["armor"]["event"].as_f64().unwrap_or(0.3),
            diorama_min_level: drops["diorama"]["min_altar_level"].as_u64().unwrap_or(3) as u32,
            banner_step: drops["banner_step"].as_f64().unwrap_or(0.25),
            head_world: drops["head"]["world"].as_f64().unwrap_or(0.0),
            head_world_elite: drops["head"]["world_elite"].as_f64().unwrap_or(0.0),
            tiers,
        }
    }

    /// The reward of the event tier `tier`, if the file has one.
    pub fn tier(&self, tier: u32) -> Option<&TierReward> {
        self.tiers.iter().find(|t| t.tier == tier)
    }

    /// The reward multiplier of the banners: 1 + step · (harder − easier), not under 0.
    pub fn multiplier(&self, effects: &Effects) -> f64 {
        (1.0 + self.banner_step * (f64::from(effects.more) - f64::from(effects.less))).max(0.0)
    }

    /// Heads a win gives: one for sure, one more a «harder» banner, one fewer an «easier» one,
    /// never under one (09-trophies.md, «Откуда берутся»).
    pub fn heads(&self, effects: &Effects) -> usize {
        (1 + i64::from(effects.more) - i64::from(effects.less)).max(1) as usize
    }
}

/// One thing a win drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trophy {
    /// The head of a mob of the event (its entity type).
    Head(String),
    /// A piece of armour (its item).
    Armor(String),
    /// A diorama of a mob of the event (its entity type).
    Diorama(String),
    /// A tier's own reward: an item and how many (shards, the tier's item).
    Item(String, u32),
}

impl Trophy {
    /// How many of [`Trophy::item`] drop in one stack.
    pub fn count(&self) -> u32 {
        match self {
            Trophy::Item(_, n) => *n,
            _ => 1,
        }
    }

    /// The item that drops: SoulsCraft's `<mob>_head` and `<mob>_trophy`, vanilla's heads for
    /// vanilla mobs (the test event's zombie head); none for a mob without a head or diorama.
    pub fn item(&self) -> Option<String> {
        match self {
            Trophy::Item(item, n) => (*n > 0).then(|| item.clone()),
            Trophy::Head(kind) => match kind.strip_prefix("soulscraft:") {
                Some(id) => Some(format!("soulscraft:{id}_head")),
                None => vanilla_head(kind).map(str::to_owned),
            },
            Trophy::Armor(item) => Some(item.clone()),
            Trophy::Diorama(kind) => kind.strip_prefix("soulscraft:").map(|id| format!("soulscraft:{id}_trophy")),
        }
    }
}

/// Vanilla's head item of a vanilla mob, if it has one.
pub fn vanilla_head(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "minecraft:zombie" | "minecraft:husk" | "minecraft:drowned" | "minecraft:zombie_villager" => "minecraft:zombie_head",
        "minecraft:skeleton" | "minecraft:stray" | "minecraft:bogged" => "minecraft:skeleton_skull",
        "minecraft:wither_skeleton" => "minecraft:wither_skeleton_skull",
        "minecraft:creeper" => "minecraft:creeper_head",
        "minecraft:piglin" => "minecraft:piglin_head",
        _ => return None,
    })
}

/// The reward of a win (09-trophies.md; data/trophies.json): [`TrophyChances::heads`] heads of
/// mobs of the event; each armour piece the event's SoulsCraft mobs drop (`loot`) by the event
/// chance times the banners' [`TrophyChances::multiplier`]; on an altar of level 3 and up a
/// diorama of one of the event's SoulsCraft mobs (none when the event had only vanilla mobs);
/// then the event tier's own reward (11-summons-rewards.md, Г19): its shards, and its item by the
/// tier's chance times the banners' multiplier. The tier's roll comes after the trophies', so a
/// tier reward never changes which trophies a seed gives.
pub fn rewards(kinds: &[String], mobs: &MobsFile, effects: &Effects, altar_level: u32, tier: u32, chances: &TrophyChances, rng: &mut Rng) -> Vec<Trophy> {
    let mut out = trophies(kinds, mobs, effects, altar_level, chances, rng);
    if let Some(t) = chances.tier(tier) {
        if t.shards > 0 {
            out.push(Trophy::Item(SOUL_SHARD.to_owned(), t.shards));
        }
        if let Some(item) = &t.item {
            if rng.unit() < (t.chance * chances.multiplier(effects)).min(1.0) {
                out.push(Trophy::Item(item.clone(), 1));
            }
        }
    }
    out
}

/// The trophies of a win: heads, armour pieces, a diorama ([`rewards`] without the tier's part).
fn trophies(kinds: &[String], mobs: &MobsFile, effects: &Effects, altar_level: u32, chances: &TrophyChances, rng: &mut Rng) -> Vec<Trophy> {
    let mut out = Vec::new();
    if kinds.is_empty() {
        return out;
    }
    for _ in 0..chances.heads(effects) {
        out.push(Trophy::Head(kinds[rng.below(kinds.len())].clone()));
    }
    let ours: Vec<&MobDef> = kinds.iter().filter_map(|k| mobs.by_key(k).map(|(_, d)| d)).collect();
    let chance = (chances.armor * chances.multiplier(effects)).min(1.0);
    let mut pieces: Vec<&str> = ours.iter().flat_map(|d| d.loot.iter().map(|l| l.item.as_str())).collect();
    pieces.dedup();
    for p in pieces {
        if rng.unit() < chance {
            out.push(Trophy::Armor(p.to_owned()));
        }
    }
    if altar_level >= chances.diorama_min_level && !ours.is_empty() {
        out.push(Trophy::Diorama(ours[rng.below(ours.len())].key.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::altar::{AltarFile, effects};

    fn event() -> EventDef {
        MobsFile::embedded().events[0].clone()
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_test_event_reads() {
        let e = event();
        assert_eq!(e.tier, 1);
        assert_eq!(e.item, "soulscraft:rotten_horn");
        assert_eq!(e.altar_health, 200.0);
        let f = Flow::of(&e);
        assert_eq!(f.zone_chunks, 3);
        assert_eq!((f.spawn_near, f.spawn_far), (12.0, 20.0));
        assert_eq!(f.telegraph_ticks, 60);
        assert_eq!(Flow::from_map(&Map::new()), f);
    }

    #[test]
    fn the_zone_is_three_chunks_round_the_core_chunk() {
        let f = Flow::of(&event());
        let (min, max) = f.zone([-600, 100, 600]);
        // -600 is in chunk -38 (-608..-593); 600 in chunk 37 (592..607).
        assert_eq!(min, [-624, 576]);
        assert_eq!(max, [-577, 623]);
        assert!(f.in_zone([-600, 100, 600], -624.0, 576.0));
        assert!(f.in_zone([-600, 100, 600], -576.5, 623.9));
        assert!(!f.in_zone([-600, 100, 600], -576.0, 600.0));
    }

    #[test]
    fn the_plan_follows_the_data_and_the_banners() {
        let mobs = MobsFile::embedded();
        let altar = AltarFile::embedded();
        let p = plan(&event(), &mobs, &effects(&altar, &[]), 0.4);
        let sizes: Vec<usize> = p.waves.iter().map(Vec::len).collect();
        assert_eq!(sizes, [6, 8, 10, 12, 15]);
        assert_eq!(p.waves[2].iter().filter(|m| m.armored).count(), 2);
        let bearer = p.waves[4].iter().find(|m| m.banner).unwrap();
        assert_eq!((bearer.kind.as_str(), bearer.health_mult), ("minecraft:zombie", 3.0));
        assert_eq!(p.rest_ticks, 750);
        assert_eq!(p.altar_health, 200.0);
        assert!((p.power - 0.4).abs() < 1e-9);
        assert!(p.waves.iter().flatten().all(|m| m.role == Role::Assault));
        // Horde, haste, resilience, weight.
        let e = effects(&altar, &ids(&["horde", "haste", "resilience", "weight"]));
        let p = plan(&event(), &mobs, &e, 0.95);
        let sizes: Vec<usize> = p.waves.iter().map(Vec::len).collect();
        assert_eq!(sizes, [8, 10, 13, 15, 19]);
        assert_eq!(p.rest_ticks, 550);
        assert_eq!(p.altar_health, 300.0);
        assert_eq!(p.power, 1.0);
        // Calm halves nothing below one mob; rest never under 5 s.
        let e = effects(&altar, &ids(&["calm", "calm", "calm", "calm", "haste", "haste", "haste", "haste"]));
        let p = plan(&event(), &mobs, &e, 0.0);
        assert!(p.waves.iter().all(|w| w.len() == 1 || w.len() == 2));
        assert_eq!(p.rest_ticks, 100);
    }

    #[test]
    fn soulscraft_mobs_in_a_wave_keep_their_role_and_height() {
        let mobs = MobsFile::embedded();
        let mut e = event();
        e.waves[0].mobs[0].kind = "brute".into();
        e.waves[1].mobs[0].kind = "stalker".into();
        e.waves[2].mobs[0].kind = "marksman".into();
        let p = plan(&e, &mobs, &Effects::default(), 0.0);
        assert_eq!((p.waves[0][0].kind.as_str(), p.waves[0][0].role, p.waves[0][0].height), ("soulscraft:brute", Role::Assault, 4));
        assert_eq!(p.waves[1][0].role, Role::Hunter);
        assert_eq!(p.waves[2][0].role, Role::Ranged);
        e.rest_seconds.clear();
        e.altar_health = 0.0;
        let p = plan(&e, &mobs, &Effects::default(), 0.0);
        assert_eq!((p.rest_ticks, p.altar_health), (600, 200.0));
    }

    fn small_run() -> (Run, Plan, Flow) {
        let mobs = MobsFile::embedded();
        let mut e = event();
        e.waves.truncate(2);
        let p = plan(&e, &mobs, &Effects::default(), 0.0);
        let f = Flow::of(&e);
        let (r, step) = Run::start(&p, &f, 100);
        assert_eq!(step, Step::Telegraph(0));
        (r, p, f)
    }

    #[test]
    fn a_run_goes_through_its_waves_to_a_win() {
        let (mut r, p, _) = small_run();
        assert_eq!(r.step(150, 0, 1, false), None);
        assert_eq!(r.step(160, 0, 1, false), Some(Step::Spawn(0)));
        assert_eq!(r.step(161, 6, 1, false), None);
        assert_eq!(r.wave(), 0);
        assert_eq!(r.step(300, 0, 1, false), Some(Step::Rest(1)));
        assert_eq!(r.wave(), 1);
        assert_eq!(r.step(300 + p.rest_ticks - 1, 0, 1, false), None);
        assert_eq!(r.step(300 + p.rest_ticks, 0, 1, false), Some(Step::Telegraph(1)));
        assert_eq!(r.step(300 + p.rest_ticks + 60, 0, 1, false), Some(Step::Spawn(1)));
        assert_eq!(r.step(2000, 0, 1, false), Some(Step::Won));
        assert!(r.over());
        assert_eq!(r.step(2001, 0, 1, false), None);
        assert_eq!(r.stop(), None);
        assert_eq!(r.wave(), 2);
    }

    #[test]
    fn a_run_is_lost_by_the_altar_or_an_empty_zone() {
        let (mut r, _, _) = small_run();
        assert_eq!(r.step(161, 0, 1, true), Some(Step::Lost(Loss::Altar)));
        assert_eq!(r.phase, Phase::Lost(Loss::Altar));
        let (mut r, _, f) = small_run();
        assert_eq!(r.step(110, 0, 0, false), None);
        // Someone came back in time: the clock starts over.
        assert_eq!(r.step(150, 0, 1, false), None);
        assert_eq!(r.step(151, 0, 0, false), None);
        assert_eq!(r.step(151 + f.participant_grace_ticks, 0, 0, false), Some(Step::Lost(Loss::Nobody)));
        let (mut r, _, _) = small_run();
        assert_eq!(r.stop(), Some(Step::Lost(Loss::Stopped)));
        assert_eq!([Loss::Altar.name(), Loss::Nobody.name(), Loss::Stopped.name()], ["altar", "nobody", "stopped"]);
    }

    #[test]
    fn rewards_count_heads_by_banners_and_dioramas_by_level() {
        let altar = AltarFile::embedded();
        let mobs = MobsFile::embedded();
        let c = TrophyChances::embedded();
        assert_eq!((c.armor, c.diorama_min_level, c.banner_step), (0.3, 3, 0.25));
        let kinds = ids(&["minecraft:zombie"]);
        let heads = |t: &[Trophy]| t.iter().filter(|x| matches!(x, Trophy::Head(_))).count();
        let mut rng = Rng::new(1);
        let plain = rewards(&kinds, &mobs, &Effects::default(), 1, 0, &c, &mut rng);
        assert_eq!(plain, vec![Trophy::Head("minecraft:zombie".into())]);
        assert_eq!(plain[0].item().as_deref(), Some("minecraft:zombie_head"));
        let hard = effects(&altar, &ids(&["weight", "horde", "haste"]));
        assert_eq!(heads(&rewards(&kinds, &mobs, &hard, 1, 0, &c, &mut rng)), 4);
        let easy = effects(&altar, &ids(&["mercy", "calm", "rest", "resilience"]));
        assert_eq!(heads(&rewards(&kinds, &mobs, &easy, 1, 0, &c, &mut rng)), 1);
        // Only vanilla mobs: no diorama even on level 4, no armour.
        let lvl4 = rewards(&kinds, &mobs, &hard, 4, 0, &c, &mut rng);
        assert!(lvl4.iter().all(|t| matches!(t, Trophy::Head(_))), "{lvl4:?}");
        // A SoulsCraft mob in the event: its diorama on level 3, not on level 2.
        let mixed = ids(&["minecraft:zombie", "soulscraft:revenant"]);
        let lvl3 = rewards(&mixed, &mobs, &Effects::default(), 3, 0, &c, &mut rng);
        assert!(lvl3.contains(&Trophy::Diorama("soulscraft:revenant".into())), "{lvl3:?}");
        assert!(!rewards(&mixed, &mobs, &Effects::default(), 2, 0, &c, &mut rng).iter().any(|t| matches!(t, Trophy::Diorama(_))));
        assert!(rewards(&[], &mobs, &hard, 4, 0, &c, &mut rng).is_empty());
        assert_eq!(rng.below(0), 0);
    }

    /// The share of runs, seeded, in which `item` drops.
    fn armour_rate(kinds: &[String], e: &Effects, item: &str) -> f64 {
        let (mobs, c) = (MobsFile::embedded(), TrophyChances::embedded());
        let mut rng = Rng::new(42);
        let n = 4000;
        let hit = (0..n).filter(|_| rewards(kinds, &mobs, e, 1, 0, &c, &mut rng).contains(&Trophy::Armor(item.into()))).count();
        hit as f64 / f64::from(n)
    }

    #[test]
    fn each_armour_piece_comes_by_the_chance_times_the_banners() {
        let altar = AltarFile::embedded();
        let revenant = ids(&["soulscraft:revenant"]);
        // Both rusty pieces, each about 30% (seeded: the same numbers every run).
        for item in ["soulscraft:rusty_helmet", "soulscraft:rusty_chestplate"] {
            let r = armour_rate(&revenant, &Effects::default(), item);
            assert!((0.27..0.33).contains(&r), "{item} {r}");
        }
        // Two «harder» banners: × 1.5 = 45%; two «easier»: × 0.5 = 15%; eight «easier»: never.
        let hard = effects(&altar, &ids(&["weight", "horde"]));
        let easy = effects(&altar, &ids(&["mercy", "calm"]));
        let r = armour_rate(&revenant, &hard, "soulscraft:rusty_helmet");
        assert!((0.42..0.48).contains(&r), "{r}");
        let r = armour_rate(&revenant, &easy, "soulscraft:rusty_helmet");
        assert!((0.12..0.18).contains(&r), "{r}");
        let none = effects(&altar, &ids(&["mercy"; 8]));
        assert_eq!(armour_rate(&revenant, &none, "soulscraft:rusty_helmet"), 0.0);
        // A shrike has no armour.
        assert_eq!(armour_rate(&ids(&["soulscraft:shrike"]), &Effects::default(), "soulscraft:rusty_helmet"), 0.0);
        let c = TrophyChances::embedded();
        assert_eq!((c.multiplier(&hard), c.multiplier(&easy), c.multiplier(&none)), (1.5, 0.5, 0.0));
        assert_eq!(TrophyChances::parse("{}"), TrophyChances { armor: 0.3, diorama_min_level: 3, banner_step: 0.25, head_world: 0.0, head_world_elite: 0.0, tiers: Vec::new() });
    }

    #[test]
    fn trophies_name_their_items() {
        assert_eq!(Trophy::Head("soulscraft:brute".into()).item().as_deref(), Some("soulscraft:brute_head"));
        assert_eq!(Trophy::Diorama("soulscraft:bog_slime".into()).item().as_deref(), Some("soulscraft:bog_slime_trophy"));
        assert_eq!(Trophy::Diorama("minecraft:zombie".into()).item(), None);
        assert_eq!(Trophy::Armor("soulscraft:stalker_hood".into()).item().as_deref(), Some("soulscraft:stalker_hood"));
        assert_eq!(Trophy::Head("minecraft:stray".into()).item().as_deref(), Some("minecraft:skeleton_skull"));
        assert_eq!(Trophy::Head("minecraft:spider".into()).item(), None);
        for (k, h) in [("minecraft:wither_skeleton", "minecraft:wither_skeleton_skull"), ("minecraft:creeper", "minecraft:creeper_head"), ("minecraft:piglin", "minecraft:piglin_head")] {
            assert_eq!(vanilla_head(k), Some(h));
        }
    }

    #[test]
    fn kin_banners_turn_a_share_of_each_wave() {
        let mobs = MobsFile::embedded();
        let altar = AltarFile::embedded();
        fn kinds(w: &[SpawnMob]) -> Vec<&str> {
            w.iter().map(|m| m.kind.as_str()).collect()
        }
        // One undead banner: a quarter of wave 1's six zombies (two, rounded), the last ones,
        // the weaker undead first (revenant 0.2, shrike 0.25; marksman next; the elite brute never).
        let p = plan(&event(), &mobs, &effects(&altar, &ids(&["kin_undead"])), 0.0);
        assert_eq!(kinds(&p.waves[0]), ["minecraft:zombie"; 4].iter().copied().chain(["soulscraft:shrike", "soulscraft:revenant"]).collect::<Vec<_>>());
        assert_eq!(p.waves[0][5].role, Role::Assault);
        assert_eq!(p.waves[0][4].role, Role::Hunter);
        // The armoured zombies of wave 3 (the first two) and the banner-bearer of wave 5 stay.
        assert_eq!(p.waves[2].iter().filter(|m| m.armored).count(), 2);
        assert!(p.waves[4].iter().any(|m| m.banner && m.kind == "minecraft:zombie"));
        let undead = kin_wave_mobs(&mobs, "undead").iter().map(|m| m.id.as_str()).collect::<Vec<_>>();
        assert_eq!(undead, ["revenant", "shrike", "marksman"]);
        // Water: the bog slime walks; the lurker, water-only, never comes into a land wave.
        assert_eq!(kin_wave_mobs(&mobs, "water").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["bog_slime"]);
        // Two kins, three banners: 75%, kins in turn (flyers, undead, undead).
        let p = plan(&event(), &mobs, &effects(&altar, &ids(&["kin_undead", "kin_flyers", "kin_undead"])), 0.0);
        let w = kinds(&p.waves[1]);
        assert_eq!(w.iter().filter(|k| k.starts_with("soulscraft:")).count(), 6, "{w:?}");
        assert_eq!(w[7], "soulscraft:shrike");
        assert_eq!(w[6], "soulscraft:revenant");
        assert_eq!(w[5], "soulscraft:shrike");
        // A wave already of the kin keeps its mobs.
        let mut wave = vec![SpawnMob::of(mobs.mob("revenant").unwrap()); 4];
        kin_bias(&mut wave, &mobs, &effects(&altar, &ids(&["kin_undead", "kin_undead", "kin_undead", "kin_undead"])));
        assert!(wave.iter().all(|m| m.kind == "soulscraft:revenant"));
        // No kin banner, or a wave of only the bearer: nothing changes.
        let mut bearer = vec![SpawnMob { banner: true, ..SpawnMob::vanilla("minecraft:zombie") }];
        kin_bias(&mut bearer, &mobs, &effects(&altar, &ids(&["kin_slimes"])));
        assert_eq!(bearer[0].kind, "minecraft:zombie");
        let p = plan(&event(), &mobs, &Effects::default(), 0.0);
        assert!(p.waves.iter().flatten().all(|m| m.kind == "minecraft:zombie"));
    }

    #[test]
    fn each_tier_has_its_reward_and_events_use_those_tiers() {
        let c = TrophyChances::embedded();
        let mobs = MobsFile::embedded();
        assert_eq!(c.tier(1).map(|t| (t.shards, t.item.clone())), Some((6, None)));
        assert_eq!(c.tier(2).and_then(|t| t.item.clone()).as_deref(), Some("soulscraft:revenant_blade"));
        assert_eq!(c.tier(3).and_then(|t| t.item.clone()).as_deref(), Some("soulscraft:brute_maul"));
        assert_eq!(c.tier(4).and_then(|t| t.item.clone()).as_deref(), Some("soulscraft:abyss_heart"));
        // Every event's tier has a reward, and higher tiers give more shards.
        for e in &mobs.events {
            assert!(c.tier(e.tier).is_some(), "{}", e.id);
        }
        let shards: Vec<u32> = (1..=4).map(|t| c.tier(t).unwrap().shards).collect();
        assert!(shards.windows(2).all(|w| w[0] < w[1]), "{shards:?}");
        assert_eq!((c.head_world, c.head_world_elite), (0.01, 0.03));
    }

    #[test]
    fn a_win_adds_its_tier_shards_and_item_after_the_same_trophies() {
        let c = TrophyChances::embedded();
        let mobs = MobsFile::embedded();
        let kinds = ids(&["soulscraft:revenant", "soulscraft:stalker"]);
        let e = Effects::default();
        // The tier's part comes after the trophies: the same seed gives the same trophies.
        let base = rewards(&kinds, &mobs, &e, 3, 0, &c, &mut Rng::new(7));
        let tier2 = rewards(&kinds, &mobs, &e, 3, 2, &c, &mut Rng::new(7));
        assert_eq!(&tier2[..base.len()], &base[..]);
        assert_eq!(tier2[base.len()], Trophy::Item(SOUL_SHARD.into(), 8));
        assert_eq!(tier2[base.len()].item().as_deref(), Some(SOUL_SHARD));
        assert_eq!(tier2[base.len()].count(), 8);
        // The item's chance: about 50% for tier 2, more with «harder» banners, capped at 1.
        let rate = |tier: u32, e: &Effects| {
            let mut rng = Rng::new(11);
            let n = 4000;
            (0..n).filter(|_| rewards(&kinds, &mobs, e, 3, tier, &c, &mut rng).iter().any(|t| matches!(t, Trophy::Item(i, 1) if i != SOUL_SHARD))).count() as f64 / n as f64
        };
        assert!((rate(2, &e) - 0.5).abs() < 0.04, "{}", rate(2, &e));
        assert!((rate(4, &e) - 0.35).abs() < 0.04, "{}", rate(4, &e));
        let hard = Effects { more: 2, ..Effects::default() };
        assert!((rate(2, &hard) - 0.75).abs() < 0.04, "{}", rate(2, &hard));
        let easy = Effects { less: 4, ..Effects::default() };
        assert_eq!(rate(3, &easy), 0.0);
        // Tier 1 has no item; an unknown tier gives nothing extra.
        assert_eq!(rate(1, &e), 0.0);
        assert_eq!(rewards(&kinds, &mobs, &e, 3, 9, &c, &mut Rng::new(7)), base);
    }
}