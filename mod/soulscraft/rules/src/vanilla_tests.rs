use super::*;
use crate::data::{EMBEDDED, MobsFile};

fn group(id: &str) -> VanillaDef {
    MobsFile::embedded().vanilla.groups.into_iter().find(|g| g.id == id).unwrap()
}

/// The group with these parameters set (a chance of 1 makes a decision certain).
fn with(id: &str, params: &[(&str, f64)]) -> VanillaDef {
    let mut g = group(id);
    for (k, v) in params {
        g.params.insert((*k).to_owned(), serde_json::json!(v));
    }
    g
}

/// A mob at the origin facing south (+z), its target `d` blocks south.
fn senses(now: u64, d: f64) -> VSenses {
    VSenses { now, pos: [0.0; 3], health_frac: 1.0, on_ground: true, target: Some([0.0, 0.0, d]), sees_target: true, ..VSenses::default() }
}

fn run(b: &mut VBrain, g: &VanillaDef, from: u64, ticks: u64, f: impl Fn(u64) -> VSenses) -> Vec<VTick> {
    (from..from + ticks).map(|now| b.tick(g, 10, &f(now))).collect()
}

fn starts(ticks: &[VTick]) -> Vec<(usize, usize)> {
    ticks
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            t.events.iter().filter_map(move |e| match e {
                Event::AttackStart { attack } => Some((i, *attack)),
                _ => None,
            })
        })
        .collect()
}

fn strike_ticks(ticks: &[VTick]) -> Vec<usize> {
    ticks.iter().enumerate().filter(|(_, t)| !t.strikes.is_empty()).map(|(i, _)| i).collect()
}

#[test]
fn the_data_names_nineteen_types_in_eleven_rows() {
    let v = MobsFile::embedded().vanilla;
    assert_eq!(v.groups.len(), 11);
    assert_eq!(v.groups.iter().map(|g| g.types.len()).sum::<usize>(), 19);
    assert!(v.untouched.iter().any(|t| t == "minecraft:creeper"));
    assert_eq!(v.spawner_radius_blocks, 5);
    let (_, z) = v.group_of("minecraft:husk").unwrap();
    assert_eq!(z.behavior(), Behavior::Brawler);
    assert!(v.group_of("minecraft:creeper").is_none());
    assert_eq!(v.role_of("minecraft:skeleton"), Some("ranged"));
    assert_eq!(v.role_of("minecraft:zombie"), Some("assault"));
    assert_eq!(v.role_of("minecraft:enderman"), None);
}

#[test]
fn windups_are_the_design_ticks() {
    // 07-vanilla-mobs.md, the table: the numbers the owner approved.
    let windup = |g: &str, a: &str| {
        let d = group(g);
        let a = &d.attacks[d.attack_index(a).unwrap()].def;
        (a.windup, a.hold)
    };
    assert_eq!(windup("zombie", "lunge"), (10, 0));
    assert_eq!(windup("zombie", "trident_throw"), (14, 0));
    assert_eq!(windup("spider", "pounce"), (8, 0));
    assert_eq!(windup("witch", "potion_throw"), (12, 0));
    assert_eq!(windup("vindicator", "overhead"), (10, 6));
    assert_eq!(windup("piglin_brute", "heavy_strike"), (12, 0));
    assert_eq!(windup("hoglin", "ram"), (10, 0));
    assert_eq!(windup("slime", "high_jump"), (10, 0));
    assert_eq!(windup("phantom", "cry"), (10, 0));
    let brute = group("piglin_brute");
    assert_eq!(brute.attacks[0].def.recover, 15);
    assert_eq!(group("hoglin").attacks[0].def.recover, 20);
    let sk = group("skeleton");
    assert_eq!((sk.param("retreat_below_blocks"), sk.param("shots_before_pause"), sk.param("pause_ticks")), (Some(4.0), Some(3.0), Some(20.0)));
}

#[test]
fn bad_vanilla_data_is_refused() {
    let creeper = EMBEDDED.replacen("\"minecraft:witch\"]", "\"minecraft:witch\", \"minecraft:creeper\"]", 1);
    assert_ne!(creeper, EMBEDDED);
    assert!(MobsFile::parse(&creeper).unwrap_err().contains("creeper"));
    let twice = EMBEDDED.replacen("\"types\": [\"minecraft:pillager\"]", "\"types\": [\"minecraft:pillager\", \"minecraft:witch\"]", 1);
    assert!(MobsFile::parse(&twice).unwrap_err().contains("two groups"));
    let role = EMBEDDED.replacen("\"behavior\": \"witch\",\n        \"undead\": [],\n        \"wave_role\": \"ranged\"", "\"behavior\": \"witch\",\n        \"undead\": [],\n        \"wave_role\": \"sniper\"", 1);
    assert_ne!(role, EMBEDDED);
    assert!(MobsFile::parse(&role).unwrap_err().contains("wave_role"));
    let behaviour = EMBEDDED.replacen("\"behavior\": \"ram\"", "\"behavior\": \"rammer\"", 1);
    assert!(MobsFile::parse(&behaviour).unwrap_err().contains("behavior"));
    let only = EMBEDDED.replacen("\"only\": [\"minecraft:drowned\"]", "\"only\": [\"minecraft:guardian\"]", 1);
    assert!(MobsFile::parse(&only).unwrap_err().contains("only"));
    let lost = EMBEDDED.replacen("\"id\": \"ram\"", "\"id\": \"charge\"", 1);
    assert!(MobsFile::parse(&lost).unwrap_err().contains("needs the attack ram"));
    let undead = EMBEDDED.replacen("\"undead\": [\"minecraft:zoglin\"]", "\"undead\": [\"minecraft:zombie\"]", 1);
    assert!(MobsFile::parse(&undead).unwrap_err().contains("undead"));
}

#[test]
fn cues_fill_the_type_name_and_overrides() {
    let hog = group("hoglin");
    assert_eq!(hog.attacks[0].cue.for_type("minecraft:zoglin").sound.as_deref(), Some("minecraft:entity.zoglin.angry"));
    let slime = group("slime");
    let magma = slime.attacks[0].cue.for_type("minecraft:magma_cube");
    assert_eq!(magma.particle.as_deref(), Some("minecraft:flame"));
    assert_eq!(magma.sound.as_deref(), Some("minecraft:entity.magma_cube.squish"));
    assert_eq!(magma.hop, Some(0.2));
    assert_eq!(slime.attacks[0].cue.for_type("minecraft:slime").particle.as_deref(), Some("minecraft:item_slime"));
    assert!(group("vindicator").attacks[0].cue.aggressive());
}

/// Each move forced: control while it runs, its strike on the data's hit tick (or a dash's first
/// contact), the cue on tick 0, and the phases on their ticks.
#[test]
fn every_vanilla_move_lands_on_its_data_tick() {
    for g in MobsFile::embedded().vanilla.groups {
        for (i, va) in g.attacks.iter().enumerate() {
            let kind = va.only.first().cloned().unwrap_or_else(|| g.types[0].clone());
            let mut b = VBrain::new(&g, &kind, 7, 0);
            b.force(i);
            let a = &va.def;
            let line = Timeline::of(a);
            // In reach for a blow; the target stands just ahead for a dash, whose contact is at once.
            let d = if a.dash_blocks.is_some() { 1.2 } else if a.projectile.is_some() { 8.0 } else { 1.5 };
            let ticks = run(&mut b, &g, 100, u64::from(line.end()) + 2, |now| VSenses { size: 2, ..senses(now, d) });
            assert_eq!(starts(&ticks)[0], (0, i), "{} {}", g.id, a.id);
            assert_eq!(ticks[0].cue, Some(i), "{} {}", g.id, a.id);
            let phantom = g.behavior() == Behavior::Phantom;
            let ended = ticks.iter().position(|t| t.events.iter().any(|e| matches!(e, Event::AttackEnd { attack, .. } if *attack == i)));
            let ended = ended.unwrap_or_else(|| panic!("{} {} never ended", g.id, a.id));
            // A cube recovers from its landing (3 ticks after the leap here), not from the hit tick.
            let late = if g.behavior() == Behavior::Cube { 1 } else { 0 };
            assert!(ended <= line.end() as usize + late, "{} {} ended late", g.id, a.id);
            assert!(ticks[..ended].iter().all(|t| t.control != phantom), "{} {}: control", g.id, a.id);
            let hit = line.hit_start() as usize;
            let s = strike_ticks(&ticks);
            match g.behavior() {
                Behavior::Phantom => {
                    assert!(s.is_empty());
                    assert!(ticks[..hit].iter().all(|t| t.hold_dive));
                    assert!(ticks[hit].release_dive && !ticks[hit].hold_dive);
                }
                // The cube's blow is its landing (on ground at once in this test: 3 ticks up).
                Behavior::Cube => assert_eq!(s, vec![hit + 3], "{}", g.id),
                _ => {
                    let want: Vec<usize> = match &a.strikes_at {
                        Some(list) => list.iter().map(|x| hit + *x as usize).collect(),
                        None => vec![hit],
                    };
                    assert_eq!(s, want, "{} {}", g.id, a.id);
                    let st = &ticks[hit].strikes[0];
                    assert_eq!(st.ranged, a.projectile.is_some(), "{} {}", g.id, a.id);
                    assert_eq!(st.hits_target, a.projectile.is_none(), "{} {}", g.id, a.id);
                }
            }
            if a.hold > 0 {
                assert!(ticks[a.windup as usize].look_at.is_none(), "{} holds without tracking", g.id);
            }
        }
    }
}

#[test]
fn a_zombie_lunges_from_its_range_and_not_when_slowed() {
    let g = with("zombie", &[("lunge_chance", 1.0)]);
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    let ticks = run(&mut b, &g, 0, 4, |now| senses(now, 4.0));
    assert_eq!(starts(&ticks), vec![(0, 0)]);
    assert!(ticks[0].control && ticks[0].order == Order::Stay);
    assert_eq!(ticks[0].aggressive, Some(true));
    // PCT's stun (slowness): no new lunge.
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    let ticks = run(&mut b, &g, 0, 40, |now| VSenses { slowed: true, ..senses(now, 4.0) });
    assert!(starts(&ticks).is_empty());
    // Too close or too far: vanilla's own melee and walk.
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    let ticks = run(&mut b, &g, 0, 40, |now| senses(now, 1.5));
    assert!(starts(&ticks).is_empty() && ticks.iter().all(|t| !t.control));
}

#[test]
fn a_lunge_dashes_its_blocks_over_the_hit_phase() {
    let g = group("zombie");
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    b.force(0);
    let ticks = run(&mut b, &g, 0, 16, |now| senses(now, 4.0));
    let Order::Dash { dir, per_tick } = ticks[10].order else { panic!("{:?}", ticks[10].order) };
    assert_eq!(dir, [0.0, 0.0, 1.0]);
    assert!((per_tick - 0.75).abs() < 1e-9);
    // Out of contact the whole dash: no blow, then the recovery stands.
    assert!(strike_ticks(&ticks).is_empty());
    assert_eq!(ticks[14].order, Order::Stay);
}

#[test]
fn a_zombie_steps_back_after_its_blow() {
    let g = with("zombie", &[("step_back_chance", 1.0), ("lunge_chance", 0.0)]);
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    let ticks = run(&mut b, &g, 0, 14, |now| VSenses { swung: now == 0, ..senses(now, 1.2) });
    assert!(ticks[0].events.contains(&Event::Mode("step_back")));
    assert!(ticks[..10].iter().all(|t| t.control && matches!(t.order, Order::Walk { to, .. } if to[2] < 0.0)));
    assert!(!ticks[11].control);
}

#[test]
fn a_third_zombie_comes_round_the_side() {
    let g = with("zombie", &[("lunge_chance", 0.0)]);
    let leader = [0.0, 0.0, 0.0];
    let target = [0.0, 0.0, 8.0];
    let at = |index, count, now| VSenses {
        target: Some(target),
        flank: Some(Flank { index, count, leader }),
        ..senses(now, 8.0)
    };
    let mut b = VBrain::new(&g, "minecraft:zombie", 1, 0);
    let t = b.tick(&g, 10, &at(1, 3, 0));
    let Order::Walk { to, .. } = t.order else { panic!("{:?}", t.order) };
    assert!(t.control);
    // 70° round from the leader's side of the target, 3 blocks out.
    assert!((geom::flat_dist(to, target) - 3.0).abs() < 1e-9);
    assert!(to[0] < -2.0, "{to:?}");
    let t2 = VBrain::new(&g, "minecraft:zombie", 2, 0).tick(&g, 10, &at(2, 3, 0));
    let Order::Walk { to: to2, .. } = t2.order else { panic!() };
    assert!(to2[0] > 2.0, "the next goes the other way: {to2:?}");
    // The leader and a pair: vanilla.
    assert!(!VBrain::new(&g, "minecraft:zombie", 1, 0).tick(&g, 10, &at(0, 3, 0)).control);
    assert!(!VBrain::new(&g, "minecraft:zombie", 1, 0).tick(&g, 10, &at(1, 2, 0)).control);
}

#[test]
fn only_the_drowned_throws_a_trident() {
    let g = group("zombie");
    let throw = |kind: &str, trident: bool| {
        let mut b = VBrain::new(&g, kind, 3, 0);
        let ticks = run(&mut b, &g, 0, 30, |now| VSenses { holding_trident: trident, ..senses(now, 9.0) });
        (starts(&ticks), ticks[0].control)
    };
    assert_eq!(throw("minecraft:drowned", true), (vec![(0, 1)], true));
    let (s, control) = throw("minecraft:drowned", false);
    assert!(!s.contains(&(0, 1)) && !control);
    let (s, _) = throw("minecraft:zombie", true);
    assert!(s.iter().all(|x| x.1 != 1));
}

#[test]
fn a_skeleton_backs_off_climbs_and_pauses_after_three_arrows() {
    let g = group("skeleton");
    let mut b = VBrain::new(&g, "minecraft:skeleton", 1, 0);
    let t = b.tick(&g, 10, &senses(0, 3.0));
    assert!(t.control && matches!(t.order, Order::Walk { to, .. } if to[2] < 0.0));
    assert!(t.events.contains(&Event::Mode("retreat")));
    // Three bow releases, then 20 ticks of pause, then vanilla again.
    let mut b = VBrain::new(&g, "minecraft:skeleton", 1, 0);
    let using = |now: u64| matches!(now, 1..=5 | 8..=12 | 15..=19);
    let ticks = run(&mut b, &g, 0, 45, |now| VSenses { using_item: using(now), ..senses(now, 10.0) });
    assert!(ticks[..20].iter().all(|t| !t.control));
    assert!(ticks[20].events.contains(&Event::Mode("pause")));
    assert!(ticks[20..40].iter().all(|t| t.control && t.order == Order::Stay));
    assert!(!ticks[41].control);
    // Below its target: it asks for a high spot, walks there, and is free on arrival.
    let mut b = VBrain::new(&g, "minecraft:skeleton", 1, 0);
    let low = |now: u64| VSenses { target: Some([0.0, 0.0, 10.0]), ..senses(now, 10.0) };
    let ask = run(&mut b, &g, 100, 1, low);
    assert_eq!(ask[0].want, Some(Want::HighSpot));
    let spot = [3.0, 3.0, 2.0];
    let t = b.tick(&g, 10, &VSenses { answer: Some(Some(spot)), ..low(101) });
    assert!(t.control && t.order == Order::Walk { to: spot, speed: 1.1 });
    let t = b.tick(&g, 10, &VSenses { pos: spot, ..low(130) });
    assert!(!t.control);
}

#[test]
fn a_skeleton_backs_off_again_only_after_a_shot() {
    // Taking a skeleton over stops vanilla's bow goal, which walks in again for its first 20 ticks:
    // a retreat on every close tick kept it from ever shooting (the vslow self-test, 2026-11-15).
    let g = group("skeleton");
    let mut b = VBrain::new(&g, "minecraft:skeleton", 1, 0);
    assert!(b.tick(&g, 10, &senses(0, 3.0)).control, "the first retreat");
    assert!(!b.tick(&g, 10, &senses(1, 8.0)).control, "far enough: free");
    // Close again with no arrow loosed: vanilla's goal keeps it, so it draws and shoots.
    assert!(!b.tick(&g, 10, &senses(2, 3.0)).control);
    // A draw cut short by the take-over is not an arrow.
    let close = |now: u64, using: bool| VSenses { using_item: using, ..senses(now, 3.0) };
    assert!(!b.tick(&g, 10, &close(3, true)).control);
    let t = b.tick(&g, 10, &close(4, false));
    assert_eq!(b.shots(), 1, "released: one arrow");
    assert!(t.control && t.events.contains(&Event::Mode("retreat")), "an arrow re-arms the retreat");
    let mut b = VBrain::new(&g, "minecraft:skeleton", 1, 0);
    b.tick(&g, 10, &VSenses { using_item: true, ..senses(0, 3.0) });
    b.tick(&g, 10, &senses(1, 3.0));
    assert_eq!(b.shots(), 0, "the draw the retreat stopped");
}

#[test]
fn a_spider_waits_above_and_pounces_when_the_player_passes_under() {
    let g = with("spider", &[("pounce_chance", 0.0)]);
    let mut b = VBrain::new(&g, "minecraft:spider", 1, 0);
    let ledge = |now: u64, z: f64| VSenses { pos: [0.0, 3.0, 0.0], target: Some([0.0, 0.0, z]), ..senses(now, z) };
    let t = b.tick(&g, 10, &ledge(0, 6.0));
    assert!(t.control && t.events.contains(&Event::Mode("wait")));
    assert!(t.no_leap);
    let t = b.tick(&g, 10, &ledge(1, 5.0));
    assert!(t.control && t.events.is_empty());
    let t = b.tick(&g, 10, &ledge(2, 2.0));
    assert!(t.events.contains(&Event::AttackStart { attack: 0 }));
    // On level ground the pounce comes from its range.
    let g = with("spider", &[("pounce_chance", 1.0)]);
    let mut b = VBrain::new(&g, "minecraft:spider", 1, 0);
    let ticks = run(&mut b, &g, 0, 10, |now| senses(now, 4.0));
    assert_eq!(starts(&ticks), vec![(0, 0)]);
    let launch = ticks[8].launch.unwrap();
    assert!(launch[2] > 0.1 && (launch[1] - 0.42).abs() < 1e-9);
}

#[test]
fn a_witch_keeps_her_distance_throws_and_hides_to_drink() {
    let g = with("witch", &[]);
    let mut b = VBrain::new(&g, "minecraft:witch", 1, 0);
    let t = b.tick(&g, 10, &senses(0, 3.0));
    assert!(t.control && starts(&[t.clone()]) == vec![(0, 0)]);
    let mut b = VBrain::new(&g, "minecraft:witch", 1, 0);
    let t = b.tick(&g, 10, &VSenses { sees_target: false, ..senses(0, 3.0) });
    assert!(matches!(t.order, Order::Walk { to, .. } if to[2] < 0.0), "{:?}", t.order);
    // Far away: vanilla walks her in.
    assert!(!VBrain::new(&g, "minecraft:witch", 1, 0).tick(&g, 10, &senses(0, 20.0)).control);
    // Hurt: cover asked, found, walked to.
    let mut b = VBrain::new(&g, "minecraft:witch", 1, 0);
    let hurt = |now| VSenses { health_frac: 0.3, sees_target: false, ..senses(now, 8.0) };
    assert_eq!(b.tick(&g, 10, &hurt(0)).want, Some(Want::Cover));
    let spot = [4.0, 0.0, -3.0];
    let t = b.tick(&g, 10, &VSenses { answer: Some(Some(spot)), ..hurt(1) });
    assert!(t.events.contains(&Event::Mode("hide")) && t.order == Order::Walk { to: spot, speed: 1.2 });
    let t = b.tick(&g, 10, &VSenses { pos: spot, ..hurt(40) });
    assert!(t.control && t.order == Order::Stay && t.label == "hide");
}

#[test]
fn a_pillager_reloads_in_cover() {
    let g = group("pillager");
    let mut b = VBrain::new(&g, "minecraft:pillager", 1, 0);
    let ticks = run(&mut b, &g, 0, 11, |now| senses(now, 10.0));
    assert_eq!(starts(&ticks), vec![(0, 0)]);
    assert_eq!(ticks[10].want, Some(Want::Cover));
    assert!(ticks[10].strikes[0].ranged);
    let spot = [5.0, 0.0, -2.0];
    let t = b.tick(&g, 10, &VSenses { answer: Some(Some(spot)), ..senses(12, 10.0) });
    assert_eq!(t.order, Order::Walk { to: spot, speed: 1.1 });
}

#[test]
fn a_brute_misses_into_a_long_window_and_hits_into_a_short_one() {
    let g = group("piglin_brute");
    let a = &g.attacks[0].def;
    let end = |d: f64| {
        let mut b = VBrain::new(&g, "minecraft:piglin_brute", 1, 0);
        b.force(0);
        let ticks = run(&mut b, &g, 0, 40, |now| senses(now, d));
        ticks.iter().position(|t| t.events.iter().any(|e| matches!(e, Event::AttackEnd { .. }))).unwrap()
    };
    assert_eq!(end(1.5), (a.windup + a.hit + 4) as usize);
    assert_eq!(end(3.5), (a.windup + a.hit + a.recover) as usize);
    // Close in, it holds the fight: every blow has its windup.
    let mut b = VBrain::new(&g, "minecraft:piglin_brute", 1, 0);
    let t = b.tick(&g, 10, &senses(0, 2.0));
    assert!(t.control && starts(&[t.clone()]) == vec![(0, 0)]);
}

#[test]
fn a_hoglin_rams_and_a_wither_skeleton_steps_back_after_its_series() {
    let g = with("hoglin", &[("ram_chance", 1.0)]);
    let mut b = VBrain::new(&g, "minecraft:hoglin", 1, 0);
    let ticks = run(&mut b, &g, 0, 12, |now| senses(now, 5.0));
    assert_eq!(starts(&ticks), vec![(0, 0)]);
    assert!(matches!(ticks[10].order, Order::Dash { .. }));
    let g = group("wither_skeleton");
    let mut b = VBrain::new(&g, "minecraft:wither_skeleton", 1, 0);
    b.force(0);
    let a = &g.attacks[0].def;
    let ticks = run(&mut b, &g, 0, u64::from(Timeline::of(a).end()) + 3, |now| senses(now, 1.5));
    assert_eq!(strike_ticks(&ticks), vec![8, 16]);
    let last = Timeline::of(a).end() as usize;
    assert!(ticks[last].events.contains(&Event::Mode("step_back")));
    assert!(ticks[last + 1].control && matches!(ticks[last + 1].order, Order::Walk { .. }));
}

#[test]
fn a_cube_jumps_only_from_size_two() {
    let g = with("slime", &[("jump_chance", 1.0)]);
    let tick = |size| VBrain::new(&g, "minecraft:slime", 1, 0).tick(&g, 10, &VSenses { size, ..senses(0, 5.0) });
    assert!(starts(&[tick(1)]).is_empty());
    assert_eq!(starts(&[tick(2)]), vec![(0, 0)]);
    assert!((landing_radius(&g.attacks[0].def, 4) - 3.0).abs() < 1e-9);
}

#[test]
fn a_cube_holds_its_hit_phase_while_it_flies() {
    let g = group("slime");
    let mut b = VBrain::new(&g, "minecraft:slime", 1, 0);
    b.force(0);
    // Up in the air from the leap (tick 10) until tick 28.
    let ticks = run(&mut b, &g, 0, 45, |now| VSenses { size: 2, on_ground: !(10..28).contains(&now), ..senses(now, 5.0) });
    assert_eq!(strike_ticks(&ticks), vec![28]);
    assert!(ticks[11..28].iter().all(|t| t.label.starts_with("high_jump:hit")), "{:?}", ticks[20].label);
    let end = ticks.iter().position(|t| t.events.iter().any(|e| matches!(e, Event::AttackEnd { .. }))).unwrap();
    assert_eq!(end, 28 + 10);
}

#[test]
fn a_phantom_cries_and_holds_its_dive() {
    let g = group("phantom");
    let mut b = VBrain::new(&g, "minecraft:phantom", 1, 0);
    let ticks = run(&mut b, &g, 0, 12, |now| VSenses { swoop_started: now == 0, ..senses(now, 10.0) });
    assert_eq!(ticks[0].cue, Some(0));
    assert!(ticks[..10].iter().all(|t| t.hold_dive && !t.control));
    assert!(ticks[10].release_dive);
    assert!(!ticks[11].hold_dive);
}

#[test]
fn a_leap_comes_down_near_its_target() {
    // Simulated as vanilla moves a mob in the air: drag 0.91 on the ground plane, gravity 0.08.
    for (d, up) in [(4.0, 0.42), (6.0, 0.75), (2.5, 0.42)] {
        let v = leap_velocity([0.0; 3], [0.0, 0.0, d], up, 8.0);
        let (mut p, mut vel) = ([0.0f64; 3], v);
        for _ in 0..200 {
            p = geom::add(p, vel);
            if p[1] <= 0.0 {
                break;
            }
            vel = [vel[0] * AIR_DRAG, (vel[1] - GRAVITY) * 0.98, vel[2] * AIR_DRAG];
        }
        assert!((p[2] - d).abs() < 0.8, "leap of {d} at {up} came down at {:?}", p);
    }
}
