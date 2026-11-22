// The bundle era's entity, mob and player-state calls on 8f83e5c (`game::soulscraft_compat_entities`,
// re-exported by `soulscraft_compat`). Each `sc_<name>` keeps the signature and result of
// 7e4e421's `Game::native_<name>`; the old body is ported where 8f83e5c dropped the function (the
// mob AI calls of native_npcs.rs, the mob helpers of native_mobs.rs), and it forwards to the
// renamed, handle-based function where one exists.
use super::Game;
use super::commands::Target;
use horizon_client::mobs::{Mob, MobWorld, Who};
use horizon_core::geometry::Vec3;
use horizon_core::nbt::Compound;
use mod_api::EntityRef;

/// An effect on a player, as the old `native_add_effect` took it (vanilla's `MobEffectInstance`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeEffect {
    /// `minecraft:speed`, or a content mod's `<namespace>:<path>`.
    pub id: String,
    /// 0 for level I.
    pub amplifier: u8,
    /// Ticks left; -1 for an infinite one.
    pub duration: i32,
    /// From a beacon or a conduit: translucent particles, a framed icon.
    pub ambient: bool,
    /// Whether it shows particles.
    pub particles: bool,
}

/// One mob type's sampled tick cost, as the old `native_take_mob_costs` gave it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeMobCost {
    /// The level, `minecraft:overworld`.
    pub dimension: String,
    /// The mob type, `minecraft:zombie`.
    pub kind: String,
    /// The time its mobs' own ticks took (their riders' inside), in nanoseconds.
    pub nanos: u64,
    /// The mob ticks timed.
    pub mob_ticks: u64,
}

/// An item entity of a level, as `native_item_entities` lists it (unchanged in 8f83e5c).
pub(super) use super::native_hooks::NativeItemEntity;

/// Whether `pos` is inside `/summon`'s bounds (native_mobs.rs `spawnable`, private there).
fn spawnable(pos: [f64; 3]) -> bool {
    pos.iter().all(|v| v.is_finite())
        && (-30_000_000.0..30_000_000.0).contains(&pos[0])
        && (-30_000_000.0..30_000_000.0).contains(&pos[2])
        && (-20_000_000.0..20_000_000.0).contains(&pos[1].floor())
}

/// A level that simulates (not a client's copy) and is the one in place (native_mobs.rs
/// `native_mob_level`, private there).
fn mob_level(g: &Game<'_>, dimension: &str) -> bool {
    g.remote.is_none() && dimension == g.dim.id()
}

/// Whether this game simulates the level `dimension` (native_entities.rs, private there).
fn simulates_level(g: &Game<'_>, dimension: &str) -> bool {
    (g.serving.is_some() || g.options.dedicated.is_some()) && dimension == g.dim.id()
}

/// 7e4e421's `npc_in_level`: `f` with the loaded level `dimension` in place, then the level that
/// was put back (`execute in <dimension> run …`). None on a client, or for a dimension the server
/// hasn't loaded.
pub(super) fn in_level<'g, R>(
    g: &mut Game<'g>,
    dimension: &str,
    f: impl FnOnce(&mut Game<'g>) -> R,
) -> Option<R> {
    if g.remote.is_some() || g.options.execution_role.is_client() {
        return None;
    }
    if dimension == g.dim.id() {
        return Some(f(g));
    }
    let dim = g.levels.parked_dims().into_iter().find(|d| d.id() == dimension)?;
    let back = g.enter_command_level(dim)?;
    let out = f(g);
    let _ = g.enter_command_level(back);
    Some(out)
}

/// 7e4e421's `npc_drive`: `f` on the living mob `mob` of the loaded level `dimension`, with the
/// world its goals see. The mob list is out of the level meanwhile, as in its own tick (8f83e5c's
/// `mob_move_to` takes it the same way).
fn drive<R>(
    g: &mut Game<'_>,
    dimension: &str,
    mob: u32,
    f: impl FnOnce(&mut Mob, &MobWorld<'_>) -> R,
) -> Option<R> {
    in_level(g, dimension, |g| {
        let i = g.mobs.iter().position(|m| m.id == mob && !m.removed && m.alive())?;
        let mut mobs = g.mobs.lift();
        let out = f(&mut mobs[i], &g.mob_world_base(&[]));
        g.mobs.restore(mobs, horizon_client::join_seq::RestoreOrder::SurvivorsFirst);
        Some(out)
    })
    .flatten()
}

/// 7e4e421's `hurt_entity_target`: the player or mob `id` names in the level `dimension`: a player
/// by their entity id (the one in place, whose turn it is, as `Target::Player`), a living mob.
fn hurt_entity_target(g: &Game<'_>, dimension: &str, id: u32) -> Option<Target> {
    let entity = i32::try_from(id).ok()?;
    let guest = g
        .lan
        .iter()
        .flat_map(|h| h.hub.guests.iter())
        .find(|x| x.placed && x.entity_id == entity && x.dimension == dimension);
    if let Some(x) = guest {
        let acting = g.lan_acting == Some(x.conn) && g.player.is_some();
        return Some(if acting { Target::Player } else { Target::Guest(x.conn) });
    }
    // The player in place while the hub is out (a guest's turn), or a LAN host.
    if (g.lan_acting.is_some() || g.options.dedicated.is_none())
        && g.player.as_ref().is_some_and(|p| p.entity_id == entity)
    {
        return Some(Target::Player);
    }
    match g.level_entity(entity)? {
        Target::Mob(mob) if g.mobs.iter().any(|m| m.id == mob && m.alive()) => Some(Target::Mob(mob)),
        _ => None,
    }
}

/// 7e4e421's `npc_mob_attack`, in the level in place: `Mob.doHurtTarget` of `mob` on `target`
/// (a living mob or a guest of this level), with the world and the damage level its tick has.
fn mob_attack_here(g: &mut Game<'_>, mob: u32, target: u32) -> bool {
    let live = |m: &Mob, id: u32| m.id == id && !m.removed && m.alive();
    let Some(i) = g.mobs.iter().position(|m| live(m, mob)) else {
        log::info!("soulscraft: mob_attack refused mob={mob} target={target} why=no-living-mob");
        return false;
    };
    // The guest by its entity id in the level in place, as `sc_hurt_entity` finds it (placed, in
    // this level, alive). The old lookup also asked `hub.here` and `loaded` (the client's
    // PlayerLoaded packet), which the bench's bot clients on 8f83e5c need not pass: the blow was
    // then refused while `sc_hurt_entity`'s blows landed (`revenant_hold`, the bash `hit=false`).
    let dim = g.dim.id().to_owned();
    let guest = g.lan.as_ref().and_then(|h| {
        h.hub
            .guests
            .iter()
            .find(|x| x.placed && !x.dead && x.dimension == dim && u32::try_from(x.entity_id) == Ok(target))
            .map(|x| {
                let moved = [0, 1, 2].map(|k| x.position[k] - x.previous_position[k]);
                (x.conn, x.uuid, moved)
            })
    });
    // The mob code sees one player (`Who::Player`): the guest hit, as in a mob's tick that sees them.
    let (who, player) = match guest {
        Some((conn, uuid, moved)) => {
            let view = g.lan_guest_views().into_iter().find(|(c, _)| *c == conn).map(|(_, v)| v).or_else(|| {
                // Not among the views mobs see (not `loaded`): the same view, made as
                // lan_mobs.rs `lan_guest_views` makes it.
                g.as_guest(conn, |gg| gg.player_view()).flatten().map(|mut v| {
                    v.hero = false;
                    v.velocity = horizon_core::geometry::Vec3::new(moved[0], moved[1], moved[2]);
                    v.uuid = horizon_client::lan::playerdata::uuid_ints(uuid);
                    v.allies = Default::default();
                    v.remote = true;
                    v
                })
            });
            match view {
                Some(view) => (Who::Player, Some(view)),
                None => {
                    log::info!("soulscraft: mob_attack refused mob={mob} target={target} why=no-guest-view conn={conn}");
                    return false;
                }
            }
        }
        None if g.mobs.iter().any(|m| live(m, target)) => (Who::Mob(target), None),
        None => {
            log::info!("soulscraft: mob_attack refused mob={mob} target={target} why=no-such-target dim={dim}");
            return false;
        }
    };
    let snapshot: Vec<_> = g
        .mobs
        .iter()
        .map(|m| super::creatures::seen(m, g.level.player_uuid, g.items()))
        .collect();
    let mut events = Vec::new();
    let mut mobs = g.mobs.lift();
    let mut lent = horizon_client::health::LentLevel::take(&mut g.world);
    {
        let mut w = g.mob_world_base(&snapshot);
        let pos = mobs[i].position;
        w.difficulty = g.local_difficulty([pos.x, pos.y, pos.z]);
        if player.is_some() {
            w.player = player;
        }
        let mut level = horizon_client::health::DamageLevel::lent(
            &g.world,
            g.pack.assets.registry,
            &mut lent,
            Some(g.items()),
            Some(&g.pack.block_registries),
            &g.damage_hooks,
        );
        let mut cx = horizon_client::mobs::goals::Cx {
            w: &w,
            level: &mut level,
            events: &mut events,
        };
        horizon_client::mobs::combat::do_hurt_target(&mut mobs[i], who, &mut cx);
    }
    lent.give_back(&mut g.world);
    g.mobs.restore(mobs, horizon_client::join_seq::RestoreOrder::SurvivorsFirst);
    // What acts on the player goes to the guest, the rest as a tick's events.
    let (to_guest, events): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|e| guest.is_some() && super::lan_mobs::acts_on_player(e));
    if let Err(e) = g.handle_mob_events(events) {
        log::warn!("soulscraft: mob {mob} attacking {target}: {e}");
    }
    if let Some(conn) = guest {
        let mut to_guest: Vec<_> = to_guest.into_iter().map(|e| (conn, mob, e)).collect();
        g.lan_guest_mob_events(&mut to_guest);
    }
    true
}

fn namespaced(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

pub(super) trait ScEntities {
    fn sc_entity(&self, id: u32) -> Option<EntityRef>;
    fn sc_entity_tags(&self, entity: EntityRef) -> Option<Vec<String>>;
    fn sc_entity_add_tag(&mut self, entity: EntityRef, tag: &str) -> bool;
    fn sc_entity_remove_tag(&mut self, entity: EntityRef, tag: &str) -> bool;
    fn sc_entity_bounding_box(&self, entity: EntityRef) -> Option<[[f64; 3]; 2]>;
    fn sc_hurt_entity(&mut self, dimension: &str, target: u32, amount: f32, damage_type: &str, attacker: Option<u32>) -> bool;
    fn sc_kill_mob(&mut self, dimension: &str, id: u32) -> bool;
    fn sc_remove_mob(&mut self, dimension: &str, id: u32) -> bool;
    fn sc_set_mob_health(&mut self, dimension: &str, id: u32, health: f32) -> bool;
    fn sc_summon(&mut self, dimension: &str, kind: &str, pos: [f64; 3], nbt: Compound) -> Option<u32>;
    fn sc_teleport_mob(&mut self, dimension: &str, id: u32, pos: [f64; 3]) -> bool;
    fn sc_teleport_player(&mut self, uuid: u128, dimension: &str, pos: [f64; 3], yaw: f32, pitch: f32) -> bool;
    fn sc_mob_navigate(&mut self, dimension: &str, mob: u32, to: [f64; 3], reach: i32, speed: f64) -> bool;
    fn sc_mob_navigating(&mut self, dimension: &str, mob: u32) -> bool;
    fn sc_mob_stop(&mut self, dimension: &str, mob: u32) -> bool;
    fn sc_mob_look_at(&mut self, dimension: &str, mob: u32, at: [f64; 3]) -> bool;
    fn sc_mob_attack(&mut self, dimension: &str, mob: u32, target: u32) -> bool;
    fn sc_add_effect(&mut self, uuid: u128, effect: &NativeEffect) -> bool;
    fn sc_attribute(&mut self, uuid: u128, attribute: &str) -> Option<(f64, f64)>;
    fn sc_set_player_status(
        &mut self,
        uuid: u128,
        health: Option<f32>,
        food: Option<(i32, f32, f32)>,
        xp: Option<(i32, f32, i32)>,
    ) -> bool;
    fn sc_item_entities(&self, dimension: &str, min: [f64; 3], max: [f64; 3]) -> Vec<NativeItemEntity>;
    fn sc_mob_cost_sampling(&self, on: bool);
    fn sc_take_mob_costs(&self) -> Vec<NativeMobCost>;
    fn sc_tick_stats(&self) -> (f32, f64);
}

impl ScEntities for Game<'_> {
    /// The entity `id` as a handle (native_entity_refs.rs `entity`).
    fn sc_entity(&self, id: u32) -> Option<EntityRef> {
        self.entity(id)
    }
    /// native_entity_data.rs `entity_tags`.
    fn sc_entity_tags(&self, entity: EntityRef) -> Option<Vec<String>> {
        self.entity_tags(entity)
    }
    /// native_entity_data.rs `entity_add_tag`.
    fn sc_entity_add_tag(&mut self, entity: EntityRef, tag: &str) -> bool {
        self.entity_add_tag(entity, tag)
    }
    /// native_entity_data.rs `entity_remove_tag`.
    fn sc_entity_remove_tag(&mut self, entity: EntityRef, tag: &str) -> bool {
        self.entity_remove_tag(entity, tag)
    }
    /// native_entity_motion.rs `entity_bounding_box`.
    fn sc_entity_bounding_box(&self, entity: EntityRef) -> Option<[[f64; 3]; 2]> {
        self.entity_bounding_box(entity)
    }
    /// The old body: `/damage <target> <amount> <type> by <attacker>` through the engine's one
    /// damage entry (world_cmds.rs `hurt_target`; the old `damage_target(.., None, by, from)`), the
    /// knockback from the attacker's position.
    fn sc_hurt_entity(&mut self, dimension: &str, target: u32, amount: f32, damage_type: &str, attacker: Option<u32>) -> bool {
        if !simulates_level(self, dimension) || !amount.is_finite() {
            return false;
        }
        let Some(victim) = hurt_entity_target(self, dimension, target) else {
            return false;
        };
        let by = match attacker {
            Some(id) => match hurt_entity_target(self, dimension, id) {
                Some(t) => Some(t),
                None => return false,
            },
            None => None,
        };
        let from = by.and_then(|t| self.target_position(t));
        self.hurt_target(victim, amount, damage_type, by, None, from)
    }
    /// native_mobs.rs `kill_mob`, unchanged.
    fn sc_kill_mob(&mut self, dimension: &str, id: u32) -> bool {
        self.kill_mob(dimension, id)
    }
    /// No death and no loot: `Entity.discard` (native_entity_remove.rs `entity_discard`, which also
    /// tells the mods and the residency the mob left, where the old body only set `removed`).
    fn sc_remove_mob(&mut self, dimension: &str, id: u32) -> bool {
        if !mob_level(self, dimension) || !self.mobs.iter().any(|m| m.id == id && !m.removed) {
            return false;
        }
        let Some(entity) = self.entity(id) else {
            return false;
        };
        self.entity_discard(entity)
    }
    /// The old body (native_mobs.rs dropped it): the health, cut to the mob's maximum.
    fn sc_set_mob_health(&mut self, dimension: &str, id: u32, health: f32) -> bool {
        if !mob_level(self, dimension) || !health.is_finite() || health <= 0.0 {
            return false;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed && m.alive()) else {
            return false;
        };
        m.health = health.min(m.attrs.max_health);
        m.needs_sync = true;
        true
    }
    /// native_entities.rs `summon`; another loaded level is put in place first, as the old one did
    /// (8f83e5c's refuses another dimension).
    fn sc_summon(&mut self, dimension: &str, kind: &str, pos: [f64; 3], nbt: Compound) -> Option<u32> {
        if (self.serving.is_some() || self.options.dedicated.is_some()) && dimension != self.dim.id() {
            return in_level(self, dimension, |g| g.summon(dimension, kind, pos, nbt)).flatten();
        }
        self.summon(dimension, kind, pos, nbt)
    }
    /// The old body (native_mobs.rs dropped it), as `/tp` does a mob: it dismounts, drops its path
    /// and resets its fall.
    fn sc_teleport_mob(&mut self, dimension: &str, id: u32, pos: [f64; 3]) -> bool {
        if !mob_level(self, dimension) || !spawnable(pos) {
            return false;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
            return false;
        };
        let to = Vec3::new(pos[0], pos[1], pos[2]);
        m.ai.nav.stop();
        m.velocity.y = 0.0;
        m.extra.mount = None;
        m.extra.passenger = false;
        m.position = to;
        m.previous = to;
        m.fall_distance = 0.0;
        m.needs_sync = true;
        true
    }
    /// The player's handle (native_server.rs `player`), then `entity_teleport_to` in their own
    /// level or `entity_teleport_to_level` into another (native_entity_moves.rs). The old function
    /// queued a move made inside another player's turn itself; the handle methods do their own.
    fn sc_teleport_player(&mut self, uuid: u128, dimension: &str, pos: [f64; 3], yaw: f32, pitch: f32) -> bool {
        if !pos.iter().all(|v| v.is_finite()) || !yaw.is_finite() || !pitch.is_finite() {
            return false;
        }
        let Some(player) = self.player(uuid) else {
            return false;
        };
        let rotation = Some([yaw, pitch.clamp(-90.0, 90.0)]);
        let entity = player.entity();
        let here = self.entity_level(entity).is_some_and(|l| l.dimension_id() == dimension);
        if here {
            self.entity_teleport_to(entity, pos, rotation)
        } else {
            self.entity_teleport_to_level(entity, dimension, pos, rotation)
        }
    }
    /// The old body: `PathNavigation.moveTo(x, y, z, reach, speed)` (nav.rs `create_path_to`,
    /// `nav_move_to_path`).
    fn sc_mob_navigate(&mut self, dimension: &str, mob: u32, to: [f64; 3], reach: i32, speed: f64) -> bool {
        if !to.into_iter().all(f64::is_finite) || !speed.is_finite() || reach < 0 {
            return false;
        }
        let at = to.map(|v| v.floor() as i32);
        drive(self, dimension, mob, |m, w| {
            let path = m.create_path_to(w, at, reach);
            m.nav_move_to_path(w, path, speed)
        })
        .unwrap_or(false)
    }
    /// The old body: `PathNavigation.isInProgress`.
    fn sc_mob_navigating(&mut self, dimension: &str, mob: u32) -> bool {
        in_level(self, dimension, |g| {
            g.mobs
                .iter()
                .any(|m| m.id == mob && !m.removed && m.alive() && m.ai.nav.is_in_progress())
        })
        .unwrap_or(false)
    }
    /// The old body: `PathNavigation.stop` (nav.rs `nav_stop`).
    fn sc_mob_stop(&mut self, dimension: &str, mob: u32) -> bool {
        drive(self, dimension, mob, |m, _| m.nav_stop()).is_some()
    }
    /// The old body: `LookControl.setLookAt` for the mob's next tick.
    fn sc_mob_look_at(&mut self, dimension: &str, mob: u32, at: [f64; 3]) -> bool {
        if !at.into_iter().all(f64::is_finite) {
            return false;
        }
        drive(self, dimension, mob, |m, _| m.look_at(at)).is_some()
    }
    /// The old body (no 8f83e5c function makes a mob strike): `Mob.doHurtTarget` (mobs/combat.rs).
    fn sc_mob_attack(&mut self, dimension: &str, mob: u32, target: u32) -> bool {
        if mob == target {
            return false;
        }
        in_level(self, dimension, |g| mob_attack_here(g, mob, target)).unwrap_or(false)
    }
    /// `LivingEntity.addEffect` through the player's living handle (native_living_effects.rs
    /// `living_add_effect`). False for an unknown effect, a duration of 0 or below -1, a player not
    /// on.
    fn sc_add_effect(&mut self, uuid: u128, effect: &NativeEffect) -> bool {
        if effect.duration == 0 || effect.duration < -1 {
            return false;
        }
        let Some(key) = self.effect_key(&namespaced(&effect.id)) else {
            return false;
        };
        let Some(living) = self.player(uuid).and_then(|p| self.living(p.entity())) else {
            return false;
        };
        let active = mod_api::ActiveEffect::new(
            key,
            effect.duration,
            i32::from(effect.amplifier),
            effect.ambient,
            effect.particles,
            true,
        );
        self.living_add_effect(living, &active)
    }
    /// The base value and the value with every modifier (native_living_refs.rs
    /// `living_attribute_base`, `living_attribute_value`).
    fn sc_attribute(&mut self, uuid: u128, attribute: &str) -> Option<(f64, f64)> {
        let id = namespaced(attribute);
        let living = self.player(uuid).and_then(|p| self.living(p.entity()))?;
        Some((self.living_attribute_base(living, &id)?, self.living_attribute_value(living, &id)?))
    }
    /// Food and experience, then health, as 7e4e421 did (native_player_state.rs
    /// `native_set_player_status`): food through the player's handle (native_player_food.rs), the
    /// experience and the health with the player's state in place (native_server.rs
    /// `native_as_player_handle`, the turn `player_set_experience_levels` writes in). Level,
    /// progress and total are written together, as Bukkit's `setLevel`, `setExp` and
    /// `setTotalExperience`; 8f83e5c's host has no setter for the total. Health above 0 is set and
    /// capped at the maximum; 0 or less kills as `/kill` does for the player in place
    /// (administration/commands/mod.rs `kill_target_in_level`, `Target::Player`), with
    /// vanilla's generic death message.
    fn sc_set_player_status(
        &mut self,
        uuid: u128,
        health: Option<f32>,
        food: Option<(i32, f32, f32)>,
        xp: Option<(i32, f32, i32)>,
    ) -> bool {
        let bad_food = food.is_some_and(|(_, s, e)| !s.is_finite() || !e.is_finite());
        let bad_xp = xp.is_some_and(|(l, p, t)| l < 0 || t < 0 || !(0.0..=1.0).contains(&p));
        if health.is_some_and(|h| !h.is_finite()) || bad_food || bad_xp {
            return false;
        }
        let Some(player) = self.player(uuid) else {
            return false;
        };
        if let Some((level, saturation, exhaustion)) = food {
            self.player_set_food_level(player, level.clamp(0, 20));
            self.player_set_saturation(player, saturation.max(0.0));
            self.player_set_exhaustion(player, exhaustion.clamp(0.0, 40.0));
        }
        if xp.is_none() && health.is_none() {
            return true;
        }
        self.native_as_player_handle(player, |game| {
            if let Some((level, progress, total)) = xp {
                game.experience.level = level;
                game.experience.progress = progress;
                game.experience.total = total;
            }
            if let (Some(health), Some(p)) = (health, game.player.as_mut()) {
                if health > 0.0 {
                    let _ = p.set_health(health, false);
                    let _ = p.clamp_health_to_max();
                } else {
                    // LivingEntity.die(damageSources().generic()).
                    let _ = p.set_health(0.0, false);
                    game.ui.death_cause = Some("death.attack.generic".into());
                }
            }
        })
    }
    /// native_hooks/shared.rs `native_item_entities`, unchanged.
    fn sc_item_entities(&self, dimension: &str, min: [f64; 3], max: [f64; 3]) -> Vec<NativeItemEntity> {
        self.native_item_entities(dimension, min, max)
    }
    /// The old body over horizon_gameplay's tick profile (native_diagnostics.rs `mob_cost_sampling`
    /// does the same, refusing a client, with `&mut self`).
    fn sc_mob_cost_sampling(&self, on: bool) {
        horizon_gameplay::tick_profile::set_mod_sampling(on);
    }
    /// The old body over horizon_gameplay's tick profile, costliest first; clears them.
    fn sc_take_mob_costs(&self) -> Vec<NativeMobCost> {
        horizon_gameplay::tick_profile::take_mod()
            .into_iter()
            .map(|c| NativeMobCost {
                dimension: c.dimension.to_owned(),
                kind: c.kind,
                nanos: c.nanos,
                mob_ticks: c.mob_ticks,
            })
            .collect()
    }
    /// native_server.rs `tick_stats`, unchanged.
    fn sc_tick_stats(&self) -> (f32, f64) {
        self.tick_stats()
    }
}

/// The level-side calls of the old API, on the `GameLevel` a `with_level` closure gets.
pub(super) trait ScLevel {
    fn sc_play_sound(&mut self, sound: &str, at: [f64; 3], source: usize, volume: f32, pitch: f32) -> bool;
    fn sc_send_particles(
        &mut self,
        options: &horizon_client::particles::ParticleOptions,
        state: Option<horizon_world::registry::BlockStateId>,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool;
}

impl ScLevel for super::GameLevel<'_> {
    /// native_hooks/shared.rs `GameLevel::native_play_sound`, unchanged.
    fn sc_play_sound(&mut self, sound: &str, at: [f64; 3], source: usize, volume: f32, pitch: f32) -> bool {
        self.native_play_sound(sound, at, source, volume, pitch)
    }
    /// native_hooks/shared.rs `GameLevel::native_send_particles`, unchanged.
    fn sc_send_particles(
        &mut self,
        options: &horizon_client::particles::ParticleOptions,
        state: Option<horizon_world::registry::BlockStateId>,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool {
        self.native_send_particles(options, state, at, spread, speed, count)
    }
}
