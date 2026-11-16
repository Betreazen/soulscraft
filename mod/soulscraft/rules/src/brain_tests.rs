use super::*;
use crate::data::{MobDef, MobsFile};

fn mob(id: &str) -> MobDef {
    MobsFile::embedded().mob(id).unwrap().clone()
}

/// A mob at the origin facing south (+z) and a target `d` blocks south of it.
fn senses(now: u64, d: f64) -> Senses {
    Senses {
        now,
        pos: [0.0; 3],
        yaw: 0.0,
        health_frac: 1.0,
        on_ground: true,
        in_water: false,
        wall_hit: false,
        wait_turn: false,
        target: Some(TargetSense { pos: [0.0, 0.0, d], ..TargetSense::default() }),
    }
}

/// Runs `ticks` ticks from `from` with the senses `f` gives each tick; every tick's output.
fn run(b: &mut Brain, m: &MobDef, p: f64, from: u64, ticks: u64, f: impl Fn(u64) -> Senses) -> Vec<Tick> {
    let sc = Scaled::at(&m.stats, p);
    (from..from + ticks).map(|now| b.tick(m, &sc, &f(now))).collect()
}

fn starts(ticks: &[Tick]) -> Vec<(usize, usize)> {
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

fn strikes(ticks: &[Tick]) -> Vec<(usize, Strike)> {
    ticks.iter().enumerate().flat_map(|(i, t)| t.strikes.iter().map(move |s| (i, *s))).collect()
}

#[test]
fn forced_overhead_runs_its_phases_on_the_data_ticks() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 1, 100);
    let o = m.attack_index("overhead").unwrap();
    b.force(o);
    let out = run(&mut b, &m, 0.0, 100, 40, |n| senses(n, 2.0));
    let phases: Vec<(usize, Phase)> = out
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            t.events.iter().filter_map(move |e| match e {
                Event::Phase { attack, phase, .. } if *attack == o => Some((i, *phase)),
                _ => None,
            })
        })
        .collect();
    assert_eq!(
        phases,
        vec![(0, Phase::Windup), (10, Phase::Hold), (18, Phase::Hit), (20, Phase::Recover), (36, Phase::Done)]
    );
    let s = strikes(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].0, 18);
    assert!(s[0].1.hits_target);
    assert_eq!(out[0].label, "overhead:windup 0");
    assert_eq!(b.clip().name, "");
    assert!(out[10].look_at.is_none(), "a held blow stops tracking");
    assert!(!out[5].guarding, "the shield is down while it attacks");
}

#[test]
fn the_blow_misses_a_target_that_stepped_away_during_the_hold() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 1, 0);
    b.force(m.attack_index("overhead").unwrap());
    let out = run(&mut b, &m, 0.0, 0, 30, |n| senses(n, if n < 12 { 2.0 } else { 5.0 }));
    let s = strikes(&out);
    assert_eq!(s.len(), 1, "the blow still falls on its tick");
    assert!(!s[0].1.hits_target);
}

#[test]
fn a_target_behind_the_mob_is_not_hit() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 1, 0);
    b.force(m.attack_index("thrust").unwrap());
    let out = run(&mut b, &m, 0.0, 0, 20, |n| senses(n, -2.0));
    assert!(!strikes(&out)[0].1.hits_target);
}

#[test]
fn revenant_attacks_on_its_first_decision_in_reach() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 7, 0);
    let out = run(&mut b, &m, 0.0, 0, 3, |n| senses(n, 2.0));
    assert_eq!(starts(&out).len(), 1);
    assert_eq!(starts(&out)[0].0, 0);
}

#[test]
fn revenant_walks_up_when_out_of_reach_and_guards() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 7, 0);
    let out = run(&mut b, &m, 0.0, 0, 3, |n| senses(n, 8.0));
    assert!(starts(&out).is_empty());
    assert!(matches!(out[0].order, Order::Walk { .. }));
    assert!(out[0].repath && !out[1].repath);
    assert!(out[0].guarding);
}

#[test]
fn revenant_circles_a_raised_shield_then_bashes_it() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 3, 0);
    let shielded = |n| {
        let mut s = senses(n, 2.0);
        s.target.as_mut().unwrap().shield_up = true;
        s
    };
    let out = run(&mut b, &m, 0.0, 0, 120, shielded);
    let st = starts(&out);
    assert!(!st.is_empty());
    let (when, which) = st[0];
    assert_eq!(which, m.attack_index("shield_bash").unwrap());
    assert!(when >= 60, "waited {when} ticks, at least circle_wait_max_ticks");
    assert!(when <= 60 + 20, "not much past the wait ({when})");
    assert!(out[1..60].iter().all(|t| t.label == "circle"));
    assert_eq!(out[5].order, out[5].order);
    assert!(matches!(out[5].order, Order::Walk { speed, .. } if speed < 1.0));
    assert!(out[0].events.contains(&Event::Mode("circle")));
}

#[test]
fn revenant_attacks_when_the_shield_comes_down() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 3, 0);
    let out = run(&mut b, &m, 0.0, 0, 60, |n| {
        let mut s = senses(n, 2.0);
        s.target.as_mut().unwrap().shield_up = n < 25;
        s
    });
    let st = starts(&out);
    assert!(!st.is_empty());
    assert!(st[0].0 >= 25 && st[0].0 <= 45, "attacked at {}", st[0].0);
    assert_ne!(st[0].1, m.attack_index("shield_bash").unwrap());
}

#[test]
fn revenant_backs_off_after_a_series_of_two() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 11, 0);
    let out = run(&mut b, &m, 1.0, 0, 200, |n| senses(n, 2.0));
    let backoffs: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, t)| t.events.contains(&Event::Mode("backoff")))
        .map(|(i, _)| i)
        .collect();
    assert!(!backoffs.is_empty());
    let ends_before = out[..=backoffs[0]]
        .iter()
        .flat_map(|t| &t.events)
        .filter(|e| matches!(e, Event::AttackEnd { .. }))
        .count();
    assert_eq!(ends_before, 2);
    let i = backoffs[0] + 1;
    match out[i].order {
        Order::Walk { to, .. } => assert!(to[2] < 0.0, "backs away from the target: {to:?}"),
        o => panic!("{o:?}"),
    }
    assert_eq!(out[i].label, "backoff");
}

#[test]
fn decisions_wait_for_their_interval() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 5, 0);
    // Out of reach at first, in reach from tick 3: the next decision is at tick 20 (P = 0).
    let out = run(&mut b, &m, 0.0, 0, 25, |n| senses(n, if n < 3 { 8.0 } else { 2.0 }));
    assert_eq!(starts(&out)[0].0, 20);
}

#[test]
fn stalker_double_slash_strikes_twice() {
    let m = mob("stalker");
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    b.force(m.attack_index("double_slash").unwrap());
    let out = run(&mut b, &m, 0.0, 0, 20, |n| senses(n, 1.5));
    let s: Vec<usize> = strikes(&out).iter().map(|(i, _)| *i).collect();
    assert_eq!(s, vec![6, 9]);
}

#[test]
fn stalker_lunge_dashes_and_lands_once_on_contact() {
    let m = mob("stalker");
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    b.force(m.attack_index("lunge").unwrap());
    // The target is 5 blocks off; the dash closes in a block a tick from tick 12.
    let out = run(&mut b, &m, 0.0, 0, 40, |n| {
        let mut s = senses(n, 5.0);
        s.pos = [0.0, 0.0, if n >= 13 { (n - 12) as f64 } else { 0.0 }];
        s
    });
    match out[12].order {
        Order::Dash { dir, per_tick } => {
            assert!((dir[2] - 1.0).abs() < 1e-9);
            assert!((per_tick - 1.0).abs() < 1e-9);
        }
        o => panic!("{o:?}"),
    }
    let s = strikes(&out);
    assert_eq!(s.len(), 1);
    assert!(s[0].0 >= 13 && s[0].0 < 18);
}

#[test]
fn stalker_flees_when_hurt_and_comes_back() {
    let m = mob("stalker");
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    let out = run(&mut b, &m, 0.0, 0, 230, |n| {
        let mut s = senses(n, 8.0);
        s.health_frac = 0.2;
        s
    });
    assert!(out[0].events.contains(&Event::Mode("flee")));
    assert_eq!(b.clip().name, "");
    assert_eq!(out[10].label, "flee");
    assert!(matches!(out[10].order, Order::Walk { to, speed } if to[2] < 0.0 && speed > 1.0));
    assert!(out[205].label != "flee", "back after return_after_ticks");
}

#[test]
fn marksman_keeps_its_distance_and_shoots_in_range() {
    let m = mob("marksman");
    let mut b = Brain::new(Style::Shooter, 4, 0);
    let near = run(&mut b, &m, 0.0, 0, 2, |n| senses(n, 6.0));
    assert!(matches!(near[0].order, Order::Walk { to, .. } if to[2] < 0.0));
    let mut b = Brain::new(Style::Shooter, 4, 0);
    let out = run(&mut b, &m, 0.0, 0, 50, |n| senses(n, 12.0));
    assert_eq!(starts(&out)[0].1, m.attack_index("aimed_shot").unwrap());
    let s = strikes(&out);
    assert_eq!(s[0].0, 16);
    assert!(s[0].1.ranged && !s[0].1.hits_target);
    assert!(matches!(out[30].order, Order::Walk { .. }), "reloads walking away");
}

#[test]
fn marksman_butts_when_close() {
    let m = mob("marksman");
    let mut b = Brain::new(Style::Shooter, 4, 0);
    let out = run(&mut b, &m, 0.0, 0, 30, |n| senses(n, 2.0));
    assert_eq!(starts(&out)[0].1, m.attack_index("butt_strike").unwrap());
}

#[test]
fn brute_charges_from_afar_and_is_stunned_by_a_wall() {
    let m = mob("brute");
    let charge = m.attack_index("charge").unwrap();
    let mut b = Brain::new(Style::Heavy, 9, 0);
    let wall = run(&mut b, &m, 0.0, 0, 80, |n| {
        let mut s = senses(n, 10.0);
        s.wall_hit = n == 16;
        s
    });
    assert_eq!(starts(&wall)[0].1, charge);
    assert!(matches!(wall[14].order, Order::Dash { .. }));
    assert_eq!(wall[17].order, Order::Stay);
    let end = |out: &[Tick]| {
        out.iter()
            .flat_map(|t| &t.events)
            .find_map(|e| match e {
                Event::AttackEnd { t, .. } => Some(*t),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(end(&wall), 62, "the wall's full window");
    let mut b = Brain::new(Style::Heavy, 9, 0);
    let open = run(&mut b, &m, 0.0, 0, 80, |n| senses(n, 10.0));
    assert_eq!(end(&open), 22 + CHARGE_OPEN_RECOVER);
}

#[test]
fn brute_swings_in_reach_and_rages_when_low() {
    let m = mob("brute");
    let mut b = Brain::new(Style::Heavy, 9, 0);
    let out = run(&mut b, &m, 0.0, 0, 2, |n| {
        let mut s = senses(n, 3.0);
        s.health_frac = 0.2;
        s
    });
    assert!(starts(&out).is_empty(), "it roars first");
    let mut b = Brain::new(Style::Heavy, 9, 0);
    let out = run(&mut b, &m, 0.0, 0, 25, |n| {
        let mut s = senses(n, 3.0);
        s.health_frac = 0.2;
        s
    });
    let (when, a) = starts(&out)[0];
    assert_eq!(when, 20, "after the roar's clip_ticks");
    assert!(a == m.attack_index("wide_swing").unwrap() || a == m.attack_index("overhead_wave").unwrap());
    assert!(out[0].rage);
    assert!((out[0].speed_mult - 1.2).abs() < 1e-9);
    let slam = m.attack("overhead_wave").unwrap();
    let mut b = Brain::new(Style::Heavy, 9, 0);
    b.force(m.attack_index("overhead_wave").unwrap());
    let out = run(&mut b, &m, 0.0, 0, 20, |n| senses(n, 3.0));
    let s = strikes(&out);
    assert_eq!(s[0].0 as u32, slam.windup);
    assert_eq!(s[0].1.area, Some(3.0));
}

#[test]
fn shrike_circles_above_then_dives_and_climbs() {
    let m = mob("shrike");
    let mut b = Brain::new(Style::Diver, 6, 0);
    let above = |n: u64| {
        let mut s = senses(n, 3.0);
        s.pos = [0.0, 10.0, 0.0];
        s
    };
    let out = run(&mut b, &m, 0.0, 0, 70, above);
    assert!(matches!(out[0].order, Order::Steer { to, .. } if to[1] >= 8.0));
    let (when, which) = starts(&out)[0];
    assert_eq!(which, 0);
    assert!(when >= 40);
    match out[when + 14].order {
        Order::Dash { dir, per_tick } => {
            assert!(dir[1] < -0.9, "falls on the target: {dir:?}");
            assert!(per_tick > 4.0);
        }
        o => panic!("{o:?}"),
    }
    let mut b = Brain::new(Style::Diver, 6, 0);
    b.force(0);
    let landed = run(&mut b, &m, 0.0, 0, 30, |n| {
        let mut s = senses(n, 1.0);
        s.pos = [0.0, if n >= 15 { 0.5 } else { 10.0 }, 0.0];
        s
    });
    let s = strikes(&landed);
    assert_eq!(s.len(), 1);
    assert!(s[0].1.hits_target);
    assert!(matches!(landed[20].order, Order::Steer { .. }), "climbs in its window");
}

#[test]
fn shrike_waits_for_a_target_under_a_roof() {
    let m = mob("shrike");
    let mut b = Brain::new(Style::Diver, 6, 0);
    let out = run(&mut b, &m, 0.0, 0, 120, |n| {
        let mut s = senses(n, 3.0);
        s.pos = [0.0, 10.0, 0.0];
        s.target.as_mut().unwrap().under_roof = true;
        s
    });
    assert!(starts(&out).is_empty());
}

#[test]
fn lurker_creeps_when_unseen_and_strikes_only_from_water() {
    let m = mob("lurker");
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    let dry = run(&mut b, &m, 0.0, 0, 30, |n| senses(n, 4.0));
    assert!(starts(&dry).is_empty());
    assert_eq!(dry[0].order, Order::Stay);
    let wet = |seen: bool| {
        move |n| {
            let mut s = senses(n, 10.0);
            s.in_water = true;
            s.target.as_mut().unwrap().sees_me = seen;
            s
        }
    };
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    let unseen = run(&mut b, &m, 0.0, 0, 2, wet(false));
    assert!(matches!(unseen[0].order, Order::Steer { .. }));
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    let seen = run(&mut b, &m, 0.0, 0, 2, wet(true));
    assert_eq!(seen[0].order, Order::Stay);
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    let close = run(&mut b, &m, 0.0, 0, 2, |n| {
        let mut s = senses(n, 5.0);
        s.in_water = true;
        s
    });
    assert_eq!(starts(&close)[0].1, m.attack_index("ambush").unwrap());
}

#[test]
fn slime_leaps_and_its_landing_hits_an_area() {
    let m = mob("bog_slime");
    let mut b = Brain::new(Style::Jumper, 1, 0);
    let out = run(&mut b, &m, 0.0, 0, 40, |n| {
        let mut s = senses(n, 1.5);
        s.on_ground = !(13..20).contains(&n);
        s
    });
    assert_eq!(starts(&out)[0].0, 0);
    assert!(matches!(out[12].order, Order::Leap { .. }));
    let s = strikes(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].0, 20);
    assert_eq!(s[0].1.area, Some(2.0));
    assert!(s[0].1.hits_target);
}

#[test]
fn slime_sticks_on_contact_at_most_once_a_second() {
    let mut b = Brain::new(Style::Jumper, 1, 0);
    assert!(b.contact(&senses(5, 1.0)));
    assert!(!b.contact(&senses(10, 1.0)));
    assert!(b.contact(&senses(25, 1.0)));
    assert!(!b.contact(&senses(50, 3.0)));
    let mut d = Brain::new(Style::Duelist, 1, 0);
    assert!(!d.contact(&senses(5, 1.0)));
}

#[test]
fn no_target_means_idle() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 1, 0);
    let out = run(&mut b, &m, 0.0, 0, 3, |n| Senses { target: None, ..senses(n, 1.0) });
    assert!(out.iter().all(|t| t.order == Order::Stay && t.label == "idle"));
}

#[test]
fn styles_by_id_and_helpers() {
    assert_eq!(Style::of("revenant"), Style::Duelist);
    assert_eq!(Style::of("bog_slime"), Style::Jumper);
    assert!(Style::of("shrike").steered() && Style::of("lurker").steered());
    assert!(!Style::of("brute").steered());
    let p = away([0.0; 3], [0.0, 0.0, 2.0], 3.0);
    assert!((p[2] + 3.0).abs() < 1e-9);
    assert_eq!(rage_mult(&mob("revenant")), 1.0);
    let b = mob("brute");
    assert_eq!(reach_of(&b, b.attack("wide_swing").unwrap(), Style::Heavy), HEAVY_REACH);
    assert_eq!(reach_of(&b, b.attack("charge").unwrap(), Style::Heavy), DASH_CONTACT);
}

#[test]
fn every_mob_runs_every_attack_to_its_end() {
    for m in MobsFile::embedded().mobs {
        let style = Style::of(&m.id);
        for (idx, a) in m.phased_attacks() {
            let mut b = Brain::new(style, 1, 0);
            b.force(idx);
            let out = run(&mut b, &m, 0.5, 0, u64::from(Timeline::of(a).end()) + 2, |n| {
                let mut s = senses(n, 1.5);
                s.in_water = true;
                s
            });
            let ended = out.iter().flat_map(|t| &t.events).any(|e| matches!(e, Event::AttackEnd { attack, .. } if *attack == idx));
            assert!(ended, "{} {} ends", m.id, a.id);
        }
    }
}

#[test]
fn brute_roars_once_when_rage_starts() {
    let m = mob("brute");
    let mut b = Brain::new(Style::Heavy, 9, 0);
    let sc = Scaled::at(&m.stats, 0.0);
    let low = |n: u64| {
        let mut s = senses(n, 3.0);
        s.health_frac = 0.2;
        s
    };
    let first = b.tick(&m, &sc, &low(0));
    assert!(first.events.contains(&Event::Mode("rage")));
    assert_eq!(first.order, Order::Stay);
    assert_eq!(b.clip().name, "rage");
    for n in 1..20 {
        let t = b.tick(&m, &sc, &low(n));
        assert_eq!(b.clip().name, "rage", "tick {n}");
        assert!(t.strikes.is_empty() && !t.events.iter().any(|e| matches!(e, Event::AttackStart { .. })));
    }
    let rest = run(&mut b, &m, 0.0, 20, 200, low);
    assert!(!rest.iter().flat_map(|t| &t.events).any(|e| *e == Event::Mode("rage")), "the roar is once");
}

/// A shrike 10 blocks over its target, in the air.
fn above(n: u64) -> Senses {
    let mut s = senses(n, 3.0);
    s.pos = [0.0, 10.0, 0.0];
    s.on_ground = false;
    s
}

#[test]
fn shrike_dive_locks_at_fold() {
    let m = mob("shrike");
    let dive = m.attack("dive").unwrap();
    let fold = u64::from(dive.windup) - 1;
    let hit = Timeline::of(dive).hit_start() as usize;
    let mut b = Brain::new(Style::Diver, 6, 0);
    b.force(0);
    let sc = Scaled::at(&m.stats, 0.0);
    // The target steps aside the tick after the wings fold.
    let moved = |n: u64| {
        let mut s = above(n);
        if n > fold {
            s.target.as_mut().unwrap().pos = [6.0, 0.0, 3.0];
        }
        s
    };
    let mut out = Vec::new();
    let mut clips = Vec::new();
    for n in 0..30 {
        out.push(b.tick(&m, &sc, &moved(n)));
        clips.push(b.clip().name.clone());
    }
    let dir = match out[hit].order {
        Order::Dash { dir, .. } => dir,
        o => panic!("{o:?}"),
    };
    // Aimed at where the target stood at the fold, (0, 0.5, 3), not at (6, 0.5, 3).
    let line = geom::sub([0.0, 0.5, 3.0], [0.0, 10.0, 0.0]);
    let want = geom::scale(line, 1.0 / geom::len(line));
    assert!(geom::dist(dir, want) < 1e-9, "{dir:?} not {want:?}");
    assert_eq!(out[hit + 1].order, out[hit].order, "the dive keeps its line");
    assert_eq!(clips[hit - 1], "attack_dive");
    let window_end = Timeline::of(dive).recover_start() as usize + 3;
    for (n, c) in clips.iter().enumerate().take(window_end).skip(hit) {
        assert_eq!(c, "dive", "no contact, no ground: still diving at {n}");
    }
    assert_eq!(clips[window_end], "climb");
}

#[test]
fn shrike_dive_clip_ends_on_contact() {
    let m = mob("shrike");
    let mut b = Brain::new(Style::Diver, 6, 0);
    b.force(0);
    let sc = Scaled::at(&m.stats, 0.0);
    let mut clips = Vec::new();
    for n in 0..20 {
        let mut s = above(n);
        if n >= 15 {
            s.pos = [0.0, 0.5, 2.5];
        }
        b.tick(&m, &sc, &s);
        clips.push(b.clip().name.clone());
    }
    assert_eq!(clips[14], "dive");
    assert_eq!(clips[15], "climb", "it touched the target");
}

#[test]
fn shrikes_take_turns() {
    let m = mob("shrike");
    let sc = Scaled::at(&m.stats, 0.0);
    let dive_len = u64::from(Timeline::of(m.attack("dive").unwrap()).end());
    const TARGET: u32 = 77;
    let mut brains = [Brain::new(Style::Diver, 6, 0), Brain::new(Style::Diver, 7, 0)];
    let mut turns = DiveTurns::default();
    // (mob, start, end) of every dive.
    let mut dives: Vec<(u32, u64, Option<u64>)> = Vec::new();
    for now in 0..400 {
        for (i, b) in brains.iter_mut().enumerate() {
            let id = i as u32 + 1;
            let mut s = above(now);
            s.wait_turn = turns.waits(TARGET, id, now);
            let t = b.tick(&m, &sc, &s);
            turns.note(TARGET, id, now, &t, dive_len);
            for e in &t.events {
                match e {
                    Event::AttackStart { .. } => dives.push((id, now, None)),
                    Event::AttackEnd { .. } => {
                        if let Some(d) = dives.iter_mut().rev().find(|d| d.0 == id && d.2.is_none()) {
                            d.2 = Some(now);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(dives.iter().any(|d| d.0 == 1) && dives.iter().any(|d| d.0 == 2), "both dive: {dives:?}");
    for a in &dives {
        for b in dives.iter().filter(|b| b.0 != a.0 && b.1 >= a.1) {
            let end = a.2.unwrap_or(u64::MAX);
            assert!(b.1 >= end, "{} started at {} while {} held the turn ({}..{end})", b.0, b.1, a.0, a.1);
        }
    }
}

#[test]
fn a_turn_held_too_long_lapses_and_a_dead_holder_lets_go() {
    let mut turns = DiveTurns::default();
    let t = Tick {
        order: Order::Stay,
        repath: false,
        look_at: None,
        events: vec![Event::AttackStart { attack: 0 }],
        strikes: Vec::new(),
        guarding: false,
        rage: false,
        speed_mult: 1.0,
        label: String::new(),
        hold: None,
        shadow: None,
    };
    turns.note(5, 1, 100, &t, 46);
    assert!(turns.waits(5, 2, 120));
    assert!(!turns.waits(5, 1, 120), "the holder itself may go on");
    assert!(!turns.waits(6, 2, 120), "another target is free");
    assert!(!turns.waits(5, 2, 146), "lapsed after the dive's length");
    turns.release(1);
    assert!(!turns.waits(5, 2, 120));
}

#[test]
fn slime_shows_jump_air_and_casts_a_shadow() {
    let m = mob("bog_slime");
    let mut b = Brain::new(Style::Jumper, 1, 0);
    b.force(0);
    let sc = Scaled::at(&m.stats, 0.0);
    let mut shadows = Vec::new();
    let mut clips = Vec::new();
    for n in 0..30 {
        let mut s = senses(n, 1.5);
        s.on_ground = !(13..20).contains(&n);
        let t = b.tick(&m, &sc, &s);
        if let Some(at) = t.shadow {
            shadows.push((n, at));
        }
        clips.push(b.clip().clone());
    }
    assert_eq!(shadows, vec![(12, [0.0, 0.0, 1.5]), (16, [0.0, 0.0, 1.5])], "every shadow_every_ticks at the landing");
    assert_eq!(clips[11].name, "attack_jump");
    for (n, c) in clips.iter().enumerate().take(20).skip(12) {
        assert_eq!(c.name, "jump_air", "in the air at {n}");
    }
    // Landed: the attack's clip again, at its recovery (tick 14 of it now).
    assert_eq!(clips[20], Clip { name: "attack_jump".into(), start: 6 });
}

#[test]
fn a_long_jump_recovers_after_it_lands() {
    // The big slime's jump (0.8 up) is in the air past the attack's 29 ticks: its window is the
    // 15 ticks after the landing (window_note), and its blow is the landing.
    let m = mob("bog_slime");
    let jump = m.attack("high_jump").unwrap();
    let mut b = Brain::new(Style::Jumper, 1, 0);
    b.force(0);
    // To the tick it ends at, not into the next jump.
    let out = run(&mut b, &m, 0.0, 0, 49, |n| {
        let mut s = senses(n, 1.5);
        s.on_ground = !(13..33).contains(&n);
        s
    });
    let phases: Vec<(usize, Phase)> = out
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            t.events.iter().filter_map(move |e| match e {
                Event::Phase { phase, .. } => Some((i, *phase)),
                _ => None,
            })
        })
        .collect();
    let land = 33;
    assert_eq!(
        phases,
        vec![(0, Phase::Windup), (12, Phase::Hit), (land, Phase::Recover), (land + jump.recover as usize, Phase::Done)]
    );
    assert_eq!(strikes(&out).iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![land]);
}

/// A lurker in water, its target 1.5 blocks south: the forced ambush lands on the hit phase's
/// first tick (10). `hurt` gives the ticks the lurker loses health on.
fn grab_run(hurt: &[u64], ticks: u64) -> Vec<Tick> {
    let m = mob("lurker");
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    b.force(m.attack_index("ambush").unwrap());
    let sc = Scaled::at(&m.stats, 0.0);
    let mut health = 1.0;
    (0..ticks)
        .map(|n| {
            if hurt.contains(&n) {
                health -= 0.05;
            }
            let mut s = senses(n, 1.5);
            s.in_water = true;
            s.health_frac = health;
            b.tick(&m, &sc, &s)
        })
        .collect()
}

fn mode_at(out: &[Tick], mode: &str) -> Option<usize> {
    out.iter().position(|t| t.events.iter().any(|e| matches!(e, Event::Mode(m) if *m == mode)))
}

fn bites(out: &[Tick]) -> Vec<usize> {
    let grab = mob("lurker").attack_index("grab").unwrap();
    strikes(out).iter().filter(|(_, s)| s.attack == grab).map(|(i, _)| *i).collect()
}

#[test]
fn grab_ends_after_hits() {
    let out = grab_run(&[15, 17, 19], 60);
    assert_eq!(mode_at(&out, "grab"), Some(10), "the ambush landed, the grab follows");
    assert!(out[10].events.iter().any(|e| matches!(e, Event::AttackEnd { .. })), "the ambush ends on its blow");
    for (n, t) in out.iter().enumerate().take(19).skip(10) {
        assert_eq!(t.hold, Some([0.0, 0.0, 0.8]), "held at the mouth on {n}");
        assert!(matches!(t.order, Order::Dash { dir, per_tick } if dir == [0.0, -1.0, 0.0] && (per_tick - 0.05).abs() < 1e-9));
    }
    assert_eq!(mode_at(&out, "backoff"), Some(19), "the third blow frees the target");
    assert!(out[19..].iter().all(|t| t.hold.is_none()));
    assert!(bites(&out).is_empty(), "freed before the first bite");
}

#[test]
fn grab_lasts_its_max_ticks_and_bites() {
    let out = grab_run(&[], 70);
    assert_eq!(mode_at(&out, "grab"), Some(10));
    assert_eq!(mode_at(&out, "backoff"), Some(50), "grab_max_ticks 40 after it began");
    assert_eq!(bites(&out), vec![20, 30, 40], "grab.damage every grab_hit_every_ticks");
    let grab = mob("lurker").attack_index("grab").unwrap();
    assert!(strikes(&out).iter().filter(|(_, s)| s.attack == grab).all(|(_, s)| s.hits_target && s.area.is_none()));
}

#[test]
fn an_ambush_that_misses_grabs_nothing() {
    let m = mob("lurker");
    let mut b = Brain::new(Style::Ambusher, 8, 0);
    b.force(m.attack_index("ambush").unwrap());
    let out = run(&mut b, &m, 0.0, 0, 40, |n| {
        let mut s = senses(n, 12.0);
        s.in_water = true;
        s
    });
    assert_eq!(mode_at(&out, "grab"), None);
    assert!(out.iter().all(|t| t.hold.is_none()));
}
