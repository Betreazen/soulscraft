//! `data/mobs.json` read into types: the numbers of every mob, its attacks with their tick
//! phases, its behaviour parameters and the waves of the test event. The format is described in
//! data/README.md; this module only reads it and checks what the server relies on.
//!
//! The server starts from the copy compiled in ([`EMBEDDED`], written by mod/sync.ps1 from
//! data/mobs.json) and `/sc reload` swaps in a file read at run time, so numbers and timings are
//! tuned without a rebuild.

use serde::Deserialize;
use serde_json::{Map, Value};

/// The copy of data/mobs.json the crate was built with (mod/sync.ps1 writes it; never edit it).
pub const EMBEDDED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/mobs.json"));

#[derive(Clone, Debug, Deserialize)]
pub struct MobsFile {
    pub format: String,
    pub version: u32,
    pub tick_rate: u32,
    pub mobs: Vec<MobDef>,
    #[serde(default)]
    pub events: Vec<EventDef>,
    /// Effects SoulsCraft owns for every mod (`bleeding`): their numbers.
    #[serde(default)]
    pub effects: Map<String, Value>,
    /// The player power P's weights (hypothesis Г1).
    #[serde(default)]
    pub power: PowerDef,
    /// Room for a mob's box where it appears (round 9): how far the spot may move.
    #[serde(default)]
    pub spawn_fit: SpawnFit,
    /// Cover from shooting turrets (crate::cover::CoverRules reads it).
    #[serde(default)]
    pub cover: Value,
    /// The approved changes of vanilla mobs (07-vanilla-mobs.md; crate::vanilla).
    #[serde(default)]
    pub vanilla: crate::vanilla::VanillaSection,
    /// The mobs in the natural spawn lists and the lurker's spawner (10-survival.md; crate::natural).
    #[serde(default)]
    pub natural: crate::natural::NaturalSection,
    /// The soul shard a credited kill drops (Г15).
    #[serde(default)]
    pub soul_shard: crate::natural::ShardDef,
    /// The summoning horn a credited kill drops now and then (11-summons-rewards.md, Г18).
    #[serde(default)]
    pub horn_drop: crate::natural::HornDropDef,
}

/// Room for a mob's box where it appears (owner's decision, round 9; crate::fit).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpawnFit {
    /// The farthest a spot moves, in blocks, to one where the box fits.
    pub search_blocks: i32,
}

impl Default for SpawnFit {
    fn default() -> Self {
        Self { search_blocks: 4 }
    }
}

impl MobDef {
    /// Whether it stands on the ground (not a flyer or a swimmer): its spot needs a floor.
    pub fn grounded(&self) -> bool {
        matches!(self.env.as_str(), "land" | "land_water")
    }
}

/// Г1's weights: `P = tree_share · T + (1 − tree_share) · G`, where `T` is the mean of the
/// `tree_top_branches` best combat branches of Path-of-craft-tree's summary (each
/// `level_weight · level share + rank_weight · rank share`), `G = armor_weight · A +
/// weapon_weight · W`. Without the summary `T = 0`. Hypotheses; data/README.md, «Сила игрока».
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct PowerDef {
    pub tree_share: f64,
    pub tree_top_branches: usize,
    pub level_weight: f64,
    pub rank_weight: f64,
    /// Ranks count full at this share of what a branch's own levels can buy (a point a level):
    /// the rank share is `ranks / (rank_full · min(levels highest, ranks highest))`.
    pub rank_full: f64,
    pub armor_weight: f64,
    pub weapon_weight: f64,
    /// Armour points that count as full (`minecraft:armor`, 20 for full netherite).
    pub armor_full: f64,
    /// Weapon tier that counts as full (netherite 5).
    pub weapon_full: f64,
}

impl Default for PowerDef {
    fn default() -> Self {
        Self {
            tree_share: 0.5,
            tree_top_branches: 2,
            level_weight: 0.5,
            rank_weight: 0.5,
            rank_full: 1.0,
            armor_weight: 0.6,
            weapon_weight: 0.4,
            armor_full: 20.0,
            weapon_full: 5.0,
        }
    }
}

/// A mob's collision box in blocks (`EntityDimensions`): `width` on both ground axes, `height`
/// up from the feet, `eye` the eye height. Derived from the rig's rest pose (data/README.md,
/// «Рамка столкновений»). The bog slime's is its size 4's; a smaller size scales it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Hitbox {
    pub width: f64,
    pub height: f64,
    pub eye: f64,
}

impl Hitbox {
    /// Vanilla zombie's, the template's: what a mob without a `hitbox` keeps.
    pub const ZOMBIE: Hitbox = Hitbox { width: 0.6, height: 1.95, eye: 1.74 };

    /// The box at a slime size (`size` of `full`), or this one for size 0 (not a slime).
    pub fn at_size(self, size: u8, full: u8) -> Hitbox {
        if size == 0 || full == 0 || size == full {
            return self;
        }
        let k = f64::from(size) / f64::from(full);
        Hitbox { width: self.width * k, height: self.height * k, eye: self.eye * k }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MobDef {
    /// Short id, the rig's and the texture's name: `revenant`.
    pub id: String,
    /// Entity type id: `soulscraft:revenant`.
    pub key: String,
    #[serde(default)]
    pub name: String,
    /// `land`, `air`, `water`, `land_water`.
    pub env: String,
    #[serde(default)]
    pub elite: bool,
    #[serde(default)]
    pub tier_p: f64,
    /// What it does in a wave (06-altar.md, «Оборона алтаря»): `assault`, `hunter`, `ranged`.
    #[serde(default)]
    pub wave_role: String,
    /// Undead (round 11): in `#minecraft:undead`, poison and regeneration do nothing, instant
    /// harm heals. Not undead clears the zombie template's `flags.undead` (hook_register_goals.rs).
    pub undead: bool,
    /// The kin banners that call it more often (06-altar.md, «Модификаторы»): some of [`KINS`].
    /// `undead` is exactly the `undead` flag.
    #[serde(default)]
    pub kin: Vec<String>,
    /// Its decorative drops (the bestiary's armour pieces).
    #[serde(default)]
    pub loot: Vec<LootDef>,
    /// Effect id to duration multiplier for RustCraft's turret modules: 0 immune, missing 1.
    #[serde(default)]
    pub turret_effects: Map<String, Value>,
    pub stats: Stats,
    /// Its own collision box; none keeps the template's ([`Hitbox::ZOMBIE`]).
    #[serde(default)]
    pub hitbox: Option<Hitbox>,
    #[serde(default)]
    pub attacks: Vec<AttackDef>,
    #[serde(default)]
    pub moves: Vec<MoveDef>,
    #[serde(default)]
    pub passives: Value,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub sizes: Vec<SizeDef>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Stats {
    pub health: f64,
    pub damage: f64,
    #[serde(default)]
    pub armor: f64,
    /// Movement speed attribute; flyers and swimmers have none in the data (vanilla zombie's then).
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub knockback_resistance: f64,
    #[serde(default)]
    pub flying: bool,
    #[serde(default)]
    pub water_only: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AttackDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The rig clip; empty for a vanilla mob's attack (vanilla keeps its own model).
    #[serde(default)]
    pub clip: String,
    pub windup: u32,
    #[serde(default)]
    pub hold: u32,
    pub hit: u32,
    pub recover: u32,
    /// Multiplier on `stats.damage`.
    pub damage: f64,
    #[serde(default)]
    pub knockback: bool,
    #[serde(default)]
    pub effects: Vec<EffectDef>,
    /// Ticks of the strikes inside the hit phase, when there are several.
    #[serde(default)]
    pub strikes_at: Option<Vec<u32>>,
    /// A dash of this many blocks; the hit phase is then the dash.
    #[serde(default)]
    pub dash_blocks: Option<f64>,
    #[serde(default)]
    pub projectile: Option<String>,
    #[serde(default)]
    pub wave: Option<WaveDef>,
    #[serde(default)]
    pub arc_degrees: Option<f64>,
    #[serde(default)]
    pub recover_if: Option<String>,
    /// A state after another attack (the lurker's grab), not an attack with phases.
    #[serde(default)]
    pub follows: Option<String>,
    #[serde(default)]
    pub grab_max_ticks: Option<u32>,
    /// What an own mob's attack sounds and shows like (a vanilla group's is `VanillaAttack.cue`).
    #[serde(default)]
    pub cue: Option<AttackCue>,
}

/// An own mob's cue: the vanilla groups' schema (`sound`, `pitch`, `particle`, `pose`, `hop`) and
/// the phase it plays at, the windup's start unless `on` says `hit` or `recover`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AttackCue {
    #[serde(flatten)]
    pub cue: crate::vanilla::Cue,
    #[serde(default)]
    pub on: CueOn,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CueOn {
    #[default]
    Windup,
    Hit,
    Recover,
}

impl CueOn {
    /// The attack phase it plays at.
    pub fn phase(self) -> crate::attack::Phase {
        match self {
            CueOn::Windup => crate::attack::Phase::Windup,
            CueOn::Hit => crate::attack::Phase::Hit,
            CueOn::Recover => crate::attack::Phase::Recover,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct EffectDef {
    pub id: String,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default)]
    pub level: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct WaveDef {
    pub radius_blocks: f64,
    #[serde(default)]
    pub damage: Option<f64>,
    #[serde(default)]
    pub effects: Vec<EffectDef>,
    #[serde(default)]
    pub jumpable: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct MoveDef {
    pub id: String,
    #[serde(default)]
    pub clip: Option<String>,
    /// What the move sounds like when it starts (the marksman's reload behind cover).
    #[serde(default)]
    pub cue: Option<crate::vanilla::Cue>,
}

/// The kin groups of the kin banners (data/altar.json `kin_names`).
pub const KINS: [&str; 5] = ["undead", "beasts", "slimes", "flyers", "water"];

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LootDef {
    pub item: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub chance: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct SizeDef {
    pub size: u32,
    pub health: f64,
    pub damage: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EventDef {
    pub id: String,
    #[serde(default)]
    pub rest_seconds: Vec<f64>,
    /// The altar's health at the start, before the resilience banner (hypothesis Г12).
    #[serde(default)]
    pub altar_health: f64,
    /// The altar level the event needs (06-altar.md: an older event runs on a younger altar).
    #[serde(default)]
    pub tier: u32,
    /// The summon item that starts it from the pedestal.
    #[serde(default)]
    pub item: String,
    /// The event zone, the path map and the spawn points (crate::event::Flow).
    #[serde(default)]
    pub flow: Map<String, Value>,
    /// The altar defense numbers (hypothesis Г12).
    #[serde(default)]
    pub defense: Map<String, Value>,
    #[serde(default)]
    pub waves: Vec<EventWave>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EventWave {
    pub wave: u32,
    pub mobs: Vec<EventMob>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EventMob {
    #[serde(rename = "type")]
    pub kind: String,
    pub count: u32,
    #[serde(default)]
    pub armored: u32,
    #[serde(default)]
    pub health_mult: Option<f64>,
}

impl MobsFile {
    /// Reads and checks a mobs.json: the format, and every attack's phases.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(text).map_err(|e| format!("mobs.json: {e}"))?;
        if file.format != "soulscraft-mobs" {
            return Err(format!("mobs.json: format {:?}, not soulscraft-mobs", file.format));
        }
        if file.tick_rate != 20 {
            return Err(format!("mobs.json: tick_rate {}, the game runs 20", file.tick_rate));
        }
        for mob in &file.mobs {
            mob.check()?;
        }
        file.vanilla.check()?;
        Ok(file)
    }

    /// The compiled-in copy. It is checked by the crate's tests, so it always reads.
    pub fn embedded() -> Self {
        Self::parse(EMBEDDED).expect("the embedded mobs.json reads (crate tests check it)")
    }

    pub fn mob(&self, id: &str) -> Option<&MobDef> {
        self.mobs.iter().find(|m| m.id == id || m.key == id)
    }

    /// The mobs of a kin, in the file's order.
    pub fn of_kin(&self, kin: &str) -> Vec<&MobDef> {
        self.mobs.iter().filter(|m| m.has_kin(kin)).collect()
    }

    /// The mob whose entity type is `key` (`soulscraft:revenant`), with its index.
    pub fn by_key(&self, key: &str) -> Option<(usize, &MobDef)> {
        self.mobs.iter().enumerate().find(|(_, m)| m.key == key)
    }
}

impl MobDef {
    fn check(&self) -> Result<(), String> {
        if !self.key.starts_with("soulscraft:") || self.key[11..] != self.id {
            return Err(format!("mob {}: key {} is not soulscraft:{}", self.id, self.key, self.id));
        }
        if !(self.stats.health > 0.0) || self.stats.damage < 0.0 || self.stats.armor < 0.0 {
            return Err(format!("mob {}: health, damage or armor out of range", self.id));
        }
        if self.undead && self.turret_effects.get("poison").and_then(Value::as_f64) != Some(0.0) {
            return Err(format!("mob {}: undead, so turret_effects.poison must be 0", self.id));
        }
        if let Some(k) = self.kin.iter().find(|k| !KINS.contains(&k.as_str())) {
            return Err(format!("mob {}: kin {k:?} is none of {KINS:?}", self.id));
        }
        if self.has_kin("undead") != self.undead {
            return Err(format!("mob {}: kin undead must match undead {}", self.id, self.undead));
        }
        if let Some(h) = self.hitbox
            && !(h.width > 0.0 && h.height > 0.0 && h.eye > 0.0 && h.eye < h.height)
        {
            return Err(format!("mob {}: hitbox {h:?}: sizes above 0, eyes below the top", self.id));
        }
        for a in &self.attacks {
            if a.follows.is_some() {
                continue;
            }
            if a.hit == 0 {
                return Err(format!("mob {} attack {}: hit phase of 0 ticks", self.id, a.id));
            }
            if let Some(strikes) = &a.strikes_at
                && strikes.iter().any(|s| *s >= a.hit)
            {
                return Err(format!("mob {} attack {}: a strike past the hit phase", self.id, a.id));
            }
        }
        Ok(())
    }

    pub fn has_kin(&self, kin: &str) -> bool {
        self.kin.iter().any(|k| k == kin)
    }

    pub fn attack(&self, id: &str) -> Option<&AttackDef> {
        self.attacks.iter().find(|a| a.id == id)
    }

    pub fn attack_index(&self, id: &str) -> Option<usize> {
        self.attacks.iter().position(|a| a.id == id)
    }

    /// Attacks with phases (the lurker's grab is a state, not one of them).
    pub fn phased_attacks(&self) -> impl Iterator<Item = (usize, &AttackDef)> {
        self.attacks.iter().enumerate().filter(|(_, a)| a.follows.is_none())
    }

    pub fn param(&self, name: &str) -> Option<f64> {
        self.params.get(name).and_then(Value::as_f64)
    }

    /// A `[from, to]` parameter (`circle_radius_blocks`), or a single number as both ends.
    pub fn param_range(&self, name: &str) -> Option<(f64, f64)> {
        match self.params.get(name)? {
            Value::Array(v) if v.len() == 2 => Some((v[0].as_f64()?, v[1].as_f64()?)),
            v => v.as_f64().map(|x| (x, x)),
        }
    }

    pub fn param_bool(&self, name: &str) -> bool {
        self.params.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    /// A passive by id (`rage`, `split`); `passives` is one object or a list of them.
    pub fn passive(&self, id: &str) -> Option<&Map<String, Value>> {
        let is = |v: &&Value| v.get("id").and_then(Value::as_str) == Some(id);
        match &self.passives {
            Value::Array(list) => list.iter().find(is).and_then(Value::as_object),
            v @ Value::Object(_) => Some(v).filter(is).and_then(Value::as_object),
            _ => None,
        }
    }

    pub fn move_clip(&self, id: &str) -> Option<&str> {
        self.moves.iter().find(|m| m.id == id)?.clip.as_deref()
    }

    /// The largest size (the slime's 4), or 0 for a mob without sizes.
    pub fn full_size(&self) -> u8 {
        self.sizes.iter().map(|s| s.size).max().unwrap_or(0) as u8
    }

    /// Its collision box at `size` (0 for a mob without sizes).
    pub fn hitbox_at(&self, size: u8) -> Hitbox {
        self.hitbox.unwrap_or(Hitbox::ZOMBIE).at_size(size, self.full_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_file_reads_with_seven_mobs() {
        let f = MobsFile::embedded();
        assert_eq!(f.mobs.len(), 7);
        let ids: Vec<_> = f.mobs.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"]);
    }

    #[test]
    fn revenant_numbers_match_the_bestiary() {
        let f = MobsFile::embedded();
        let r = f.mob("revenant").unwrap();
        assert_eq!(r.stats.health, 30.0);
        assert_eq!(r.stats.damage, 4.0);
        assert_eq!(r.stats.armor, 6.0);
        let o = r.attack("overhead").unwrap();
        assert_eq!((o.windup, o.hold, o.hit, o.recover), (10, 8, 2, 16));
        assert_eq!(o.damage, 1.3);
        assert_eq!(r.param("circle_wait_max_ticks"), Some(60.0));
        assert_eq!(r.param_range("circle_radius_blocks"), Some((2.0, 3.0)));
        assert_eq!(r.move_clip("circle"), Some("circle_strafe"));
    }

    #[test]
    fn lookups_by_key_and_passives() {
        let f = MobsFile::embedded();
        let (i, b) = f.by_key("soulscraft:brute").unwrap();
        assert_eq!(i, 3);
        assert_eq!(b.passive("rage").unwrap()["below_health"], 0.3);
        let slime = f.mob("bog_slime").unwrap();
        assert!(slime.passive("split").is_some());
        assert!(slime.passive("rage").is_none());
        assert_eq!(slime.sizes.len(), 3);
        let lurker = f.mob("lurker").unwrap();
        assert_eq!(lurker.phased_attacks().count(), 1);
        assert!(f.mob("nobody").is_none());
        assert_eq!(f.events[0].waves.len(), 5);
        assert_eq!(f.events[0].waves[4].mobs[1].health_mult, Some(3.0));
        assert_eq!(f.events[0].defense["drain_windup_ticks"], 40);
        assert_eq!(f.effects["bleeding"]["period_ticks"], 20);
        let roles: Vec<&str> = f.mobs.iter().map(|m| m.wave_role.as_str()).collect();
        assert_eq!(roles, ["assault", "hunter", "ranged", "assault", "hunter", "ranged", "assault"]);
        assert_eq!(f.mob("revenant").unwrap().turret_effects["poison"], 0);
        assert_eq!(f.mob("brute").unwrap().turret_effects["slow"], 0.5);
        assert!(f.mob("stalker").unwrap().turret_effects.is_empty());
        let undead: Vec<&str> = f.mobs.iter().filter(|m| m.undead).map(|m| m.id.as_str()).collect();
        assert_eq!(undead, ["revenant", "marksman", "brute", "shrike"]);
        assert_eq!(f.mob("brute").unwrap().turret_effects["poison"], 0);
    }

    #[test]
    fn kin_follows_the_banners_and_undead_is_the_undead_flag() {
        let f = MobsFile::embedded();
        for m in &f.mobs {
            assert!(!m.kin.is_empty(), "{} has no kin", m.id);
            assert!(m.kin.iter().all(|k| KINS.contains(&k.as_str())), "{}: {:?}", m.id, m.kin);
            // Round 11 and the iteration-3 brief: undead kin is exactly the undead flag (the shrike too).
            assert_eq!(m.has_kin("undead"), m.undead, "{}", m.id);
        }
        let ids = |k: &str| f.of_kin(k).iter().map(|m| m.id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids("undead"), ["revenant", "marksman", "brute", "shrike"]);
        assert_eq!(ids("flyers"), ["shrike"]);
        assert_eq!(ids("slimes"), ["bog_slime"]);
        assert_eq!(ids("water"), ["lurker", "bog_slime"]);
        assert_eq!(ids("beasts"), ["stalker", "lurker"]);
        assert!(ids("dragons").is_empty());
        let bad = EMBEDDED.replacen("\"kin\": [\"beasts\"]", "\"kin\": [\"beast\"]", 1);
        assert!(MobsFile::parse(&bad).unwrap_err().contains("kin"));
        let lost = EMBEDDED.replacen("\"kin\": [\"undead\", \"flyers\"]", "\"kin\": [\"flyers\"]", 1);
        assert!(MobsFile::parse(&lost).unwrap_err().contains("undead"));
    }

    #[test]
    fn loot_names_the_armour_pieces() {
        let f = MobsFile::embedded();
        let items = |id: &str| f.mob(id).unwrap().loot.iter().map(|l| l.item.as_str()).collect::<Vec<_>>();
        assert_eq!(items("revenant"), ["soulscraft:rusty_helmet", "soulscraft:rusty_chestplate"]);
        assert_eq!(items("brute"), ["soulscraft:brute_helmet"]);
        assert!(items("shrike").is_empty());
        assert_eq!(f.mob("stalker").unwrap().loot[0].chance, 0.05);
    }

    #[test]
    fn bad_files_are_refused_with_a_reason() {
        assert!(MobsFile::parse("{").unwrap_err().contains("mobs.json"));
        let poisoned = EMBEDDED.replace("\"bleed\": 0.5, \"poison\": 0,", "\"bleed\": 0.5, \"poison\": 0.5,");
        assert!(MobsFile::parse(&poisoned).unwrap_err().contains("undead"));
        let wrong = EMBEDDED.replace("\"soulscraft-mobs\"", "\"other\"");
        assert!(MobsFile::parse(&wrong).unwrap_err().contains("format"));
        let rate = EMBEDDED.replacen("\"tick_rate\": 20", "\"tick_rate\": 10", 1);
        assert!(MobsFile::parse(&rate).unwrap_err().contains("tick_rate"));
        let key = EMBEDDED.replacen("soulscraft:revenant", "soulscraft:knight", 1);
        assert!(MobsFile::parse(&key).unwrap_err().contains("key"));
    }

    #[test]
    fn own_mobs_cue_their_attacks() {
        let f = MobsFile::embedded();
        let r = f.mob("revenant").unwrap();
        let swing = r.attack("slash").unwrap().cue.as_ref().unwrap();
        assert_eq!(swing.cue.sound.as_deref(), Some("minecraft:entity.zombie.attack_iron_door"));
        assert_eq!(swing.on, CueOn::Windup);
        let bash = r.attack("shield_bash").unwrap().cue.as_ref().unwrap();
        assert_eq!(bash.cue.sound.as_deref(), Some("minecraft:item.shield.block"));
        assert_eq!(bash.on, CueOn::Hit);
        let lunge = f.mob("stalker").unwrap().attack("lunge").unwrap().cue.as_ref().unwrap();
        assert_eq!(lunge.cue.sound.as_deref(), Some("minecraft:entity.fox.screech"));
        let m = f.mob("marksman").unwrap();
        let shot = m.attack("aimed_shot").unwrap().cue.as_ref().unwrap();
        assert_eq!((shot.cue.sound.as_deref(), shot.on), (Some("minecraft:item.crossbow.shoot"), CueOn::Hit));
        let hide = m.moves.iter().find(|x| x.id == "hide").and_then(|x| x.cue.as_ref()).unwrap();
        assert_eq!(hide.sound.as_deref(), Some("minecraft:item.crossbow.loading_start"));
        // A vanilla group's cue stays its own (`VanillaAttack.cue`), not its attack's.
        assert!(f.vanilla.groups.iter().flat_map(|g| &g.attacks).all(|a| a.def.cue.is_none()));
        assert!(f.vanilla.groups.iter().flat_map(|g| &g.attacks).any(|a| a.cue.sound.is_some()));
    }

    #[test]
    fn a_strike_outside_the_hit_phase_is_refused() {
        let text = EMBEDDED.replacen("\"strikes_at\": [0, 3]", "\"strikes_at\": [0, 9]", 1);
        assert_ne!(text, EMBEDDED, "the stalker's strikes_at was found");
        assert!(MobsFile::parse(&text).unwrap_err().contains("strike"));
    }
}

#[cfg(test)]
mod fit_tests {
    use super::*;

    #[test]
    fn spawn_fit_reads_and_mobs_know_their_ground() {
        let f = MobsFile::embedded();
        assert_eq!(f.spawn_fit.search_blocks, 4);
        assert!(f.mob("brute").unwrap().grounded());
        assert!(!f.mob("shrike").unwrap().grounded());
        assert!(!f.mob("lurker").unwrap().grounded());
        assert_eq!(SpawnFit::default().search_blocks, 4);
    }
}
