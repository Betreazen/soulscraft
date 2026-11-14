// The hunter's book's self-test scenarios (soulscraft_selftest run book_), on the altar's site
// (soulscraft_selftest_altar: its platform, its core, its cleanup). The bench has no players, so
// still iron golems stand in for them (soulscraft_book::add_stand_in): their blows, the brains'
// targets and where they stand count as a player's, and their books live in memory. Kills are
// real deaths (`native_hurt_entity` with the golem as the attacker), heard by the death injection
// and credited in the level's tick, as a player's would be.
use crate::game::Game;
use crate::game::soulscraft_book as book;
use crate::game::soulscraft_event as event;
use crate::game::soulscraft_selftest_altar::{Case, DIM, at, build, centre, has, level, start};
use crate::game::soulscraft_server as server;
use soulscraft_rules::{
    book::{self as rb, BookFile, Progress},
    defense::Role,
    event::SpawnMob,
};

/// The scenarios and their timeouts in ticks.
pub(super) const CASES: &[(&str, u64)] = &[
    ("book_stages_and_elite", 80),
    ("book_credit_sharing", 320),
    ("book_spawner_weight", 40),
    ("book_met_trigger", 60),
    ("book_turret_report", 40),
    ("book_persist", 10),
    ("book_altar_levels", 120),
    ("book_event_entries", 400),
];

const A: u128 = 0x5c_b00c_0000_0000_0000_0000_0000_00a1;
const B: u128 = 0x5c_b00c_0000_0000_0000_0000_0000_00b2;
const C: u128 = 0x5c_b00c_0000_0000_0000_0000_0000_00c3;
const K: u128 = 0x5c_b00c_0000_0000_0000_0000_0000_00d4;

pub(super) fn step(game: &mut Game<'_>, now: u64, t: u64, name: &str, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        book::clear_stand_ins();
    }
    let out = match name {
        "book_stages_and_elite" => stages(game, t, c),
        "book_credit_sharing" => sharing(game, t, c),
        "book_spawner_weight" => spawner(game, t, c),
        "book_met_trigger" => met(game, t, c),
        "book_turret_report" => turret(game, t, c),
        "book_persist" => Some(persist()),
        "book_altar_levels" => altar_levels(game, now, t, c),
        "book_event_entries" => event_entries(game, now, t, c),
        other => Some(Err(format!("no scenario {other}"))),
    };
    if out.is_some() {
        book::clear_stand_ins();
    }
    out
}

/// A still golem standing in for player `uuid` at site cell `cell`.
fn stand_in(game: &mut Game<'_>, uuid: u128, name: &str, cell: [i32; 3]) -> Option<u32> {
    let g = crate::game::soulscraft_selftest_altar::golem(game, cell)?;
    book::add_stand_in(uuid, name, g);
    Some(g)
}

/// A mob of ours at site cell `cell`, P 0 and full health; `tags` added.
fn mob(game: &mut Game<'_>, id: &str, cell: [i32; 3], tags: &str) -> Option<u32> {
    let snbt = format!("{{Rotation:[0f,0f],Tags:[\"soulscraft.p=0.00\",\"{}\"{tags}],PersistenceRequired:1b}}", server::TAG_FRESH);
    let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&snbt) else { return None };
    game.native_summon(DIM, &format!("soulscraft:{id}"), centre(at(cell)), nbt)
}

fn hit(game: &mut Game<'_>, mob: u32, by: Option<u32>, amount: f32) -> bool {
    game.native_hurt_entity(DIM, mob, amount, "minecraft:mob_attack", by)
}

fn stage_of(uuid: u128, id: &str) -> (usize, f64) {
    let file = BookFile::embedded();
    let p = book::progress_of(uuid).unwrap_or_default();
    let m = p.mob(id);
    let elite = id == "brute";
    (file.stage(m.met, m.kills, elite), m.kills)
}

/// A batch of `n` mobs of `id` killed by golem `by`, laid out in a row at z `row`.
fn kill_batch(game: &mut Game<'_>, c: &mut Case, id: &str, n: usize, row: i32, by: u32, t: u64, at_t: u64) -> Option<()> {
    if t == at_t {
        c.note = String::new();
        let ids: Vec<String> = (0..n).filter_map(|i| mob(game, id, [-15 + 4 * i as i32, 0, row], "")).map(|x| x.to_string()).collect();
        c.note = ids.join(",");
    } else if t == at_t + 2 {
        for x in c.note.clone().split(',').filter_map(|s| s.parse::<u32>().ok()) {
            hit(game, x, Some(by), 1000.0);
        }
    }
    Some(())
}

/// Revenants 1, 4, 10 kills (stages at 1, 5, 15) and brutes 1, 2, 5 (elite: 1, 3, 8), checked
/// after each batch, with the action-bar lines each new stage gave.
fn stages(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        c.golem = stand_in(game, A, "stand_a", [0, 0, -18]);
        return None;
    }
    let by = c.golem?;
    let plan: [(&str, usize, usize, u64); 6] =
        [("revenant", 1, rb::KILLED, 4), ("revenant", 4, rb::STUDIED, 12), ("revenant", 10, rb::EXPERT, 20), ("brute", 1, rb::KILLED, 28), ("brute", 2, rb::STUDIED, 40), ("brute", 5, rb::EXPERT, 52)];
    for (i, (id, n, want, at_t)) in plan.iter().enumerate() {
        kill_batch(game, c, id, *n, if i % 2 == 0 { 8 } else { 14 }, by, t, *at_t)?;
        if t == at_t + 6 {
            let (stage, kills) = stage_of(A, id);
            if stage != *want {
                return Some(Err(format!("{id} after {kills} kills: stage {stage}, wanted {want}")));
            }
        }
    }
    if t < 60 {
        return None;
    }
    // A hit may be heard before its death: «Встречен» then, before «Убит».
    let lines: Vec<String> = book::lines().into_iter().filter(|l| !l.ends_with("Встречен")).collect();
    let want = [
        "stand_a Книга охотника: Латник-мертвец — Убит",
        "stand_a Книга охотника: Латник-мертвец — Изучен",
        "stand_a Книга охотника: Латник-мертвец — Знаток",
        "stand_a Книга охотника: Громила — Убит",
        "stand_a Книга охотника: Громила — Изучен",
        "stand_a Книга охотника: Громила — Знаток",
    ];
    Some(if lines == want { Ok(()) } else { Err(format!("lines {lines:?}")) })
}

/// The killer far away, a near recent hitter: credited; a hitter 11 s before the kill, a hitter
/// 35 blocks away: not.
fn sharing(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    const M: [i32; 3] = [-18, 0, 0];
    if t == 1 {
        let ids = [
            stand_in(game, A, "stand_a", [-14, 0, 2]),
            stand_in(game, B, "stand_b", [-18, 0, 5]),
            stand_in(game, C, "stand_c", [17, 0, 0]),
            stand_in(game, K, "stand_k", [18, 0, 18]),
        ];
        c.note = ids.iter().map(|x| x.unwrap_or(0).to_string()).collect::<Vec<_>>().join(",");
        c.golem = mob(game, "revenant", M, "");
        return None;
    }
    let g: Vec<u32> = c.note.split(',').filter_map(|s| s.parse().ok()).collect();
    let m = c.golem?;
    match t {
        3 => {
            hit(game, m, Some(g[1]), 1.0);
        }
        215 => {
            hit(game, m, Some(g[0]), 1.0);
        }
        227 => {
            hit(game, m, Some(g[2]), 1.0);
        }
        239 => {
            hit(game, m, Some(g[3]), 1000.0);
        }
        _ => {}
    }
    if t < 245 {
        return None;
    }
    let got: Vec<f64> = [A, B, C, K].iter().map(|u| stage_of(*u, "revenant").1).collect();
    Some(if got == [1.0, 0.0, 0.0, 1.0] {
        Ok(())
    } else {
        Err(format!("kills a,b(old hit),c(far),killer(far) = {got:?}, wanted [1, 0, 0, 1]"))
    })
}

/// Two spawners' revenants (one tagged at its summoning, one by the spawner's ids and the tag the
/// book adds): a quarter each.
fn spawner(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        c.golem = stand_in(game, A, "stand_a", [0, 0, -6]);
        let one = mob(game, "revenant", [-4, 0, 0], &format!(",\"{}\"", rb::SPAWNER_TAG))?;
        let two = mob(game, "revenant", [4, 0, 0], "")?;
        book::mark_spawner(two);
        c.note = format!("{one},{two}");
        return None;
    }
    let ids: Vec<u32> = c.note.split(',').filter_map(|s| s.parse().ok()).collect();
    if t == 4 {
        let tags = game.native_entity(ids[1]).and_then(|e| game.native_entity_tags(e)).unwrap_or_default();
        if !tags.iter().any(|x| x == rb::SPAWNER_TAG) {
            return Some(Err(format!("the spawner's mob was not tagged: {tags:?}")));
        }
        for id in &ids {
            hit(game, *id, c.golem, 1000.0);
        }
    }
    if t < 8 {
        return None;
    }
    let (stage, kills) = stage_of(A, "revenant");
    Some(if kills == 0.5 && stage == rb::MET { Ok(()) } else { Err(format!("two spawner kills gave {kills} (stage {stage}), wanted 0.5 (met)")) })
}

/// A revenant whose brain targets a stand-in 10 blocks away: met; a stalker targeting one 20
/// blocks away: not yet.
fn met(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        let near = stand_in(game, A, "stand_a", [0, 0, 10])?;
        let far = stand_in(game, B, "stand_b", [0, 0, -20])?;
        let r = mob(game, "revenant", [0, 0, 0], "")?;
        let s = mob(game, "stalker", [1, 0, 0], "")?;
        c.note = format!("{r},{near},{s},{far}");
        return None;
    }
    let ids: Vec<u32> = c.note.split(',').filter_map(|s| s.parse().ok()).collect();
    if t <= 4 {
        let mut s = server::state();
        for pair in ids.chunks(2) {
            if let Some(m) = s.mobs.get_mut(&pair[0]) {
                m.target_override = Some(pair[1]);
            }
        }
        return None;
    }
    if t < 7 {
        return None;
    }
    let (near, _) = stage_of(A, "revenant");
    let (far, _) = stage_of(B, "stalker");
    let lines = book::lines();
    Some(if near == rb::MET && far == rb::UNKNOWN && lines.iter().any(|l| l == "stand_a Книга охотника: Латник-мертвец — Встречен") {
        Ok(())
    } else {
        Err(format!("revenant at 10: stage {near}; stalker at 20: stage {far}; lines {lines:?}"))
    })
}

/// RustCraft's turret reports (`pct-machine kill`): the owner within 32 blocks gets the kill, one
/// 35 blocks away does not.
fn turret(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        stand_in(game, A, "stand_a", [-10, 0, 0])?;
        stand_in(game, B, "stand_b", [17, 0, 0])?;
        let one = mob(game, "revenant", [-18, 0, 4], "")?;
        let two = mob(game, "revenant", [-18, 0, -4], "")?;
        c.note = format!("{one},{two}");
        return None;
    }
    let ids: Vec<u32> = c.note.split(',').filter_map(|s| s.parse().ok()).collect();
    if t == 3 {
        for id in &ids {
            hit(game, *id, None, 1000.0);
        }
        book::on_console(&format!("pct-machine kill {A:032x} {} soulscraft:revenant", ids[0]));
        book::on_console(&format!("pct-machine kill {B:032x} {} soulscraft:revenant", ids[1]));
    }
    if t < 8 {
        return None;
    }
    let (a, b) = (stage_of(A, "revenant").1, stage_of(B, "revenant").1);
    Some(if a == 1.0 && b == 0.0 { Ok(()) } else { Err(format!("owner near {a} kills, owner far {b}, wanted 1 and 0")) })
}

/// The progress in a persistent-data compound beside another mod's key, read back (a relog or a
/// restart reads the same compound); writing again replaces only the book's key.
fn persist() -> Result<(), String> {
    use crate::mod_api::Tag;
    let file = BookFile::embedded();
    let catalog = rb::Catalog::embedded();
    let mut p = Progress::default();
    p.kill(&file, &catalog.get("brute").ok_or("no brute")?.0, 3.0);
    p.altar_levels(2);
    let mut data = Tag::Compound(Default::default());
    if let Tag::Compound(m) = &mut data {
        m.insert("pct:summary".into(), Tag::String("{\"v\":1}".into()));
    }
    if !book::put(&mut data, &Progress::default().to_json()) || !book::put(&mut data, &p.to_json()) {
        return Err("not a compound".into());
    }
    let back = book::get(&data).ok_or("no book key")??;
    let Tag::Compound(m) = &data else { return Err("not a compound".into()) };
    let pct = matches!(m.get(&horizon_world::nbt::JavaString::from("pct:summary")), Some(Tag::String(s)) if s.to_string_lossy() == "{\"v\":1}");
    if back != p || !pct || m.len() != 2 {
        return Err(format!("read back {back:?}, pct kept {pct}, keys {}", m.len()));
    }
    if back.stage(&file, &catalog.get("brute").ok_or("no brute")?.0) != rb::STUDIED {
        return Err("the brute's stage did not survive".into());
    }
    if book::get(&Tag::Int(3)).is_some() {
        return Err("a tag that is not a compound read".into());
    }
    Ok(())
}

/// A stand-in by the core sees levels 1 and 2 built; one 20 blocks out sees nothing.
fn altar_levels(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        c.golem = stand_in(game, A, "stand_a", [2, 0, 3]);
        stand_in(game, B, "stand_b", [-19, 0, -19]);
        build(game, 1);
        return None;
    }
    if t == 50 {
        let _ = level(game, now);
        build(game, 2);
    }
    if t < 100 {
        return None;
    }
    let near = book::progress_of(A).unwrap_or_default().altar.levels;
    let far = book::progress_of(B).unwrap_or_default().altar.levels;
    Some(if near.iter().copied().eq([1, 2]) && far.is_empty() { Ok(()) } else { Err(format!("near {near:?}, far {far:?}")) })
}

/// An event with the stand-in in it: «Оборона алтаря» and the description at its start, the waves
/// and reward at its win.
fn event_entries(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let id = server::state().data.events.first().map(|e| e.id.clone())?;
    if t == 1 {
        build(game, 1);
        c.golem = stand_in(game, A, "stand_a", [2, 0, 3]);
        return None;
    }
    if t == 4 {
        level(game, now)?;
        let zombie = SpawnMob { kind: "minecraft:zombie".into(), role: Role::Assault, armored: false, banner: false, health_mult: 1.0, height: 2 };
        let ov = event::Overrides {
            waves: Some(vec![vec![zombie]]),
            telegraph_ticks: Some(10),
            rest_ticks: Some(20),
            power: Some(0.0),
            stand_ins: c.golem.into_iter().collect(),
            ..event::Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    if t < 5 {
        return None;
    }
    let p = book::progress_of(A).unwrap_or_default();
    let ev = p.events.get(&id).copied().unwrap_or_default();
    if c.phase == 0 {
        if p.altar.defense && ev.tried && !ev.won {
            c.phase = 1;
        }
        return None;
    }
    if has(&event::records(), "won") {
        if ev.won {
            return Some(Ok(()));
        }
        c.count += 1;
        return (c.count > 40).then(|| Err(format!("won, but the book has {ev:?}")));
    }
    for x in event::wave_ids(DIM, crate::game::soulscraft_selftest_altar::CORE) {
        game.native_kill_mob(DIM, x);
    }
    None
}
