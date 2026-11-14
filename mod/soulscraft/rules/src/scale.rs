//! Player power P and how a mob grows with it: hypotheses Г1, Г2 and Г5 of
//! docs/design/03-hypotheses.md. The numbers here are those hypotheses; playtests change them.

use crate::data::{PowerDef, Stats};
use crate::pct::Summary;

/// Vanilla's zombie movement speed, for mobs whose data names none (flyers, swimmers).
pub const DEFAULT_SPEED: f64 = 0.23;
/// Г5: players this close to the one nearest the spawn count as a group.
pub const GROUP_RADIUS: f64 = 32.0;

pub fn clamp_p(p: f64) -> f64 {
    if p.is_finite() { p.clamp(0.0, 1.0) } else { 0.0 }
}

/// A mob's numbers at power P (Г2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scaled {
    pub max_health: f64,
    /// One normal blow; an attack multiplies it by its own `damage`.
    pub damage: f64,
    pub armor: f64,
    pub speed: f64,
    pub knockback_resistance: f64,
    /// How often the smart AI decides, in ticks: `20 − 10 · P`.
    pub decide_every: u32,
}

impl Scaled {
    pub fn at(stats: &Stats, p: f64) -> Self {
        let p = clamp_p(p);
        Self {
            max_health: stats.health * (1.0 + 0.9 * p),
            damage: stats.damage * (1.0 + 0.6 * p),
            armor: stats.armor + 6.0 * p,
            speed: stats.speed.unwrap_or(DEFAULT_SPEED) * (1.0 + 0.10 * p),
            knockback_resistance: stats.knockback_resistance,
            decide_every: (20.0 - 10.0 * p).round() as u32,
        }
    }

    /// The damage of an attack with multiplier `mult`.
    pub fn blow(&self, mult: f64) -> f64 {
        self.damage * mult
    }
}

/// The tier of the weapon in hand (Г1): hand 0; wood and gold 1; stone 2; iron 3; diamond 4;
/// netherite 5. Only melee and ranged weapons count; a pickaxe in hand is a hand.
pub fn weapon_tier(item: Option<&str>) -> u32 {
    let Some(item) = item else { return 0 };
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    let weapon = ["_sword", "_axe", "_spear"].iter().any(|s| name.ends_with(s));
    if !weapon {
        return match name {
            "mace" | "trident" => 4,
            "bow" | "crossbow" => 2,
            _ => 0,
        };
    }
    match name.split('_').next() {
        Some("wooden" | "golden") => 1,
        Some("stone" | "copper") => 2,
        Some("iron") => 3,
        Some("diamond") => 4,
        Some("netherite") => 5,
        _ => 0,
    }
}

/// What the player's power is made of.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerInputs {
    /// Path-of-craft-tree's combat summary of the player (`pct:summary`), when PCT publishes it.
    pub tree: Option<Summary>,
    /// The player's armour points (`minecraft:armor`).
    pub armor: f64,
    pub weapon_tier: u32,
}

/// Г1's tree part `T`, 0 to 1: each combat branch is `level_weight · level share +
/// rank_weight · rank share` (weights normalised to sum 1; without ranks, the level share
/// alone), and `T` is the mean of the best `tree_top_branches`. A rank share counts the working
/// ranks against what the branch's own levels can buy at most (a point a level, PCT's
/// `branch_points`), so a player who spent a full branch's levels there has 1.
pub fn tree_part(s: &Summary, w: &PowerDef) -> f64 {
    let mut branch: Vec<f64> = (0..3)
        .map(|i| {
            let level = s.levels[i].share();
            let Some(ranks) = s.ranks else { return level };
            let r = ranks[i];
            let can_buy = r.max.min(s.levels[i].max) * w.rank_full;
            let rank = if can_buy > 0.0 { (r.value / can_buy).clamp(0.0, 1.0) } else { 0.0 };
            let total = w.level_weight + w.rank_weight;
            if total > 0.0 { (w.level_weight * level + w.rank_weight * rank) / total } else { level }
        })
        .collect();
    branch.sort_by(|a, b| b.total_cmp(a));
    let n = w.tree_top_branches.clamp(1, 3);
    (branch[..n].iter().sum::<f64>() / n as f64).clamp(0.0, 1.0)
}

/// Г1's gear part `G`, 0 to 1: `armor_weight · A + weapon_weight · W`.
pub fn gear_part(i: &PowerInputs, w: &PowerDef) -> f64 {
    let a = if w.armor_full > 0.0 { (i.armor / w.armor_full).clamp(0.0, 1.0) } else { 0.0 };
    let wp = if w.weapon_full > 0.0 { (f64::from(i.weapon_tier) / w.weapon_full).clamp(0.0, 1.0) } else { 0.0 };
    (w.armor_weight * a + w.weapon_weight * wp).clamp(0.0, 1.0)
}

/// Г1: `P = tree_share · T + (1 − tree_share) · G`; without PCT's summary `T = 0` (gear only).
pub fn player_power(i: &PowerInputs, w: &PowerDef) -> f64 {
    let t = i.tree.as_ref().map_or(0.0, |s| tree_part(s, w));
    let share = w.tree_share.clamp(0.0, 1.0);
    clamp_p(share * t + (1.0 - share) * gear_part(i, w))
}
/// Г5: the power a mob spawned at `at` takes from the players near: those within
/// [`GROUP_RADIUS`] of the player nearest to it, averaged. None when nobody is there.
pub fn group_power(at: [f64; 3], players: &[([f64; 3], f64)]) -> Option<f64> {
    let d2 = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>();
    let nearest = players.iter().min_by(|a, b| d2(a.0, at).total_cmp(&d2(b.0, at)))?;
    let group: Vec<f64> = players
        .iter()
        .filter(|p| d2(p.0, nearest.0) <= GROUP_RADIUS * GROUP_RADIUS)
        .map(|p| p.1)
        .collect();
    Some(clamp_p(group.iter().sum::<f64>() / group.len() as f64))
}

/// The entity tag Path-of-craft-tree reads the kill experience multiplier from (Г3, variant A;
/// PCT v0.4.6): `pct.xp145` is × 1.45. One on each mob; PCT takes the largest if there are more.
pub const XP_TAG_PREFIX: &str = "pct.xp";

/// The experience percentage for a mob whose health is `hp_mult` times its kind's (Г3):
/// `round(100 · (1 + 0.5 · (hp_mult − 1)))`, from 1 to 500. P = 0 gives 100 and P = 1 gives 145.
pub fn xp_percent(hp_mult: f64) -> u32 {
    xp_percent_sized(hp_mult, 1.0)
}

/// [`xp_percent`] times `share`, the part of its kind's experience the mob is worth (a smaller
/// slime's [`size_share`]), rounded after the product and clamped to 1..500. A broken multiplier
/// counts as P 0.
pub fn xp_percent_sized(hp_mult: f64, share: f64) -> u32 {
    let bonus = 1.0 + 0.5 * (hp_mult - 1.0);
    let bonus = if bonus.is_finite() { bonus } else { 1.0 };
    let v = 100.0 * bonus * share;
    if v.is_finite() { v.round().clamp(1.0, 500.0) as u32 } else { 100 }
}

/// What a slime of `size` is worth of its kind's experience: Path-of-craft-tree pays the bog slime
/// its full size's health (`stats.health`, 16), and a slime's health goes with its area, so size
/// 4 of 4 is × 1, size 2 × 1/4, size 1 × 1/16 (PCT's request, 2026-11-13). Whole for a mob
/// without sizes (`size` or `full` 0).
pub fn size_share(size: u8, full: u8) -> f64 {
    if size == 0 || full == 0 {
        return 1.0;
    }
    (f64::from(size) / f64::from(full)).powi(2)
}

/// The tag for a mob of base health `base` scaled to `scaled` (times any multiplier of its own,
/// as the event's banner-bearer's × 3).
pub fn xp_tag(base: f64, scaled: f64) -> String {
    xp_tag_sized(base, scaled, 1.0)
}

/// [`xp_tag`] for a mob worth `share` of its kind ([`size_share`]).
pub fn xp_tag_sized(base: f64, scaled: f64, share: f64) -> String {
    let mult = if base > 0.0 { scaled / base } else { 1.0 };
    format!("{XP_TAG_PREFIX}{}", xp_percent_sized(mult, share))
}

/// A mob's health after its maximum went from `old_max` to `new_max` (Г2 on an adopted vanilla
/// mob): the same fraction of the new maximum, never above it. A mob another mod hurt or set low
/// stays as low in proportion (RustCraft's test spider at 2 of 16 stays at 2 with P 0); a fresh,
/// full mob is full at its grown maximum. A broken maximum keeps the health as it was, capped.
pub fn kept_health(health: f32, old_max: f32, new_max: f32) -> f32 {
    if !(old_max > 0.0 && new_max > 0.0 && health.is_finite()) {
        return if new_max > 0.0 { health.min(new_max) } else { health };
    }
    (health * new_max / old_max).min(new_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoption_keeps_the_health_fraction() {
        // RustCraft's test spider: 2 of 16 at P 0 stays 2; at P 1 (× 1.9) it is 2 of 30.4 → 3.8.
        assert_eq!(kept_health(2.0, 16.0, 16.0), 2.0);
        assert!((kept_health(2.0, 16.0, 30.4) - 3.8).abs() < 1e-5);
        // A fresh, full zombie is full at its grown maximum.
        assert_eq!(kept_health(20.0, 20.0, 38.0), 38.0);
        // Never above the new maximum, never raised past its proportion.
        assert_eq!(kept_health(25.0, 20.0, 20.0), 20.0);
        assert_eq!(kept_health(10.0, 20.0, 10.0), 5.0);
        // Broken maxima: as it was, capped.
        assert_eq!(kept_health(7.0, 0.0, 20.0), 7.0);
        assert_eq!(kept_health(30.0, 0.0, 20.0), 20.0);
        assert_eq!(kept_health(7.0, 20.0, 0.0), 7.0);
    }

    #[test]
    fn xp_tag_for_pct() {
        let s = Stats { health: 20.0, damage: 3.0, armor: 2.0, ..Stats::default() };
        let at = |p: f64| xp_tag(s.health, Scaled::at(&s, p).max_health);
        assert_eq!(at(0.0), "pct.xp100");
        assert_eq!(at(1.0), "pct.xp145");
        assert_eq!(at(0.5), "pct.xp123");
        // The banner-bearer's × 3 at P = 0.
        assert_eq!(xp_tag(20.0, 60.0), "pct.xp200");
        assert_eq!(xp_percent(20.0), 500);
        assert_eq!(xp_percent(-5.0), 1);
        assert_eq!(xp_percent(f64::NAN), 100);
        assert_eq!(xp_tag(0.0, 5.0), "pct.xp100");
    }

    #[test]
    fn a_smaller_slime_pays_its_share_of_the_full_size() {
        // PCT pays the bog slime its full size's health (16); size 2 is a quarter, size 1 a 16th.
        assert_eq!(size_share(4, 4), 1.0);
        assert_eq!(size_share(2, 4), 0.25);
        assert_eq!(size_share(1, 4), 0.0625);
        // Not a slime (size 0, or a mob without sizes): whole.
        assert_eq!(size_share(0, 4), 1.0);
        assert_eq!(size_share(0, 0), 1.0);
        assert_eq!(size_share(3, 0), 1.0);
        let slime = MobsFile::embedded().mob("bog_slime").unwrap().clone();
        let full = slime.full_size();
        assert_eq!(full, 4);
        let hp = |size: u8, p: f64| {
            let s = slime.sizes.iter().find(|s| s.size == u32::from(size)).unwrap();
            let stats = Stats { health: s.health, ..slime.stats };
            (s.health, Scaled::at(&stats, p).max_health)
        };
        let tag = |size: u8, p: f64| {
            let (base, scaled) = hp(size, p);
            xp_tag_sized(base, scaled, size_share(size, full))
        };
        assert_eq!(tag(4, 0.0), "pct.xp100");
        assert_eq!(tag(4, 1.0), "pct.xp145");
        assert_eq!(tag(2, 0.0), "pct.xp25");
        // 145 / 4 = 36.25: the share multiplies the P bonus, then it rounds.
        assert_eq!(tag(2, 1.0), "pct.xp36");
        assert_eq!(tag(1, 0.0), "pct.xp6");
        assert_eq!(tag(1, 1.0), "pct.xp9");
        // Clamped 1..500 after the share.
        assert_eq!(xp_percent_sized(1.0, 0.001), 1);
        assert_eq!(xp_percent_sized(30.0, 1.0), 500);
        // A broken multiplier counts as P 0: the share alone.
        assert_eq!(xp_percent_sized(f64::NAN, 0.25), 25);
        assert_eq!(xp_tag(20.0, 38.0), xp_tag_sized(20.0, 38.0, 1.0));
    }
    use crate::data::MobsFile;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn growth_at_p_one_matches_hypothesis_g2() {
        let zombie = Stats {
            health: 20.0,
            damage: 3.0,
            armor: 2.0,
            speed: Some(0.23),
            ..Stats::default()
        };
        let s = Scaled::at(&zombie, 1.0);
        assert!(close(s.max_health, 38.0));
        assert!(close(s.damage, 4.8));
        assert!(close(s.armor, 8.0));
        assert!(close(s.speed, 0.253));
        assert_eq!(s.decide_every, 10);
        assert_eq!(Scaled::at(&zombie, 0.0).decide_every, 20);
    }

    #[test]
    fn revenant_overhead_at_p_zero_and_one() {
        let r = MobsFile::embedded().mob("revenant").unwrap().clone();
        let o = r.attack("overhead").unwrap().damage;
        assert!(close(Scaled::at(&r.stats, 0.0).blow(o), 5.2));
        assert!(close(Scaled::at(&r.stats, 1.0).blow(o), 4.0 * 1.6 * 1.3));
        assert!(close(Scaled::at(&r.stats, 1.0).max_health, 57.0));
    }

    #[test]
    fn p_is_clamped() {
        assert_eq!(clamp_p(-1.0), 0.0);
        assert_eq!(clamp_p(3.0), 1.0);
        assert_eq!(clamp_p(f64::NAN), 0.0);
        let s = Stats { health: 10.0, ..Stats::default() };
        assert_eq!(Scaled::at(&s, 5.0).max_health, 19.0);
        assert_eq!(Scaled::at(&s, 0.0).speed, DEFAULT_SPEED);
    }

    #[test]
    fn weapon_tiers() {
        assert_eq!(weapon_tier(None), 0);
        assert_eq!(weapon_tier(Some("minecraft:wooden_sword")), 1);
        assert_eq!(weapon_tier(Some("minecraft:golden_axe")), 1);
        assert_eq!(weapon_tier(Some("minecraft:stone_sword")), 2);
        assert_eq!(weapon_tier(Some("minecraft:iron_sword")), 3);
        assert_eq!(weapon_tier(Some("minecraft:diamond_axe")), 4);
        assert_eq!(weapon_tier(Some("minecraft:netherite_sword")), 5);
        assert_eq!(weapon_tier(Some("minecraft:iron_pickaxe")), 0);
        assert_eq!(weapon_tier(Some("minecraft:mace")), 4);
        assert_eq!(weapon_tier(Some("minecraft:bow")), 2);
        assert_eq!(weapon_tier(Some("rustcraft:wrench")), 0);
    }

    fn summary(levels: [f64; 3], ranks: Option<[f64; 3]>) -> Summary {
        use crate::pct::Pair;
        Summary {
            levels: levels.map(|l| Pair { value: l, max: 75.0 }),
            ranks: ranks.map(|r| r.map(|v| Pair { value: v, max: 96.0 })),
        }
    }

    #[test]
    fn player_power_g1_gear_only_without_the_summary() {
        let w = PowerDef::default();
        assert_eq!(player_power(&PowerInputs::default(), &w), 0.0);
        let netherite = PowerInputs { tree: None, armor: 20.0, weapon_tier: 5 };
        assert!(close(player_power(&netherite, &w), 0.5));
        let iron = PowerInputs { tree: None, armor: 15.0, weapon_tier: 3 };
        assert!(close(player_power(&iron, &w), 0.5 * (0.6 * 0.75 + 0.4 * 0.6)));
    }

    #[test]
    fn player_power_g1_with_the_tree() {
        let w = PowerDef::default();
        let netherite = PowerInputs { tree: None, armor: 20.0, weapon_tier: 5 };
        // Levels only (a summary without ranks): the two best of 75, 10, 75.
        let veteran = PowerInputs { tree: Some(summary([75.0, 10.0, 75.0], None)), ..netherite };
        assert!(close(player_power(&veteran, &w), 1.0));
        // Levels and ranks: melee 60/75 with 45 of 75 buyable ranks, defense 30/75 with 20.
        let mid = PowerInputs { tree: Some(summary([60.0, 12.0, 30.0], Some([45.0, 3.0, 20.0]))), armor: 10.0, weapon_tier: 3 };
        let melee = 0.5 * 0.8 + 0.5 * 0.6;
        let defense = 0.5 * 0.4 + 0.5 * (20.0 / 75.0);
        let t = (melee + defense) / 2.0;
        assert!(close(tree_part(mid.tree.as_ref().unwrap(), &w), t));
        assert!(close(player_power(&mid, &w), 0.5 * t + 0.5 * (0.3 + 0.24)));
        // High levels with nothing learned count half: experience without nodes.
        let unspent = summary([75.0, 75.0, 75.0], Some([0.0; 3]));
        assert!(close(tree_part(&unspent, &w), 0.5));
    }

    #[test]
    fn power_weights_come_from_the_data() {
        let w = PowerDef { tree_share: 0.25, tree_top_branches: 1, level_weight: 1.0, rank_weight: 0.0, ..PowerDef::default() };
        let s = summary([75.0, 0.0, 15.0], Some([0.0; 3]));
        assert!(close(tree_part(&s, &w), 1.0));
        let i = PowerInputs { tree: Some(s), armor: 0.0, weapon_tier: 0 };
        assert!(close(player_power(&i, &w), 0.25));
        assert_eq!(MobsFile::embedded().power, PowerDef::default());
    }
    #[test]
    fn group_power_averages_the_players_near_the_nearest() {
        assert_eq!(group_power([0.0; 3], &[]), None);
        let players = [([10.0, 0.0, 0.0], 0.2), ([30.0, 0.0, 0.0], 0.6), ([200.0, 0.0, 0.0], 1.0)];
        assert!(close(group_power([0.0; 3], &players).unwrap(), 0.4));
        assert!(close(group_power([190.0, 0.0, 0.0], &players).unwrap(), 1.0));
    }
}
