// The player power P (hypothesis Г1, weights in data/mobs.json `power`): gear from the engine
// (armour attribute, weapon in hand) and Path-of-craft-tree's combat summary from the player's
// persistent data (`pct:summary`, soulscraft_rules::pct).
//
// The persistent data is one compound per player that every mod shares (NeoForgeData;
// `native_player_persistent_data`, game/native_player_state.rs:381-392), so this bundle reads
// PCT's key without depending on PCT. The engine answers only for a player it can reach
// (`native_place`): inside one player's action the others read None. So the summaries are read on
// the server's own level tick, every READ_EVERY ticks and at once for a player not read yet, and
// kept in a cache; a None read keeps what was known, and P anywhere is computed from the cache.
use crate::game::Game;
use crate::mod_api::{NativeServerPlayer, Tag};
use serde_json::Value;
use soulscraft_rules::{
    data::PowerDef,
    geom::V3,
    pct::{self, Cache, Read, Summary},
    scale::{self, PowerInputs},
};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// How often each online player's summary is read again (PCT writes it on its own changes).
const READ_EVERY: u64 = 20;

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

fn cache() -> MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The level tick: reads the summaries due. `online` is every player of the server.
pub(super) fn refresh(game: &Game<'_>, online: &[NativeServerPlayer], now: u64) {
    let uuids: Vec<u128> = online.iter().map(|p| p.uuid).collect();
    let due: Vec<&NativeServerPlayer> = {
        let mut c = cache();
        c.keep_only(&uuids);
        online.iter().filter(|p| now % READ_EVERY == 0 || !c.has(p.uuid)).collect()
    };
    for p in due {
        let read = read_summary(game, p.uuid);
        let changed = cache().update(p.uuid, read);
        if let Some((summary, error)) = changed {
            let line = match (&summary, &error) {
                (_, Some(e)) => format!("unreadable, gear only: {e}"),
                (Some(s), None) => describe(s),
                (None, None) => "none, gear only".to_owned(),
            };
            log::info!("soulscraft: pct player={} summary {line} tick={now}", p.name);
        }
    }
}

fn read_summary(game: &Game<'_>, uuid: u128) -> Read {
    let Some(data) = game.native_player_persistent_data(uuid) else { return Read::Unreachable };
    match data.get(pct::KEY) {
        None => Read::Absent,
        Some(tag) => Read::Held(to_json(tag)),
    }
}

/// An NBT tag as JSON: a string stays a string (PCT's JSON text), a compound becomes an object,
/// numbers and lists their JSON forms.
fn to_json(tag: &Tag) -> Value {
    match tag {
        Tag::End => Value::Null,
        Tag::Byte(v) => Value::from(*v),
        Tag::Short(v) => Value::from(*v),
        Tag::Int(v) => Value::from(*v),
        Tag::Long(v) => Value::from(*v),
        Tag::Float(bits) => Value::from(f64::from(f32::from_bits(*bits))),
        Tag::Double(bits) => Value::from(f64::from_bits(*bits)),
        Tag::ByteArray(v) => Value::from(v.iter().map(|b| i64::from(*b as i8)).collect::<Vec<_>>()),
        Tag::String(s) => Value::String(s.to_string_lossy()),
        Tag::List(v) => Value::Array(v.iter().map(to_json).collect()),
        Tag::Compound(c) => Value::Object(c.iter().map(|(k, v)| (k.to_string_lossy(), to_json(v))).collect()),
        Tag::IntArray(v) => Value::from(v.clone()),
        Tag::LongArray(v) => Value::from(v.clone()),
    }
}

fn describe(s: &Summary) -> String {
    let mut out = Vec::new();
    for (i, b) in pct::BRANCHES.iter().enumerate() {
        let l = s.levels[i];
        let r = s.ranks.map_or(String::new(), |r| format!(" ranks {}/{}", r[i].value, r[i].max));
        out.push(format!("{b} {}/{}{r}", l.value, l.max));
    }
    out.join(", ")
}

/// What a player's P is made of, from the cache and their gear now.
pub(super) fn inputs(game: &mut Game<'_>, uuid: u128) -> PowerInputs {
    let armor = game.native_attribute(uuid, "minecraft:armor").map_or(0.0, |(_, v)| v);
    let weapon = game.lan.as_ref().and_then(|h| {
        let g = h.hub.guests.iter().find(|g| g.uuid == uuid)?;
        g.inventory.get(g.selected)?.as_ref().map(|s| s.item.to_string())
    });
    PowerInputs { tree: cache().get(uuid), armor, weapon_tier: scale::weapon_tier(weapon.as_deref()) }
}

/// Г1 for one player.
pub(super) fn player_power(game: &mut Game<'_>, uuid: u128, w: &PowerDef) -> f64 {
    scale::player_power(&inputs(game, uuid), w)
}

/// The power a mob takes where it stands: the `/sc p` override, else the players near it (Г5).
pub(super) fn power_here(game: &mut Game<'_>, at: V3, players: &[V3U]) -> f64 {
    let (over, w) = {
        let s = crate::game::soulscraft_server::state();
        (s.p_override, s.data.power)
    };
    if let Some(p) = over {
        return p;
    }
    let powers: Vec<(V3, f64)> = players.iter().map(|(pos, uuid)| (*pos, player_power(game, *uuid, &w))).collect();
    scale::group_power(at, &powers).unwrap_or(0.0)
}

/// A player as P needs them: where they stand and who they are.
pub(super) type V3U = (V3, u128);

/// `/sc power`: each online player's P, with its tree and gear parts.
pub(super) fn report(game: &mut Game<'_>) -> Vec<String> {
    let w = crate::game::soulscraft_server::state().data.power;
    let players = game.native_server_players();
    let mut lines = Vec::new();
    for p in &players {
        let i = inputs(game, p.uuid);
        let t = i.tree.as_ref().map(|s| scale::tree_part(s, &w));
        let g = scale::gear_part(&i, &w);
        let tree = match (t, &i.tree) {
            (Some(t), Some(s)) => format!("T {t:.2} ({})", describe(s)),
            _ => "T 0 (no pct:summary: gear only)".to_owned(),
        };
        lines.push(format!(
            "{}: P {:.2} = {:.2}·{tree} + {:.2}·G {g:.2} (armour {:.0}, weapon tier {})",
            p.name,
            scale::player_power(&i, &w),
            w.tree_share,
            1.0 - w.tree_share,
            i.armor,
            i.weapon_tier
        ));
    }
    if lines.is_empty() {
        lines.push("no players online".to_owned());
    }
    lines
}

/// `/sc pct <player> …`: writes (or takes away) a test summary in PCT's key on that player, in
/// place in the shared compound (the other mods' keys stay). For bots only: PCT itself writes
/// over it on its next change.
pub(super) fn set_fixture(game: &mut Game<'_>, name: &str, levels: Option<[u32; 3]>) -> Result<String, String> {
    let p = game
        .native_server_players()
        .into_iter()
        .find(|p| p.name == name)
        .ok_or(format!("no player {name} online"))?;
    let mut data = game.native_player_persistent_data(p.uuid).ok_or("the player cannot be reached now")?;
    let Tag::Compound(map) = &mut data else { return Err("persistent data is not a compound".into()) };
    match levels {
        Some(l) => {
            map.insert(pct::KEY.into(), Tag::String(pct::fixture(l, 96).as_str().into()));
        }
        None => {
            map.remove(&horizon_world::nbt::JavaString::from(pct::KEY));
        }
    }
    if !game.native_set_player_persistent_data(p.uuid, data) {
        return Err("not written".into());
    }
    Ok(match levels {
        Some(l) => format!("{name}: pct:summary melee {} archery {} defense {} (read within {READ_EVERY} ticks)", l[0], l[1], l[2]),
        None => format!("{name}: pct:summary taken away"),
    })
}
