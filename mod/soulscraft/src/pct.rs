//! Path-of-craft-tree's combat summary of a player, as PCT publishes it for other mods
//! (`pct:summary` in the player's persistent data, PCT v0.4.4; integration.md 3.1):
//!
//! ```json
//! {"v":1,"levels":{"archery":[L,75],"defense":[L,75],"melee":[L,75]},
//!  "ranks":{"archery":[R,Rmax],"defense":[R,Rmax],"melee":[R,Rmax]}}
//! ```
//!
//! It comes as a string holding that JSON, or as a compound with the same fields: both read.
//! `levels` grow with experience; `ranks` are the working node ranks of the branch (switched-off
//! big nodes don't count) with all the ranks it has. PCT only adds fields and raises `v` only on
//! a change readers must know about, so unknown fields are ignored and another `v` is refused.
//! A missing or refused summary means gear-only power (Г1 with `T = 0`).

use serde_json::Value;

/// The persistent-data key PCT writes.
pub const KEY: &str = "pct:summary";
/// The summary shape this reader knows.
pub const VERSION: u64 = 1;
/// The combat branches, in the order [`Summary`] keeps them.
pub const BRANCHES: [&str; 3] = ["melee", "archery", "defense"];

/// A value and its highest, `[value, highest]` in the summary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pair {
    pub value: f64,
    pub max: f64,
}

impl Pair {
    /// The value as a share of its highest, 0 to 1; 0 when the highest is 0.
    pub fn share(self) -> f64 {
        if self.max > 0.0 { (self.value / self.max).clamp(0.0, 1.0) } else { 0.0 }
    }
}

/// One player's combat summary: per branch of [`BRANCHES`], the level and the working ranks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub levels: [Pair; 3],
    /// None when the summary has no `ranks` (a reader of v1 accepts its absence).
    pub ranks: Option<[Pair; 3]>,
}

/// Reads the summary from what the key holds: a JSON string, or the same as a JSON object
/// (a compound turned into JSON).
pub fn parse(held: &Value) -> Result<Summary, String> {
    match held {
        Value::String(text) => {
            let v: Value = serde_json::from_str(text).map_err(|e| format!("{KEY}: {e}"))?;
            parse_object(&v)
        }
        v @ Value::Object(_) => parse_object(v),
        other => Err(format!("{KEY}: neither a JSON string nor a compound: {other}")),
    }
}

fn parse_object(v: &Value) -> Result<Summary, String> {
    let version = v.get("v").and_then(Value::as_u64).ok_or(format!("{KEY}: no version v"))?;
    if version != VERSION {
        return Err(format!("{KEY}: version {version}, this reader knows {VERSION}"));
    }
    let levels = pairs(v.get("levels").ok_or(format!("{KEY}: no levels"))?, "levels")?;
    let ranks = v.get("ranks").map(|r| pairs(r, "ranks")).transpose()?;
    Ok(Summary { levels, ranks })
}

fn pairs(v: &Value, field: &str) -> Result<[Pair; 3], String> {
    let mut out = [Pair::default(); 3];
    for (i, branch) in BRANCHES.iter().enumerate() {
        let p = v.get(branch).ok_or(format!("{KEY}: {field} has no {branch}"))?;
        // A compound turned into JSON may hold the pair as an int array: the same two numbers.
        let list = p.as_array().filter(|l| l.len() == 2).ok_or(format!("{KEY}: {field}.{branch} is not [value, highest]"))?;
        let num = |x: &Value| x.as_f64().filter(|n| n.is_finite() && *n >= 0.0);
        match (num(&list[0]), num(&list[1])) {
            (Some(value), Some(max)) => out[i] = Pair { value, max },
            _ => return Err(format!("{KEY}: {field}.{branch} holds no two numbers of 0 or more")),
        }
    }
    Ok(out)
}

/// What one read of a player's persistent data gave.
#[derive(Clone, Debug, PartialEq)]
pub enum Read {
    /// The engine could not reach the player now (`native_player_persistent_data` is None:
    /// offline, or another player's action is running, `native_place`): keep what was known.
    Unreachable,
    /// The compound has no `pct:summary`: no PCT, or it has not written yet. Gear only.
    Absent,
    /// What the key holds, as JSON (a string, or a compound turned into an object).
    Held(Value),
}

/// The summaries of the players, read on the server's own tick and kept, so a power asked for
/// anywhere (a spawn inside some player's action included) never reads the engine.
#[derive(Clone, Debug, Default)]
pub struct Cache {
    known: std::collections::HashMap<u128, Option<Summary>>,
}

impl Cache {
    /// Takes one read for `player`. Returns `Some(new)` when what is known changed (for the log),
    /// with the parse error of a summary that does not read (counted as absent).
    pub fn update(&mut self, player: u128, read: Read) -> Option<(Option<Summary>, Option<String>)> {
        let (now, error) = match read {
            Read::Unreachable => return None,
            Read::Absent => (None, None),
            Read::Held(v) => match parse(&v) {
                Ok(s) => (Some(s), None),
                Err(e) => (None, Some(e)),
            },
        };
        let before = self.known.insert(player, now);
        (before != Some(now) || error.is_some()).then_some((now, error))
    }

    /// The last summary read for `player`; None for none (gear only).
    pub fn get(&self, player: u128) -> Option<Summary> {
        self.known.get(&player).copied().flatten()
    }

    /// Whether the player was ever read (a new player is read at once, not on the next round).
    pub fn has(&self, player: u128) -> bool {
        self.known.contains_key(&player)
    }

    /// Forgets the players not in `online`.
    pub fn keep_only(&mut self, online: &[u128]) {
        self.known.retain(|p, _| online.contains(p));
    }
}

/// A v1 summary as PCT writes it (a test fixture, `/sc pct`): the levels of melee, archery and
/// defense of 75, and as many working ranks of `ranks_max`.
pub fn fixture(levels: [u32; 3], ranks_max: u32) -> String {
    let [m, a, d] = levels;
    format!(
        r#"{{"levels":{{"archery":[{a},75],"defense":[{d},75],"melee":[{m},75]}},"ranks":{{"archery":[{a},{ranks_max}],"defense":[{d},{ranks_max}],"melee":[{m},{ranks_max}]}},"v":1}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_unreachable_read_keeps_the_cached_summary() {
        let mut c = Cache::default();
        assert!(!c.has(7));
        let changed = c.update(7, Read::Held(Value::String(FIXTURE.into())));
        assert!(changed.is_some_and(|(s, e)| s.is_some() && e.is_none()));
        let known = c.get(7).unwrap();
        // Inside another player's action the engine answers None: the summary stays, not gear only.
        assert_eq!(c.update(7, Read::Unreachable), None);
        assert_eq!(c.get(7), Some(known));
        // The same summary again is no change.
        assert_eq!(c.update(7, Read::Held(Value::String(FIXTURE.into()))), None);
        // A player without PCT's key: gear only.
        assert_eq!(c.update(7, Read::Absent), Some((None, None)));
        assert_eq!(c.get(7), None);
        assert!(c.has(7));
        // A summary that does not read counts as absent, with the reason.
        let bad = c.update(7, Read::Held(json!({"v": 9})));
        assert!(bad.is_some_and(|(s, e)| s.is_none() && e.unwrap().contains("version 9")));
        c.keep_only(&[]);
        assert!(!c.has(7));
    }

    #[test]
    fn the_fixture_reads_as_pct_writes() {
        let s = parse(&Value::String(fixture([60, 12, 30], 96))).unwrap();
        assert_eq!(s.levels[0], Pair { value: 60.0, max: 75.0 });
        assert_eq!(s.ranks.unwrap()[2], Pair { value: 30.0, max: 96.0 });
    }

    const FIXTURE: &str =
        r#"{"levels":{"archery":[12,75],"defense":[30,75],"melee":[60,75]},"ranks":{"archery":[3,90],"defense":[20,88],"melee":[45,96]},"v":1}"#;

    #[test]
    fn reads_the_string_pct_writes() {
        let s = parse(&Value::String(FIXTURE.into())).unwrap();
        assert_eq!(s.levels[0], Pair { value: 60.0, max: 75.0 });
        assert_eq!(s.levels[1].value, 12.0);
        assert_eq!(s.levels[2].value, 30.0);
        assert_eq!(s.ranks.unwrap()[0], Pair { value: 45.0, max: 96.0 });
    }

    #[test]
    fn reads_the_same_as_a_compound() {
        let v: Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(parse(&v).unwrap(), parse(&Value::String(FIXTURE.into())).unwrap());
    }

    #[test]
    fn ignores_fields_it_does_not_know() {
        let v = json!({"v":1,"total":9,"levels":{"melee":[1,75],"archery":[2,75],"defense":[3,75],"magic":[4,75]}});
        let s = parse(&v).unwrap();
        assert_eq!(s.levels[2].value, 3.0);
        assert!(s.ranks.is_none());
    }

    #[test]
    fn refuses_another_version_and_broken_summaries() {
        let v2 = FIXTURE.replace("\"v\":1", "\"v\":2");
        assert!(parse(&Value::String(v2)).unwrap_err().contains("version 2"));
        assert!(parse(&Value::String("{".into())).is_err());
        assert!(parse(&json!(5)).unwrap_err().contains("neither"));
        assert!(parse(&json!({"v":1})).unwrap_err().contains("no levels"));
        let short = json!({"v":1,"levels":{"melee":[1],"archery":[2,75],"defense":[3,75]}});
        assert!(parse(&short).unwrap_err().contains("melee"));
        let negative = json!({"v":1,"levels":{"melee":[-1,75],"archery":[2,75],"defense":[3,75]}});
        assert!(parse(&negative).is_err());
    }

    #[test]
    fn shares_are_clamped() {
        assert_eq!(Pair { value: 90.0, max: 75.0 }.share(), 1.0);
        assert_eq!(Pair { value: 5.0, max: 0.0 }.share(), 0.0);
        assert_eq!(Pair { value: 15.0, max: 75.0 }.share(), 0.2);
    }
}
