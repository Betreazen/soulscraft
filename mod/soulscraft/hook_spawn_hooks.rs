fn soulscraft_spawn_hooks(value: bool, level: &crate::game::natural_spawning::GameLevel<'_, '_>) -> bool {
    let dim = level.game.dim.id();
    dim == "minecraft:overworld" || crate::game::soulscraft_spawn_boost::spawn_hooks(dim, value)
}
