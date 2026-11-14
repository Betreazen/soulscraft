//! SoulsCraft's game-free rules: the mob data, attack phases, power scaling, the mobs' minds,
//! the rigs the client draws, the wire format between the halves and the commands. The bundle's
//! helpers and modules (compiled into the game) only glue these to the engine.

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
