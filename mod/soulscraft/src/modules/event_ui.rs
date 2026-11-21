// What players see of a run of the altar's event (soulscraft_event): the boss bar with its
// warning, titles to the zone, the view clients draw (spawn points, beams, the altar's health),
// the menu's event lines, and the self-test's snapshot.
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::soulscraft_altar as altar_srv;
use crate::game::soulscraft_event::{EventRun, core_centre, events};
use soulscraft_rules::{
    defense::Goal,
    event as rev,
    geom,
    view::{self, Beam, BeamKind},
};

pub(super) fn zone_selector(r: &EventRun) -> String {
    let (min, max) = r.flow.zone(r.core);
    format!(
        "@a[x={},y={},z={},dx={},dy=96,dz={}]",
        min[0],
        r.core[1] - 48,
        min[1],
        max[0] - min[0] + 1,
        max[1] - min[1] + 1
    )
}

pub(super) fn tell_zone(game: &mut Game<'_>, r: &EventRun, title: &str, subtitle: &str) {
    let sel = zone_selector(r);
    altar_srv::run(game, &r.dim, &format!("title {sel} subtitle \"{subtitle}\""));
    altar_srv::run(game, &r.dim, &format!("title {sel} title \"{title}\""));
}

/// The boss bar: the wave and the altar's health; while the altar loses health it flashes red
/// and white, a bell sounds and the action bar warns, once a second.
pub(super) fn show_bar(game: &mut Game<'_>, r: &mut EventRun, now: u64) {
    if now % 10 != 0 {
        return;
    }
    let dim = r.dim.clone();
    let bar = r.bar.clone();
    let sel = zone_selector(r);
    let phase = match r.run.phase {
        rev::Phase::Telegraph { .. } => "враги близко",
        rev::Phase::Fight { .. } => "бой",
        rev::Phase::Rest { .. } => "передышка",
        _ => "",
    };
    let name = format!(
        "Волна {} из {} · {} · {} · алтарь {:.0}/{:.0}",
        r.run.wave() + 1,
        r.plan.waves.len(),
        r.name,
        phase,
        r.hp.hp.ceil(),
        r.hp.max
    );
    let shown = (r.hp.hp.ceil() as i64, r.hp.max as i64, name.clone());
    if shown != r.bar_shown {
        altar_srv::run(game, &dim, &format!("bossbar set {bar} max {}", shown.1.max(1)));
        altar_srv::run(game, &dim, &format!("bossbar set {bar} value {}", shown.0.max(0)));
        altar_srv::run(game, &dim, &format!("bossbar set {bar} name \"{name}\""));
        r.bar_shown = shown;
    }
    if now % 20 == 0 {
        altar_srv::run(game, &dim, &format!("bossbar set {bar} players {sel}"));
    }
    let losing = r.hp.losing(now);
    if losing {
        r.flash = !r.flash;
        altar_srv::run(game, &dim, &format!("bossbar set {bar} color {}", if r.flash { "red" } else { "white" }));
        if now % 20 == 0 {
            let c = core_centre(r.core);
            altar_srv::run(game, &dim, &format!("playsound minecraft:block.bell.use hostile {sel} {:.1} {:.1} {:.1} 1 0.6", c[0], c[1], c[2]));
            altar_srv::run(game, &dim, &format!("title {sel} actionbar {{\"text\":\"Алтарь теряет силу!\",\"color\":\"red\"}}"));
        }
    } else if r.flash {
        r.flash = false;
        altar_srv::run(game, &dim, &format!("bossbar set {bar} color yellow"));
    }
}

/// What clients draw of a run: wave, health, the coming spawn points, the beams, and the zone's
/// border for its first `flow.outline_ticks`.
pub(super) fn view(dim: &str, core: [i32; 3], _now: u64) -> Option<view::EventView> {
    let ev = events();
    let r = ev.get(&(dim.to_owned(), core))?;
    let phase = match r.run.phase {
        rev::Phase::Telegraph { .. } => "telegraph",
        rev::Phase::Fight { .. } => "fight",
        rev::Phase::Rest { .. } => "rest",
        rev::Phase::Won => "won",
        rev::Phase::Lost(_) => "lost",
    };
    Some(view::EventView {
        wave: r.run.wave() as u32 + 1,
        waves: r.plan.waves.len() as u32,
        hp: r.hp.hp,
        max: r.hp.max,
        losing: r.hp.losing(_now),
        phase: phase.into(),
        spawns: r.spawns.clone(),
        beams: r
            .mobs
            .values()
            .filter_map(|w| w.beam.map(|k| Beam { from: geom::add(w.pos, [0.0, 1.4, 0.0]), kind: k }))
            .collect(),
        outline: (_now < r.started + r.flow.outline_ticks).then(|| r.flow.outline(r.core)),
    })
}

/// The menu's event lines.
pub(super) fn menu_lines(dim: &str, core: [i32; 3], _now: u64) -> Vec<String> {
    let ev = events();
    let Some(r) = ev.get(&(dim.to_owned(), core)) else { return Vec::new() };
    let phase = match r.run.phase {
        rev::Phase::Telegraph { .. } => "враги близко",
        rev::Phase::Fight { .. } => "бой",
        rev::Phase::Rest { .. } => "передышка",
        _ => "конец",
    };
    let names: Vec<String> = r.parts_now.iter().filter_map(|p| p.player).filter_map(|u| r.joined.get(&u).cloned()).collect();
    vec![
        format!("Ивент «{}»: волна {} из {} ({phase})", r.name, r.run.wave() + 1, r.plan.waves.len()),
        format!("Здоровье алтаря: {:.0} / {:.0}", r.hp.hp.ceil(), r.hp.max),
        format!("Участники: {}", if names.is_empty() { format!("{} в зоне", r.parts_now.len()) } else { names.join(", ") }),
    ]
}

/// For the self-test: a run's state.
#[derive(Clone, Debug, Default)]
pub(super) struct Snapshot {
    pub phase: String,
    pub wave: usize,
    pub hp: f64,
    pub max: f64,
    pub mobs: Vec<(u32, String, Goal, bool, Option<BeamKind>)>,
    pub spawns: Vec<[i32; 3]>,
    pub builds: u32,
    pub breaches: usize,
    pub open_from: Option<u32>,
}

pub(super) fn snapshot(dim: &str, core: [i32; 3], probe: Option<[i32; 3]>) -> Option<Snapshot> {
    let ev = events();
    let r = ev.get(&(dim.to_owned(), core))?;
    Some(Snapshot {
        phase: format!("{:?}", r.run.phase),
        wave: r.run.wave(),
        hp: r.hp.hp,
        max: r.hp.max,
        mobs: r.mobs.iter().map(|(id, w)| (*id, w.role.name().to_owned(), w.goal, w.mind.cursed, w.beam)).collect(),
        spawns: r.spawns.clone(),
        builds: r.builds,
        breaches: r.breaches.list.len(),
        open_from: probe.and_then(|p| r.field.as_ref().and_then(|f| f.locate(p).and_then(|c| f.open_at(c)))),
    })
}

/// Every run's mobs and their tags (the self-test checks the experience tag).
pub(super) fn wave_ids(dim: &str, core: [i32; 3]) -> Vec<u32> {
    events().get(&(dim.to_owned(), core)).map(|r| r.mobs.keys().copied().collect()).unwrap_or_default()
}
