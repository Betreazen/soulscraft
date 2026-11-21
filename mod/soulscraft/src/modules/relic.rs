// The tier items' bonuses the engine doesn't apply itself (docs/design/11-summons-rewards.md, Г19).
//
// The Abyss Heart gives +4 max health while it is in the off hand. Horizon 7e4e421 applies no
// attribute modifier of an off-hand item and no max_health modifier of any item (checked on the
// bench 2026-11-16: a stick with +3 armour in the off hand gives 0, a chestplate with +4 max health
// gives 20), while the `attribute` command works. So twice a second the server reads each player's
// off hand (`native_player_equipment`, game/native_player_reads.rs:185; until 0.6.2 a command per
// player, `execute if items`) and, only when it changed, puts on or takes off the item's own
// modifier, by the same id as the item's component: were the engine to apply the item one day, it
// would never count twice.
// A player's first check this session takes it off when the hand is empty, so a modifier saved in
// the player's data never outlives the item.
//
// Log line (with `/sc log on`):
//   soulscraft: relic player=Alex holds=true
use crate::game::Game;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

/// The item, its attribute, the modifier's id and amount (the item's component in mod/sync.ps1).
const ITEM: &str = "soulscraft:abyss_heart";
const ATTRIBUTE: &str = "minecraft:max_health";
const MODIFIER: &str = "soulscraft:abyss_heart_health";
const AMOUNT: f64 = 4.0;
/// Ticks between two looks at the players' off hands.
const EVERY: u64 = 10;

/// Each online player's modifier as last set: on or off.
static ON: LazyLock<Mutex<HashMap<u128, bool>>> = LazyLock::new(Mutex::default);

/// The overworld's level tick, once every `EVERY` ticks: every online player, whatever their level.
pub(super) fn on_level_tick(game: &mut Game<'_>, dim: &str, now: u64) {
    if dim != "minecraft:overworld" || now % EVERY != 0 {
        return;
    }
    let players = game.native_server_players();
    let mut on = ON.lock().unwrap_or_else(PoisonError::into_inner);
    on.retain(|u, _| players.iter().any(|p| p.uuid == *u));
    for p in &players {
        let holds = game
            .native_player_equipment(p.uuid, horizon_client::mod_api::EquipmentSlot::OffHand)
            .is_some_and(|s| &*s.item == ITEM);
        if on.get(&p.uuid) == Some(&holds) {
            continue;
        }
        let line = if holds {
            format!("attribute {} {ATTRIBUTE} modifier add {MODIFIER} {AMOUNT} add_value", p.name)
        } else {
            format!("attribute {} {ATTRIBUTE} modifier remove {MODIFIER}", p.name)
        };
        game.native_run_command(&line);
        on.insert(p.uuid, holds);
        log::log!(crate::game::soulscraft_server::chatty(), "soulscraft: relic player={} holds={holds}", p.name);
    }
}
