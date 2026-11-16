// A level's mob by id without a walk through all of them (docs/design/12-playtest-tidewill.md,
// plan items 1 and 2): on a live server the level holds hundreds of mobs, and every adopted or own
// mob asked for itself several times a tick, each ask a walk through the whole list.
//
// The index is rebuilt at the start of each level tick. A look-up trusts it only where the place
// still holds that id; otherwise (a mob that joined since, a list reordered, another level's
// index) it walks the list as before, so the answer is always the walk's.
use crate::game::Game;
use horizon_client::mobs::Mob;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static INDEX: RefCell<HashMap<u32, usize>> = RefCell::new(HashMap::new());
}

/// The level's mobs as they stand now: once per level tick, before any of them is driven.
pub(super) fn reindex(game: &Game<'_>) {
    INDEX.with_borrow_mut(|index| {
        index.clear();
        index.extend(game.mobs.iter().enumerate().map(|(i, m)| (m.id, i)));
    });
}

/// Where mob `id` stands in `game.mobs`.
pub(super) fn index_of(game: &Game<'_>, id: u32) -> Option<usize> {
    let hint = INDEX.with_borrow(|index| index.get(&id).copied());
    if let Some(i) = hint
        && game.mobs.get(i).is_some_and(|m| m.id == id)
    {
        return Some(i);
    }
    game.mobs.iter().position(|m| m.id == id)
}

pub(super) fn mob<'g>(game: &'g Game<'_>, id: u32) -> Option<&'g Mob> {
    index_of(game, id).map(|i| &game.mobs[i])
}

pub(super) fn mob_mut<'g>(game: &'g mut Game<'_>, id: u32) -> Option<&'g mut Mob> {
    index_of(game, id).map(|i| &mut game.mobs[i])
}
