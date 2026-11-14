"""Self-check of the axis conversion between Blender and the game model frame.

    blender --background --factory-startup --python-exit-code 1 --python assets/models/selfcheck.py

Three checks, each compares a point placed by Blender's own world matrices with the same point
placed by the game chain built only from rig.json numbers:

    draw frame = scale(1, -1, -1) translate(0, -1.501, 0)
                 [for each part root to leaf: translate(pivot / 16) rotZ rotY rotX]  point / 16

1. Static: three nested parts turned by quaternions about skewed axes (all three axes each), exported by
   blender_local_to_game + decompose_zyx: no Euler angles are copied.
2. Animated: the same three parts keyed through rig.Clips, sampled and written by rig_json,
   every tick; the middle part also scales unevenly (S), so its children inherit a sheared frame;
   plus a hand socket and an item through the held-item chain.
3. The built revenant in mobs.blend against assets/rigs/revenant.rig.json: the socket_hand_r frame
   and the sword grip, every tick of every clip. The sword is a sprite (item/handheld): its PNG
   must be what the mob file paints, the Blender preview mesh its vanilla extrusion, and the
   leather handle texels must sit on item px (4, 4), where the old element sword had its grip
   and where the handheld display puts the fist.
4. Blocks (block_files_check): the block files on disk keep the engine limits and point at
   models and textures that exist; the z-fighting check works on block elements too.
5. Trophies (trophy_check): the exported trophy.json files, through the game chain from their
   numbers alone: heads are the rig's own parts, the diorama names its block, clip, flags and
   particle types, and its figure stays inside the block at every tick of its loop.
Blender world (X, Y, Z) is the draw frame (X, Z, -Y): +x the mob's left, y up, +z where it looks.
"""
import json
import math
import os
import runpy
import sys

import bpy
from mathutils import Matrix, Quaternion, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)

import items  # noqa: E402
import paint  # noqa: E402
import rig  # noqa: E402

TOL = 1e-4  # blocks; rig.json rounds to 5 decimals
DRAW = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, -1, 0, 0), (0, 0, 0, 1)))  # Blender world -> draw frame
ENTRY = Matrix.Diagonal((1, -1, -1, 1)) @ Matrix.Translation((0, -rig.ORIGIN_Y / 16, 0))
failures = []


def game_part(pivot, angles, scale=None):
    """ModelPart.translateAndRotate: translate(pivot / 16), rotZ rotY rotX, then scale."""
    m = rig.rot_game(*angles).to_4x4()
    m.translation = Vector(pivot) * rig.PX
    if scale:
        m = m @ Matrix.Diagonal(tuple(scale) + (1,))
    return m


def chain(rows):
    """rows: [(pivot px, (x, y, z) radians[, (sx, sy, sz)])] from the root down."""
    m = ENTRY.copy()
    for row in rows:
        m = m @ game_part(*row)
    return m


def track_row(row):
    """A rig.json track row as a chain row: [x, y, z, xRot, yRot, zRot(, xScale, yScale, zScale)]."""
    return (row[:3], row[3:6], row[6:9] or None)


def check(label, a, b):
    d = (a - b).length
    if d > TOL:
        failures.append('%s: %.6f blocks apart (%s vs %s)' % (label, d, tuple(round(c, 5) for c in a), tuple(round(c, 5) for c in b)))
    return d


def blender_point(ob, local_game_px):
    """A point given in a part's local game px, through Blender's world matrix, in the draw frame."""
    local = rig.KT @ (Vector(local_game_px) * rig.PX)
    return (DRAW @ ob.matrix_world @ local.to_4d()).to_3d()


AXES = ((3, 2, 2.5), (-2, 1, 0.7), (0.6, -1, 2))
ANGLES = (70, -124, 140)


def static_check():
    scene = bpy.data.scenes.new('static')
    bpy.context.window.scene = scene
    pivots = [Vector((3, 20, -2)), Vector((-5, 8, 4)), Vector((2, 6, -3))]  # game px, root absolute
    obs = []
    for i, p in enumerate(pivots):
        ob = bpy.data.objects.new('p%d' % i, None)
        scene.collection.objects.link(ob)
        if obs:
            ob.parent = obs[-1]
            ob.matrix_parent_inverse = Matrix.Identity(4)
        ob.rotation_mode = 'QUATERNION'
        # A turn about a skewed axis: not built from Euler angles, yet turns on all three game axes.
        q = Quaternion(Vector(AXES[i]).normalized(), math.radians(ANGLES[i]))
        ob.rotation_quaternion = q
        if i == 0:
            ob.location = rig.KT @ ((p - Vector((0, rig.ORIGIN_Y, 0))) * rig.PX)
        else:
            ob.location = rig.KT @ (p * rig.PX)
        obs.append(ob)
    bpy.context.view_layer.update()
    rows = []
    for i, ob in enumerate(obs):
        r, t = rig.blender_local_to_game(ob.matrix_basis, i == 0)
        angles = rig.decompose_zyx(r)
        print('selfcheck: static part %d game angles (deg) %s' % (i, tuple(round(math.degrees(a), 1) for a in angles)))
        assert all(abs(a) > 0.05 for a in angles), 'each part must turn on all three axes: %s' % (angles,)
        rows.append((t, angles))
    worst = 0
    for pt in ((0, 0, 0), (1.5, -7, 2), (-4, 3, -6), (8, 8, 8)):
        g = (chain(rows) @ (Vector(pt) * rig.PX).to_4d()).to_3d()
        worst = max(worst, check('static %s' % (pt,), blender_point(obs[-1], pt), g))
    return worst


def animated_check():
    scene = bpy.data.scenes.new('animated')
    bpy.context.window.scene = scene
    scene.render.fps = rig.FPS
    m = rig.Model('check', {'m': ['#808080']})
    m.part('a', (3, 20, -2), rot=(25, -40, 60)).cube((-1, -1, -1), (2, 2, 2), 'm')
    m.part('b', (-2, 10, 4), parent='a', rot=(-70, 35, -20)).cube((-1, -1, -1), (2, 2, 2), 'm')
    m.part('arm_r', (1, 4, -3), parent='b', rot=(110, -15, 45)).cube((-3, -2, -2), (4, 12, 4), 'm')
    m.socket('socket_hand_r')
    sword = items.Item('check_sword', {'m': ['#808080']}, items.HANDHELD)
    sword.element((7, 0, 7.5), (9, 14, 8.5), 'm', rotation={'origin': [8, 8, 8], 'axis': 'z', 'angle': -45})
    sword.pack()
    m.hold('socket_hand_r', sword)
    cubes = [c for p in m.parts.values() for c in p.cubes]
    sheet = rig.pack(cubes)
    img = bpy.data.images.new('check', *sheet)
    objs = rig.build_objects(m, scene, sheet, img)
    tex = os.path.join(bpy.app.tempdir, 'check_sword.png')
    sword.paint(1).save(tex)
    held = items.preview_object(sword, objs['socket_hand_r'], 'right', tex, 'check:held')
    clips = rig.Clips()
    c = clips.clip('turn', 10)
    c.key(0, {})
    c.key(4, {'a': rig.R(-50, 70, 15), 'b': rig.R(30, -60, 80) + rig.S(1.4, 0.6, 1.1),
              'arm_r': rig.R(-120, 25, -35) + rig.M(1, -2, 3)}, 'in')
    c.key(10, {'a': rig.R(140, -20, -95), 'b': rig.R(-45, 170, 30) + rig.S(0.7, 1.3, 0.5), 'arm_r': rig.R(60, -80, 120)}, 'back')
    rig.key_clips(m, clips.clips, objs)
    samples = rig.sample_clips(m, clips.clips, objs, scene)
    data = json.loads(json.dumps(rig.rig_json(m, clips.clips, samples, sheet, 'check')))
    parents = {p['name']: p['parent'] for p in data['parts']}
    sock = data['sockets'][0]
    worst = 0
    tracks = data['clips']['turn']['tracks']
    for f in range(11):
        for t in data['clips']['turn']['tracks'].values():
            assert len(t) == 11
        assert len(tracks['b'][f]) == 9 and len(tracks['a'][f]) == 6, 'only the scaled part has scale columns'
        scene.frame_set(f)
        bpy.context.view_layer.update()
        # rig.json chain root -> arm_r
        names, n = [], 'arm_r'
        while n:
            names.insert(0, n)
            n = parents[n]
        rows = [track_row(tracks[n][f]) for n in names]
        g = chain(rows)
        for pt in ((0, 0, 0), (-1, 10, -2), (2, -2, 2)):
            gp = (g @ (Vector(pt) * rig.PX).to_4d()).to_3d()
            worst = max(worst, check('animated tick %d %s' % (f, pt), blender_point(objs['arm_r'], pt), gp))
        # socket and item: the held-item chain from rig.json numbers vs the Blender preview object
        sm = rig.ops_matrix([(a, d) for a, d in sock['rotate']]).to_4x4()
        sm.translation = Vector(sock['offset']) * rig.PX
        gi = g @ sm @ items.held_matrix(sword.display, 'right')
        for pt in ((8, 8, 8), (8, 1, 8), (8, 14, 8)):
            p = rig.Vector(items.rotate_el(pt, sword.elements[0].rotation)) * rig.PX
            gp = (gi @ p.to_4d()).to_3d()
            bp = (DRAW @ held.matrix_world @ p.to_4d()).to_3d()
            worst = max(worst, check('item tick %d %s' % (f, pt), bp, gp))
    return worst


OLD_GRIP = (4, 4)  # item px: the grip of the element sword before it became a sprite
SIDES = ((1, 0), (-1, 0), (0, 1), (0, -1))


def sword_sprite():
    """The revenant's sword sprite from its mob file: checks the shipped PNG is the painted one
    and the handle (leather texels) covers OLD_GRIP. Returns (image, handle texel centres on the
    front face in item space, blocks)."""
    mod = runpy.run_path(os.path.join(HERE, 'mobs', 'revenant.py'))
    sword = next(s for s in mod['SPRITES'] if s.id == 'rusty_sword')
    img = sword.image()
    with open(os.path.join(ROOT, 'assets', 'soulscraft', 'textures', 'item', 'rusty_sword.png'), 'rb') as f:
        if f.read() != img.png():
            failures.append('rusty_sword.png is not what mobs/revenant.py paints: rebuild')
    leather = {paint.rgb(c) for c in mod['PALETTES']['leather']}
    handle = [(x, y) for y in range(img.h) for x in range(img.w) if img.opaque(x, y) and img.get(x, y) in leather]
    gx, gy = OLD_GRIP
    # texel (x, y) covers item px x..x+1, 15-y..16-y (rows run down, item y up)
    if not any(x <= gx <= x + 1 and 15 - y <= gy <= 16 - y for x, y in handle):
        failures.append('sword grip: no handle texel touches item px %s (handle %s)' % (OLD_GRIP, handle))
    if handle:
        cx = sum(x + 0.5 for x, _ in handle) / len(handle)
        cy = sum(15.5 - y for _, y in handle) / len(handle)
        d = math.hypot(cx - gx, cy - gy)
        print('selfcheck: sword handle %d texels, centre (%.2f, %.2f) px, %.2f px from the old grip' % (len(handle), cx, cy, d))
        if d > 1.5:
            failures.append('sword grip: handle centre (%.2f, %.2f) is %.2f px from %s' % (cx, cy, d, OLD_GRIP))
    z = items.SPRITE_DEPTH[1] / 16
    return img, [Vector(((x + 0.5) / 16, (15.5 - y) / 16, z)) for x, y in handle]


def extrusion_check(img, ob, handle):
    """The Blender preview mesh is the vanilla extrusion of the sprite: a front and a back face per
    opaque texel, a side face per texel edge with nothing beyond it, depth 7.5..8.5 px, and a
    front face on every handle texel."""
    want = sum(2 + sum(not img.opaque(x + a, y + b) for a, b in SIDES)
               for y in range(img.h) for x in range(img.w) if img.opaque(x, y))
    me = ob.data
    if len(me.polygons) != want:
        failures.append('sword mesh: %d faces, the extrusion of its sprite has %d' % (len(me.polygons), want))
    zs = {round(v.co.z * 16, 4) for v in me.vertices}
    if zs != set(items.SPRITE_DEPTH):
        failures.append('sword mesh: depth %s px, want %s' % (sorted(zs), items.SPRITE_DEPTH))
    centres = [p.center.copy() for p in me.polygons]
    for h in handle:
        if min((c - h).length for c in centres) > 1e-5:
            failures.append('sword mesh: no front face on the handle texel at %s' % (tuple(round(c * 16, 2) for c in h),))


def revenant_check():
    """The built model: socket_hand_r and the sword grip in mobs.blend vs revenant.rig.json."""
    path = os.path.join(HERE, 'mobs.blend')
    rig_path = os.path.join(ROOT, 'assets', 'rigs', 'revenant.rig.json')
    if not (os.path.exists(path) and os.path.exists(rig_path)):
        print('selfcheck: no built revenant yet, skipped')
        return 0
    with open(rig_path, encoding='utf-8') as f:
        data = json.load(f)
    with open(os.path.join(ROOT, 'assets', 'soulscraft', 'models', 'item', 'rusty_sword.json'), encoding='utf-8') as f:
        sword = json.load(f)
    if sword.get('parent') != items.HANDHELD_PARENT or 'elements' in sword:
        failures.append('rusty_sword.json: want a sprite with parent %s' % items.HANDHELD_PARENT)
    sword_display = sword.get('display') or items.PARENT_DISPLAY[sword['parent']]
    img, handle = sword_sprite()
    bpy.ops.wm.open_mainfile(filepath=path)
    scene = bpy.data.scenes['revenant']
    bpy.context.window.scene = scene
    parts = {p['name']: p for p in data['parts']}
    sock = next(s for s in data['sockets'] if s['name'] == 'socket_hand_r')
    obs = {o.name.split(':', 1)[1]: o for o in scene.objects}
    held = obs['held_rusty_sword']
    extrusion_check(img, held, handle)
    sm = rig.ops_matrix([(a, d) for a, d in sock['rotate']]).to_4x4()
    sm.translation = Vector(sock['offset']) * rig.PX
    worst = 0
    for name, clip in data['clips'].items():
        for o in scene.objects:
            if o.animation_data:
                for tr in o.animation_data.nla_tracks:
                    tr.mute = tr.name != name
        for f in range(clip['ticks'] + 1):
            scene.frame_set(f)
            rows, n = [], sock['parent']
            while n:
                p = parts[n]
                t = clip['tracks'].get(n)
                rows.insert(0, track_row(t[f]) if t else (p['pivot'], p['rotation']))
                n = p['parent']
            g = chain(rows) @ sm
            gb = DRAW @ obs['socket_hand_r'].matrix_world
            for axis in range(3):
                # A point a quarter block along each game axis of the socket; the Empty's own
                # (Blender) local frame holds the same point as K^T times it.
                e = Vector([0.25 if i == axis else 0.0 for i in range(3)])
                eb = (rig.KT @ e).to_4d()
                worst = max(worst, check('revenant %s tick %d socket axis %d' % (name, f, axis), (gb @ eb).to_3d(), (g @ e.to_4d()).to_3d()))
            gi = g @ items.held_matrix(sword_display, 'right')
            bi = DRAW @ held.matrix_world
            for p in handle + [Vector((OLD_GRIP[0] / 16, OLD_GRIP[1] / 16, 0.5))]:
                worst = max(worst, check('revenant %s tick %d grip %s' % (name, f, tuple(round(c * 16, 2) for c in p)),
                                         (bi @ p.to_4d()).to_3d(), (gi @ p.to_4d()).to_3d()))
    return worst


def zfight_check():
    """zfight.py finds a decoration flush with its base and an elbow bending about x, and no longer
    once the top cube is 0.1 px proud; two solid cubes touching face to face are not a fight."""
    import zfight
    found = []
    for lift, want in ((0, 2), (0.1, 0)):
        m = rig.Model('zf', {'m': ['#808080']})
        m.part('root', (0, 24, 0))
        a = m.part('a', (0, 12, 0), parent='root')
        a.cube((-2, -6, -2), (4, 6, 4), 'm')
        a.cube((-2, -6, -2), (4, 2, 2), 'm', inflate=lift)                  # flush top, side, front
        a.cube((-2, 0, -2), (4, 3, 4), 'm')                                  # touches the first one's bottom
        b = m.part('b', (0, 15, 0), parent='a', rot=(30, 0, 0))              # bent about x
        b.cube((-2, 0, -2), (4, 4, 4), 'm', inflate=(lift, 0, 0))
        n = len(zfight.fights(zfight.model_faces(m, zfight.rest_pose(m))))
        found.append(n)
        if n != want:
            failures.append('zfight: %d fights with lift %.1f, want %d' % (n, lift, want))
    print('selfcheck: zfight found %s fights (flush, fixed)' % found)


def block_files_check():
    """The block files on disk as the game reads them: every blockstate variant names a model
    that exists; every model keeps the engine limits (elements -16..32 px, one rotation axis at
    -45..45 in steps of 22.5, uv within the 16 x 16 tile, light_emission 0..15) and names
    textures that exist; every block item parents a block model or is a sprite with its PNG.
    Then the block z-fighting check itself: a band flush with a slab's sides fights, two slabs
    side by side do not, a turned flush element does."""
    import blocks
    ns = os.path.join(ROOT, 'assets', 'soulscraft')
    states_dir = os.path.join(ns, 'blockstates')
    if not os.path.isdir(states_dir):
        print('selfcheck: no blocks built yet, skipped')
        return
    models, n_variants = {}, 0
    for name in sorted(os.listdir(os.path.join(ns, 'models', 'block'))):
        with open(os.path.join(ns, 'models', 'block', name), encoding='utf-8') as f:
            models[name[:-5]] = json.load(f)
    for mid, m in models.items():
        for t in m['textures'].values():
            if not os.path.exists(os.path.join(ns, 'textures', 'block', t.split('/', 1)[1] + '.png')):
                failures.append('block model %s: no texture %s' % (mid, t))
        for i, el in enumerate(m['elements']):
            where = 'block model %s element %d' % (mid, i)
            if any(not -16 <= c <= 32 for c in el['from'] + el['to']):
                failures.append(where + ': outside -16..32 px')
            rot = el.get('rotation')
            if rot and (rot['axis'] not in 'xyz' or rot['angle'] not in (-45, -22.5, 0, 22.5, 45)):
                failures.append(where + ': rotation %s' % rot)
            if not 0 <= el.get('light_emission', 0) <= 15:
                failures.append(where + ': light_emission')
            for face, fj in el['faces'].items():
                if face not in blocks.FACES or any(not 0 <= c <= 16 for c in fj['uv']) or fj['texture'][1:] not in m['textures']:
                    failures.append(where + ': face %s %s' % (face, fj))
    for name in sorted(os.listdir(states_dir)):
        with open(os.path.join(states_dir, name), encoding='utf-8') as f:
            st = json.load(f)
        for key, v in st['variants'].items():
            n_variants += 1
            if v['model'].split('/', 1)[1] not in models:
                failures.append('blockstate %s %s: no model %s' % (name, key, v['model']))
            if v.get('y', 0) not in (0, 90, 180, 270):
                failures.append('blockstate %s %s: y %s' % (name, key, v.get('y')))
        with open(os.path.join(ns, 'models', 'item', name), encoding='utf-8') as f:
            item = json.load(f)
        parent = item.get('parent', '')
        if parent.startswith('soulscraft:block/'):
            if parent.split('/', 1)[1] not in models:
                failures.append('item %s: no block model %s' % (name, parent))
        else:
            # A sprite names its texture as layer0, a model of elements as 0 (a wall head's item
            # shows its head item).
            tex = item.get('textures', {})
            layer = tex.get('layer0') or tex.get('0') or 'soulscraft:item/' + name[:-5]
            if not os.path.exists(os.path.join(ns, 'textures', 'item', layer.split('/', 1)[1] + '.png')):
                failures.append('item %s: no sprite PNG %s' % (name, layer))
    m = blocks.Model('probe')
    m.box((0, 0, 0), (16, 8, 16), 'a')
    m.box((2, 8, 2), (4, 9, 4), 'a')
    m.box((0, 2, 0), (16, 4, 16), 'a')
    slab = blocks.Model('slab')
    slab.box((0, 0, 0), (16, 8, 16), 'a')
    thin = blocks.Model('thin')
    thin.box((0, 7, 0), (16, 8, 16), 'a')
    got = (len(blocks.check_models([m])), len(blocks.check_placed([('slab', (0, 0, 0), 0), ('slab', (1, 0, 0), 0)], {'slab': slab})),
           len(blocks.check_placed([('slab', (0, 0, 0), 0), ('thin', (0, 0, 0), 90)], {'slab': slab, 'thin': thin})))
    if got != (1, 0, 1):
        failures.append('block zfight: found %s, want (1, 0, 1)' % (got,))
    print('selfcheck: blocks: %d models, %d blockstate variants checked; block zfight probe %s' % (len(models), n_variants, got))


def trophy_check():
    """assets/rigs/<mob>.trophy.json as the mod will read it: the head is the rig's own parts
    (same cubes, UVs and pivots below the top part); the diorama names a block that exists, a
    clip of the rig with its length, state flags of the rig, a light level and particle types
    whose files exist, boxes inside the block; and the game chain built from the JSON numbers
    alone keeps every point of the figure inside its block at every tick of the loop and every
    facing, and the head for all 16 rotations on the floor and 4 facings on a wall."""
    import glob
    import trophies
    ns = os.path.join(ROOT, 'assets', 'soulscraft')
    held = trophies.Items(ns)
    lim = 0.01 / 16
    n = 0
    files = sorted(glob.glob(os.path.join(ROOT, 'assets', 'rigs', '*.trophy.json')))
    for path in files:
        with open(path, encoding='utf-8') as f:
            t = json.load(f)
        with open(os.path.join(ROOT, 'assets', 'rigs', t['id'] + '.rig.json'), encoding='utf-8') as f:
            rj = json.load(f)
        by = {p['name']: p for p in rj['parts']}

        def inside(label, pts):
            for p in pts:
                if not (-0.5 - lim <= p.x <= 0.5 + lim and -lim <= p.y <= 1 + lim and -0.5 - lim <= p.z <= 0.5 + lim):
                    failures.append('trophy %s: leaves the block at (%.2f, %.2f, %.2f) px' % (label, p.x * 16, p.y * 16, p.z * 16))
                    return
        for p in t['head']['parts']:
            r = by.get(p['name'])
            if r is None or r['cubes'] != p['cubes'] or (p['parent'] is not None and (r['pivot'], r['rotation'], r['parent']) != (p['pivot'], p['rotation'], p['parent'])):
                failures.append('trophy %s: head part %s is not the rig part' % (t['id'], p['name']))
        head = t['head']['parts']
        rows = {p['name']: (p['pivot'], p['rotation'], None) for p in head}
        for where, turns in (('floor', [-22.5 * r for r in range(16)]), ('wall', [0, -90, -180, -270])):
            fit = t['head'][where]
            for turn in turns:
                m = trophies.place_matrix(turn, fit['offset'], fit['yaw'], fit['scale'])
                inside('%s head %s turn %g' % (t['id'], where, turn), trophies.geometry({'sockets': []}, head, rows, trophies.shown_parts(head), held, 'x', m)[0])
                n += 1
        d = t['diorama']
        bid = d['block'].split(':', 1)[1]
        if not os.path.exists(os.path.join(ns, 'blockstates', bid + '.json')):
            failures.append('trophy %s: no block %s' % (t['id'], bid))
        c = rj['clips'].get(d['clip'])
        if c is None or c['ticks'] != d['ticks'] or bool(c.get('loop')) != d['loop']:
            failures.append('trophy %s: clip %s' % (t['id'], d['clip']))
            continue
        flags = {p['state'].lstrip('!') for p in rj['parts'] if p.get('state')}
        if not set(d['flags']) <= flags or not 0 <= d['light'] <= 15:
            failures.append('trophy %s: flags %s or light %s' % (t['id'], d['flags'], d['light']))
        for e in d['particles']:
            pid = e['type'].split(':', 1)[1]
            if not os.path.exists(os.path.join(ns, 'particles', pid + '.json')) or                     not os.path.exists(os.path.join(ns, 'textures', 'particle', pid + '.png')) or e['every'] < 1 or                     any(not 0 <= v <= 16 for v in e['box']['from'] + e['box']['to']):
                failures.append('trophy %s: particle %s' % (t['id'], e))
        shown = trophies.shown_parts(rj['parts'], (), d['flags'])
        for tick in range(c['ticks']):
            rows = trophies.pose_rows(rj, d['clip'], tick)
            for turn in (0, -90, -180, -270):
                m = trophies.place_matrix(turn, d['offset'], d['yaw'], d['scale'])
                inside('%s diorama tick %d turn %g' % (t['id'], tick, turn), trophies.geometry(rj, rj['parts'], rows, shown, held, 'x', m)[0])
                n += 1
    print('selfcheck: trophies: %d files, %d placements inside their block' % (len(files), n))


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    results = [('static, skewed quaternions', static_check()), ('animated clip + socket + item', animated_check()),
               ('revenant rig.json vs mobs.blend', revenant_check())]
    zfight_check()
    block_files_check()
    trophy_check()
    for label, worst in results:
        print('selfcheck: %-34s worst difference %.2e blocks' % (label, worst))
    if failures:
        print('selfcheck: FAILED')
        for line in failures[:20]:
            print('  ' + line)
        sys.exit(1)
    print('selfcheck: OK')


main()
