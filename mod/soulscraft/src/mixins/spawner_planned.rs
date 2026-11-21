fn soulscraft_spawner_planned(value: HashMap<u32, u32>) -> HashMap<u32, u32> {
    crate::game::soulscraft_book::on_spawner_planned(&value);
    value
}
