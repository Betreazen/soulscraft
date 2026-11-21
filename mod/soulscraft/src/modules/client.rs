// SoulsCraft's client half: each SoulsCraft mob is drawn from its rig (assets/rigs/<mob>.rig.json,
// shipped in the content mod as assets/soulscraft/rigs/ and read with `native_client_resources`,
// game/native_block_sprites.rs:75, horizon-client-render/src/client_resources.rs:38) instead of
// the zombie model (`render_living_pre`, its `canceled` set: game/modding/native_mob_draw.rs:166-203).
//
// The rig becomes one `ModelPart` tree per mob type, built with vanilla's builders
// (mobs/model_layers.rs: MeshDefinition, CubeListBuilder, LayerDefinition::bake_root) and drawn by
// `native_draw_model_part` (game/native_block_entity_draw.rs:727) in the model's frame
// (`scale(1, -1, -1)`, `translate(0, -1.501, 0)`, native_block_entity_draw.rs:716-726). Each
// frame every part takes its pose from the clip the server named (soulscraft::rules::wire), at
// `game_time - start + partial` ticks, interpolated by soulscraft::rules::rig. Cubes marked `glow`
// are children of their part, drawn in a second pass at full light (`draw.light = 0xF0F0`,
// native_block_entity_draw.rs:51-56). The weapon is drawn at its socket: the parts' chain to the
// socket's parent, its offset and turns, then `native_draw_item` with the socket's display context
// (native_block_entity_draw.rs:776-797, which applies the item model's `display` transform).
#[allow(unused_imports)]
use super::soulscraft_compat::*;
use crate::game::Game;
use crate::game::native_block_entity_draw::BlockEntityDraw;
use crate::game::native_mob_draw::MobRender;
use horizon_assets::item_model::DisplayContext;
use horizon_client::items::ItemStack;
use horizon_client::world_geometry::WorldGeometry;
use mod_api::{CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, ModelPart, PartPose};
use horizon_core::geometry::Direction;
use soulscraft::rules::{
    rig::{Pose, Rig},
    wire,
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

const GLOW: &str = "~glow";
const FULL_LIGHT: u32 = 0xF0F0;
/// Tint while the mob's hurt flash runs (the engine draws none for a mod's model:
/// docs/research/horizon-mob-models.md, 1.4).
const HURT_TINT: u32 = 0xFF8080;
/// A clip that ended more than this long ago gives way to walking and idling.
const AFTER_CLIP: f32 = 2.0;
/// Clips that hold their last frame for as long as the server names them: the slime's `jump_air`
/// lasts until it lands, however long the jump.
const HELD: [&str; 1] = ["jump_air"];

struct Model {
    rig: Rig,
    tree: ModelPart,
    /// For each rig part, the names from the tree's root down to it.
    paths: Vec<Vec<String>>,
    /// Rig parts that have glowing cubes.
    glows: Vec<usize>,
    stacks: Vec<Option<(usize, ItemStack, DisplayContext)>>,
}

#[derive(Clone, Debug, Default)]
struct Shown {
    clip: String,
    /// The client's game tick the clip started at.
    start: i64,
    flags: u8,
    size: u8,
    label: String,
}

#[derive(Default)]
struct Client {
    models: HashMap<String, Option<Model>>,
    shown: HashMap<u32, Shown>,
    batches: u64,
}

static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(Mutex::default);
/// Mobs whose first clip from the server was drawn: logged once each.
static LOGGED: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Mutex::default);

fn c_logged(id: u32) -> bool {
    LOGGED.lock().unwrap_or_else(PoisonError::into_inner).insert(id)
}

fn client() -> MutexGuard<'static, Client> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The server's clip notes: their start ticks moved onto this client's clock.
pub(super) fn on_net(game: &Game<'_>, channel: &str, data: &[u8]) {
    if channel != wire::STATE {
        return;
    }
    let Some(batch) = wire::decode(data) else {
        log::warn!("soulscraft: a state note that does not read");
        return;
    };
    let offset = game.world.game_time - batch.now;
    let mut c = client();
    c.batches += 1;
    if c.batches == 1 || c.batches % 400 == 0 {
        log::info!(
            "soulscraft: state batch {} from the server: {} notes, server tick {}, offset {offset}",
            c.batches,
            batch.notes.len(),
            batch.now
        );
    }
    for n in batch.notes {
        c.shown.insert(
            n.id,
            Shown { clip: n.clip, start: n.start + offset, flags: n.flags, size: n.size, label: n.label },
        );
    }
    if c.shown.len() > 4096 {
        // Notes of mobs long gone: keep the newest half.
        let mut ids: Vec<(u32, i64)> = c.shown.iter().map(|(id, s)| (*id, s.start)).collect();
        ids.sort_by_key(|(_, s)| *s);
        for (id, _) in &ids[..ids.len() / 2] {
            c.shown.remove(id);
        }
    }
}

fn face(name: &str) -> Option<Direction> {
    // rig.py's faces in the model frame (assets/models/rig.py:186-193) as model_layers' directions
    // (model_layers.rs: Down is the corners at y0, the top of a part in a +y-down frame).
    Some(match name {
        "top" => Direction::Down,
        "bottom" => Direction::Up,
        "right" => Direction::West,
        "left" => Direction::East,
        "front" => Direction::North,
        "back" => Direction::South,
        _ => return None,
    })
}

fn build(rig: Rig) -> Model {
    let mut mesh = MeshDefinition::new();
    let mut ids = Vec::with_capacity(rig.parts.len());
    let mut glows = Vec::new();
    for (i, p) in rig.parts.iter().enumerate() {
        let parent = rig.parents[i].map_or(mesh.root(), |j| ids[j]);
        let cubes = |glow: bool| {
            let mut b = CubeListBuilder::create();
            for c in p.cubes.iter().filter(|c| c.glow == glow) {
                b = b.tex_offs(c.uv[0], c.uv[1]).set_mirror(c.mirror);
                b = match &c.faces {
                    Some(list) => {
                        let faces: Vec<Direction> = list.iter().filter_map(|f| face(f)).collect();
                        b.add_box_faces(c.origin, c.size, &faces)
                    }
                    None => {
                        let [x, y, z] = c.grow();
                        b.add_box_with(c.origin, c.size, CubeDeformation::xyz(x, y, z))
                    }
                };
            }
            b
        };
        let pose = PartPose::offset_and_rotation(
            p.pivot[0],
            p.pivot[1],
            p.pivot[2],
            p.rotation[0],
            p.rotation[1],
            p.rotation[2],
        );
        let id = mesh.add_or_replace_child(parent, &p.name, cubes(false), pose);
        if p.cubes.iter().any(|c| c.glow) {
            mesh.add_or_replace_child(id, &format!("{}{GLOW}", p.name), cubes(true), PartPose::ZERO);
            glows.push(i);
        }
        ids.push(id);
    }
    let tree = LayerDefinition::create(mesh, rig.texture.width, rig.texture.height).bake_root();
    let paths = (0..rig.parts.len())
        .map(|i| rig.chain(i).into_iter().map(|j| rig.parts[j].name.clone()).collect())
        .collect();
    let stacks = rig
        .sockets
        .iter()
        .map(|s| {
            let item = s.item.as_deref().filter(|i| !i.is_empty())?;
            let ctx = match s.display.as_deref() {
                Some("thirdperson_lefthand") => DisplayContext::ThirdPersonLeftHand,
                Some("thirdperson_righthand") => DisplayContext::ThirdPersonRightHand,
                Some("head") => DisplayContext::Head,
                _ => DisplayContext::Fixed,
            };
            Some((rig.part(&s.parent)?, ItemStack::new(item, 1), ctx))
        })
        .collect();
    Model { rig, tree, paths, glows, stacks }
}

fn part_mut<'a>(tree: &'a mut ModelPart, path: &[String]) -> Option<&'a mut ModelPart> {
    let mut at = tree;
    for name in path {
        at = at.get_child_mut(name)?;
    }
    Some(at)
}

/// The mob's rig from the content mod's resources.
pub(super) fn load_rig(game: &Game<'_>, id: &str) -> Option<Rig> {
    let path = format!("assets/soulscraft/rigs/{id}.rig.json");
    let Some(bytes) = game.sc_client_resources().read(&path) else {
        log::warn!("soulscraft: no {path} in the resources (is the content mod on?)");
        return None;
    };
    match Rig::parse(&String::from_utf8_lossy(&bytes)) {
        Ok(rig) => {
            log::info!("soulscraft: rig {id}: {} parts, {} clips", rig.parts.len(), rig.clips.len());
            Some(rig)
        }
        Err(e) => {
            log::warn!("soulscraft: {path}: {e}");
            None
        }
    }
}

fn load(game: &Game<'_>, id: &str) -> Option<Model> {
    load_rig(game, id).map(build)
}

/// Draws the rig cached as `key` (made by `make` the first time) posed by `clip` at `t` ticks
/// (rest pose for a clip it lacks), from the draw's frame at the feet: the model frame, the
/// glowing cubes at full light, the socket items. The trophies' figure and heads
/// (soulscraft_trophy_client). False when there is no such rig.
pub(super) fn draw_rig(
    game: &Game<'_>,
    key: &str,
    make: impl FnOnce() -> Option<Rig>,
    clip: &str,
    t: f32,
    flags: &[&str],
    draw: &mut BlockEntityDraw,
    seed: u32,
) -> bool {
    draw_rig_with(game, key, make, clip, t, flags, draw, seed, 0xFF_FFFF, true)
}

/// [`draw_rig`] tinted (`0` for the hunter's book's black silhouette) and with or without the
/// socket items.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_rig_with(
    game: &Game<'_>,
    key: &str,
    make: impl FnOnce() -> Option<Rig>,
    clip: &str,
    t: f32,
    flags: &[&str],
    draw: &mut BlockEntityDraw,
    seed: u32,
    tint: u32,
    items: bool,
) -> bool {
    let mut c = client();
    if !c.models.contains_key(key) {
        let m = make().map(build);
        c.models.insert(key.to_owned(), m);
    }
    let Some(model) = c.models.get_mut(key).and_then(Option::as_mut) else { return false };
    let poses = model.rig.sample(clip, t);
    game.native_draw_push(draw);
    game.native_draw_scale(draw, [1.0, -1.0, -1.0]);
    game.native_draw_translate(draw, [0.0, -1.501, 0.0]);
    pose_tree(model, &poses, flags);
    paint(game, model, &poses, flags, tint, draw, seed, items);
    game.native_draw_pop(draw);
    true
}

/// The clips of mob `id`'s rig (loaded the first time): name, ticks, looped.
pub(super) fn clips_of(game: &Game<'_>, id: &str) -> Vec<(String, f32, bool)> {
    let mut c = client();
    if !c.models.contains_key(id) {
        let m = load(game, id);
        c.models.insert(id.to_owned(), m);
    }
    c.models
        .get(id)
        .and_then(Option::as_ref)
        .map(|m| m.rig.clips.iter().map(|(n, c)| (n.clone(), c.ticks as f32, c.looped)).collect())
        .unwrap_or_default()
}

/// The posed model drawn: its cubes, then its glowing cubes at full light, then its items.
#[allow(clippy::too_many_arguments)]
fn paint(game: &Game<'_>, model: &mut Model, poses: &[Pose], flags: &[&str], tint: u32, draw: &mut BlockEntityDraw, seed: u32, items: bool) {
    let sprite = model.rig.texture.sprite.clone();
    show_glow(model, false);
    game.sc_draw_model_part(draw, &model.tree, &sprite, tint);
    if !model.glows.is_empty() {
        show_glow(model, true);
        let light = game.native_draw_light(draw);
        game.native_draw_set_light(draw, FULL_LIGHT);
        game.sc_draw_model_part(draw, &model.tree, &sprite, tint);
        game.native_draw_set_light(draw, light);
        show_glow(model, false);
    }
    if items {
        draw_items(game, model, poses, flags, draw, seed);
    }
}

/// The clip a mob shows when the server names none: its walk or idle, by how it moves.
fn locomotion(rig: &Rig, id: &str, mob: &MobRender<'_>) -> (String, f32) {
    let moving = mob.walk_speed > 0.05;
    let pick = |names: &[&str]| names.iter().find(|n| rig.has_clip(n)).map(|n| (*n).to_owned());
    let clip = match (id, moving) {
        ("shrike", _) => pick(&["fly", "glide", "idle"]),
        ("lurker", true) => pick(&["swim", "walk"]),
        ("lurker", false) => pick(&["drift", "idle"]),
        ("bog_slime", true) => pick(&["hop", "idle"]),
        (_, true) => pick(&["walk", "idle"]),
        (_, false) => pick(&["idle"]),
    }
    .unwrap_or_default();
    let t = if clip == "walk" { rig.walk_ticks("walk", mob.walk_position) } else { mob.age };
    (clip, t)
}

pub(super) fn draw_mob(game: &Game<'_>, mob: &MobRender<'_>, draw: &mut BlockEntityDraw, drawn: &mut bool) {
    if *drawn {
        return;
    }
    let Some(id) = mob.kind.strip_prefix("soulscraft:") else { return };
    let mut c = client();
    if !c.models.contains_key(id) {
        let m = load(game, id);
        c.models.insert(id.to_owned(), m);
    }
    let shown = c.shown.get(&mob.id).cloned().unwrap_or_default();
    let Some(model) = c.models.get_mut(id).and_then(Option::as_mut) else { return };
    let now = game.native_draw_game_time(draw) as f32 + mob.partial;
    let (clip, t) = if mob.death_time > 0 && model.rig.has_clip("death") {
        ("death".to_owned(), mob.death_time as f32 + mob.partial)
    } else {
        let t = now - shown.start as f32;
        let held = HELD.contains(&shown.clip.as_str());
        let fresh = model.rig.clips.get(&shown.clip).is_some_and(|c| c.looped || held || t <= c.ticks as f32 + AFTER_CLIP);
        if !shown.clip.is_empty() && fresh {
            if c_logged(mob.id) {
                log::info!("soulscraft: mob {} draws {} at t {t:.1}", mob.id, shown.clip);
            }
            (shown.clip.clone(), t)
        } else {
            locomotion(&model.rig, id, mob)
        }
    };
    let poses = model.rig.sample(&clip, t);
    let flags: Vec<&str> = if shown.flags & wire::RAGE != 0 { vec!["rage"] } else { vec![] };
    *drawn = true;
    game.native_draw_push(draw);
    if shown.size > 0 && shown.size < 4 {
        // The bog slime's smaller sizes: the size-4 model scaled at its feet.
        let k = f32::from(shown.size) / 4.0;
        game.native_draw_scale(draw, [k, k, k]);
    }
    game.native_draw_scale(draw, [1.0, -1.0, -1.0]);
    game.native_draw_translate(draw, [0.0, -1.501, 0.0]);
    pose_tree(model, &poses, &flags);
    let tint = if mob.hurt_time > 0 { HURT_TINT } else { 0xFF_FFFF };
    paint(game, model, &poses, &flags, tint, draw, mob.id, true);
    game.native_draw_pop(draw);
}

fn pose_tree(model: &mut Model, poses: &[Pose], flags: &[&str]) {
    model.tree.reset_all_poses();
    for (i, pose) in poses.iter().enumerate() {
        let visible = model.rig.visible(i, flags);
        let Some(part) = part_mut(&mut model.tree, &model.paths[i]) else { continue };
        part.set_pos(pose.pos[0], pose.pos[1], pose.pos[2]);
        part.set_rotation(pose.rot[0], pose.rot[1], pose.rot[2]);
        (part.x_scale, part.y_scale, part.z_scale) = (pose.scale[0], pose.scale[1], pose.scale[2]);
        part.visible = visible;
    }
}

/// Glow pass on: only the glowing cubes draw; off: only the others.
fn show_glow(model: &mut Model, on: bool) {
    for i in 0..model.rig.parts.len() {
        let Some(part) = part_mut(&mut model.tree, &model.paths[i]) else { continue };
        part.skip_draw = on;
    }
    for &i in &model.glows {
        let name = format!("{}{GLOW}", model.rig.parts[i].name);
        if let Some(g) = part_mut(&mut model.tree, &model.paths[i]).and_then(|p| p.get_child_mut(&name)) {
            g.visible = on;
        }
    }
}

/// `ModelPart.translateAndRotate` for one pose, on the draw's stack (native_block_entity_draw.rs:
/// 842-856: to the place in blocks, then Z, Y, X, then the scale).
fn part_transform(game: &Game<'_>, draw: &mut BlockEntityDraw, p: &Pose) {
    game.native_draw_translate(draw, [p.pos[0] / 16.0, p.pos[1] / 16.0, p.pos[2] / 16.0]);
    game.native_draw_rotate_about(draw, [0.0, 0.0, 1.0], p.rot[2].to_degrees(), [0.0; 3]);
    game.native_draw_rotate_about(draw, [0.0, 1.0, 0.0], p.rot[1].to_degrees(), [0.0; 3]);
    game.native_draw_rotate_about(draw, [1.0, 0.0, 0.0], p.rot[0].to_degrees(), [0.0; 3]);
    if p.scale != [1.0; 3] {
        game.native_draw_scale(draw, p.scale);
    }
}

fn draw_items(game: &Game<'_>, model: &Model, poses: &[Pose], flags: &[&str], draw: &mut BlockEntityDraw, mob: u32) {
    for (socket, stack) in model.rig.sockets.iter().zip(&model.stacks) {
        let Some((parent, stack, ctx)) = stack else { continue };
        if !model.rig.visible(*parent, flags) {
            continue;
        }
        game.native_draw_push(draw);
        for j in model.rig.chain(*parent) {
            part_transform(game, draw, &poses[j]);
        }
        game.native_draw_translate(draw, [socket.offset[0] / 16.0, socket.offset[1] / 16.0, socket.offset[2] / 16.0]);
        for (axis, degrees) in &socket.rotate {
            let a = match axis.as_str() {
                "x" => [1.0, 0.0, 0.0],
                "y" => [0.0, 1.0, 0.0],
                _ => [0.0, 0.0, 1.0],
            };
            game.native_draw_rotate_about(draw, a, *degrees, [0.0; 3]);
        }
        // The seed vanilla's hand layer gives: id + 2 for the right hand, + 1 for the left
        // (game/mob_blocks.rs:478-497).
        let seed = mob as i32 + if *ctx == DisplayContext::ThirdPersonLeftHand { 1 } else { 2 };
        game.sc_draw_item(draw, stack, *ctx, None, seed);
        game.native_draw_pop(draw);
    }
}

/// `/sc debug on`: the server sends each mob's phase as a label; over the mob, coloured by phase
/// (windup yellow, hold orange, hit red, recover green), seen through walls.
///
/// 8f83e5c's world geometry (`submit_custom_geometry`: lines and quads, world_geometry.rs:234-282)
/// draws no text, and the old `WorldPrimitives` text is no longer fed by mods. So each labelled
/// mob gets a post over its head in its phase's colour, through walls (`/sc log on` prints the
/// phases' text on the server).
// TODO(port): the label's text over the mob; a name-tag style text needs a game text path in the
// world (`name_tag_quads` is the game's own) or one drawn in render_living_pre's draw.
pub(super) fn debug_labels(game: &Game<'_>, _view: &mod_api::WorldRenderView, draw: &mut WorldGeometry) {
    let c = client();
    if c.shown.values().all(|s| s.flags & wire::DEBUG == 0) {
        return;
    }
    let mut posts = Vec::new();
    // SoulsCraft's mobs and the vanilla mobs the server labels (07-vanilla-mobs.md).
    for m in game.mobs.iter().filter(|m| !m.removed && c.shown.contains_key(&m.id)) {
        let Some(s) = c.shown.get(&m.id) else { continue };
        if s.flags & wire::DEBUG == 0 || s.label.is_empty() {
            continue;
        }
        let color = match s.label.split(':').nth(1).and_then(|r| r.split(' ').next()) {
            Some("windup") => 0xFFFF_E040,
            Some("hold") => 0xFFFF_9020,
            Some("hit") => 0xFFFF_3030,
            Some("recover") => 0xFF40_E060,
            _ => 0xFFE0_E0E0,
        };
        let top = m.position.y + m.kind.height;
        let from = [m.position.x, top + 0.3, m.position.z];
        let to = [m.position.x, top + 1.0, m.position.z];
        posts.push(mod_api::WorldLine::new(from, to, color, 4.0, mod_api::WorldDepth::ThroughWalls));
    }
    drop(c);
    if !posts.is_empty() {
        let _ = draw.lines(&posts);
    }
}
