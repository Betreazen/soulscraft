"""Shrike (soulscraft:shrike, Сорокопут): an undead bird of prey that circles and dives.

A dark-feathered shrike gone to bone: a long hooked beak of bare bone, a skull showing through the
crown, a black mask through a glowing eye, ribs through a torn flank, bare wrist bones on the
leading edge of ragged wings. Wingspan 24 px tip to tip, about 12 px tall (data/mobs.json).
Silhouette from below, where the player sees it: broad wings with stepped, torn trailing edges
and a pale bone patch on the primaries, a long tail, the beak hook in front.

Feathers are stepped thin cubes, as vanilla does it (phantom wing tips, parrot): a covert row
on top, then one-texel strips of different lengths for the secondaries and primaries, so the
ragged edge is real geometry. Everything is opaque; no texel is a hole.

Game model px throughout (see rig.py): +x is the bird's left, +y down, -z its front.
Rotation signs used below (degrees):
  body / head x > 0  pitches the nose down;  x < 0 rears up
  body z > 0         rolls to the left (left wing down)
  head / body y > 0  turns the front to the right
  jaw x > 0          opens the lower beak
  wing_r_* z > 0     raises the right wing; the left side mirrors (z < 0 raises it)
  wing_r_* y > 0     sweeps the right wing back; the left side mirrors
  wing_*   x < 0     twists the trailing edge down (x = -90 stands the wing on its leading edge)
  tail x > 0         raises the tail;  leg x > 0 swings the foot back

Flying base. The model is drawn by the mod itself (native_mob_submit, drawn = true), so it works
on either base of research 5 (a zombie-template type with Flying navigation, or a vanilla
flyer redrawn). Assumptions: the mod picks the clip from its own state (attack phase from the
server attachment, turning left or right for the banked circle clips), plays flight clips by
time and not by walk_position, and turns the model by body_yaw only. Bank and dive pitch are in
the clips, because the game's draw frame has no roll or pitch. On death the mod should drop
no_gravity so the body falls; the death clip only tumbles and lies down on the hitbox floor.
"""
from paint import Material
from rig import J, M, R, lerp, pose

ID = 'shrike'

PALETTES = {
    'feather': Material(['#15151b', '#1e1e26', '#282832', '#34343f', '#43434f'], noise=0.7, patch=2),
    'tip': ['#0b0b0e', '#121216', '#1a1a1f'],                       # dark feather ends
    'pale': Material(['#3e3c3a', '#535049', '#6a665d', '#7f7a6f'], noise=0.6, patch=2),  # dirty belly, underwing
    'bone': Material(['#6f6857', '#8e8571', '#ada38b', '#c9bea3', '#ded4ba'], noise=0.5, patch=2),
    'flesh': ['#241717', '#342120', '#472c2a', '#5a3835'],          # dried, under the torn feathers
    'mask': ['#060607', '#0d0d0f'],
}
EYE = '#9cf5e4'
BONE_PATCH = ('bone', 3)   # the pale patch on the primaries, the old shrike's white wing mirror


# ---------------------------------------------------------------------------- decals
# Face coordinates (paint.Face): top and bottom faces have row 0 at the back and the last row at
# the front, x to the bird's left. On a right-wing cube x = 0 is therefore the outer end.

def covert_decal(face):
    """The covert row of the inner wing: a lit leading edge, scalloped feather ends behind it."""
    w, h = face.w, face.h
    if face.name == 'top':
        face.rect(0, h - 1, w, 1, ('feather', 3))
        for x in range(w):
            face.px(x, 0, ('feather', 0) if x % 2 else ('tip', 2))
    elif face.name == 'bottom':
        face.rect(0, 0, w, h, ('pale', 1))
        face.rect(0, h - 1, w, 1, ('feather', 2))
        for x in range(0, w, 2):
            face.px(x, 0, ('pale', 0))
    elif face.name == 'front':
        face.rect(0, 0, w, h, ('feather', 3))


def strip_decal(face):
    """A one-texel feather strip running back (secondaries, tail): dark end, a lit shaft now and
    then, pale grey underneath."""
    w, h = face.w, face.h
    shade = 2 if (face.u // 2) % 2 else 1          # neighbouring strips differ by a step
    if face.name == 'top':
        face.rect(0, 0, w, h, ('feather', shade))
        face.rect(0, 0, w, 1, ('tip', 0))
        if h > 3:
            face.px(0, h - 2, ('feather', shade + 1))
    elif face.name == 'bottom':
        face.rect(0, 0, w, h, ('pale', 1 if shade == 2 else 0))
        face.rect(0, 0, w, 1, ('tip', 1))
    elif face.name == 'back':
        face.rect(0, 0, w, h, ('tip', 0))


def primary_decal(face):
    """A primary running out to the tip: dark end, the bone-white patch near its base."""
    w, h = face.w, face.h
    if face.name in ('top', 'bottom'):
        face.rect(0, 0, w, h, ('feather', 1) if face.name == 'top' else ('feather', 2))
        face.px(0, 0, ('tip', 0))
        if w > 3:
            face.px(1, 0, ('tip', 1))
            face.px(w - 2, 0, BONE_PATCH)
        if w > 4:
            face.px(w - 3, 0, ('bone', 2))
    elif face.name == 'right':
        face.rect(0, 0, w, h, ('tip', 0))


def trail_decal(face):
    """Short inner primaries running back from the hand."""
    strip_decal(face)
    if face.name in ('top', 'bottom') and face.h > 1:
        face.px(0, face.h - 1, BONE_PATCH)


def wingbone_decal(face):
    """Bare wrist and finger bone on the leading edge: knuckles, a crack."""
    if face.name in ('top', 'front', 'bottom'):
        face.px(0, 0, ('bone', 1))
        face.px(face.w - 1, 0, ('bone', 4))


def claw_decal(face):
    face.px(0, 0, ('bone', 1))


def torso_decal(face):
    w, h = face.w, face.h
    if face.name == 'top':          # 4 wide, 7 deep: dark mantle, scalloped rows
        for y in range(1, h, 2):
            for x in range(w):
                if (x + y) % 3 == 0:
                    face.px(x, y, ('tip', 2))
        face.rect(0, h - 1, w, 1, ('feather', 1))
    elif face.name == 'bottom':     # the belly: dirty pale, ribs where the feathers are gone
        face.rect(0, 0, w, h, ('pale', 1))
        face.patch(0, 1, w, 5, ('flesh', 1), 0.7, 3)
        for y in (2, 4):
            face.rect(0, y, w, 1, ('bone', 2))
        face.px(1, 3, ('bone', 1))
        face.rect(0, h - 1, w, 1, ('pale', 2))
    elif face.name == 'front':      # the breast under the chin
        face.rect(0, 1, w, h - 1, ('pale', 2))
        face.px(0, 1, ('feather', 2))
        face.px(w - 1, 1, ('feather', 2))
    elif face.name == 'right':      # x = 0 back, x = 6 front: a torn flank, ribs showing
        face.rect(0, 2, w, 2, ('pale', 1))
        face.patch(1, 1, 4, 3, ('flesh', 1), 0.75, 5)
        for x in (2, 4):
            face.rect(x, 1, 1, 3, ('bone', 2))
        face.px(3, 2, ('flesh', 0))
    elif face.name == 'left':       # x = 0 front, x = 6 back: feathers, a smaller bald patch
        face.rect(0, 2, w, 2, ('pale', 1))
        face.patch(3, 2, 2, 2, ('flesh', 2), 0.6, 7)
        face.px(3, 3, ('bone', 2))
    elif face.name == 'back':
        face.rect(0, 2, w, 2, ('pale', 0))


def vertebra_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.px(0, 0, ('bone', 3))


def head_decal(face):
    """4 x 4 x 4 head: bare skull on the crown, the black shrike mask through the eye."""
    w, h = face.w, face.h
    if face.name == 'top':          # row 0 back, row 3 front
        face.rect(0, 2, w, 2, ('bone', 3))
        face.px(1, 3, ('bone', 1))  # a crack in the skull
        face.px(1, 2, ('bone', 2))
        face.px(3, 1, ('bone', 2))
    elif face.name == 'front':
        face.rect(0, 0, w, 1, ('bone', 3))
        face.rect(0, 1, w, 1, ('mask', 0))
        face.rect(0, 2, w, 2, ('pale', 2))
        face.px(2, 3, ('pale', 1))
    elif face.name in ('left', 'right'):
        front = 0 if face.name == 'left' else w - 1
        step = 1 if face.name == 'left' else -1
        face.px(front, 0, ('bone', 3))
        face.px(front + step, 0, ('bone', 2))
        face.rect(0, 1, w, 1, ('mask', 0))          # the mask band, beak to nape
        face.px(front + step, 2, ('mask', 1))        # the socket under the eye
        face.rect(0, 3, w, 1, ('pale', 2))
        face.px(front, 2, ('pale', 2))
    elif face.name == 'bottom':
        face.rect(0, 0, w, h, ('pale', 2))
    elif face.name == 'back':
        face.rect(0, 1, w, 1, ('mask', 1))


def neck_decal(face):
    """The neck: dark nape, pale throat underneath."""
    if face.name in ('left', 'right'):
        face.rect(0, 2, face.w, 1, ('pale', 1))
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('pale', 1))


def eye_decal(face):
    face.rect(0, 0, face.w, face.h, EYE)


def tuft_decal(face):
    if face.name in ('top', 'back', 'left', 'right'):
        face.px(0, 0, ('tip', 0))


def beak_decal(face):
    """Old bone: lighter on top, a dark nostril, the hook yellowed and worn at the tip."""
    w, h = face.w, face.h
    if face.name in ('left', 'right') and w >= 2 and h >= 2:
        face.px(1 if face.name == 'left' else w - 2, 0, ('flesh', 0))   # nostril
        face.rect(0, h - 1, w, 1, ('bone', 1))
    elif face.name == 'top':
        face.rect(0, 0, w, h, ('bone', 4))


def hook_decal(face):
    if face.name in ('front', 'left', 'right', 'back'):
        face.px(0, face.h - 1, ('bone', 0))
        face.px(0, face.h - 2, ('bone', 1))


def jaw_decal(face):
    if face.name == 'top':          # the inside of the mouth shows when it opens
        face.rect(0, 0, face.w, face.h, ('flesh', 1))
        face.px(0, face.h - 1, ('flesh', 2))


def shank_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.px(0, 0, ('feather', 2))


def foot_decal(face):
    if face.name in ('front', 'left', 'right', 'back'):
        face.rect(0, 0, face.w, face.h, ('bone', 1))


def talon_decal(face):
    if face.name in ('front', 'left', 'right', 'back'):
        face.px(0, face.h - 1, ('tip', 0))


def tail_decal(face):
    """Tail strips: dark, the outer pair with a pale edge like a shrike's white outer feathers."""
    strip_decal(face)
    if face.name in ('top', 'bottom') and face.cube.origin[0] in (-2.5, 1.5):
        face.rect(0, 1, face.w, face.h - 2, ('pale', 2))


# ---------------------------------------------------------------------------- wings
# Right-wing cubes as (key, u0, u1, z0, z1, material, decal, y0): u runs outward from the pivot
# (x = -u on the right wing), z from the leading edge back, y0 the top of the 1 px plate.

INNER = [
    ('cov', 0, 5, -1, 2, 'feather', covert_decal, -0.75),   # coverts, a quarter pixel proud
    ('s0', 0, 1, 2, 7, 'feather', strip_decal, -0.5),       # secondaries: ragged lengths
    ('s1', 1, 2, 2, 6, 'feather', strip_decal, -0.5),
    ('s2', 2, 3, 2, 7, 'feather', strip_decal, -0.5),
    ('s3', 3, 4, 2, 5, 'feather', strip_decal, -0.5),       # torn
    ('s4', 4, 5, 2, 6, 'feather', strip_decal, -0.5),
]
OUTER = [
    ('bone', 0, 2, -1, 0, 'bone', wingbone_decal, -0.75),   # bare wrist on the leading edge
    ('claw', 0, 1, -2, -1, 'bone', claw_decal, -0.75),      # the wing claw sticking forward
    ('p0', 2, 5, -1, 0, 'feather', primary_decal, -0.5),    # primaries out to the tip
    ('p1', 0, 4, 0, 1, 'feather', primary_decal, -0.5),
    ('p2', 0, 5, 1, 2, 'feather', primary_decal, -0.5),
    ('p3', 0, 3, 2, 3, 'feather', primary_decal, -0.5),     # torn: the notch between the fingers
    ('p4', 0, 4, 3, 4, 'feather', primary_decal, -0.5),
    ('t0', 0, 1, 4, 6, 'feather', trail_decal, -0.5),       # inner primaries running back
    ('t1', 1, 2, 4, 5, 'feather', trail_decal, -0.5),
]
# The left wing is the right one mirrored, torn in other places.
LEFT_TORN = {'s3': (3, 4, 2, 7), 's4': (4, 5, 2, 4), 'p3': (0, 4, 2, 3), 'p4': (0, 2, 3, 4)}

SHOULDER = (2, 14.5, -2)   # wing pivot on the right (x negated) and left side of the back
INNER_SPAN = 5
OUTER_DROP = 0.1          # the outer plates sit 0.1 px lower and back: never in the inner plates' planes


def wings(m):
    """Both wings. The right one (outward = -x) is built from INNER and OUTER; the left one is
    its mirror (vanilla .mirror(), the same sheet region), except the strips torn elsewhere."""
    share = {'in': {}, 'out': {}}
    for side, s in ((-1, 'r'), (1, 'l')):
        sx = SHOULDER[0] * side
        inner = m.part('wing_%s_in' % s, (sx, SHOULDER[1], SHOULDER[2]), parent='body')
        outer = m.part('wing_%s_out' % s, (sx + INNER_SPAN * side, SHOULDER[1], SHOULDER[2]),
                       parent='wing_%s_in' % s)
        for part, cubes, store in ((inner, INNER, share['in']), (outer, OUTER, share['out'])):
            for key, u0, u1, z0, z1, mat, decal, y0 in cubes:
                dz = OUTER_DROP if part is outer else 0
                if part is outer:
                    y0 += OUTER_DROP
                if side > 0 and key in LEFT_TORN:
                    u0, u1, z0, z1 = LEFT_TORN[key]
                size = (u1 - u0, 1, z1 - z0)
                if side < 0:
                    store[key] = part.cube((-u1, y0, z0 + dz), size, mat, decal=decal)
                elif store[key].size == size:
                    part.cube((u0, y0, z0 + dz), size, mat, share=store[key], mirror=True)
                else:
                    part.cube((u0, y0, z0 + dz), size, mat, decal=decal, mirror=True)


# ---------------------------------------------------------------------------- model

def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 16, 0), parent='root')
    body.cube((-2, -2, -3.5), (4, 4, 7), 'feather', decal=torso_decal)
    body.cube((-0.5, -2.5, -1), (1, 1, 1), 'bone', decal=vertebra_decal)   # spine through the mantle
    body.cube((-0.5, -2.5, 1), (1, 1, 1), 'bone', decal=vertebra_decal)

    head = m.part('head', (0, 14.5, -3.5), parent='body')
    # The head is 0.1 px wider than the body on each side, so its sides never share the body's
    # planes when it pitches; the neck's front stops 0.1 px behind the body's front.
    head.cube((-2, -3.5, -4), (4, 4, 4), 'feather', inflate=(0.1, 0, 0), decal=head_decal)
    head.cube((-1.5, -2, 0), (3, 3, 2), 'feather', inflate=(0, 0, -0.1), decal=neck_decal)       # the neck, sunk into the mantle
    head.cube((-1, -4, -1), (2, 1, 3), 'feather', decal=tuft_decal)        # ragged crest at the nape
    head.cube((-0.5, -4.5, 0), (1, 1, 2), 'tip', inflate=(0, 0, 0.1), decal=tuft_decal)
    head.cube((2.12, -2.5, -3), (0, 1, 1), 'mask', glow=True, decal=eye_decal, faces=('left',))
    head.cube((-2.12, -2.5, -3), (0, 1, 1), 'mask', glow=True, decal=eye_decal, faces=('right',))

    beak = m.part('beak', (0, 13, -7.5), parent='head')
    beak.cube((-1, -1, -2), (2, 2, 2), 'bone', decal=beak_decal)
    beak.cube((-0.5, -1, -4), (1, 1, 2), 'bone', decal=beak_decal)
    beak.cube((-0.5, -1, -5), (1, 3, 1), 'bone', decal=hook_decal)        # the hook, curling down
    jaw = m.part('jaw', (0, 14, -7.5), parent='head')
    jaw.cube((-0.5, 0, -3), (1, 1, 3), 'bone', decal=jaw_decal)

    tail = m.part('tail', (0, 15, 3.5), parent='body')
    for x, length in ((-2.5, 4), (-1.5, 5), (-0.5, 6), (0.5, 5), (1.5, 3)):   # fanned, one broken
        tail.cube((x, -0.5, 0), (1, 1, length), 'feather', decal=tail_decal)

    for s, x in (('r', -1), ('l', 1)):
        leg = m.part('leg_' + s, (x, 17.5, 1.5), parent='body', rot=(84, 0, 0))   # tucked back
        if s == 'r':
            a = leg.cube((-0.5, 0, -0.5), (1, 3, 1), 'bone', decal=shank_decal)
            b = leg.cube((-1, 3, -1.5), (2, 1, 2), 'flesh', decal=foot_decal)
            c = leg.cube((-0.5, 3, -2.5), (1, 2, 1), 'bone', decal=talon_decal)
        else:
            leg.cube((-0.5, 0, -0.5), (1, 3, 1), 'bone', share=a, mirror=True)
            leg.cube((-1, 3, -1.5), (2, 1, 2), 'flesh', share=b, mirror=True)
            leg.cube((-0.5, 3, -2.5), (1, 2, 1), 'bone', share=c, mirror=True)

    wings(m)
    m.socket('socket_head', 'head', (0, -4, -2))


# ---------------------------------------------------------------------------- poses

def flip(j):
    """A joint pose seen in a mirror across the bird's midline."""
    return J((j.rot[0], -j.rot[1], -j.rot[2]), (-j.move[0], j.move[1], j.move[2]), j.scale)


def mirror(p):
    """A whole pose mirrored: left and right parts swap, yaw and roll change sign."""
    out = {}
    for k, j in p.items():
        other = k.replace('_r', '_L').replace('_l', '_r').replace('_L', '_l')
        out[other] = flip(j)
    return out


def wingpose(inner=R(), outer=R(), inner_l=None, outer_l=None):
    """Both wings from the right wing's angles; the left one mirrors unless given."""
    return {'wing_r_in': inner, 'wing_r_out': outer,
            'wing_l_in': flip(inner) if inner_l is None else flip(inner_l),
            'wing_l_out': flip(outer) if outer_l is None else flip(outer_l)}


# Gliding: wings spread with a little dihedral, tips flexed up, tail spread, a level body.
GLIDE = pose(wingpose(R(0, 0, 7), R(-4, 0, 6)), tail=R(2, 0, 0), head=R(4, 0, 0))

# Flap cycle: the hand lags the arm on the way down and folds back on the way up.
F_UP = pose(wingpose(R(0, -4, 52), R(0, 0, 14)), body=M(0, 0.6, 0), tail=R(4, 0, 0))
F_DOWN_MID = pose(wingpose(R(-6, 0, 4), R(-8, 0, 26)), body=M(0, 0, 0))
F_DOWN = pose(wingpose(R(-4, 4, -40), R(0, 0, -14)), body=M(0, -0.7, 0), tail=R(-3, 0, 0), head=R(2, 0, 0))
F_UP_MID = pose(wingpose(R(8, 16, -6), R(10, 34, -30)), body=M(0, -0.3, 0), tail=R(0, 0, 0))

# Hovering: the body reared up, the head held level, the legs let down, short strokes.
HOVER = dict(body=R(-38, 0, 0), head=R(32, 0, 0), tail=R(-16, 0, 0), leg_r=R(-55, 0, 6), leg_l=R(-55, 0, -6),
             jaw=R(4, 0, 0))
H_UP = pose(HOVER, wingpose(R(-20, -18, 58), R(-10, -10, 18)), root=M(0, 0.4, 0))
H_DOWN = pose(HOVER, wingpose(R(-30, 14, -34), R(-16, 0, -12)), root=M(0, -0.6, 0))

# Wings folded tight for the stoop: each wing stood on its leading edge and swept back along the
# flank, the hand laid along the arm, the tips crossing over the tail.
FOLD_WINGS = wingpose(R(-84, 78, -8), R(0, -16, 0))
FOLD = pose(FOLD_WINGS, body=R(58, 0, 0), head=R(-30, 0, 0) + M(0, 0, -0.5), tail=R(-4, 0, 0),
            leg_r=R(18, 0, 0), leg_l=R(18, 0, 0))

# The screech: braking in the air, reared up, head thrown back, beak gaping, wings flared high.
SCREECH = pose(wingpose(R(-30, -16, 46), R(-10, -8, 16)), body=R(-30, 0, 0) + M(0, -0.5, 0), head=R(-26, 0, 0),
               beak=R(-14, 0, 0), jaw=R(46, 0, 0), tail=R(-26, 0, 0), leg_r=R(-60, 0, 10), leg_l=R(-60, 0, -10))

# Banked circling to the left: rolled in, head turned into the turn and kept level, tail twisted.
CIRCLE = pose(GLIDE, body=R(2, -6, 28), head=R(0, -20, -18), tail=R(4, 10, 8),
              wing_r_in=R(0, 0, 4), wing_l_in=R(0, 0, 2))


def climb_keys(clip, t0, start):
    """The slow climb after the dive, 30 ticks from t0: three heavy, deep wingbeats just above
    the ground, the body nose-up and labouring, head low. start is the pose it leaves from."""
    labour = dict(body=R(-16, 0, 0), head=R(18, 0, 0), tail=R(-10, 0, 0), leg_r=R(-20, 0, 0), leg_l=R(-20, 0, 0))
    up = pose(labour, wingpose(R(-6, -6, 58), R(-4, 0, 22)), body=M(0, 0.9, 0))
    down = pose(labour, wingpose(R(-10, 6, -46), R(-6, 0, -22)), body=M(0, -1.1, 0), tail=R(-4, 0, 0))
    upmid = pose(labour, wingpose(R(8, 20, -4), R(10, 38, -34)), body=M(0, -0.2, 0))
    clip.key(t0, start, 'slow')
    for i, f in enumerate((4, 14, 24)):            # beats at +4..+13, +14..+23, +24..+30
        clip.key(t0 + f, up if i == 0 else pose(up, body=R(-3 * i, 0, 0)), 'inout')
        clip.key(t0 + f + 4, down, 'out')
        if t0 + f + 8 < t0 + 30:
            clip.key(t0 + f + 8, upmid, 'inout')
    clip.key(t0 + 30, F_UP, 'inout')


def attack_dive(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    # Windup, 14 ticks: brake and screech (0..8), then the wings fold tight (8..12), held, released.
    a.key(0, GLIDE, 'out')
    a.key(3, SCREECH, 'inout')
    a.key(5, pose(SCREECH, head=R(-6, 4, 0), jaw=R(4, 0, 0)), 'inout')     # the cry shakes the head
    a.key(7, pose(SCREECH, head=R(-4, -4, 0)), 'inout')
    a.key(8, SCREECH, 'out')
    a.key(11, pose(FOLD, body=R(-30, 0, 0), head=R(10, 0, 0)), 'inout')     # folded, still level
    a.key(hit - 2, FOLD, 'in')                                              # tipped into the stoop
    # Hit: the beak drives in, the talons come forward, the hands start to open.
    strike = pose(FOLD, wingpose(R(-50, 50, 0), R(0, -10, 0)), body=R(-18, 0, 0), head=R(16, 0, 0) + M(0, 0.5, -1.5),
                  jaw=R(18, 0, 0), leg_r=R(-120, 0, 8), leg_l=R(-120, 0, -8), tail=R(-20, 0, 0))
    a.key(hit, strike, 'out')
    # Recover: flared wide to pull out of the dive just above the ground, then the slow climb.
    flare = pose(wingpose(R(-30, -6, -10), R(-10, 0, 10)), body=R(-34, 0, 0) + M(0, 0.6, 0), head=R(26, 0, 0),
                 tail=R(-30, 0, 0), leg_r=R(-60, 0, 0), leg_l=R(-60, 0, 0))
    climb_keys(a, rec, flare)
    assert end == rec + 30


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}

    idle = c.clip('idle', 16, loop=True)            # hovering in place: two short beats
    idle.key(0, H_UP, 'inout')
    idle.key(4, H_DOWN, 'inout')
    idle.key(8, pose(H_UP, wingpose(R(0, 0, -6)), head=R(0, 6, 0)), 'inout')
    idle.key(12, pose(H_DOWN, head=R(0, 6, 0)), 'inout')
    idle.key(16, H_UP)

    fly = c.clip('fly', 12, loop=True)              # powered flight: one flap cycle
    fly.key(0, F_UP, 'inout')
    fly.key(3, F_DOWN_MID, 'inout')
    fly.key(5, F_DOWN, 'inout')
    fly.key(8, F_UP_MID, 'inout')
    fly.key(12, F_UP)

    glide = c.clip('glide', 40, loop=True)          # soaring straight, small corrections
    glide.key(0, GLIDE, 'inout')
    glide.key(10, pose(GLIDE, body=R(0, 0, 3), head=R(0, 10, 0), wing_r_out=R(-4, 0, 3)), 'inout')
    glide.key(20, pose(GLIDE, body=R(1, 0, 0) + M(0, 0.3, 0)), 'inout')
    glide.key(30, pose(GLIDE, body=R(0, 0, -3), head=R(0, -10, 0), wing_l_out=R(-4, 0, -3)), 'inout')
    glide.key(40, GLIDE)

    for name, base in (('circle', CIRCLE), ('circle_r', mirror(CIRCLE))):   # banked turn, left / right
        cl = c.clip(name, 40, loop=True)
        cl.key(0, base, 'inout')
        cl.key(10, pose(base, mirror(dict(head=R(4, -6, 0))) if name == 'circle_r' else dict(head=R(4, -6, 0)),
                        body=M(0, 0.2, 0)), 'inout')
        cl.key(20, pose(base, body=R(0, 0, 3 if name == 'circle' else -3)), 'inout')
        cl.key(30, pose(base, mirror(dict(head=R(-4, 6, 0))) if name == 'circle_r' else dict(head=R(-4, 6, 0))),
               'inout')
        cl.key(40, base)

    dive = c.clip('dive', 6, loop=True)             # the stoop between the windup and the strike
    dive.key(0, FOLD, 'inout')
    dive.key(3, pose(FOLD, wing_r_out=R(0, 3, 0), wing_l_out=R(0, -3, 0), tail=R(3, 0, 0), head=R(-2, 0, 0)), 'inout')
    dive.key(6, FOLD)

    climb = c.clip('climb', 30)                     # the pull-up alone, the punish window
    climb_keys(climb, 0, pose(GLIDE, body=R(-20, 0, 0)))

    attack_dive(c.attack('attack_dive', attacks['attack_dive']))

    hurt = c.clip('hurt', 10)                       # vanilla hurt_time
    jolt = pose(wingpose(R(-10, -10, 48), R(0, 0, -36)), body=R(-22, 0, 8) + M(0, -1, 1), head=R(-28, 10, 0),
                tail=R(-20, 0, 0), jaw=R(30, 0, 0), leg_r=R(-40, 0, 0), leg_l=R(-40, 0, 0))
    hurt.key(0, GLIDE, 'out')
    hurt.key(2, jolt, 'slow')
    hurt.key(6, pose(lerp(jolt, GLIDE, 0.6), wingpose(R(0, 0, -20))), 'inout')
    hurt.key(10, GLIDE)

    death = c.clip('death', 20)                     # vanilla death_time; tumbles and lies on its back
    limp = wingpose(R(-30, 10, 30), R(0, 20, 40), R(10, -20, -10), R(0, -10, 50))
    death.key(0, GLIDE, 'out')
    death.key(2, jolt, 'linear')
    death.key(6, pose(limp, body=R(70, 10, 50) + M(0, 0.5, 0), head=R(30, 20, 0), jaw=R(30, 0, 0), tail=R(-20, 0, 0)),
              'linear')
    death.key(11, pose(limp, body=R(170, 20, 110) + M(0, 2, 0), head=R(40, -20, 0), jaw=R(24, 0, 0)), 'linear')
    death.key(16, pose(limp, body=R(300, 30, 170) + M(0, 5, 0), head=R(30, 30, 0), jaw=R(20, 0, 0)), 'out')
    dead = pose(wingpose(R(0, -8, 26), R(0, 12, 10), R(-6, 4, 18), R(0, -6, 22)),
                body=R(360, 24, 180) + M(0, 6, 0), head=R(40, 30, 0), jaw=R(22, 0, 0), tail=R(10, 0, 0),
                leg_r=R(-90, 0, 10), leg_l=R(-80, 0, -14))
    death.key(18, pose(dead, body=M(0, -0.5, 0)), 'back')
    death.key(20, dead)

    trophy_clip(c)


# The trophy pose: landed on a dead branch, wings mantled high, talons gripping, screeching down
# at the viewer. A slow loop: the wings flex, the head jerks.
PERCH = pose(wingpose(R(-30, -16, 50), R(-10, -8, 18)), body=R(-12, 0, 0), head=R(16, 0, 0), beak=R(-14, 0, 0),
             jaw=R(46, 0, 0), tail=R(-12, 0, 0), leg_r=R(-78, 0, 6), leg_l=R(-78, 0, -6))


def trophy_clip(c):
    t = c.clip('trophy', 80, loop=True)
    t.key(0, PERCH)
    t.key(30, pose(PERCH, wingpose(R(4, 0, -6), R(0, 0, 4)), head=R(4, 8, 0), jaw=R(-14, 0, 0)))
    t.key(50, pose(PERCH, wingpose(R(2, 0, -4), R(0, 0, 2)), head=R(2, -6, 0), jaw=R(-8, 0, 0)))
    t.key(80, PERCH)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['head'], 'scale': 1.25},
}
