"""Z-fighting check: coplanar overlapping faces of cubes, in the rest pose and on every tick.

Two faces fight when they lie in one plane (within EPS_PX), overlap by more than AREA_PX2, and
either face the same way, or face opposite ways while one of the two cubes is flat (no thickness
along the normal; the showcase draws faces from both sides, as the game's entityCutoutNoCull does).
Two solid cubes that touch face to face are not a fight: both faces are inside the pair.

Checked: every cube of a model against every other (same part or not, the same cube excluded),
with inflate, after the part transforms (pivot, rotation, scale) of the rest pose and of every
tick of every clip; element items held in a socket (the game's item chain); and the elements of
every element item model against each other. Sprites (extruded item/handheld, item/generated)
are left out: a 1 px blade does not lie in the plane of a cube face.

A cube may opt out with Cube(..., coplanar_ok='why it cannot be seen'), only when the fight is
provably hidden; the reason stays in the mob file. No cube uses it today.
"""
import math

from mathutils import Matrix, Vector

PX = 1 / 16
EPS_PX = 0.001        # coplanar within this, px
AREA_PX2 = 0.001      # overlap above this, px^2

# Cube faces: axis, side (0 = low, 1 = high).
CUBE_FACES = {'right': (0, 0), 'left': (0, 1), 'top': (1, 0), 'bottom': (1, 1), 'front': (2, 0), 'back': (2, 1)}
# Element faces in item space (y up): west -x, east +x, down -y, up +y, north -z, south +z.
EL_FACES = {'west': (0, 0), 'east': (0, 1), 'down': (1, 0), 'up': (1, 1), 'north': (2, 0), 'south': (2, 1)}


def rot_game(x, y, z):
    return (Matrix.Rotation(z, 4, 'Z') @ Matrix.Rotation(y, 4, 'Y') @ Matrix.Rotation(x, 4, 'X'))


def part_matrix(pivot_px, angles, scale):
    m = Matrix.Translation(Vector(pivot_px) * PX) @ rot_game(*angles)
    if scale and any(abs(s - 1) > 1e-9 for s in scale):
        m = m @ Matrix.Diagonal(tuple(scale) + (1,))
    return m


def box_faces(lo, hi, names, table, matrix, label, pre=None):
    """Faces of an axis-aligned box lo..hi (blocks) under matrix: [(label, corners, thickness)].
    pre: a function applied to local points first (an element's rotation)."""
    out = []
    thick = [hi[i] - lo[i] for i in range(3)]
    for name in names:
        axis, side = table[name]
        a, b = (axis + 1) % 3, (axis + 2) % 3
        pts = []
        for u, v in ((0, 0), (1, 0), (1, 1), (0, 1)):
            p = [0.0, 0.0, 0.0]
            p[axis] = hi[axis] if side else lo[axis]
            p[a] = hi[a] if u else lo[a]
            p[b] = hi[b] if v else lo[b]
            if pre:
                p = pre(p)
            pts.append(matrix @ Vector(p))
        n = (pts[1] - pts[0]).cross(pts[3] - pts[0])
        if n.length < 1e-12:
            continue  # a face of zero area (a part scaled to 0, a flat side)
        if side == 0:
            n = -n  # outward; the corner order above runs the same way on both sides
        # Thickness along the normal in world units, from the local box and the matrix's scale.
        t = (matrix.to_3x3() @ Vector([thick[i] if i == axis else 0 for i in range(3)])).length
        out.append((label, pts, n.normalized(), t))
    return out


def cube_faces(cube, matrix, label):
    lo, hi = [], []
    for i in range(3):
        a = cube.origin[i] - cube.inflate[i]
        b = cube.origin[i] + cube.size[i] + cube.inflate[i]
        lo.append(min(a, b) * PX)
        hi.append(max(a, b) * PX)
    return box_faces(lo, hi, cube.faces, CUBE_FACES, matrix, label)


def element_faces(el, matrix, label):
    from items import rotate_el
    pre = (lambda p: rotate_el(p, el.rotation)) if el.rotation else None
    lo = [c for c in el.frm]
    hi = [c for c in el.to]
    m = matrix @ Matrix.Diagonal((PX, PX, PX, 1))
    return box_faces(lo, hi, el.faces, EL_FACES, m, label, pre)


def clip_polygon(poly, edge_a, edge_b):
    """Sutherland-Hodgman: the part of a convex polygon (2D) left of the edge a -> b."""
    def side(p):
        return (edge_b[0] - edge_a[0]) * (p[1] - edge_a[1]) - (edge_b[1] - edge_a[1]) * (p[0] - edge_a[0])
    out = []
    for i, p in enumerate(poly):
        q = poly[(i + 1) % len(poly)]
        sp, sq = side(p), side(q)
        if sp >= 0:
            out.append(p)
        if (sp >= 0) != (sq >= 0):
            t = sp / (sp - sq)
            out.append((p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t))
    return out


def area(poly):
    return 0.5 * sum(p[0] * q[1] - q[0] * p[1] for p, q in zip(poly, poly[1:] + poly[:1]))


def overlap(f, g):
    """Area (blocks^2) of the overlap of two coplanar faces."""
    n = f[2]
    u = (f[1][1] - f[1][0]).normalized()
    v = n.cross(u)
    o = f[1][0]

    def flat(pts):
        poly = [((p - o).dot(u), (p - o).dot(v)) for p in pts]
        return poly if area(poly) > 0 else poly[::-1]
    a, b = flat(f[1]), flat(g[1])
    for i in range(len(b)):
        a = clip_polygon(a, b[i], b[(i + 1) % len(b)])
        if len(a) < 3:
            return 0.0
    return abs(area(a))


def canon(n):
    """The normal with its first clear component positive, and the sign it took."""
    for c in n:
        if abs(c) > 1e-6:
            return (n, 1) if c > 0 else (-n, -1)
    return n, 1


def fights(faces):
    """Pairs of fighting faces: [(label a, label b, area px^2)]."""
    eps = EPS_PX * PX
    buckets = {}
    keyed = []
    for f in faces:
        n, s = canon(f[2])
        d = n.dot(f[1][0])
        key = (round(n.x, 3), round(n.y, 3), round(n.z, 3))
        k = math.floor(d / (10 * eps))
        keyed.append((f, n, s, d, key, k))
        buckets.setdefault((key, k), []).append(len(keyed) - 1)
    out = []
    seen = set()
    for i, (f, n, s, d, key, k) in enumerate(keyed):
        for kk in (k - 1, k, k + 1):
            for j in buckets.get((key, kk), ()):
                if j <= i:
                    continue
                g, n2, s2, d2, _, _ = keyed[j]
                if f[0] == g[0] or abs(d - d2) > eps or n.dot(n2) < 1 - 1e-7:
                    continue
                if any(abs(n.dot(p) - d) > eps for p in g[1]):
                    continue
                if s != s2 and min(f[3], g[3]) > eps:
                    continue  # two solid cubes face to face: inside the pair
                a = overlap(f, g) / (PX * PX)
                if a > AREA_PX2 and (f[0], g[0]) not in seen:
                    seen.add((f[0], g[0]))
                    out.append((f[0], g[0], a, '%s %s' % ('same way' if s == s2 else 'opposite',
                                                         plane_text(f[2], f[1][0]))))
    return out


def plane_text(n, p):
    """The plane of a face for the report, px in the model frame."""
    return 'n (%.2f %.2f %.2f) at (%.2f %.2f %.2f) px' % (tuple(n) + tuple(c / PX for c in p))


def label_of(part, i, cube):
    return '%s#%d(%s)' % (part, i, cube.mat)


def model_faces(model, pose):
    """Every face of the model's cubes and held element items for one pose: pose maps a part
    name to (pivot px, angles rad, scale or None), local to its parent."""
    import items
    world = {}
    for name, part in model.parts.items():
        m = part_matrix(*pose[name])
        world[name] = world[part.parent] @ m if part.parent else m
    faces = []
    for name, part in model.parts.items():
        for i, c in enumerate(part.cubes):
            if getattr(c, 'coplanar_ok', None):
                continue
            faces.extend(cube_faces(c, world[name], label_of(name, i, c)))
    for s in model.sockets.values():
        if s.item is None or not getattr(s.item, 'elements', None):
            continue
        sm = s.matrix()  # blocks, parent frame
        m = world[s.parent] @ sm @ items.held_matrix(s.item.display, s.hand)
        for i, el in enumerate(s.item.elements):
            faces.extend(element_faces(el, m, 'held:%s#%d(%s)' % (s.item.id, i, el.mat)))
    return faces


def rest_pose(model):
    return {n: (tuple(p.local_rest()), tuple(math.radians(a) for a in p.rot), None) for n, p in model.parts.items()}


def check_model(model, clips, samples):
    """Fights of a model: {(a, b): [first clip, first tick, ticks, area]}; 'rest' is the rest pose."""
    found = {}

    def note(where, tick, pose):
        for a, b, ar, how in fights(model_faces(model, pose)):
            e = found.setdefault((a, b), [where, tick, 0, ar, how])
            e[2] += 1
    note('rest', 0, rest_pose(model))
    for name, clip in clips.items():
        for t in range(clip.length + 1):
            pose = {}
            for p in model.parts:
                row = samples[name][p][t]
                pose[p] = (row[:3], row[3:6], row[6:9])
            note(name, t, pose)
    return found


def check_item(item):
    """Fights between the elements of an element item model, in item space."""
    faces = []
    for i, el in enumerate(getattr(item, 'elements', [])):
        faces.extend(element_faces(el, Matrix.Identity(4), '%s#%d(%s)' % (item.id, i, el.mat)))
    return {(a, b): ['item', 0, 1, ar, how] for a, b, ar, how in fights(faces)}


def report(owner, found, limit=40):
    lines = []
    for (a, b), (where, tick, n, ar, how) in sorted(found.items(), key=lambda kv: (kv[1][0] != 'rest', kv[0])):
        lines.append('%s: %s <-> %s, %.2f px^2, %s, first %s tick %d, %d tick(s)' % (owner, a, b, ar, how, where, tick, n))
    return lines[:limit] + (['%s: ... %d more' % (owner, len(lines) - limit)] if len(lines) > limit else [])
