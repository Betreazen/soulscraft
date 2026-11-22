// The client half of the bundle era's game API on 8f83e5c (`game::soulscraft_compat_client`,
// re-exported by soulscraft_compat): HUD text and fills, block entity drawing, the scene in a
// screen, the mod screen, the resources, the serverbound payload. Each `sc_<name>` keeps the
// 7e4e421 `native_<name>`'s arguments and result.
//
// The pose stack of a `BlockEntityDraw` (push, pop, translate, rotate_about, scale) and its
// `light`, `partial` and `game_time` are private in 8f83e5c: the glue calls the game's
// `native_draw_push`, `native_draw_light`, … (game/modding/native_block_entity_draw.rs:935-1010).
use super::Game;
use super::native_block_entity_draw::BlockEntityDraw;
use horizon_assets::item_model::DisplayContext;
use horizon_client::client_resources::ClientResources;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::registry::BlockStateId;

/// A block of a scene drawn in a screen (the bundle era's `mod_api::NativeSceneBlock`): its
/// position in the scene, its state, and the block entity drawn with it. 8f83e5c takes the block
/// entity as owned data (`mod_api::SceneBlockEntity`, id and fields), not a borrowed world entity:
/// the scene makes a `mod_api::ModBlockEntity` of it for `native_block_entity_submit`.
pub(super) struct NativeSceneBlock<'a> {
    pub pos: [i32; 3],
    pub state: BlockStateId,
    pub entity: Option<&'a mod_api::SceneBlockEntity>,
}

/// How a scene is seen (the bundle era's `mod_api::NativeSceneView`).
#[derive(Clone, Copy, Debug)]
pub(super) struct NativeSceneView {
    pub center: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub scale: f32,
}

pub(super) trait ScClient {
    fn sc_text(&self, batch: &mut HudBatch, text: &str, at: [f32; 2], rgb: u32, shadow: bool);
    fn sc_text_width(&self, text: &str) -> f32;
    fn sc_fill(&self, batch: &mut HudBatch, rect: [f32; 4], argb: u32);
    fn sc_draw_block(&self, draw: &mut BlockEntityDraw, block: &str, properties: &[(&str, &str)]) -> bool;
    fn sc_draw_item(
        &self,
        draw: &mut BlockEntityDraw,
        stack: &ItemStack,
        display: DisplayContext,
        owner_yaw: Option<f32>,
        seed: i32,
    ) -> bool;
    fn sc_draw_model_part(&self, draw: &mut BlockEntityDraw, part: &mod_api::ModelPart, sprite: &str, tint: u32) -> bool;
    fn sc_draw_text(
        &self,
        draw: &mut BlockEntityDraw,
        text: &Text,
        at: [f32; 2],
        color: u32,
        glowing: bool,
        outline: u32,
    ) -> f32;
    fn sc_draw_scene(&self, batch: &mut HudBatch, rect: [f32; 4], blocks: &[NativeSceneBlock<'_>], view: NativeSceneView);
    fn sc_item_model(&self, batch: &mut HudBatch, item: &str, model: &str, at: [f32; 2], size: f32) -> bool;
    fn sc_mod_screen(&self) -> Option<&str>;
    fn sc_mod_screen_open(&mut self, mod_id: &str) -> bool;
    fn sc_playing(&self) -> bool;
    fn sc_client_resources(&self) -> ClientResources;
    fn sc_net_send_to_server(&mut self, channel: &str, data: &[u8]) -> Result<(), String>;
}

impl ScClient for Game<'_> {
    /// The old call took 0xRRGGBB and drew it opaque; 8f83e5c's `native_text` takes ARGB and draws
    /// nothing with a zero alpha (native_hooks/base.rs:4691).
    fn sc_text(&self, batch: &mut HudBatch, text: &str, at: [f32; 2], rgb: u32, shadow: bool) {
        self.native_text(batch, text, at, 0xFF00_0000 | (rgb & 0xFF_FFFF), shadow);
    }

    /// 8f83e5c's `native_text_width` wants the batch (native_hooks/draw_measures.rs:8) only to
    /// ignore it; this measures with the same atlas, so the menus can wrap before they draw.
    fn sc_text_width(&self, text: &str) -> f32 {
        HudBatch::text_width(&self.gui.atlas, text)
    }

    /// ARGB, as before (native_hooks/base.rs:4500).
    fn sc_fill(&self, batch: &mut HudBatch, rect: [f32; 4], argb: u32) {
        self.native_fill(batch, rect, argb);
    }

    /// modding/native_block_entity_draw.rs:1163, unchanged.
    fn sc_draw_block(&self, draw: &mut BlockEntityDraw, block: &str, properties: &[(&str, &str)]) -> bool {
        self.native_draw_block(draw, block, properties)
    }

    /// modding/native_block_entity_draw.rs:1298, unchanged.
    fn sc_draw_item(
        &self,
        draw: &mut BlockEntityDraw,
        stack: &ItemStack,
        display: DisplayContext,
        owner_yaw: Option<f32>,
        seed: i32,
    ) -> bool {
        self.native_draw_item(draw, stack, display, owner_yaw, seed)
    }

    /// modding/native_block_entity_draw.rs:1248; the part tree is `mod_api::ModelPart` now (the
    /// same builders, moved from mobs::model_layers into the API).
    fn sc_draw_model_part(&self, draw: &mut BlockEntityDraw, part: &mod_api::ModelPart, sprite: &str, tint: u32) -> bool {
        self.native_draw_model_part(draw, part, sprite, tint)
    }

    /// modding/native_block_entity_draw.rs:1052: the outline is an `Option` now (`Some(0)` is a
    /// black ring); the old 0 meant none.
    fn sc_draw_text(
        &self,
        draw: &mut BlockEntityDraw,
        text: &Text,
        at: [f32; 2],
        color: u32,
        glowing: bool,
        outline: u32,
    ) -> f32 {
        let outline = (outline != 0).then_some(outline);
        self.native_draw_text(draw, text, at, color, glowing, outline)
    }

    /// A scene of blocks alone (no entities, particles or clock): modding/native_scene_live.rs:211
    /// takes it to `draw_scene_quads` (modding/native_scene.rs:91), which calls
    /// `native_block_entity_submit` for each block entity (native_scene.rs:164-178). `rect` is the
    /// corners, as before. A refusal is logged once in a while, as the old call drew nothing.
    fn sc_draw_scene(&self, batch: &mut HudBatch, rect: [f32; 4], blocks: &[NativeSceneBlock<'_>], view: NativeSceneView) {
        let blocks: Vec<super::native_scene::SceneBlock<'_>> = blocks
            .iter()
            .map(|b| super::native_scene::SceneBlock { pos: b.pos, state: b.state, entity: b.entity })
            .collect();
        let view = mod_api::SceneView::new(view.center, view.yaw, view.pitch, view.scale);
        let scene = mod_api::Scene::new(Vec::new());
        if let Err(e) = self.native_draw_scene(batch, rect, &blocks, &scene, view) {
            static LOGGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            if LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 600 == 0 {
                log::warn!("soulscraft: scene not drawn: {e:?}");
            }
        }
    }

    /// native_hooks/base.rs:4604, unchanged.
    fn sc_item_model(&self, batch: &mut HudBatch, item: &str, model: &str, at: [f32; 2], size: f32) -> bool {
        self.native_item_model(batch, item, model, at, size)
    }

    /// Renamed `mod_screen` (native_hooks/base.rs:4675).
    fn sc_mod_screen(&self) -> Option<&str> {
        self.mod_screen()
    }

    /// Renamed `mod_screen_open` (native_hooks/base.rs:4650).
    fn sc_mod_screen_open(&mut self, mod_id: &str) -> bool {
        self.mod_screen_open(mod_id)
    }

    /// Renamed `playing` (native_hooks/client_input_reads.rs:9), the same test.
    fn sc_playing(&self) -> bool {
        self.playing()
    }

    /// The loaded resources' tree, as before (horizon-client-render/src/client_resources.rs:26;
    /// the game's `options.resources`). The mod package's own assets are in it.
    fn sc_client_resources(&self) -> ClientResources {
        ClientResources::new(self.options.resources.clone())
    }

    /// The serverbound payload `channel` (declared `play_to_server` in lib.rs) with exactly the
    /// old bytes: modding/payload_transport.rs:41, queued until the root transaction commits.
    fn sc_net_send_to_server(&mut self, channel: &str, data: &[u8]) -> Result<(), String> {
        self.send_payload_to_server(&mod_api::PayloadId(channel.to_owned()), mod_api::PayloadBytes(data.to_vec()))
            .map(|_| ())
            .map_err(|e| format!("{e:?}"))
    }
}

/// A line of text over something in the world, facing the camera as a name tag does
/// (`EntityRenderer.renderNameTag`: the camera's rotation, then 0.025 of a block a font pixel),
/// drawn in `draw`'s pose, full bright with a black ring so it reads on any background. `at` is in
/// blocks from the pose's origin; `turn` undoes the pose's own turn about Y (a mob's draw is turned
/// with its body: pass its body yaw; a block entity's is not turned: 0). 8f83e5c's world geometry
/// draws no text, so the old `WorldPrimitives` labels come here, through the game's world text
/// path (`native_draw_text`, the one signs, text displays and name tags use). Unlike the old
/// labels it is depth tested: walls hide it.
pub(super) fn draw_label(game: &Game<'_>, draw: &mut BlockEntityDraw, at: [f32; 3], turn: f32, line: &str, argb: u32) {
    // The camera's facing, turned round in third-person front view (interface/names.rs
    // `styled_name_tag_quads`).
    let (yaw, pitch) = match game.posed_rotation {
        Some(rotation) => rotation,
        None if game.camera_type == mod_api::CameraType::ThirdPersonFront => (game.yaw + 180.0, -game.pitch),
        None => (game.yaw, game.pitch),
    };
    let text = Text::literal(line.to_owned());
    game.native_draw_push(draw);
    game.native_draw_translate(draw, at);
    game.native_draw_rotate_about(draw, [0.0, 1.0, 0.0], turn, [0.0; 3]);
    // Camera.rotation(): rotationYXZ(180° - yaw, -pitch, 0).
    game.native_draw_rotate_about(draw, [0.0, 1.0, 0.0], 180.0 - yaw, [0.0; 3]);
    game.native_draw_rotate_about(draw, [1.0, 0.0, 0.0], -pitch, [0.0; 3]);
    game.native_draw_scale(draw, [0.025, -0.025, 0.025]);
    let width = game.native_draw_text_width(draw, &text);
    game.native_draw_text(draw, &text, [-width / 2.0, 0.0], argb & 0xFF_FFFF, true, Some(0));
    game.native_draw_pop(draw);
}
