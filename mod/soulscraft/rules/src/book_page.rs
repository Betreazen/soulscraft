//! The hunter's book's pages as the client draws them (08-guide-book.md, the stage table): text
//! lines, the clips the model may play, and each attack's phase strip, made from data/mobs.json,
//! data/altar.json, data/trophies.json and data/book.json for a player's progress. The client only
//! lays them out.

use crate::{
    altar::AltarFile,
    book::{self, BookFile, Catalog, Entry, Progress},
    data::Stats,
    scale::Scaled,
};
use serde_json::Value;

/// How a line is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// A heading.
    Head,
    Text,
    /// A hint or a locked line.
    Dim,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
}

fn line(tone: Tone, text: impl Into<String>) -> Line {
    Line { text: text.into(), tone }
}

/// A part of an attack's phase strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seg {
    Windup,
    Hold,
    Hit,
    /// The recovery: the player's window.
    Window,
}

impl Seg {
    pub fn label(self) -> &'static str {
        match self {
            Seg::Windup => "замах",
            Seg::Hold => "пауза",
            Seg::Hit => "удар",
            Seg::Window => "окно игрока",
        }
    }
}

/// One attack with its phases in ticks (data/mobs.json), drawn as a strip under the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub name: String,
    pub clip: String,
    /// How to read the windup (`tell`).
    pub tell: String,
    pub segs: Vec<(Seg, u32)>,
}

impl Strip {
    pub fn ticks(&self) -> u32 {
        self.segs.iter().map(|s| s.1).sum()
    }

    /// The part at tick `t` of the attack (looped).
    pub fn seg_at(&self, t: u32) -> Option<Seg> {
        let total = self.ticks();
        if total == 0 {
            return None;
        }
        let mut t = t % total;
        for (s, n) in &self.segs {
            if t < *n {
                return Some(*s);
            }
            t -= n;
        }
        None
    }
}

/// One page of the book.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: String,
    pub stage: usize,
    pub lines: Vec<Line>,
    /// Draw the model as a silhouette (a locked entry).
    pub silhouette: bool,
    /// The model may walk (stage 2 on); before that it stands at rest.
    pub moves: bool,
    /// The attacks with their strips (stage 3 on).
    pub strips: Vec<Strip>,
}

fn num(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-9 { format!("{}", r.round() as i64) } else { format!("{r}") }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// A phased attack's strip (the lurker's grab, a state after another attack, has none).
pub fn strip(a: &Value) -> Option<Strip> {
    if a.get("follows").is_some_and(|f| !f.is_null()) {
        return None;
    }
    let n = |k: &str| a.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    let segs: Vec<(Seg, u32)> = [(Seg::Windup, n("windup")), (Seg::Hold, n("hold")), (Seg::Hit, n("hit")), (Seg::Window, n("recover"))]
        .into_iter()
        .filter(|(_, t)| *t > 0)
        .collect();
    if segs.is_empty() {
        return None;
    }
    Some(Strip { name: s(a, "name").to_owned(), clip: s(a, "clip").to_owned(), tell: s(a, "tell").to_owned(), segs })
}

fn attacks(m: &Value) -> Vec<&Value> {
    m.get("attacks").and_then(Value::as_array).map(|v| v.iter().collect()).unwrap_or_default()
}

/// A mob's page at the player's stage (08-guide-book.md, «Ступени записи о враге»).
pub fn mob_page(book: &BookFile, catalog: &Catalog, trophies: &Value, e: &Entry, m: &Value, p: &Progress) -> Page {
    let mp = p.mob(&e.id);
    let stage = book.stage(mp.met, mp.kills, e.elite);
    if stage == book::UNKNOWN {
        return Page {
            title: "???".into(),
            stage,
            lines: vec![line(Tone::Dim, "???"), line(Tone::Dim, "Встретьте этого врага, чтобы узнать о нём.")],
            silhouette: true,
            ..Page::default()
        };
    }
    let mut lines = vec![line(Tone::Dim, format!("Ступень: {} · убито: {}", book.stage_name(stage), num(mp.kills)))];
    if stage < book::EXPERT {
        let left = (book.need(stage + 1, e.elite) - mp.kills).max(0.0);
        lines.push(line(Tone::Dim, format!("До ступени «{}»: ещё {} убийств", book.stage_name(stage + 1), num(left))));
    }
    for k in ["role", "look"] {
        if !s(m, k).is_empty() {
            lines.push(line(Tone::Text, s(m, k)));
        }
    }
    if !s(m, "spawn").is_empty() {
        lines.push(line(Tone::Text, format!("Где водится: {}", s(m, "spawn"))));
    }
    let mut page = Page { title: e.name.clone(), stage, moves: stage >= book::KILLED, ..Page::default() };
    if stage >= book::KILLED {
        stats_lines(m, &mut lines);
    }
    if stage >= book::STUDIED {
        page.strips = attacks(m).into_iter().filter_map(strip).collect();
    }
    if stage >= book::EXPERT {
        expert_lines(catalog, trophies, e, m, &mut lines);
    }
    page.lines = lines;
    page
}

fn stats_lines(m: &Value, lines: &mut Vec<Line>) {
    let st = m.get("stats").cloned().unwrap_or(Value::Null);
    let f = |k: &str| st.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    lines.push(line(Tone::Head, "Числа"));
    lines.push(line(Tone::Text, format!("Здоровье {} · урон {} · броня {}", num(f("health")), num(f("damage")), num(f("armor")))));
    let names: Vec<&str> = attacks(m).into_iter().map(|a| s(a, "name")).filter(|n| !n.is_empty()).collect();
    if !names.is_empty() {
        lines.push(line(Tone::Text, format!("Атаки: {}", names.join(", "))));
    }
}

fn expert_lines(catalog: &Catalog, trophies: &Value, e: &Entry, m: &Value, lines: &mut Vec<Line>) {
    if let Some(b) = m.get("behavior").and_then(Value::as_array).filter(|b| !b.is_empty()) {
        lines.push(line(Tone::Head, "Тактика"));
        lines.extend(b.iter().filter_map(Value::as_str).map(|t| line(Tone::Text, t)));
    }
    let mut loot: Vec<String> = m
        .get("loot")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|x| format!("{} — {}%", s(x, "name"), num(x.get("chance").and_then(Value::as_f64).unwrap_or(0.0) * 100.0))).collect())
        .unwrap_or_default();
    if !e.vanilla {
        let head = &trophies["drops"]["head"];
        let chance = head[if e.elite { "world_elite" } else { "world" }].as_f64().unwrap_or(0.0);
        loot.push(format!("голова — {}%", num(chance * 100.0)));
    }
    if !loot.is_empty() {
        lines.push(line(Tone::Head, "Добыча"));
        lines.push(line(Tone::Text, loot.join(", ")));
    }
    if let Some(stats) = m.get("stats").and_then(|v| serde_json::from_value::<Stats>(v.clone()).ok()) {
        let (a, b) = (Scaled::at(&stats, 0.0), Scaled::at(&stats, 1.0));
        lines.push(line(Tone::Head, "Рост по силе P"));
        lines.push(line(
            Tone::Text,
            format!(
                "От P = 0 до P = 1: здоровье {} → {}, урон {} → {}, броня {} → {}",
                num(a.max_health),
                num(b.max_health),
                num(a.damage),
                num(b.damage),
                num(a.armor),
                num(b.armor)
            ),
        ));
    }
    let events = event_links(catalog, e);
    if !events.is_empty() {
        lines.push(line(Tone::Head, "Ивенты"));
        lines.extend(events.into_iter().map(|t| line(Tone::Text, t)));
    }
}

/// The events whose waves bring this mob: «name: волны 2, 4».
fn event_links(catalog: &Catalog, e: &Entry) -> Vec<String> {
    let mut out = Vec::new();
    for ev in &catalog.events {
        let waves: Vec<String> = ev
            .get("waves")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|w| {
                w.get("mobs").and_then(Value::as_array).is_some_and(|ms| ms.iter().any(|x| e.keys.iter().any(|k| k == s(x, "type"))))
            })
            .map(|w| w.get("wave").and_then(Value::as_u64).unwrap_or(0).to_string())
            .collect();
        if !waves.is_empty() {
            out.push(format!("{}: волны {}", s(ev, "name"), waves.join(", ")));
        }
    }
    out
}

/// A piece's or an item's name for the page (the altar menu's words).
pub fn piece_name(id: &str) -> String {
    match id.trim_start_matches("soulscraft:") {
        "altar_pillar" => "колонна".into(),
        "altar_brazier" => "жаровня".into(),
        "altar_obelisk" => "обелиск".into(),
        "rotten_horn" => "Гнилой горн".into(),
        other => other.to_owned(),
    }
}

fn locked(lines: &mut Vec<Line>, title: &str, when: &str) {
    lines.push(line(Tone::Dim, format!("{title}: ???")));
    if !when.is_empty() {
        lines.push(line(Tone::Dim, format!("  откроется: {when}")));
    }
}

/// The «Алтарь» section: the base, levels 1–4, the banners, «Оборона алтаря».
pub fn altar_page(book: &BookFile, altar: &AltarFile, p: &Progress) -> Page {
    let a = &p.altar;
    let entry = |id: &str| book.altar_entry(id).cloned().unwrap_or_else(|| book::AltarEntryDef { id: id.into(), name: id.into(), when: String::new(), text: String::new() });
    let mut lines = Vec::new();
    let base = entry("altar");
    lines.push(line(Tone::Head, base.name.clone()));
    if a.base {
        lines.push(line(Tone::Text, base.text.clone()));
    } else {
        locked(&mut lines, "Алтарь", &base.when);
    }
    let levels = entry("levels");
    lines.push(line(Tone::Head, levels.name.clone()));
    for l in &altar.levels {
        if a.levels.contains(&l.level) {
            let corner = l.corner.as_deref().map_or(String::new(), |c| format!(", углы: {}", piece_name(c)));
            lines.push(line(Tone::Text, format!("Уровень {}: кольцо {}×{}{corner}. Открывает: {}", l.level, l.size, l.size, l.unlocks)));
        } else {
            locked(&mut lines, &format!("Уровень {}", l.level), if l.level == 1 { &levels.when } else { "" });
        }
    }
    let banners = entry("modifiers");
    lines.push(line(Tone::Head, banners.name.clone()));
    let mut hidden = 0;
    for m in &altar.modifiers {
        if a.banners.contains(&m.id) {
            lines.push(line(Tone::Text, format!("{}: {}", m.name, m.text)));
        } else {
            hidden += 1;
        }
    }
    if hidden > 0 {
        lines.push(line(Tone::Dim, format!("Не найдено знамён: {hidden}. Откроется: {}", banners.when)));
    }
    let defense = entry("defense");
    lines.push(line(Tone::Head, defense.name.clone()));
    if a.defense {
        lines.push(line(Tone::Text, defense.text.clone()));
    } else {
        locked(&mut lines, &defense.name, &defense.when);
    }
    Page { title: "Алтарь".into(), stage: usize::from(a.base), lines, ..Page::default() }
}

/// The «Ивенты» section: each event's description after a try, its waves and reward after a win.
pub fn events_page(book: &BookFile, catalog: &Catalog, trophies: &Value, p: &Progress) -> Page {
    let when = book.altar_entry("events").map(|e| e.when.clone()).unwrap_or_default();
    let mut lines = Vec::new();
    for ev in &catalog.events {
        let id = s(ev, "id");
        let got = p.events.get(id).copied().unwrap_or_default();
        if !got.tried {
            locked(&mut lines, "Ивент", &when);
            continue;
        }
        lines.push(line(Tone::Head, s(ev, "name")));
        let tier = ev.get("tier").and_then(Value::as_u64).unwrap_or(1);
        let item = piece_name(s(ev, "item"));
        lines.push(line(Tone::Text, format!("Алтарь уровня {tier} и выше; призыв: {item} на подставке.")));
        if !s(ev, "target").is_empty() {
            lines.push(line(Tone::Text, format!("Цель: {}", s(ev, "target"))));
        }
        if !got.won {
            lines.push(line(Tone::Dim, "Волны и награда откроются после первой победы."));
            continue;
        }
        for w in ev.get("waves").and_then(Value::as_array).into_iter().flatten() {
            let mobs: Vec<String> = w
                .get("mobs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|x| {
                    let kind = s(x, "type");
                    let name = catalog.by_key(kind).map_or(kind.to_owned(), |e| e.name.clone());
                    format!("{} × {name}", x.get("count").and_then(Value::as_u64).unwrap_or(0))
                })
                .collect();
            lines.push(line(Tone::Text, format!("Волна {}: {}", w.get("wave").and_then(Value::as_u64).unwrap_or(0), mobs.join(", "))));
        }
        let d = &trophies["drops"];
        lines.push(line(Tone::Text, format!("Награда: {}; части брони — {}%; {}", s(&d["head"], "event"), num(d["armor"]["event"].as_f64().unwrap_or(0.0) * 100.0), s(&d["diorama"], "event"))));
    }
    Page { title: "Ивенты".into(), lines, ..Page::default() }
}
