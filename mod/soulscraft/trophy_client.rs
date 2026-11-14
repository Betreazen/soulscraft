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
    true
}
