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
    assert!(out[18].look_at.is_some(), "the blow faces the target");
    assert!(!out[5].guarding, "the shield is down while it attacks");
}

/// The revenant's held blow follows the target: it looks at it and steps after it at
/// `hold_step_speed`; the blow itself still lands only on a target in reach on the hit tick.
#[test]
fn hold_tracks_target() {
    let m = mob("revenant");
    let step = m.param("hold_step_speed").expect("hold_step_speed in the data");
    let mut b = Brain::new(Style::Duelist, 1, 0);
    b.force(m.attack_index("overhead").unwrap());
    // The target steps back to 4 blocks as the hold starts.
    let out = run(&mut b, &m, 0.0, 0, 20, |n| senses(n, if n < 10 { 2.0 } else { 4.0 }));
    for t in &out[10..18] {
        assert_eq!(t.look_at, Some([0.0, 1.5, 4.0]), "{}", t.label);
        match t.order {
            Order::Walk { to, speed } => {
                assert_eq!(to, [0.0, 0.0, 4.0]);
                assert!((speed - step).abs() < 1e-9);
            }
            o => panic!("{}: {o:?}", t.label),
        }
    }
    assert_eq!(out[9].order, Order::Stay, "the windup stands");
}

/// Stepping away during the hold is followed; stepping away on the blow's tick is a miss.
#[test]
fn dodge_during_hit_misses() {
    let m = mob("revenant");
    let mut b = Brain::new(Style::Duelist, 1, 0);
    b.force(m.attack_index("overhead").unwrap());
    // In reach through the hold (the mob kept up), out of reach from the hit tick (18).
    let out = run(&mut b, &m, 0.0, 0, 30, |n| senses(n, if n < 18 { 2.0 } else { 4.0 }));
    let s = strikes(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].0, 18);
    assert!(!s[0].1.hits_target);
    let mut b = Brain::new(Style::Duelist, 1, 0);
    b.force(m.attack_index("overhead").unwrap());
    let kept = run(&mut b, &m, 0.0, 0, 30, |n| senses(n, 2.0));
    assert!(strikes(&kept)[0].1.hits_target, "a target that stayed is hit");
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

/// A stalker 2 blocks from a target facing it (yaw 180: north, at the mob).
fn facing_stalker(n: u64, seen: bool) -> Senses {
    let mut s = senses(n, 2.0);
    let t = s.target.as_mut().unwrap();
    t.yaw = 180.0;
    t.looking_at_me = seen;
    s
}

#[test]
fn stalker_waits_while_seen() {
    let m = mob("stalker");
    let max = m.param("flank_max_ticks").expect("flank_max_ticks in the data") as usize;
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    let out = run(&mut b, &m, 0.0, 0, max as u64, |n| facing_stalker(n, true));
    assert!(starts(&out).is_empty(), "no attack while seen, before flank_max_ticks");
    assert_eq!(out[1].label, "circle");
    let (lo, hi) = m.param_range("circle_radius_blocks").unwrap();
    let tg = [0.0, 0.0, 2.0];
    match out[1].order {
        Order::Walk { to, .. } => {
            let off = geom::off_facing(tg, 180.0, to);
            assert!(off > 90.0, "the flank point is beside or behind the target: {off:.0}°");
            let r = geom::flat_dist(tg, to);
            assert!(r >= lo - 1e-9 && r <= hi + 1e-9, "on the ring: {r}");
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn stalker_strikes_when_unseen() {
    let m = mob("stalker");
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    let out = run(&mut b, &m, 0.0, 0, 3, |n| facing_stalker(n, false));
    assert_eq!(starts(&out), vec![(0, m.attack_index("double_slash").unwrap())]);
}

#[test]
fn stalker_strikes_after_timeout() {
    let m = mob("stalker");
    let max = m.param("flank_max_ticks").unwrap() as usize;
    let mut b = Brain::new(Style::Skirmisher, 2, 0);
    let out = run(&mut b, &m, 0.0, 0, max as u64 + 30, |n| facing_stalker(n, true));
    let st = starts(&out);
    assert!(!st.is_empty(), "attacks after flank_max_ticks even when seen");
    assert!(st[0].0 >= max && st[0].0 <= max + 20, "at {}", st[0].0);
    assert_eq!(st[0].1, m.attack_index("double_slash").unwrap());
}

/// A wall the marksman reloads behind: 3 wide (x 4..6) and 3 high (y 0..2) at z 3, the target
/// 11 blocks south of a marksman at the origin, the floor at y −1.
fn reload_wall(c: [i32; 3]) -> bool {
    c[1] == -1 || ((4..=6).contains(&c[0]) && c[2] == 3 && (0..=2).contains(&c[1]))
}

#[test]
fn marksman_reloads_behind_cover() {
    let m = mob("marksman");
    let shot = m.attack_index("aimed_shot").unwrap();
    let k = m.param("reload_cover_blocks").expect("reload_cover_blocks in the data") as i32;
    let top = m.param("cover_min_height_blocks").unwrap();
    let eye = m.hitbox_at(0).eye;
    let target_eye = [0.5, 2.3, 11.5];
    let sight = |a: V3, b: V3| crate::cover::clear_line(a, b, &reload_wall);
    let spots: Vec<V3> = (-k..=k)
        .flat_map(|x| (-k..=k).map(move |z| [f64::from(x) + 0.5, 0.0, f64::from(z) + 0.5]))
        .filter(|p| !reload_wall([p[0].floor() as i32, 0, p[2].floor() as i32]))
        .collect();
    let from = [0.5, 0.0, 0.5];
    let spot = crate::cover::reload_cover(from, &spots, &[eye, top], target_eye, &sight).expect("a spot behind the wall");
    let s = crate::cover::ARRIVAL_SLACK;
    for (dx, dz) in [(0.0, 0.0), (-s, -s), (s, -s), (-s, s), (s, s)] {
        for h in [eye, top] {
            assert!(!sight(target_eye, geom::add(spot, [dx, h, dz])), "hidden wherever its walk ends: {spot:?}");
        }
    }
    assert!(geom::flat_dist(from, spot) <= f64::from(k) * 1.5, "near: {spot:?}");
    // No wall: no cover, the old step back.
    let open = |a: V3, b: V3| crate::cover::clear_line(a, b, &|c: [i32; 3]| c[1] == -1);
    assert_eq!(crate::cover::reload_cover(from, &spots, &[eye, top], target_eye, &open), None);

    // The brain: the shot's reload asks for cover once, shows `reload` all through, walks there.
    let mut b = Brain::new(Style::Shooter, 4, 0);
    b.force(shot);
    let line = Timeline::of(&m.attacks[shot]);
    let out = run(&mut b, &m, 0.0, 0, u64::from(line.recover_start()) + 1, |n| senses(n, 11.0));
    let r = line.recover_start() as usize;
    assert!(out[r].wants_cover && out.iter().filter(|t| t.wants_cover).count() == 1);
    assert_eq!(b.clip().name, "reload");
    b.set_cover(Some(spot));
    let sc = Scaled::at(&m.stats, 0.0);
    let next = b.tick(&m, &sc, &senses(u64::from(line.recover_start()) + 1, 11.0));
    assert_eq!(next.order, Order::Walk { to: spot, speed: crate::cover::COVER_SPEED });
    assert_eq!(b.clip().name, "reload");
    let rest = run(&mut b, &m, 0.0, u64::from(line.recover_start()) + 2, u64::from(line.recover) + 2, |n| senses(n, 11.0));
    assert!(rest.iter().any(|t| t.events.iter().any(|e| matches!(e, Event::AttackEnd { .. }))));
    assert_ne!(b.clip().name, "reload", "the clip ends with the reload");
}

#[test]
fn marksman_prefers_high_ground() {
    let m = mob("marksman");
    assert!(m.param_bool("prefers_high_ground"));
    let (lo, hi) = m.param_range("keep_distance_blocks").unwrap();
    let target = [0.0, 0.0, 0.0];
    let from = [0.0, 0.0, -6.0];
    let cands = post_candidates(target, from, lo, hi);
    assert!(cands.len() >= 5, "several candidates");
    assert!(cands.iter().all(|c| (lo..=hi).contains(&geom::flat_dist(*c, target))));
    // Raise one far candidate on a hill: the high ground wins over the near ones.
    let mut ground: Vec<V3> = cands.clone();
    let far = ground.len() - 1;
    ground[far][1] = 4.0;
    assert_eq!(pick_post(&ground, from, true), Some(ground[far]));
    let near = pick_post(&ground, from, false).unwrap();
    assert!(near[1] == 0.0, "without the preference: the nearest");
    assert_eq!(pick_post(&[], from, true), None);
    // In its band already, on flat ground, it stays where it is.
    let at = [0.0, 0.0, -12.0];
    let flat = post_candidates(target, at, lo, hi);
    assert_eq!(pick_post(&flat, at, true), Some(at));

    // The brain asks for a post on its decision and walks to the one it is given.
    let mut b = Brain::new(Style::Shooter, 4, 0);
    let sc = Scaled::at(&m.stats, 0.0);
    let s0 = senses(0, 20.0);
    let t0 = b.tick(&m, &sc, &s0);
    assert!(t0.wants_post);
    let post = [3.0, 4.0, 8.0];
    b.set_post(Some(post));
    let t1 = b.tick(&m, &sc, &senses(1, 20.0));
    assert_eq!(t1.order, Order::Walk { to: post, speed: 1.0 });
    assert!(!t1.wants_post);
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
    let a = starts(&out)[0].1;
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
            s.target.as_mut().unwrap().looking_at_me = seen;
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
