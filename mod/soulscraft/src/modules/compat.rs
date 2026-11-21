// The bundle era's game API on 8f83e5c, for SoulsCraft's glue (`game::soulscraft_compat`).
//
// The glue was written against Horizon 7e4e421's `Game::native_*` methods. 8f83e5c renamed most of
// them, made them handle-based (`EntityRef`, `PlayerRef`, `LevelRef`) or removed them. Every old
// call in src/modules is spelled `game.sc_<old name>(…)` and keeps its old arguments and result;
// the `sc_` methods live on the extension traits of the three files below, one per area, and are
// re-exported here. A support module cannot add inherent methods to `Game` (it is compiled in the
// mod's patch crate, outside the game crate), so they are traits, and the `sc_` prefix keeps them
// from being shadowed by an inherent `native_*` method that kept its name but not its signature.
//
// RustCraft's port plans a shared shim (`mod/rustcraft/compat.rs` in rustcraft-modapi). It did not
// exist when this was written (2026-11-21); when it lands, SoulsCraft can take it as a
// `[[native.module]]` and keep only what it lacks here.
//
// Every module of the glue starts with `use super::soulscraft_compat::*;`.

pub(super) use super::soulscraft_compat_client::*;
pub(super) use super::soulscraft_compat_entities::*;
pub(super) use super::soulscraft_compat_world::*;

use super::Game;

/// An authoritative server's game (a dedicated server, or single player's integrated one): not a
/// client's copy of a remote world. The old level tick fired only there; 8f83e5c's fires on
/// clients too (network/remote/client_level_tick.rs).
pub(super) fn is_server(game: &Game<'_>) -> bool {
    game.remote.is_none() && !game.options.execution_role.is_client()
}
