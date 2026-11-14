"""Brute (soulscraft:brute, Громила): a huge undead giant with a two-handed stone maul.

Horned iron helmet, a butcher's leather apron with a bib, grey-green dead flesh, arms down to the
knees, a hunched back with the spine showing. 43 px from the feet to the top of the head, the
helmet reaches 44 and the horns 49. Wide: 34 px from fist to fist at rest.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front.
Pose angles in degrees: limb x < 0 swings forward/up, body x > 0 leans forward, y > 0 turns the
front to the mob's right (left shoulder forward), right arm z > 0 lifts it out sideways.

Each arm has an elbow: the upper arm (arm_r, arm_l) and the forearm with the fist (forearm_r,
forearm_l), a hinge that bends the forearm forward. The maul is an element item model
(soulscraft:brute_maul) in socket_hand_r, drawn by the game's item chain like the revenant's
sword. The socket hangs under a cubeless wrist part, hand_r, in the middle of the right fist on
the forearm, so a clip can turn the maul in the fist: a two-handed weapon needs another wrist
angle for the slam, the swing and the rest pose. The fist wraps the haft cleanly only while the
forearm lies square to it, so every key bends the elbow to make it so (clash() checks the maul
against the cubes). The left arm is not keyed by hand in two-handed poses: reach() aims it at the
grip on the haft, worked out through the same socket and display chain the game uses, and turns
the elbow so that forearm too lies square to the haft. The trophy pose rests the maul's head on the
right shoulder, the fist near the middle of the haft (carry()).

Rage: eyes_rage is a glow part (red, full-bright) over the eye sockets with state='rage': rig.json
marks it, and the game draws it only while the mob has the rage flag (below 30% health). The rage
clip is the roar that goes with the flag turning on.
"""
import math

from mathutils import Matrix, Vector

import items
import rig
from items import HELMET, Item
from paint import Material
from rig import J, M, R, lerp, pose

ID = 'brute'
TEXTURE = (128, 128)

PALETTES = {
    'flesh': Material(['#36403b', '#48544d', '#5b6860', '#6f7c72', '#849083'], noise=0.7, patch=3),
    'flesh_dark': ['#232925', '#2f3631', '#3c443e', '#4a534c'],
    'rot': ['#2a2420', '#3a2f27', '#4b3c30'],
    'bone': ['#7f7663', '#9c917a', '#b7ad93', '#d0c7ad'],
    'horn': Material(['#3b2f22', '#5a4a35', '#7c6a4d', '#9e8c69', '#bfae89'], noise=0.4, patch=4),
    'iron': Material(['#2a2a2c', '#3b3b3e', '#4f4f53', '#656569', '#7c7c80'], noise=0.7, patch=3),
    'iron_dark': ['#1d1d1f', '#29292c', '#36363a', '#444448'],
    'leather': Material(['#33221a', '#472f21', '#5d3e2a', '#735034', '#8a6340'], noise=0.6, patch=3),
    'leather_dark': ['#1f1510', '#2c1e16', '#3a281d', '#4a3426'],
    'cloth': ['#25221f', '#322e2a', '#413b35', '#4f4840'],
    'rope': ['#4d3f28', '#665436', '#806b47', '#998356'],
    'stone': Material(['#3f3f3d', '#545451', '#6a6a66', '#80807b', '#979690'], noise=0.9, patch=3),
    'wood': Material(['#2f2217', '#433021', '#58402b', '#6d5035'], noise=0.4, patch=4),
    'blood': ['#2e100d', '#431813', '#592119'],
    'socket': ['#070606', '#100e0d', '#1a1716'],
    'rage': ['#ff2a1a', '#ff6a50', '#ffc0b0'],
}
EYE_DIM = '#bba868'      # dull, not glowing: the eyes of a calm brute
EYE_DIM_CORE = '#b7ab70'
SOCKET = ('socket', 0)
BLOOD = ('blood', 1)
STITCH = ('flesh_dark', 0)
RIVET = ('iron', -1)


# ---------------------------------------------------------------------------- decals

def rivets(face, color=RIVET, inset=0):
    w, h = face.w, face.h
    for x, y in ((inset, inset), (w - 1 - inset, inset), (inset, h - 1 - inset), (w - 1 - inset, h - 1 - inset)):
        face.px(x, y, color)


def stitches(face, x0, y, length, horizontal=True):
    """A stitched scar: a dark seam with cross stitches every other texel."""
    for i in range(length):
        x, yy = (x0 + i, y) if horizontal else (x0, y + i)
        face.px(x, yy, STITCH)
        if i % 2 == 0:
            face.px(x if horizontal else x - 1, yy - 1 if horizontal else yy, ('flesh', 0))


def head_decal(face):
    if face.name == 'front':
        face.rect(0, 5, 8, 1, ('flesh_dark', 1))           # the brow shadow under the helmet rim
        for x in (1, 2, 5, 6):                             # deep sockets
            face.px(x, 5, SOCKET)
        face.px(2, 5, EYE_DIM)
        face.px(5, 5, EYE_DIM)
        face.px(1, 5, ('socket', 1))
        face.px(6, 5, ('socket', 1))
    elif face.name in ('left', 'right'):
        stitches(face, 1, 6, 5)                             # cheek seam from the jaw to the ear
        face.px(5, 5, ('rot', 1))
        face.px(6, 5, ('rot', 0))
    elif face.name == 'back':
        face.patch(1, 4, 6, 4, ('rot', 1), 0.45, 3)


def jaw_decal(face):
    if face.name == 'front':
        for x in range(face.w):                             # a row of broken teeth along the top
            face.px(x, 0, ('bone', 2) if x % 2 else ('socket', 1))
        face.px(4, 0, SOCKET)
        face.rect(0, 1, face.w, 1, ('flesh_dark', 1))
        stitches(face, 2, 2, 5)
    elif face.name == 'top':
        face.rect(1, 0, face.w - 2, 2, ('socket', 1))      # the open mouth seen from above
    elif face.name in ('left', 'right'):
        face.px(face.w - 2, 0, ('bone', 1))


def tusk_decal(face):
    if face.name == 'top':
        face.px(0, 0, ('bone', -1))


def cap_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('iron_dark', 1))
        face.px(1, 1, RIVET)
        face.px(face.w - 2, 1, RIVET)
        if face.name == 'front':
            face.px(3, 0, ('iron', 0))                     # a dent above the brow
            face.px(4, 1, ('iron_dark', 0))
        if face.name == 'left':
            face.patch(3, 0, 4, 2, ('iron_dark', 1), 0.5, 7)
    elif face.name == 'top':
        face.outline(('iron', -2), 'tb')
        face.rect(3, 3, 2, 2, ('iron_dark', 1))            # where the knob is riveted on
        face.px(5, 6, ('iron', 0))


def rim_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, face.h, ('iron_dark', 2))
        for x in range(1, face.w, 3):
            face.px(x, 0, RIVET)


def nasal_decal(face):
    if face.name == 'front':
        face.px(0, 0, RIVET)
        face.px(0, face.h - 1, ('iron_dark', 1))


def knob_decal(face):
    if face.name == 'top':
        face.px(0, 0, ('iron', -1))


def horn_decal(face, rings=(), tip=False):
    """Horn rings: darker bands across the horn; the tip darker still."""
    if face.name in ('top', 'bottom'):
        if tip:
            face.rect(0, 0, face.w, face.h, ('horn', 0))
        return
    for y in rings:
        face.rect(0, y, face.w, 1, ('horn', 1))
    if tip:
        face.rect(0, 0, face.w, face.h, ('horn', 1))
        face.px(0, 0, ('horn', 0))


def horn_base(face):
    if face.name in ('front', 'back', 'top', 'bottom'):
        face.rect(0, 0, 1, face.h, ('horn', 1))            # the ring where it meets the helmet
        face.px(face.w - 1, 0, ('horn', -1))


def horn_mid(face):
    horn_decal(face, rings=(1,))


def horn_up(face):
    horn_decal(face, rings=(0, 2))


def horn_tip(face):
    horn_decal(face, tip=True)


def chest_decal(face):
    if face.name == 'front':
        stitches(face, 1, 3, 3)                            # pecs either side of the bib
        face.px(15, 6, ('rot', 1))
        face.px(16, 6, ('rot', 0))
        face.px(16, 7, ('rot', 1))
        face.rect(4, 0, 2, 1, ('leather', 1))              # neck straps of the bib
        face.rect(12, 0, 2, 1, ('leather', 1))
    elif face.name == 'left':                               # rotten flank with the ribs showing
        face.patch(2, 3, 7, 7, ('rot', 0), 0.65, 11)
        for y in (4, 6, 8):
            face.rect(3, y, 5, 1, ('bone', 1))
            face.px(3, y, ('bone', 0))
    elif face.name == 'right':
        stitches(face, 2, 5, 7)
    elif face.name == 'back':
        face.line(2, 0, 15, 10, ('leather', 1))           # the apron straps cross on the back
        face.line(15, 0, 2, 10, ('leather', 1))
        face.rect(8, 4, 2, 2, ('iron', 1))                 # the ring where they cross
        face.patch(1, 6, 4, 4, ('rot', 1), 0.5, 12)
    elif face.name == 'top':
        face.rect(4, 0, 2, face.h, ('leather', 1))
        face.rect(12, 0, 2, face.h, ('leather', 1))


def traps_decal(face):
    if face.name == 'top':
        face.rect(2, 0, 2, face.h, ('leather', 1))
        face.rect(10, 0, 2, face.h, ('leather', 1))
    elif face.name == 'back':
        face.patch(4, 0, 6, 3, ('rot', 1), 0.5, 13)


def belly_decal(face):
    if face.name in ('left', 'right'):
        stitches(face, 1, 3, 6)
    elif face.name == 'back':
        face.patch(3, 1, 9, 4, ('rot', 1), 0.45, 14)


def spine_decal(face):
    if face.name in ('back', 'top'):
        face.px(0, 0, ('bone', -1))
        face.px(1, 1, ('bone', 0))


def belt_decal(face):
    if face.name == 'front':
        face.rect(6, 0, 4, 2, ('iron', 2))                 # the big buckle
        face.rect(7, 0, 2, 1, ('iron_dark', 0))
        for x in (2, 13):
            face.px(x, 0, RIVET)
    elif face.name in ('left', 'right', 'back'):
        for x in range(1, face.w, 3):
            face.px(x, 0, RIVET)


def skull_decal(face):
    if face.name == 'left':                                 # a trophy skull on the hip, facing out
        face.px(0, 1, SOCKET)
        face.px(2, 1, SOCKET)
        face.px(1, 2, ('bone', 0))
    elif face.name == 'front':
        face.px(1, 0, ('rope', 1))


def bib_decal(face):
    if face.name == 'front':
        face.outline(('leather_dark', 2), 'lr')
        face.rect(0, 0, face.w, 1, ('leather_dark', 2))
        face.px(0, 0, RIVET)
        face.px(face.w - 1, 0, RIVET)
        face.rect(4, 0, 1, face.h, ('leather_dark', 3))    # a centre seam
        for y in range(1, face.h, 2):
            face.px(4, y, ('leather', -1))
        face.patch(1, 8, 5, 5, BLOOD, 0.55, 21)            # old blood: a butcher's apron
        face.patch(6, 10, 3, 4, ('blood', 0), 0.5, 22)
        face.px(7, 3, ('blood', 0))


def apron_decal(face):
    if face.name == 'front':
        face.outline(('leather_dark', 2), 'lr')
        face.rect(6, 0, 1, face.h, ('leather_dark', 3))
        for y in range(0, face.h, 2):
            face.px(6, y, ('leather', -1))
        face.patch(1, 1, 5, 6, BLOOD, 0.55, 23)
        face.patch(8, 6, 4, 4, ('blood', 0), 0.5, 24)
        face.streak(3, 7, 3, BLOOD, tail=('blood', 0))
        for x in range(face.w):                             # a ragged, darker hem
            face.px(x, face.h - 1, ('leather_dark', 1) if x % 3 else ('leather_dark', 0))
        face.px(2, face.h - 2, ('leather_dark', 1))
        face.px(9, face.h - 2, ('leather_dark', 1))
    elif face.name == 'back':
        face.rect(0, 0, face.w, face.h, ('leather_dark', 2))


def arm_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        stitches(face, 1, 4, face.w - 2)
    if face.name == 'right':
        face.patch(1, 7, 4, 3, ('rot', 1), 0.5, 31)
    elif face.name == 'bottom':                             # the crook of the elbow, seen when it bends
        face.outline(('flesh_dark', 1))
        face.rect(2, 2, face.w - 4, face.h - 4, ('flesh_dark', 0))


def forearm_decal(face):
    if face.name == 'top':                                  # the elbow: row 0 is the back of the arm
        face.rect(0, 0, face.w, face.h, ('flesh', 1))
        face.rect(1, 2, face.w - 2, face.h - 3, ('flesh_dark', 1))
        face.rect(3, 0, 2, 2, ('bone', 1))                  # the point of the elbow, bone through the skin
        face.px(3, 0, ('bone', 2))


def bracer_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        for y in (0, 3):                                   # wrapped straps, darker edges
            face.rect(0, y, face.w, 1, ('leather_dark', 2))
        face.px(2, 1, RIVET)
        face.px(face.w - 3, 4, RIVET)


def fist_decal(face):
    if face.name == 'front':
        for x in range(0, face.w, 2):                       # bare knuckle bones
            face.px(x, 0, ('bone', 1))
        face.rect(0, 2, face.w, 1, ('flesh_dark', 1))      # the line of the folded fingers
    elif face.name in ('left', 'right'):
        face.rect(0, 2, face.w, 1, ('flesh_dark', 1))
    elif face.name == 'bottom':
        face.rect(1, 1, face.w - 2, face.h - 2, ('flesh_dark', 2))


def shoulder_decal(face):
    if face.name in ('front', 'back', 'left', 'right', 'top'):
        face.line(0, 0, face.w - 1, face.h - 1, ('rope', 2))   # rope wound round the bare shoulder
        face.line(0, 2, face.w - 3, face.h - 1, ('rope', 1))


def pauldron_decal(face):
    if face.name == 'top':
        face.rect(1, 1, face.w - 2, face.h - 2, ('iron', 2))
        rivets(face, RIVET, 1)
        face.px(4, 6, ('iron_dark', 1))
    elif face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 2, ('iron', 2))
        face.rect(0, 2, face.w, 1, ('iron_dark', 1))
        for x in range(1, face.w, 3):
            face.px(x, 0, RIVET)
        face.rect(0, face.h - 1, face.w, 1, ('leather_dark', 2))


def spike_decal(face):
    if face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('iron', -1))


def leg_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.patch(0, 5, face.w, 4, ('flesh', 1), 0.45, 41)   # torn at the knee, dead skin under it
        face.rect(0, 10, face.w, 1, ('rope', 1))               # ties round the shin
        face.px(1, 11, ('rope', 0))
    if face.name == 'front':
        face.px(2, 6, ('flesh_dark', 1))


def foot_decal(face):
    if face.name == 'front':
        for x in (0, 2, 4, 6):                              # toes and their dark nails
            face.px(x, 0, ('bone', 0))
        face.rect(0, 2, face.w, 1, ('flesh_dark', 1))
    elif face.name == 'top':
        face.rect(0, 3, face.w, 2, ('leather', 1))        # a strap over the instep
        for x in (0, 2, 4, 6):
            face.px(x, 0, ('flesh', 1))


def eyes_rage_decal(face):
    face.rect(0, 0, face.w, face.h, SOCKET)
    for x, c in ((0, ('rage', 0)), (1, ('rage', 2)), (4, ('rage', 2)), (5, ('rage', 0))):
        face.px(x, 0, c)


# ---------------------------------------------------------------------------- maul

def haft_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        for y in range(2, face.h, 5):
            face.px(1, y, ('wood', 0))                      # grain
            face.px(1, y + 1, ('wood', 0))


def wrap_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        for y in range(0, face.h, 2):                       # wound leather strip
            face.rect(0, y, face.w, 1, ('leather_dark', 2))
            face.px(y % face.w, y, ('leather', -1))


def stone_decal(face):
    """The maul head: a hewn block. Cracks on the long faces, worn light striking faces."""
    if face.name in ('left', 'right'):                      # the striking faces (the head's x ends)
        face.rect(1, 1, face.w - 2, face.h - 2, ('stone', -2))
        face.patch(2, 2, face.w - 4, face.h - 4, ('stone', -1), 0.6, 51)
        face.line(2, 3, 5, 6, ('stone', 0))                 # a crack from the impacts
        face.px(6, 5, ('stone', 1))
        face.outline(('stone', 1))
    elif face.name in ('front', 'back'):
        face.line(1, 1, 3, 6, ('stone', 0))
        face.line(3, 6, 2, 8, ('stone', 0))
        face.line(10, 7, 11, 3, ('stone', 0))
        face.px(0, 0, ('stone', 1))
        face.px(face.w - 1, face.h - 1, ('stone', 0))
        face.rect(0, 0, face.w, 1, ('stone', -1))
    elif face.name == 'top':
        face.outline(('stone', -2), 'tb')
        face.line(2, 4, 9, 2, ('stone', 1))


def band_decal(face):
    if face.name in ('front', 'back', 'top', 'bottom'):
        face.outline(('iron_dark', 1), 'lr')
        for y in range(1, face.h, 3):
            face.px(1, y, RIVET)
            face.px(face.w - 2, y, RIVET)


def collar_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('iron', -1))
        face.px(1, 1, RIVET)


def butt_decal(face):
    if face.name == 'bottom':
        face.outline(('iron', -1))


# Item px (block-model frame: 0..16 px, +y up). The haft runs along +y with the head on top.
GRIP_R = -2                 # item y of the right fist
GRIP_L = (-12.5, -6.5)         # where the left fist may sit on the haft, just below the right one
HEAD_FROM, HEAD_TO = (2, 20, 3.5), (14, 29, 12.5)
# With this display the wrist at rest holds the haft forward out of the fist, head up front and
# the striking faces up and down: item +y -> hand -z, +x -> hand +y (down), +z -> hand -x.
MAUL_DISPLAY = {
    'thirdperson_righthand': {'rotation': [0, 90, 0], 'translation': [0, 6.5, 2], 'scale': [1, 1, 1]},
    'thirdperson_lefthand': {'rotation': [0, -90, 0], 'translation': [0, 6.5, 2], 'scale': [1, 1, 1]},
    'firstperson_righthand': {'rotation': [0, 90, 25], 'translation': [1.13, 0, 1.13], 'scale': [0.5, 0.5, 0.5]},
    'firstperson_lefthand': {'rotation': [0, -90, -25], 'translation': [1.13, 0, 1.13], 'scale': [0.5, 0.5, 0.5]},
    'gui': {'rotation': [0, 0, -45], 'translation': [0, 0, 0], 'scale': [0.36, 0.36, 0.36]},
    'ground': {'rotation': [0, 0, 0], 'translation': [0, 2, 0], 'scale': [0.3, 0.3, 0.3]},
    'fixed': {'rotation': [0, 180, -45], 'translation': [0, 0, 0], 'scale': [0.4, 0.4, 0.4]},
}


def brute_maul():
    it = Item('brute_maul', PALETTES, MAUL_DISPLAY, texture=64, name='Каменный молот громилы')
    # The haft stops 0.1 px inside the collar: their tops were one plane (z-fighting).
    it.element((6.5, -12, 6.5), (9.5, 19.9, 9.5), 'wood', texels=(3, 32, 3), decal=haft_decal)
    it.element((6.2, -11, 6.2), (9.8, 1, 9.8), 'leather', texels=(4, 12, 4), decal=wrap_decal)
    it.element((6, -14, 6), (10, -11, 10), 'iron', decal=butt_decal)
    it.element((6, 16.5, 6), (10, 20, 10), 'iron', texels=(4, 4, 4), decal=collar_decal)
    it.element(HEAD_FROM, HEAD_TO, 'stone', decal=stone_decal)
    it.element((5.6, 19.6, 3.1), (10.4, 29.4, 12.9), 'iron', texels=(5, 10, 10), decal=band_decal)
    return it


MAUL = brute_maul()


# ---------------------------------------------------------------------------- helmet

def helmet_cubes(cube):
    """The horned helmet as head-local cubes; the mob wears them and the dropped helmet is made of
    the same cubes. Stepped ox horns: out, up, then the tips forward. cube(origin, size, mat, **kw)."""
    out = [
        cube((-4, -8, -4), (8, 3, 8), 'iron', inflate=0.6, decal=cap_decal),
        cube((-4, -5, -4), (8, 1, 8), 'iron_dark', inflate=(0.9, 0.3, 0.9), decal=rim_decal),
        cube((-0.5, -5, -5.1), (1, 3, 1), 'iron', decal=nasal_decal),
        cube((-1, -10, -1), (2, 2, 2), 'iron', decal=knob_decal),
    ]
    for side in (-1, 1):  # -1: the mob's right (-x)
        def at(x0, w):
            return x0 if side < 0 else -x0 - w
        out += [
            cube((at(-7.4, 3), -7.5, -1.5), (3, 3, 3), 'horn', decal=horn_base),
            # 0.1 px proud in z: its front and back met the base segment's in one plane.
            cube((at(-9.9, 3), -10, -1.5), (3, 3, 3), 'horn', inflate=(0, 0, 0.1), decal=horn_mid),
            cube((at(-10.4, 2), -14, -2.5), (2, 4, 3), 'horn', decal=horn_up),
            cube((at(-10.4, 2), -16, -4.5), (2, 2, 2), 'horn', decal=horn_tip),
        ]
    return out


def brute_helmet():
    from rig import Cube
    it = Item('brute_helmet', PALETTES, HELMET, texture=64, name='Рогатый шлем громилы')
    it.from_head_cubes(helmet_cubes(lambda o, s, mat, **kw: Cube(o, s, mat, **kw)))
    return it


ITEMS = [MAUL, brute_helmet()]


# ---------------------------------------------------------------------------- model

FIST_Y = 21          # fist centre below the shoulder pivot, px
ELBOW_Y = 9          # the elbow pivot below the shoulder pivot, where the upper arm cube ends
EYE_Z = -4.05        # eyes_rage sits a hair in front of the face, over the eye sockets
_M = {}              # the model and the maul socket, for the pose solvers below


def build_body(m):
    body = m.part('body', (0, 8, 0), parent='root')
    body.cube((-8, -7, -4.5), (16, 8, 9), 'flesh', decal=belly_decal)
    # 0.1 px taller each way: at rest its bottom met the bottom of the shoulders in one plane.
    body.cube((-9, -18, -5.5), (18, 11, 11), 'flesh', inflate=(0, 0.1, 0), decal=chest_decal)
    body.cube((-7, -21, -2.5), (14, 3, 8), 'flesh', inflate=(0, 0, -0.1), decal=traps_decal)   # back 0.1 px inside the chest's
    body.cube((-8, -1, -4.5), (16, 2, 9), 'leather_dark', inflate=0.45, decal=belt_decal)
    body.cube((-5, -15, -6.2), (10, 14, 1), 'leather', decal=bib_decal)
    knob = body.cube((-1, -16, 5.5), (2, 2, 1), 'bone', decal=spine_decal)
    body.cube((-1, -12, 5.5), (2, 2, 1), 'bone', share=knob)
    body.cube((-1, -8, 5.5), (2, 2, 1), 'bone', share=knob)
    body.cube((8.2, -0.5, -1.5), (3, 3, 3), 'bone', decal=skull_decal)
    # One pauldron, on the left shoulder; it sits on the body so the arm turns under it.
    body.cube((8, -21, -4.5), (8, 4, 9), 'leather', inflate=0.3, decal=pauldron_decal)
    body.cube((11, -23, -1), (2, 2, 2), 'iron', decal=spike_decal)
    apron = m.part('apron', (0, 8, -5.3), parent='body')
    apron.cube((-6, 0, -0.5), (12, 13, 1), 'leather', decal=apron_decal)


def build_head(m):
    head = m.part('head', (0, -11, -3.5), parent='body')
    head.cube((-4, -8, -4), (8, 8, 8), 'flesh', decal=head_decal)
    head.cube((-4.5, -2, -5), (9, 3, 6), 'flesh_dark', decal=jaw_decal)
    t = head.cube((-3.5, -3, -5.5), (1, 1, 1), 'bone', decal=tusk_decal)
    head.cube((2.5, -3, -5.5), (1, 1, 1), 'bone', share=t)
    helmet_cubes(head.cube)
    eyes = m.part('eyes_rage', (0, -11, -3.5), parent='head', glow=True, state='rage')
    eyes.cube((-3, -3, EYE_Z), (6, 1, 0), 'socket', decal=eyes_rage_decal, faces=('front',), grain=0)


def build_arms(m):
    """Two bones per arm: the upper arm (arm_r, from the shoulder) and the forearm with the fist
    (forearm_r, from the elbow 9 px below the shoulder pivot). The elbow bends the forearm about
    its own x: x < 0 brings the fist forward and up, as a limb swings. The wrist hand_r sits in the
    middle of the fist, under the forearm. The left arm mirrors the right one."""
    arm_r = m.part('arm_r', (-12, -8, 0), parent='body')
    a = arm_r.cube((-3.5, -3, -3), (7, 12, 6), 'flesh', decal=arm_decal)
    arm_r.cube((-3.5, -3, -3), (7, 3, 6), 'flesh', inflate=0.3, decal=shoulder_decal)   # the left one is under the pauldron
    fore_r = m.part('forearm_r', (-12, -8 + ELBOW_Y, 0), parent='arm_r')
    b = fore_r.cube((-4, 0, -3.5), (8, 10, 7), 'flesh', decal=forearm_decal)
    c = fore_r.cube((-4, 2, -3.5), (8, 6, 7), 'leather', inflate=0.4, decal=bracer_decal)
    d = fore_r.cube((-3.5, 10, -3.5), (7, 5, 7), 'flesh', decal=fist_decal)   # 5 tall: room for a haft held low in it
    m.part('hand_r', (-12, -8 + FIST_Y, 0), parent='forearm_r')   # the wrist: turns the maul in the fist

    arm_l = m.part('arm_l', (12, -8, 0), parent='body')
    arm_l.cube((-3.5, -3, -3), (7, 12, 6), 'flesh', share=a, mirror=True)
    fore_l = m.part('forearm_l', (12, -8 + ELBOW_Y, 0), parent='arm_l')
    fore_l.cube((-4, 0, -3.5), (8, 10, 7), 'flesh', share=b, mirror=True)
    fore_l.cube((-4, 2, -3.5), (8, 6, 7), 'leather', inflate=0.4, share=c, mirror=True)
    fore_l.cube((-3.5, 10, -3.5), (7, 5, 7), 'flesh', share=d, mirror=True)


def build_legs(m):
    leg_r = m.part('leg_r', (-4.5, 8, 0), parent='root')
    e = leg_r.cube((-3, 0, -3), (6, 13, 6), 'cloth', decal=leg_decal)
    f = leg_r.cube((-3.5, 13, -5), (7, 3, 8), 'flesh', decal=foot_decal)
    leg_l = m.part('leg_l', (4.5, 8, 0), parent='root')
    leg_l.cube((-3, 0, -3), (6, 13, 6), 'cloth', share=e, mirror=True)
    leg_l.cube((-3.5, 13, -5), (7, 3, 8), 'flesh', share=f, mirror=True)


def model(m):
    m.part('root', (0, 24, 0))
    build_body(m)
    build_head(m)
    build_arms(m)
    build_legs(m)
    _M['model'] = m
    _M['socket'] = m.socket('socket_hand_r', 'hand_r', (0, 2, -3.5))   # centre of the fist bottom, front
    m.socket('socket_hand_l', 'forearm_l', (0, FIST_Y - ELBOW_Y + 2, -3.5))
    m.socket('socket_head', 'head', (0, -8.6, 0))
    m.socket('socket_back', 'body', (0, -12, 6.5))
    m.hold('socket_hand_r', MAUL)


# ---------------------------------------------------------------------------- pose solvers

GROUND_Y = 24.0      # model-frame y of the soles at rest


def local(part, j):
    m = rig.rot_game(*(math.radians(a + b) for a, b in zip(part.rot, j.rot))).to_4x4()
    m.translation = part.local_rest() + Vector(j.move)
    return m


def world(p, name):
    """4x4 of a part in the model frame (px) for pose p."""
    parts = _M['model'].parts
    chain = []
    while name:
        chain.append(parts[name])
        name = parts[name].parent
    out = local(chain[-1], p.get(chain[-1].name, J()))
    for part in reversed(chain[:-1]):
        out = out @ local(part, p.get(part.name, J()))
    return out


def maul_point(p, pt):
    """An item-space point of the maul (px) in the model frame, through the socket and display."""
    s = _M['socket'].matrix() @ items.held_matrix(MAUL.display, 'right')
    return world(p, 'hand_r') @ (s @ Vector([c / 16 for c in pt]) * 16)


def haft_rest_dir():
    s = _M['socket'].matrix().to_3x3() @ items.held_matrix(MAUL.display, 'right').to_3x3()
    return (s @ Vector((0, 1, 0))).normalized()


def aim(p, d, face=None):
    """Wrist rotation that points the haft (butt to head) along direction d of the model frame.
    face: a model-frame direction the maul head's top (item +z, the face of its band rivets)
    turns to as far as the haft allows; without it, the smallest turn from the rest wrist."""
    fore = world(p, 'forearm_r').to_3x3()
    e = (fore.transposed() @ Vector(d)).normalized()
    if face is None:
        q = haft_rest_dir().rotation_difference(e).to_matrix()
    else:
        s = _M['socket'].matrix().to_3x3() @ items.held_matrix(MAUL.display, 'right').to_3x3()
        y0, z0 = haft_rest_dir(), (s @ Vector((0, 0, 1))).normalized()
        z1 = fore.transposed() @ Vector(face)
        z1 = (z1 - e * z1.dot(e)).normalized()
        a = Matrix((y0.cross(z0), y0, z0)).transposed()
        b = Matrix((e.cross(z1), e, z1)).transposed()
        q = b @ a.transposed()
    return R(*(math.degrees(a) for a in rig.continuous_zyx(q, (0, 0, 0))))


def slide(p, grip):
    """Moves the wrist so the fist holds the haft at item y `grip` instead of GRIP_R (lower on the
    leather wrap, say): a move of hand_r along the aimed haft, in the forearm's frame."""
    q = dict(p)
    e = maul_point(q, (8, grip, 8)) - maul_point(q, (8, GRIP_R, 8))
    fore = world(q, 'forearm_r').to_3x3()
    q['hand_r'] = q.get('hand_r', J()) + M(*(fore.transposed() @ -e))
    return q


def ground(p):
    """Moves the root up or down so the lower sole stands on the ground."""
    low = -1e9
    for leg in ('leg_r', 'leg_l'):
        w = world(p, leg)
        for x in (-3.5, 3.5):
            for z in (-5, 3):
                low = max(low, (w @ Vector((x, 16, z))).y)
    root = p.get('root', J())
    return J(root.rot, (root.move[0], root.move[1] + GROUND_Y - low, root.move[2]))


UPPER = ELBOW_Y                 # shoulder pivot to elbow pivot, px
LOWER = FIST_Y - ELBOW_Y        # elbow pivot to fist centre
MIN_REACH = 9                   # the fist no closer to the shoulder: a tighter fold digs the arm into itself
# Where the elbow points while the arm reaches (body frame: +x the mob's left, +y down, +z back):
# out to the side, down and a little back, as a heavy arm hangs.
ELBOW_POLE = {'r': (-0.8, 0.5, 0.35), 'l': (0.8, 0.5, 0.35)}


def limb(p, side, target, pole=None, ref=(0, 0, 0)):
    """Two-bone reach: the upper arm and the forearm of side 'r' or 'l' that put the fist centre
    on `target` (model frame, px), the elbow bent towards `pole` (body frame; ELBOW_POLE by
    default). The elbow is a hinge: the forearm turns about its own x, forward (x < 0), so the
    upper arm takes the twist that lays the bend into the plane of the elbow. ref: the arm angles
    (deg) the solution should stay close to. Returns (arm J, forearm J, miss): miss is how far
    the fist stays short of the target (px, + out of reach, - closer than MIN_REACH allows)."""
    body = world(p, 'body').to_3x3()
    shoulder = world(p, 'arm_' + side).translation
    t = Vector(target) - shoulder
    d = min(max(t.length, MIN_REACH), UPPER + LOWER - 1e-6)
    miss = t.length - d
    axis = t.normalized()
    n = body @ Vector(pole or ELBOW_POLE[side])
    n = (n - axis * n.dot(axis)).normalized()
    cos_a = (UPPER ** 2 + d ** 2 - LOWER ** 2) / (2 * UPPER * d)
    a = math.acos(max(-1.0, min(1.0, cos_a)))
    up = axis * math.cos(a) + n * math.sin(a)                   # shoulder to elbow
    fore = (shoulder + axis * d - (shoulder + up * UPPER)).normalized()
    flex = math.acos(max(-1.0, min(1.0, up.dot(fore))))
    front = -(n - up * n.dot(up)).normalized()                  # the arm's -z: where the fist folds to
    y, z = up, -front
    arm = body.transposed() @ Matrix((y.cross(z), y, z)).transposed()
    ang = rig.continuous_zyx(arm, tuple(math.radians(v) for v in ref))
    return R(*(math.degrees(v) for v in ang)), R(-math.degrees(flex), 0, 0), miss


def forearm_dir(p, side):
    """Elbow to fist, model frame (unit)."""
    w = world(p, 'forearm_' + side)
    return (w.to_3x3() @ Vector((0, 1, 0))).normalized()


REPORT = []
# Swivels of the left elbow tried around ELBOW_POLE (deg about the shoulder-to-fist line): the
# one that lays the forearm most square to the haft, so the haft runs through the fist and not
# through the forearm above it.
SWIVELS = (0, -15, 15, -30, 30, -45, 45)


def reach(p):
    """Left fist onto the haft: the point of GRIP_L nearest its middle that the arm reaches, then
    the elbow swivel that lays the forearm most square to the haft. Returns the arm, the forearm
    and how far the fist ends up from the haft (px, + short of it)."""
    shoulder = world(p, 'arm_l').translation
    a, b = (maul_point(p, (8, y, 8)) for y in GRIP_L)
    haft = (b - a).normalized()
    ts = sorted((i / 20 for i in range(21)), key=lambda v: (abs(v - 0.5), v))
    ok = [t for t in ts if MIN_REACH <= (a + (b - a) * t - shoulder).length <= UPPER + LOWER]
    t = ok[0] if ok else min(ts, key=lambda v: abs((a + (b - a) * v - shoulder).length - (UPPER + LOWER)))
    target = a + (b - a) * t
    body = world(p, 'body').to_3x3()
    axis = (target - shoulder).normalized()
    r = p.get('arm_r', J()).rot
    best = None
    for sw in SWIVELS:
        pole = body.transposed() @ (Matrix.Rotation(math.radians(sw), 3, axis) @ (body @ Vector(ELBOW_POLE['l'])))
        arm, fore, miss = limb(p, 'l', target, tuple(pole), (r[0], -r[1], -r[2]))
        q = dict(p, arm_l=arm, forearm_l=fore)
        score = abs(forearm_dir(q, 'l').dot(haft)) + 0.002 * abs(sw)
        if best is None or score < best[0] - 1e-9:
            best = (score, arm, fore, miss)
    return best[1], best[2], best[3]


# Right-arm sideways turns tried, smallest first, until the left fist reaches the haft.
GRIP_TURNS = sorted(range(-48, 31, 3), key=lambda dz: (abs(dz), dz))
GRIP_TOLERANCE = 0.5


def two_hands(q, haft, label, sit=None):
    """Both hands on the haft: turns the right arm in (or out) as little as it takes for the left
    fist to land on the grip, re-aiming the maul each time. Fills hand_r, arm_l and forearm_l."""
    base = q.get('arm_r', J())
    best = None
    for dz in GRIP_TURNS:
        t = dict(q)
        t['arm_r'] = base + R(0, 0, dz)
        t['hand_r'] = aim(t, haft)
        if sit is not None:
            t = seat(t, *sit)
        t['arm_l'], t['forearm_l'], miss = reach(t)
        if abs(miss) <= GRIP_TOLERANCE:
            return t
        if best is None or abs(miss) < abs(best[1]):
            best = (t, miss)
    REPORT.append('%s: left fist %.1f px off the haft' % (label, best[1]))
    return best[0]


def drape(p):
    """The apron hangs: it takes back most of the body's lean and lifts off a forward thigh."""
    lean = p.get('body', J()).rot[0]
    thigh = min(p.get('leg_r', J()).rot[0], p.get('leg_l', J()).rot[0], 0)
    return R(-0.85 * lean + 0.8 * thigh, 0, 0)


def seat(q, low, side):
    """Seats the haft lower in the right fist (towards its bottom, px) and to one side of it, so a
    haft not quite square to the forearm clears the forearm's end."""
    q = dict(q)
    fore = world(q, 'forearm_r').to_3x3()
    f = fore @ Vector((0, 1, 0))
    d = (maul_point(q, (8, 20, 8)) - maul_point(q, (8, 0, 8))).normalized()
    q['hand_r'] = q.get('hand_r', J()) + M(*(fore.transposed() @ (f * low + f.cross(d).normalized() * side)))
    return q


def fin(p, label, haft=None, grip=True, feet=True, face=None, hold=None, sit=None):
    """A finished key pose: feet on the ground, the maul aimed, the left fist on the haft.
    face: where the maul head's top turns (aim); hold: the item y the right fist holds (slide);
    sit: (low, side) where the haft sits in the right fist (seat)."""
    q = dict(p)
    if feet:
        q['root'] = ground(q)
    if grip:
        q = two_hands(q, haft, label, sit)
    elif haft is not None:
        q['hand_r'] = aim(q, haft, face)
    if hold is not None:
        q = slide(q, hold)
    if sit is not None and not grip:
        q = seat(q, *sit)
    q['apron'] = drape(q)
    return q


def head_height(p):
    """Height of the lowest corner of the maul head above the ground, px."""
    lo, hi = HEAD_FROM, HEAD_TO
    ys = [maul_point(p, (x, y, z)).y for x in (lo[0], hi[0]) for y in (lo[1], hi[1]) for z in (lo[2], hi[2])]
    return GROUND_Y - max(ys)


def boxes(p):
    """The mob's cubes as boxes in the model frame for pose p: (label, centre, axes, half sizes).
    Parts under a state flag (the flat rage eyes) are left out."""
    out = []
    for part in _M['model'].parts.values():
        if part.state:
            continue
        w = world(p, part.name)
        for i, c in enumerate(part.cubes):
            lo = [c.origin[k] - c.inflate[k] for k in range(3)]
            hi = [c.origin[k] + c.size[k] + c.inflate[k] for k in range(3)]
            mid = Vector([(a + b) / 2 for a, b in zip(lo, hi)])
            out.append(('%s#%d' % (part.name, i), w @ mid, [w.to_3x3().col[k].normalized() for k in range(3)],
                        [(b - a) / 2 for a, b in zip(lo, hi)]))
    return out


def maul_boxes(p):
    """The maul's elements as boxes in the model frame (same tuple as boxes)."""
    out = []
    for i, e in enumerate(MAUL.elements):
        mid = Vector([(a + b) / 2 for a, b in zip(e.frm, e.to)])
        c = maul_point(p, mid)
        axes = [(maul_point(p, mid + Vector([1.0 if k == j else 0.0 for k in range(3)])) - c).normalized() for j in range(3)]
        out.append(('maul#%d' % i, c, axes, [(b - a) / 2 for a, b in zip(e.frm, e.to)]))
    return out


def depth(a, b):
    """How deep two boxes cut into each other (px, separating axis test); <= 0 when apart."""
    axes = list(a[2]) + list(b[2]) + [u.cross(w) for u in a[2] for w in b[2]]
    t = b[1] - a[1]
    best = 1e9
    for ax in axes:
        if ax.length < 1e-6:
            continue
        ax = ax.normalized()
        ra = sum(h * abs(u.dot(ax)) for h, u in zip(a[3], a[2]))
        rb = sum(h * abs(u.dot(ax)) for h, u in zip(b[3], b[2]))
        best = min(best, ra + rb - abs(t.dot(ax)))
        if best <= 0:
            return best
    return best


# The haft and its leather wrap run through the right fist (forearm_r cube 2): that is the grip.
GRIP_CUBES = {('forearm_r#2', 'maul#0'), ('forearm_r#2', 'maul#1')}


def clash(p, allow=GRIP_CUBES, tol=0.02):
    """Where the maul cuts into the mob in pose p: [(cube, maul element, depth px)] deeper than
    tol, apart from the pairs in allow."""
    out = []
    for m in maul_boxes(p):
        for c in boxes(p):
            if (c[0], m[0]) in allow:
                continue
            d = depth(c, m)
            if d > tol:
                out.append((c[0], m[0], round(d, 2)))
    return out


# ---------------------------------------------------------------------------- poses

# Directions in the model frame: +x the mob's left, +y down, -z forward.
FWD, BACK, DOWN, UP, RIGHT, LEFT = (0, 0, -1), (0, 0, 1), (0, 1, 0), (0, -1, 0), (-1, 0, 0), (1, 0, 0)


def v(*parts):
    """A direction as a weighted sum: v((FWD, 2), (DOWN, 1))."""
    out = Vector((0, 0, 0))
    for d, w in parts:
        out += Vector(d) * w
    return tuple(out.normalized())


def d(x, y, z):
    """A direction from its model-frame components (normalised)."""
    return tuple(Vector((x, y, z)).normalized())


# The keys below were laid out by hand and then settled with a search over the right arm, the
# elbow, a few degrees of haft and the seat of the haft in the fist, against the maul's boxes
# (clash): the maul stays out of the body, the forearm lies square to the haft where the fist
# holds it, and the maul head and the fist stay within a few pixels of where the key had them,
# so the attacks read as before. The numbers are the search's, rounded.

# Rest stance: hunched, head pushed forward, the maul held low across the thighs in both hands
# in front of the belly, its head hanging by the right knee; the elbows a little bent.
STANCE = dict(body=R(16, 0, 0), head=R(-14, 0, 0), arm_r=R(-26, 0, -9), forearm_r=R(-12, 0, 0),
              leg_r=R(6, 0, 4), leg_l=R(-8, 0, -4))
STANCE_HAFT = d(-0.980, 0.172, -0.101)
STANCE_SIT = (0.5, -0.1)


def stance(label):
    return fin(STANCE, label, STANCE_HAFT, sit=STANCE_SIT)


def attack_swing(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    # Wound to its right like a batter: both fists by the right shoulder, the maul head out to
    # the side and back, the left shoulder turned in, the elbows cocked. Then a flat sweep across
    # the front on straight arms, and the arms fold round after it.
    wound = dict(body=R(8, 54, 0), head=R(-12, -46, 0), arm_r=R(-60, -10, -7), forearm_r=R(-35, 0, 0),
                 leg_r=R(12, 14, 4), leg_l=R(-14, 14, -4))
    peak = dict(pose(wound, body=R(-2, 8, 0)), arm_r=R(-73, 0, 11), forearm_r=R(-40, 0, 0))
    cut = dict(body=R(18, -30, -2), head=R(-16, 26, 0), arm_r=R(-56, 13, -5),
               leg_r=R(14, -12, 4), leg_l=R(-22, -12, -4))
    through = dict(body=R(20, -64, -4), head=R(-16, 50, 0), arm_r=R(-22, 27, -73), forearm_r=R(-86, 0, 0),
                   leg_r=R(14, -20, 4), leg_l=R(-22, -20, -4))
    hang = pose(through, body=R(2, 4, 0))
    a.key(0, stance('swing 0'), 'out')
    a.key(hit - 5, fin(wound, 'swing wound', d(-0.623, -0.241, 0.744), sit=(0.9, 0.1)), 'inout')   # the tell
    a.key(hit - 2, fin(peak, 'swing peak', d(-0.633, -0.266, 0.727), sit=(0.2, 0.5)), 'in')
    a.key(hit, fin(cut, 'swing cut', d(0.187, -0.164, -0.969), sit=(1.2, -0.5)), 'out')
    a.key(rec, fin(through, 'swing through', d(0.884, -0.004, 0.468), sit=(1.2, -0.5)), 'hold')   # overbalanced: the window
    a.key(rec + 5, fin(hang, 'swing hang', d(0.884, 0.04, 0.468), sit=(1.2, -0.5)), 'slow')
    a.key(end, stance('swing end'))


def slam_poses():
    gather = dict(body=R(28, 8, 0), head=R(-24, -6, 0), arm_r=R(33, 19, 34), forearm_r=R(-93, 0, 0),
                  leg_r=R(10, 0, 4), leg_l=R(-14, 0, -4))
    raised = dict(body=R(-12, 0, 0), head=R(-34, 0, 0), arm_r=R(-137, -34, -17), forearm_r=R(-10, 0, 0),
                  leg_r=R(12, 0, 4), leg_l=R(-16, 0, -4))
    slam = dict(body=R(44, 0, 0), head=R(-34, 0, 0), arm_r=R(-57, -12, -6), leg_r=R(20, 0, 5), leg_l=R(-30, 0, -5))
    return gather, raised, slam


def attack_slam(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    gather, raised, slam = slam_poses()
    peak = dict(pose(raised, body=R(-5, 0, 0), head=R(-4, 0, 0)), arm_r=R(-146, -30, -14), forearm_r=R(-13, 0, 0))
    stuck = dict(pose(slam, body=R(3, 0, 0) + M(0, 0.4, 0)), arm_r=R(-55, -2, 2))
    a.key(0, stance('slam 0'), 'out')
    a.key(4, fin(gather, 'slam gather', d(-0.386, 0.262, 0.884), sit=(0.3, 0)), 'inout')
    a.key(11, fin(raised, 'slam raised', d(-0.072, -0.581, 0.811), sit=(0.7, -0.8)), 'inout')   # maul over the head, a roar
    a.key(hit - 2, fin(peak, 'slam peak', d(-0.095, -0.488, 0.868), sit=(1.2, -1.1)), 'in')
    a.key(hit, fin(slam, 'slam hit', d(-0.135, 0.079, -0.988), sit=(1.2, 0)), 'out')
    a.key(rec, fin(stuck, 'slam stuck', d(-0.147, 0.186, -0.971), sit=(1.2, 0)), 'hold')    # head bitten into the ground
    a.key(rec + 6, fin(dict(pose(stuck, body=R(-4, -4, 0), head=R(-6, 10, 0)), arm_r=R(-57, -8, 1)), 'slam tug',
                       d(-0.063, 0.112, -0.992), sit=(1.2, -0.2)), 'inout')
    a.key(rec + 12, fin(dict(pose(stuck, body=R(-2, 5, 0), head=R(-4, -8, 0)), arm_r=R(-58, -20, -5)), 'slam tug2',
                        d(-0.106, 0.102, -0.989), sit=(1.2, -0.7)), 'inout')
    a.key(rec + 16, fin(dict(pose(stuck, body=R(-10, 0, 0)), arm_r=R(-56, -16, -15)), 'slam yank',
                        d(-0.122, -0.009, -0.993), sit=(1.2, -0.3)), 'slow')
    a.key(end, stance('slam end'))


# The maul trails one-handed behind the charge: the right elbow bent, the haft square to it.
TRAIL = d(-0.094, 0.182, 0.979)
TRAIL_SIT = (1.2, -0.8)


def charge_poses():
    paw = dict(body=R(30, 0, 0), head=R(26, 0, 0), arm_r=R(18, 22, 6), forearm_r=R(-39, 0, 0),
               arm_l=R(14, 0, -12), forearm_l=R(-20, 0, 0), leg_r=R(34, 0, 4), leg_l=R(-12, 0, -4))
    crouch = dict(body=R(40, 0, 0), head=R(30, 0, 0), arm_r=R(32, 14, 17), forearm_r=R(-53, 0, 0),
                  arm_l=R(30, 0, -12), forearm_l=R(-30, 0, 0), leg_r=R(24, 0, 4), leg_l=R(-20, 0, -4))
    run = dict(body=R(44, 0, 0), head=R(32, 0, 0), arm_r=R(30, 9, 13), forearm_r=R(-60, 0, 0))
    impact = dict(body=R(-14, 0, 0), head=R(-38, 0, 0), arm_r=R(-30, -2, 36), forearm_r=R(-12, 0, 0),
                  arm_l=R(-46, 0, -40), leg_r=R(-10, 0, 6), leg_l=R(6, 0, -6), root=M(0, 0, 3))
    dazed = dict(body=R(28, 0, -7), head=R(30, 14, 14), arm_r=R(8, 0, 3), arm_l=R(8, 0, -2), forearm_l=R(-15, 0, 0),
                 leg_r=R(12, 0, 8), leg_l=R(-4, 0, -8), root=M(0, 0, 3))
    return paw, crouch, run, impact, dazed


def attack_charge(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    paw, crouch, run, impact, dazed = charge_poses()
    drop = d(-0.284, 0.610, -0.740)                          # the maul head drops to the ground
    drop_sit = (0.6, 0)
    a.key(0, stance('charge 0'), 'out')
    a.key(3, fin(paw, 'paw 1', d(-0.203, 0.174, 0.964), grip=False, sit=(1.2, -1.3)), 'inout')    # paws the ground
    a.key(6, fin(dict(pose(paw, leg_r=R(-34, 0, 0), body=R(4, 0, 0)), arm_r=R(30, 11, 14), forearm_r=R(-55, 0, 0)),
                 'scrape 1', d(-0.051, 0.235, 0.971), grip=False, sit=(1.2, -0.8)), 'inout')
    a.key(9, fin(paw, 'paw 2', d(-0.203, 0.174, 0.964), grip=False, sit=(1.2, -1.3)), 'inout')
    a.key(hit - 2, fin(crouch, 'crouch', d(-0.029, 0.103, 0.994), grip=False, sit=(1.2, -1.1)), 'in')   # horns lowered
    for i, f in enumerate(range(hit, rec, 2)):
        s = 1 if i % 2 == 0 else -1                           # the left arm pumps, bent as a runner's
        a.key(f, fin(pose(run, leg_l=R(-36 * s, 0, -4), leg_r=R(36 * s, 0, 4), arm_l=R(-40 * s, 0, -14),
                          forearm_l=R(-55 + 15 * s, 0, 0), arm_r=R(4 * s, 0, 0)), 'run', TRAIL, grip=False, sit=TRAIL_SIT),
              'linear')
    a.key(rec, fin(impact, 'impact', d(-0.916, -0.276, 0.291), grip=False, feet=False), 'out')
    a.key(rec + 5, fin(pose(dazed, body=R(-14, 0, 12), head=R(-20, 0, -20)), 'stagger', drop, grip=False, sit=drop_sit),
          'inout')
    for i, f in enumerate(range(rec + 11, end - 6, 7)):  # dazed: head lolling, body swaying
        s = 1 if i % 2 == 0 else -1
        a.key(f, fin(pose(dazed, body=R(0, 0, 9 * s), head=R(0, -16 * s, -14 * s)), 'dazed', drop, grip=False, sit=drop_sit),
              'inout')
    a.key(end - 4, fin(pose(dazed, body=R(-14, 0, 0), head=R(-18, 26, 0)), 'shake', drop, grip=False, sit=drop_sit), 'inout')
    a.key(end, stance('charge end'))


def charge_stop(c):
    """No wall: the dash ends in a skid, a short recovery (the game picks this clip)."""
    skid = dict(body=R(-6, 0, 0), head=R(-10, 0, 0), arm_r=R(-1, 3, 24), arm_l=R(-30, 0, -30), forearm_l=R(-20, 0, 0),
                leg_r=R(20, 0, 6), leg_l=R(-34, 0, -6))
    k = c.clip('charge_stop', 16)
    k.key(0, fin(dict(body=R(44, 0, 0), head=R(32, 0, 0), arm_r=R(30, 9, 13), forearm_r=R(-60, 0, 0),
                      leg_l=R(-36, 0, -4), leg_r=R(36, 0, 4)), 'stop 0', TRAIL, grip=False, sit=TRAIL_SIT), 'out')
    k.key(4, fin(skid, 'skid', d(-0.205, 0.236, -0.950), grip=False, sit=(1.2, 0)), 'slow')
    k.key(16, stance('stop end'))


def rage(c):
    """Below 30% health: a roar. The red eyes are the rage state flag, not this clip."""
    gather = dict(pose(STANCE, body=R(14, 0, 0), head=R(16, 0, 0)), arm_r=R(-29, 17, -8), forearm_r=R(0, 0, 0))
    # A gorilla roar: chest forward, arms flung wide, the left fist clenched up at the elbow,
    # the face to the enemy so the eyes show.
    roar = dict(body=R(4, 0, 0), head=R(-12, 0, 0), arm_r=R(-99, 3, 71), arm_l=R(-30, 0, -70), forearm_l=R(-45, 0, 0),
                leg_r=R(10, 0, 6), leg_l=R(-14, 0, -6))
    roar_haft, roar_sit = d(-0.669, -0.743, -0.006), (0.1, 0)
    k = c.clip('rage', 34)
    k.key(0, stance('rage 0'), 'out')
    k.key(5, fin(gather, 'rage gather', d(-0.985, 0.170, -0.025), sit=(0, -0.3)), 'out')
    k.key(12, fin(roar, 'roar', roar_haft, grip=False, sit=roar_sit), 'inout')
    k.key(18, fin(pose(roar, head=R(-4, 12, 0), body=R(2, 0, 0)), 'roar shake', roar_haft, grip=False, sit=roar_sit), 'inout')
    k.key(24, fin(pose(roar, head=R(-2, -12, 0)), 'roar shake 2', roar_haft, grip=False, sit=roar_sit), 'slow')
    k.key(34, stance('rage end'))


def idle_walk(c):
    idle = c.clip('idle', 40, loop=True)
    idle.key(0, stance('idle 0'))
    idle.key(20, fin(pose(STANCE, body=R(-2, 0, 0) + M(0, -0.4, 0), head=R(3, 0, 0), forearm_r=R(-3, 0, 0)), 'idle 20',
                     STANCE_HAFT, sit=STANCE_SIT))
    idle.key(40, stance('idle 0'))

    walk = c.clip('walk', 20, loop=True)   # one heavy step cycle; the game drives it by walk_position
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        step = pose(STANCE, leg_l=R(-20 * s, 0, 0), leg_r=R(20 * s, 0, 0), body=R(2 * abs(s), 4 * s, 3 * s),
                    head=R(0, -4 * s, -3 * s), forearm_r=R(-4 * abs(s), 0, 0))
        walk.key(f, fin(step, 'walk %d' % f, STANCE_HAFT, sit=STANCE_SIT))


def hurt_death(c):
    hurt = c.clip('hurt', 10)   # vanilla hurt_time; heavy, so it barely rocks
    hurt.key(0, stance('hurt 0'), 'out')
    hurt.key(2, fin(dict(pose(STANCE, body=R(-8, 0, 3), head=R(-18, 10, 0)), arm_r=R(-30, 0, -8), forearm_r=R(0, 0, 0)),
                    'hurt 2', d(-0.971, 0.171, -0.165), sit=(0.9, 0)), 'slow')
    hurt.key(10, stance('hurt end'))

    death = c.clip('death', 20)  # vanilla death_time; the left hand lets go, the maul drops
    sag = dict(body=R(34, 0, 6), head=R(36, -10, 0), arm_r=R(16, 0, 17), arm_l=R(14, 0, -24), forearm_l=R(-10, 0, 0),
               leg_r=R(12, 0, 4), leg_l=R(-6, 0, -4))
    fall = dict(sag, arm_r=R(3, -14, 21), forearm_r=R(-10, 0, 0))
    death.key(0, stance('death 0'), 'out')
    death.key(6, fin(sag, 'death sag', d(-0.331, 0.758, -0.562), grip=False, sit=(0.3, 0)), 'in')
    death.key(18, fin(pose(fall, root=R(0, 0, 88)), 'death fall', d(-0.329, 0.775, -0.540), grip=False, feet=False,
                      sit=(0.9, -0.6)), 'back')
    death.key(20, fin(pose(fall, root=R(0, 0, 90), arm_r=R(-1, -5, 2)), 'death down', d(-0.341, 0.775, -0.532), grip=False,
                      feet=False, sit=(0.9, -1.1)))


# The maul on the shoulder (trophy pose), round 9: the head rests on the right shoulder and the
# haft runs forward and down from it, across the chest, to the right fist, which holds it near
# the middle. Body frame, from the right shoulder pivot: the haft's axis at item y
# CARRY['rest'] (in the head) sits `lift` px above the pivot, shifted dx, dz (px) across the
# shoulder; lift leaves about 0.15 px between the head and the shoulder cap under it
# (trophy_clip checks that the head rests: dropped REST_PROBE px straight down, it cuts the
# shoulder and nothing else). yaw turns the haft's run (butt to head) from straight back
# towards the mob's right (deg), pitch lifts it (deg). The right fist holds the haft at item y
# CARRY['grip'], just above the leather wrap: the wrist part hand_r slides along the haft
# (slide()), and only this clip keys that, so the attack clips keep their grip (GRIP_R and
# seat()). low and side seat the haft low in the fist and to one side of it (px), so its tilt
# against the forearm clears the forearm's end; face is where the head's top (item +z) turns.
# Found by a search over these numbers: no clash on any tick, the head on the shoulder cap, the
# haft 48 deg below level, the figure at 0.247; the forearm is 15 deg off square to the haft
# (a grip this far from the head needs the arm almost straight).
CARRY = dict(yaw=24.6, pitch=28.2, rest=18.4, grip=5.7, dx=-2.2, dz=-3.5, face='left', low=1.43, side=-0.54, lift=7.01)
CARRY_FACES = {'up': (0, -1, 0), 'back': (0, 0, 1), 'right': (-1, 0, 0), 'left': (1, 0, 0), 'fwd': (0, 0, -1),
               'down': (0, 1, 0)}
MAUL_HEAD = ('maul#3', 'maul#4', 'maul#5')   # collar, stone, iron band
REST_PROBE = 0.4   # px: the maul head dropped this far must cut the shoulder (it rests on it)


def carry(p, k=CARRY):
    """The right arm and wrist that rest the maul's head on the right shoulder in pose p (see
    CARRY): the fist on the haft where the arm reaches, the elbow swivelled until the forearm
    lies as square to the haft as it gets, so the haft crosses the fist without cutting the
    forearm."""
    body = world(p, 'body').to_3x3()
    shoulder = world(p, 'arm_r').translation
    yaw, pitch = math.radians(k['yaw']), math.radians(k['pitch'])
    d = (body @ Vector((-math.sin(yaw) * math.cos(pitch), -math.sin(pitch), math.cos(yaw) * math.cos(pitch)))).normalized()
    rest = shoulder + body @ Vector((k['dx'], -k['lift'], k['dz']))
    fist = rest - d * (k['rest'] - k['grip'])
    axis = (fist - shoulder).normalized()
    best = None
    for sw in range(0, 360, 6):
        pole = body.transposed() @ (Matrix.Rotation(math.radians(sw), 3, axis) @ (body @ Vector(ELBOW_POLE['r'])))
        arm, fore, miss = limb(p, 'r', fist, tuple(pole))
        assert abs(miss) < 1e-6, 'carry: the fist is %.1f px out of reach' % miss
        q = dict(p, arm_r=arm, forearm_r=fore)
        score = abs(forearm_dir(q, 'r').dot(d))
        if best is None or score < best[0] - 1e-9:
            best = (score, q)
    q = best[1]
    q['hand_r'] = aim(q, d, CARRY_FACES[k['face']])
    return seat(slide(q, k['grip']), k['low'], k['side'])


def rests_on_shoulder(q):
    """True when the maul head in pose q lies on the right shoulder: the maul alone (the arm
    stays), dropped REST_PROBE px straight down, cuts the right arm's cubes and nothing else."""
    fore = world(q, 'forearm_r').to_3x3()
    low = dict(q, hand_r=q.get('hand_r', J()) + M(*(fore.transposed() @ Vector((0, REST_PROBE, 0)))))
    under = {c for c, m, _ in clash(low, tol=0.0) if m in MAUL_HEAD}
    return bool(under) and all(c.startswith('arm_r#') for c in under)


def trophy_clip(c):
    """The trophy pose: a roar with the maul's head resting on the right shoulder and the haft
    running forward and down from it to the right fist, which holds it near the middle, above
    the leather wrap; the left fist clenched and raised, the rage eyes burning (the diorama
    draws it with the rage flag). The right arm reaches forward to the fist, the elbow turned so
    the forearm crosses the haft. A slow loop: the chest heaves, the head shakes; the maul stays
    on the shoulder on every tick (checked between the keys too)."""
    roar = dict(body=R(23, 0, 0), head=R(-40, 0, 0), arm_l=R(-30, 0, -50), forearm_l=R(-80, 0, 0),
                leg_r=R(12, 0, 6), leg_l=R(-16, 0, -6))
    heave = pose(roar, body=M(0, -0.4, 0), head=R(-4, 6, 0), arm_l=R(-6, 0, -4), forearm_l=R(-8, 0, 0))
    t = c.clip('trophy', 80, loop=True)
    keys = []
    for f, p in ((0, roar), (40, heave), (80, roar)):
        q = dict(p)
        q['root'] = ground(q)
        q = carry(q)
        assert rests_on_shoulder(q), 'trophy %d: the maul head does not rest on the right shoulder' % f
        q['apron'] = drape(q)
        t.key(f, q)
        keys.append((f, q))
    # Every tick: the keys ease in and out (sine), every channel by the same fraction.
    for (f0, a), (f1, b) in zip(keys, keys[1:]):
        for f in range(f0, f1 + 1):
            s = (1 - math.cos(math.pi * (f - f0) / (f1 - f0))) / 2
            cut = clash(lerp(a, b, s))
            assert not cut, 'trophy tick %d: the maul cuts the brute: %s' % (f, cut)


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}
    REPORT.clear()
    grip = maul_point({}, (8, GRIP_R, 8)) - world({}, 'hand_r').translation
    assert grip.length < 1e-4, 'brute_maul display: the grip is %s px off the fist centre' % tuple(grip)
    idle_walk(c)
    attack_swing(c.attack('attack_swing', attacks['attack_swing']))
    attack_slam(c.attack('attack_slam', attacks['attack_slam']))
    attack_charge(c.attack('attack_charge', attacks['attack_charge']))
    charge_stop(c)
    rage(c)
    hurt_death(c)
    trophy_clip(c)
    slam = c.clips['attack_slam']
    for f, p, _ in slam.keys:
        REPORT.append('slam tick %d: maul head bottom %.1f px above the ground' % (f, head_height(p)))
    hit = {f: p for f, p, _ in slam.keys}[slam.at('hit')]
    centre = maul_point(hit, (8, 24.5, 8))
    REPORT.append('slam impact point (blocks, game frame): x %.2f z %.2f' % (centre.x / 16, centre.z / 16))
    for line in REPORT:
        print('brute:', line)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` with the rage flag on (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['head'], 'scale': 0.7},
    'flags': ['rage'],
}
