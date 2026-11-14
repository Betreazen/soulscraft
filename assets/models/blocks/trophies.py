"""Mob heads (docs/design/09-trophies.md): <mob>_head (state rotation 0..15, on the floor) and
<mob>_wall_head (facing). Models without elements, the block entity draws the head from the mob's
rig; the item is a 3D head on the mob's texture (HeadItem), worn on the head like a vanilla mob head.
The trophy proper, the diorama, is blocks/dioramas.py.
"""
import json
import math
import os

import items
import paint
from blocks import Block, Texture
from paint import Material

SET = 'trophies'
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
    MOBS = json.load(f)['mobs']

PALETTES = {
    'bone': Material(['#6f6650', '#8c826a', '#a99f84', '#c3b99c', '#d8cfb3'], noise=0.45, patch=3, fine=0.1),
}
TEXTURES = [Texture('trophy_bone', 'bone')]   # the heads' break particles


# ---------------------------------------------------------------------------- heads

def no_elements(m):
    """A mob head: the block entity draws it; the model gives only the break particles."""


# ---------------------------------------------------------------------------- the 3D head item

# Where an item face's texels are on the mob texture (box UV of rig.json, as items.Element.rects):
# (du from u, dv from v, width axis, height axis) with w, h, d the cube's texel sizes along the
# item frame's x, y, z. The item frame is the helmets' (items.from_head_cubes): +x the mob's right
# (east), +y up, -z the mob's front (north).
_RECT = {
    'up': ('d', '0', 'w', 'd'), 'down': ('d+w', '0', 'w', 'd'),
    'east': ('0', 'd', 'd', 'h'), 'north': ('d', 'd', 'w', 'h'),
    'west': ('d+w', 'd', 'd', 'h'), 'south': ('2d+w', 'd', 'w', 'h'),
}
_RIG_FACE = {'top': 'up', 'bottom': 'down', 'right': 'east', 'left': 'west', 'front': 'north', 'back': 'south'}
_DIRS = {'east': (1, 0, 0), 'west': (-1, 0, 0), 'up': (0, 1, 0), 'down': (0, -1, 0), 'south': (0, 0, 1), 'north': (0, 0, -1)}
_SNAP = (-45.0, -22.5, 0.0, 22.5, 45.0)
# Item models may sit from -16 to 32 px (the game's element limit).
_LIMIT = (-16.0, 32.0)


def _mat_rot(rx, ry, rz):
    """ModelPart's turn, Z then Y then X (README «Оси»): Rz · Ry · Rx."""
    cx, sx, cy, sy, cz, sz = math.cos(rx), math.sin(rx), math.cos(ry), math.sin(ry), math.cos(rz), math.sin(rz)
    rxm = ((1, 0, 0), (0, cx, -sx), (0, sx, cx))
    rym = ((cy, 0, sy), (0, 1, 0), (-sy, 0, cy))
    rzm = ((cz, -sz, 0), (sz, cz, 0), (0, 0, 1))
    return _mul(_mul(rzm, rym), rxm)


def _mul(a, b):
    return tuple(tuple(sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)) for i in range(3))


def _apply(m, v):
    return tuple(sum(m[i][k] * v[k] for k in range(3)) for i in range(3))


def _grow(cube):
    g = cube.get('inflate', 0)
    return [float(g)] * 3 if not isinstance(g, list) else [float(x) for x in g]


class HeadItem:
    """The head item (`<mob>_head`): the head's cubes from the trophy file as model elements, at the
    floor head's scale, on the mob's own texture (copied to textures/item/<mob>_head.png), with
    the helmets' display (items.HELMET; display.head identity: CustomHeadLayer puts item px p at
    head px (8 - p) * 0.625). Worn, it sits on the player's head as the head sits on the mob,
    centred on the head's main cube; in the inventory and the hand it is a small 3D head.

    A part's turn becomes the element's: quarter turns swap the box's sides, what is left snaps
    to the one axis and the 22.5-degree steps (to 45) block models allow. The lurker's stalk and
    the slime's roots lean a little differently from the rig; a 90-degree turned face keeps its
    texels unturned."""

    def __init__(self, mob_id):
        self.id, self.mob = mob_id + '_head', mob_id
        self.name = 'Голова: ' + MOBS_BY[mob_id]['name']
        with open(os.path.join(ROOT, 'assets', 'rigs', mob_id + '.trophy.json'), encoding='utf-8') as f:
            head = json.load(f)['head']
        self.tex_w, self.tex_h = head['texture']['width'], head['texture']['height']
        self.scale = head['floor']['scale']
        parts = {p['name']: p for p in head['parts']}
        world = {}
        for p in head['parts']:
            m = _mat_rot(*p['rotation'])
            if p['parent'] is None:
                world[p['name']] = (m, tuple(p['pivot']))
            else:
                pm, pt = world[p['parent']]
                world[p['name']] = (_mul(pm, m), tuple(a + b for a, b in zip(pt, _apply(pm, p['pivot']))))
        self.cubes = []
        for p in head['parts']:
            m, t = world[p['name']]
            for c in p['cubes']:
                self.cubes.append((m, t, c))
        part, index = head['icon']
        m, t = world[part]
        c = parts[part]['cubes'][index]
        centre = [c['origin'][i] + c['size'][i] / 2 for i in range(3)]
        self.anchor = tuple(a + b for a, b in zip(t, _apply(m, centre)))
        self.elements = [e for e in (self._element(*x) for x in self.cubes) if e]
        lo = [min(min(e['from'][i], e['to'][i]) for e in self.elements) for i in range(3)]
        hi = [max(max(e['from'][i], e['to'][i]) for e in self.elements) for i in range(3)]
        for i in range(3):
            assert _LIMIT[0] <= lo[i] and hi[i] <= _LIMIT[1], 'head item %s: element outside -16..32 px on axis %d' % (self.id, i)
        self.extent = max(hi[i] - lo[i] for i in range(3))

    def _to_item(self, p):
        """Model px (y down, -z front) to item px: x and y flipped, scaled about the anchor."""
        k = self.scale / items.HEAD_SCALE
        a = self.anchor
        return (8 - (p[0] - a[0]) * k, 8 - (p[1] - a[1]) * k, 8 + (p[2] - a[2]) * k)

    def _element(self, m, t, cube):
        size, origin, grow = cube['size'], cube['origin'], _grow(cube)
        centre = [origin[i] + size[i] / 2 for i in range(3)]
        half = [size[i] / 2 + grow[i] for i in range(3)]
        c = self._to_item(tuple(a + b for a, b in zip(t, _apply(m, centre))))
        # The turn in the item frame: D R D with D = diag(-1, -1, 1) (the flips of _to_item).
        d = (-1, -1, 1)
        r = tuple(tuple(d[i] * m[i][j] * d[j] for j in range(3)) for i in range(3))
        # Its nearest quarter-turn permutation P (signed), and the rest Q = Pt R.
        perm = []
        for j in range(3):
            col = [r[i][j] for i in range(3)]
            i = max(range(3), key=lambda n: abs(col[n]))
            perm.append((i, 1 if col[i] > 0 else -1))   # local axis j -> item axis i, sign
        if len({i for i, _ in perm}) < 3:
            perm = [(0, 1), (1, 1), (2, 1)]
        pmat = [[0] * 3 for _ in range(3)]
        for j, (i, s) in enumerate(perm):
            pmat[i][j] = s
        q = tuple(tuple(sum(pmat[k][i] * r[k][j] for k in range(3)) for j in range(3)) for i in range(3))
        omega = ((q[2][1] - q[1][2]) / 2, (q[0][2] - q[2][0]) / 2, (q[1][0] - q[0][1]) / 2)
        axis = max(range(3), key=lambda n: abs(omega[n]))
        angle = math.degrees(math.asin(max(-1.0, min(1.0, omega[axis]))))
        angle = min(_SNAP, key=lambda s: abs(s - angle))
        k = self.scale / items.HEAD_SCALE
        ext = [0.0] * 3
        for j, (i, _) in enumerate(perm):
            ext[i] = half[j] * k
        el = {'from': [items.rnd(c[i] - ext[i]) for i in range(3)], 'to': [items.rnd(c[i] + ext[i]) for i in range(3)]}
        if angle:
            el['rotation'] = {'angle': angle, 'axis': 'xyz'[axis], 'origin': [items.rnd(v) for v in c]}
        if cube.get('glow'):
            el['light_emission'] = 15
        shown = None
        if cube.get('faces'):
            shown = {_RIG_FACE[f] for f in cube['faces']}
        el['faces'] = self._faces(cube, perm, shown)
        return el if el['faces'] else None

    def _faces(self, cube, perm, shown):
        w, h, dd = cube['size']
        u, v = cube['uv']
        val = {'0': 0, 'd': dd, 'd+w': dd + w, '2d+w': 2 * dd + w, 'w': w, 'h': h}
        out = {}
        for face, n in _DIRS.items():
            # The cube's own face that looks this way: P maps local axis j to item axis i.
            local = None
            for j, (i, s) in enumerate(perm):
                if n[i]:
                    vec = [0, 0, 0]
                    vec[j] = n[i] * s
                    local = next(f for f, dv in _DIRS.items() if tuple(vec) == dv)
            if shown is not None and local not in shown:
                continue
            if cube.get('mirror') and local in ('east', 'west'):
                local = 'west' if local == 'east' else 'east'
            du, dv, fw, fh = _RECT[local]
            u0, v0 = u + val[du], v + val[dv]
            u1, v1 = u0 + val[fw], v0 + val[fh]
            if cube.get('mirror'):
                u0, u1 = u1, u0
            sx, sy = 16 / self.tex_w, 16 / self.tex_h
            out[face] = {'uv': [items.rnd(u0 * sx), items.rnd(v0 * sy), items.rnd(u1 * sx), items.rnd(v1 * sy)], 'texture': '#0'}
        return out

    def display(self):
        """The helmets' display; a head larger than a block is shown smaller in the inventory, on the
        ground, in a frame and in the hand (by its extent)."""
        f = min(1.0, 16.0 / self.extent)
        out = {}
        # First person: the helmets have none, so a held head would be drawn at its full size in
        # the view; this is vanilla's block/block first-person display.
        hands = {'firstperson_righthand': {'rotation': [0, 45, 0], 'translation': [0, 0, 0], 'scale': [0.4, 0.4, 0.4]},
                 'firstperson_lefthand': {'rotation': [0, 225, 0], 'translation': [0, 0, 0], 'scale': [0.4, 0.4, 0.4]}}
        for ctx, d in list(items.HELMET.items()) + list(hands.items()):
            if ctx == 'head':
                out[ctx] = d
            else:
                out[ctx] = dict(d, scale=[items.rnd(s * f) for s in d['scale']])
        return out

    def json(self):
        tex = 'soulscraft:item/' + self.id
        return {
            'credit': 'SoulsCraft; generated by assets/models/build_blocks.py, do not edit by hand',
            'textures': {'0': tex, 'particle': tex},
            'elements': self.elements,
            'display': self.display(),
        }

    def image(self):
        """The mob's own texture: the faces' UVs point into it."""
        return paint.load_png(os.path.join(ROOT, 'assets', 'textures', 'entity', self.mob + '.png'))


MOBS_BY = {m['id']: m for m in MOBS}
HEADS = [m['id'] for m in MOBS if os.path.exists(os.path.join(ROOT, 'assets', 'rigs', m['id'] + '.trophy.json'))]
ICONS = {mid: HeadItem(mid) for mid in HEADS}
SPRITES = list(ICONS.values())

BLOCKS = []
for mid in HEADS:
    name = MOBS_BY[mid]['name']
    BLOCKS.append(Block(mid + '_head', 'Голова: ' + name, no_elements, entity_states={'rotation': tuple(range(16))},
                        particle='trophy_bone', item=ICONS[mid], role='Голова на полу, 16 направлений'))
    BLOCKS.append(Block(mid + '_wall_head', 'Голова на стене: ' + name, no_elements, facing=True, particle='trophy_bone',
                        item=ICONS[mid], role='Голова на стене'))
