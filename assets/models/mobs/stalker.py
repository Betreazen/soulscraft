"""Stalker (soulscraft:stalker, Ловчий): a gaunt forest hunter with two sickles.

A pointed hide hood with the face sunk in shadow (two dim amber eyes), a fur mantle on the
shoulders, a ragged cloak of stitched hides down the back, long thin arms and a sickle in each
fist (socket_hand_r and socket_hand_l, vanilla handheld chain). 30 px from the feet to the top of
the hood: the body is hunched forward in its rest pose (part rotation), so every clip starts from
the stoop and the pointed hood and the two curved blades make the silhouette.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front.
Pose angles in degrees: limb x < 0 swings forward/up, body x > 0 leans forward, y > 0 turns the
front to the mob's right (left shoulder forward), right arm z > 0 lifts it out sideways, left
arm z < 0 lifts it out sideways.

Details are placed, not sprinkled: the stitched seams of the hood and the cloak, a strap across
the chest with a bone toggle, bindings on the forearms and shins, mud on the cloak hem and the
boots, a dried blood stain near the tip of each sickle. The hides carry only the patchy noise.
"""
from mathutils import Vector

from items import HANDHELD_PARENT, HELMET, Item, Sprite
from paint import Material
from rig import M, R, lerp, pose

ID = 'stalker'

PALETTES = {
    'hood': Material(['#1d2018', '#292d22', '#363b2c', '#444a37', '#525942'], noise=0.7, patch=3),
    'hide': Material(['#2a1f17', '#3b2c20', '#4d3a2a', '#604935', '#735a42'], noise=0.9, patch=4),
    'fur': Material(['#2f2a24', '#443d34', '#5a5144', '#716654', '#887c66'], noise=0.8, patch=2),
    'leather': ['#231a13', '#33261b', '#453424', '#57422e'],
    'cloth': ['#1b1b18', '#272622', '#34322d', '#423f38'],
    'wrap': Material(['#433d32', '#514a3d', '#5f5748', '#6d6453'], noise=0.4, patch=3),
    'skin': ['#3e3a33', '#544e44', '#6a6355', '#7f7766'],
    'shadow': Material(['#080807', '#0e0d0b', '#141210'], noise=0.4, patch=2),
    'bone': ['#8a8270', '#a89f89', '#c4bba3', '#ddd5bd'],
    'mud': ['#1f1a14', '#2b241b', '#383023'],
    'blood': ['#3a1212', '#511a17', '#6a221d'],
    'blade': ['#3e4142', '#5a5e5f', '#7c8182', '#a0a5a4', '#c6cac6'],
    'wood': Material(['#33251a', '#4a3524', '#62472f', '#795a3b'], noise=0.4, patch=3),
}
EYE = '#e9b85a'
EYE_CORE = '#fff0b8'
MUD = ('mud', 1)
BLOOD = ('blood', 1)


# ---------------------------------------------------------------------------- decals

def stitches(face, x0, y0, x1, y1, step=2):
    """A seam: a dark line with lighter stitches across it every `step` texels."""
    face.line(x0, y0, x1, y1, ('hood', 0) if face.cube and face.cube.mat == 'hood' else ('hide', 0))
    n = max(abs(x1 - x0), abs(y1 - y0))
    for i in range(0, n + 1, step):
        x = x0 + (x1 - x0) * i // max(n, 1)
        y = y0 + (y1 - y0) * i // max(n, 1)
        face.px(x, y, ('wrap', 1))


def hood_top_decal(face):
    """The crown of the hood: a seam from front to back, the front edge worn lighter."""
    if face.name == 'top':
        stitches(face, face.w // 2, 0, face.w // 2, face.h - 1)
    if face.name == 'back':
        stitches(face, face.w // 2, 0, face.w // 2, face.h - 1)
        face.patch(1, face.h - 2, face.w - 2, 3, ('hood', 0), 0.35, 3)


def hood_back_decal(face):
    if face.name == 'back':
        stitches(face, face.w // 2, 0, face.w // 2, face.h - 1, 3)
        face.patch(0, face.h - 2, face.w, 3, ('hood', 0), 0.4, 5)   # darker where it rubs the cloak


def hood_side_decal(face):
    if face.name in ('left', 'right'):
        x = face.w // 2
        face.line(x, 1, x - 1, face.h - 2, ('hood', 1))        # a fold in the hide: a crease...
        face.line(x + 1, 1, x, face.h - 2, ('hood', -2))       # ...and its lit ridge
        face.patch(0, face.h - 2, face.w, 3, ('hood', 0), 0.4, 7)


def brim_decal(face):
    """The overhanging lip of the hood: a lit worn edge along the bottom, the underside black."""
    if face.name == 'front':
        face.rect(0, face.h - 1, face.w, 1, ('hood', -1))
        face.px(face.w // 2, 0, ('hood', 0))
        face.px(face.w // 2 - 1, 0, ('hood', 0))
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('shadow', 2))


def flap_decal(face):
    """The cheek flaps of the hood: lit outer edge, the inner side in shadow."""
    if face.name == 'front':
        face.rect(0, 0, face.w, face.h, ('hood', 2))
        face.px(0, face.h - 1, ('hood', 1))
        face.px(face.w - 1, face.h - 1, ('hood', 1))
    elif face.name in ('left', 'right'):
        face.rect(0, face.h - 2, face.w, 2, ('hood', 0))


def peak_decal(face):
    if face.name in ('top', 'back'):
        stitches(face, face.w // 2, 0, face.w // 2, face.h - 1)


def face_decal(face):
    """The face inside the hood: shadow, a gaunt chin barely catching light at the bottom."""
    face.rect(0, 0, face.w, face.h, ('shadow', 0))
    if face.name == 'front':
        face.rect(1, face.h - 2, face.w - 2, 1, ('shadow', 2))
        face.rect(2, face.h - 1, face.w - 4, 1, ('skin', 0))


def eyes_decal(face):
    face.rect(0, 0, face.w, face.h, ('shadow', 0))
    face.px(1, 0, EYE)
    face.px(4, 0, EYE)


def mantle_decal(face):
    """Fur over the shoulders: lighter on top, a ragged fringe of tufts along the lower edge."""
    if face.name in ('front', 'back', 'left', 'right'):
        for x in range(face.w):
            if x % 3 == 1:
                face.px(x, face.h - 1, ('fur', 0))
            elif x % 3 == 2:
                face.px(x, face.h - 1, ('fur', 3))
        if face.name == 'front':
            face.rect(face.w // 2 - 1, 0, 2, face.h, ('fur', 1))  # parted where the hood sits
            face.px(face.w // 2, face.h - 1, ('bone', 1))           # a bone clasp
    elif face.name == 'top':
        face.patch(1, 1, face.w - 2, face.h - 2, ('fur', 3), 0.3, 11)


def torso_decal(face):
    """Leather jerkin: a strap across the chest from the right shoulder, a bone toggle on it."""
    if face.name == 'front':
        w, h = face.w, face.h
        for i in range(w):
            y = i * (h - 3) // (w - 1)
            face.px(i, y, ('leather', 3))
            face.px(i, y + 1, ('leather', 1))
        face.px(4, 3, ('bone', 2))
        face.px(4, 4, ('bone', 0))
        face.rect(3, h - 4, 1, 3, ('leather', 0))   # lacing down the belly
        face.px(3, h - 3, ('wrap', 1))
    elif face.name == 'back':
        face.patch(1, 1, face.w - 2, 4, ('leather', 0), 0.35, 13)


def belt_decal(face):
    if face.name == 'front':
        face.rect(3, 0, 2, 1, ('bone', 1))
        face.px(3, 0, ('bone', 0))
    elif face.name in ('left', 'right', 'back'):
        face.px(1, 0, ('leather', 3))


def pouch_decal(face):
    if face.name == 'front':
        face.rect(0, 0, face.w, 1, ('leather', 3))
        face.px(face.w // 2, 1, ('bone', 1))
    elif face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('leather', 3))


def cloak_decal(face):
    """The skirt of hides behind the legs: a seam across, a darker patched hide, mud at the hem."""
    if face.name == 'back':
        stitches(face, 0, 1, face.w - 1, 1)
        face.patch(5, 2, 3, 3, ('hide', 0), 0.55, 17)
        face.patch(0, face.h - 2, face.w, 3, MUD, 0.35, 19)
    elif face.name == 'front':
        face.rect(0, 0, face.w, face.h, ('hide', 0))   # the flesh side of the hide, in shadow
        face.patch(1, 1, face.w - 2, face.h - 2, ('hide', 1), 0.4, 23)


def cape_decal(face):
    """The cape on the back: the stitched seam where two hides meet, a darker patched hide."""
    if face.name == 'back':
        stitches(face, 0, 3, face.w - 1, 3)
        face.patch(4, 4, 4, 4, ('hide', 0), 0.55, 41)
        stitches(face, 4, 5, 7, 5, 3)


def tatter_decal(face):
    """The hanging strips of the hem: darker, muddy at the ends."""
    if face.name in ('back', 'front', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, MUD)
        if face.h > 2:
            face.px(0, face.h - 2, ('mud', 2))


def arm_decal(face):
    """Leather sleeve, wrapped forearm, a bony grey hand at the bottom."""
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 2, face.w, 2, ('skin', 1))     # the fist
        face.px(1, face.h - 1, ('skin', 0))
        face.rect(0, face.h - 3, face.w, 1, ('skin', 0))     # the wrist in the shadow of the wraps
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('skin', 0))


def wrap_decal(face):
    """Bandage wraps on the forearm: diagonal turns of the binding."""
    if face.name in ('front', 'back', 'left', 'right'):
        for y in range(0, face.h, 3):
            face.line(0, y + 1, face.w - 1, y, ('wrap', 0))
        face.px(0, face.h - 1, ('wrap', 3))


def shoulder_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('fur', 0))
        face.px(1, face.h - 1, ('fur', 3))
    elif face.name == 'top':
        face.patch(0, 0, face.w, face.h, ('fur', 3), 0.4, 29)


def leg_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('leather', 0))   # boot sole
        face.patch(0, face.h - 4, face.w, 3, MUD, 0.45, 31)    # mud on the boots
        face.rect(0, 0, face.w, 1, ('leather', 2))             # under the belt


def shin_decal(face):
    """Cross-bound shin wraps."""
    if face.name in ('front', 'back', 'left', 'right'):
        for y in (1, 3):
            face.rect(0, y, face.w, 1, ('leather', 1))     # two leather straps over the wraps


def toe_decal(face):
    if face.name in ('front', 'left', 'right', 'top'):
        face.patch(0, 0, face.w, face.h, MUD, 0.4, 37)


# ---------------------------------------------------------------------------- items

OUTLINE = '#151412'


def in_grip(x, y):
    """The grip texels of the sickle: a two-texel band from the butt up to the ferrule; it
    covers item px (4, 4), the corner of texels x 3-4, rows 11-12, where the fist closes."""
    return 14 <= x + y <= 15 and 1 <= x <= 6


# 16 x 16 hunter's sickle, laid out like a vanilla sword: the grip on the diagonal from the
# bottom left, an iron ferrule, then the blade rising and hooking over to the right in a
# crescent with its point turned down. o outline, b blade, e the lit cutting edge on the inside
# of the crescent, s the dark spine, f ferrule, g grip.
SICKLE_ROWS = [
    '................',
    '.........oooo...',
    '.......oosbbsoo.',
    '......osbbeeebbo',
    '.....osbeooooebo',
    '.....obeo....obo',
    '.....obeo....oeo',
    '.....offo.....oo',
    '......gfo.......',
    '.....gg.........',
    '....gg..........',
    '...gg...........',
    '..gg............',
    '.gg.............',
    '.g..............',
    '................',
]
SICKLE_KEY = {'o': OUTLINE, 'b': ('blade', 2), 'e': ('blade', 4), 's': ('blade', 1), 'f': ('blade', 0)}


def sickle_sprite(c):
    for y in range(16):
        for x in range(16):
            if in_grip(x, y):
                c.px(x, y, ('wood', 2) if x + y == 14 else ('wood', 1))
    for x, y in ((2, 13), (4, 11)):                    # two turns of a leather wrap on the grip
        c.px(x, y, ('leather', 2))
    c.px(1, 14, ('wood', 0))                           # butt of the grip
    c.px(14, 5, BLOOD)                                 # dried blood near the point
    c.px(13, 3, ('blood', 2))


SICKLE = Sprite('stalker_sickle', sickle_sprite, rows=SICKLE_ROWS, key=SICKLE_KEY, parent=HANDHELD_PARENT,
                palettes=PALETTES, name='Серп ловчего')


def hood_cubes(cube):
    """The pointed hood as head-local cubes: an open shell around a vanilla 8 x 8 x 8 head, an
    overhanging lip and cheek flaps around the opening, and a stepped peak rising up and back.
    The mob wears these; the dropped hood is made of the same cubes, so it fits a player's head
    with the player's face in its shadow."""
    t = 0.25  # the shell walls are half a pixel thick: one texel, inflated by -0.25 across
    return [
        cube((-4, -8.75, -4), (8, 1, 8), 'hood', inflate=(0, -t, 0.25), decal=hood_top_decal),
        # The back stops 0.1 px inside the side walls' outer faces and top, so they share no plane.
        cube((-4, -8, 3.75), (8, 8, 1), 'hood', inflate=(0.4, -0.1, -t), decal=hood_back_decal),
        # The side walls stop 0.1 px short of the back's outer face (one plane otherwise).
        cube((-4.75, -8, -4), (1, 8, 8), 'hood', inflate=(-t, 0, 0.4), decal=hood_side_decal),
        cube((3.75, -8, -4), (1, 8, 8), 'hood', inflate=(-t, 0, 0.4), decal=hood_side_decal),
        cube((-4.5, -8.5, -6), (9, 2, 2), 'hood', inflate=(0.05, 0.05, 0), decal=brim_decal),
        # The flaps sit 0.1 px proud of the side walls and the brim's front (z-fighting).
        cube((-4.5, -6.5, -6), (2, 7, 2), 'hood', inflate=0.1, decal=flap_decal),
        cube((2.5, -6.5, -6), (2, 7, 2), 'hood', inflate=0.1, decal=flap_decal),
        cube((-3.5, -9.5, -3.5), (7, 1, 7), 'hood', decal=peak_decal),
        cube((-2.5, -10.5, -2), (5, 1, 6), 'hood', decal=peak_decal),
        cube((-1.5, -11.5, -0.5), (3, 1, 5), 'hood', decal=peak_decal),
        cube((-1, -12.5, 1.5), (2, 1, 4), 'hood', decal=peak_decal),
        cube((-0.5, -13.5, 3.5), (1, 1, 3), 'hood', decal=peak_decal),
    ]


def stalker_hood():
    from rig import Cube
    it = Item('stalker_hood', PALETTES, HELMET, texture=64, name='Капюшон ловчего')
    it.from_head_cubes(hood_cubes(lambda o, s, mat, **kw: Cube(o, s, mat, **kw)))
    return it


ITEMS = [stalker_hood()]
SPRITES = [SICKLE]
ARMOR = []


# ---------------------------------------------------------------------------- model

HUNCH = 24      # rest lean of the body, degrees
ARM_LEN = 15    # px from the shoulder pivot region to the bottom of the fist


def at(m, parent, local):
    """Absolute rest position of a point given in a parent part's frame (child pivots of a
    tilted part)."""
    r, p = m.parts[parent].world_rest()
    v = p + r @ Vector(local)
    return tuple(round(c, 4) for c in v)


def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 12, 0), parent='root', rot=(HUNCH, 0, 0))
    body.cube((-3.5, -10, -2), (7, 10, 4), 'leather', decal=torso_decal)
    body.cube((-3.5, -2, -2), (7, 1, 4), 'leather', inflate=0.3, decal=belt_decal)
    body.cube((-4.5, -2.5, -2.5), (2, 3, 2), 'leather', decal=pouch_decal)          # pouch on the right hip
    body.cube((-4.5, -10.5, -2.5), (9, 3, 5), 'fur', inflate=0.2, decal=mantle_decal)

    head = m.part('head', at(m, 'body', (0, -8.5, -1.5)), parent='body', rot=(-HUNCH + 14, 0, 0))
    hood_cubes(head.cube)
    head.cube((-2.5, -6.5, -4), (5, 7, 4), 'shadow', decal=face_decal)
    head.cube((-3, -4.5, -4.1), (6, 1, 0), 'shadow', glow=True, decal=eyes_decal, faces=('front',))

    # Cloak of hides: a cape lying on the stooped back, and a ragged skirt of hides hanging from
    # the waist behind the legs (its own part, so it trails in the dash and the run).
    body.cube((-4, -9, 2), (8, 8, 1), 'hide', inflate=(0, 0, 0.15), decal=cape_decal)
    cloak = m.part('cloak', at(m, 'body', (0, -1.5, 3.0)), parent='body', rot=(-HUNCH + 2, 0, 0))
    cloak.cube((-4.5, 0, 0), (9, 6, 1), 'hide', decal=cloak_decal)
    for x, w, h in ((-4.5, 2, 4), (-2.5, 2, 2), (-0.5, 2, 5), (1.5, 2, 3), (3.5, 1, 4)):
        cloak.cube((x, 6, 0), (w, h, 1), 'hide', decal=tatter_decal)

    # Long thin arms hanging a little in front of the stoop; the fists reach the knees.
    arm_r = m.part('arm_r', at(m, 'body', (-5, -8.5, 0)), parent='body', rot=(-HUNCH - 6, 0, 0))
    a = arm_r.cube((-2.5, -1.5, -1.5), (3, ARM_LEN, 3), 'leather', decal=arm_decal)
    b = arm_r.cube((-2.5, 7, -1.5), (3, 5, 3), 'wrap', inflate=0.3, decal=wrap_decal)
    s = arm_r.cube((-3, -2, -2), (4, 3, 4), 'fur', inflate=(0.1, 0, 0), decal=shoulder_decal)   # off the hood's side
    arm_l = m.part('arm_l', at(m, 'body', (5, -8.5, 0)), parent='body', rot=(-HUNCH - 6, 0, 0))
    arm_l.cube((-0.5, -1.5, -1.5), (3, ARM_LEN, 3), 'leather', share=a, mirror=True)
    arm_l.cube((-0.5, 7, -1.5), (3, 5, 3), 'wrap', inflate=0.3, share=b, mirror=True)
    arm_l.cube((-1, -2, -2), (4, 3, 4), 'fur', inflate=(0.1, 0, 0), share=s, mirror=True)

    leg_r = m.part('leg_r', (-1.7, 12, 0.5), parent='root')
    e = leg_r.cube((-1.5, 0, -1.5), (3, 12, 3), 'cloth', decal=leg_decal)
    f = leg_r.cube((-1.5, 5, -1.5), (3, 5, 3), 'wrap', inflate=0.25, decal=shin_decal)
    g = leg_r.cube((-1.5, 10, -2.5), (3, 2, 1), 'leather', decal=toe_decal)
    leg_l = m.part('leg_l', (1.7, 12, 0.5), parent='root')
    leg_l.cube((-1.5, 0, -1.5), (3, 12, 3), 'cloth', share=e, mirror=True)
    # 0.1 px deeper than the right wrap: at rest the two overlap at the middle in one plane.
    leg_l.cube((-1.5, 5, -1.5), (3, 5, 3), 'wrap', inflate=(0.25, 0.35, 0.35), share=f, mirror=True)
    leg_l.cube((-1.5, 10, -2.5), (3, 2, 1), 'leather', share=g, mirror=True)

    # The fist: centre of the bottom of the hand on its front face (rig.SOCKETS for a custom arm).
    m.socket('socket_hand_r', offset=(-1, ARM_LEN - 1.5, -1.5))
    m.socket('socket_hand_l', offset=(1, ARM_LEN - 1.5, -1.5))
    m.socket('socket_head', 'head', (0, -9.5, 0))
    m.socket('socket_back', 'body', (0, -6, 2.5))
    m.hold('socket_hand_r', SICKLE)
    m.hold('socket_hand_l', SICKLE)


# ---------------------------------------------------------------------------- poses

# Prowling stance: knees apart, the weight low and forward, both sickles held low in front, the
# head up under the hood watching. BASE is the stance with the blades along the arms' swing;
# STANCE rolls the hanging forearms outward (y on a hanging arm is a roll about its length), so
# the crescents turn out to the sides and read from the front too. Attack poses build on BASE,
# their first and last keys are STANCE, so every clip starts and ends on the same pose.
BASE = dict(
    body=R(4, 0, 0),
    head=R(-6, 0, 0),
    arm_r=R(-18, 0, 4),
    arm_l=R(-18, 0, -4),
    leg_r=R(-10, 0, 5),
    leg_l=R(10, 0, -5),
    cloak=R(4, 0, 0),
)
ROLL = 28
STANCE = pose(BASE, arm_r=R(0, ROLL, 0), arm_l=R(0, -ROLL, 0))


def attack_double(a):
    """Twin slash: both sickles swing out wide to the sides (the tell), then the right one cuts
    across at the hit marker, the left one three ticks later (strikes_at 0 and 3), and the arms
    end crossed low in front, open for the 8-tick window."""
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    spread = pose(BASE, body=R(-10, 0, 0) + M(0, 0, 1), head=R(6, 0, 0), arm_r=R(24, 0, 112), arm_l=R(24, 0, -112),
                  leg_r=R(-6, 0, 0), leg_l=R(6, 0, 0), cloak=R(-6, 0, 0))
    cut_r = pose(BASE, body=R(14, -36, 0) + M(0, 0.5, -1), head=R(-10, 30, 0), arm_r=R(-80, -55, 70),
                 arm_l=R(30, 0, -100), leg_r=R(-24, 0, 0), leg_l=R(16, 0, 0), cloak=R(14, 0, 0))
    cut_l = pose(BASE, body=R(18, 34, 0) + M(0, 0.8, -1.5), head=R(-12, -28, 0), arm_r=R(-40, -60, 0),
                 arm_l=R(-80, 55, -70), leg_r=R(-24, 0, 0), leg_l=R(18, 0, 0), cloak=R(18, 0, 0))
    crossed = pose(BASE, body=R(26, 8, 0) + M(0, 1, -1.5), head=R(-16, -6, 0), arm_r=R(-30, -50, 40),
                   arm_l=R(-28, 52, -40), leg_r=R(-24, 0, 0), leg_l=R(18, 0, 0), cloak=R(16, 0, 0))
    a.key(0, STANCE, 'out')
    a.key(hit - 2, spread, 'in')       # arms flung wide: the tell
    a.key(hit, cut_r, 'out')           # strike 1: the right sickle across
    a.key(hit + 1, pose(cut_r, arm_r=R(4, -8, 0), arm_l=R(6, 0, -6)), 'in')
    a.key(hit + 3, cut_l, 'out')       # strike 2: the left sickle across
    a.key(rec, crossed, 'hold')        # overextended, arms crossed: the window
    a.key(rec + 4, pose(crossed, body=R(-2, 0, 0)), 'slow')
    a.key(end, STANCE)


def attack_lunge(a):
    """Lunge-dash: a long crouch with both sickles drawn back (12 ticks), the launch and the
    flight (6 ticks, the server moves it up to 6 blocks), then it lands overbalanced with the
    blades in the ground, stuck for 15 ticks, and twists round to find the player again."""
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    crouch = pose(BASE, root=M(0, 2.5, 1), body=R(22, 0, 0), head=R(-26, 0, 0), arm_r=R(70, -10, 30),
                  arm_l=R(70, 10, -30), leg_r=R(-42, 0, 6), leg_l=R(24, 0, -6), cloak=R(-10, 0, 0))
    coil = pose(crouch, root=M(0, 0.5, 0.5), body=R(4, 0, 0), arm_r=R(12, 0, 4), arm_l=R(12, 0, -4), head=R(-4, 0, 0))
    launch = pose(BASE, root=M(0, -1, -2), body=R(40, 0, 0), head=R(-40, 0, 0), arm_r=R(-120, 14, 18),
                  arm_l=R(-120, -14, -18), leg_r=R(-30, 0, 0), leg_l=R(50, 0, 0), cloak=R(26, 0, 0))
    flight = pose(launch, arm_r=R(30, 0, 0), arm_l=R(30, 0, 0), leg_r=R(10, 0, 0), cloak=R(6, 0, 0))
    stuck = pose(BASE, root=M(0, 3, -1), body=R(44, 0, 0), head=R(-30, 0, 0), arm_r=R(-58, 4, 8),
                 arm_l=R(-58, -4, -8), leg_r=R(-62, 0, 6), leg_l=R(34, 0, -6), cloak=R(30, 0, 0))
    twist = pose(stuck, root=M(0, -1, 0), body=R(-14, 40, 0), head=R(6, 30, 0), arm_r=R(30, 0, 10),
                 arm_l=R(10, 0, -16), cloak=R(-10, 0, 0))
    a.key(0, STANCE, 'out')
    a.key(hit - 4, crouch, 'inout')    # down into the crouch, sickles back: the tell
    a.key(hit - 2, coil, 'snap')       # coiled; then the release
    a.key(hit, launch, 'out')
    a.key(hit + 3, flight, 'inout')
    a.key(rec - 1, pose(flight, arm_r=R(20, 0, 0), arm_l=R(20, 0, 0)), 'in')
    a.key(rec, stuck, 'hold')          # lands, the blades bite the ground: stuck
    a.key(rec + 8, pose(stuck, root=M(0, -0.5, 0), arm_r=R(10, 0, 0), arm_l=R(10, 0, 0)), 'slow')
    a.key(rec + 12, twist, 'slow')     # twists round to find the player
    a.key(end, STANCE)


def walk_clip(c):
    """Prowl: low, long strides, the sickles swinging a little against the legs."""
    walk = c.clip('walk', 20, loop=True)
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        bob = M(0, 0.6 if s == 0 else 0, 0)
        walk.key(f, pose(STANCE, leg_l=R(-30 * s, 0, 0), leg_r=R(30 * s, 0, 0), arm_r=R(-14 * s, 0, 0),
                         arm_l=R(14 * s, 0, 0), body=bob + R(0, 4 * s, 0), head=R(0, -4 * s, 0),
                         cloak=R(4 + 3 * abs(s), 0, 0)))


def run_clip(c):
    """Flight at a sprint: leaning hard, the sickles pumping, the cloak streaming behind."""
    run = c.clip('run', 12, loop=True)
    lean = dict(body=R(14, 0, 0), head=R(-16, 0, 0), cloak=R(30, 0, 0))
    for f, s in ((0, 1), (3, 0), (6, -1), (9, 0), (12, 1)):
        run.key(f, pose(STANCE, lean, leg_l=R(-50 * s, 0, 0), leg_r=R(50 * s, 0, 0), arm_r=R(40 * s, 0, 0),
                        arm_l=R(-40 * s, 0, 0), body=M(0, -0.8 if s == 0 else 0, 0) + R(0, 6 * s, 0),
                        cloak=R(6 if s == 0 else 0, 0, 0)))


def strafe_clip(c):
    """Circling at 4-6 blocks: crab steps to its right, facing the player, sickles ready."""
    ready = pose(STANCE, body=R(6, 0, 0), arm_r=R(-14, 0, 14), arm_l=R(-30, 0, -6), head=R(-4, 0, 0))
    strafe = c.clip('circle_strafe', 20, loop=True)
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        strafe.key(f, pose(ready, leg_l=R(-10 * s, 0, -12 * s), leg_r=R(10 * s, 0, -12 * s),
                           body=M(0, 0.5 if s == 0 else 0, 0) + R(0, 0, 3 * s), cloak=R(2 * abs(s), 0, 3 * s)))


def backstep_clip(c):
    """The hop back after a hit: a short crouch, a leap back, a crouched landing."""
    back = c.clip('backstep', 10)
    dip = pose(STANCE, root=M(0, 1.5, 0), body=R(10, 0, 0), leg_r=R(-20, 0, 0), leg_l=R(16, 0, 0))
    hop = pose(STANCE, root=M(0, -3, 0), body=R(-6, 0, 0), head=R(6, 0, 0), arm_r=R(-30, 0, 30),
               arm_l=R(-30, 0, -30), leg_r=R(-30, 0, 0), leg_l=R(-10, 0, 0), cloak=R(-20, 0, 0))
    land = pose(STANCE, root=M(0, 2, 0), body=R(14, 0, 0), head=R(-10, 0, 0), leg_r=R(-26, 0, 0), leg_l=R(20, 0, 0),
                cloak=R(14, 0, 0))
    back.key(0, STANCE, 'out')
    back.key(2, dip, 'out')
    back.key(5, hop, 'in')
    back.key(7, land, 'slow')
    back.key(10, STANCE)


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}

    idle = c.clip('idle', 40, loop=True)
    idle.key(0, STANCE)
    idle.key(14, pose(STANCE, body=R(2, 0, 0) + M(0, 0.3, 0), head=R(-2, 10, 0), arm_r=R(-3, 0, 2), arm_l=R(-4, 0, -2)))
    idle.key(28, pose(STANCE, body=R(1, 0, 0), head=R(-4, -8, 0), arm_r=R(-2, 0, 0), arm_l=R(-2, 0, 0)))
    idle.key(40, STANCE)

    walk_clip(c)
    run_clip(c)
    strafe_clip(c)
    backstep_clip(c)
    attack_double(c.attack('attack_double', attacks['attack_double']))
    attack_lunge(c.attack('attack_lunge', attacks['attack_lunge']))

    hurt = c.clip('hurt', 10)   # vanilla hurt_time is 10 ticks
    hurt.key(0, STANCE, 'out')
    hurt.key(2, pose(STANCE, body=R(-18, -8, 4) + M(0, 0, 1), head=R(-18, 10, 0), arm_r=R(-20, 0, 30),
                     arm_l=R(-10, 0, -24), cloak=R(-10, 0, 0), root=M(0, 0, 1)), 'slow')
    hurt.key(10, STANCE)

    death = c.clip('death', 20)  # vanilla death_time is 20 ticks
    sag = pose(STANCE, body=R(30, 10, 0), head=R(26, -10, 0), arm_r=R(20, 0, 24), arm_l=R(30, 0, -30),
               leg_l=R(-10, 0, 0), leg_r=R(6, 0, 0), cloak=R(10, 0, 0))
    death.key(0, STANCE, 'out')
    death.key(5, sag, 'in')
    death.key(18, pose(sag, root=R(0, 0, -88)), 'back')
    death.key(20, pose(sag, root=R(0, 0, -90)))

    trophy_clip(c)


# The trophy pose: crouched mid-lunge on coiled legs, the left sickle thrust out at the prey, the
# right drawn back for the second cut, the hood up. A slow loop: the coil tightens and eases.
TROPHY_POSE = pose(BASE, root=M(0, 2.5, 0.5), body=R(24, -16, 0), head=R(-34, 14, 0), arm_r=R(60, -10, 34),
                   arm_l=R(-70, -16, -24), leg_r=R(-46, 0, 6), leg_l=R(30, 0, -6), cloak=R(-14, 0, 0))


def trophy_clip(c):
    t = c.clip('trophy', 80, loop=True)
    t.key(0, TROPHY_POSE)
    t.key(40, pose(TROPHY_POSE, root=M(0, 0.3, 0), body=R(2, 0, 0), head=R(-2, -4, 0), arm_l=R(-3, 0, 0),
                   arm_r=R(3, 0, 0), cloak=R(3, 0, 0)))
    t.key(80, TROPHY_POSE)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['head'], 'rot': (0, 0, 0), 'scale': 0.75},
}
