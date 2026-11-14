"""Renders PNG previews of built models from mobs.blend into assets/models/previews/.

    blender --background assets/models/mobs.blend --python assets/models/preview.py -- revenant
    blender --background assets/models/mobs.blend --python assets/models/preview.py -- all
    blender --background assets/models/blocks.blend --python assets/models/preview.py -- blocks
    blender --background assets/models/mobs.blend --python assets/models/preview.py -- site

Views of a mob (only clips it has; a flying mob, env 'air' in data/mobs.json, also from below):
    rest pose front, side and 3/4 (flyers: and from below)
    idle 3/4 and front, a close-up of the idle stance from the weapon side
    every state flag of its parts (part(..., state=...)) on, idle 3/4
    every attack clip: windup (the hold marker of a delayed attack, else hit - 2, where the
    windup pose key sits), the hit marker and the recover marker
    every other clip: one key frame (the middle of a one-shot clip, a quarter into a loop)
and <id>_sheet.png with all of them. Before rendering, the mob's old previews are deleted, so
the folder holds exactly the current set. Each shot is framed by the model's bounds: the rest
pose sets one scale for the mob, a frame that reaches further gets a wider shot.

EEVEE with a sun, texture colours, orthographic cameras; the showcase uses the same light
(viewer.js). Not part of the deterministic build: renders may differ between machines.
"""
import glob
import json
import os
import sys

import bpy
import numpy as np
from mathutils import Euler, Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
OUT = os.path.join(HERE, 'previews')
SIZE = 360
MARGIN = 1.12  # ortho scale over the model's extent
WIDEST = 1.6   # a shot widens at most this far over the mob's rest scale

# Camera directions (Blender world: +X the mob's left, -Y its front, +Z up), from the target.
VIEWS = {
    'front': Vector((0, -8, 0)),
    'side': Vector((8, 0, 0)),                 # the mob's left side
    'three_quarter': Vector((5.5, -5.5, 2.0)),
    'three_quarter_r': Vector((-5.5, -5.5, 1.6)),  # the right (weapon) side
    'below': Vector((3.0, -5.0, -5.5)),        # a flyer seen from the ground
    # Blocks (blocks.blend): Blender -Y is the block's south, +Y its north (where models face).
    'three_quarter_n': Vector((5.5, 5.5, 3.2)),
    'north': Vector((0, 8, 1.2)),
    'top': Vector((0.4, -1.2, 8)),
    'level': Vector((6.5, -5.0, 4.6)),
}


def setup(scene):
    """EEVEE, a sun from the front right, high, and a grey world: faces turned to the sun are
    lighter, the others get the world's grey, close to the light of a mob in the game."""
    scene.render.engine = 'BLENDER_EEVEE'
    scene.eevee.taa_render_samples = 16
    scene.render.resolution_x = scene.render.resolution_y = SIZE
    scene.render.film_transparent = False
    scene.view_settings.view_transform = 'Standard'
    world = bpy.data.worlds.get('preview') or bpy.data.worlds.new('preview')
    scene.world = world
    bg = next(n for n in world.node_tree.nodes if n.type == 'BACKGROUND')
    bg.inputs['Color'].default_value = (0.28, 0.3, 0.35, 1)
    bg.inputs['Strength'].default_value = 1.0
    sun = bpy.data.objects.new('preview_sun', bpy.data.lights.new('preview_sun', 'SUN'))
    scene.collection.objects.link(sun)
    sun.data.energy = 2.4
    sun.data.angle = 0.2
    sun.rotation_euler = (0.75, 0.0, -0.5)
    target = bpy.data.objects.new('preview_target', None)
    scene.collection.objects.link(target)
    cam = bpy.data.objects.new('preview_cam', bpy.data.cameras.new('preview_cam'))
    scene.collection.objects.link(cam)
    cam.data.type = 'ORTHO'
    cam.data.clip_end = 200
    con = cam.constraints.new('TRACK_TO')
    con.target = target
    con.track_axis = 'TRACK_NEGATIVE_Z'
    con.up_axis = 'UP_Y'
    scene.camera = cam
    return cam, target


CURRENT = {'clip': None}  # the clip play() set, for the framing


def play(scene, clip):
    """Mutes every NLA track but the clip; clip None puts the parts back into the rest pose."""
    CURRENT['clip'] = clip
    for ob in scene.objects:
        if ob.animation_data:
            for tr in ob.animation_data.nla_tracks:
                tr.mute = clip is None or tr.name != clip
        if clip is None and 'soulscraft_rest' in ob:
            rest = list(ob['soulscraft_rest'])
            ob.location, ob.rotation_euler, ob.scale = rest[:3], rest[3:], (1, 1, 1)


def set_states(scene, flags):
    """Parts with a state flag render only while it is on ('!flag': only while it is off)."""
    for ob in scene.objects:
        state = ob.get('soulscraft_state')
        if state:
            flag, on = state.lstrip('!'), not state.startswith('!')
            hide = (flag in flags) != on
            for o in [ob] + list(ob.children_recursive):
                o.hide_render = hide


def basis(ob, clip, frame):
    """The object's local matrix at a frame of a clip, from the clip's own F-curves; objects the
    clip does not key (sockets, held items) keep their matrix_basis. The bounds do not go through
    the depsgraph: in background mode it does not follow every mute change and frame (a render
    does)."""
    tracks = ob.animation_data.nla_tracks if (clip and ob.animation_data) else ()
    track = next((t for t in tracks if t.name == clip), None)
    if not track or not track.strips:
        return ob.matrix_basis
    strip = track.strips[0]
    bag = strip.action.layers[0].strips[0].channelbag(strip.action_slot)
    ch = {'location': list(ob.location), 'rotation_euler': list(ob.rotation_euler), 'scale': list(ob.scale)}
    for fc in bag.fcurves:
        if fc.data_path in ch:
            ch[fc.data_path][fc.array_index] = fc.evaluate(frame)
    return Matrix.LocRotScale(Vector(ch['location']), Euler(ch['rotation_euler'], ob.rotation_mode), Vector(ch['scale']))


def points(scene, clip):
    """World positions of every vertex that renders, at the scene's frame of the clip."""
    frame, cache = scene.frame_current, {}

    def world(ob):
        if ob.name not in cache:
            m = basis(ob, clip, frame)
            cache[ob.name] = world(ob.parent) @ ob.matrix_parent_inverse @ m if ob.parent else m
        return cache[ob.name]
    out = []
    for ob in scene.objects:
        if ob.type == 'MESH' and not ob.hide_render:
            m = world(ob)
            out.extend(m @ v.co for v in ob.data.vertices)
    return out


def extent(pts, direction):
    """Centre and size (the larger of width and height) of the points seen along direction."""
    fwd = -direction.normalized()
    right = fwd.cross(Vector((0, 0, 1)))
    right = right.normalized() if right.length > 1e-6 else Vector((1, 0, 0))
    up = right.cross(fwd).normalized()
    xs = [p.dot(right) for p in pts]
    ys = [p.dot(up) for p in pts]
    zs = [p.dot(fwd) for p in pts]
    centre = right * (min(xs) + max(xs)) / 2 + up * (min(ys) + max(ys)) / 2 + fwd * (min(zs) + max(zs)) / 2
    return centre, max(max(xs) - min(xs), max(ys) - min(ys))


class Shooter:
    def __init__(self, mob, scene):
        self.mob, self.scene = mob, scene
        self.cam, self.target = setup(scene)
        self.shots = []
        play(scene, None)
        set_states(scene, ())
        scene.frame_set(0)
        rest = points(scene, None)
        # One scale for the mob from its rest pose, the widest of the side views.
        self.base = max(extent(rest, VIEWS[v])[1] for v in ('front', 'side', 'three_quarter')) * MARGIN
        self.centre = sum(rest, Vector()) / len(rest)

    def render(self, view, name, zoom=1.0):
        """zoom < 1 is a close-up at that part of the mob's scale, centred on the model. A shot
        widens for a pose that reaches further, up to WIDEST times the mob's scale; whatever is
        further off (a bolt in flight) is left out of the framing."""
        pts = points(self.scene, CURRENT['clip'])
        centre, size = extent(pts, VIEWS[view])
        if size * MARGIN > self.base * WIDEST:
            near = [p for p in pts if (p - self.centre).length < self.base * WIDEST / 2]
            centre, size = extent(near or pts, VIEWS[view])
        self.target.location = centre
        self.cam.location = centre + VIEWS[view].normalized() * 40
        self.cam.data.ortho_scale = self.base * zoom if zoom != 1.0 else min(max(self.base, size * MARGIN), self.base * WIDEST)
        path = os.path.join(OUT, '%s_%s.png' % (self.mob, name))
        self.scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        self.shots.append(path)


def clip_actions(mob):
    return {a.name.split(':', 1)[1]: a for a in bpy.data.actions if a.name.startswith(mob + ':')}


def load_mob(mob):
    with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
        return next((m for m in json.load(f)['mobs'] if m['id'] == mob), None)


def weapon_view(scene):
    """The close-up looks at the side with a held item, the right hand by default."""
    held = [o for o in scene.objects if ':held_' in o.name]
    if held and all(o.parent and o.parent.name.endswith('socket_hand_l') for o in held):
        return 'three_quarter'
    return 'three_quarter_r'


def preview(mob):
    os.makedirs(OUT, exist_ok=True)
    for old in glob.glob(os.path.join(OUT, mob + '_*.png')):
        os.remove(old)
    scene = bpy.data.scenes[mob]
    bpy.context.window.scene = scene
    data = load_mob(mob)
    flyer = bool(data and (data['env'] == 'air' or data['stats'].get('flying')))
    acts = clip_actions(mob)
    s = Shooter(mob, scene)

    for v in ('front', 'side', 'three_quarter') + (('below',) if flyer else ()):
        s.render(v, 'rest_' + v)
    if 'idle' in acts:
        play(scene, 'idle')
        scene.frame_set(0)
        s.render('three_quarter', 'idle_three_quarter')
        s.render('front', 'idle_front')
        s.render(weapon_view(scene), 'idle_closeup', zoom=0.66)
    flags = sorted({o['soulscraft_state'].lstrip('!') for o in scene.objects if o.get('soulscraft_state')})
    for flag in flags:
        set_states(scene, (flag,))
        s.render('front', 'state_' + flag)
        set_states(scene, ())

    attacks = {a['clip'] for a in (data or {}).get('attacks', [])}
    for clip in sorted(acts):
        act = acts[clip]
        marks = {m.name: m.frame for m in act.pose_markers}
        play(scene, clip)
        if clip in attacks and 'hit' in marks:
            windup = marks['hold'] if 'hold' in marks else max(0, marks['hit'] - 2)
            for label, frame in (('windup', windup), ('hit', marks['hit']), ('recover', marks['recover'])):
                scene.frame_set(frame)
                s.render('three_quarter' if clip == 'shield_bash' else weapon_view(scene), '%s_%s' % (clip, label))
                if flyer:
                    s.render('below', '%s_%s_below' % (clip, label))
        elif clip != 'idle':
            end = int(act.frame_range[1])
            frame = end // 4 if act.use_cyclic else end // 2
            scene.frame_set(frame)
            s.render('side' if clip in ('walk', 'run', 'swim') else 'three_quarter', '%s_%d' % (clip, frame))
    sheet(s.shots, os.path.join(OUT, mob + '_sheet.png'))
    print('preview.py: %d views of %s in %s' % (len(s.shots), mob, OUT))


def sheet(paths, out, cols=4, size=SIZE):
    rows = (len(paths) + cols - 1) // cols
    canvas = np.zeros((rows * size, cols * size, 4), dtype=np.float32)
    canvas[..., 3] = 1
    for i, p in enumerate(paths):
        img = bpy.data.images.load(p)
        px = np.array(img.pixels[:], dtype=np.float32).reshape(size, size, 4)
        r, c = rows - 1 - i // cols, i % cols  # Blender pixel rows start at the bottom
        canvas[r * size:(r + 1) * size, c * size:(c + 1) * size] = px
        bpy.data.images.remove(img)
    im = bpy.data.images.new('sheet', cols * size, rows * size, alpha=True)
    im.pixels = canvas.ravel()
    im.filepath_raw = out
    im.file_format = 'PNG'
    im.save()


def preview_blocks():
    """blocks.blend: every block scene (three-quarter, and from the north for blocks that face
    a way, banners) and every structure scene (three-quarter, top, front, larger), into
    previews/altar_*.png with altar_blocks_sheet.png and altar_levels_sheet.png. A block whose id
    does not start with altar_ gets the prefix (banner_weight -> altar_banner_weight)."""
    os.makedirs(OUT, exist_ok=True)
    for old in glob.glob(os.path.join(OUT, 'altar_*.png')):
        os.remove(old)
    block_shots, level_shots = [], []
    for scene in [sc for sc in bpy.data.scenes if not sc.name.startswith('export:') and sc.get('soulscraft_set', 'altar') in ('altar', 'structure')]:
        bpy.context.window.scene = scene
        name = scene.name if scene.name.startswith('altar_') else 'altar_' + scene.name
        s = Shooter(name, scene)
        if scene.name.startswith('altar_level_'):
            big = SIZE * 2
            scene.render.resolution_x = scene.render.resolution_y = big
            for v in ('level', 'top', 'front'):
                s.render(v, v)
            level_shots.append(s.shots[0])
            continue
        faces = any('banner_' in o.name for o in scene.objects)
        s.render('three_quarter_n' if faces else 'three_quarter', 'three_quarter')
        s.render('north' if faces else 'front', 'front')
        block_shots.extend(s.shots)
    sheet(block_shots, os.path.join(OUT, 'altar_blocks_sheet.png'), cols=6)
    sheet(level_shots, os.path.join(OUT, 'altar_levels_sheet.png'), cols=2, size=SIZE * 2)
    print('preview.py: %d block and %d structure views in %s' % (len(block_shots), len(level_shots), OUT))


TROPHY_VIEW = Vector((5.5, 5.5, 3.2))  # heads: from the north-east, above (they face north)
TROPHY_SIZE = 400
TROPHY_SCALE = 1.7  # ortho scale, blocks: every head shot at one scale, so sizes compare
# Dioramas: from the north-west, above, as a player standing in front and a little to the left
# sees one on a shelf; a close view, a front view, the block as big as it is on screen from 3
# blocks away (about a quarter of a 1080 px screen, so 260 px), and night: no sun, a dim moon
# and the block's own light, so the glow reads.
DIORAMA_VIEW = Vector((-4.6, 6.4, 3.6))
DIORAMA_FRONT = Vector((0, 6, 1.2))
DIORAMA_SIZE = 520
DIORAMA_FAR = 260
DIORAMA_SCALE = 1.42
FIGURE_SCALE = 0.8  # the close-up of the figure


def preview_trophies(only=None):
    """mobs.blend with the diorama blocks appended from blocks.blend: every mob's head (floor,
    facing north and turned 2 steps, and on a wall) and its diorama with the figure in its trophy
    clip, each drawn from assets/rigs/<mob>.trophy.json and rig.json the way the mod's block
    entity will (part matrices from the JSON numbers, not the Blender rig). Writes
    previews/head_<mob>_*.png, trophy_<mob>[_front|_far|_night|_figure|_t<tick>].png and the
    sheets head_sheet.png, trophy_sheet.png, trophy_front_sheet.png, trophy_far_sheet.png,
    trophy_night_sheet.png, trophy_figure_sheet.png."""
    sys.path.insert(0, HERE)
    sys.dont_write_bytecode = True
    import blocks
    import rig as rigmod
    import trophies
    os.makedirs(OUT, exist_ok=True)
    for pat in ('head_*.png', 'trophy_*.png'):
        for old in glob.glob(os.path.join(OUT, pat)):
            os.remove(old)
    with bpy.data.libraries.load(os.path.join(HERE, 'blocks.blend')) as (src, dst):
        dst.meshes = [n for n in src.meshes if n.endswith('_trophy')]
    meshes = {m.name[len('block:'):]: m for m in bpy.data.meshes if m.name.endswith('_trophy')}
    stone = bpy.data.images.load(os.path.join(ROOT, 'assets', 'soulscraft', 'textures', 'block', 'altar_stone.png'))
    wall_model = blocks.Model('preview_wall')
    wall_model.box((0, 0, 0), (16, 16, 16), 'altar_stone')
    wall_mesh = blocks.make_mesh(wall_model, lambda tex, glow: rigmod.texture_material('preview_wall', stone))
    kb = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))       # MC / draw frame -> Blender
    k4 = rigmod.K.to_4x4()
    entry = trophies.entry()
    with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
        mob_ids = [m['id'] for m in json.load(f)['mobs']]
    shots = {k: [] for k in ('head', 'trophy', 'front', 'far', 'night', 'figure')}

    def shoot(scene, cam, target, name, view=TROPHY_VIEW, scale=TROPHY_SCALE, size=TROPHY_SIZE, at=(0, 0, 0.5)):
        target.location = at
        cam.location = Vector(at) + view.normalized() * 40
        cam.data.ortho_scale = scale
        scene.render.resolution_x = scene.render.resolution_y = size
        path = os.path.join(OUT, name + '.png')
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        return path

    def block_object(scene, mesh, at=(0, 0, 0), name='trophy_base'):
        ob = bpy.data.objects.new(name, mesh)
        scene.collection.objects.link(ob)
        ob.matrix_world = blocks.blender_matrix(at, 0)
        return ob

    def night(scene, sun, light):
        """Moonlight and the block's light: a dim blue world, no sun, a warm point light inside
        the block as strong as its light level (0..15)."""
        bg = next(n for n in scene.world.node_tree.nodes if n.type == 'BACKGROUND')
        bg.inputs['Color'].default_value = (0.03, 0.035, 0.06, 1)
        sun.data.energy = 0.15
        lamp = bpy.data.objects.new('preview_lamp', bpy.data.lights.new('preview_lamp', 'POINT'))
        scene.collection.objects.link(lamp)
        lamp.data.energy = 6 * light
        lamp.data.color = (1.0, 0.86, 0.66)
        lamp.data.shadow_soft_size = 0.3
        lamp.location = (0, 0.3, 0.8)
        return lamp, bg

    for mob in mob_ids:
        if only and mob not in only:
            continue
        path = os.path.join(ROOT, 'assets', 'rigs', mob + '.trophy.json')
        if not os.path.exists(path) or mob not in bpy.data.scenes:
            continue
        with open(path, encoding='utf-8') as f:
            t = json.load(f)
        with open(os.path.join(ROOT, 'assets', 'rigs', mob + '.rig.json'), encoding='utf-8') as f:
            rj = json.load(f)
        scene = bpy.data.scenes[mob]
        bpy.context.window.scene = scene
        cam, target = setup(scene)
        sun = scene.objects['preview_sun'] if 'preview_sun' in scene.objects else next(o for o in scene.objects if o.type == 'LIGHT')
        play(scene, None)
        set_states(scene, ())
        parts = {p['name']: bpy.data.objects[rigmod.obj_name(types_ns(mob), p['name'])] for p in rj['parts']}
        for ob in parts.values():
            ob.parent = None
        held = [o for o in scene.objects if ':held_' in o.name]

        def draw(place, part_list, rows, shown):
            """Puts every part where the game chain puts it: Blender = KB @ place @ entry @ W @ K."""
            world = trophies.world_of(part_list, rows)
            for name, ob in parts.items():
                vis = name in shown and name in world
                ob.hide_render = not vis
                if vis:
                    ob.matrix_world = kb @ place @ entry @ world[name] @ k4
            for o in held:
                sock = o.parent
                o.hide_render = not (sock and sock.parent and sock.parent.name.split(':', 1)[1] in shown)
            bpy.context.view_layer.update()

        # Heads: the head's own rig parts (re-rooted), rest pose, no base.
        head = t['head']['parts']
        hrows = {p['name']: (p['pivot'], p['rotation'], None) for p in head}
        hshown = trophies.shown_parts(head)
        for label, where, turn, wall in (('floor', 'floor', -22.5 * 8, False), ('floor_r10', 'floor', -22.5 * 10, False),
                                         ('wall', 'wall', 0, True)):
            f = t['head'][where]
            extra = [block_object(scene, wall_mesh, (0, 0, 1), 'preview_wall')] if wall else []
            draw(trophies.place_matrix(turn, f['offset'], f['yaw'], f['scale']), head, hrows, hshown)
            shots['head'].append(shoot(scene, cam, target, 'head_%s_%s' % (mob, label)))
            for ob in extra:
                bpy.data.objects.remove(ob)
        # The diorama: its block and the figure in the trophy clip.
        d = t['diorama']
        base = block_object(scene, meshes[d['block'].split(':', 1)[1]])
        shown = trophies.shown_parts(rj['parts'], (), d['flags'])
        place = trophies.place_matrix(0, d['offset'], d['yaw'], d['scale'])
        fig_at = (d['offset'][0] / 16, -d['offset'][2] / 16, (d['offset'][1] + 5.5) / 16)
        draw(place, rj['parts'], trophies.pose_rows(rj, d['clip'], 0), shown)
        shots['trophy'].append(shoot(scene, cam, target, 'trophy_%s' % mob, DIORAMA_VIEW, DIORAMA_SCALE, DIORAMA_SIZE))
        shots['front'].append(shoot(scene, cam, target, 'trophy_%s_front' % mob, DIORAMA_FRONT, DIORAMA_SCALE, DIORAMA_SIZE))
        shots['far'].append(shoot(scene, cam, target, 'trophy_%s_far' % mob, DIORAMA_VIEW, DIORAMA_SCALE, DIORAMA_FAR))
        shots['figure'].append(shoot(scene, cam, target, 'trophy_%s_figure' % mob, DIORAMA_VIEW, FIGURE_SCALE, DIORAMA_SIZE, fig_at))
        draw(place, rj['parts'], trophies.pose_rows(rj, d['clip'], d['ticks'] // 2), shown)
        shoot(scene, cam, target, 'trophy_%s_t%d' % (mob, d['ticks'] // 2), DIORAMA_VIEW, DIORAMA_SCALE, DIORAMA_SIZE)
        draw(place, rj['parts'], trophies.pose_rows(rj, d['clip'], 0), shown)
        lamp, bg = night(scene, sun, d['light'])
        shots['night'].append(shoot(scene, cam, target, 'trophy_%s_night' % mob, DIORAMA_VIEW, DIORAMA_SCALE, DIORAMA_SIZE))
        bpy.data.objects.remove(lamp)
        bpy.data.objects.remove(base)
    if shots['head']:
        sheet(shots['head'], os.path.join(OUT, 'head_sheet.png'), cols=3, size=TROPHY_SIZE)
    for key in ('trophy', 'front', 'night', 'figure'):
        if shots[key]:
            sheet(shots[key], os.path.join(OUT, ('trophy_sheet.png' if key == 'trophy' else 'trophy_%s_sheet.png' % key)),
                  cols=4, size=DIORAMA_SIZE)
    if shots['far']:
        sheet(shots['far'], os.path.join(OUT, 'trophy_far_sheet.png'), cols=4, size=DIORAMA_FAR)
    print('preview.py: %d head views, %d dioramas in %s' % (len(shots['head']), len(shots['trophy']), OUT))


# The site preview (site.toml preview): mobs around altar level 4, each frozen at a tick of a
# clip: (mob, clip, tick, Blender x, y, z in blocks, turn in degrees about Z). Blender -Y is the
# altar's south, the side the camera looks from and the side a mob faces at turn 0 (a positive
# turn faces it to the camera's right). The altar stays whole in the middle of the frame (round
# 9): the brute raises its maul at the front left corner, the revenant and the stalker come from
# the front right, the marksman aims from the right side and the shrike dives over the back.
SITE_CAST = (
    ('brute', 'attack_slam', 11, -5.6, -8.2, 0, 12),
    ('revenant', 'idle', 0, 4.6, -7.6, 0, -20),
    ('stalker', 'attack_double', 4, 7.4, -4.4, 0, -35),
    ('marksman', 'aim', 10, 7.0, 1.6, 0, -65),
    ('shrike', 'attack_dive', 8, -3.0, 2.8, 4.4, 20),
)
SITE_SIZE = (1280, 720)
SITE_OUT = os.path.join(ROOT, 'site', 'preview.webp')


def freeze(scene, clip, tick):
    """Puts every part of a mob scene into its pose at a tick of a clip and drops the animation,
    so mobs of different clips and ticks share one render frame."""
    poses = {ob.name: basis(ob, clip, tick) for ob in scene.objects}
    for ob in scene.objects:
        if ob.animation_data:
            ob.animation_data_clear()
        ob.matrix_basis = poses[ob.name]


def preview_site():
    """site/preview.webp: altar level 4 (assets/site/altar_level_4.glb) whole in the middle, the
    brute's slam beside it, the revenant, the stalker, the marksman and the shrike around it, in
    the preview light. Run on mobs.blend; nothing is saved back to it."""
    import math
    scene = bpy.data.scenes.new('site_preview')
    bpy.context.window.scene = scene
    cam, target = setup(scene)
    scene.render.resolution_x, scene.render.resolution_y = SITE_SIZE
    scene.eevee.taa_render_samples = 64
    cam.data.type = 'PERSP'
    cam.data.lens = 32
    bpy.ops.import_scene.gltf(filepath=os.path.join(ROOT, 'assets', 'site', 'altar_level_4.glb'))
    ground = bpy.data.meshes.new('site_ground')
    ground.from_pydata([(-200, -200, 0), (200, -200, 0), (200, 200, 0), (-200, 200, 0)], [], [(0, 1, 2, 3)])
    mat = bpy.data.materials.new('site_ground')
    bsdf = next(n for n in mat.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
    bsdf.inputs['Base Color'].default_value = (0.055, 0.06, 0.05, 1)
    bsdf.inputs['Roughness'].default_value = 1.0
    ground.materials.append(mat)
    floor = bpy.data.objects.new('site_ground', ground)
    floor.location.z = -0.002
    scene.collection.objects.link(floor)
    for mob, clip, tick, x, y, z, turn in SITE_CAST:
        src = bpy.data.scenes[mob]
        set_states(src, ())
        freeze(src, clip, tick)
        place = bpy.data.objects.new('site_place_' + mob, None)
        scene.collection.objects.link(place)
        place.location = (x, y, z)
        place.rotation_euler = (0, 0, math.radians(turn))
        for ob in list(src.objects):
            scene.collection.objects.link(ob)
            if ob.parent is None:
                ob.parent = place
                ob.matrix_parent_inverse = Matrix.Identity(4)
    target.location = (0.3, -3.0, 1.2)
    cam.location = (4.0, -20.5, 9.0)
    scene.render.image_settings.file_format = 'WEBP'
    scene.render.image_settings.quality = 90
    os.makedirs(os.path.dirname(SITE_OUT), exist_ok=True)
    scene.render.filepath = SITE_OUT
    bpy.ops.render.render(write_still=True)
    print('preview.py: %s' % SITE_OUT)


def types_ns(mob_id):
    """What rig.obj_name needs of a model: its id."""
    import types
    return types.SimpleNamespace(id=mob_id)


def main():
    arg = sys.argv[sys.argv.index('--') + 1] if '--' in sys.argv else 'revenant'
    if arg == 'blocks':
        return preview_blocks()
    if arg == 'site':
        return preview_site()
    if arg.startswith('trophies'):
        return preview_trophies(arg.split(':', 1)[1].split(',') if ':' in arg else None)
    with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
        every = [m['id'] for m in json.load(f)['mobs']]
    for mob in (every if arg == 'all' else arg.split(',')):
        preview(mob)


main()
