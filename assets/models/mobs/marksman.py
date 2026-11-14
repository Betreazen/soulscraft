"""Marksman (soulscraft:marksman, Костяной арбалетчик): a skeleton crossbowman in a ragged cloak.

Vanilla skeleton proportions (head 8, a body box 8 x 12 x 4 made of a ribcage, 2 x 2 limbs), 32 px
tall, the hood adds two. The limbs are split at the elbow and the knee (upper 7 / lower 5, thigh 6
/ shin 6) so the two hands can sit on the crossbow and the knees can bend into a crouch; at rest
they line up into the vanilla 2 x 12 x 2 bones. A hood and a shoulder mantle, a ragged cape down
the back, a quiver of bolts slung over it (mouth above the right shoulder), and a heavy crossbow.

The crossbow is a part of the model, not an item: item models that depend on the owner (crossbow
charge) do not animate in a mod mob's hand (research 3.2). Its moving bits are parts too: the two
prod limbs flex back when spanned, the string is two halves hinged at the limb tips that fold back
to the nut, a cranequin crank on the right side of the butt turns while the string is wound back,
the bolt flies off on the shot and is re-seated from the quiver during the reload, and a glint
(full-bright `glow` cubes) sits at the bolt tip while the marksman aims. The spent bolt and the
glint are scaled to 0 when not in use (S(0) in the clip keys).

The hands are placed by a two-bone solver (ik2) onto points of the crossbow, the string or the
quiver, so the grip holds in every key; the clip interpolates the angles between the keys.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front. Pose angles in
degrees: x < 0 swings a limb forward/up, body x > 0 leans forward, y > 0 turns the front to the
mob's right (left shoulder forward). Crossbow frame: origin at the right-hand grip, -z towards the
prod, -y up, so the tiller sits above the grip.
"""
import math

from mathutils import Matrix, Vector

from items import HELMET, Item
from paint import Material
from rig import J, M, R, S, continuous_zyx, decompose_zyx, pose, rot_game

ID = 'marksman'

PALETTES = {
    'bone': Material(['#4b473f', '#6c675b', '#8e8878', '#ada694', '#c8c1ac', '#dcd5c1'], noise=0.55, patch=3),
    'cloth': Material(['#2b2620', '#3b342c', '#4c4338', '#5d5244', '#6c604f'], noise=0.7, patch=3),
    'cloth_dark': ['#14110f', '#1d1916', '#28221e'],
    'leather': ['#2c1c12', '#3f2919', '#533622', '#67452c'],
    'wood': Material(['#2b1d13', '#3d2b1d', '#513a27', '#654a31'], noise=0.45, patch=4),
    'iron': ['#2b2b2e', '#404044', '#57575c', '#707076', '#8b8b91'],
    'string': Material(['#a39572', '#b9ab86'], noise=0, fine=0),
    'feather': ['#4c1914', '#6e231a', '#913325', '#b04a33'],
    'socket': ['#070606', '#100e0c'],
    'glint': Material(['#fff1b8', '#fffbe8'], noise=0, fine=0),
}
BONE_SHADOW = ('bone', 1)
EYE = '#3b1d14'          # a dull ember deep in the socket: not glowing, the glint is the only light


# ---------------------------------------------------------------------------- decals

def skull_decal(face):
    if face.name == 'front':
        face.rect(0, 0, 8, 3, ('bone', 0))           # the hood throws its shadow over the brow
        face.rect(0, 3, 8, 1, ('bone', 1))
        face.rect(0, 3, 1, 5, ('bone', 1))           # and over the cheeks
        face.rect(7, 3, 1, 5, ('bone', 1))
        for x in (1, 5):
            face.rect(x, 3, 2, 2, ('socket', 0))     # eye sockets
            face.px(x + 1, 4, EYE)
        face.rect(3, 5, 2, 1, ('socket', 1))         # nose hole
        face.px(3, 6, ('bone', 2))
        for x in range(1, 7):                         # teeth: dark gaps between lit teeth
            face.px(x, 6, ('socket', 1) if x % 2 else ('bone', -1))
        face.rect(1, 7, 6, 1, ('bone', 2))
    elif face.name in ('left', 'right'):
        face.rect(0, 0, face.w, 2, ('bone', 1))
        face.px(5 if face.name == 'left' else 2, 4, ('socket', 1))   # the ear hole
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('bone', 0))


def hood_slab(inner=None):
    """Decal of a hood slab; inner names the face that looks at the skull (dark lining)."""
    def decal(face):
        if face.name == inner:
            face.rect(0, 0, face.w, face.h, ('cloth_dark', 1))
        else:
            hood_outer(face)
    return decal


def hood_outer(face):
    """A hood slab: dusty cloth, a darker hem, a few torn texels along the lower edge."""
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('cloth_dark', 0))
        return
    if face.name == 'top':
        face.patch(1, 1, max(1, face.w - 3), max(1, face.h - 3), ('cloth', 3), 0.35, 7)
        return
    if face.h >= 6:
        face.rect(0, face.h - 1, face.w, 1, ('cloth', 0))
        for x in range(0, face.w, 3):
            face.px(x, face.h - 2, ('cloth', 0))       # tatters creeping up from the hem
        face.line(1, 2, 1, face.h - 3, ('cloth', 1))  # a fold
        if face.w > 6:
            face.patch(face.w - 5, 4, 3, 3, ('cloth', 0), 0.6, 5)   # a worn, darker patch


def hood_peak(face):
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('cloth_dark', 2))
    elif face.name == 'front':
        face.rect(0, 0, face.w, face.h, ('cloth', 1))


def mantle_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('cloth', 3))
        for x in range(face.w):                        # a ragged hem: torn every other texel
            if (x * 7 + len(face.name)) % 3 == 0:
                face.px(x, face.h - 1, ('cloth', 0))
        if face.name == 'front':
            face.px(face.w // 2, 1, ('iron', 3))       # the cloak pin
            face.px(face.w // 2 - 1, 1, ('iron', 2))
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('cloth_dark', 1))


def cape_decal(face):
    if face.name == 'back':
        for x in (1, 4, 6):
            face.line(x, 1, x, face.h - 2, ('cloth', 1))     # long folds
        face.rect(0, face.h - 1, face.w, 1, ('cloth', 0))
        face.patch(2, face.h - 5, 3, 3, ('cloth', 0), 0.5, 3)  # dirt where it drags
    elif face.name == 'front':
        face.rect(0, 0, face.w, face.h, ('cloth_dark', 2))


def rag_decal(face):
    if face.name in ('back', 'front', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('cloth', 0))


def rib_decal(face):
    if face.name == 'front':
        face.rect(0, 0, face.w, 1, ('bone', -2))
        face.px(face.w // 2 - 1, 0, ('bone', 2))       # the sternum joint
        face.px(face.w // 2, 0, ('bone', 2))
    elif face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('bone', 0))


def limb_decal(face):
    """A 2 px bone: a lit edge, a dark joint at the ends."""
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, 1, face.h, ('bone', -2))
        face.rect(0, face.h - 1, face.w, 1, ('bone', 1))


def wrap_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('cloth', 3))
        face.px(0, face.h - 1, ('cloth', 0))


def bracer_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 1, face.w, 1, ('leather', 0))
        if face.name == 'left':
            face.px(0, 1, ('iron', 3))                 # buckle


def belt_decal(face):
    if face.name == 'front':
        face.rect(3, 0, 2, 1, ('iron', 3))
    elif face.name in ('left', 'right'):
        face.px(1, 0, ('iron', 2))


def flap_decal(face):
    if face.name in ('front', 'back'):
        face.px(0, face.h - 1, ('cloth', 0))
        face.line(1, 0, 1, face.h - 2, ('cloth', 1))


def strap_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.px(0, 4, ('iron', 3))


def quiver_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('leather', 3))      # rolled rim
        face.rect(0, 3, face.w, 1, ('iron', 2))         # bands
        face.rect(0, face.h - 3, face.w, 1, ('iron', 2))
        face.rect(0, face.h - 1, face.w, 1, ('leather', 0))
    elif face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('cloth_dark', 0))   # the dark mouth


def fletch_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('feather', 3))
        face.px(0, face.h - 1, ('wood', 2))


def wood_decal(face):
    if face.name == 'top':
        face.line(face.w // 2, 0, face.w // 2, face.h - 1, ('wood', 0))   # the bolt groove
    elif face.name in ('left', 'right') and face.w >= 8:
        for x in (2, face.w - 3):
            face.rect(x, 0, 1, face.h, ('iron', 2))    # iron bands round the tiller


def butt_decal(face):
    if face.name == 'back':
        face.outline(('iron', 2))
    elif face.name in ('left', 'right'):
        face.rect(face.w - 1, 0, 1, face.h, ('iron', 2))
        face.px(1, 1, ('wood', 0))


def iron_decal(face):
    face.outline(('iron', 1), 'b')


def prod_decal(face):
    if face.name in ('front', 'top'):
        face.rect(0, 0, face.w, 1, ('iron', -1))


def shaft_decal(face):
    if face.name in ('front', 'back', 'left', 'right', 'top', 'bottom'):
        face.line(0, 0, face.w - 1, 0, ('wood', 3))


def head_decal(face):
    face.outline(('iron', -1), 'tl')


def glint_decal(face):
    face.px(face.w // 2, face.h // 2, '#ffffff')


# ---------------------------------------------------------------------------- the hood item

def hood_cubes(cube):
    """The hood as head-local cubes: the mob wears them, the dropped hood is made of them too."""
    return [
        cube((-5, -9, -5), (10, 1, 10), 'cloth', decal=hood_outer),       # crown
        cube((-5, -8, -5), (1, 9, 10), 'cloth', decal=hood_slab('left')),   # right side
        cube((4, -8, -5), (1, 9, 10), 'cloth', decal=hood_slab('right')),   # left side
        cube((-4, -8, 4), (8, 9, 1), 'cloth', decal=hood_slab('front')),    # back
        cube((-4, -8, -5), (8, 2, 1), 'cloth', decal=hood_peak),          # brow, shades the eyes
        cube((-4, -6, -5), (1, 6, 1), 'cloth', decal=hood_peak),          # cheek edges narrow the face
        cube((3, -6, -5), (1, 6, 1), 'cloth', decal=hood_peak),
        cube((-3, -10, -3), (6, 1, 7), 'cloth', decal=hood_outer),        # the crown rises to a point
        cube((-1, -11, 1), (2, 1, 4), 'cloth', decal=hood_peak),
        cube((-1, -10, 5), (2, 5, 1), 'cloth', decal=hood_outer),         # the tail of the point
    ]


def marksman_hood():
    from rig import Cube
    it = Item('marksman_hood', PALETTES, HELMET, texture=64, name='Рваный капюшон стрелка')
    it.from_head_cubes(hood_cubes(lambda o, s, mat, **kw: Cube(o, s, mat, **kw)))
    return it


ITEMS = [marksman_hood()]


# ---------------------------------------------------------------------------- model

# Pivots relative to the parent pivot (all rest rotations are zero, except quiver and strap).
REL = {
    'body': (0, -12, 0), 'head': (0, -12, 0),
    'arm_r': (-5, -10, 0), 'arm_l': (5, -10, 0), 'forearm_r': (0, 5, 0), 'forearm_l': (0, 5, 0),
    'thigh_r': (-2, -12, 0), 'thigh_l': (2, -12, 0), 'shin_r': (0, 6, 0), 'shin_l': (0, 6, 0),
    'crossbow': (-5, -0.5, 0),
}
UPPER, FORE, HAND = 5.0, 4.5, (0, 4.5, 0)     # shoulder -> elbow, elbow -> grip, grip in the forearm
THIGH, SHIN = 6.0, 6.0

# Crossbow frame points.
GRIP = (0, 0, 0)                 # right hand
FORE_GRIP = (0, 0, -3)           # left hand under the tiller
PROD = (-1.5, -3.5, -8)          # right prod limb pivot; the left one mirrors it
LIMB = 6
STRING_AT = (-LIMB, 0, 1.5)      # string pivot in the limb frame: just behind the limb tip
NUT_Z = -1.5                     # spanned string centre
BOLT_AT = (0, -3.5, -1)          # bolt pivot (the nock) in the crossbow frame
GLINT_AT = (0, -3.5, -12.5)      # glint pivot: the bolt tip
CRANK_AT = (-2.5, -1.5, 3.5)     # cranequin axle on the right side of the butt (axis x)
CRANK_KNOB = (-2.5, 0, 3)        # the knob the right hand turns, crank frame


def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 12, 0), parent='root')
    for i, y in enumerate((-11, -9, -7)):
        body.cube((-4, y, -1.5), (8, 1, 3), 'bone', decal=rib_decal)
    body.cube((-3, -5, -1.5), (6, 1, 3), 'bone', decal=rib_decal)
    body.cube((-1, -10.9, 0), (2, 10, 1), 'bone', decal=limb_decal)       # spine, its top 0.1 px under the rib's
    body.cube((-3, -2, -1.5), (6, 2, 3), 'bone', decal=rib_decal)         # pelvis
    body.cube((-4.5, -12.5, -2.5), (9, 3, 5), 'cloth', decal=mantle_decal)  # shoulder mantle
    body.cube((-3.5, -2.5, -2), (7, 1, 4), 'leather', inflate=0.25, decal=belt_decal)
    body.cube((-1, -1.5, -2.5), (2, 5, 1), 'cloth', decal=flap_decal)     # loincloth between the legs

    strap = m.part('strap', (-3.5, 1, -2), parent='body', rot=(0, 0, -36))   # bandolier, front
    strap.cube((-0.5, 0, -0.25), (1, 13, 1), 'leather', inflate=(0, 0, 0.1), decal=strap_decal)   # over the belt

    cape = m.part('cape', (0, 0.5, 2.5), parent='body')
    # The cape and the middle strip are 0.1 px wider than the ribs and the loincloth: swung
    # forward, their sides would lie in theirs (z-fighting).
    wide = (0.1, 0, 0)
    cape.cube((-4, 0, 0), (8, 13, 1), 'cloth', inflate=wide, decal=cape_decal)
    cape.cube((-4, 13, 0), (2, 3, 1), 'cloth', decal=rag_decal)           # torn strips at the hem
    cape.cube((-1, 13, 0), (1, 5, 1), 'cloth', inflate=wide, decal=rag_decal)
    cape.cube((1, 13, 0), (2, 2, 1), 'cloth', decal=rag_decal)
    cape.cube((3, 13, 0), (1, 4, 1), 'cloth', decal=rag_decal)

    quiver = m.part('quiver', (0.5, 6, 4.5), parent='body', rot=(0, 0, -25))
    quiver.cube((-1.5, -6, -1), (3, 12, 3), 'leather', decal=quiver_decal)
    for k, (x, h) in enumerate(((-1, 3), (0, 2), (1, 3))):   # each bolt 1 px deeper in: no shared bottoms
        quiver.cube((x - 0.5, -6 - h, -0.5 + (x % 2) * 0.5), (1, h + k, 1), 'wood', inflate=0.2, decal=fletch_decal)

    head = m.part('head', (0, 0, 0), parent='body')
    head.cube((-4, -8, -4), (8, 8, 8), 'bone', decal=skull_decal)
    hood_cubes(head.cube)

    arm_r = m.part('arm_r', (-5, 2, 0), parent='body')
    # The upper arm is 0.1 px narrower in x: at rest its inner side met the hood's side in one plane.
    up = arm_r.cube((-1, -2, -1), (2, 7, 2), 'bone', inflate=(-0.1, 0, 0), decal=limb_decal)
    rag = arm_r.cube((-1.5, -2.5, -1.5), (3, 3, 3), 'cloth', inflate=0.1, decal=rag_decal)   # over the mantle and ribs
    fore_r = m.part('forearm_r', (-5, 7, 0), parent='arm_r')
    # Forearms and shins are 0.1 px wider in x than the bones above: an elbow or a knee bends
    # about x and would keep their sides in one plane (z-fighting at the joint).
    lo = fore_r.cube((-1, 0, -1), (2, 5, 2), 'bone', inflate=(0.1, 0, 0), decal=limb_decal)
    fore_r.cube((-1, 0.5, -1), (2, 2, 2), 'cloth', inflate=0.3, decal=wrap_decal)
    arm_l = m.part('arm_l', (5, 2, 0), parent='body')
    arm_l.cube((-1, -2, -1), (2, 7, 2), 'bone', inflate=(-0.1, 0, 0), share=up, mirror=True)
    arm_l.cube((-1.5, -2.5, -1.5), (3, 3, 3), 'cloth', inflate=0.1, share=rag, mirror=True)
    fore_l = m.part('forearm_l', (5, 7, 0), parent='arm_l')
    fore_l.cube((-1, 0, -1), (2, 5, 2), 'bone', inflate=(0.1, 0, 0), share=lo, mirror=True)
    fore_l.cube((-1, 0.5, -1), (2, 3, 2), 'leather', inflate=0.35, decal=bracer_decal)   # archer's bracer

    thigh_r = m.part('thigh_r', (-2, 12, 0), parent='root')
    th = thigh_r.cube((-1, 0, -1), (2, 6, 2), 'bone', decal=limb_decal)
    shin_r = m.part('shin_r', (-2, 18, 0), parent='thigh_r')
    sh = shin_r.cube((-1, 0, -1), (2, 6, 2), 'bone', inflate=(0.1, 0, 0), decal=limb_decal)
    wr = shin_r.cube((-1, 1, -1), (2, 2, 2), 'cloth', inflate=0.3, decal=wrap_decal)
    thigh_l = m.part('thigh_l', (2, 12, 0), parent='root')
    thigh_l.cube((-1, 0, -1), (2, 6, 2), 'bone', share=th, mirror=True)
    shin_l = m.part('shin_l', (2, 18, 0), parent='thigh_l')
    shin_l.cube((-1, 0, -1), (2, 6, 2), 'bone', inflate=(0.1, 0, 0), share=sh, mirror=True)
    shin_l.cube((-1, 1, -1), (2, 2, 2), 'cloth', inflate=0.3, share=wr, mirror=True)

    # The crossbow, at rest in the hanging right hand, pointing forward.
    cb = m.part('crossbow', (-5, 11.5, 0), parent='body')
    cb.cube((-1, -3, -8), (2, 2, 11), 'wood', decal=wood_decal)            # tiller
    cb.cube((-1.5, -3, 3), (3, 4, 5), 'wood', decal=butt_decal)            # butt (holds the glint)
    cb.cube((-0.5, -1, -0.5), (1, 2, 1), 'wood')                           # grip
    cb.cube((-0.5, -1, 1), (1, 1, 4), 'iron', decal=iron_decal)            # long trigger bar
    cb.cube((-1, -4, -1), (2, 1, 1), 'iron', inflate=(0.2, 0.1, 0.1), decal=iron_decal)   # nut, proud of the tiller, string, hand
    cb.cube((-1.5, -3, -9), (3, 2, 2), 'iron', inflate=(0, 0.1, 0), decal=iron_decal)   # prod mount, over the tiller
    cb.cube((-1.5, -2.5, -11), (3, 1, 1), 'iron', decal=iron_decal)        # stirrup
    st = cb.cube((-1.5, -2.5, -10), (1, 1, 1), 'iron')
    cb.cube((0.5, -2.5, -10), (1, 1, 1), 'iron', share=st, mirror=True)
    cb.cube((-2.5, -3, 2), (1, 3, 3), 'iron', decal=iron_decal)            # cranequin housing

    crank = m.part('crank', (-5 + CRANK_AT[0], 11.5 + CRANK_AT[1], CRANK_AT[2]), parent='crossbow')
    crank.cube((-1, -0.5, -0.5), (1, 1, 4), 'iron', decal=iron_decal)
    crank.cube((-3, -0.5, 2.5), (2, 1, 1), 'wood')                          # knob

    px, py, pz = PROD
    prod_r = m.part('prod_r', (-5 + px, 11.5 + py, pz), parent='crossbow')
    limb = prod_r.cube((-LIMB, -0.5, -1), (LIMB, 1, 2), 'iron', decal=prod_decal)
    prod_l = m.part('prod_l', (-5 - px, 11.5 + py, pz), parent='crossbow')
    prod_l.cube((0, -0.5, -1), (LIMB, 1, 2), 'iron', share=limb, mirror=True)
    sx, sy, sz = STRING_AT
    string_r = m.part('string_r', (-5 + px + sx, 11.5 + py + sy, pz + sz), parent='prod_r')
    # The string halves are 0.1 px apart in thickness and both thinner than the bolt: where
    # they cross each other or the nock, no two faces share a plane.
    s = string_r.cube((0, -0.5, -0.5), (9, 1, 1), 'string', grain=0)
    string_l = m.part('string_l', (-5 - px - sx, 11.5 + py + sy, pz + sz), parent='prod_l')
    string_l.cube((-9, -0.5, -0.5), (9, 1, 1), 'string', inflate=(0, -0.1, -0.1), grain=0, share=s, mirror=True)

    bx, by, bz = BOLT_AT
    bolt = m.part('bolt', (-5 + bx, 11.5 + by, bz), parent='crossbow')
    bolt.cube((-0.5, -0.5, -9.5), (1, 1, 9), 'wood', inflate=(0.1, 0.1, 0), decal=shaft_decal)   # thicker than the string
    bolt.cube((-0.5, -0.5, -11), (1, 1, 2), 'iron', inflate=(0.3, 0.3, 0), decal=head_decal)
    bolt.cube((-0.5, -0.5, -3), (1, 1, 2), 'feather', inflate=(0.35, 0.35, 0), decal=fletch_decal)

    gx, gy, gz = GLINT_AT
    glint = m.part('glint', (-5 + gx, 11.5 + gy, gz), parent='crossbow', glow=True)
    # Three crossed bars, each 0.1 px thinner than the one before across: where they cross, no two
    # faces share a plane.
    for size, inf in (((3, 1, 1), (-0.1, -0.1, -0.1)), ((1, 3, 1), (-0.2, -0.1, -0.2)), ((1, 1, 3), (-0.3, -0.3, -0.1))):
        glint.cube(tuple(-s / 2 for s in size), size, 'glint', inflate=inf, grain=0, decal=glint_decal)

    m.socket('socket_head', 'head', (0, -9.5, 0))
    m.socket('socket_back', 'body', (0, -6, 6))


# ---------------------------------------------------------------------------- solver

def xf(rot=(0, 0, 0), t=(0, 0, 0)):
    """A rigid transform (3x3, translation px) from game angles in degrees."""
    return rot_game(*(math.radians(a) for a in rot)), Vector(t)


def mul(a, b):
    return a[0] @ b[0], a[0] @ b[1] + a[1]


def inv(a):
    rt = a[0].transposed()
    return rt, -(rt @ a[1])


def at(a, p):
    return a[0] @ Vector(p) + a[1]


def to_j(rel, rest):
    """A local transform as a pose joint against the part's rest pivot (rest rotation zero)."""
    ang = decompose_zyx(rel[0])
    return J(tuple(math.degrees(v) for v in ang), tuple(rel[1] - Vector(rest)))


TAG = [None]


def ik2(s, t, l1, l2, pole, sign):
    """Two-bone limb from s to t: the upper bone's rotation (game angles, deg) that points its +y
    at the elbow, and the lower bone's x bend (deg). The joint goes towards pole; sign picks the
    bend direction of the lower bone (-1: towards its front, arms; +1: towards its back, legs)."""
    s, t, pole = Vector(s), Vector(t), Vector(pole)
    d = t - s
    dist = d.length
    if dist > (l1 + l2) * 1.04:
        print('marksman: limb short by %.1f px' % (dist - l1 - l2), TAG[0])
    dirv = d.normalized()
    dc = min(max(dist, abs(l1 - l2) + 1e-3), l1 + l2 - 1e-4)
    a = math.acos(max(-1.0, min(1.0, (l1 * l1 + dc * dc - l2 * l2) / (2 * l1 * dc))))
    p = (pole - dirv * pole.dot(dirv)).normalized()
    d1 = dirv * math.cos(a) + p * math.sin(a)
    d2 = (s + dirv * dc - (s + d1 * l1)).normalized()
    n = dirv.cross(p).normalized()
    for xa in (n, -n):
        r1 = Matrix((xa, d1, xa.cross(d1))).transposed()
        l2v = r1.transposed() @ d2
        th = math.atan2(l2v.z, l2v.y)
        if th * sign >= -1e-9:
            break
    return tuple(math.degrees(v) for v in decompose_zyx(r1)), math.degrees(th)


def string_state(f):
    """Prod limbs and string halves for a span f (0 slack, 1 spanned at the nut), and the string
    centre in the crossbow frame."""
    phi = 10.0 * f
    tip = Vector(PROD) + rot_game(0, math.radians(phi), 0) @ Vector(STRING_AT)
    centre = Vector((0, PROD[1], PROD[2] + STRING_AT[2] + (NUT_Z - PROD[2] - STRING_AT[2]) * f))
    d = centre - tip
    psi = math.degrees(math.atan2(-d.z, d.x)) - phi
    return {'prod_r': R(0, phi, 0), 'prod_l': R(0, -phi, 0), 'string_r': R(0, psi, 0), 'string_l': R(0, -psi, 0)}, centre


def crank_point(angle):
    rest = Vector(CRANK_AT)
    return rest + rot_game(math.radians(angle), 0, 0) @ Vector(CRANK_KNOB)


ARM_POLE = {'r': (-1, 0.6, 1.2), 'l': (1, 0.6, 1.2)}


def rig(body=J(), head=J(), root=J(), cb=None, hr=('cb', GRIP), hl=('cb', FORE_GRIP), pr=None, pl=None,
        draw=1.0, bolt='seat', glint=None, crank=0.0, feet=None, legs=None, cape=0.0, extra=None, frame='ground'):
    """One full pose. cb: the crossbow's place (px) and angles (deg). Places are in the ground
    frame (origin at the feet at rest; root moves are taken out), or with frame='root' in the
    posed root frame (they topple with the root). hr / hl: where the hands go: ('cb', crossbow
    point), ('pt', place), ('body', body-frame point), ('crank',) or ('fk', arm J, forearm J).
    feet: (x, y, z) places per foot for the leg solver, else legs: FK joints. bolt: 'seat',
    'hide', ('fly', px) or 'hand'. glint: None (hidden) or its spin in degrees."""
    cb = cb or LOW_CB
    out = {'root': root, 'body': body, 'head': head}
    w_root = xf(root.rot, root.move) if frame == 'ground' else xf()
    w_body = mul(w_root, xf(body.rot, Vector(REL['body']) + Vector(body.move)))
    to_body = inv(w_body)
    w_cb = mul(to_body, xf(cb[1], cb[0]))           # crossbow in the body frame
    out['crossbow'] = to_j(w_cb, REL['crossbow'])
    parts, centre = string_state(draw)
    out.update(parts)
    out['crank'] = R(crank, 0, 0)

    arm_xf = {}
    for side, spec, pole in (('r', hr, pr), ('l', hl, pl)):
        kind = spec[0]
        if kind == 'fk':
            ja, jf = spec[1], spec[2]
        else:
            if kind == 'cb':
                p = at(w_cb, spec[1])
            elif kind == 'crank':
                p = at(w_cb, crank_point(crank))
            elif kind == 'body':
                p = Vector(spec[1])
            else:
                p = at(to_body, spec[1])
            TAG[0] = (side, spec[0], cb[0])
            ua, fa = ik2(REL['arm_' + side], p, UPPER, FORE, pole or ARM_POLE[side], -1)
            ja, jf = J(ua), R(fa, 0, 0)
        out['arm_' + side], out['forearm_' + side] = ja, jf
        arm_xf[side] = mul(mul(xf(ja.rot, REL['arm_' + side]), xf(jf.rot, REL['forearm_' + side])), xf((0, 0, 0), HAND))

    if bolt == 'hide':   # spent: shrunk away in its seat
        out['bolt'] = S(0)
    elif bolt == 'hand':   # across the right fist, tip forward, held near its middle
        b = mul(arm_xf['r'], xf((0, 0, 0), (0, 0, 5)))
        out['bolt'] = to_j(mul(inv(w_cb), b), BOLT_AT)
    elif isinstance(bolt, tuple):
        out['bolt'] = M(0, 0, -bolt[1])
    out['glint'] = S(0) if glint is None else R(0, 0, glint)

    if feet:
        for side, foot in zip('rl', feet):
            TAG[0] = ('foot', side, foot)
            ua, fa = ik2(REL['thigh_' + side], at(inv(w_root), foot), THIGH, SHIN, (0, -0.2, -1), 1)
            out['thigh_' + side], out['shin_' + side] = J(ua), R(fa, 0, 0)
    for k, v in (legs or {}).items():
        out[k] = v
    out['cape'] = R(max(0.0, body.rot[0]) * 0.6 + cape, 0, 0)
    out.update(extra or {})
    return out


def smooth(clip):
    """Unwraps every part's angles key to key (Z-Y-X has two solutions and 2 pi shifts), so the
    interpolation between keys takes the short way."""
    prev = {}
    keys = []
    for frame, p, ease in clip.keys:
        q = {}
        for name, j in p.items():
            r = rot_game(*(math.radians(a) for a in j.rot))
            ref = prev.get(name, tuple(math.radians(a) for a in j.rot))
            ang = continuous_zyx(r, ref)
            prev[name] = ang
            q[name] = J(tuple(math.degrees(a) for a in ang), j.move, j.scale)
        keys.append((frame, q, ease))
    if clip.loop:
        keys[-1] = (keys[-1][0], keys[0][1], keys[-1][2])
    clip.keys = keys


# ---------------------------------------------------------------------------- poses

LEGS_STAND = dict(thigh_r=R(3, 0, 3), thigh_l=R(-6, 0, -3), shin_l=R(5, 0, 0))
STANCE_BODY, STANCE_HEAD = R(5, 14, 0), R(-4, -14, 0)
LOW_CB = ((-1, -17, -2.5), (38, -24, 6))          # low ready: nose down, across the belly


def stance(**kw):
    spec = dict(body=STANCE_BODY, head=STANCE_HEAD, cb=LOW_CB, hl=('cb', (0, 0, -2)), legs=LEGS_STAND)
    spec.update(kw)
    return rig(**spec)


AIM_BODY, AIM_HEAD = R(4, 32, 0), R(6, -30, -10)
AIM_CB = ((-2, -20.5, -6.5), (0, 0, 0))            # level at the right shoulder, butt in the shoulder
AIM_LEGS = dict(thigh_r=R(8, 0, 4), thigh_l=R(-14, 0, -3), shin_l=R(10, 0, 0), shin_r=R(2, 0, 0))


def aim(glint=None, **kw):
    spec = dict(body=AIM_BODY, head=AIM_HEAD, cb=AIM_CB, hl=('cb', (0, 0, -2.5)), legs=AIM_LEGS, glint=glint)
    spec.update(kw)
    return rig(**spec)


# Kneeling on the left knee, the right foot forward: the hips 5 px lower.
KNEEL_ROOT = M(0, 5, 0)
KNEEL_LEGS = dict(thigh_r=R(-80, 0, 4), shin_r=R(78, 0, 0), thigh_l=R(8, 0, -3), shin_l=R(82, 0, 0))
LOW = 5   # how much lower the hands work when kneeling


def kneel(**kw):
    spec = dict(root=KNEEL_ROOT, body=R(12, 10, 0), head=R(-12, -10, 0),
                cb=((-1.5, -13, -6), (-66, -8, 4)), hl=('cb', (0, 0, -3.5)), legs=KNEEL_LEGS)
    spec.update(kw)
    return rig(**spec)


def reload_keys(clip, t0, low):
    """The reload from the slack crossbow to a seated bolt: the stirrup down on the ground under
    the right foot, the cranequin wound 2.5 turns while the string creeps back to the nut, the
    crossbow up again, a bolt drawn from the quiver over the right shoulder and seated in the
    groove. Keys t0+3 .. t0+24. low: kneeling behind cover; else standing in a lunge (the
    recovery of the shot)."""
    if low:
        base = dict(root=KNEEL_ROOT, body=R(14, 10, 0), head=R(-6, -10, 0), legs=KNEEL_LEGS)
        stirrup_z, d = -6.5, LOW
    else:
        base = dict(root=M(0, 3.5, 0), body=R(12, 10, 0), head=R(-4, -10, 0),
                    feet=((-2, -0.3, -6), (2, 0, 3.5)))
        stirrup_z, d = -6.5, 0
    # The crossbow stands on its stirrup by the right foot, the butt up and a little forward.
    tilt = (95, 0, -4)
    stirrup = Vector((-2, -1.5, stirrup_z))
    grip = stirrup - rot_game(*(math.radians(a) for a in tilt)) @ Vector((0, -2, -10.5))
    down = dict(base, cb=(tuple(grip), tilt), hl=('cb', (1.5, -1.5, 5.5)), pl=(1, 1, 1))
    # 0-3: the stirrup goes down by the right foot, the right hand finds the crank.
    clip.key(t0 + 3, rig(draw=0, bolt='hide', hr=('crank',), crank=0, **down), 'linear')
    # 3-13: 2.5 turns of the crank, a tick per quarter; the string comes back with it.
    for i in range(1, 11):
        spec = dict(down, body=down['body'] + M(0, 0.4 if i % 2 else 0, 0))
        clip.key(t0 + 3 + i, rig(draw=i / 10, bolt='hide', hr=('crank',), crank=-90 * i, **spec),
                 'linear' if i < 10 else 'out')
    # 13-16: crossbow up to the low ready, the right hand off the crank.
    up_cb = ((-1, -16.5 + d * 0.7, -3), (30, -24, 6))
    lift = dict(base, cb=up_cb, body=base['body'] + R(-8, 0, 0))
    clip.key(t0 + 16, rig(bolt='hide', hl=('cb', (0, 0, -3)), hr=('body', (-3.5, -4, -5)), **lift), 'out')
    # 16-19: over the right shoulder to the quiver; the bolt is in the fist from 19 on.
    reach = dict(lift, head=base['head'] + R(-4, 16, 0), body=base['body'] + R(-12, -6, 0))
    clip.key(t0 + 19, rig(bolt='hide', hr=('body', (-2.4, -13, 5)), pr=(-1, -1, -0.3), **reach), 'hold')
    clip.key(t0 + 20, rig(bolt='hand', hr=('body', (-3, -15.5, 4)), pr=(-1, -1, -0.3), **reach), 'inout')
    # 20-24: down and along the groove; seated on 24.
    clip.key(t0 + 23, rig(bolt='hand', hr=('cb', (0, -5, -1)), pr=(-1, 0.3, 1), **lift), 'in')
    clip.key(t0 + 24, rig(bolt='seat', hr=('cb', (0, -4.5, -4)), pr=(-1, 0.3, 1), **lift), 'out')


# ---------------------------------------------------------------------------- clips

def attack_shoot(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    a.key(0, stance(), 'out')
    a.key(4, aim(body=AIM_BODY + R(-4, 0, 0), cb=((-2, -21.5, -6), (-6, 0, 0))), 'out')   # up, a touch high
    a.key(6, aim(glint=0), 'linear')                 # settled on the target: the glint shows
    for i, t in enumerate(range(8, hit, 2)):          # dead still, only the glint twinkles
        a.key(t, aim(glint=45 * (i + 1), body=AIM_BODY + M(0, 0.15 * (i % 2), 0)), 'linear')
    a.key(hit, aim(glint=45 * 5), 'snap')             # release
    kick = aim(cb=((-2, -21.5, -4.5), (-14, 0, 0)), body=AIM_BODY + R(-6, 0, 0) + M(0, 0, 1),
               head=AIM_HEAD + R(-6, 0, 0), draw=0, bolt=('fly', 40), extra={'prod_r': R(0, -6, 0), 'prod_l': R(0, 6, 0)})
    a.key(hit + 1, kick, 'linear')
    a.key(rec, aim(cb=((-2, -21, -5), (-10, 0, 0)), body=AIM_BODY + R(-4, 0, 0), draw=0, bolt=('fly', 80)), 'hold')
    a.key(rec + 1, aim(cb=((-2, -21, -5), (-8, 0, 0)), body=AIM_BODY + R(-3, 0, 0), draw=0, bolt='hide'), 'slow')
    reload_keys(a, rec + 2, low=False)
    a.key(end, stance(), 'inout')


def attack_butt(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    # Pulled to the right shoulder: nose up and back, butt low and forward, body wound to the right.
    cock = dict(body=R(-4, -28, 0), head=R(0, 26, 0), cb=((-4.5, -21, -1), (-118, -18, 0)),
                hr=('cb', GRIP), hl=('cb', (0, 0, -4)), pl=(1, 1, 0.6),
                legs=dict(thigh_r=R(12, 0, 4), thigh_l=R(-18, 0, -4), shin_l=R(10, 0, 0)))
    smash = dict(body=R(22, 34, 0) + M(0, 0.5, -1.5), head=R(-16, -30, 0), cb=((-1, -19.5, -6), (162, 30, 0)),
                 hr=('cb', GRIP), hl=('cb', (0, 0, -4)), pl=(1, 1, 0.6), root=M(0, 0, -1.5),
                 legs=dict(thigh_r=R(22, 0, 3), thigh_l=R(-30, 0, -3), shin_l=R(20, 0, 0)))
    a.key(0, stance(), 'out')
    a.key(hit - 2, rig(**cock), 'in')                 # held at the shoulder: the tell
    a.key(hit, rig(**smash), 'out')                   # butt into the face
    stuck = dict(smash, body=smash['body'] + R(4, 2, 0), cb=((-1, -18.5, -5.5), (156, 30, 0)))
    a.key(rec, rig(**stuck), 'hold')
    a.key(rec + 4, rig(**dict(stuck, body=stuck['body'] + R(-2, 0, 0))), 'slow')
    a.key(end, stance())


def trophy_clip(c):
    """The trophy pose: down on one knee on the ruined ledge, the crossbow level at the shoulder,
    the glint lit. A slow loop: the glint twinkles, the shoulders rise and fall."""
    t = c.clip('trophy', 80, loop=True)
    for i, f in enumerate(range(0, 81, 10)):
        k = i % 2
        t.key(f, kneel(body=AIM_BODY + R(6, 0, 0) + M(0, 0.2 * k, 0), head=AIM_HEAD + R(-4, 0, 0),
                       cb=((-2, -20.5 + LOW, -6.5 + 0.1 * k), (0, 0, 0)), hl=('cb', (0, 0, -2.5)), glint=45 * k, cape=2 * k),
              'inout')


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}

    idle = c.clip('idle', 40, loop=True)
    idle.key(0, stance())
    idle.key(20, stance(body=STANCE_BODY + R(2, 0, 0) + M(0, 0.3, 0), head=STANCE_HEAD + R(-3, 4, 0),
                        cb=((-1, -16.7, -2.6), (40, -24, 6)), cape=3))
    idle.key(40, stance())

    walk = c.clip('walk', 20, loop=True)   # one step cycle; the game drives it by walk_position
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        bob = M(0, -0.6 if s == 0 else 0, 0)
        legs = dict(thigh_l=R(-24 * s, 0, -2), thigh_r=R(24 * s, 0, 2),
                    shin_l=R(30 if s < 0 else 4, 0, 0), shin_r=R(30 if s > 0 else 4, 0, 0))
        if s == 0:
            legs = dict(thigh_l=R(0, 0, -2), thigh_r=R(0, 0, 2), shin_l=R(2, 0, 0), shin_r=R(2, 0, 0))
            back = 'l' if f == 5 else 'r'
            legs['thigh_' + back], legs['shin_' + back] = R(14, 0, (-2 if back == 'l' else 2)), R(40, 0, 0)
        walk.key(f, stance(body=STANCE_BODY + bob + R(2, 3 * s, 0), head=STANCE_HEAD + R(0, -3 * s, 0),
                           cb=((-1, -17 - (0.6 if s == 0 else 0), -2.5), (38, -24 + 3 * s, 6)),
                           legs=legs, cape=6 + (2 if s == 0 else 0)))

    aimc = c.clip('aim', 20, loop=True)    # aim held while the target moves: the glint keeps turning
    for i, f in enumerate(range(0, 21, 5)):
        aimc.key(f, aim(glint=(0 if f in (0, 20) else 45 * i), body=AIM_BODY + M(0, 0.15 if f == 10 else 0, 0)), 'linear')

    attack_shoot(c.attack('attack_shoot', attacks['attack_shoot']))
    attack_butt(c.attack('attack_butt', attacks['attack_butt']))

    # Reload behind cover (the hide move): five hurried crouched steps back, down on one knee, the
    # same winding and bolt, up again. 30 ticks, the length of the shot's recovery.
    rl = c.clip('reload', 30)
    rl.key(0, aim(cb=((-2, -21, -5), (-8, 0, 0)), draw=0, bolt='hide'), 'out')
    for f, s in ((1, 1), (2, -1), (3, 1)):
        rl.key(f, rig(root=M(0, 2, 0), body=R(24, 10, 0), head=R(-18, -10, 0), cb=((-1.5, -14.5, -6), (24, -20, 6)), frame='root',
                      draw=0, bolt='hide', cape=10,
                      legs=dict(thigh_r=R(-30 * s, 0, 3), shin_r=R(36 if s < 0 else 20, 0, 0),
                                thigh_l=R(30 * s, 0, -3), shin_l=R(36 if s > 0 else 20, 0, 0))), 'linear')
    reload_keys(rl, 4, low=True)
    rl.key(30, stance(), 'inout')

    crouch = c.clip('crouch', 20, loop=True)   # waiting behind cover, loaded, the crossbow upright
    crouch.key(0, kneel())
    crouch.key(10, kneel(body=R(15, 10, 0) + M(0, 0.3, 0), head=R(-10, -6, 0), cape=2))
    crouch.key(20, kneel())

    back = c.clip('backstep', 12)              # a hop back after the butt strike
    crouch_down = dict(root=M(0, 2.5, 0), body=R(16, 14, 0), head=R(-12, -14, 0),
                       legs=dict(thigh_r=R(-34, 0, 3), shin_r=R(52, 0, 0), thigh_l=R(-20, 0, -3), shin_l=R(44, 0, 0)))
    air = dict(root=M(0, -3, 1), body=R(-8, 14, 0), head=R(4, -14, 0), cape=-18,
               legs=dict(thigh_r=R(-40, 0, 3), shin_r=R(60, 0, 0), thigh_l=R(-12, 0, -3), shin_l=R(38, 0, 0)))
    land = dict(root=M(0, 2, 0), body=R(18, 14, 0), head=R(-14, -14, 0), cape=10,
                legs=dict(thigh_r=R(-30, 0, 3), shin_r=R(46, 0, 0), thigh_l=R(-36, 0, -3), shin_l=R(54, 0, 0)))
    back.key(0, stance(), 'out')
    back.key(3, stance(frame='root', **crouch_down), 'in')
    back.key(6, stance(frame='root', **air), 'inout')
    back.key(8, stance(frame='root', **land), 'slow')
    back.key(12, stance())

    hurt = c.clip('hurt', 10)   # vanilla hurt_time is 10 ticks
    hurt.key(0, stance(), 'out')
    hurt.key(2, stance(body=R(-14, 6, 4) + M(0, 0, 1), head=R(-22, -8, 6), root=M(0, 0, 1), cape=-10,
                       cb=((-2, -18, -1.5), (24, -30, 14)), frame='root'), 'slow')
    hurt.key(10, stance())

    death = c.clip('death', 20)  # the bones fold: knees give, the crossbow drops, the frame topples
    sag = dict(root=M(0, 4, 0), body=R(30, 8, 0), head=R(34, -10, 10),
               cb=((-2, -12.5, -5.5), (70, -30, 30)), frame='root', hl=('fk', R(30, 0, -30), R(-10, 0, 0)),
               legs=dict(thigh_r=R(-50, 0, 6), shin_r=R(70, 0, 0), thigh_l=R(-40, 0, -6), shin_l=R(64, 0, 0)))
    death.key(0, stance(), 'out')
    death.key(6, rig(**sag), 'in')
    death.key(18, rig(**dict(sag, root=M(0, 4, 0) + R(0, 0, 86))), 'back')
    death.key(20, rig(**dict(sag, root=M(0, 4, 0) + R(0, 0, 90))))

    trophy_clip(c)

    for clip in c.clips.values():
        smooth(clip)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['head'], 'scale': 0.8},
}
