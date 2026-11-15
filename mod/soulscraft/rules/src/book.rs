//! The hunter's book (docs/design/08-guide-book.md, data/book.json): what each player has met,
//! killed and built, the stage each entry stands at, who gets a kill, the action-bar line of a new
//! stage, the message that carries a player's progress to their client, and the pages the client
//! draws. The entries are read from data/mobs.json, so a mob added there (a changed vanilla mob
//! too) has its page without code.

use crate::{
    altar::AltarFile,
    geom::{self, V3},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The copy of data/book.json the crate was built with (mod/sync.ps1 writes it).
pub const EMBEDDED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/book.json"));
/// The player's persistent data key the progress lives under (PCT's `pct:summary` sits beside it).
pub const KEY: &str = "soulscraft:book";
/// Server → client: a [`BookMsg`].
pub const CHANNEL: &str = "soulscraft:book";
/// The item that opens the book.
pub const ITEM: &str = "soulscraft:hunter_book";
/// The entity tag a spawner's mob carries (it outlives a restart).
pub const SPAWNER_TAG: &str = "soulscraft.spawner";
/// The stages, from 0.
pub const UNKNOWN: usize = 0;
pub const MET: usize = 1;
pub const KILLED: usize = 2;
pub const STUDIED: usize = 3;
pub const EXPERT: usize = 4;
const EPS: f64 = 1e-9;

// ---- data/book.json ----

#[derive(Clone, Debug, Deserialize)]
pub struct StageDef {
    pub id: String,
    pub n: usize,
    pub name: String,
    pub kills: Option<f64>,
    pub elite_kills: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Credit {
    pub radius_blocks: f64,
    pub window_seconds: f64,
    pub spawner_weight: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AltarEntryDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub when: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BookFile {
    pub format: String,
    pub item: String,
    pub sections: Vec<String>,
    pub stages: Vec<StageDef>,
    pub credit: Credit,
    pub altar_entries: Vec<AltarEntryDef>,
    pub action_bar: String,
}

impl BookFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: Self = serde_json::from_str(text).map_err(|e| format!("book.json: {e}"))?;
        if f.format != "soulscraft-book" {
            return Err(format!("book.json: format {:?}", f.format));
        }
        if f.stages.len() != 5 || f.stages.iter().enumerate().any(|(i, s)| s.n != i) {
            return Err("book.json: five stages numbered 0 to 4".into());
        }
        let need = |s: &StageDef| s.kills.zip(s.elite_kills);
        let mut last = (-1.0, -1.0);
        for s in &f.stages[MET..] {
            let (k, e) = need(s).ok_or(format!("book.json: stage {} without kills", s.id))?;
            if k < last.0 || e < last.1 {
                return Err(format!("book.json: stage {} needs fewer kills than the one before", s.id));
            }
            last = (k, e);
        }
        if !(f.credit.spawner_weight > 0.0 && f.credit.spawner_weight <= 1.0) {
            return Err("book.json: spawner_weight outside (0, 1]".into());
        }
        Ok(f)
    }

    pub fn embedded() -> Self {
        Self::parse(EMBEDDED).expect("the embedded book.json reads (crate tests check it)")
    }

    /// The kills a stage needs (an elite mob's own column).
    pub fn need(&self, stage: usize, elite: bool) -> f64 {
        let s = &self.stages[stage.min(EXPERT)];
        if elite { s.elite_kills } else { s.kills }.unwrap_or(0.0)
    }

    /// The stage of an entry met (or not) with `kills` (weighted) kills.
    pub fn stage(&self, met: bool, kills: f64, elite: bool) -> usize {
        for s in [EXPERT, STUDIED, KILLED] {
            if kills + EPS >= self.need(s, elite) {
                return s;
            }
        }
        if met || kills > EPS { MET } else { UNKNOWN }
    }

    pub fn stage_name(&self, stage: usize) -> &str {
        &self.stages[stage.min(EXPERT)].name
    }

    /// The window of a participant's hit, in ticks.
    pub fn window_ticks(&self) -> u64 {
        (self.credit.window_seconds * 20.0).round() as u64
    }

    /// The action-bar line: `{mob}` and `{stage}` filled in.
    pub fn line(&self, unlock: &Unlock) -> String {
        self.action_bar.replace("{mob}", &unlock.title).replace("{stage}", &unlock.stage)
    }

    pub fn altar_entry(&self, id: &str) -> Option<&AltarEntryDef> {
        self.altar_entries.iter().find(|e| e.id == id)
    }
}

// ---- The entries: data/mobs.json ----

/// One enemy's entry: a SoulsCraft mob or a changed vanilla one.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    /// The entity types it stands for (`soulscraft:revenant`, `minecraft:zombie`).
    pub keys: Vec<String>,
    pub name: String,
    pub elite: bool,
    /// A vanilla mob: no rig of SoulsCraft's to draw.
    pub vanilla: bool,
}

/// Every entry of the «Враги» section, with the data its page is made of.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub entries: Vec<(Entry, Value)>,
    pub events: Vec<Value>,
    /// The world drops of SoulsCraft's mobs: mobs.json's soul_shard and horn_drop.
    pub world: Value,
    by_key: HashMap<String, usize>,
}

/// The vanilla mobs' list in mobs.json, whatever shape it takes: `vanilla` as a list, an object
/// with a `groups` (07-vanilla-mobs.md's rows) or `mobs` list, or an object of mobs.
fn vanilla_list(root: &Value) -> Vec<Value> {
    match root.get("vanilla") {
        Some(Value::Array(v)) => v.clone(),
        Some(Value::Object(o)) => match o.get("groups").or_else(|| o.get("mobs")) {
            Some(Value::Array(v)) => v.clone(),
            _ => o
                .iter()
                .filter_map(|(k, v)| {
                    let mut v = v.as_object()?.clone();
                    v.entry("id").or_insert_with(|| Value::String(k.clone()));
                    Some(Value::Object(v))
                })
                .collect(),
        },
        _ => Vec::new(),
    }
}

fn keys_of(m: &Value, id: &str) -> Vec<String> {
    let mut keys: Vec<String> = match m.get("keys").or_else(|| m.get("types")) {
        Some(Value::Array(v)) => v.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    };
    if let Some(k) = m.get("key").and_then(Value::as_str) {
        keys.insert(0, k.to_owned());
    }
    if keys.is_empty() {
        keys.push(if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") });
    }
    keys.dedup();
    keys
}

impl Catalog {
    /// From the text of data/mobs.json.
    pub fn parse(mobs_json: &str) -> Result<Self, String> {
        let root: Value = serde_json::from_str(mobs_json).map_err(|e| format!("mobs.json: {e}"))?;
        Ok(Self::from_value(&root))
    }

    pub fn embedded() -> Self {
        Self::parse(crate::data::EMBEDDED).expect("the embedded mobs.json reads")
    }

    pub fn from_value(root: &Value) -> Self {
        let mut c = Catalog::default();
        c.world = serde_json::json!({ "soul_shard": root["soul_shard"], "horn_drop": root["horn_drop"] });
        let own = root.get("mobs").and_then(Value::as_array).cloned().unwrap_or_default();
        for (list, vanilla_list_flag) in [(own, false), (vanilla_list(root), true)] {
            for m in list {
                let Some(id) = m.get("id").and_then(Value::as_str).map(str::to_owned) else { continue };
                if c.entries.iter().any(|(e, _)| e.id == id) {
                    continue;
                }
                let keys = keys_of(&m, &id);
                let vanilla = vanilla_list_flag || !keys[0].starts_with("soulscraft:");
                let entry = Entry {
                    name: m.get("name").and_then(Value::as_str).unwrap_or(&id).to_owned(),
                    elite: m.get("elite").and_then(Value::as_bool).unwrap_or(false),
                    id,
                    keys,
                    vanilla,
                };
                c.entries.push((entry, m));
            }
        }
        for (i, (e, _)) in c.entries.iter().enumerate() {
            for k in &e.keys {
                c.by_key.entry(k.clone()).or_insert(i);
            }
        }
        c.events = root.get("events").and_then(Value::as_array).cloned().unwrap_or_default();
        c
    }

    /// The entry of an entity type.
    pub fn by_key(&self, key: &str) -> Option<&Entry> {
        self.by_key.get(key).map(|i| &self.entries[*i].0)
    }

    pub fn get(&self, id: &str) -> Option<&(Entry, Value)> {
        self.entries.iter().find(|(e, _)| e.id == id)
    }

    pub fn event_name(&self, id: &str) -> String {
        self.events
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(id))
            .and_then(|e| e.get("name").and_then(Value::as_str))
            .unwrap_or(id)
            .to_owned()
    }
}

// ---- A player's progress ----

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MobProgress {
    #[serde(default)]
    pub met: bool,
    /// Weighted kills (a spawner's mob counts `spawner_weight`).
    #[serde(default)]
    pub kills: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AltarProgress {
    #[serde(default)]
    pub base: bool,
    #[serde(default)]
    pub levels: BTreeSet<u32>,
    /// Modifier ids (`weight`, `kin_undead`).
    #[serde(default)]
    pub banners: BTreeSet<String>,
    #[serde(default)]
    pub defense: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventProgress {
    #[serde(default)]
    pub tried: bool,
    #[serde(default)]
    pub won: bool,
}

/// What one player has opened, as saved under [`KEY`] (JSON text) and sent to their client.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    #[serde(default)]
    pub mobs: BTreeMap<String, MobProgress>,
    #[serde(default)]
    pub altar: AltarProgress,
    #[serde(default)]
    pub events: BTreeMap<String, EventProgress>,
}

/// A new stage or a newly opened entry, for the action bar and the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Unlock {
    /// The entry: a mob's name, «Уровень 2», a banner's name, an event's name.
    pub title: String,
    pub stage: String,
    /// The entry's id (`revenant`, `level_2`, `banner_weight`, `event_test_zombies`).
    pub id: String,
}

impl Progress {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Saved text back; nothing that reads is an empty book (never an error that wipes it: the
    /// server keeps the text it could not read and does not write over it).
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("{KEY}: {e}"))
    }

    pub fn mob(&self, id: &str) -> MobProgress {
        self.mobs.get(id).cloned().unwrap_or_default()
    }

    pub fn stage(&self, book: &BookFile, e: &Entry) -> usize {
        let m = self.mob(&e.id);
        book.stage(m.met, m.kills, e.elite)
    }

    fn mob_unlock(&self, book: &BookFile, e: &Entry, before: usize) -> Option<Unlock> {
        let after = self.stage(book, e);
        (after > before).then(|| Unlock { title: e.name.clone(), stage: book.stage_name(after).to_owned(), id: e.id.clone() })
    }

    /// The mob was in 16 blocks and saw the player, or hit them.
    pub fn meet(&mut self, book: &BookFile, e: &Entry) -> Option<Unlock> {
        let before = self.stage(book, e);
        self.mobs.entry(e.id.clone()).or_default().met = true;
        self.mob_unlock(book, e, before)
    }

    /// A kill credited, `weight` of a full one.
    pub fn kill(&mut self, book: &BookFile, e: &Entry, weight: f64) -> Option<Unlock> {
        let before = self.stage(book, e);
        let m = self.mobs.entry(e.id.clone()).or_default();
        m.met = true;
        m.kills = ((m.kills + weight) * 100.0).round() / 100.0;
        self.mob_unlock(book, e, before)
    }

    fn opened(title: String, id: String) -> Option<Unlock> {
        Some(Unlock { title, stage: "открыто".into(), id })
    }

    /// The altar's base: a summon item or an altar block in hand.
    pub fn altar_base(&mut self, book: &BookFile) -> Option<Unlock> {
        if std::mem::replace(&mut self.altar.base, true) {
            return None;
        }
        let name = book.altar_entry("altar").map_or("Алтарь: основа", |e| &e.name).to_owned();
        Self::opened(name, "altar".into())
    }

    /// Levels 1 to `level` of an altar built (or built near the player): each new one.
    pub fn altar_levels(&mut self, level: u32) -> Vec<Unlock> {
        (1..=level.min(crate::altar::MAX_RING))
            .filter(|n| self.altar.levels.insert(*n))
            .filter_map(|n| Self::opened(format!("Уровень алтаря {n}"), format!("level_{n}")))
            .collect()
    }

    /// A banner held: its modifier.
    pub fn banner(&mut self, altar: &AltarFile, modifier: &str) -> Option<Unlock> {
        let m = altar.modifiers.iter().find(|m| m.id == modifier)?;
        if !self.altar.banners.insert(m.id.clone()) {
            return None;
        }
        Self::opened(m.name.clone(), format!("banner_{}", m.id))
    }

    /// An event started near the player: «Оборона алтаря» and the event's description.
    pub fn event_tried(&mut self, book: &BookFile, catalog: &Catalog, event: &str) -> Vec<Unlock> {
        let mut out = Vec::new();
        if !std::mem::replace(&mut self.altar.defense, true) {
            let name = book.altar_entry("defense").map_or("Оборона алтаря", |e| &e.name).to_owned();
            out.extend(Self::opened(name, "defense".into()));
        }
        let ev = self.events.entry(event.to_owned()).or_default();
        if !std::mem::replace(&mut ev.tried, true) {
            out.push(Unlock { title: catalog.event_name(event), stage: "описание".into(), id: format!("event_{event}") });
        }
        out
    }

    /// The event won with the player in it: its waves and reward.
    pub fn event_won(&mut self, book: &BookFile, catalog: &Catalog, event: &str) -> Vec<Unlock> {
        let mut out = self.event_tried(book, catalog, event);
        let ev = self.events.entry(event.to_owned()).or_default();
        if !std::mem::replace(&mut ev.won, true) {
            out.push(Unlock { title: catalog.event_name(event), stage: "волны и награда".into(), id: format!("event_{event}") });
        }
        out
    }
}

/// What an item in hand opens: the altar's base (its blocks, a summon item) or a banner's modifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    Base,
    Banner(String),
}

pub fn held(item: &str, altar: &AltarFile, summon_items: &[String]) -> Option<Held> {
    if let Some(m) = altar.modifiers.iter().find(|m| format!("soulscraft:{}", m.banner) == item) {
        return Some(Held::Banner(m.id.clone()));
    }
    let base = item.starts_with("soulscraft:altar_") || summon_items.iter().any(|s| s == item);
    base.then_some(Held::Base)
}

// ---- Kill credit ----

/// A player's hit on a mob, by the game tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub player: u128,
    pub tick: u64,
}

/// The hits each mob took lately (players only), kept for the window.
#[derive(Clone, Debug, Default)]
pub struct HitLog {
    hits: HashMap<u32, Vec<Hit>>,
}

impl HitLog {
    pub fn add(&mut self, mob: u32, player: u128, tick: u64) {
        let list = self.hits.entry(mob).or_default();
        list.retain(|h| h.player != player);
        list.push(Hit { player, tick });
    }

    pub fn take(&mut self, mob: u32) -> Vec<Hit> {
        self.hits.remove(&mob).unwrap_or_default()
    }

    /// Hits older than `window` go; a mob with none left is forgotten.
    pub fn prune(&mut self, now: u64, window: u64) {
        self.hits.retain(|_, v| {
            v.retain(|h| now.saturating_sub(h.tick) <= window);
            !v.is_empty()
        });
    }

    pub fn len(&self) -> usize {
        self.hits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }
}

/// Who gets a kill (08-guide-book.md, «Кто получает засчитанное убийство»): the player who dealt
/// the last blow, wherever they stand, and every player who hit the mob in the last
/// `window_seconds` and stands within `radius_blocks` of where it died. `at` is where each player
/// stands now (a player not listed has left). Each player once, the killer first.
pub fn credited(book: &BookFile, killer: Option<u128>, hits: &[Hit], now: u64, died_at: V3, at: &HashMap<u128, V3>) -> Vec<u128> {
    let mut out: Vec<u128> = killer.into_iter().collect();
    let window = book.window_ticks();
    for h in hits {
        if out.contains(&h.player) || now.saturating_sub(h.tick) > window {
            continue;
        }
        if at.get(&h.player).is_some_and(|p| geom::dist(*p, died_at) <= book.credit.radius_blocks) {
            out.push(h.player);
        }
    }
    out
}

/// A turret's or a guard's kill goes to its owner when they stand within `radius_blocks`.
pub fn owner_credited(book: &BookFile, owner_at: Option<V3>, died_at: V3) -> bool {
    owner_at.is_some_and(|p| geom::dist(p, died_at) <= book.credit.radius_blocks)
}

/// A kill's weight: a spawner's mob counts `spawner_weight`.
pub fn weight(book: &BookFile, from_spawner: bool) -> f64 {
    if from_spawner { book.credit.spawner_weight } else { 1.0 }
}

/// SoulsCraft's own console command for a machine's kill (docs/research/integration.md, «Убийства
/// машин RustCraft»): `soulscraft-kill <source> <owner, 32 hex> <entity id> <type> <x> <y> <z>`.
pub const KILL_COMMAND: &str = "soulscraft-kill";

/// What killed the mob for its owner: both are credited the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    Turret,
    Guard,
}

impl Machine {
    pub fn name(self) -> &'static str {
        match self {
            Machine::Turret => "turret",
            Machine::Guard => "guard",
        }
    }
}

/// A machine's kill, from either report.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineKill {
    pub machine: Machine,
    pub owner: u128,
    pub entity: u32,
    pub kind: String,
    /// The victim's block at its death: only `soulscraft-kill` gives it.
    pub at: Option<[i32; 3]>,
}

/// A machine's kill report as RustCraft writes it on the console: SoulsCraft's own
/// `soulscraft-kill turret|guard <owner> <entity> <type> <x> <y> <z>` (RustCraft 0.10.0 on), or
/// its older report to Path-of-craft-tree, `pct-machine kill <owner> <entity> <type>` (a turret's,
/// sent only while PCT is in the build). Anything else, or a broken one: None.
pub fn machine_kill(line: &str) -> Option<MachineKill> {
    let line = line.trim_start_matches('/');
    let mut w = line.split_whitespace();
    let (machine, own) = match w.next()? {
        KILL_COMMAND => match w.next()? {
            "turret" => (Machine::Turret, true),
            "guard" => (Machine::Guard, true),
            _ => return None,
        },
        "pct-machine" if w.next()? == "kill" => (Machine::Turret, false),
        _ => return None,
    };
    let owner = w.next().filter(|h| h.len() <= 32).and_then(|h| u128::from_str_radix(h, 16).ok())?;
    let entity = w.next()?.parse().ok()?;
    let kind = w.next()?.to_owned();
    let at = if own {
        let mut c = [0i32; 3];
        for v in &mut c {
            *v = w.next()?.parse().ok()?;
        }
        Some(c)
    } else {
        None
    };
    Some(MachineKill { machine, owner, entity, kind, at })
}

// ---- The message to the client ----

/// Server → client: the player's whole progress, and whether to open the book now.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BookMsg {
    #[serde(default)]
    pub open: bool,
    pub progress: Progress,
}

pub fn encode(m: &BookMsg) -> Vec<u8> {
    serde_json::to_vec(m).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<BookMsg> {
    serde_json::from_slice(bytes).ok()
}

#[cfg(test)]
#[path = "book_tests.rs"]
mod tests;
