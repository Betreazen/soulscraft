//! What the server tells clients about altars, and the presses a client sends back, as JSON in
//! mod messages (API 40): the ghost of the next ring, the event's beams and spawn points, and the
//! altar's menu. The client only draws what it was last told; nothing waits for an answer, so a
//! LetsCam replay draws the same.

use serde::{Deserialize, Serialize};

/// Server → client: an altar's [`View`].
pub const ALTAR: &str = "soulscraft:altar";
/// Server → client: open (or refresh) the altar's [`Menu`].
pub const MENU: &str = "soulscraft:menu";
/// Client → server: a [`Press`] on the menu.
pub const PRESS: &str = "soulscraft:press";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GhostCell {
    pub pos: [i32; 3],
    pub block: String,
    #[serde(default)]
    pub part: Option<String>,
}

/// A beam from a wave mob to the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamKind {
    /// The drain's windup: thin, a hit stops it.
    Windup,
    Drain,
    /// A walled-out mob's curse, seen through blocks.
    Curse,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub from: [f64; 3],
    pub kind: BeamKind,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventView {
    /// The wave in progress or coming, from 1.
    pub wave: u32,
    pub waves: u32,
    pub hp: f64,
    pub max: f64,
    /// The altar lost health in the last second.
    pub losing: bool,
    /// `telegraph`, `fight`, `rest`.
    pub phase: String,
    /// Where the coming wave appears.
    #[serde(default)]
    pub spawns: Vec<[i32; 3]>,
    #[serde(default)]
    pub beams: Vec<Beam>,
}

/// One altar as clients see it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub core: [i32; 3],
    pub level: u32,
    /// The cells the next ring still needs.
    #[serde(default)]
    pub ghost: Vec<GhostCell>,
    /// Cells of the next ring with a wrong block (drawn red).
    #[serde(default)]
    pub wrong: Vec<[i32; 3]>,
    #[serde(default)]
    pub event: Option<EventView>,
}

/// The altar's menu (06-altar.md, «Меню алтаря»): lines to show and the spawn boost's switch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Menu {
    pub core: [i32; 3],
    pub level: u32,
    pub sky: bool,
    /// What the next level needs, a line per block kind.
    #[serde(default)]
    pub missing: Vec<String>,
    /// The banners standing, by name.
    #[serde(default)]
    pub banners: Vec<String>,
    /// Their sum, as lines.
    #[serde(default)]
    pub effects: Vec<String>,
    /// The spawn boost is on.
    pub boost: bool,
    /// The event tiers this altar runs, or why none.
    #[serde(default)]
    pub unlocks: Vec<String>,
    /// The event in progress: wave, altar health, participants.
    #[serde(default)]
    pub event: Vec<String>,
}

/// A button of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Button {
    BoostOn,
    BoostOff,
    /// The menu was closed: no more refreshes.
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Press {
    pub core: [i32; 3],
    pub button: Button,
}

pub fn encode<T: Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    serde_json::from_slice(bytes).ok()
}

/// The menu's text, one line each, top to bottom (the client draws them and the buttons).
pub fn menu_lines(m: &Menu) -> Vec<String> {
    let mut out = vec![format!("Уровень алтаря: {}", m.level)];
    if !m.sky {
        out.push("Алтарю нужно небо: над ядром не должно быть крыши".into());
    }
    if m.missing.is_empty() {
        if m.level >= 4 {
            out.push("Алтарь достроен".into());
        }
    } else {
        out.push(format!("До уровня {}:", m.level + 1));
        out.extend(m.missing.iter().map(|l| format!("  {l}")));
    }
    if m.banners.is_empty() {
        out.push("Знамён нет".into());
    } else {
        out.push(format!("Знамёна: {}", m.banners.join(", ")));
        out.extend(m.effects.iter().map(|l| format!("  {l}")));
    }
    out.push(format!("Учащённый спавн: {}", if m.boost { "включён" } else { "выключен" }));
    if m.event.is_empty() {
        out.extend(m.unlocks.iter().cloned());
    } else {
        out.extend(m.event.iter().cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let v = View {
            core: [1, 2, 3],
            level: 2,
            ghost: vec![GhostCell { pos: [3, 2, 3], block: "soulscraft:altar_slab".into(), part: None }],
            wrong: vec![[4, 2, 3]],
            event: Some(EventView {
                wave: 2,
                waves: 5,
                hp: 150.0,
                max: 200.0,
                losing: true,
                phase: "fight".into(),
                spawns: vec![[10, 2, 3]],
                beams: vec![Beam { from: [5.0, 3.0, 3.0], kind: BeamKind::Curse }],
            }),
        };
        assert_eq!(decode::<View>(&encode(&v)), Some(v));
        let p = Press { core: [1, 2, 3], button: Button::BoostOff };
        assert_eq!(decode::<Press>(&encode(&p)), Some(p));
        assert_eq!(decode::<Press>(b"nonsense"), None);
    }

    #[test]
    fn menu_lines_say_what_is_missing_and_the_event() {
        let m = Menu {
            core: [0; 3],
            level: 1,
            sky: false,
            missing: vec!["плита алтаря ×3".into()],
            banners: vec![],
            effects: vec![],
            boost: true,
            unlocks: vec!["Ивенты первой ступени".into()],
            event: vec![],
        };
        let l = menu_lines(&m);
        assert_eq!(l[0], "Уровень алтаря: 1");
        assert!(l.iter().any(|x| x.contains("нужно небо")));
        assert!(l.iter().any(|x| x == "До уровня 2:"));
        assert!(l.iter().any(|x| x == "Знамён нет"));
        assert!(l.iter().any(|x| x == "Учащённый спавн: включён"));
        assert!(l.iter().any(|x| x == "Ивенты первой ступени"));
        let m = Menu {
            level: 4,
            sky: true,
            missing: vec![],
            banners: vec!["Знамя орды".into()],
            effects: vec!["врагов в волне +25%".into()],
            event: vec!["Волна 2 из 5".into()],
            ..m
        };
        let l = menu_lines(&m);
        assert!(l.iter().any(|x| x == "Алтарь достроен"));
        assert!(l.iter().any(|x| x == "Знамёна: Знамя орды"));
        assert!(l.iter().any(|x| x == "Волна 2 из 5"));
        assert!(!l.iter().any(|x| x.contains("первой ступени")));
    }
}
