fn soulscraft_effect_due(effect: &str, tick_count: i32, amplifier: i32, apply: &mut bool) {
    crate::game::soulscraft_server::bleed_due(effect, tick_count, amplifier, apply);
}
