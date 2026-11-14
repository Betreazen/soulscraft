// The vanilla changes in the world (07-vanilla-mobs.md; soulscraft_vanilla, soulscraft_rules::vanilla),
// on the altar site's platform (soulscraft_selftest_altar::clear_site), against a still invulnerable
// iron golem set as the mob's target (its vanilla target too):
//   vanilla_adopt_p          zombies at P 0 and 1: Г2 on the vanilla base (20 → 38 health, 3 → 4.8
//                            damage, armour 2 → 8), PCT's pct.xp100 and pct.xp145
//   vanilla_spawner_grows_only  a skeleton next to a spawner: tagged, grows, no mind (Г4)
//   vanilla_wave_left_alone  a zombie tagged soulscraft.wave is the event's: not adopted
//   vtiming_<group>_<attack> every move forced: cue on tick 0, phases and the blow on the data's
//                            ticks, vanilla's goals held (no walking in the windup); the drowned's
//                            trident, the witch's potion and the pillager's bolt are thrown; the
//                            phantom's swoop is held 10 ticks and released on the hit tick
//   vskeleton_backs_off      a skeleton 2 blocks from its target walks off to its distance
// Slow, only when the filter names them (`run vslow`):
//   vslow_skeleton_pause     three arrows, then the pause
//   vslow_zombie_flank       three zombies on one target: two come round the sides
//   vslow_witch_hides        a hurt witch walks behind a wall
use crate::game::Game;
use crate::game::soulscraft_selftest_altar::{at, centre, clear_site};
use crate::game::soulscraft_vanilla as vanilla;
use horizon_client::mobs::Who;
use soulscraft_rules::{attack::Timeline, data::MobsFile, geom, vanilla::Behavior};
use std::sync::{Mutex, PoisonError};

const DIM: &str = "minecraft:overworld";
/// Ticks after the summon before a check: the server adopts first.
const SETTLE: u64 = 4;

/// The scenarios and their timeouts.
pub(super) fn cases(data: &MobsFile) -> Vec<(String, u64)> {
    let mut out = vec![
        ("vanilla_adopt_p".to_owned(), 30),
        ("vanilla_spawner_grows_only".to_owned(), 30),
        ("vanilla_wave_left_alone".to_owned(), 30),
    ];
    for g in &data.vanilla.groups {
        for a in &g.attacks {
            out.push((format!("vtiming_{}_{}", g.id, a.def.id), u64::from(Timeline::of(&a.def).end()) + 60));
        }
    }
    out.extend([
        ("vskeleton_backs_off".to_owned(), 80),
        ("vslow_skeleton_pause".to_owned(), 600),
        ("vslow_zombie_flank".to_owned(), 200),
        ("vslow_witch_hides".to_owned(), 200),
    ]);
    out
}

#[derive(Default)]
struct Case {
    mark: u64,
    mobs: Vec<u32>,
    golem: Option<u32>,
    started: Option<u64>,
    forced: bool,
    from: [f64; 3],
    /// The phantom's swoop flag on each tick after the start: (tick, flag).
    flags: Vec<(u64, bool)>,
    moved: f64,
}

static CASE: Mutex<Option<Case>> = Mutex::new(None);

fn summon(game: &mut Game<'_>, kind: &str, c: [i32; 3], extra: &str) -> Option<u32> {
    let snbt = format!("{{Rotation:[0f,0f],PersistenceRequired:1b{extra}}}");
    let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&snbt) else { return None };
    game.native_summon(DIM, kind, centre(at(c)), nbt)
}

fn golem(game: &mut Game<'_>, c: [i32; 3]) -> Option<u32> {
    crate::game::soulscraft_selftest_altar::golem(game, c)
}

fn tags(game: &Game<'_>, id: u32) -> Vec<String> {
    game.native_entity(id).and_then(|e| game.native_entity_tags(e)).unwrap_or_default()
}

fn pos(game: &Game<'_>, id: u32) -> Option<[f64; 3]> {
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z])
}

/// Vanilla's own target and the mind's: the golem.
fn aim(game: &mut Game<'_>, mob: u32, golem: u32) {
    vanilla::set_target(mob, golem);
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob) {
        m.ai.target = Some(Who::Mob(golem));
    }
}

pub(super) fn cleanup(game: &mut Game<'_>) {
    if let Some(c) = CASE.lock().unwrap_or_else(PoisonError::into_inner).take() {
        for id in c.mobs.iter().chain(c.golem.iter()) {
            game.native_remove_mob(DIM, *id);
            vanilla::state().targets.remove(id);
        }
    }
    clear_site(game);
}

/// Where a mob of this move stands and where its golem stands (blocks south).
fn layout(behavior: Behavior, attack: &soulscraft_rules::data::AttackDef) -> f64 {
    match behavior {
        _ if attack.projectile.is_some() => 8.0,
        Behavior::Cube => 4.0,
        Behavior::Phantom => 6.0,
        _ if attack.dash_blocks.is_some() => attack.dash_blocks.unwrap_or(3.0).min(4.0) + 0.5,
        _ => 1.5,
    }
}

pub(super) fn step(game: &mut Game<'_>, now: u64, name: &str, first: bool) -> Option<Result<(), String>> {
    let mut guard = CASE.lock().unwrap_or_else(PoisonError::into_inner);
    if first {
        *guard = Some(Case { mark: now, ..Case::default() });
        drop(guard);
        clear_site(game);
        return None;
    }
    let c = guard.as_mut()?;
    let t = now - c.mark;
    if t == 1 {
        let ok = arrange(game, name, c);
        return (!ok).then(|| Err("summon refused".into()));
    }
    if t < SETTLE {
        return None;
    }
    if let Some(rest) = name.strip_prefix("vtiming_") {
        return timing(game, now, rest, c);
    }
    match name {
        "vanilla_adopt_p" => adopt_p(game, c),
        "vanilla_spawner_grows_only" => {
            let id = *c.mobs.first()?;
            let s = vanilla::state();
            let v = s.mobs.get(&id)?;
            let tagged = tags(game, id).iter().any(|t| t == "soulscraft.spawner");
            let xp = tags(game, id).iter().any(|t| t.starts_with("pct.xp"));
            Some(if v.brain.is_none() && tagged && xp { Ok(()) } else { Err(format!("mind {}, tagged {tagged}, xp {xp}", v.brain.is_some())) })
        }
        "vanilla_wave_left_alone" => {
            let id = *c.mobs.first()?;
            let s = vanilla::state();
            Some(if !s.mobs.contains_key(&id) && !tags(game, id).iter().any(|t| t.starts_with("pct.xp")) { Ok(()) } else { Err("adopted".into()) })
        }
        "vskeleton_backs_off" | "vslow_skeleton_pause" => {
            let (id, golem) = (*c.mobs.first()?, c.golem?);
            if !c.forced {
                vanilla::state().mobs.get(&id)?;
                aim(game, id, golem);
                c.forced = true;
                c.from = pos(game, id)?;
                return None;
            }
            aim(game, id, golem);
            let want = if name == "vskeleton_backs_off" { "retreat" } else { "pause" };
            let s = vanilla::state();
            let mode = s.modes.iter().any(|(m, mode, _)| *m == id && mode == want);
            drop(s);
            let gp = pos(game, golem)?;
            let d = geom::flat_dist(pos(game, id)?, gp);
            if mode && (name != "vskeleton_backs_off" || d >= 4.0) {
                return Some(Ok(()));
            }
            None
        }
        "vslow_zombie_flank" => {
            let golem = c.golem?;
            if !c.forced {
                if c.mobs.iter().any(|id| !vanilla::state().mobs.contains_key(id)) {
                    return None;
                }
                for id in c.mobs.clone() {
                    aim(game, id, golem);
                }
                c.forced = true;
                return None;
            }
            for id in c.mobs.clone() {
                aim(game, id, golem);
            }
            let s = vanilla::state();
            let flankers: std::collections::BTreeSet<u32> = s.flanks.iter().filter(|f| c.mobs.contains(&f.0)).map(|f| f.0).collect();
            (flankers.len() >= 2).then_some(Ok(()))
        }
        "vslow_witch_hides" => {
            let (id, golem) = (*c.mobs.first()?, c.golem?);
            if !c.forced {
                vanilla::state().mobs.get(&id)?;
                if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                    m.health = m.attrs.max_health * 0.3;
                }
                c.forced = true;
            }
            aim(game, id, golem);
            let s = vanilla::state();
            s.modes.iter().any(|(m, mode, _)| *m == id && mode == "hide").then_some(Ok(()))
        }
        other => Some(Err(format!("no scenario {other}"))),
    }
}

fn arrange(game: &mut Game<'_>, name: &str, c: &mut Case) -> bool {
    let fresh = |p: &str| format!(",Tags:[\"soulscraft.p={p}\",\"soulscraft.fresh\"]");
    match name {
        "vanilla_adopt_p" => {
            let a = summon(game, "minecraft:zombie", [-3, 0, 0], &fresh("0.00"));
            let b = summon(game, "minecraft:zombie", [3, 0, 0], &fresh("1.00"));
            c.mobs = [a, b].into_iter().flatten().collect();
            c.mobs.len() == 2
        }
        "vanilla_spawner_grows_only" => {
            let p = at([2, 0, 0]);
            game.native_run_command(&format!("setblock {} {} {} minecraft:spawner", p[0], p[1], p[2]));
            c.mobs = summon(game, "minecraft:skeleton", [0, 0, 0], ",NoAI:1b").into_iter().collect();
            !c.mobs.is_empty()
        }
        "vanilla_wave_left_alone" => {
            c.mobs = summon(game, "minecraft:zombie", [0, 0, 0], ",NoAI:1b,Tags:[\"soulscraft.wave\"]").into_iter().collect();
            !c.mobs.is_empty()
        }
        "vskeleton_backs_off" => {
            c.golem = golem(game, [0, 0, 2]);
            c.mobs = summon(game, "minecraft:skeleton", [0, 0, 0], &fresh("0.00")).into_iter().collect();
            c.golem.is_some() && !c.mobs.is_empty()
        }
        "vslow_skeleton_pause" => {
            c.golem = golem(game, [0, 0, 10]);
            c.mobs = summon(game, "minecraft:skeleton", [0, 0, 0], &fresh("0.00")).into_iter().collect();
            c.golem.is_some() && !c.mobs.is_empty()
        }
        "vslow_zombie_flank" => {
            c.golem = golem(game, [0, 0, 9]);
            c.mobs = (-1..=1).filter_map(|x| summon(game, "minecraft:zombie", [x * 2, 0, 0], &fresh("0.00"))).collect();
            c.golem.is_some() && c.mobs.len() == 3
        }
        "vslow_witch_hides" => {
            c.golem = golem(game, [0, 0, 8]);
            // A wall 3 high, 5 wide, 3 blocks east: its far side is out of the golem's sight.
            let (a, b) = (at([3, 0, -2]), at([3, 2, 2]));
            game.native_run_command(&format!("fill {} {} {} {} {} {} minecraft:stone", a[0], a[1], a[2], b[0], b[1], b[2]));
            c.mobs = summon(game, "minecraft:witch", [0, 0, 0], &fresh("0.00")).into_iter().collect();
            c.golem.is_some() && !c.mobs.is_empty()
        }
        _ => arrange_timing(game, name, c),
    }
}

/// A move's mob (the first type that has it; the drowned with a trident, a slime of size 2) and
/// its golem the move's distance south.
fn arrange_timing(game: &mut Game<'_>, name: &str, c: &mut Case) -> bool {
    let data = crate::game::soulscraft_server::state().data.clone();
    let Some(rest) = name.strip_prefix("vtiming_") else { return false };
    let Some((g, a)) = data.vanilla.groups.iter().find_map(|g| {
        let a = rest.strip_prefix(&format!("{}_", g.id))?;
        g.attacks.iter().find(|x| x.def.id == a).map(|x| (g, x))
    }) else {
        return false;
    };
    let kind = a.only.first().cloned().unwrap_or_else(|| g.types[0].clone());
    let mut extra = ",Tags:[\"soulscraft.p=0.00\",\"soulscraft.fresh\"]".to_owned();
    match kind.as_str() {
        "minecraft:drowned" => extra.push_str(",equipment:{mainhand:{id:\"minecraft:trident\",count:1}}"),
        "minecraft:slime" => extra.push_str(",Size:1"),
        _ => {}
    }
    let d = layout(g.behavior(), &a.def);
    let up = if g.behavior() == Behavior::Phantom { 4 } else { 0 };
    c.golem = golem(game, [0, 0, d.ceil() as i32]);
    c.mobs = summon(game, &kind, [0, up, 0], &extra).into_iter().collect();
    c.golem.is_some() && !c.mobs.is_empty()
}

fn adopt_p(game: &mut Game<'_>, c: &mut Case) -> Option<Result<(), String>> {
    let s = vanilla::state();
    if c.mobs.iter().any(|id| !s.mobs.contains_key(id)) {
        return None;
    }
    drop(s);
    let mut why = Vec::new();
    use horizon_client::mobs::finalize::{Attr, base_value};
    for (id, p, xp_want) in c.mobs.iter().zip([(0.0, "pct.xp100"), (1.0, "pct.xp145")]).map(|(id, (p, x))| (id, p, x)) {
        let m = game.mobs.iter().find(|m| m.id == *id)?;
        let (max, dmg, hp) = (f64::from(m.attrs.max_health), f64::from(m.attrs.attack_damage), f64::from(m.health));
        let (base_h, base_d) = (base_value(m, Attr::MaxHealth), base_value(m, Attr::AttackDamage));
        // Armour without what it wears (a zombie may spawn in a helmet): its type's 2 plus 6·P.
        let armor = f64::from(horizon_client::mobs::armor::values(m).0) - f64::from(m.worn_armor().0).floor();
        let xp: Vec<String> = tags(game, *id).into_iter().filter(|t| t.starts_with("pct.xp")).collect();
        let want = (base_h * (1.0 + 0.9 * p), base_d * (1.0 + 0.6 * p), 2.0 + 6.0 * p);
        let ok = (base_h - 20.0).abs() < 0.01
            && (max - want.0).abs() < 0.01
            && (dmg - want.1).abs() < 0.01
            && (hp - max).abs() < 0.01
            && (armor - want.2).abs() < 0.01
            && xp == [xp_want];
        if !ok {
            why.push(format!("P {p}: health {hp}/{max} (base {base_h}) damage {dmg} (base {base_d}) armour {armor} xp {xp:?}, wanted {want:?} {xp_want}"));
        }
    }
    Some(if why.is_empty() { Ok(()) } else { Err(why.join("; ")) })
}

fn timing(game: &mut Game<'_>, now: u64, rest: &str, c: &mut Case) -> Option<Result<(), String>> {
    let data = crate::game::soulscraft_server::state().data.clone();
    let (g, a) = data.vanilla.groups.iter().find_map(|g| {
        let id = rest.strip_prefix(&format!("{}_", g.id))?;
        g.attacks.iter().find(|x| x.def.id == id).map(|x| (g.clone(), x.def.clone()))
    })?;
    let (id, golem) = (*c.mobs.first()?, c.golem?);
    vanilla::state().mobs.get(&id)?;
    aim(game, id, golem);
    if !c.forced {
        if !vanilla::force(id, &a.id) {
            return Some(Err("not forced".into()));
        }
        c.forced = true;
        return None;
    }
    let line = Timeline::of(&a);
    let s = vanilla::state();
    let mine = |r: &&vanilla::VRecord| r.mob == id && r.attack == a.id;
    let Some(start) = s.phases.iter().filter(mine).find(|r| r.t == 0).map(|r| r.tick) else { return None };
    let behavior = g.behavior();
    if c.started.is_none() {
        c.started = Some(start);
        c.from = pos(game, id)?;
    }
    let ended = s.phases.iter().filter(mine).any(|r| r.phase.is_some_and(|p| p.label() == "end"));
    let cue = s.cues.iter().any(|(m, x, tick)| *m == id && *x == a.id && *tick == start);
    let strikes: Vec<(u64, bool)> = s.strikes.iter().filter(mine).map(|r| (r.tick - start, r.hit)).collect();
    let hit_at = s.phases.iter().filter(mine).find(|r| r.phase.is_some_and(|p| p.label() == "hit")).map(|r| r.tick - start);
    drop(s);
    // Vanilla's own goals rest in the windup: the mob stands (a cube's hop is up, not along).
    if now - start < u64::from(a.windup) && behavior != Behavior::Phantom {
        let p = pos(game, id)?;
        c.moved = c.moved.max(geom::flat_dist(p, c.from));
    }
    if behavior == Behavior::Phantom {
        let flag = game.mobs.iter().find(|m| m.id == id).is_some_and(|m| m.extra.flag);
        c.flags.push((now - start, flag));
    }
    if !ended && now - start <= u64::from(line.end()) + 4 {
        return None;
    }
    let hit = u64::from(line.hit_start());
    let mut why = Vec::new();
    if !cue {
        why.push("no cue on tick 0".to_owned());
    }
    if hit_at != Some(hit) {
        why.push(format!("hit phase at {hit_at:?}, data {hit}"));
    }
    if c.moved > 0.4 {
        why.push(format!("walked {:.2} in the windup: the gate did not hold", c.moved));
    }
    match behavior {
        Behavior::Phantom => {
            let held = c.flags.iter().filter(|(t, _)| *t < hit).all(|(_, f)| !f);
            let released = c.flags.iter().any(|(t, f)| *t == hit && *f);
            if !(held && released) {
                why.push(format!("swoop flag {:?}", c.flags));
            }
        }
        Behavior::Cube => {
            if !strikes.iter().any(|(t, _)| *t >= hit && *t <= hit + 40) {
                why.push(format!("no landing: {strikes:?}"));
            }
        }
        _ if a.dash_blocks.is_some() || behavior == Behavior::Pouncer => {
            let end = u64::from(line.recover_start()) + 4;
            if !strikes.iter().any(|(t, h)| *h && *t >= hit && *t <= end) {
                why.push(format!("no contact in {hit}..{end}: {strikes:?}"));
            }
        }
        _ => {
            let want: Vec<u64> = match &a.strikes_at {
                Some(list) => list.iter().map(|x| hit + u64::from(*x)).collect(),
                None => vec![hit],
            };
            let got: Vec<u64> = strikes.iter().map(|x| x.0).collect();
            if got != want || !strikes.iter().all(|x| x.1) {
                why.push(format!("blows {strikes:?}, data {want:?}"));
            }
        }
    }
    log::info!("soulscraft: selftest vtiming {rest} start={start} strikes={strikes:?} moved={:.2}", c.moved);
    Some(if why.is_empty() { Ok(()) } else { Err(why.join("; ")) })
}
