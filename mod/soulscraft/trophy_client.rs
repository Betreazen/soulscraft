// Trophies on the client (docs/design/09-trophies.md; soulscraft_rules::trophy): the block entities
// of the heads (`<mob>_head`, `<mob>_wall_head`) and of the dioramas (`<mob>_trophy`) draw the mob's
// head, or its figure in the looping clip `trophy`, with the mob's own rig and texture
// (soulscraft_client::draw_rig), from `assets/soulscraft/rigs/<mob>.trophy.json` in the content
// mod. The chain is the file's (assets/models/README.md, «Трофеи»): from the bottom centre, the
// block's turn (a floor head −22.5 × rotation, a wall head and a diorama −facing), the offset,
// the figure's yaw, its scale, then the model frame.
//
// Heads stand still: their frame is kept (`BlockEntityDraw::keep`, API 94,
// native_block_entity_draw.rs:200-212) until the block changes. A diorama's figure breathes from
// `game_time` and `partial` each frame (native_block_entity_draw.rs:44-56, 427-440), so it is
// never kept. Glowing cubes are drawn at full light (`draw.light = 0xF0F0`); the rest by the
// light of the block's cell, which a diorama's own `light` raises (its block lets light through).
use crate::game::Game;
use crate::game::native_block_entity_draw::BlockEntityDraw;
use crate::game::soulscraft_client as client;
use soulscraft_rules::trophy::{self, Kind, TrophyFile};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

static FILES: LazyLock<Mutex<HashMap<String, Option<Arc<TrophyFile>>>>> = LazyLock::new(Mutex::default);

fn file(game: &Game<'_>, mob: &str) -> Option<Arc<TrophyFile>> {
    let mut files = FILES.lock().unwrap_or_else(PoisonError::into_inner);
    files
        .entry(mob.to_owned())
        .or_insert_with(|| {
            let path = format!("assets/soulscraft/rigs/{mob}.trophy.json");
            let bytes = game.native_client_resources().read(&path)?;
            match TrophyFile::parse(&String::from_utf8_lossy(&bytes)) {
                Ok(f) => {
                    log::info!("soulscraft: trophy {mob}: {} particle rows, light {}", f.diorama.particles.len(), f.diorama.light);
                    Some(Arc::new(f))
                }
                Err(e) => {
                    log::warn!("soulscraft: {path}: {e}");
                    None
                }
            }
        })
        .clone()
}

/// `native_block_entity_submit` for a trophy block. False for any other block.
pub(super) fn draw_block_entity(game: &Game<'_>, pos: [i32; 3], block: &str, draw: &mut BlockEntityDraw) -> bool {
    let Some(kind) = Kind::of(block) else { return false };
    let mob = kind.mob();
    let Some(f) = file(game, mob) else { return true };
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let rotation = game.native_block_property(pos, "rotation").and_then(|r| r.parse().ok()).unwrap_or(0);
    let (place, figure) = match kind {
        Kind::Head(_) => (f.head.floor, false),
        Kind::WallHead(_) => (f.head.wall, false),
        Kind::Diorama(_) => (trophy::Place { scale: f.diorama.scale, offset: f.diorama.offset, yaw: f.diorama.yaw }, true),
    };
    let seed = (pos[0] ^ pos[2].rotate_left(11) ^ pos[1].rotate_left(22)) as u32;
    draw.push();
    draw.translate([0.5, 0.0, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], trophy::turn(kind, rotation, &facing), [0.0; 3]);
    draw.translate([place.offset[0] / 16.0, place.offset[1] / 16.0, place.offset[2] / 16.0]);
    draw.rotate_about([0.0, 1.0, 0.0], place.yaw, [0.0; 3]);
    draw.scale([place.scale; 3]);
    if figure {
        let t = trophy::clip_tick(pos, draw.game_time, draw.partial, f.diorama.ticks);
        let flags: Vec<&str> = f.diorama.flags.iter().map(String::as_str).collect();
        client::draw_rig(game, mob, || client::load_rig(game, mob), &f.diorama.clip, t, &flags, draw, seed);
    } else {
        let key = format!("{mob}#head");
        let drawn = client::draw_rig(game, &key, || f.head_rig().map_err(|e| log::warn!("soulscraft: head {mob}: {e}")).ok(), "", 0.0, &[], draw, seed);
        if drawn {
            draw.keep();
        }
    }
    draw.pop();
    if let (Kind::Diorama(_), Some(plaque)) = (kind, f.diorama.plaque) {
        draw_plaque(game, mob, trophy::turn(kind, rotation, &facing), plaque, draw);
    }
    true
}

/// Engraved brass: dark letters lit by the block's light.
const PLAQUE_INK: u32 = 0x3B_2A12;
/// The letters stand this far (model pixels) in front of the plaque's face.
const PLAQUE_LIFT: f32 = 0.02;

/// The plaque's text (09-trophies.md, «Подставка»): the mob's name and the viewer's kills from the
/// hunter's book, on the plaque's front face (north in the model, `plaque.from` z), as a sign's
/// lines are drawn (`native_draw_text`, API draw_text).
fn draw_plaque(game: &Game<'_>, mob: &str, turn: f32, plaque: trophy::PxBox, draw: &mut BlockEntityDraw) {
    let Some((name, kills)) = crate::game::soulscraft_book_client::plaque(mob) else { return };
    let lines = trophy::plaque_lines(&name, kills);
    let widths: Vec<f32> = lines.iter().map(|l| game.native_text_width(l)).collect();
    let size = [(plaque.to[0] - plaque.from[0]) as f32, (plaque.to[1] - plaque.from[1]) as f32];
    let k = widths.iter().map(|w| trophy::plaque_scale(*w, size, lines.len())).fold(f32::INFINITY, f32::min);
    if !k.is_finite() {
        return;
    }
    let centre = [
        (plaque.from[0] + plaque.to[0]) as f32 / 2.0,
        (plaque.from[1] + plaque.to[1]) as f32 / 2.0,
        plaque.from[2] as f32 - PLAQUE_LIFT,
    ];
    draw.push();
    draw.translate([0.5, 0.0, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], turn, [0.0; 3]);
    draw.translate([centre[0] / 16.0 - 0.5, centre[1] / 16.0, centre[2] / 16.0 - 0.5]);
    // The text faces +z; the plaque faces north (−z), and its reader's right is −x.
    draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
    draw.scale([k / 16.0, -k / 16.0, k / 16.0]);
    let top = -(lines.len() as f32) * 9.0 / 2.0;
    for (i, (line, w)) in lines.iter().zip(&widths).enumerate() {
        let text = horizon_client::text::Text::literal(line.clone());
        game.native_draw_text(draw, &text, [-w / 2.0, top + i as f32 * 9.0 + 0.5], PLAQUE_INK, false, 0);
    }
    draw.pop();
}
