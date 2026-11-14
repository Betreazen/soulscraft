fn soulscraft_register_commands(commands: &mut Vec<crate::mod_api::NativeCommand>) {
    crate::game::soulscraft_tools::on_register_commands(commands);
    crate::game::soulscraft_book::on_register_commands(commands);
}
