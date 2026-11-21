//! SoulsCraft, a native Horizon mod on the Mod API (Nightly 8f83e5c).
//!
//! One crate, `soulscraft_rules` (the name the glue has always called it by):
//! - the game-free rules: the mob data, attack phases, power scaling, the mobs' minds, the rigs
//!   the client draws, the wire formats between the halves, the commands, the altar, the event,
//!   trophies, the book (the modules below, all with their own tests), and [`loot`], the loot
//!   table changes;
//! - the `Hooks` impl and Parts: the registrations (block entity types, Parts, payloads,
//!   commands), the hooks that need no game (loot tables, the bleed's period, block entity
//!   answers, placement states, the book's right click), and the empty methods that open the
//!   game's gates for the mixins;
//! - the game glue, outside this crate's module tree: support modules (src/modules, compiled as
//!   `crate::game::soulscraft_*` in the game's mirror) and the `[[native.mixin]]` helpers
//!   (src/mixins) that call them with the game, as horizon-mod.toml lists them.
//!
//! The glue can call this crate; this crate cannot call the glue. What the crate hears without
//! the game goes through [`bridge`]. The JSON it compiles in is the package's own
//! `data/soulscraft/*.json` and `assets/soulscraft/rigs/*.trophy.json`, written by mod/sync.ps1.

pub mod attack;
pub mod brain;
pub mod data;
pub mod geom;
pub mod pct;
pub mod scale;
pub mod rig;
pub mod wire;
pub mod command;
pub mod altar;
pub mod flow;
pub mod defense;
pub mod event;
pub mod view;
pub mod fit;
pub mod cover;
pub mod trophy;
pub mod boost;
pub mod vanilla;
pub mod book;
pub mod book_page;
pub mod natural;
pub mod loot;

pub mod bridge;

use mod_api::{
    BlockEntityPart, BlockPart, Flow, Hooks, IPayloadContext, IPayloadHandler, ItemPart, PayloadBytes,
    PayloadHandleError, PayloadId,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MOD_ID: &str = "soulscraft";

/// The mobs with trophies: each has a floor head, a wall head and a diorama, all with block
/// entities (soulscraft_trophy).
pub const TROPHY_MOBS: [&str; 7] = ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"];

/// Clientbound channels: the clip notes (a contract with LetsCam: same id, wire v1 bytes, no
/// wrapper), the altar views, the altar menu, the book.
pub const TO_CLIENT: [&str; 4] = [wire::STATE, view::ALTAR, view::MENU, book::CHANNEL];
/// Serverbound: the altar menu's buttons.
pub const TO_SERVER: [&str; 1] = [view::PRESS];
/// The payloads' schema version (`PayloadRegistrar::registrar`); the bytes carry their own.
pub const PAYLOAD_VERSION: &str = "1";

/// The pedestal's (and core's) item in its block entity (soulscraft_altar `ITEM_KEY`).
pub const ALTAR_ITEM_KEY: &str = "item";

/// Every block with a block entity: the altar's core and pedestal and every trophy. Each gets a
/// block entity type with the block's own path, so the saved `id` (`soulscraft:altar_core`) of a
/// world from the bundle era resolves to it and its data loads (save.rs:748-765 drops the data
/// of an undeclared type).
pub fn block_entity_blocks() -> Vec<String> {
    let mut blocks = vec![altar::CORE.to_owned(), altar::PEDESTAL.to_owned()];
    for mob in TROPHY_MOBS {
        blocks.push(trophy::Kind::Head(mob).block());
        blocks.push(trophy::Kind::WallHead(mob).block());
        blocks.push(trophy::Kind::Diorama(mob).block());
    }
    blocks
}

fn path(id: &str) -> &str {
    id.strip_prefix("soulscraft:").unwrap_or(id)
}

/// `keys` of `from`, copied (a block entity's fields for its client copy).
fn copy_keys<K: Ord + From<&'static str>, V: Clone>(from: &BTreeMap<K, V>, keys: &[&'static str]) -> BTreeMap<K, V> {
    let mut out = BTreeMap::new();
    for key in keys {
        let key = K::from(key);
        if let Some(value) = from.get(&key) {
            let value = value.clone();
            out.insert(key, value);
        }
    }
    out
}

#[derive(Default)]
struct SoulsCraft;

#[mod_api::horizon_mod]
impl Hooks for SoulsCraft {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        for block in block_entity_blocks() {
            let id = path(&block);
            let kind = r.block_entity_type(id).blocks(&[id]).build();
            if let Err(e) = r.block_entity_part(&kind, SoulsBlockEntity) {
                log::error!("soulscraft: block entity part for {block}: {e:?}");
            }
        }
        // Floor heads turn to the placer (rotation), dioramas face them (facing).
        for mob in TROPHY_MOBS {
            for block in [trophy::Kind::Head(mob).block(), trophy::Kind::Diorama(mob).block()] {
                let key = r.block_key(&block);
                r.block_part(key, Placement);
            }
        }
        let book_item = r.item_key(book::ITEM);
        r.item_part(book_item, HunterBook);
        let bleeding = r.effect_key("soulscraft:bleeding");
        let _ = bridge::BLEEDING.set(bleeding);

        let mut payloads = r.payload_handlers();
        let mut registrar = payloads.registrar(PAYLOAD_VERSION);
        for channel in TO_CLIENT {
            registrar.play_to_client(PayloadId(channel.to_owned()), Some(Arc::new(ToClient(channel))));
        }
        for channel in TO_SERVER {
            registrar.play_to_server(PayloadId(channel.to_owned()), Arc::new(ToServer(channel)));
        }
    }

    // ---- Server ----

    /// Gate: the level tick mixin (src/mixins/level_tick.rs) runs the server half.
    fn level_tick_post(&self, _: mod_api::HostSide, _: &mut mod_api::LevelTickPostEvent) {}

    /// `/sc` for operators, the book's command, the bench's console command (level 4: only the
    /// console and the mods run it) and RustCraft's machine kills.
    fn register_commands(&self, _: &mut dyn mod_api::ServerHost, event: &mut mod_api::RegisterCommandsEvent<'_>) {
        for usage in command::USAGES {
            event.commands.push(mod_api::Command::op(usage, 2));
        }
        event
            .commands
            .push(mod_api::Command::op(&format!("{} run [filter:word]", command::SELFTEST), 4));
        event.commands.push(mod_api::Command::op("sc book <player:word> [action:text]", 2));
        // The type has a colon, which a `word` does not take: the rest of the line is one `text`.
        event.commands.push(mod_api::Command::op(
            &format!("{} <source:word> <owner:word> <entity:integer> <kind_and_block:text>", book::KILL_COMMAND),
            4,
        ));
    }

    /// Gate: src/mixins/entity_join.rs fits a new SoulsCraft mob before its first tick (goals, box,
    /// flags), as the bundle era's `register_goals` did at its construction.
    fn entity_join_level(&self, _: &mut dyn mod_api::ServerHost, _: &mut mod_api::EntityJoinLevelEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// Gate: the command mixin (src/mixins/run_mod_command.rs) runs them with the game.
    fn run_mod_command(&self, _: &mut dyn mod_api::ServerHost, _: &mut mod_api::RunModCommandEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// A hit about to land: the revenant's raised shield cuts a blow from the front, and a hit on
    /// a wave mob breaks its drain (soulscraft_server::on_incoming_damage, installed in
    /// [`bridge`] by the level tick).
    fn living_incoming_damage(
        &self,
        _: &mut dyn mod_api::LevelRandomHost,
        event: &mut mod_api::LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        let Some(hook) = bridge::incoming_damage() else { return Flow::Continue };
        let mut amount = event.container.new_damage();
        let before = amount;
        hook(event.entity, event.kind, event.source.damage_type, event.source.causing, &mut amount);
        if amount != before {
            event.container.set_new_damage(amount);
        }
        Flow::Continue
    }

    /// soulscraft:bleeding hurts every `period_ticks` (data/mobs.json `effects.bleeding`).
    fn should_apply_effect_tick_this_tick(
        &self,
        _: &dyn mod_api::Host,
        event: &mut mod_api::ShouldApplyEffectTickThisTickEvent,
    ) {
        if bridge::is_bleeding(event.effect) {
            event.apply = event.tick_count % bridge::bleed_period() == 0;
        }
    }

    /// Gate: the bleed's hurt is queued by the effect tick mixin (src/mixins/effect_tick.rs).
    fn apply_effect_tick(&self, _: &dyn mod_api::Host, _: &mut mod_api::ApplyEffectTickEvent<'_>) {}

    /// Gate: src/mixins/right_click_block.rs (wall heads, the pedestal, the core).
    fn right_click_block(&self, _: mod_api::HostSide, _: &mut mod_api::RightClickBlockEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// Gate: src/mixins/break_block.rs (a creative player's tall piece drops nothing).
    fn break_block(&self, _: &mut dyn mod_api::ServerHost, _: &mut mod_api::BreakBlockEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// Gate: src/mixins/potential_spawns.rs (the biome's SoulsCraft mobs, a sector's extras).
    fn potential_spawns(&self, _: &dyn mod_api::Host, _: &mut mod_api::PotentialSpawnsEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// Chest horns and shards, zombies' Rotten Horn (data/loot.json): the same each time a
    /// table loads, on a server and a client.
    /// The table is the SDK's `serde_json::Value`, a crate the mod can't share (SDK values keep
    /// their crate identity; ours derives with the registry's serde), so it goes through text.
    fn loot_table_load(&self, _: &dyn mod_api::DataLoadContext, event: &mut mod_api::LootTableLoadEvent<'_>) {
        let Ok(text) = mod_api::serde_json::to_string(&*event.table) else { return };
        let Ok(mut ours) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let before = ours.clone();
        loot::apply(event.id, &mut ours);
        if ours == before {
            return;
        }
        match serde_json::to_string(&ours).map(|t| mod_api::serde_json::from_str(&t)) {
            Ok(Ok(table)) => *event.table = table,
            _ => log::error!("soulscraft: loot table {} could not be written back", event.id),
        }
    }

    // ---- Client ----

    /// Gate: src/mixins/client_tick.rs (the client halves' net and windows).
    fn client_tick_post(&self, _: &mut dyn mod_api::ClientHost, _: &mut mod_api::ClientTickPostEvent) {}

    /// A replay delivers the clientbound payloads as plain custom payload packets, which the
    /// payload handlers never see (live ones arrive as raw payloads and skip this hook:
    /// network/remote/raw_ingress.rs): queue ours for the client tick the same way.
    fn client_packet(&self, _: &mut dyn mod_api::ClientHost, event: &mut mod_api::ClientPacketEvent<'_>) {
        if let Some(payload) = &event.packet.custom_payload
            && TO_CLIENT.contains(&payload.channel.as_str())
        {
            bridge::push_client(&payload.channel, payload.data.clone());
        }
    }

    /// Gate: src/mixins/render_living_pre.rs (SoulsCraft's mobs drawn from their rigs).
    fn render_living_pre(&self, _: &dyn mod_api::ClientHost, _: &mut mod_api::RenderLivingPreEvent<'_>) -> Flow {
        Flow::Continue
    }

    /// Gate: src/mixins/custom_geometry.rs (debug labels, the altar's beams and wave points).
    fn submit_custom_geometry(
        &self,
        _: &dyn mod_api::ClientHost,
        _: &mut mod_api::SubmitCustomGeometryEvent<'_>,
    ) {
    }

    /// Gate: src/mixins/screen_render_post.rs (the altar menu and the book's window). Subscribing
    /// also makes the game build the screen view the mixin reads (native_hooks/shared.rs:828-837).
    fn screen_render_post(&self, _: &dyn mod_api::ClientHost, _: &mut mod_api::ScreenRenderPostEvent<'_>) {}

    /// Gate: src/mixins/screen_press.rs (their buttons).
    fn screen_mouse_button_pressed_pre(
        &self,
        _: &dyn mod_api::ClientHost,
        _: &mut mod_api::ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        Flow::Continue
    }
}

/// The block entities of the altar's core and pedestal and of the trophies. The declared type
/// creates each with an empty compound (the bundle era's initial data), so `block_entity` is not
/// answered.
struct SoulsBlockEntity;

#[mod_api::horizon_part]
impl BlockPart for SoulsBlockEntity {}

#[mod_api::horizon_part]
impl BlockEntityPart for SoulsBlockEntity {
    /// The core ticks, so a core in a loaded chunk is found; a diorama ticks for its particles.
    fn block_entity_ticks(&self, _: &dyn mod_api::Host, event: &mut mod_api::BlockEntityTicksEvent<'_>) {
        if event.block == altar::CORE || matches!(trophy::Kind::of(event.block), Some(trophy::Kind::Diorama(_))) {
            event.ticks = true;
        }
    }

    /// Queued for the level's tick (soulscraft_altar, soulscraft_trophy); the entity is untouched.
    fn block_entity_tick(&self, _: &mut dyn mod_api::ServerHost, event: &mut mod_api::BlockEntityTickEvent<'_>) {
        let id = event.entity.id().to_owned();
        if id == altar::CORE {
            bridge::core_ticked(event.pos);
        } else if matches!(trophy::Kind::of(&id), Some(trophy::Kind::Diorama(_))) {
            bridge::diorama_ticked(event.pos, &id);
        }
        *event.reply = mod_api::BlockEntityTickReply::Unchanged;
    }

    /// What clients get: the pedestal's and the core's item, a diorama's owner and kills.
    fn block_entity_update_tag(&self, event: &mut mod_api::BlockEntityUpdateTagEvent<'_>) -> Option<mod_api::Compound> {
        let entity = event.entity;
        let id = entity.id();
        let keys: &[&'static str] = if id == altar::PEDESTAL || id == altar::CORE {
            &[ALTAR_ITEM_KEY]
        } else if matches!(trophy::Kind::of(id), Some(trophy::Kind::Diorama(_))) {
            &[trophy::OWNER_KEY, trophy::KILLS_KEY]
        } else {
            return None;
        };
        Some(copy_keys(&entity.nbt, keys))
    }

    /// Gate: src/mixins/block_entity_submit.rs draws the pedestal's item, the core's ghost,
    /// heads and dioramas.
    fn block_entity_submit(&self, _: &dyn mod_api::ClientHost, _: &mut mod_api::BlockEntitySubmitEvent<'_>) {}

    /// The core's ghost reaches four blocks round it.
    fn block_entity_render_bounding_box(
        &self,
        _: &dyn mod_api::ClientHost,
        event: &mut mod_api::BlockEntityRenderBoundingBoxEvent<'_>,
    ) {
        if event.block == altar::CORE {
            *event.bounds = [-4.0, -1.0, -4.0, 5.0, 4.0, 5.0];
        }
    }
}

/// The state a floor head (rotation 0..15) or a diorama (facing) is placed in.
struct Placement;

#[mod_api::horizon_part]
impl BlockPart for Placement {
    #[allow(clippy::option_option)]
    fn state_for_placement(
        &self,
        level: &dyn mod_api::LevelHost,
        event: &mut mod_api::StateForPlacementEvent<'_>,
    ) -> Option<Option<mod_api::BlockState>> {
        let block = level.state_block(event.state)?;
        let (key, value) = match trophy::Kind::of(block)? {
            trophy::Kind::Head(_) => ("rotation", trophy::floor_rotation(event.context.yaw).to_string()),
            trophy::Kind::Diorama(_) => ("facing", trophy::facing_toward(event.context.yaw).to_owned()),
            trophy::Kind::WallHead(_) => return None,
        };
        level.state_with_property(event.state, key, &value).map(Some)
    }
}

/// The hunter's book: a right click opens its window (soulscraft_book, on the level's tick).
struct HunterBook;

#[mod_api::horizon_part]
impl ItemPart for HunterBook {
    fn use_item(&self, _: mod_api::HostSide, event: &mut mod_api::UseItemEvent<'_>) -> Option<mod_api::InteractionResult> {
        bridge::book_opened(event.player);
        Some(mod_api::InteractionResult::Success)
    }
}

/// A clientbound payload, queued for the client tick.
struct ToClient(&'static str);

impl IPayloadHandler for ToClient {
    fn handle(&self, payload: PayloadBytes, _: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        bridge::push_client(self.0, payload.0);
        Ok(())
    }
}

/// A serverbound payload with its sender, queued for the level tick.
struct ToServer(&'static str);

impl IPayloadHandler for ToServer {
    fn handle(&self, payload: PayloadBytes, context: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let player = context
            .player()
            .map_err(|e| PayloadHandleError::Failed(format!("{}: no player: {e:?}", self.0)))?;
        bridge::push_server(player.uuid(), self.0, payload.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trophy_mob_has_its_file() {
        for mob in TROPHY_MOBS {
            assert!(trophy::embedded_text(mob).is_some(), "{mob}");
        }
    }

    #[test]
    fn block_entity_types_are_the_blocks() {
        let blocks = block_entity_blocks();
        assert_eq!(blocks.len(), 2 + 3 * TROPHY_MOBS.len());
        assert!(blocks.iter().all(|b| b.starts_with("soulscraft:")));
    }
}
