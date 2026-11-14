"""Mob rigs for SoulsCraft: parts, cubes, sockets, clips; Blender objects; rig.json for the game.

A mob file (mobs/<id>.py) describes its model in GAME model-space pixels, the way vanilla Java
model code does (HumanoidModel: head cube (-4, -8, -4, 8, 8, 8), right arm pivot (-5, 2, 0)):

    +x  the mob's left side      +y  down      -z  where the mob looks
    origin 24.016 px (1.501 blocks) above the feet, rotations in degrees, applied Z, then Y, then X

This module turns that into Blender objects (1 unit = 1 block, Z up, the mob looks at -Y, feet at
Z = 0) and back. A point converts as  blender = (x, z, 24.016 - y) / 16  ->  game = (16X, 24.016 -
16Z, 16Y). It is a rotation (+90 degrees about X) plus a shift, no mirror.

Rotations are never copied as Euler angles: rig.json is sampled from the evaluated Blender pose
matrices, converted into the game frame and decomposed into Z, Y, X there (decompose_zyx).

A part may also scale in a clip (S(x, y, z), game axes, after the rotation, like ModelPart's
xScale, yScale, zScale; children inherit it) and may be shown only under a mob state flag
(part(..., state='rage'), or '!flag' for "only while the flag is off").
"""
import json
import math

import bpy
from mathutils import Matrix, Vector

PX = 1 / 16
ORIGIN_Y = 24.016  # px from the model origin down to the feet: translate(0, -1.501, 0) in the game
FPS = 20  # one Blender frame is one game tick

# game = K @ blender (both as directions); K is a proper rotation, det = +1.
K = Matrix(((1, 0, 0), (0, 0, -1), (0, 1, 0)))
KT = K.transposed()

FACES = ('front', 'back', 'left', 'right', 'top', 'bottom')

# Default sockets, game px from the parent pivot (05-art-pipeline.md, research 3.4). The rotation
# is a list of draw-stack operations in order: rotX(-90 deg), then rotY(180 deg).
HAND_ROTATE = (('x', -90.0), ('y', 180.0))
SOCKETS = {
    'socket_hand_r': ('arm_r', (-1.0, 10.0, -2.0), HAND_ROTATE),
    'socket_hand_l': ('arm_l', (1.0, 10.0, -2.0), HAND_ROTATE),
}

# Easing of a key: how the curve leaves this key towards the next one (Blender interpolation).
EASE = {
    'linear': ('LINEAR', 'AUTO'),
    'inout': ('SINE', 'EASE_IN_OUT'),   # soft both ends: settles, idles, recoveries
    'slow': ('CUBIC', 'EASE_IN_OUT'),   # heavier soft both ends
    'out': ('QUAD', 'EASE_OUT'),        # fast start, settles: going into an anticipation pose
    'in': ('CUBIC', 'EASE_IN'),         # slow start, fast end: the release into the hit
    'snap': ('EXPO', 'EASE_IN'),        # almost still, then a snap: the release of a held strike
    'back': ('BACK', 'EASE_OUT'),       # overshoots and settles
    'hold': ('CONSTANT', 'AUTO'),       # frozen until the next key
}


# ---------------------------------------------------------------------------- math

def rot_game(x, y, z):
    """Game part rotation (radians) as a 3x3 matrix: Rz @ Ry @ Rx, X applied to a vertex first."""
    return Matrix.Rotation(z, 3, 'Z') @ Matrix.Rotation(y, 3, 'Y') @ Matrix.Rotation(x, 3, 'X')


def decompose_zyx(r):
    """Inverse of rot_game: angles (x, y, z) in radians with r = Rz(z) Ry(y) Rx(x)."""
    sy = max(-1.0, min(1.0, -r[2][0]))
    y = math.asin(sy)
    if abs(sy) < 0.9999999:
        x = math.atan2(r[2][1], r[2][2])
        z = math.atan2(r[1][0], r[0][0])
    else:  # gimbal lock: put everything into z
        x = 0.0
        z = math.atan2(-r[0][1], r[1][1])
    return x, y, z


def continuous_zyx(r, prev):
    """decompose_zyx, but of the two Z-Y-X solutions (and their 2 pi shifts) the one closest to
    the previous tick, so the game can lerp angles between ticks without a jump at +-180 deg."""
    x, y, z = decompose_zyx(r)
    best = None
    for cand in ((x, y, z), (x + math.pi, math.pi - y, z + math.pi)):
        c = [a + 2 * math.pi * round((p - a) / (2 * math.pi)) for a, p in zip(cand, prev)]
        d = sum((a - p) ** 2 for a, p in zip(c, prev))
        if best is None or d < best[0] - 1e-12:
            best = (d, c)
    return tuple(best[1])


def ops_matrix(ops):
    """A 3x3 matrix of draw-stack rotations [('x', deg), ...] applied in order (each multiplies
    the stack on the right, as BlockEntityDraw.rotate_about does)."""
    m = Matrix.Identity(3)
    for axis, deg in ops:
        m = m @ Matrix.Rotation(math.radians(deg), 3, axis.upper())
    return m


def game_to_blender_local(rot3, t_px):
    """A local game transform (rotation, translation in px) as a Blender 4x4 local matrix."""
    m = (KT @ rot3 @ K).to_4x4()
    m.translation = KT @ (Vector(t_px) * PX)
    return m


def blender_local_to_game(m, root):
    """Inverse of game_to_blender_local; a root part's translation also gets the model origin."""
    r = K @ m.to_3x3() @ KT
    t = (K @ m.translation) / PX
    if root:
        t = t + Vector((0.0, ORIGIN_Y, 0.0))
    return r, t


def euler_game_to_blender(x, y, z):
    """Game angles as a Blender 'XZY' Euler: Ry(z) Rz(-y) Rx(x) = K^T Rz(z) Ry(y) Rx(x) K.
    Keyframes go through this so Blender interpolates exactly the game's angle channels."""
    return (x, z, -y)


def scale_game_to_blender(x, y, z):
    """A part scale on game axes as Blender object scale: K^T diag(x, y, z) K = diag(x, z, y).
    Blender scales after the rotation in the object's own frame, as ModelPart does."""
    return (x, z, y)


def num(v, places=5):
    """A float rounded for the JSON files; no '-0'."""
    v = round(v, places)
    return 0 if v == 0 else (int(v) if v == int(v) else v)


# ---------------------------------------------------------------------------- model data

class Cube:
    """An axis-aligned box in its part's frame, game px: addBox(x0, y0, z0, w, h, d) plus
    inflate (CubeDeformation). Sizes are whole pixels: one cube pixel is one texel."""

    def __init__(self, origin, size, mat, inflate=0.0, glow=False, decal=None, grain=1.0, faces=FACES,
                 mirror=False, share=None, coplanar_ok=None):
        for s in size:
            assert s == int(s) and s >= 0, 'cube sizes are whole pixels: %r' % (size,)
        self.origin = tuple(float(c) for c in origin)
        self.size = tuple(int(s) for s in size)
        self.mat, self.glow, self.decal, self.grain = mat, glow, decal, grain
        self.inflate = tuple(float(i) for i in inflate) if isinstance(inflate, (tuple, list)) else (float(inflate),) * 3
        self.faces = tuple(faces)
        # share: another cube of the same size whose sheet region this one reuses (vanilla left
        # limbs reuse the right ones); mirror: CubeListBuilder.mirror(), the texture flips in x.
        if share is not None:
            assert share.size == self.size, 'a shared region needs the same cube size'
        self.share, self.mirror = share, mirror
        # Why this cube's coplanar overlaps cannot be seen; zfight.py skips it. Use only with proof.
        self.coplanar_ok = coplanar_ok
        self.uv = None  # (u, v) on the sheet, set by the packer

    def region(self):
        w, h, d = self.size
        return 2 * (d + w), d + h

    def face_rects(self):
        """Sheet rectangles (u0, v0, u1, v1) of the six faces, vanilla ModelPart.Cube layout.
        For 'bottom' v0 > v1, as in vanilla: its texture is flipped front to back."""
        u, v = self.uv
        w, h, d = self.size
        return {
            'top': (u + d, v, u + d + w, v + d),
            'bottom': (u + d + w, v + d, u + d + 2 * w, v),
            'right': (u, v + d, u + d, v + d + h),
            'front': (u + d, v + d, u + d + w, v + d + h),
            'left': (u + d + w, v + d, u + 2 * d + w, v + d + h),
            'back': (u + 2 * d + w, v + d, u + 2 * d + 2 * w, v + d + h),
        }

    def corners(self):
        """The eight corners in vanilla order v1..v8 (index 0..7), inflated."""
        g = self.inflate
        x0, y0, z0 = (self.origin[i] - g[i] for i in range(3))
        x1, y1, z1 = (self.origin[i] + self.size[i] + g[i] for i in range(3))
        if self.mirror:
            x0, x1 = x1, x0
        return [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
                (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]

    # Vertex order of each face (vanilla polygons) and the outward normal in the game frame.
    POLY = {
        'top': ((5, 4, 0, 1), (0, -1, 0)),
        'bottom': ((2, 3, 7, 6), (0, 1, 0)),
        'right': ((0, 4, 7, 3), (-1, 0, 0)),
        'front': ((1, 0, 3, 2), (0, 0, -1)),
        'left': ((5, 1, 2, 6), (1, 0, 0)),
        'back': ((4, 5, 6, 7), (0, 0, 1)),
    }

    def json(self):
        out = {'origin': [num(c) for c in self.origin], 'size': list(self.size), 'uv': list(self.uv)}
        if any(self.inflate):
            out['inflate'] = [num(c) for c in self.inflate] if len(set(self.inflate)) > 1 else num(self.inflate[0])
        if self.mirror:
            out['mirror'] = True
        if self.glow:
            out['glow'] = True
        if self.faces != FACES:
            out['faces'] = list(self.faces)
        return out


class Part:
    """A ModelPart: pivot, rest rotation, cubes, children."""

    def __init__(self, model, name, pivot, parent, rot, glow, state=None):
        self.model, self.name, self.parent = model, name, parent
        self.pivot = tuple(float(c) for c in pivot)  # absolute, game model px, rest pose
        self.rot = tuple(float(c) for c in rot)  # rest rotation, degrees, game Z-Y-X
        self.glow = glow
        # Shown only while the mob state flag is on ('rage'), or only while it is off ('!lit').
        # A hidden part hides its children too, as ModelPart.visible does.
        self.state = state
        self.cubes = []

    def cube(self, origin, size, mat, **kw):
        """Adds addBox(origin, size) in this part's frame, px. kw: inflate, glow, decal, grain, faces."""
        kw.setdefault('glow', self.glow)
        c = Cube(origin, size, mat, **kw)
        self.cubes.append(c)
        return c

    def world_rest(self):
        """Rest rotation of this part in the model frame (3x3) and pivot (absolute px)."""
        r = rot_game(*(math.radians(a) for a in self.rot))
        if self.parent:
            pr, _ = self.model.parts[self.parent].world_rest()
            r = pr @ r
        return r, Vector(self.pivot)

    def local_rest(self):
        """Pivot relative to the parent pivot in the parent's frame, as ModelPart stores it."""
        if not self.parent:
            return Vector(self.pivot)
        pr, pp = self.model.parts[self.parent].world_rest()
        return pr.transposed() @ (Vector(self.pivot) - pp)


class Socket:
    """An attachment point: an Empty under a part; items, helmets and shields go only here."""

    def __init__(self, name, parent, offset, rotate):
        self.name, self.parent = name, parent
        self.offset = tuple(float(c) for c in offset)
        self.rotate = tuple((a, float(d)) for a, d in rotate)
        self.item = None  # items.Item or items.Sprite held here (preview in Blender, real chain in the showcase)
        self.hand = None  # 'right' or 'left' for display.thirdperson_<hand>hand

    def matrix(self):
        """Socket transform in its parent's game frame: translate(offset/16) then the rotations."""
        m = ops_matrix(self.rotate).to_4x4()
        m.translation = Vector(self.offset) * PX
        return m


class Model:
    """What a mob file's model(m) fills in."""

    def __init__(self, mob_id, palettes, texture=None):
        self.id = mob_id
        self.palettes = palettes
        self.texture = texture  # (w, h) or None: 64x64, or 128x128 if the cubes do not fit
        self.parts = {}
        self.sockets = {}

    def part(self, name, pivot=(0, 0, 0), parent=None, rot=(0, 0, 0), glow=False, state=None):
        """A part with its pivot in absolute game model px (rest pose), like PartPose.offset of a
        root part. rot is the rest rotation in degrees; a tilted cube is a part with rot. state:
        a mob state flag the part is shown under ('rage'), or '!flag' to show it only without."""
        assert name not in self.parts, 'part %s twice' % name
        assert name == name.lower() and name.replace('_', '').isalnum(), 'part names are lower-case latin: %s' % name
        assert parent is None or parent in self.parts, 'parent %s of %s must come first' % (parent, name)
        if state is not None:
            flag = state[1:] if state.startswith('!') else state
            assert flag and flag == flag.lower() and flag.replace('_', '').isalnum(), 'state flags are lower-case latin: %s' % state
        p = Part(self, name, pivot, parent, rot, glow, state)
        self.parts[name] = p
        return p

    def socket(self, name, parent=None, offset=None, rotate=None):
        """A socket. socket_hand_r / socket_hand_l default to the vanilla hand point (-1|1, 10, -2)
        under arm_r / arm_l with rotX(-90) rotY(180); other sockets need parent and offset."""
        default = SOCKETS.get(name)
        parent = parent or (default and default[0])
        offset = offset if offset is not None else (default and default[1])
        rotate = rotate if rotate is not None else (default[2] if default else ())
        assert parent in self.parts, 'socket %s needs a parent part' % name
        s = Socket(name, parent, offset, rotate)
        self.sockets[name] = s
        return s

    def hold(self, socket, item):
        """Puts an item model (items.Item or items.Sprite) into a hand socket."""
        s = self.sockets[socket]
        s.item = item
        s.hand = 'left' if socket.endswith('_l') else 'right'


# ---------------------------------------------------------------------------- poses and clips

class J:
    """A joint pose: rotation (deg, game X, Y, Z), move (px, game frame, parent space) and scale
    (game X, Y, Z of the part, after its rotation). R(x, y, z), M(x, y, z) and S(x, y, z) make
    one; + adds rotations and moves and multiplies scales."""

    def __init__(self, rot=(0, 0, 0), move=(0, 0, 0), scale=(1, 1, 1)):
        self.rot = tuple(float(c) for c in rot)
        self.move = tuple(float(c) for c in move)
        self.scale = tuple(float(c) for c in scale)

    def __add__(self, other):
        return J([a + b for a, b in zip(self.rot, other.rot)], [a + b for a, b in zip(self.move, other.move)],
                 [a * b for a, b in zip(self.scale, other.scale)])

    def scaled(self, f):
        """f of the way from no change to this pose (a scale goes linearly from 1)."""
        return J([a * f for a in self.rot], [a * f for a in self.move], [1 + (a - 1) * f for a in self.scale])

    def same(self, other):
        return self.rot == other.rot and self.move == other.move and self.scale == other.scale


def R(x=0, y=0, z=0):
    return J(rot=(x, y, z))


def M(x=0, y=0, z=0):
    return J(move=(x, y, z))


def S(x=1, y=None, z=None):
    """Scale of a part on game axes: S(0) hides it, S(1.2) is uniform, S(1, 0.5, 1) squashes y."""
    return J(scale=(x, x if y is None else y, x if z is None else z))


def pose(*poses, **parts):
    """Merges pose dicts {part: J}; the same part in two poses adds up. Keyword parts too."""
    out = {}
    for p in poses + (parts,):
        for k, j in p.items():
            out[k] = out[k] + j if k in out else j
    return out


def lerp(a, b, f):
    """Pose a moved by f of the way to pose b; every channel, scale too, goes linearly."""
    def mix(u, v):
        return [x + (y - x) * f for x, y in zip(u, v)]
    out = {}
    for k in set(a) | set(b):
        ja, jb = a.get(k, J()), b.get(k, J())
        out[k] = J(mix(ja.rot, jb.rot), mix(ja.move, jb.move), mix(ja.scale, jb.scale))
    return out


class Clip:
    """One animation: keys at ticks, each a full pose (parts left out are at rest)."""

    def __init__(self, name, length, loop=False, markers=None):
        assert length > 0
        self.name, self.length, self.loop = name, int(length), loop
        self.markers = dict(markers or {})
        self.keys = []  # (frame, pose, ease)

    def key(self, frame, p=None, ease='inout'):
        assert ease in EASE, 'unknown ease %s' % ease
        assert 0 <= frame <= self.length, '%s: key %s outside 0..%d' % (self.name, frame, self.length)
        self.keys = [k for k in self.keys if k[0] != frame]
        self.keys.append((int(frame), p or {}, ease))
        self.keys.sort(key=lambda k: k[0])
        return self

    def at(self, marker):
        return self.markers[marker]

    def check(self):
        frames = [k[0] for k in self.keys]
        assert frames and frames[0] == 0 and frames[-1] == self.length, \
            '%s: keys must start at 0 and end at %d, got %s' % (self.name, self.length, frames)
        for m, f in self.markers.items():
            assert f in frames, '%s: no key on marker %s (frame %d)' % (self.name, m, f)
        if self.loop:
            first, last = self.keys[0][1], self.keys[-1][1]
            for k in set(first) | set(last):
                a, b = first.get(k, J()), last.get(k, J())
                assert a.same(b), '%s: loop must end on its first pose (%s)' % (self.name, k)


class Clips:
    """What a mob file's clips(c, mob) fills in."""

    def __init__(self):
        self.clips = {}

    def clip(self, name, length, loop=False, markers=None):
        assert name not in self.clips, 'clip %s twice' % name
        c = Clip(name, length, loop, markers)
        self.clips[name] = c
        return c

    def attack(self, name, attack):
        """A clip for an attack of data/mobs.json. The markers follow the attack's ticks; the
        build still compares them (and the key on every marker) with mobs.json afterwards."""
        return self.clip(name, attack_end(attack), markers=attack_markers(attack))


def attack_markers(a):
    """Marker frames of an attack: windup at 0, hold (delayed only), hit, recover, end."""
    w, h, hit, rec = a['windup'], a.get('hold', 0), a['hit'], a['recover']
    out = {'windup': 0, 'hit': w + h, 'recover': w + h + hit, 'end': w + h + hit + rec}
    if h:
        out['hold'] = w
    return out


def attack_end(a):
    return a['windup'] + a.get('hold', 0) + a['hit'] + a['recover']


# ---------------------------------------------------------------------------- texture sheet

def fit(regions, w, h):
    """First-fit packing of (rw, rh) regions on a w x h grid, largest first, scanning rows top to
    bottom and left to right. Deterministic. Returns [(x, y)] in input order, or None."""
    used = [[False] * w for _ in range(h)]
    order = sorted(range(len(regions)), key=lambda i: (-regions[i][1], -regions[i][0], i))
    out = [None] * len(regions)
    for i in order:
        rw, rh = regions[i]
        spot = None
        for y in range(h - rh + 1):
            for x in range(w - rw + 1):
                if not any(used[y + j][x + k] for j in range(rh) for k in range(rw)):
                    spot = (x, y)
                    break
            if spot:
                break
        if spot is None:
            return None
        x, y = spot
        for j in range(rh):
            for k in range(rw):
                used[y + j][x + k] = True
        out[i] = spot
    return out


def pack(cubes, size=None):
    """Packs the box-UV regions of the cubes onto the sheet (fit); returns the sheet size, the
    smallest of 64x64, 128x64, 128x128 that holds them unless size is given. A cube with share=
    takes the region of the cube it shares."""
    own = [c for c in cubes if c.share is None]
    for w, h in ([size] if size else [(64, 64), (128, 64), (128, 128)]):
        spots = fit([c.region() for c in own], w, h)
        if spots:
            for c, uv in zip(own, spots):
                c.uv = uv
            for c in cubes:
                if c.share is not None:
                    c.uv = c.share.uv
            return w, h
    raise AssertionError('cubes do not fit the texture sheet %r' % (size,))


def paint_sheet(model, cubes, sheet, seed):
    import paint
    img = paint.Image(*sheet)
    for c in cubes:
        if c.share is not None:
            continue  # painted with the cube it shares
        rects = c.face_rects()
        for name in FACES:
            u0, v0, u1, v1 = rects[name]
            face = paint.Face(img, name, min(u0, u1), min(v0, v1), abs(u1 - u0), abs(v1 - v0), model.palettes, c)
            paint.fill(img, face, model.palettes, c.mat, seed, c.grain)
            if c.decal:
                c.decal(face)
    return img


# ---------------------------------------------------------------------------- Blender objects

def cube_mesh_data(cube, sheet):
    """Vertices (Blender local, blocks), quads and per-corner UVs of one cube."""
    tw, th = sheet
    corners = cube.corners()
    rects = cube.face_rects()
    verts, quads, uvs = [], [], []
    for name in cube.faces:
        order, normal = Cube.POLY[name]
        u0, v0, u1, v1 = rects[name]
        corner_uv = [(u1, v0), (u0, v0), (u0, v1), (u1, v1)]  # vanilla Polygon remap order
        pts = [KT @ (Vector(corners[i]) * PX) for i in order]
        uv = [(u / tw, 1 - v / th) for u, v in corner_uv]
        # Keep the face counter-clockwise from outside in Blender, whatever the vanilla order.
        if cube.mirror:  # x0 and x1 swapped: the right face sits on +x and the left one on -x
            normal = (-normal[0], normal[1], normal[2])
        n = (pts[1] - pts[0]).cross(pts[2] - pts[0])
        if n.dot(KT @ Vector(normal)) < 0:
            pts, uv = pts[::-1], uv[::-1]
        base = len(verts)
        verts.extend(pts)
        uvs.append(uv)
        quads.append((base, base + 1, base + 2, base + 3))
    return verts, quads, uvs


def make_mesh(name, cubes_with_mat, sheet, materials):
    """One Blender mesh from cubes; cubes_with_mat = [(cube, material index)]."""
    verts, quads, uvs, mats = [], [], [], []
    for cube, mi in cubes_with_mat:
        v, q, u = cube_mesh_data(cube, sheet)
        base = len(verts)
        verts.extend(v)
        quads.extend(tuple(i + base for i in f) for f in q)
        uvs.extend(u)
        mats.extend([mi] * len(q))
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(v) for v in verts], [], quads)
    layer = me.uv_layers.new(name='UVMap')
    for poly, uv in zip(me.polygons, uvs):
        poly.use_smooth = False
        poly.material_index = mats[poly.index]
        for li, corner in zip(poly.loop_indices, uv):
            layer.data[li].uv = corner
    for m in materials:
        me.materials.append(m)
    me.validate()
    return me


def texture_material(name, image, glow=False):
    """Image texture with Closest filtering into a rough Principled BSDF; glow also drives the
    emission with the same texture (the glTF export writes it as the emissive map)."""
    mat = bpy.data.materials.new(name)
    nt = mat.node_tree
    bsdf = next(n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED')
    tex = nt.nodes.new('ShaderNodeTexImage')
    tex.image = image
    tex.interpolation = 'Closest'
    nt.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
    bsdf.inputs['Roughness'].default_value = 1.0
    bsdf.inputs['Metallic'].default_value = 0.0
    if 'Specular IOR Level' in bsdf.inputs:
        bsdf.inputs['Specular IOR Level'].default_value = 0.0
    if glow:
        nt.links.new(tex.outputs['Color'], bsdf.inputs['Emission Color'])
        bsdf.inputs['Emission Strength'].default_value = 1.0
    return mat


def obj_name(model, name):
    """Blender names are unique across the whole .blend; parts are '<mob>:<part>'."""
    return '%s:%s' % (model.id, name)


def build_objects(model, scene, sheet, image):
    """Part objects (origin = pivot, parented as the ModelPart tree) and socket Empties."""
    mat = texture_material(model.id, image)
    glow = texture_material(model.id + '_glow', image, glow=True)
    objs = {}
    for part in model.parts.values():
        me = None
        if part.cubes:  # a part without cubes (root) is an Empty
            me = make_mesh(obj_name(model, part.name), [(c, 1 if c.glow else 0) for c in part.cubes], sheet, [mat, glow])
        ob = bpy.data.objects.new(obj_name(model, part.name), me)
        scene.collection.objects.link(ob)
        if part.parent:
            ob.parent = objs[part.parent]
            ob.matrix_parent_inverse = Matrix.Identity(4)
        rest = rest_matrix(part)
        ob.rotation_mode = 'XZY'
        ob.location = rest.translation
        ob.rotation_euler = euler_game_to_blender(*(math.radians(a) for a in part.rot))
        # The rest pose, for tools that need it after clips moved the part (preview.py).
        ob["soulscraft_rest"] = list(ob.location) + list(ob.rotation_euler)
        if part.state:  # shown only under a mob state flag; preview.py hides it unless the flag is on
            ob["soulscraft_state"] = part.state
        objs[part.name] = ob
    for s in model.sockets.values():
        em = bpy.data.objects.new(obj_name(model, s.name), None)
        em.empty_display_type = 'ARROWS'
        em.empty_display_size = 0.12
        scene.collection.objects.link(em)
        em.parent = objs[s.parent]
        em.matrix_parent_inverse = Matrix.Identity(4)
        g = s.matrix()
        em.rotation_mode = 'QUATERNION'
        em.matrix_basis = game_to_blender_local(g.to_3x3(), Vector(s.offset))
        objs[s.name] = em
    return objs


def rest_matrix(part):
    """Blender local matrix of a part in its rest pose."""
    local = part.local_rest()
    r = rot_game(*(math.radians(a) for a in part.rot))
    m = game_to_blender_local(r, local)
    if not part.parent:
        m.translation = KT @ ((local - Vector((0, ORIGIN_Y, 0))) * PX)
    return m


def key_clips(model, clips, objs):
    """One slotted Action per clip (a slot per part), on an NLA track named as the clip, so the
    glTF export (NLA_TRACKS) writes one animation per clip. Every part is keyed in every clip."""
    for clip in clips.values():
        clip.check()
        act = bpy.data.actions.new(obj_name(model, clip.name))
        act.use_fake_user = True
        act.use_frame_range = True
        act.frame_range = (0, clip.length)
        act.use_cyclic = clip.loop
        for mname, frame in sorted(clip.markers.items(), key=lambda kv: (kv[1], kv[0])):
            act.pose_markers.new(mname).frame = frame
        strip = act.layers.new('keys').strips.new(type='KEYFRAME')
        for part in model.parts.values():
            ob = objs[part.name]
            slot = act.slots.new(id_type='OBJECT', name=ob.name)
            bag = strip.channelbag(slot, ensure=True)
            rest = rest_matrix(part).translation
            values = []  # per key: {Blender channel: xyz}
            for frame, p, ease in clip.keys:
                j = p.get(part.name, J())
                rx, ry, rz = (math.radians(a + b) for a, b in zip(part.rot, j.rot))
                loc = rest + KT @ (Vector(j.move) * PX)
                values.append((frame, ease, {'location': tuple(loc), 'rotation_euler': euler_game_to_blender(rx, ry, rz),
                                             'scale': scale_game_to_blender(*j.scale)}))
            for path in ('location', 'rotation_euler', 'scale'):
                for i in range(3):
                    fc = bag.fcurves.new(path, index=i)
                    fc.keyframe_points.add(len(values))
                    for kp, (frame, ease, ch) in zip(fc.keyframe_points, values):
                        kp.co = (frame, ch[path][i])
                        kp.interpolation, kp.easing = EASE[ease]
                        kp.handle_left_type = kp.handle_right_type = 'AUTO_CLAMPED'
                    fc.update()
            ad = ob.animation_data_create()
            track = ad.nla_tracks.new()
            track.name = clip.name
            st = track.strips.new(clip.name, 0, act)
            st.action_slot = slot
            st.extrapolation = 'HOLD'


# ---------------------------------------------------------------------------- rig.json

def sample_clips(model, clips, objs, scene):
    """Evaluates every clip in Blender tick by tick and converts each part's local pose into the
    game frame: pivot px (x, y, z), Z-Y-X angles in radians and the scale on game axes. The
    rotation comes from the evaluated location and Euler channels, not from matrix_basis, so a
    part scaled to 0 still has one."""
    out = {}
    bpy.context.window.scene = scene  # frame_set evaluates the depsgraph of the active scene
    for clip in clips.values():
        for ob in (objs[p] for p in model.parts):
            for tr in ob.animation_data.nla_tracks:
                tr.mute = tr.name != clip.name
        tracks = {p: [] for p in model.parts}
        prev = {p: [math.radians(a) for a in part.rot] for p, part in model.parts.items()}
        for f in range(clip.length + 1):
            scene.frame_set(f)
            for name, part in model.parts.items():
                ob = objs[name]
                m = Matrix.LocRotScale(ob.location, ob.rotation_euler, None)
                r, t = blender_local_to_game(m, not part.parent)
                prev[name] = continuous_zyx(r, prev[name])
                bx, by, bz = ob.scale
                tracks[name].append(list(t) + list(prev[name]) + [bx, bz, by])
        out[clip.name] = tracks
    for ob in (objs[p] for p in model.parts):
        for tr in ob.animation_data.nla_tracks:
            tr.is_solo = False
    scene.frame_set(0)
    return out


def rig_json(model, clips, samples, sheet, texture_ref):
    parts = []
    for part in model.parts.values():
        rest_r = [math.radians(a) for a in part.rot]
        entry = {
            'name': part.name,
            'parent': part.parent,
            'pivot': [num(c) for c in part.local_rest()],
            'rotation': [num(a) for a in rest_r],
            'cubes': [c.json() for c in part.cubes],
        }
        if part.state:
            entry['state'] = part.state
        parts.append(entry)
    sockets = []
    for s in model.sockets.values():
        e = {'name': s.name, 'parent': s.parent, 'offset': [num(c) for c in s.offset],
             'rotate': [[a, num(d)] for a, d in s.rotate]}
        if s.item:
            e['item'] = 'soulscraft:' + s.item.id
            e['display'] = 'thirdperson_%shand' % s.hand
        sockets.append(e)
    out_clips = {}
    for name, clip in clips.items():
        tracks = {}
        for pname, frames in samples[name].items():
            part = model.parts[pname]
            rest = list(part.local_rest()) + [math.radians(a) for a in part.rot] + [1.0, 1.0, 1.0]
            if all(all(abs(a - b) < 1e-5 for a, b in zip(fr, rest)) for fr in frames):
                continue  # at rest for the whole clip
            # Rows are [x, y, z, xRot, yRot, zRot], and [xScale, yScale, zScale] after that on
            # every row when the part scales at any tick of the clip.
            scales = any(abs(v - 1) >= 1e-5 for fr in frames for v in fr[6:])
            tracks[pname] = [[num(v) for v in (fr if scales else fr[:6])] for fr in frames]
        out_clips[name] = {'ticks': clip.length, 'loop': clip.loop, 'markers': dict(sorted(clip.markers.items(), key=lambda kv: (kv[1], kv[0]))), 'tracks': tracks}
    return {
        'format': 'soulscraft-rig',
        'version': 2,
        'id': model.id,
        'frame': 'game model space: px, +x mob left, +y down, -z front; origin 24.016 px above the feet; part rotation Z then Y then X, radians; part scale after the rotation',
        'texture': {'sprite': texture_ref, 'width': sheet[0], 'height': sheet[1]},
        'parts': parts,
        'sockets': sockets,
        'clips': out_clips,
    }


def dumps(obj, indent=0):
    """JSON with one line per number array: compact frames, readable structure, stable bytes."""
    pad = ' ' * indent
    if isinstance(obj, dict):
        if not obj:
            return '{}'
        items = ['%s %s: %s' % (pad, json.dumps(k, ensure_ascii=False), dumps(v, indent + 1)) for k, v in obj.items()]
        return '{\n' + ',\n'.join(items) + '\n' + pad + '}'
    if isinstance(obj, list):
        if all(isinstance(v, (int, float, str, bool)) or v is None for v in obj):
            return '[' + ', '.join(json.dumps(v, ensure_ascii=False) for v in obj) + ']'
        return '[\n' + ',\n'.join(pad + ' ' + dumps(v, indent + 1) for v in obj) + '\n' + pad + ']'
    return json.dumps(obj, ensure_ascii=False)
