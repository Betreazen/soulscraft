// The bundle era's world, player, command and net helpers on 8f83e5c (`game::soulscraft_compat_world`,
// re-exported by compat.rs): blocks and block entities, the players (`NativeServerPlayer`, now
// built from a `PlayerRef`), console commands, items in the acting player's hands, persistent
// data and the clientbound payloads. Each `sc_<name>` keeps the 7e4e421 `native_<name>`'s
// arguments and result; the comment on each says what it stands on in 8f83e5c.
use super::Game;
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_core::nbt::Tag;
use horizon_world::registry::BlockStateId;
use std::path::PathBuf;

/// What a hook said of a player in the bundle era (`mod_api::NativeServerPlayer`), now read
/// through the player's handle. The fields SoulsCraft never read are kept with neutral values.
#[derive(Clone, Debug, Default)]
pub(super) struct NativeServerPlayer {
    pub uuid: u128,
    pub name: String,
    pub permission: u8,
    pub dimension: String,
    pub position: [f64; 3],
    /// 0 survival, 1 creative, 2 adventure, 3 spectator.
    pub game_mode: i32,
    pub health: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub entity_id: i32,
    pub latency: i32,
    pub on_ground: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub invulnerable: bool,
    pub walk_speed: f32,
    pub fly_speed: f32,
    pub sneaking: bool,
    pub eye_height: f32,
    pub selected_slot: u8,
}

/// The player `player` as the bundle era described them; None for a stale handle.
pub(super) fn server_player(game: &Game<'_>, player: mod_api::PlayerRef) -> Option<NativeServerPlayer> {
    let host: &dyn mod_api::Host = game;
    let entity = player.entity();
    let name = player.name(host)?;
    let position = entity.position(host)?;
    let [yaw, pitch] = entity.rotation(host).unwrap_or([0.0, 0.0]);
    let abilities = player.abilities(host);
    let game_mode = match player.game_mode(host) {
        Some(mod_api::GameMode::Creative) => 1,
        Some(mod_api::GameMode::Adventure) => 2,
        Some(mod_api::GameMode::Spectator) => 3,
        _ => 0,
    };
    let health = game.living(entity).and_then(|l| l.health(host)).unwrap_or(0.0);
    Some(NativeServerPlayer {
        uuid: player.uuid(),
        name,
        permission: player.permission_level(host).unwrap_or(0),
        dimension: player.dimension(host).map(|d| d.to_string()).unwrap_or_default(),
        position,
        game_mode,
        health,
        yaw,
        pitch,
        entity_id: entity.id() as i32,
        latency: player.latency(host).unwrap_or(0),
        on_ground: entity.on_ground(host).unwrap_or(false),
        flying: abilities.as_ref().is_some_and(|a| a.flying),
        may_fly: abilities.as_ref().is_some_and(|a| a.may_fly),
        invulnerable: abilities.as_ref().is_some_and(|a| a.invulnerable),
        walk_speed: abilities.as_ref().map_or(0.1, |a| a.walk_speed),
        fly_speed: abilities.as_ref().map_or(0.05, |a| a.fly_speed),
        sneaking: entity.is_shift_key_down(host).unwrap_or(false),
        eye_height: entity.eye_height(host).unwrap_or(1.62) as f32,
        selected_slot: player.selected_slot(host).unwrap_or(0) as u8,
    })
}

/// `native_run_command_captured`'s answer: whether it ran, its feedback lines, and the summed
/// result of its successful commands (vanilla's `/execute store result`), 0 or 1 for a mod's.
#[derive(Clone, Debug, Default)]
pub(super) struct NativeCommandResult {
    pub ran: bool,
    pub output: Vec<Text>,
    pub result: i32,
}

/// `native_command_info`: what SoulsCraft read of a command (only whether a mod registered it).
#[derive(Clone, Debug)]
pub(super) struct NativeCommandInfo {
    pub name: String,
    pub level: u8,
}

/// The heightmaps `native_surface_heights` took: the API's own now.
pub(super) type NativeHeightmap = mod_api::Heightmap;

/// A stack with no damage and no components (`native_held`'s third value).
fn plain(stack: &ItemStack) -> bool {
    stack.damage == 0 && stack.extra.is_none()
}

/// The off hand's index in [`slots`].
const OFFHAND: usize = 36;

/// The slots the item helpers look at, in `Inventory.add`'s order: the selected slot, the off
/// hand, then the hotbar and the main inventory.
fn order(selected: usize) -> impl Iterator<Item = usize> {
    std::iter::once(selected).chain(std::iter::once(OFFHAND)).chain((0..36).filter(move |&i| i != selected))
}

/// The 36 inventory slots and the off hand as one list ([`OFFHAND`] last).
fn slots<'a>(inventory: &'a mut [Option<ItemStack>], offhand: &'a mut Option<ItemStack>) -> Vec<&'a mut Option<ItemStack>> {
    inventory.iter_mut().take(36).chain(std::iter::once(offhand)).collect()
}

pub(super) trait ScWorld {
    fn sc_block(&self, pos: [i32; 3]) -> String;
    fn sc_block_property(&self, pos: [i32; 3], key: &str) -> Option<String>;
    fn sc_set_block_property_with(&mut self, pos: [i32; 3], key: &str, value: &str, flags: i32) -> bool;
    fn sc_block_collision(&self, pos: [i32; 3]) -> Vec<[f64; 6]>;
    fn sc_state_block(&self, state: BlockStateId) -> Option<&str>;
    fn sc_state_with_property(&self, state: BlockStateId, key: &str, value: &str) -> Option<BlockStateId>;
    fn sc_mod_block_entity(&self, pos: [i32; 3]) -> Option<ModBlockEntity>;
    fn sc_with_mod_block_entity<R>(&mut self, pos: [i32; 3], change: impl FnOnce(&mut ModBlockEntity) -> R) -> Option<R>;
    fn sc_block_entity_sync(&mut self, pos: [i32; 3]);
    fn sc_surface_heights(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
        heightmap: NativeHeightmap,
    ) -> Result<Vec<Option<i32>>, String>;
    fn sc_biome(&self, pos: [i32; 3]) -> Option<String>;
    fn sc_server_players(&self) -> Vec<NativeServerPlayer>;
    fn sc_acting_player(&self) -> Option<NativeServerPlayer>;
    fn sc_run_command(&mut self, line: &str) -> bool;
    fn sc_run_command_captured(&mut self, line: &str) -> NativeCommandResult;
    fn sc_run_mod_command(&mut self, line: &str) -> bool;
    fn sc_potential_spawns(
        &self,
        dimension: &str,
        category: &str,
        pos: [i32; 3],
        entries: &mut Vec<horizon_client::biome::SpawnEntry>,
        canceled: &mut bool,
    );
    fn sc_feedback(&mut self, line: Text);
    fn sc_action_bar(&mut self, uuid: u128, line: Text) -> bool;
    fn sc_command_info(&self, label: &str) -> Option<NativeCommandInfo>;
    fn sc_held(&self) -> Option<(String, u32, bool)>;
    fn sc_take_items(&mut self, item: &str, count: u32) -> u32;
    fn sc_give_items(&mut self, item: &str, count: u32) -> u32;
    fn sc_player_equipment(&self, uuid: u128, slot: mod_api::EquipmentSlot) -> Option<ItemStack>;
    fn sc_player_persistent_data(&self, uuid: u128) -> Option<Tag>;
    fn sc_set_player_persistent_data(&mut self, uuid: u128, data: Tag) -> bool;
    fn sc_net_send_to_player(&mut self, player: u128, channel: &str, data: &[u8]) -> Result<(), String>;
    fn sc_config_dir() -> Result<PathBuf, String>;
}

impl ScWorld for Game<'_> {
    /// The block's name at `pos` in the level in place (`world.block` and the block facts, as
    /// 7e4e421's `native_block`).
    fn sc_block(&self, pos: [i32; 3]) -> String {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.res.facts().get(state).name.to_string()
    }

    /// A property of the block at `pos` (the registry state's `properties`, as
    /// native_block_entity_draw.rs `block_state_property` reads it).
    fn sc_block_property(&self, pos: [i32; 3], key: &str) -> Option<String> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.pack.assets.registry.state(state)?.properties.get(key).cloned()
    }

    /// native_hooks/base.rs `set_block_property_with` (the old `native_set_block_property_with`
    /// under its new name).
    fn sc_set_block_property_with(&mut self, pos: [i32; 3], key: &str, value: &str, flags: i32) -> bool {
        self.set_block_property_with(pos, key, value, flags)
    }

    /// blocks/respawn_anchor.rs `block_collision_shape`, as boxes.
    fn sc_block_collision(&self, pos: [i32; 3]) -> Vec<[f64; 6]> {
        self.block_collision_shape(pos)
            .boxes()
            .iter()
            .map(|bounds| {
                let min = bounds.min_position();
                let max = bounds.max_position();
                [min.x, min.y, min.z, max.x, max.y, max.z]
            })
            .collect()
    }

    /// modding/native_block_entity_draw.rs `native_state_block` (Game's).
    fn sc_state_block(&self, state: BlockStateId) -> Option<&str> {
        self.native_state_block(state)
    }

    /// `state` with its property `key` set to `value` (`BlockState.setValue`): the body of
    /// native_hooks/shared.rs `with_property` over the game's registry, which GameLevel's
    /// `native_state_with_property` calls.
    fn sc_state_with_property(&self, state: BlockStateId, key: &str, value: &str) -> Option<BlockStateId> {
        let registry = self.pack.assets.registry;
        let mut s = registry.state(state)?.clone();
        match s.properties.get(key) {
            Some(v) if v == value => return Some(state),
            Some(_) => {}
            None => return None,
        }
        s.properties.insert(key.into(), value.into());
        registry.resolve(&s).ok()
    }

    /// A copy of the mod block entity at `pos` (the level's `block_entities`, as 7e4e421).
    fn sc_mod_block_entity(&self, pos: [i32; 3]) -> Option<ModBlockEntity> {
        match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Mod(m)) if !m.id().is_empty() => Some(ModBlockEntity::clone(m)),
            _ => None,
        }
    }

    /// native_hooks/shared.rs:2074 `native_with_mod_block_entity`, unchanged.
    fn sc_with_mod_block_entity<R>(&mut self, pos: [i32; 3], change: impl FnOnce(&mut ModBlockEntity) -> R) -> Option<R> {
        self.native_with_mod_block_entity(pos, change)
    }

    /// native_hooks/base.rs:4447 `block_entity_sync` (renamed).
    fn sc_block_entity_sync(&mut self, pos: [i32; 3]) {
        self.block_entity_sync(pos);
    }

    /// modding/native_world_columns.rs:112 `surface_heights`, which now takes `mod_api::Heightmap`.
    fn sc_surface_heights(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
        heightmap: NativeHeightmap,
    ) -> Result<Vec<Option<i32>>, String> {
        self.surface_heights(dimension, columns, heightmap)
    }

    /// The biome's name at `pos`, zoomed as vanilla's `getBiome` (7e4e421's `native_biome`, with
    /// administration/commands/execute.rs `biome_name_at_block`'s body inline).
    fn sc_biome(&self, pos: [i32; 3]) -> Option<String> {
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        let [x, y, z] = self.streaming.biome_zoom.quart_at_block(at);
        let p = [x << 2, y << 2, z << 2];
        let chunk = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))?;
        Some(world_biomes!(self).get(chunk.biome(p[0], p[1], p[2])).name.clone())
    }

    /// modding/native_server.rs:1131 `native_server_players` (handles now), each read back.
    fn sc_server_players(&self) -> Vec<NativeServerPlayer> {
        self.native_server_players().into_iter().filter_map(|p| server_player(self, p)).collect()
    }

    /// modding/native_server.rs:1723 `native_acting_player` (a handle now).
    fn sc_acting_player(&self) -> Option<NativeServerPlayer> {
        let p = self.native_acting_player()?;
        server_player(self, p)
    }

    /// modding/native_server.rs:1739 `native_run_command`, unchanged.
    fn sc_run_command(&mut self, line: &str) -> bool {
        self.native_run_command(line)
    }

    /// modding/native_server.rs `run_console_command` with its capture on, and the command
    /// results (`cmds.results`) summed as 7e4e421 did.
    fn sc_run_command_captured(&mut self, line: &str) -> NativeCommandResult {
        let mut out = NativeCommandResult::default();
        if self.lan.is_none() {
            return out;
        }
        let line = line.strip_prefix('/').unwrap_or(line);
        let modded = self.native_commands.level(line).is_some();
        let outer_results = self.cmds.results.replace(Vec::new());
        let (ran, output) = self.run_console_command(line, true);
        let results = std::mem::replace(&mut self.cmds.results, outer_results).unwrap_or_default();
        out.ran = ran;
        out.output = output;
        out.result = if modded { i32::from(ran) } else { results.iter().filter(|r| r.0).map(|r| r.1).sum() };
        out
    }

    /// A console line for a mod's registered command: modding/native_server.rs
    /// `perform_native_console_command` (true when a mod registered it; it then runs through
    /// the dispatcher and `run_mod_command`).
    fn sc_run_mod_command(&mut self, line: &str) -> bool {
        let line = line.strip_prefix('/').unwrap_or(line);
        self.perform_native_console_command(line)
    }

    /// native_hooks/native_potential_spawns.rs:9 `native_potential_spawns`, unchanged.
    fn sc_potential_spawns(
        &self,
        dimension: &str,
        category: &str,
        pos: [i32; 3],
        entries: &mut Vec<horizon_client::biome::SpawnEntry>,
        canceled: &mut bool,
    ) {
        self.native_potential_spawns(dimension, category, pos, entries, canceled);
    }

    /// modding/native_server.rs:1709 `native_feedback` (now routed to the command's source).
    fn sc_feedback(&mut self, line: Text) {
        self.native_feedback(line);
    }

    /// modding/native_server.rs:1293 `player_action_bar` for the online player `uuid`.
    fn sc_action_bar(&mut self, uuid: u128, line: Text) -> bool {
        let Some(player) = self.player(uuid) else { return false };
        self.player_action_bar(player, line)
    }

    /// Whether a mod registered `label` (modding/native_server/registered_commands.rs
    /// `NativeCommandState::level`). Vanilla commands are not described any more.
    fn sc_command_info(&self, label: &str) -> Option<NativeCommandInfo> {
        let label = label.strip_prefix('/').unwrap_or(label);
        self.lan.as_ref()?;
        let level = self.native_commands.level(label)?;
        Some(NativeCommandInfo { name: label.to_owned(), level })
    }

    /// The acting player's selected stack: item, count, plain (Game's `inventory`/`selected`).
    fn sc_held(&self) -> Option<(String, u32, bool)> {
        let s = self.inventory.get(self.selected)?.as_ref()?;
        Some((s.item.to_string(), s.count, plain(s)))
    }

    /// Takes up to `count` plain `item` from the acting player, selected slot first.
    fn sc_take_items(&mut self, item: &str, count: u32) -> u32 {
        if self.player.is_none() {
            return 0;
        }
        let selected = self.selected;
        let mut slots = slots(&mut self.inventory, &mut self.offhand);
        let mut taken = 0;
        for i in order(selected) {
            let slot = &mut *slots[i];
            if let Some(s) = slot
                && &*s.item == item
                && plain(s)
            {
                let n = s.count.min(count - taken);
                s.count -= n;
                taken += n;
                if s.count == 0 {
                    *slot = None;
                }
            }
            if taken == count {
                break;
            }
        }
        taken
    }

    /// Gives the acting player up to `count` plain `item`: onto matching stacks first, then empty
    /// slots (not the off hand); what went in.
    fn sc_give_items(&mut self, item: &str, count: u32) -> u32 {
        let Some(max) = self.res.items().get(item).map(|i| i.max_stack.max(1)) else {
            return 0;
        };
        if self.player.is_none() {
            return 0;
        }
        let selected = self.selected;
        let mut slots = slots(&mut self.inventory, &mut self.offhand);
        let mut added = 0;
        for pass in 0..2 {
            for i in order(selected) {
                if added == count {
                    return added;
                }
                let slot = &mut *slots[i];
                let n = match slot {
                    Some(s) if pass == 0 && &*s.item == item && plain(s) => {
                        let n = max.saturating_sub(s.count).min(count - added);
                        s.count += n;
                        n
                    }
                    None if pass == 1 && i != OFFHAND => {
                        let n = max.min(count - added);
                        *slot = Some(ItemStack { item: item.into(), count: n, damage: 0, extra: None });
                        n
                    }
                    _ => 0,
                };
                added += n;
            }
        }
        added
    }

    /// modding/native_living_refs.rs:766 `living_equipment` of the online player `uuid` (the
    /// engine's stack, as 7e4e421 returned).
    fn sc_player_equipment(&self, uuid: u128, slot: mod_api::EquipmentSlot) -> Option<ItemStack> {
        let player = self.player(uuid)?;
        let living = self.living(player.entity())?;
        self.living_equipment(living, slot).filter(|s| s.count > 0)
    }

    /// modding/native_player_persistent_data.rs:38 `player_persistent_data`, as a compound tag.
    fn sc_player_persistent_data(&self, uuid: u128) -> Option<Tag> {
        let player = self.player(uuid)?;
        self.player_persistent_data(player).map(Tag::Compound)
    }

    /// modding/native_player_persistent_data.rs:51 `player_set_persistent_data`; false for a tag
    /// that is not a compound or a refused write.
    fn sc_set_player_persistent_data(&mut self, uuid: u128, data: Tag) -> bool {
        let Tag::Compound(data) = data else { return false };
        let Some(player) = self.player(uuid) else { return false };
        match self.player_set_persistent_data(player, data) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("soulscraft: persistent data of {uuid:x} not written: {e:?}");
                false
            }
        }
    }

    /// The bytes on the declared clientbound payload `channel` (lib.rs `TO_CLIENT`), as they are:
    /// modding/payload_transport.rs:28 `player_send_payload`, queued until the root commit.
    fn sc_net_send_to_player(&mut self, player: u128, channel: &str, data: &[u8]) -> Result<(), String> {
        if self.remote.is_some() {
            return Err("a client has no players to send to".into());
        }
        let Some(to) = self.player(player) else {
            return Err(format!("no online player {player:x}"));
        };
        let id = mod_api::PayloadId(channel.to_owned());
        self.player_send_payload(to, &id, mod_api::PayloadBytes(data.to_vec()))
            .map(|_| ())
            .map_err(|e| format!("{channel}: {e:?}"))
    }

    /// `config/` in the game folder, beside `options.txt`, made when missing (native_hooks/shared.rs
    /// `config_dir` over interface/options.rs `game_dir`).
    fn sc_config_dir() -> Result<PathBuf, String> {
        let dir = Self::game_dir().join("config");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(dir)
    }
}
