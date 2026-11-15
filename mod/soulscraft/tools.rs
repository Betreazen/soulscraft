// The operator's fast-test kit, `/sc …` (soulscraft_rules::command), and the bench's console
// command `soulscraft_selftest run [filter]`. A player's command runs with their state swapped in
// (game/native_server.rs:1480-1500), where `native_run_command` is refused (native_server.rs:
// 726-736): whatever changes the world is queued here and done in the level's next tick
// (`run_due`), with the player's place and facing taken at the command.
use crate::game::Game;
use crate::game::soulscraft_server::{self as server, TAG_FRESH};
use crate::mod_api::{NativeCommand, NativeServerPlayer};
use horizon_client::text::Text;
use soulscraft_rules::{
    command::{self, Cmd, WaveAction},
    geom::{self, V3},
};
use std::sync::{Mutex, PoisonError};

/// SoulsCraft's corner of the joint bench's world (horizon-joint-dev/README.md) is x −1000..0,
/// z 0..1000; the arena stands in its middle.
pub(super) const ARENA: [i32; 3] = [-500, 100, 500];
const ARENA_HALF: i32 = 15;
/// Spawns stand this far in front of the player.
const SPAWN_AHEAD: f64 = 5.0;

/// Something a command asked for, done in the level's next tick.
#[derive(Clone, Debug)]
enum Task {
    Spawn { mob: String, p: Option<f64>, count: u32, at: V3, yaw: f64 },
    Arena { who: Option<String> },
    Wave(WaveAction, V3),
    Force { attack: String, at: V3 },
    Clear { at: V3 },
    Bench { count: u32, mob: Option<String>, at: V3 },
    Power,
    Pct { player: String, levels: Option<[u32; 3]> },
    Run(String),
    Altar { level: u32, banners: bool, at: V3, yaw: f64 },
    Event { start: bool, at: V3 },
}

struct Waves {
    centre: V3,
    wave: usize,
    ids: Vec<u32>,
    rest_until: Option<u64>,
}

struct Tools {
    tasks: Vec<(String, Task)>,
    waves: Option<Waves>,
    cost_since: Option<std::time::Instant>,
}

static TOOLS: Mutex<Tools> = Mutex::new(Tools { tasks: Vec::new(), waves: None, cost_since: None });

fn tools() -> std::sync::MutexGuard<'static, Tools> {
    TOOLS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    for usage in command::USAGES {
        commands.push(NativeCommand::op(usage, 2));
    }
    commands.push(NativeCommand::op(&format!("{} run [filter:word]", command::SELFTEST), 4));
}

fn say(game: &mut Game<'_>, line: impl Into<String>) {
    let line = line.into();
    log::info!("soulscraft: {line}");
    game.native_feedback(Text::literal(format!("[SoulsCraft] {line}")));
}

fn queue(dim: &str, task: Task) {
    tools().tasks.push((dim.to_owned(), task));
}

/// `/sc …` from a player, or a console line (`player` None).
pub(super) fn on_command(game: &mut Game<'_>, player: Option<&NativeServerPlayer>, line: &str, ran: &mut bool) {
    let Some(parsed) = command::parse(line) else { return };
    *ran = true;
    let cmd = match parsed {
        Ok(c) => c,
        Err(e) => return say(game, e),
    };
    let dim = player.map_or("minecraft:overworld".to_owned(), |p| p.dimension.clone());
    let at = player.map_or([ARENA[0] as f64 + 0.5, ARENA[1] as f64 + 1.0, ARENA[2] as f64 + 0.5], |p| p.position);
    let yaw = player.map_or(0.0, |p| f64::from(p.yaw));
    match cmd {
        Cmd::Spawn { mob, p, count } => {
            if server::state().data.mob(&mob).is_none() {
                let ids: Vec<String> = server::state().data.mobs.iter().map(|m| m.id.clone()).collect();
                return say(game, format!("no mob {mob}: {}", ids.join(", ")));
            }
            queue(&dim, Task::Spawn { mob, p, count, at, yaw });
        }
        Cmd::SetP(p) => {
            server::state().p_override = p.map(soulscraft_rules::scale::clamp_p);
            say(game, format!("P for new spawns: {}", p.map_or("players' own".into(), |v| format!("{v:.2}"))));
        }
        Cmd::Reload => match server::reload() {
            Ok(m) => say(game, m),
            Err(e) => say(game, format!("reload refused: {e}")),
        },
        Cmd::Debug(on) => {
            server::state().debug = on;
            say(game, format!("debug {}", if on { "on: labels over the mobs, creative players hunted" } else { "off" }));
        }
        Cmd::Log(on) => {
            server::set_chatty(on);
            say(game, format!("log {}", if on { "on: per-mob lines at info" } else { "off: per-mob lines at debug" }));
        }
        Cmd::Slowmo(rate) => queue(&dim, Task::Run(format!("tick rate {}", rate.unwrap_or(20.0)))),
        Cmd::Freeze(on) => queue(&dim, Task::Run(if on { "tick freeze".into() } else { "tick unfreeze".into() })),
        Cmd::Arena => queue(&dim, Task::Arena { who: player.map(|p| p.name.clone()) }),
        Cmd::Wave(a) => queue(&dim, Task::Wave(a, at)),
        Cmd::Force(attack) => queue(&dim, Task::Force { attack, at }),
        Cmd::Clear => queue(&dim, Task::Clear { at }),
        Cmd::Info => info(game),
        Cmd::Bench { count, mob } => {
            if let Some(m) = mob.as_deref().filter(|m| *m != "mixed")
                && server::state().data.mob(m).is_none()
            {
                return say(game, format!("no mob {m}: a mob id or mixed"));
            }
            queue(&dim, Task::Bench { count, mob, at });
        }
        // Both read other players' state, which a player's own command cannot (native_place):
        // done in the level's tick.
        Cmd::Power => queue(&dim, Task::Power),
        Cmd::Pct { player, levels } => queue(&dim, Task::Pct { player, levels }),
        Cmd::Cost(on) => cost(game, on),
        Cmd::Altar { level, banners } => queue(&dim, Task::Altar { level, banners, at, yaw }),
        Cmd::Horn => match player {
            Some(p) => queue(&dim, Task::Run(format!("give {} soulscraft:rotten_horn 1", p.name))),
            None => say(game, "sc horn is a player's command"),
        },
        Cmd::Event(start) => queue(&dim, Task::Event { start, at }),
        Cmd::SelfTest(filter) => {
            crate::game::soulscraft_selftest::start(filter.clone());
            say(game, format!("self-test started{}", filter.map_or(String::new(), |f| format!(" ({f})"))));
        }
    }
}

fn info(game: &mut Game<'_>) {
    let line = {
        let s = server::state();
        let alive = s.mobs.values().filter(|t| t.alive).count();
        format!(
            "{alive} mobs driven; numbers from the {} mobs.json; P {}; debug {}",
            s.source,
            s.p_override.map_or("players' own".into(), |p| format!("{p:.2}")),
            if s.debug { "on" } else { "off" }
        )
    };
    say(game, line);
    say(game, crate::game::soulscraft_vanilla::summary());
}

/// П5: mob cost sampling (`native_mob_cost_sampling`, game/native_diagnostics.rs:144-160) and
/// SoulsCraft's own time in its level tick.
fn cost(game: &mut Game<'_>, on: bool) {
    if on {
        game.native_mob_cost_sampling(true);
        server::reset_own_time();
        tools().cost_since = Some(std::time::Instant::now());
        return say(game, "cost sampling on");
    }
    game.native_mob_cost_sampling(false);
    let costs = game.native_take_mob_costs();
    let since = tools().cost_since.take().map_or(0.0, |t| t.elapsed().as_secs_f64());
    let (own_ns, own_ticks) = server::own_time();
    let (rate, mspt) = game.native_tick_stats();
    say(game, format!("cost over {since:.1} s: tick rate {rate}, server tick average {mspt:.2} ms"));
    // The samples come per mob type entry, and each SoulsCraft mob has its own (its type is
    // copied in register_goals to stop it burning): summed here by type name.
    let mut by_kind: Vec<(String, u64, u64)> = Vec::new();
    for c in &costs {
        match by_kind.iter_mut().find(|(k, _, _)| *k == c.kind) {
            Some(row) => {
                row.1 += c.mob_ticks;
                row.2 += c.nanos;
            }
            None => by_kind.push((c.kind.clone(), c.mob_ticks, c.nanos)),
        }
    }
    by_kind.sort_by(|a, b| b.2.cmp(&a.2));
    for (kind, ticks, nanos) in by_kind.iter().take(8) {
        let per = *nanos as f64 / (*ticks).max(1) as f64 / 1000.0;
        let ms_per_tick = *nanos as f64 / 1e6 / (since * 20.0).max(1.0);
        say(
            game,
            format!("cost kind={kind} mob_ticks={ticks} total_ms={:.2} us_per_mob_tick={per:.2} ms_per_server_tick={ms_per_tick:.3}", *nanos as f64 / 1e6),
        );
    }
    let per_tick = own_ns as f64 / own_ticks.max(1) as f64 / 1e6;
    say(game, format!("cost soulscraft_level_tick ticks={own_ticks} ms_per_tick={per_tick:.3}"));
}

/// The level's tick: the queued tasks of this level, and the waves.
pub(super) fn run_due(game: &mut Game<'_>, dim: &str, now: u64) {
    let due: Vec<Task> = {
        let mut t = tools();
        let (mine, rest): (Vec<_>, Vec<_>) = t.tasks.drain(..).partition(|(d, _)| d == dim);
        t.tasks = rest;
        mine.into_iter().map(|(_, task)| task).collect()
    };
    for task in due {
        run(game, dim, now, task);
    }
    if dim == "minecraft:overworld" {
        tick_waves(game, dim, now);
    }
}

fn run(game: &mut Game<'_>, dim: &str, now: u64, task: Task) {
    match task {
        Task::Spawn { mob, p, count, at, yaw } => {
            let ahead = geom::yaw_dir(yaw);
            let side = [ahead[2], 0.0, -ahead[0]];
            let (mut made, mut moved, mut skipped) = (0, 0, 0);
            for i in 0..count {
                let k = f64::from(i) - f64::from(count - 1) / 2.0;
                let pos = geom::add(geom::add(at, geom::scale(ahead, SPAWN_AHEAD)), geom::scale(side, 1.5 * k));
                let p = p.unwrap_or_else(|| server::state().p_override.unwrap_or(0.0));
                // Round 9: the box fits where it appears, or the spot moves, or there is no spawn.
                match fit_spot(game, &mob, 0, pos) {
                    None => skipped += 1,
                    Some((spot, m)) => {
                        moved += usize::from(m);
                        if summon_facing(game, dim, &mob, spot, p, yaw + 180.0).is_some() {
                            made += 1;
                        }
                    }
                }
            }
            let mut line = format!("spawned {made} {mob}");
            if moved > 0 {
                line += &format!(", {moved} moved to where they fit");
            }
            if skipped > 0 {
                line += &format!(", {skipped} skipped: no room within {} blocks", server::state().data.spawn_fit.search_blocks);
            }
            say(game, line);
        }
        Task::Arena { who } => arena(game, who.as_deref()),
        Task::Wave(action, at) => wave(game, dim, now, action, at),
        Task::Force { attack, at } => force(game, &attack, at, now),
        Task::Clear { at } => {
            let ids: Vec<u32> = server::state()
                .mobs
                .iter()
                .filter(|(_, t)| t.dim == dim && geom::dist(t.pos, at) < 64.0)
                .map(|(id, _)| *id)
                .collect();
            for id in &ids {
                game.native_remove_mob(dim, *id);
            }
            // Forgotten at once: a removed mob would stay tracked until FORGET_AFTER and be the
            // nearest for `/sc force` in the meantime.
            server::state().mobs.retain(|id, _| !ids.contains(id));
            say(game, format!("removed {}", ids.len()));
        }
        Task::Bench { count, mob, at } => {
            let (p, ids) = {
                let s = server::state();
                (s.p_override.unwrap_or(0.5), s.data.mobs.iter().map(|m| m.id.clone()).collect::<Vec<_>>())
            };
            let mixed = mob.as_deref() == Some("mixed");
            let mut made = 0;
            for i in 0..count {
                let a = f64::from(i) * std::f64::consts::TAU / f64::from(count);
                let r = 8.0 + f64::from(i % 3) * 3.0;
                // Mixed: every kind in turn, so 49 make 7 of each; the slime at its full size.
                let kind = if mixed { ids[i as usize % ids.len()].as_str() } else { mob.as_deref().unwrap_or("revenant") };
                let size = if kind == "bog_slime" { 4 } else { 0 };
                if summon(game, dim, kind, geom::ring_point(at, r, a), p, size).is_some() {
                    made += 1;
                }
            }
            let what = mob.unwrap_or_else(|| "revenant".into());
            say(game, format!("bench: {made} {what} at P {p:.2} round {:.0} {:.0} {:.0}", at[0], at[1], at[2]));
        }
        Task::Power => {
            for line in crate::game::soulscraft_power::report(game) {
                say(game, line);
            }
        }
        Task::Pct { player, levels } => match crate::game::soulscraft_power::set_fixture(game, &player, levels) {
            Ok(line) => say(game, line),
            Err(e) => say(game, format!("sc pct refused: {e}")),
        },
        Task::Run(line) => {
            let ok = game.native_run_command(&line);
            say(game, format!("{line}: {}", if ok { "done" } else { "refused" }));
        }
        Task::Altar { level, banners, at, yaw } => {
            let line = crate::game::soulscraft_altar::build_here(game, dim, at, yaw, level, banners, now);
            say(game, line);
        }
        Task::Event { start, at } => {
            let line = crate::game::soulscraft_altar::event_here(game, dim, at, start, now);
            say(game, line);
        }
    }
}

/// Where a mob of `mob` (a SoulsCraft id or key, else a vanilla zombie's box) fits near `at`
/// (owner's decision, round 9; soulscraft_rules::fit): the spot and whether it moved, or None
/// when no spot within `spawn_fit.search_blocks` fits.
pub(super) fn fit_spot(game: &Game<'_>, mob: &str, size: u8, at: V3) -> Option<(V3, bool)> {
    let (width, height, grounded, radius) = {
        let s = server::state();
        let r = s.data.spawn_fit.search_blocks;
        match s.data.mob(mob) {
            Some(m) => {
                let hb = m.hitbox_at(if size == 0 { m.full_size() } else { size });
                (hb.width, hb.height, m.grounded(), r)
            }
            None => (0.6, 1.95, true, r),
        }
    };
    let boxes = |c: [i32; 3]| game.native_block_collision(c);
    let spot = soulscraft_rules::fit::nearest(at, width, height, radius, grounded, &boxes)?;
    Some((spot, geom::dist(spot, at) > 1e-6))
}

/// Spawns a SoulsCraft mob with its P (and size) as tags, fresh (health full at its scaled max).
pub(super) fn summon(game: &mut Game<'_>, dim: &str, mob: &str, at: V3, p: f64, size: u8) -> Option<u32> {
    let key = server::state().data.mob(mob)?.key.clone();
    let Some((at, moved)) = fit_spot(game, mob, size, at) else {
        log::info!("soulscraft: spawn skipped mob={mob} at={:.1},{:.1},{:.1}: no room for its box", at[0], at[1], at[2]);
        return None;
    };
    if moved {
        log::info!("soulscraft: spawn moved mob={mob} to={:.1},{:.1},{:.1}", at[0], at[1], at[2]);
    }
    let mut tags = vec![format!("\"soulscraft.p={p:.2}\""), format!("\"{TAG_FRESH}\"")];
    if size > 0 {
        tags.push(format!("\"soulscraft.size={size}\""));
    }
    let snbt = format!("{{Tags:[{}],PersistenceRequired:1b}}", tags.join(","));
    summon_nbt(game, dim, &key, at, &snbt)
}

/// `summon` facing `yaw` (degrees, 0 south): a spawn in front of a player turns to the player.
pub(super) fn summon_facing(game: &mut Game<'_>, dim: &str, mob: &str, at: V3, p: f64, yaw: f64) -> Option<u32> {
    let key = server::state().data.mob(mob)?.key.clone();
    let snbt = format!(
        "{{Rotation:[{yaw:.1}f,0f],Tags:[\"soulscraft.p={p:.2}\",\"{TAG_FRESH}\"],PersistenceRequired:1b}}"
    );
    summon_nbt(game, dim, &key, at, &snbt)
}

fn summon_nbt(game: &mut Game<'_>, dim: &str, key: &str, at: V3, snbt: &str) -> Option<u32> {
    let nbt = match crate::mod_api::parse_snbt(&snbt) {
        Ok((crate::mod_api::Tag::Compound(c), _)) => c,
        _ => return None,
    };
    let id = game.native_summon(dim, key, at, nbt);
    if id.is_none() {
        log::warn!("soulscraft: {key} not spawned at {at:?}");
    }
    id
}

/// `/sc force` with no SoulsCraft mob near: the nearest adopted vanilla mob within 16 blocks that
/// has the move (07-vanilla-mobs.md), for the bots' telegraph shots.
fn force_vanilla(game: &mut Game<'_>, attack: &str, at: V3) {
    let mut near: Vec<(f64, u32, String)> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive())
        .map(|m| (geom::dist([m.position.x, m.position.y, m.position.z], at), m.id, m.kind.id.to_string()))
        .filter(|x| x.0 <= 16.0)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, id, kind) in near {
        if crate::game::soulscraft_vanilla::force(id, attack) {
            return say(game, format!("{kind} {id} starts {attack}"));
        }
    }
    say(game, format!("no SoulsCraft or changed vanilla mob within 16 blocks has {attack}"));
}

/// The nearest SoulsCraft mob within 16 blocks starts `attack` now (else a vanilla one).
fn force(game: &mut Game<'_>, attack: &str, at: V3, _now: u64) {
    let living: Vec<u32> = game.mobs.iter().filter(|m| !m.removed && m.alive()).map(|m| m.id).collect();
    let mut s = server::state();
    let server::State { mobs, data, .. } = &mut *s;
    let Some((id, t)) = mobs
        .iter_mut()
        .filter(|(id, t)| t.alive && living.contains(id) && geom::dist(t.pos, at) <= 16.0)
        .min_by(|a, b| geom::dist(a.1.pos, at).total_cmp(&geom::dist(b.1.pos, at)))
    else {
        drop(s);
        return force_vanilla(game, attack, at);
    };
    let def = &data.mobs[t.kind];
    let Some(idx) = def.attack_index(attack) else {
        let list: Vec<&str> = def.attacks.iter().map(|a| a.id.as_str()).collect();
        let line = format!("{} has no {attack}: {}", def.id, list.join(", "));
        drop(s);
        return say(game, line);
    };
    t.brain.force(idx);
    let line = format!("{} {id} starts {attack}", def.id);
    drop(s);
    say(game, line);
}

/// The test arena: a 31×31 smooth stone floor at y 100 lit by sea lanterns, cleared 8 blocks up,
/// a 5×5 pool two deep (the lurker) and a wall 9 wide (the brute's charge), in SoulsCraft's area. The time of day is left alone:
/// the joint server is shared.
fn arena(game: &mut Game<'_>, who: Option<&str>) {
    let [x, y, z] = ARENA;
    let h = ARENA_HALF;
    let lines = vec![
        format!("forceload add {} {} {} {}", x - h, z - h, x + h, z + h),
        format!("fill {} {} {} {} {} {} air", x - h, y + 1, z - h, x + h, y + 8, z + h),
        format!("fill {} {} {} {} {} {} smooth_stone", x - h, y, z - h, x + h, y, z + h),
        format!("fill {} {} {} {} {} {} sea_lantern", x - h, y, z - h, x + h, y, z + h),
        format!("fill {} {} {} {} {} {} smooth_stone replace sea_lantern", x - h + 1, y, z - h + 1, x + h - 1, y, z + h - 1),
        format!("fill {} {} {} {} {} {} stone", x + 6, y - 2, z + 6, x + 12, y - 1, z + 12),
        format!("fill {} {} {} {} {} {} water", x + 7, y - 1, z + 7, x + 11, y, z + 11),
        format!("fill {} {} {} {} {} {} stone_bricks", x - 4, y + 1, z - h + 2, x + 4, y + 3, z - h + 2),
    ];
    let mut lines = lines;
    // Sea lanterns every 6 blocks: light 15 reaches the whole floor.
    for gx in (-h + 3..h).step_by(6) {
        for gz in (-h + 3..h).step_by(6) {
            lines.push(format!("setblock {} {y} {} sea_lantern", x + gx, z + gz));
        }
    }
    let refused: Vec<String> = lines.iter().filter(|l| !game.native_run_command(l)).cloned().collect();
    if let Some(name) = who {
        game.native_run_command(&format!("tp {name} {} {} {}", x, y + 1, z));
    }
    let ok = refused.is_empty();
    say(game, format!("arena at {x} {y} {z}{}", if ok { String::new() } else { format!(", refused: {}", refused.join("; ")) }));
}

/// The test event's waves (data/mobs.json `events[0]`), a stub without the altar: each wave's
/// zombies round the place it was started, the next after all are dead and the rest.
pub(super) fn wave(game: &mut Game<'_>, dim: &str, now: u64, action: WaveAction, at: V3) {
    match action {
        WaveAction::Stop => {
            let ids = tools().waves.take().map(|w| w.ids).unwrap_or_default();
            for id in &ids {
                game.native_remove_mob(dim, *id);
            }
            say(game, format!("waves stopped, {} removed", ids.len()));
        }
        WaveAction::Start => {
            tools().waves = Some(Waves { centre: at, wave: 0, ids: Vec::new(), rest_until: None });
            spawn_wave(game, dim, now);
        }
        WaveAction::Next => {
            if tools().waves.is_none() {
                tools().waves = Some(Waves { centre: at, wave: 0, ids: Vec::new(), rest_until: None });
            }
            spawn_wave(game, dim, now);
        }
    }
}

fn spawn_wave(game: &mut Game<'_>, dim: &str, now: u64) {
    let event = server::state().data.events.first().cloned();
    let Some(event) = event else { return say(game, "mobs.json has no event") };
    let (n, centre) = {
        let mut t = tools();
        let Some(w) = t.waves.as_mut() else { return };
        w.wave += 1;
        w.rest_until = None;
        (w.wave, w.centre)
    };
    let Some(spec) = event.waves.iter().find(|w| w.wave as usize == n) else {
        tools().waves = None;
        return say(game, format!("event {} won: all {} waves cleared", event.id, n - 1));
    };
    let mut ids = Vec::new();
    let total: u32 = spec.mobs.iter().map(|m| m.count).sum();
    let mut k = 0;
    for group in &spec.mobs {
        for i in 0..group.count {
            let a = f64::from(k) * std::f64::consts::TAU / f64::from(total.max(1));
            k += 1;
            let pos = geom::ring_point(centre, 12.0, a);
            let armored = i < group.armored;
            let banner = group.kind == "banner_zombie";
            let mut gear = String::new();
            if armored {
                gear = "equipment:{head:{id:\"minecraft:iron_helmet\",count:1},chest:{id:\"minecraft:iron_chestplate\",count:1}},drop_chances:{head:0f,chest:0f},".into();
            }
            if banner {
                gear = "equipment:{head:{id:\"minecraft:white_banner\",count:1}},drop_chances:{head:0f},".into();
            }
            let snbt = format!("{{{gear}Tags:[\"soulscraft.wave\"],PersistenceRequired:1b}}");
            let Ok((crate::mod_api::Tag::Compound(nbt), _)) = crate::mod_api::parse_snbt(&snbt) else { continue };
            let Some(id) = game.native_summon(dim, "minecraft:zombie", pos, nbt) else { continue };
            if let Some(mult) = group.health_mult
                && let Some(m) = game.mobs.iter_mut().find(|m| m.id == id)
            {
                m.attrs.max_health *= mult as f32;
                m.health = m.attrs.max_health;
            }
            ids.push(id);
        }
    }
    log::info!("soulscraft: wave event={} wave={n} mobs={} tick={now}", event.id, ids.len());
    say(game, format!("wave {n}: {} zombies", ids.len()));
    if let Some(w) = tools().waves.as_mut() {
        w.ids = ids;
    }
}

fn tick_waves(game: &mut Game<'_>, dim: &str, now: u64) {
    let (ids, rest) = match tools().waves.as_ref() {
        Some(w) => (w.ids.clone(), w.rest_until),
        None => return,
    };
    let alive = ids
        .iter()
        .filter(|id| game.mobs.iter().any(|m| m.id == **id && !m.removed && m.alive()))
        .count();
    match rest {
        None if alive == 0 && !ids.is_empty() => {
            let secs = server::state().data.events.first().and_then(|e| e.rest_seconds.first().copied()).unwrap_or(30.0);
            if let Some(w) = tools().waves.as_mut() {
                w.rest_until = Some(now + (secs * 20.0) as u64);
            }
            say(game, format!("wave cleared; next in {secs:.0} s"));
        }
        Some(until) if now >= until => spawn_wave(game, dim, now),
        _ => {}
    }
}


/// The running test event: its wave, the wave's mobs, and whether it rests (for the self-test).
pub(super) fn wave_state() -> Option<(usize, Vec<u32>, bool)> {
    tools().waves.as_ref().map(|w| (w.wave, w.ids.clone(), w.rest_until.is_some()))
}
