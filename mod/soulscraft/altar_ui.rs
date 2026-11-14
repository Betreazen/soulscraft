// What players see of an altar (soulscraft_altar): the menu's lines, the views sent to the
// players near it (the ghost of its next ring, its event), and the operator's `/sc altar` and
// `/sc event`.
use crate::game::Game;
use crate::game::soulscraft_altar::{Altar, altars_in, evaluate, file, run, setblock};
use crate::game::soulscraft_event as event;
use soulscraft_rules::{
    altar::{self, AltarFile},
    view,
};

/// Views go to players this close to a core.
const VIEW_RANGE: f64 = 40.0;

/// The menu's lines for one altar (soulscraft_rules::view::Menu).
pub(super) fn menu_of(a: &Altar, now: u64) -> view::Menu {
    let file = file();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for n in &a.report.missing {
        if n.part.is_some_and(|p| p != "bottom") {
            continue;
        }
        let name = block_name(&file, n.block);
        match counts.iter_mut().find(|c| c.0 == name) {
            Some(c) => c.1 += 1,
            None => counts.push((name, 1)),
        }
    }
    let missing = counts.into_iter().map(|(n, k)| format!("{n} ×{k}")).collect();
    let banners: Vec<String> =
        a.report.banners.iter().filter_map(|(_, id)| file.modifiers.iter().find(|m| &m.id == id)).map(|m| m.name.clone()).collect();
    let effects = altar::effects(&file, &a.banner_ids()).lines();
    let mut unlocks = Vec::new();
    if a.level == 0 {
        unlocks.push(if a.sky { "Ивенты недоступны: достройте первое кольцо".into() } else { "Ивенты недоступны: над ядром крыша".into() });
    } else {
        unlocks.push(file.levels[a.level as usize - 1].unlocks.clone());
        for e in &crate::game::soulscraft_server::state().data.events {
            if e.tier <= a.level {
                unlocks.push(format!("Ивент «{}»: положите {} на подставку", e.id, item_name(&e.item)));
            }
        }
    }
    let effects_sum = altar::effects(&file, &a.banner_ids());
    unlocks.push(soulscraft_rules::boost::menu_line(
        &soulscraft_rules::boost::BoostDef::embedded(),
        a.core,
        a.level,
        a.boost,
        crate::game::soulscraft_altar::age(a, now),
        &effects_sum,
    ));
    view::Menu {
        core: a.core,
        level: a.level,
        sky: a.sky,
        missing,
        banners,
        effects,
        boost: a.boost,
        unlocks,
        event: event::menu_lines(&a.dim, a.core, now),
    }
}

pub(super) fn item_name(item: &str) -> &str {
    match item {
        "soulscraft:rotten_horn" => "Гнилой горн",
        other => other,
    }
}

fn block_name(file: &AltarFile, block: &str) -> String {
    let name = match block {
        altar::CORE => "ядро",
        altar::PEDESTAL => "подставка",
        altar::SLAB => "плита",
        altar::SOCKET => "гнездо",
        altar::PILLAR => "колонна",
        altar::BRAZIER => "жаровня",
        altar::OBELISK => "обелиск",
        other => return file.modifier_of(other).map_or(other.to_owned(), |m| m.name.clone()),
    };
    name.to_owned()
}

/// Every altar's view to the players near it: the ghost of its next ring and its event.
pub(super) fn send_views(game: &mut Game<'_>, dim: &str, now: u64) {
    let players: Vec<(u128, [f64; 3])> =
        game.native_server_players().into_iter().filter(|p| p.dimension == dim).map(|p| (p.uuid, p.position)).collect();
    if players.is_empty() {
        return;
    }
    for a in altars_in(dim) {
        let c = [f64::from(a.core[0]) + 0.5, f64::from(a.core[1]), f64::from(a.core[2]) + 0.5];
        let near: Vec<u128> = players
            .iter()
            .filter(|(_, p)| soulscraft_rules::geom::dist(*p, c) <= VIEW_RANGE)
            .map(|(u, _)| *u)
            .collect();
        if near.is_empty() {
            continue;
        }
        let at = |c: [i32; 3]| [a.core[0] + c[0], a.core[1] + c[1], a.core[2] + c[2]];
        let v = view::View {
            core: a.core,
            level: a.level,
            ghost: a
                .report
                .missing
                .iter()
                .map(|n| view::GhostCell { pos: at(n.cell), block: n.block.to_owned(), part: n.part.map(str::to_owned) })
                .collect(),
            wrong: a.report.wrong.iter().map(|c| at(*c)).collect(),
            event: event::view(dim, a.core, now),
        };
        let bytes = view::encode(&v);
        for u in near {
            let _ = game.native_net_send_to_player(u, view::ALTAR, &bytes);
        }
    }
}

/// `/sc altar <level> [banners]`: an altar of that level six blocks ahead of the player on a
/// stone floor, its pieces' bottoms set (the tick puts up the rest), the sample banners on its
/// sockets when asked.
pub(super) fn build_here(game: &mut Game<'_>, dim: &str, at: [f64; 3], yaw: f64, level: u32, banners: bool, now: u64) -> String {
    let dir = soulscraft_rules::geom::yaw_dir(yaw);
    let (dx, dz) = if dir[0].abs() > dir[2].abs() { (dir[0].signum() as i32, 0) } else { (0, dir[2].signum() as i32) };
    let core = [at[0].floor() as i32 + 6 * dx, at[1].floor() as i32, at[2].floor() as i32 + 6 * dz];
    let r = altar::MAX_RING as i32 + 1;
    run(game, dim, &format!("fill {} {} {} {} {} {} stone", core[0] - r, core[1] - 1, core[2] - r, core[0] + r, core[1] - 1, core[2] + r));
    run(game, dim, &format!("fill {} {} {} {} {} {} air", core[0] - r, core[1], core[2] - r, core[0] + r, core[1] + 4, core[2] + r));
    let file = file();
    setblock(game, dim, core, altar::CORE, "replace");
    setblock(game, dim, [core[0], core[1] + 1, core[2]], altar::PEDESTAL, "replace");
    for ring in 1..=level {
        for n in altar::ring_needs(&file, ring) {
            if n.part.is_some_and(|p| p != "bottom") || n.block == altar::CORE || n.block == altar::PEDESTAL {
                continue;
            }
            setblock(game, dim, [core[0] + n.cell[0], core[1] + n.cell[1], core[2] + n.cell[2]], n.block, "replace");
        }
    }
    let mut placed = 0;
    if banners {
        let sockets: Vec<[i32; 2]> = file.levels.iter().take(level as usize).flat_map(|l| l.sockets.clone()).collect();
        for (s, id) in sockets.iter().zip(file.sample_banners.iter()) {
            if let Some(m) = file.modifiers.iter().find(|m| &m.id == id) {
                setblock(game, dim, [core[0] + s[0], core[1] + 1, core[2] + s[1]], &format!("soulscraft:{}", m.banner), "replace");
                placed += 1;
            }
        }
    }
    let _ = now;
    format!("altar of level {level} at {} {} {} ({placed} banners)", core[0], core[1], core[2])
}

/// `/sc event start|stop`: the test event on the nearest altar within 32 blocks, with the players
/// in its zone.
pub(super) fn event_here(game: &mut Game<'_>, dim: &str, at: [f64; 3], start: bool, now: u64) -> String {
    let near = altars_in(dim)
        .into_iter()
        .map(|a| (soulscraft_rules::geom::dist(at, [f64::from(a.core[0]), f64::from(a.core[1]), f64::from(a.core[2])]), a))
        .filter(|(d, _)| *d <= 32.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, a)| a);
    let Some(a) = near else { return "no altar within 32 blocks".into() };
    let Some(a) = evaluate(game, dim, a.core, now) else { return "the altar is gone".into() };
    if !start {
        return if event::stop(game, dim, a.core, now) { "event stopped".into() } else { "no event runs there".into() };
    }
    if a.level == 0 {
        return "the altar has no level (a ring missing, or a roof over the core)".into();
    }
    match event::start(game, dim, &a, 0, now, None) {
        Ok(()) => format!("event started on the altar of level {}", a.level),
        Err(e) => format!("event not started: {e}"),
    }
}
