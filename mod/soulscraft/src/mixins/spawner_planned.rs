// The mobs a spawner made (Path-of-craft-tree reads the same ids): a book kill of one counts a quarter.
fn soulscraft_spawner_planned(value: std::collections::HashMap<u32, u32>) -> std::collections::HashMap<u32, u32> {
    crate::game::soulscraft_book::on_spawner_planned(&value);
    value
}
