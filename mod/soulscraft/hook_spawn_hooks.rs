fn soulscraft_spawn_hooks(value: bool, level: &crate::game::natural_spawning::GameLevel<'_, '_>) -> bool {
    crate::game::soulscraft_spawn_boost::spawn_hooks(level.game.dim.id(), value)
}
