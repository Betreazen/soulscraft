// Iteration 3's self-test scenarios (soulscraft_selftest run trophy_|reward_|kin_|boost_), on the
// altar's site (soulscraft_selftest_altar: its platform, its core, its cleanup): trophy blocks and
// their block entities, the state a placed head or diorama takes, a wall head, the dioramas'
// particles at their rate, the reward of a seeded win, the kin banners in a wave, and the spawn
// boost's sector, its drift and its extra spawn list.
use crate::game::Game;
use crate::game::soulscraft_altar as altar;
use crate::game::soulscraft_event::{self as event, Overrides};
use crate::game::soulscraft_selftest_altar::{Case, DIM, at, build, golem, has, items_near, level, set, start};
use crate::game::soulscraft_spawn_boost as boost;
use crate::game::soulscraft_trophy as trophy;
use horizon_client::biome::SpawnEntry;
use soulscraft_rules::{
    altar::{self as ra, AltarFile},
    event::{self as rev, Rng, SpawnMob, Trophy},
    trophy as rt,
};

/// The scenarios and their timeouts in ticks.
pub(super) const CASES: &[(&str, u64)] = &[
    ("trophy_blocks", 30),
    ("trophy_placement", 20),
    ("trophy_wall_head", 20),
    ("trophy_particles", 260),
    ("reward_seeded_win", 300),
    ("kin_wave", 60),
    ("boost_sector", 60),
];

const MOBS: [&str; 7] = ["revenant", "stalker", "marksman", "brute", "shrike", "lurker", "bog_slime"];
const SEED: u64 = 0x50_17C0;
const PARTICLE_TICKS: u64 = 200;

pub(super) fn step(game: &mut Game<'_>, now: u64, t: u64, name: &str, c: &mut Case) -> Option<Result<(), String>> {
    match name {
        "trophy_blocks" => blocks(game, t),
        "trophy_placement" => placement(game, t),
        "trophy_wall_head" => wall_head(game, t),
        "trophy_particles" => particles(game, t, c),
        "reward_seeded_win" => seeded_win(game, now, t, c),
        "kin_wave" => kin_wave(game, now, t, c),
        "boost_sector" => boost_sector(game, now, t, c),
        other => crate::game::soulscraft_selftest_book::step(game, now, t, other, c),
    }
}

fn prop(game: &Game<'_>, c: [i32; 3], key: &str) -> Option<String> {
    game.native_block_property(at(c), key)
}

/// Every mob's diorama, floor head and wall head placed: their states, their block entities and
/// each diorama's light (its trophy file's).
fn blocks(game: &mut Game<'_>, t: u64) -> Option<Result<(), String>> {
    if t == 1 {
        for (i, m) in MOBS.iter().enumerate() {
            let x = 3 + 2 * i as i32;
            set(game, [x, 0, -6], &format!("soulscraft:{m}_trophy[facing=east]"));
            set(game, [x, 0, -8], &format!("soulscraft:{m}_head_stand[rotation=5]"));
            set(game, [x, 0, -10], &format!("soulscraft:{m}_head_mount[facing=south]"));
        }
        return None;
    }
    if t < 4 {
        return None;
    }
    for (i, m) in MOBS.iter().enumerate() {
        let x = 3 + 2 * i as i32;
        for (cell, block, key, want) in [
            ([x, 0, -6], format!("soulscraft:{m}_trophy"), "facing", "east"),
            ([x, 0, -8], format!("soulscraft:{m}_head_stand"), "rotation", "5"),
            ([x, 0, -10], format!("soulscraft:{m}_head_mount"), "facing", "south"),
        ] {
            let here = game.native_block(at(cell));
            if here != block {
                return Some(Err(format!("{block}: the block at {cell:?} is {here}")));
            }
            if prop(game, cell, key).as_deref() != Some(want) {
                return Some(Err(format!("{block}: {key} {:?}, wanted {want}", prop(game, cell, key))));
            }
            let entity = game.native_mod_block_entity(at(cell)).map(|e| e.id.to_string());
            if entity.as_deref() != Some(block.as_str()) {
                return Some(Err(format!("{block}: block entity {entity:?}")));
            }
        }
        let p = at([x, 0, -6]);
        let state = game.world.block(p[0], p[1], p[2]);
        let light = game.res.facts().get(state).emission;
        let want = rt::embedded(m).map_or(0, |f| f.diorama.light);
        if light != want {
            return Some(Err(format!("{m}_trophy light {light}, its file says {want}")));
        }
    }
    Some(Ok(()))
}

/// The engine's placement hook asked for a head and a diorama as a player's `BlockItem.place`
/// asks it: the head turns to the placer (rotation 12 for a placer looking west, yaw 90), the
/// diorama's plaque too (facing east).
fn placement(game: &mut Game<'_>, t: u64) -> Option<Result<(), String>> {
    if t == 1 {
        set(game, [3, 0, -6], "soulscraft:brute_head_stand");
        set(game, [5, 0, -6], "soulscraft:brute_trophy");
        return None;
    }
    if t < 3 {
        return None;
    }
    let mut got = Vec::new();
    for (cell, key) in [([3, 0, -6], "rotation"), ([5, 0, -6], "facing")] {
        let p = at(cell);
        let state = game.world.block(p[0], p[1], p[2]);
        let ctx = horizon_world::level::PlacementContext {
            pos: horizon_core::position::BlockPos::new(p[0], p[1], p[2]),
            hit_pos: horizon_core::position::BlockPos::new(p[0], p[1] - 1, p[2]),
            face: horizon_core::geometry::Direction::Up,
            hit: horizon_core::geometry::Vec3::new(f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5),
            inside: false,
            yaw: 90.0,
            pitch: 30.0,
            sneaking: false,
            item: game.native_block(p),
            replace_clicked: false,
        };
        let answer = game.with_level(|_, level, _| {
            let mut answer = None;
            level.native_state_for_placement(level.dimension, state, &ctx, horizon_world::fluid::FluidState::EMPTY, &mut answer);
            answer
        });
        let Some(Some(chosen)) = answer else {
            return Some(Err(format!("{key}: the hook gave {answer:?}")));
        };
        let value = game.pack.assets.registry.state(chosen).and_then(|s| s.properties.get(key).cloned());
        got.push(value.unwrap_or_default());
    }
    let want = [rt::floor_rotation(90.0).to_string(), rt::facing_toward(90.0).to_owned()];
    Some(if got == want { Ok(()) } else { Err(format!("placed {got:?}, wanted {want:?}")) })
}

/// A head put on a wall as a player's click on a block's side puts it: the wall head facing out,
/// with its block entity.
fn wall_head(game: &mut Game<'_>, t: u64) -> Option<Result<(), String>> {
    if t == 1 {
        set(game, [4, 0, -6], "stone");
        set(game, [4, 1, -6], "stone");
        let face = [1, 0, 0];
        let wall = at([4, 1, -6]);
        trophy::put_wall_head(DIM, [wall[0] + face[0], wall[1], wall[2]], "stalker", rt::wall_facing(face).unwrap_or("north"));
        return None;
    }
    if t < 4 {
        return None;
    }
    let cell = [5, 1, -6];
    let block = game.native_block(at(cell));
    let facing = prop(game, cell, "facing");
    let entity = game.native_mod_block_entity(at(cell)).is_some();
    Some(if block == "soulscraft:stalker_head_mount" && facing.as_deref() == Some("east") && entity {
        Ok(())
    } else {
        Err(format!("{block} facing {facing:?} block entity {entity}"))
    })
}

/// A revenant's diorama sends its own particles on the server at its file's rates: mist each 6
/// ticks and wisps each 12 on average, so about 50 over 200 ticks (seeded rolls).
fn particles(game: &mut Game<'_>, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let cell = [6, 0, -6];
    match t {
        1 => {
            set(game, cell, "soulscraft:revenant_trophy[facing=south]");
            trophy::seed(SEED);
            None
        }
        5 => {
            trophy::take_sent(at(cell));
            c.count = 0;
            None
        }
        t if t == 5 + PARTICLE_TICKS => {
            let sent = trophy::take_sent(at(cell)) as f64;
            let rows = rt::embedded("revenant").map(|f| f.diorama.particles).unwrap_or_default();
            let want: f64 = rows.iter().map(|e| PARTICLE_TICKS as f64 / f64::from(e.every)).sum();
            log::info!("soulscraft: selftest particles sent={sent} want={want:.1} over {PARTICLE_TICKS} ticks");
            Some(if want > 0.0 && (sent - want).abs() <= want * 0.4 {
                Ok(())
            } else {
                Err(format!("{sent} particles in {PARTICLE_TICKS} ticks, the file's rate gives {want:.1}"))
            })
        }
        _ => None,
    }
}

/// A seeded win on a level-3 altar with two «harder» banners (weight, horde) of a wave with a
/// revenant and a zombie: what drops over the core is exactly what soulscraft_rules::event::rewards
/// rolls with that seed, and it has three heads and the revenant's diorama.
fn seeded_win(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        build(game, 3);
        set(game, [-2, 1, 0], "soulscraft:banner_weight");
        set(game, [2, 1, 0], "soulscraft:banner_horde");
        return None;
    }
    if t < 6 {
        return None;
    }
    if t == 6 {
        let (lv, _, _, banners) = level(game, now)?;
        if lv != 3 || banners.len() != 2 {
            return Some(Err(format!("altar level {lv}, banners {banners:?}")));
        }
        c.golem = golem(game, crate::game::soulscraft_selftest_altar::STAND_IN);
        let data = crate::game::soulscraft_server::state().data.clone();
        let revenant = data.mob("revenant").map(SpawnMob::of)?;
        let ov = Overrides {
            waves: Some(vec![vec![revenant, SpawnMob::vanilla("minecraft:zombie")]]),
            telegraph_ticks: Some(5),
            rest_ticks: Some(5),
            power: Some(0.0),
            seed: Some(SEED),
            stand_ins: c.golem.into_iter().collect(),
            ..Overrides::default()
        };
        return start(game, now, ov).err().map(Err);
    }
    let rec = event::records();
    if has(&rec, "won") {
        if c.phase == 0 {
            // The items land over the core the tick they are summoned: look one tick later.
            c.phase = 1;
            return None;
        }
        let file = AltarFile::embedded();
        let effects = ra::effects(&file, &["weight".to_owned(), "horde".to_owned()]);
        let data = crate::game::soulscraft_server::state().data.clone();
        let kinds = vec!["minecraft:zombie".to_owned(), "soulscraft:revenant".to_owned()];
        let want = rev::rewards(&kinds, &data, &effects, 3, &rev::TrophyChances::embedded(), &mut Rng::new(SEED));
        let mut counts: Vec<(String, u32)> = Vec::new();
        for item in want.iter().filter_map(Trophy::item) {
            match counts.iter_mut().find(|x| x.0 == item) {
                Some(x) => x.1 += 1,
                None => counts.push((item, 1)),
            }
        }
        let heads: usize = want.iter().filter(|x| matches!(x, Trophy::Head(_))).count();
        let diorama = want.contains(&Trophy::Diorama("soulscraft:revenant".into()));
        for (item, n) in &counts {
            let got = items_near(game, [0, 1, 0], item, 4.0);
            if got != *n {
                return Some(Err(format!("{item}: {got} dropped, the seeded roll gives {n} (all: {counts:?})")));
            }
        }
        return Some(if heads == 3 && diorama { Ok(()) } else { Err(format!("roll {want:?}: heads {heads}, diorama {diorama}")) });
    }
    for id in event::wave_ids(DIM, crate::game::soulscraft_selftest_altar::CORE) {
        game.native_kill_mob(DIM, id);
    }
    None
}

/// A kin banner (undead) on a level-2 altar: a quarter of each wave of the test event (two of
/// wave 1's six zombies) comes as SoulsCraft's undead, and the menu sums it.
fn kin_wave(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    if t == 1 {
        build(game, 2);
        set(game, [-2, 1, 0], "soulscraft:banner_kin_undead");
        return None;
    }
    if t < 6 {
        return None;
    }
    if t == 6 {
        let (lv, _, _, banners) = level(game, now)?;
        if lv != 2 || banners != ["kin_undead"] {
            return Some(Err(format!("altar level {lv}, banners {banners:?}")));
        }
        c.golem = golem(game, crate::game::soulscraft_selftest_altar::STAND_IN);
        let ov = Overrides { telegraph_ticks: Some(400), power: Some(0.0), stand_ins: c.golem.into_iter().collect(), ..Overrides::default() };
        return start(game, now, ov).err().map(Err);
    }
    let core = crate::game::soulscraft_selftest_altar::CORE;
    let waves = event::events().get(&(DIM.to_owned(), core)).map(|r| r.plan.waves.clone())?;
    let ours: Vec<String> = waves[0].iter().filter(|m| m.kind.starts_with("soulscraft:")).map(|m| m.kind.clone()).collect();
    let menu = altar::altar(DIM, core).map(|a| crate::game::soulscraft_altar_ui::menu_of(&a, now).effects).unwrap_or_default();
    let menu_ok = menu.iter().any(|l| l.starts_with("род: нежить — 25%"));
    Some(if ours == ["soulscraft:shrike", "soulscraft:revenant"] && waves[0].len() == 6 && menu_ok {
        Ok(())
    } else {
        Err(format!("wave 1 SoulsCraft mobs {ours:?} of {}, menu {menu:?}", waves[0].len()))
    })
}

/// A level-2 altar's sector: active with one extra pass, three chunks out after three drift
/// steps, its chunks among natural spawning's candidates coming again (outside chunks not), the
/// spawn list's hook on in its dimension, and a monster position in it getting the other biomes'
/// monsters and SoulsCraft's mobs through the engine's own hook point (outside: unchanged).
fn boost_sector(game: &mut Game<'_>, now: u64, t: u64, c: &mut Case) -> Option<Result<(), String>> {
    let core = crate::game::soulscraft_selftest_altar::CORE;
    if t == 1 {
        build(game, 2);
        return None;
    }
    if t < 6 {
        return None;
    }
    if t == 6 {
        let (lv, ..) = level(game, now)?;
        if lv != 2 {
            return Some(Err(format!("altar level {lv}")));
        }
        let def = soulscraft_rules::boost::BoostDef::embedded();
        // A fresh world is younger than the drift: since may lie before tick 0.
        altar::set_since(game, DIM, core, now as i64 - (3 * def.drift_ticks) as i64);
        return None;
    }
    if t < 9 {
        return None;
    }
    // After the checks: the switch off in the menu, and no sector two ticks later.
    if c.phase == 1 {
        c.phase = 2;
        return None;
    }
    if c.phase == 2 {
        return Some(if boost::sectors(DIM).is_empty() { Ok(()) } else { Err("the switch is off, the sector stays".into()) });
    }
    let list = boost::sectors(DIM);
    let Some(b) = list.iter().find(|b| b.altar == (core[0].div_euclid(16), core[2].div_euclid(16))) else {
        return Some(Err(format!("no active sector: {list:?}")));
    };
    let dist = (b.centre.0 - b.altar.0).abs().max((b.centre.1 - b.altar.1).abs());
    if b.passes != 1 || dist != 3 {
        return Some(Err(format!("sector {b:?}: passes {}, {dist} chunks out, wanted 1 and 3", b.passes)));
    }
    let inside = b.centre;
    let outside = (b.centre.0 + 5, b.centre.1 + 5);
    let extra = boost::extra_for(DIM, &[inside, outside, (inside.0 + 1, inside.1)]);
    if extra != [inside, (inside.0 + 1, inside.1)] {
        return Some(Err(format!("extra candidates {extra:?}")));
    }
    if !boost::spawn_hooks(DIM, false) {
        return Some(Err("the spawn list hook stays off by an active sector".into()));
    }
    let base = || vec![SpawnEntry { entity: "minecraft:zombie".into(), weight: 95, min: 4, max: 4, constant: true }];
    let block = |ch: (i32, i32)| [ch.0 * 16 + 8, 70, ch.1 * 16 + 8];
    let mut there = base();
    boost::ask_engine(game, DIM, block(inside), &mut there);
    let mut away = base();
    boost::ask_engine(game, DIM, block(outside), &mut away);
    let has_kind = |l: &[SpawnEntry], k: &str| l.iter().any(|e| e.entity == k);
    let ok = has_kind(&there, "minecraft:husk") && has_kind(&there, "soulscraft:revenant") && away.len() == 1;
    if !ok {
        return Some(Err(format!("in the sector {} entries, outside {}", there.len(), away.len())));
    }
    altar::set_boost(DIM, core, false);
    c.phase = 1;
    None
}
