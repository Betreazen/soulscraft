"""Trophies of the SoulsCraft mobs (docs/design/09-trophies.md): mob heads and the diorama, a
one-block miniature of the mob's home with the mob on it in its trophy pose. The game draws the
head and the diorama's figure with a block-entity renderer, from the mob's own rig.json and
texture; this module works out what that renderer needs and writes it next to the rig:

    assets/rigs/<mob>.trophy.json

    head      the head as rig parts (rig.json part format: the head part and its children, the
              top part re-rooted at its rest place in the model frame), on the floor (block
              state rotation 0..15, as vanilla skulls) and on a wall (facing)
    diorama   the block <mob>_trophy (blocks/dioramas.py: plinth and scene), the figure's clip
              (`trophy`, a slow loop), scale, offset, yaw and state flags, the block's light and
              the particles its block entity spawns

Frames (all of rig.py's model frame stays as it is):
    model     rig.json: px, +x the mob's left, +y down, -z its front, origin 24.016 px over the feet
    draw      blocks, feet at the origin, +y up, +z the mob's front: model = draw after
              scale(1, -1, -1), translate(0, -1.501, 0) (the mob's draw frame; the showcase's glTF)
    block     px from the block's bottom centre, +x east, +y up, +z south. Blocks are built facing
              north; `turn` below is the blockstate's turn.

The game chain of a trophy (BlockEntityDraw, every op multiplies on the right):

    translate(0.5, 0, 0.5)                       the block's bottom centre
    rotate_about(y, turn)                        floor head: -22.5 * rotation; wall head and
                                                 diorama: -facing_y (north 0, east 90, ...)
    translate(offset / 16)
    rotate_about(y, yaw)                         180 + the scene's turn: the figure faces the plaque
    scale(scale)
    scale(1, -1, -1); translate(0, -1.501, 0)    into the model frame, as for the mob itself
    draw the parts at the pose (the figure: clip `trophy` at (game_time + phase) % ticks plus
    partial; the head: its rest parts), state-flag parts as the flags say, items in sockets as
    for the mob

A mob file may give TROPHY (every key optional):

    TROPHY = {
        'head': {'parts': ['head'],      top parts of the head (with their children)
                 'drop': [],             parts left out (with their children): the lurker's tail
                 'rot': None,            rest rotation of a top part, degrees, instead of its
                                         place in the rest pose (a hunched head stood upright)
                 'scale': 1.0,           head scale; the build fails if it leaves the block
                 'icon': None},          (part, cube) whose front face is the item icon
        'clip': 'trophy',                the diorama's looping clip
        'flags': [],                     state flags on in the diorama (the brute's rage eyes)
    }

The scene of each mob (blocks/dioramas.py SCENES) gives the figure's box, its floor, its turn,
the light and the particles.
"""
import json
import math
import os
import runpy
import types

PX = 1 / 16
ORIGIN = 1.501  # blocks from the feet up to the model origin (translate(0, -1.501, 0))
HERE = os.path.dirname(os.path.abspath(__file__))
DIORAMAS = runpy.run_path(os.path.join(HERE, 'blocks', 'dioramas.py'))
SCENES = DIORAMAS['SCENES']

# Heads: on the floor centred on the block, standing on it; on a wall, the back of the head
# against the wall (the block's south face for facing north) and centred 8 px up, as vanilla.
HEAD_FLOOR = {'yaw': 0, 'lo': (-8, 0, -8), 'hi': (8, 16, 8), 'anchor': ('centre', 'bottom', 'centre')}
HEAD_WALL = {'yaw': 180, 'lo': (-8, 0, -8), 'hi': (8, 16, 8), 'anchor': ('centre', 'centre', 'back')}

CUBE_FACES = ('front', 'back', 'left', 'right', 'top', 'bottom')


def num(v, places=4):
    v = round(v, places)
    return 0 if v == 0 else (int(v) if v == int(v) else v)


# ---------------------------------------------------------------------------- matrices

def entry():
    """draw <- model, blocks: scale(1, -1, -1) @ translate(0, -1.501, 0)."""
    from mathutils import Matrix
    return Matrix.Diagonal((1, -1, -1, 1)) @ Matrix.Translation((0, -ORIGIN, 0))


def rot_y(deg):
    from mathutils import Matrix
    return Matrix.Rotation(math.radians(deg), 4, 'Y')


def place_matrix(turn, offset_px, yaw, scale):
    """block (blocks, from the block's bottom centre) <- draw frame: the game chain above."""
    from mathutils import Matrix, Vector
    return (rot_y(turn) @ Matrix.Translation(Vector(offset_px) * PX) @ rot_y(yaw)
            @ Matrix.Diagonal((scale, scale, scale, 1)))


def part_matrix(pivot, angles, scale=None):
    """A part's local matrix, blocks (model frame): translate(pivot / 16), Rz Ry Rx, scale."""
    from mathutils import Matrix, Vector
    m = (Matrix.Translation(Vector(pivot) * PX) @ Matrix.Rotation(angles[2], 4, 'Z')
         @ Matrix.Rotation(angles[1], 4, 'Y') @ Matrix.Rotation(angles[0], 4, 'X'))
    if scale and any(abs(s - 1) > 1e-9 for s in scale):
        m = m @ Matrix.Diagonal(tuple(scale) + (1,))
    return m


def decompose_zyx(r):
    sy = max(-1.0, min(1.0, -r[2][0]))
    y = math.asin(sy)
    if abs(sy) < 0.9999999:
        return math.atan2(r[2][1], r[2][2]), y, math.atan2(r[1][0], r[0][0])
    return 0.0, y, math.atan2(-r[0][1], r[1][1])


# ---------------------------------------------------------------------------- rig data

def pose_rows(rig, clip=None, tick=0):
    """{part: (pivot px, angles rad, scale or None)}: the rest pose, or a clip at a tick."""
    rows = {p['name']: (p['pivot'], p['rotation'], None) for p in rig['parts']}
    if clip:
        c = rig['clips'][clip]
        assert 0 <= tick <= c['ticks'], '%s: tick %d outside 0..%d' % (clip, tick, c['ticks'])
        for name, track in c['tracks'].items():
            r = track[tick]
            rows[name] = (r[:3], r[3:6], r[6:9] if len(r) > 6 else None)
    return rows


def world_of(parts, rows):
    out = {}
    for p in parts:
        m = part_matrix(*rows[p['name']])
        out[p['name']] = out[p['parent']] @ m if p['parent'] else m
    return out


def shown_parts(parts, hide=(), flags=()):
    """Names of the parts drawn: not hidden, no hidden ancestor, state flag as rig.json says."""
    out = set()
    for p in parts:
        st = p.get('state')
        ok = p['name'] not in hide and (p['parent'] is None or p['parent'] in out)
        if st:
            ok = ok and ((st.lstrip('!') in flags) != st.startswith('!'))
        if ok:
            out.add(p['name'])
    return out


class Items:
    """Item models held in sockets, read from assets/soulscraft: display, sprite pixels or
    elements, for the points (fit) and faces (z-fighting) of a held item."""

    def __init__(self, ns):
        self.ns, self.cache = ns, {}

    def get(self, item):
        if item not in self.cache:
            import items
            import paint
            iid = item.split(':', 1)[1]
            with open(os.path.join(self.ns, 'models', 'item', iid + '.json'), encoding='utf-8') as f:
                model = json.load(f)
            display = model.get('display') or items.PARENT_DISPLAY[model['parent']]
            if 'elements' in model:
                els = [types.SimpleNamespace(frm=e['from'], to=e['to'], faces=list(e['faces']), rotation=e.get('rotation'))
                       for e in model['elements']]
                pts = []
                for e in els:
                    for i in range(8):
                        p = [e.to[k] if i >> k & 1 else e.frm[k] for k in range(3)]
                        pts.append(items.rotate_el(p, e.rotation))
            else:
                els = None
                img = paint.load_png(os.path.join(self.ns, 'textures', 'item', iid + '.png'))
                pts = [c for corners, uv, n in items.extrude(img) for c in corners]
            self.cache[item] = (display, els, [tuple(c * PX for c in p) for p in pts])
        return self.cache[item]


def socket_matrix(world, s, display):
    """Model frame (blocks) <- item space for an item in a socket: the parent chain, the
    socket's translate and rotations, then display.thirdperson_<hand>hand (items.held_matrix)."""
    import items
    from mathutils import Matrix, Vector
    m = Matrix.Translation(Vector(s['offset']) * PX)
    for axis, deg in s['rotate']:
        m = m @ Matrix.Rotation(math.radians(deg), 4, axis.upper())
    hand = 'left' if s.get('display', '').endswith('lefthand') else 'right'
    return world[s['parent']] @ m @ items.held_matrix(display, hand)


def geometry(rig, parts, rows, shown, held, label, matrix=None, faces=False):
    """Points (and, with faces=True, zfight faces) of the shown parts and held items, in the
    frame `matrix` @ draw (default: the draw frame), blocks."""
    import zfight
    from mathutils import Vector
    m0 = entry() if matrix is None else matrix @ entry()
    world = world_of(parts, rows)
    pts, out_faces = [], []
    for p in parts:
        if p['name'] not in shown:
            continue
        w = m0 @ world[p['name']]
        if abs(w.to_3x3().determinant()) < 1e-12:
            continue  # a part scaled to nothing (a lure gone dark, a bolt not drawn)
        for i, c in enumerate(p['cubes']):
            g = c.get('inflate', 0)
            g = g if isinstance(g, list) else [g] * 3
            lo = [(c['origin'][k] - g[k]) * PX for k in range(3)]
            hi = [(c['origin'][k] + c['size'][k] + g[k]) * PX for k in range(3)]
            lo, hi = [min(a, b) for a, b in zip(lo, hi)], [max(a, b) for a, b in zip(lo, hi)]
            for j in range(8):
                pts.append(w @ Vector([hi[k] if j >> k & 1 else lo[k] for k in range(3)]))
            if faces:
                out_faces.extend(zfight.box_faces(lo, hi, c.get('faces', CUBE_FACES), zfight.CUBE_FACES, w,
                                                  '%s:%s#%d' % (label, p['name'], i)))
    for s in rig['sockets']:
        if not s.get('item') or s['parent'] not in shown:
            continue
        display, els, ipts = held.get(s['item'])
        w = m0 @ socket_matrix(world, s, display)
        pts.extend(w @ Vector(p) for p in ipts)
        if faces and els:
            for i, e in enumerate(els):
                out_faces.extend(zfight.element_faces(e, w, '%s:held:%s#%d' % (label, s['item'], i)))
    return pts, out_faces


# ---------------------------------------------------------------------------- the head as rig parts

def head_parts(rig, spec):
    """The head's rig parts: each top part with its children, minus `drop`; a top part gets
    parent None and its rest place in the model frame (absolute pivot px and Z-Y-X angles), or
    the angles of spec['rot']. Same format as rig.json parts."""
    from mathutils import Matrix
    by = {p['name']: p for p in rig['parts']}
    rest = world_of(rig['parts'], pose_rows(rig))
    tops, drop = spec.get('parts', ['head']), set(spec.get('drop', ()))
    out = []

    def walk(name, top):
        if name in drop:
            return
        p = json.loads(json.dumps(by[name]))
        if top:
            w = rest[name]
            p['parent'] = None
            p['pivot'] = [num(c / PX, 5) for c in w.translation]
            if spec.get('rot') is not None:
                r = Matrix.Rotation(math.radians(spec['rot'][2]), 3, 'Z') @ Matrix.Rotation(math.radians(spec['rot'][1]), 3, 'Y') \
                    @ Matrix.Rotation(math.radians(spec['rot'][0]), 3, 'X')
            else:
                r = w.to_3x3()
            p['rotation'] = [num(a, 5) for a in decompose_zyx(r)]
        out.append(p)
        for q in rig['parts']:
            if q['parent'] == name:
                walk(q['name'], False)
    for t in tops:
        walk(t, True)
    return out


# ---------------------------------------------------------------------------- fitting

def extent(pts, yaw):
    """min, max (px) of points (draw frame, blocks) turned by yaw."""
    r = rot_y(yaw).to_3x3()
    q = [r @ p for p in pts]
    return ([min(v[k] for v in q) / PX for k in range(3)], [max(v[k] for v in q) / PX for k in range(3)])


def fit_scale(ext, region, cap=1.0):
    lo, hi = ext
    s = cap
    for k in range(3):
        size = hi[k] - lo[k]
        if size > 1e-9:
            s = min(s, (region['hi'][k] - region['lo'][k]) / size)
    return s


def anchor(ext, region, s):
    lo, hi = ext
    out = []
    for k, how in enumerate(region['anchor']):
        rlo, rhi = region['lo'][k], region['hi'][k]
        if how == 'bottom':
            v = rlo - s * lo[k]
        elif how == 'back':
            v = rhi - s * hi[k]
        else:
            v = (rlo + rhi) / 2 - s * (lo[k] + hi[k]) / 2
        out.append(num(v, 3))
    return out


def figure_fit(exts, feet, region):
    """Scale and offset (block px) that put a figure into region (lo, hi, floor), the same for
    every tick of its loop so it does not jump. exts: per tick (lo, hi) px of the figure turned to
    its yaw at scale 1; feet: per tick the lowest y of the parts that stand on the floor. The lowest
    foot over the loop stands on the floor; nothing goes below lo or above hi; x and z centred."""
    lo = [min(e[0][k] for e in exts) for k in range(3)]
    hi = [max(e[1][k] for e in exts) for k in range(3)]
    foot = min(feet)
    rlo, rhi, floor = region['lo'], region['hi'], region['floor']
    s = 1.0
    for k in (0, 2):
        s = min(s, (rhi[k] - rlo[k]) / (hi[k] - lo[k]))
    s = min(s, (rhi[1] - floor) / (hi[1] - foot))
    if foot - lo[1] > 1e-9:
        s = min(s, (floor - rlo[1]) / (foot - lo[1]))
    s = num(math.floor(s * 1e4) / 1e4)
    off = [num((rlo[0] + rhi[0]) / 2 - s * (lo[0] + hi[0]) / 2, 3), num(floor - s * foot, 3),
           num((rlo[2] + rhi[2]) / 2 - s * (lo[2] + hi[2]) / 2, 3)]
    return s, off


def descendants(parts, names):
    """Names of the parts in `names` and all their children."""
    out = set(names)
    for p in parts:
        if p['parent'] in out:
            out.add(p['name'])
    return out


class Trophy:
    """Everything worked out for one mob: export() is its trophy.json."""

    def __init__(self, mob, rig, spec, held):
        self.mob, self.rig, self.spec, self.held = mob, rig, spec, held
        self.id = rig['id']
        self.scene = SCENES[self.id]
        self.head = head_parts(rig, spec.get('head', {}))
        self.head_fit = {}
        self._fit_head()
        self._fit_diorama()

    def figure(self, clip, tick, matrix=None, faces=False, only=None):
        """Points (and faces) of the figure: the full rig at a pose, the state flags on; with
        only, just those parts and their children (no held items)."""
        parts = self.rig['parts']
        shown = shown_parts(parts, (), self.flags)
        rig = self.rig
        if only:
            shown &= descendants(parts, only)
            rig = {'sockets': []}
        return geometry(rig, parts, pose_rows(self.rig, clip, tick), shown, self.held, self.id, matrix, faces)

    def head_geometry(self, matrix=None, faces=False):
        rows = {p['name']: (p['pivot'], p['rotation'], None) for p in self.head}
        return geometry({'sockets': []}, self.head, rows, shown_parts(self.head), self.held, self.id + ':head', matrix, faces)

    def _fit_head(self):
        hs = self.spec.get('head', {})
        s = hs.get('scale', 1.0)
        pts, _ = self.head_geometry()
        for where, region in (('floor', HEAD_FLOOR), ('wall', HEAD_WALL)):
            ext = extent(pts, region['yaw'])
            assert fit_scale(ext, region, 99) >= s - 1e-6, '%s head: scale %.3f leaves the block (%s)' % (self.id, s, where)
            self.head_fit[where] = {'scale': num(s), 'offset': anchor(ext, region, s), 'yaw': region['yaw']}

    def ticks(self):
        """The ticks the figure shows: 0 .. ticks - 1 of a loop (its last tick is its first)."""
        c = self.rig['clips'][self.clip]
        return range(c['ticks'] if c.get('loop') else c['ticks'] + 1)

    def _fit_diorama(self):
        sc = self.scene
        self.clip = self.spec.get('clip', 'trophy')
        assert self.clip in self.rig['clips'], '%s: no clip %s for the diorama' % (self.id, self.clip)
        self.flags = tuple(self.spec.get('flags', ()))
        self.yaw = num(180 + sc.get('turn', 0))
        feet_parts = sc['figure'].get('feet')
        exts, feet = [], []
        for t in self.ticks():
            exts.append(extent(self.figure(self.clip, t)[0], self.yaw))
            feet.append(extent(self.figure(self.clip, t, only=feet_parts)[0], self.yaw)[0][1] if feet_parts else exts[-1][0][1])
        self.scale, self.offset = figure_fit(exts, feet, sc['figure'])
        self.size = [num((max(e[1][k] for e in exts) - min(e[0][k] for e in exts)) * self.scale, 2) for k in range(3)]

    def export(self):
        rig, sc = self.rig, self.scene
        c = rig['clips'][self.clip]
        lo, hi = DIORAMAS['PLAQUE']
        return {
            'format': 'soulscraft-trophy', 'version': 2, 'id': self.id, 'name': self.mob['name'],
            'rig': 'soulscraft:' + self.id,
            'frame': 'block px from the bottom centre, +x east, +y up, +z south; chain: translate(0.5, 0, 0.5), '
                     'rotate_about(y, turn), translate(offset / 16), rotate_about(y, yaw), scale(scale), '
                     'scale(1, -1, -1), translate(0, -1.501, 0), parts at the pose; turn: floor head -22.5 * rotation, '
                     'wall head and diorama -facing_y',
            'head': {'texture': rig['texture'], 'parts': self.head, 'icon': self.icon(),
                     'floor': self.head_fit['floor'], 'wall': self.head_fit['wall']},
            'diorama': {
                'block': 'soulscraft:%s_trophy' % self.id,
                'clip': self.clip, 'ticks': c['ticks'], 'loop': bool(c.get('loop')),
                'flags': list(self.flags),
                'scale': self.scale, 'offset': self.offset, 'yaw': self.yaw,
                'light': sc['light'], 'text': sc['text'],
                'plaque': {'from': list(lo), 'to': list(hi)},
                'particles': [{'type': 'soulscraft:' + p['type'], 'every': p['every'],
                               'box': {'from': list(p['box'][0]), 'to': list(p['box'][1])}} for p in sc['particles']],
            },
        }

    def icon(self):
        """(part, cube) whose front face makes the head item's icon: spec['head']['icon'], else
        the first cube of the first top part that has one."""
        ic = self.spec.get('head', {}).get('icon')
        if ic:
            return list(ic)
        for p in self.head:
            if p['cubes']:
                return [p['name'], 0]
        raise AssertionError('%s head has no cubes' % self.id)

    # ------------------------------------------------------------------ checks

    def placements(self):
        """[(label, base model id, matrix block <- draw, (clip, tick) or 'head')]: the diorama's
        figure at each tick of its loop, the head on the floor and on a wall."""
        out = []
        m = place_matrix(0, self.offset, self.yaw, self.scale)
        for t in self.ticks():
            out.append(('%s/diorama/%d' % (self.id, t), self.id + '_trophy', m, (self.clip, t)))
        for where in ('floor', 'wall'):
            f = self.head_fit[where]
            out.append(('%s/head_%s' % (self.id, where), where, place_matrix(0, f['offset'], f['yaw'], f['scale']), 'head'))
        return out

    def check(self, base_models):
        """Every placement inside its block (the figure inside its scene box too) and free of
        z-fighting with its base: the diorama's model; a wall cube south of a wall head, the floor
        under a floor head. Returns lines."""
        import blocks
        import zfight
        bad = []
        lim = 0.01 * PX
        box = self.scene['figure']
        cells = {}
        for label, base, m, pose in self.placements():
            if pose == 'head':
                pts, faces = self.head_geometry(m, faces=True)
            else:
                pts, faces = self.figure(pose[0], pose[1], m, faces=True)
                for p in pts:
                    if not all(box['lo'][k] * PX - lim <= p[k] <= box['hi'][k] * PX + lim for k in range(3)):
                        bad.append('%s: leaves its scene box at (%.2f, %.2f, %.2f) px' % (label, p.x / PX, p.y / PX, p.z / PX))
                        break
            for p in pts:
                if not (-0.5 - lim <= p.x <= 0.5 + lim and -lim <= p.y <= 1 + lim and -0.5 - lim <= p.z <= 0.5 + lim):
                    bad.append('%s: leaves the block at (%.2f, %.2f, %.2f) px' % (label, p.x / PX, p.y / PX, p.z / PX))
                    break
            if base not in cells:
                if base == 'wall':
                    cells[base] = blocks.model_faces(base_models['_full'], blocks.placement_matrix((0, 0, 1), 0), 'wall')
                elif base == 'floor':
                    cells[base] = blocks.model_faces(base_models['_full'], blocks.placement_matrix((0, -1, 0), 0), 'floor')
                else:
                    cells[base] = blocks.model_faces(base_models[base], blocks.placement_matrix((0, 0, 0), 0), base)
            for a, b, ar, how in zfight.fights(cells[base] + faces):
                if (a.split(':')[0] == self.id) != (b.split(':')[0] == self.id):
                    bad.append('%s: %s <-> %s, %.2f px^2, %s' % (label, a, b, ar, how))
        return bad


def base_models(here):
    """{model id: blocks.Model} of the dioramas (blocks/dioramas.py) and a full cube."""
    import blocks
    out = {}
    for b in DIORAMAS['BLOCKS']:
        out.update(b.models())
    full = blocks.Model('_full', 'x')
    full.box((0, 0, 0), (16, 16, 16), 'x')
    out['_full'] = full
    return out


def build_all(root, here, specs, mobs):
    """Fits every mob with a rig, writes assets/rigs/<mob>.trophy.json, returns the problems
    found (an empty list when every trophy fits its block and fights nothing)."""
    import rig as rigmod
    held = Items(os.path.join(root, 'assets', 'soulscraft'))
    bases = base_models(here)
    problems = []
    for mob_id in sorted(specs):
        path = os.path.join(root, 'assets', 'rigs', mob_id + '.rig.json')
        with open(path, encoding='utf-8') as f:
            rig = json.load(f)
        t = Trophy(mobs[mob_id], rig, specs[mob_id] or {}, held)
        problems.extend(t.check(bases))
        with open(os.path.join(root, 'assets', 'rigs', mob_id + '.trophy.json'), 'w', encoding='utf-8', newline='\n') as f:
            f.write(rigmod.dumps(t.export()) + '\n')
        r = t.scene['figure']
        print('trophies.py: %-9s head %.2f, diorama %.4f at %s, size %s px in %s px, clip %s, %d ticks' % (
            mob_id, t.head_fit['floor']['scale'], t.scale, t.offset, t.size,
            [num(r['hi'][k] - (r['floor'] if k == 1 else r['lo'][k]), 2) for k in range(3)], t.clip, len(t.ticks())))
    return problems
