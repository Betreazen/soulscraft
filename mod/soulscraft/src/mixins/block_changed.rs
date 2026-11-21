// Every committed setBlock of an authoritative server: changes to SoulsCraft's blocks and inside
// altar zones are queued for the level's tick. Read-only level; it runs inside our own setblocks
// too, so the module takes only its own queue's lock.
fn soulscraft_block_changed(level: &GameLevel<'_>, pos: BlockPos, old: BlockStateId, new: BlockStateId) {
    if level.native_authoritative {
        crate::game::soulscraft_altar::on_block_changed(level.facts, level.dimension, [pos.x, pos.y, pos.z], old, new);
    }
}
