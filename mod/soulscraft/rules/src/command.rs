//! The operator's test commands, `/sc …`, and the bench's self-test console command. Parsed here;
//! the server half carries them out.

/// The player command's word.
pub const SC: &str = "sc";
/// The console command the joint bench sends (`.\joint.ps1 check`, mods.json `selftest`).
pub const SELFTEST: &str = "soulscraft_selftest";

/// Usages for `native_register_commands` (operators of level 2 and up).
pub const USAGES: &[&str] = &[
    "sc spawn <mob:word> [p:number] [count:integer]",
    "sc p <value:word>",
    "sc reload",
    "sc debug <on:word>",
    "sc slowmo <rate:word>",
    "sc freeze <on:word>",
    "sc arena",
    "sc wave <action:word>",
    "sc force <attack:word>",
    "sc clear",
    "sc info",
    "sc bench <count:integer> [mob:word]",
    "sc power",
    "sc pct <player:word> [levels:text]",
    "sc cost <action:word>",
    "sc altar <level:integer> [banners:word]",
    "sc horn",
    "sc event <action:word>",
];

/// The most mobs one `/sc spawn` or `/sc bench` makes.
pub const MAX_COUNT: u32 = 64;

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// Spawn `count` of `mob` in front of the player, at power `p` (else the player's own).
    Spawn { mob: String, p: Option<f64>, count: u32 },
    /// Override every player's power P for new spawns; None: back to the real one.
    SetP(Option<f64>),
    /// Read mobs.json again from the config folder (numbers and timings live).
    Reload,
    Debug(bool),
    /// `/tick rate`: 20 is normal speed; None back to 20.
    Slowmo(Option<f64>),
    Freeze(bool),
    /// Build the flat lit test arena in SoulsCraft's area.
    Arena,
    Wave(WaveAction),
    /// The nearest SoulsCraft mob starts this attack now.
    Force(String),
    /// Remove the SoulsCraft mobs near the player.
    Clear,
    Info,
    /// П5: a ring of `count` mobs round the player: revenants, one kind, or `mixed` (every kind
    /// in turn).
    Bench { count: u32, mob: Option<String> },
    /// Each online player's power P and what it is made of (Г1).
    Power,
    /// Test fixture: writes a v1 `pct:summary` on `player` (levels of melee, archery, defense),
    /// as Path-of-craft-tree would; None takes it away.
    Pct { player: String, levels: Option<[u32; 3]> },
    /// П5: mob cost sampling on, or off with a report.
    Cost(bool),
    /// Build an altar of this level (0 to 4) six blocks ahead, with the sample banners on its sockets.
    Altar { level: u32, banners: bool },
    /// A Rotten Horn (the test event's summon item) into the player's inventory.
    Horn,
    /// Start (true) or stop the test event on the nearest altar.
    Event(bool),
    /// The self-test: run the scenarios whose name contains the filter.
    SelfTest(Option<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveAction {
    Start,
    Next,
    Stop,
}

fn on_off(w: Option<&str>) -> Result<bool, String> {
    match w {
        Some("on" | "true" | "1") => Ok(true),
        Some("off" | "false" | "0") => Ok(false),
        other => Err(format!("on or off, not {}", other.unwrap_or("nothing"))),
    }
}

fn number(w: &str) -> Result<f64, String> {
    w.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("not a number: {w}"))
}

/// Parses a command line without its slash: `sc spawn revenant 0.4 3`,
/// `soulscraft_selftest run attack`. None for a line that is not ours.
pub fn parse(line: &str) -> Option<Result<Cmd, String>> {
    let mut w = line.split_whitespace();
    match w.next()? {
        SC => Some(parse_sc(w.collect())),
        SELFTEST => Some(match w.next() {
            Some("run") => Ok(Cmd::SelfTest(w.next().map(str::to_owned))),
            _ => Err(format!("{SELFTEST} run [filter]")),
        }),
        _ => None,
    }
}

fn parse_sc(w: Vec<&str>) -> Result<Cmd, String> {
    let arg = |i: usize| w.get(i).copied();
    match arg(0) {
        Some("spawn") => {
            let mob = arg(1).ok_or("sc spawn <mob> [p] [count]")?.to_owned();
            let p = arg(2).map(number).transpose()?;
            let count = match arg(3) {
                Some(c) => c.parse::<u32>().map_err(|_| format!("not a count: {c}"))?,
                None => 1,
            };
            Ok(Cmd::Spawn { mob, p, count: count.clamp(1, MAX_COUNT) })
        }
        Some("p") => match arg(1) {
            Some("off" | "clear") => Ok(Cmd::SetP(None)),
            Some(v) => Ok(Cmd::SetP(Some(number(v)?))),
            None => Err("sc p <0..1|off>".into()),
        },
        Some("reload") => Ok(Cmd::Reload),
        Some("debug") => Ok(Cmd::Debug(on_off(arg(1))?)),
        Some("slowmo") => match arg(1) {
            Some("off") | None => Ok(Cmd::Slowmo(None)),
            Some(v) => {
                let r = number(v)?;
                if (1.0..=20.0).contains(&r) { Ok(Cmd::Slowmo(Some(r))) } else { Err("a rate from 1 to 20".into()) }
            }
        },
        Some("freeze") => Ok(Cmd::Freeze(on_off(arg(1).or(Some("on")))?)),
        Some("arena") => Ok(Cmd::Arena),
        Some("wave") => match arg(1) {
            Some("start") | None => Ok(Cmd::Wave(WaveAction::Start)),
            Some("next") => Ok(Cmd::Wave(WaveAction::Next)),
            Some("stop") => Ok(Cmd::Wave(WaveAction::Stop)),
            Some(x) => Err(format!("sc wave start|next|stop, not {x}")),
        },
        Some("force") => Ok(Cmd::Force(arg(1).ok_or("sc force <attack>")?.to_owned())),
        Some("clear") => Ok(Cmd::Clear),
        Some("info") => Ok(Cmd::Info),
        Some("bench") => {
            let n = arg(1).unwrap_or("50");
            let n = n.parse::<u32>().map_err(|_| format!("not a count: {n}"))?;
            Ok(Cmd::Bench { count: n.clamp(1, MAX_COUNT), mob: arg(2).map(str::to_owned) })
        }
        Some("power") => Ok(Cmd::Power),
        Some("pct") => {
            let player = arg(1).ok_or("sc pct <player> <melee> <archery> <defense>|off")?.to_owned();
            let levels = match arg(2) {
                Some("off") => None,
                _ => {
                    let lv = |i: usize| -> Result<u32, String> {
                        let w = arg(i).ok_or("sc pct <player> <melee> <archery> <defense>|off")?;
                        w.parse::<u32>().ok().filter(|l| *l <= 75).ok_or(format!("a level from 0 to 75, not {w}"))
                    };
                    Some([lv(2)?, lv(3)?, lv(4)?])
                }
            };
            Ok(Cmd::Pct { player, levels })
        }
        Some("altar") => {
            let w = arg(1).ok_or("sc altar <0..4> [banners]")?;
            let level = w.parse::<u32>().ok().filter(|l| *l <= 4).ok_or(format!("a level from 0 to 4, not {w}"))?;
            Ok(Cmd::Altar { level, banners: arg(2) == Some("banners") })
        }
        Some("horn") => Ok(Cmd::Horn),
        Some("event") => match arg(1) {
            Some("start") | None => Ok(Cmd::Event(true)),
            Some("stop") => Ok(Cmd::Event(false)),
            Some(x) => Err(format!("sc event start|stop, not {x}")),
        },
        Some("cost") => Ok(Cmd::Cost(on_off(arg(1).map(|a| match a {
            "start" => "on",
            "stop" => "off",
            x => x,
        }))?)),
        Some(x) => Err(format!("unknown: sc {x}")),
        None => Err("sc spawn|p|reload|debug|slowmo|freeze|arena|wave|force|clear|info|bench|cost|power|pct|altar|horn|event".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(line: &str) -> Cmd {
        parse(line).unwrap().unwrap()
    }

    #[test]
    fn spawn_forms() {
        assert_eq!(ok("sc spawn revenant"), Cmd::Spawn { mob: "revenant".into(), p: None, count: 1 });
        assert_eq!(ok("sc spawn brute 0,5 3"), Cmd::Spawn { mob: "brute".into(), p: Some(0.5), count: 3 });
        assert_eq!(ok("sc spawn brute 1 999"), Cmd::Spawn { mob: "brute".into(), p: Some(1.0), count: MAX_COUNT });
        assert!(parse("sc spawn").unwrap().is_err());
        assert!(parse("sc spawn x y").unwrap().is_err());
        assert!(parse("sc spawn x 1 -2").unwrap().is_err());
    }

    #[test]
    fn the_other_commands() {
        assert_eq!(ok("sc p 0.4"), Cmd::SetP(Some(0.4)));
        assert_eq!(ok("sc p off"), Cmd::SetP(None));
        assert!(parse("sc p").unwrap().is_err());
        assert_eq!(ok("sc reload"), Cmd::Reload);
        assert_eq!(ok("sc debug on"), Cmd::Debug(true));
        assert_eq!(ok("sc debug off"), Cmd::Debug(false));
        assert!(parse("sc debug maybe").unwrap().is_err());
        assert_eq!(ok("sc slowmo 5"), Cmd::Slowmo(Some(5.0)));
        assert_eq!(ok("sc slowmo off"), Cmd::Slowmo(None));
        assert!(parse("sc slowmo 50").unwrap().is_err());
        assert_eq!(ok("sc freeze"), Cmd::Freeze(true));
        assert_eq!(ok("sc freeze off"), Cmd::Freeze(false));
        assert_eq!(ok("sc arena"), Cmd::Arena);
        assert_eq!(ok("sc wave"), Cmd::Wave(WaveAction::Start));
        assert_eq!(ok("sc wave next"), Cmd::Wave(WaveAction::Next));
        assert_eq!(ok("sc wave stop"), Cmd::Wave(WaveAction::Stop));
        assert!(parse("sc wave fly").unwrap().is_err());
        assert_eq!(ok("sc force overhead"), Cmd::Force("overhead".into()));
        assert_eq!(ok("sc clear"), Cmd::Clear);
        assert_eq!(ok("sc info"), Cmd::Info);
        assert_eq!(ok("sc bench"), Cmd::Bench { count: 50, mob: None });
        assert_eq!(ok("sc bench 20"), Cmd::Bench { count: 20, mob: None });
        assert_eq!(ok("sc bench 49 mixed"), Cmd::Bench { count: 49, mob: Some("mixed".into()) });
        assert_eq!(ok("sc power"), Cmd::Power);
        assert_eq!(ok("sc pct sc_a 60 12 30"), Cmd::Pct { player: "sc_a".into(), levels: Some([60, 12, 30]) });
        assert_eq!(ok("sc pct sc_a off"), Cmd::Pct { player: "sc_a".into(), levels: None });
        assert!(parse("sc pct sc_a 60 12").unwrap().is_err());
        assert!(parse("sc pct sc_a 60 12 99").unwrap().is_err());
        assert_eq!(ok("sc cost start"), Cmd::Cost(true));
        assert_eq!(ok("sc cost stop"), Cmd::Cost(false));
        assert!(parse("sc").unwrap().is_err());
        assert!(parse("sc dance").unwrap().is_err());
    }

    #[test]
    fn selftest_and_foreign_lines() {
        assert_eq!(ok("soulscraft_selftest run"), Cmd::SelfTest(None));
        assert_eq!(ok("soulscraft_selftest run timing"), Cmd::SelfTest(Some("timing".into())));
        assert!(parse("soulscraft_selftest").unwrap().is_err());
        assert_eq!(parse("tree"), None);
        assert_eq!(parse(""), None);
        assert_eq!(USAGES.len(), 18);
        assert_eq!(ok("sc altar 3 banners"), Cmd::Altar { level: 3, banners: true });
        assert_eq!(ok("sc altar 0"), Cmd::Altar { level: 0, banners: false });
        assert!(parse("sc altar 5").unwrap().is_err());
        assert_eq!(ok("sc horn"), Cmd::Horn);
        assert_eq!(ok("sc event"), Cmd::Event(true));
        assert_eq!(ok("sc event stop"), Cmd::Event(false));
        assert!(parse("sc event go").unwrap().is_err());
    }
}
