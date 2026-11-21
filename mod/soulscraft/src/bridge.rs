//! What the mod crate and the game glue hand each other.
//!
//! The crate (lib.rs: the `Hooks` impl, the Parts, the payload handlers) is compiled before the
//! game glue (src/modules, compiled inside `crate::game`), so it cannot call the glue. The glue
//! can call the crate. Everything the crate hears without the game (a payload, a block entity's
//! tick, a right click with the book) is queued here, and the glue drains it from a mixin that
//! has the game: the level tick on the server, the client tick on a client. The one hook that
//! must answer at once without the game, the incoming damage, calls a function the glue installs.
//!
//! A `static` here is shared by every `Game` in the process (docs/modding concepts/native-route
//! 6-native-state.md). The queues are drained every tick, so nothing of one world outlives it.
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::sync::atomic::{AtomicI32, Ordering};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- Net -----------------------------------------------------------------------------------

/// Clientbound payloads (channel, bytes), from the payload handlers and, on a replay, from the
/// `client_packet` hook. Drained by the client tick into the client halves' `on_net`.
static CLIENT_INBOX: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());
/// Serverbound payloads (player, channel, bytes). Drained by the level tick.
static SERVER_INBOX: Mutex<Vec<(u128, String, Vec<u8>)>> = Mutex::new(Vec::new());

pub fn push_client(channel: &str, data: Vec<u8>) {
    lock(&CLIENT_INBOX).push((channel.to_owned(), data));
}

pub fn take_client() -> Vec<(String, Vec<u8>)> {
    std::mem::take(&mut *lock(&CLIENT_INBOX))
}

pub fn push_server(player: u128, channel: &str, data: Vec<u8>) {
    lock(&SERVER_INBOX).push((player, channel.to_owned(), data));
}

pub fn take_server() -> Vec<(u128, String, Vec<u8>)> {
    std::mem::take(&mut *lock(&SERVER_INBOX))
}

// ---- Block entities ----------------------------------------------------------------------------

/// Altar cores whose block entity ticked since the last level tick (`soulscraft_altar`).
static CORES_TICKED: Mutex<Vec<[i32; 3]>> = Mutex::new(Vec::new());
/// Dioramas (position, block) whose block entity ticked (`soulscraft_trophy`).
static DIORAMAS_TICKED: Mutex<Vec<([i32; 3], String)>> = Mutex::new(Vec::new());

pub fn core_ticked(pos: [i32; 3]) {
    lock(&CORES_TICKED).push(pos);
}

pub fn take_cores_ticked() -> Vec<[i32; 3]> {
    std::mem::take(&mut *lock(&CORES_TICKED))
}

pub fn diorama_ticked(pos: [i32; 3], block: &str) {
    lock(&DIORAMAS_TICKED).push((pos, block.to_owned()));
}

pub fn take_dioramas_ticked() -> Vec<([i32; 3], String)> {
    std::mem::take(&mut *lock(&DIORAMAS_TICKED))
}

// ---- The hunter's book ---------------------------------------------------------------------

/// Players who right-clicked with the book since the last level tick (`soulscraft_book`).
static BOOK_OPENS: Mutex<Vec<u128>> = Mutex::new(Vec::new());

pub fn book_opened(player: u128) {
    lock(&BOOK_OPENS).push(player);
}

pub fn take_book_opens() -> Vec<u128> {
    std::mem::take(&mut *lock(&BOOK_OPENS))
}

// ---- The bleed -----------------------------------------------------------------------------

/// `soulscraft:bleeding`, as `register` requested it.
pub static BLEEDING: OnceLock<mod_api::EffectKey> = OnceLock::new();
/// Ticks between two hurts of the bleed (data/mobs.json `effects.bleeding.period_ticks`); the
/// server's level tick keeps it equal to the loaded numbers.
static BLEED_PERIOD: AtomicI32 = AtomicI32::new(20);

pub fn is_bleeding(effect: mod_api::EffectKey) -> bool {
    BLEEDING.get() == Some(&effect)
}

pub fn set_bleed_period(ticks: i32) {
    BLEED_PERIOD.store(ticks.max(1), Ordering::Relaxed);
}

pub fn bleed_period() -> i32 {
    BLEED_PERIOD.load(Ordering::Relaxed).max(1)
}

// ---- Incoming damage -------------------------------------------------------------------------

/// The glue's damage hook: (entity, kind, damage type, causing entity or 0, amount to change).
pub type IncomingDamage = fn(i32, &str, &str, i32, &mut f32);

static INCOMING_DAMAGE: OnceLock<IncomingDamage> = OnceLock::new();

/// Called by the glue on every level tick; only the first call stores.
pub fn install_incoming_damage(hook: IncomingDamage) {
    let _ = INCOMING_DAMAGE.set(hook);
}

pub fn incoming_damage() -> Option<IncomingDamage> {
    INCOMING_DAMAGE.get().copied()
}

// ---- Commands --------------------------------------------------------------------------------

/// A parsed command as the line the old `/sc` and console parsers read: its literals, then its
/// arguments in parse order. Every SoulsCraft usage puts its literals before its arguments
/// (soulscraft_rules::command::USAGES), so the line is the one the player typed, normalised.
pub fn command_line(command: &mod_api::CommandInvocation) -> String {
    use mod_api::CommandArgumentValue as V;
    let mut words: Vec<String> = command.literals.clone();
    for argument in &command.arguments {
        let word = match &argument.value {
            V::String(s) => s.clone(),
            V::Integer(i) => i.to_string(),
            V::Number(n) => n.to_string(),
            V::Boolean(b) => b.to_string(),
            V::BlockPos([x, y, z]) => format!("{x} {y} {z}"),
            V::Vec3([x, y, z]) => format!("{x} {y} {z}"),
            V::Id(id) => format!("{}:{}", id.namespace(), id.path()),
            _ => continue,
        };
        words.push(word);
    }
    words.join(" ")
}
