"""Item models and decorative armour for SoulsCraft: Minecraft item JSON, textures, previews.

Two kinds of item model:

- Sprite: a 16 x 16 texture with parent minecraft:item/generated (an icon) or item/handheld (a
  sword, an axe). The game extrudes it itself (ItemModelGenerator): the whole sprite as a front
  and a back quad, plus a side face on every texel edge that borders a transparent texel, depth
  7.5..8.5 px. Every vanilla sword and tool is built like this, and SoulsCraft handheld weapons
  are too, so they read as blades in the fist and not as sticks.
- Item: elements (block-model frame: 0..16 px, +y up, north = -z) with face UVs packed onto its
  own texture, and a display block: helmets, big weapons that are not flat.

Weapons that mobs hold use the vanilla handheld display, so they sit in a mob's hand, a player's
hand and on a stand alike. held_matrix() is the game chain from a hand socket to the item
vertices (research 3.4, model-references 3.1): translate(t / 16), rotate X Y Z, scale,
translate(-0.5). Blender places its preview of the item through it, and the showcase repeats the
same numbers and the same extrusion in three.js.
"""
import math

import paint

PX = 1 / 16

# Vanilla display blocks (item/handheld and item/generated, model-references 1.5).
HANDHELD = {
    'thirdperson_righthand': {'rotation': [0, -90, 55], 'translation': [0, 4, 0.5], 'scale': [0.85, 0.85, 0.85]},
    'thirdperson_lefthand': {'rotation': [0, 90, -55], 'translation': [0, 4, 0.5], 'scale': [0.85, 0.85, 0.85]},
    'firstperson_righthand': {'rotation': [0, -90, 25], 'translation': [1.13, 3.2, 1.13], 'scale': [0.68, 0.68, 0.68]},
    'firstperson_lefthand': {'rotation': [0, 90, -25], 'translation': [1.13, 3.2, 1.13], 'scale': [0.68, 0.68, 0.68]},
    'ground': {'rotation': [0, 0, 0], 'translation': [0, 2, 0], 'scale': [0.5, 0.5, 0.5]},
    'gui': {'rotation': [0, 0, 0], 'translation': [0, 0, 0], 'scale': [1, 1, 1]},
    'fixed': {'rotation': [0, 180, 0], 'translation': [0, 0, 0], 'scale': [1, 1, 1]},
}
# item/generated alone (icons); a model without thirdperson_lefthand uses the right hand's.
_GEN_HAND = {'rotation': [0, 0, 0], 'translation': [0, 3, 1], 'scale': [0.55, 0.55, 0.55]}
_GEN_FIRST = {'rotation': [0, -90, 25], 'translation': [1.13, 3.2, 1.13], 'scale': [0.68, 0.68, 0.68]}
GENERATED = {
    'ground': {'rotation': [0, 0, 0], 'translation': [0, 2, 0], 'scale': [0.5, 0.5, 0.5]},
    'head': {'rotation': [0, 180, 0], 'translation': [0, 13, 7], 'scale': [1, 1, 1]},
    'thirdperson_righthand': _GEN_HAND, 'thirdperson_lefthand': _GEN_HAND,
    'firstperson_righthand': _GEN_FIRST, 'firstperson_lefthand': _GEN_FIRST,
    'fixed': {'rotation': [0, 180, 0], 'translation': [0, 0, 0], 'scale': [1, 1, 1]},
}
GENERATED_PARENT = 'minecraft:item/generated'
HANDHELD_PARENT = 'minecraft:item/handheld'
# What a sprite inherits from its parent; embed.py hands the same block to the showcase.
PARENT_DISPLAY = {GENERATED_PARENT: GENERATED, HANDHELD_PARENT: HANDHELD}
SPRITE_DEPTH = (7.5, 8.5)  # px, ItemModelGenerator
# A helmet of any shape is drawn as an item model on the head (research 4.2): CustomHeadLayer
# maps item px p to head px (8 - p) * 0.625 around the head centre, so display.head stays identity.
HELMET = {
    'head': {'rotation': [0, 0, 0], 'translation': [0, 0, 0], 'scale': [1, 1, 1]},
    'gui': {'rotation': [30, 225, 0], 'translation': [0, 0, 0], 'scale': [0.625, 0.625, 0.625]},
    'ground': {'rotation': [0, 0, 0], 'translation': [0, 3, 0], 'scale': [0.25, 0.25, 0.25]},
    'fixed': {'rotation': [0, 180, 0], 'translation': [0, 0, 0], 'scale': [0.5, 0.5, 0.5]},
    'thirdperson_righthand': {'rotation': [75, 45, 0], 'translation': [0, 2.5, 0], 'scale': [0.375, 0.375, 0.375]},
    'thirdperson_lefthand': {'rotation': [75, 45, 0], 'translation': [0, 2.5, 0], 'scale': [0.375, 0.375, 0.375]},
}
HEAD_SCALE = 0.625  # CustomHeadLayer

# Face directions of an element and how its UV runs (vanilla FaceBakery): (u axis, u sign, v axis,
# v sign); u0 sits at the low end of the axis for sign +1, at the high end for -1. The mob terms
# match a helmet built from head cubes (item +x is the mob's right, item north is the mob's front).
EL_FACES = {
    'north': ((0, -1), (1, -1), 'front'),
    'south': ((0, 1), (1, -1), 'back'),
    'west': ((2, 1), (1, -1), 'left'),
    'east': ((2, -1), (1, -1), 'right'),
    'up': ((0, 1), (2, 1), 'top'),
    'down': ((0, 1), (2, -1), 'bottom'),
}
FACE_NORMAL = {'north': (0, 0, -1), 'south': (0, 0, 1), 'west': (-1, 0, 0), 'east': (1, 0, 0), 'up': (0, 1, 0), 'down': (0, -1, 0)}


class Element:
    def __init__(self, frm, to, mat, texels=None, rotation=None, decal=None, grain=1.0, faces=None):
        self.frm, self.to = [float(c) for c in frm], [float(c) for c in to]
        size = [b - a for a, b in zip(self.frm, self.to)]
        # Texel size of the box on the sheet (w along x, h along y, d along z).
        self.texels = tuple(int(t) for t in (texels or [max(1, round(s)) for s in size]))
        self.mat, self.rotation, self.decal, self.grain = mat, rotation, decal, grain
        self.faces = tuple(faces or EL_FACES)
        self.uv = None

    def region(self):
        w, h, d = self.texels
        return 2 * (d + w), d + h

    def rects(self):
        """Texel rectangles of the faces, laid out like box UV: up/down above, sides below."""
        u, v = self.uv
        w, h, d = self.texels
        return {
            'up': (u + d, v, w, d), 'down': (u + d + w, v, w, d),
            'east': (u, v + d, d, h), 'north': (u + d, v + d, w, h),
            'west': (u + d + w, v + d, d, h), 'south': (u + 2 * d + w, v + d, w, h),
        }


class Item:
    """An item model: soulscraft:<id>, texture soulscraft:item/<id>."""

    def __init__(self, item_id, palettes, display, texture=16, name=None):
        self.id, self.palettes, self.display, self.texture = item_id, palettes, display, texture
        self.name = name or item_id
        self.elements = []

    def element(self, frm, to, mat, **kw):
        e = Element(frm, to, mat, **kw)
        self.elements.append(e)
        return e

    def from_head_cubes(self, cubes, centre=(0, -4, 0)):
        """Elements from head-local mob cubes (rig.Cube), so a dropped helmet looks like the one
        the mob wears: head px h -> item px 8 -/+ (h - centre) / 0.625 (see HELMET)."""
        k = 1 / HEAD_SCALE
        for c in cubes:
            lo = [c.origin[i] - c.inflate[i] for i in range(3)]
            hi = [c.origin[i] + c.size[i] + c.inflate[i] for i in range(3)]
            frm = [8 - (hi[0] - centre[0]) * k, 8 - (hi[1] - centre[1]) * k, 8 + (lo[2] - centre[2]) * k]
            to = [8 - (lo[0] - centre[0]) * k, 8 - (lo[1] - centre[1]) * k, 8 + (hi[2] - centre[2]) * k]
            self.element(frm, to, c.mat, texels=c.size, decal=c.decal, grain=c.grain)

    def pack(self):
        from rig import fit
        spots = fit([e.region() for e in self.elements], self.texture, self.texture)
        assert spots, 'item %s does not fit %dx%d' % (self.id, self.texture, self.texture)
        for e, uv in zip(self.elements, spots):
            e.uv = uv

    def paint(self, seed):
        img = paint.Image(self.texture, self.texture)
        for e in self.elements:
            for face, (u, v, w, h) in e.rects().items():
                f = paint.Face(img, EL_FACES[face][2], u, v, w, h, self.palettes, e)
                paint.fill(img, f, self.palettes, e.mat, seed, e.grain)
                if e.decal:
                    e.decal(f)
        return img

    def face_uv(self, e, face):
        """JSON uv [u0, v0, u1, v1] in 0..16 units of the texture."""
        u, v, w, h = e.rects()[face]
        s = 16 / self.texture
        return [rnd(u * s), rnd(v * s), rnd((u + w) * s), rnd((v + h) * s)]

    def json(self):
        tex = 'soulscraft:item/' + self.id
        elements = []
        for e in self.elements:
            el = {'from': [rnd(c) for c in e.frm], 'to': [rnd(c) for c in e.to]}
            if e.rotation:
                el['rotation'] = e.rotation
            el['faces'] = {f: {'uv': self.face_uv(e, f), 'texture': '#0'} for f in EL_FACES if f in e.faces}
            elements.append(el)
        return {
            'credit': 'SoulsCraft; generated by assets/models/build.py, do not edit by hand',
            'textures': {'0': tex, 'particle': tex},
            'elements': elements,
            'display': self.display,
        }

    def quads(self):
        """Quads in item space (px, 0..16) with per-corner texel UV, element rotation applied:
        [(corners [(x, y, z)] * 4, uvs [(u, v)] * 4, normal)]. The showcase builds the same mesh
        from the JSON; this one is for the Blender preview."""
        out = []
        s = self.texture
        for e in self.elements:
            lo, hi = e.frm, e.to
            for face in e.faces:
                (ua, us), (va, vs), _ = EL_FACES[face]
                n = FACE_NORMAL[face]
                axis = [i for i in range(3) if n[i]][0]
                fixed = hi[axis] if n[axis] > 0 else lo[axis]
                u0, v0, u1, v1 = [c * s / 16 for c in self.face_uv(e, face)]
                corners, uvs = [], []
                for fu, fv in ((0, 0), (1, 0), (1, 1), (0, 1)):
                    p = [0.0, 0.0, 0.0]
                    p[axis] = fixed
                    p[ua] = lo[ua] + (hi[ua] - lo[ua]) * (fu if us > 0 else 1 - fu)
                    p[va] = lo[va] + (hi[va] - lo[va]) * (fv if vs > 0 else 1 - fv)
                    corners.append(rotate_el(p, e.rotation))
                    uvs.append((u0 + (u1 - u0) * fu, v0 + (v1 - v0) * fv))
                out.append((corners, uvs, rotate_dir(n, e.rotation)))
        return out


def rotate_el(p, rot):
    """Element rotation: angle in degrees about axis through origin (right hand rule)."""
    if not rot:
        return tuple(p)
    o = rot['origin']
    a = math.radians(rot['angle'])
    i = 'xyz'.index(rot['axis'])
    j, k = (i + 1) % 3, (i + 2) % 3
    q = [p[n] - o[n] for n in range(3)]
    r = list(q)
    r[j] = q[j] * math.cos(a) - q[k] * math.sin(a)
    r[k] = q[j] * math.sin(a) + q[k] * math.cos(a)
    return tuple(r[n] + o[n] for n in range(3))


def rotate_dir(n, rot):
    if not rot:
        return n
    z = rotate_el((0, 0, 0), dict(rot, origin=[0, 0, 0]))
    p = rotate_el(n, dict(rot, origin=[0, 0, 0]))
    return tuple(p[i] - z[i] for i in range(3))


def rnd(v):
    v = round(v, 4)
    return 0 if v == 0 else (int(v) if v == int(v) else v)


def held_matrix(display, hand='right'):
    """The game chain from the socket frame to item space, in blocks (mathutils 4x4):
    translate(t / 16) @ Rx @ Ry @ Rz @ scale @ translate(-0.5). The left hand negates t.x, ry
    and rz (ItemTransform.apply with leftHand), research 3.1."""
    from mathutils import Matrix
    d = display['thirdperson_%shand' % hand]
    t = list(d['translation'])
    rx, ry, rz = d['rotation']
    if hand == 'left':
        t[0], ry, rz = -t[0], -ry, -rz
    m = Matrix.Translation([c * PX for c in t])
    m = m @ Matrix.Rotation(math.radians(rx), 4, 'X') @ Matrix.Rotation(math.radians(ry), 4, 'Y') @ Matrix.Rotation(math.radians(rz), 4, 'Z')
    m = m @ Matrix.Diagonal(list(d['scale']) + [1])
    return m @ Matrix.Translation((-0.5, -0.5, -0.5))


class Equipment:
    """A vanilla-shape armour texture (64 x 32 humanoid layer, research 4.2): the regions of the
    vanilla armour mesh painted with palettes and decals. Cubes here only carry the layout."""

    # HumanoidModel armour layout: name -> (u, v, (w, h, d)).
    LAYOUT = {
        'head': (0, 0, (8, 8, 8)), 'hat': (32, 0, (8, 8, 8)),
        'body': (16, 16, (8, 12, 4)), 'arm': (40, 16, (4, 12, 4)), 'leg': (0, 16, (4, 12, 4)),
    }

    def __init__(self, set_id, palettes, layer='humanoid'):
        self.id, self.palettes, self.layer = set_id, palettes, layer
        self.regions = []  # (layout name, mat, decal)

    def region(self, name, mat, decal=None, grain=1.0):
        self.regions.append((name, mat, decal, grain))

    def paint(self, seed):
        from rig import Cube, FACES
        img = paint.Image(64, 32)
        for name, mat, decal, grain in self.regions:
            u, v, size = self.LAYOUT[name]
            c = Cube((0, 0, 0), size, mat, decal=decal, grain=grain)
            c.uv = (u, v)
            for fname, (u0, v0, u1, v1) in c.face_rects().items():
                f = paint.Face(img, fname, min(u0, u1), min(v0, v1), abs(u1 - u0), abs(v1 - v0), self.palettes, c)
                paint.fill(img, f, self.palettes, mat, seed, grain)
                if decal:
                    decal(f)
        return img

    def json(self):
        return {'layers': {self.layer: [{'texture': 'soulscraft:' + self.id}]}}


# ---------------------------------------------------------------------------- Blender preview

def preview_object(item, socket_obj, hand, image_path, name):
    """The item model (Item or Sprite) as a Blender object under a socket Empty, placed by the
    game chain: local matrix = K^T @ held_matrix (item vertices stay in item space, blocks)."""
    import bpy
    from mathutils import Matrix
    from rig import KT, texture_material
    img = bpy.data.images.load(image_path, check_existing=True)
    mat = bpy.data.materials.get('item:' + item.id) or texture_material('item:' + item.id, img)
    verts, faces, uvs = [], [], []
    s = item.texture
    for corners, uv, n in item.quads():
        base = len(verts)
        pts = [tuple(c * PX for c in p) for p in corners]
        # Counter-clockwise from outside.
        a, b, c = pts[0], pts[1], pts[2]
        cr = ((b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
              (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
              (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
        if sum(cr[i] * n[i] for i in range(3)) < 0:
            pts, uv = pts[::-1], uv[::-1]
        verts.extend(pts)
        faces.append(tuple(range(base, base + 4)))
        uvs.append([(u / s, 1 - v / s) for u, v in uv])
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    layer = me.uv_layers.new(name='UVMap')
    for poly, uv in zip(me.polygons, uvs):
        poly.use_smooth = False
        for li, corner in zip(poly.loop_indices, uv):
            layer.data[li].uv = corner
    me.materials.append(mat)
    me.validate()
    ob = bpy.data.objects.new(name, me)
    socket_obj.users_collection[0].objects.link(ob)
    ob.parent = socket_obj
    ob.matrix_parent_inverse = Matrix.Identity(4)
    ob.matrix_basis = KT.to_4x4() @ held_matrix(item.display, hand)
    ob['soulscraft_preview'] = True
    return ob


class Sprite:
    """A sprite item: a size x size texture that the game extrudes 1 px thick (see the module doc).

    parent GENERATED_PARENT is an icon, HANDHELD_PARENT a weapon or tool held like a vanilla
    sword: the blade runs from the handle at the bottom left to the tip at the top right, and the
    handheld display puts item px (4, 4) into the fist, the corner of texels (3..4, 11..12).
    The texture is painted from rows + key (a pixel map, paint.Face.rows), then by draw(c): c is
    a paint.Face over the whole sprite (px, rect, line, rows, edge, patch, streak, solid), colours
    '#rrggbb', '#00000000' for a hole, or (material, step) of palettes."""

    def __init__(self, item_id, draw=None, size=16, name=None, parent=GENERATED_PARENT, palettes=None,
                 rows=None, key=None):
        assert draw or rows, 'sprite %s needs draw or rows' % item_id
        assert parent in PARENT_DISPLAY, 'sprite %s: parent must be one of %s' % (item_id, sorted(PARENT_DISPLAY))
        self.id, self.draw, self.size, self.name = item_id, draw, size, name or item_id
        self.parent, self.palettes, self.rows, self.key = parent, palettes or {}, rows, key or {}
        self.display = PARENT_DISPLAY[parent]  # inherited, not written to the JSON
        self.texture = size  # texels per side, like Item.texture

    def image(self):
        img = paint.Image(self.size, self.size)
        c = paint.Face(img, 'front', 0, 0, self.size, self.size, self.palettes, None)
        if self.rows:
            assert len(self.rows) == self.size and all(len(r) == self.size for r in self.rows), \
                'sprite %s: rows must be %d strings of %d characters' % (self.id, self.size, self.size)
            c.rows(self.rows, self.key)
        if self.draw:
            self.draw(c)
        return img

    def quads(self):
        return extrude(self.image())

    def json(self):
        return {
            'credit': 'SoulsCraft; generated by assets/models/build.py, do not edit by hand',
            'parent': self.parent,
            'textures': {'layer0': 'soulscraft:item/' + self.id},
        }


def extrude(img):
    """The vanilla extrusion of a sprite (ItemModelGenerator) as quads in item space, px 0..16:
    [(corners [(x, y, z)] * 4, texel uvs [(u, v)] * 4, normal)].

    The game draws the front (south, z = 8.5) and the back (north, z = 7.5) as one quad each over
    the whole sprite and drops the transparent texels; here every opaque texel gets its own pair,
    which shows the same pixels without alpha in the Blender material. Side faces: one per texel
    edge with a transparent texel (or the sprite border) beyond it, 1 px deep, in the colour of
    that texel (UV at its centre). Texel rows run down, item +y runs up."""
    n = img.w
    k = 16 / n
    z0, z1 = SPRITE_DEPTH
    out = []
    for y in range(n):
        for x in range(n):
            if not img.opaque(x, y):
                continue
            x0, x1, y0, y1 = x * k, (x + 1) * k, (n - 1 - y) * k, (n - y) * k
            uv = [(x, y + 1), (x + 1, y + 1), (x + 1, y), (x, y)]
            out.append(([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], uv, (0, 0, 1)))
            out.append(([(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0)], uv, (0, 0, -1)))
            mid = [(x + 0.5, y + 0.5)] * 4
            for dx, dy, corners, normal in (
                    (0, -1, [(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], (0, 1, 0)),
                    (0, 1, [(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)], (0, -1, 0)),
                    (-1, 0, [(x0, y0, z0), (x0, y1, z0), (x0, y1, z1), (x0, y0, z1)], (-1, 0, 0)),
                    (1, 0, [(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)], (1, 0, 0))):
                if not img.opaque(x + dx, y + dy):
                    out.append((corners, mid, normal))
    return out
