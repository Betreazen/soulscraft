"""Lurker (soulscraft:lurker, Глубинный удильщик): a deep-sea anglerfish, water only.

A big round head that is mostly mouth: an underslung lower jaw that juts past the snout, jagged
teeth as single cubes (the lower fangs stand in front of the snout even with the mouth shut, the
upper ones sink into the lower jaw), small pectoral, dorsal and tail fins, and a three-segment
stalk (illicium) arching from the forehead over the mouth. At its tip hangs the esca: a dark hood
and under it the glowing bulb. 24 px from the tip of the lower jaw to the end of the tail fin.

The body is near black so in the dark only the lure reads. The lit bulb (`lure`, glow: full-bright
in the game, emission in the showcase) can go dark: the clips scale it to 0 (LURE_OFF), and the
dead bulb (`lure_dark`, unlit, a little smaller) that sat inside it is left in the same place. The
swap takes one tick. The darkness belongs to the clips (the lure goes out on tick 4 of the lunge
windup), so it is a scale key and not a state flag.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front. Pose angles in
degrees: jaw x > 0 opens the mouth, body x > 0 dips the nose, y > 0 turns the nose to the mob's
right, z rolls; a stalk segment x > 0 bends it forward, x < 0 sweeps it back.
"""
import math

from paint import Material
from rig import M, R, S, lerp, pose

ID = 'lurker'

PALETTES = {
    'skin': Material(['#110f14', '#1a161e', '#241f29', '#2f2935', '#3b3442'], noise=0.75, patch=3),
    'belly': Material(['#1c171c', '#251f25', '#30282f', '#3b323a'], noise=0.6, patch=3),
    'fin': Material(['#17121a', '#211a25', '#2c2331', '#382d3e', '#45384c'], noise=0.4, patch=2),
    'mouth': Material(['#1a0709', '#2b0b0f', '#3f1217', '#56191f'], noise=0.5, patch=2),
    'tooth': Material(['#5f5a4e', '#857f6d', '#aaa38c', '#cbc4ab'], noise=0.25, patch=2),
    'stalk': Material(['#141117', '#1d1922', '#27222d', '#322b39'], noise=0.4, patch=2),
    'lure': Material(['#3fcab2', '#71ead4', '#a8f7e8', '#e0fff8'], noise=0.3, patch=2, fine=0),
    'dead': Material(['#141d1e', '#1c2829', '#253435', '#2f4041'], noise=0.4, patch=2),
}
EYE = '#6f7b80'
PUPIL = '#050506'
PORE = ('skin', -1)
GAPE = '#07060a'

# How far the bulb hangs below the hood (esca frame px).
BULB_DROP = 2.7


# ---------------------------------------------------------------------------- decals

def lateral_line(face, y, x0, x1):
    """A dotted row of pores along the flank, the anglerfish lateral line: lighter dots, every other texel."""
    for x in range(x0, x1, 2):
        face.px(x, y, PORE)


def eye(face, x, y):
    face.px(x, y, EYE)
    face.px(x, y + 1, ('skin', 0))
    face.px(x, y - 1, ('skin', 3))


def globe_decal(face):
    """The round head behind the cheeks: the lip groove and the dark mouth corner under them."""
    w, h = face.w, face.h
    if face.name in ('left', 'right'):
        face.rect(0 if face.name == 'left' else 4, h - 1, w - 4, 1, GAPE)   # the lip groove to the hinge
        face.patch(0, 0, w, 2, ('skin', 3), 0.35, 3)
    elif face.name == 'front':
        face.rect(0, h - 1, w, 1, GAPE)
    elif face.name == 'bottom':                    # the palate, seen when the jaw drops
        face.rect(1, 0, w - 2, h, ('mouth', 1))
        face.rect(2, 0, w - 4, h, ('mouth', 0))
    elif face.name == 'back':
        face.patch(2, 1, w - 4, 3, ('skin', 3), 0.4, 5)


def cheek_decal(face):
    """The bulging sides of the head: a tiny dull eye high at the front, the lateral line of pores
    running back from it, a few lighter warts."""
    w, h = face.w, face.h
    if face.name == 'left':
        eye(face, 1, 1)
        lateral_line(face, 3, 3, w)
        face.px(5, 1, PORE)
    elif face.name == 'right':
        eye(face, w - 2, 1)
        lateral_line(face, 3, 0, w - 3)
        face.px(w - 6, 1, PORE)
    elif face.name == 'front':
        face.rect(0, h - 1, w, 1, GAPE)
    elif face.name == 'bottom':
        face.rect(0, 0, w, h, ('skin', 0))


def brow_decal(face):
    if face.name in ('front', 'top'):
        face.rect(0, 0, face.w, 1, ('skin', 3))


def dome_decal(face):
    """The top of the skull: a darker ridge and the pore the stalk grows out of."""
    if face.name == 'top':
        face.rect(3, 0, 2, face.h, ('skin', 0))
        face.rect(3, face.h - 3, 2, 1, ('stalk', 2))
        face.px(2, face.h - 3, ('skin', 0))
        face.px(5, face.h - 3, ('skin', 0))
    elif face.name in ('left', 'right'):
        face.rect(0, 0, face.w, 1, ('skin', 3))


def snout_decal(face):
    w, h = face.w, face.h
    if face.name == 'front':
        face.px(2, 1, GAPE)                        # nostrils
        face.px(5, 1, GAPE)
        face.rect(0, h - 1, w, 1, GAPE)            # the upper lip
        face.rect(1, 0, w - 2, 1, ('skin', 3))
    elif face.name in ('left', 'right'):
        face.rect(0, h - 1, w, 1, GAPE)
    elif face.name == 'bottom':
        face.rect(1, 0, w - 2, h, ('mouth', 1))
        face.rect(3, 0, 2, h, ('mouth', 0))


def belly_decal(face):
    if face.name == 'front':                       # the throat at the back of the open mouth
        face.rect(1, 0, face.w - 2, face.h - 1, ('mouth', 0))
        face.rect(3, 0, face.w - 6, face.h - 2, GAPE)
    elif face.name == 'bottom':
        face.rect(1, 1, face.w - 2, face.h - 2, ('belly', 3))


def jaw_decal(face):
    """The lower jaw: the mouth floor with a tongue inside a lip rim on top, a dark lip line on the sides."""
    w, h = face.w, face.h
    if face.name == 'top':
        face.rect(1, 0, w - 2, h - 1, ('mouth', 1))
        face.rect(3, 1, w - 6, h - 3, ('mouth', 2))   # tongue
        face.rect(4, 2, w - 8, h - 5, ('mouth', 3))
        face.rect(1, 0, w - 2, 2, ('mouth', 0))        # the throat end, in shadow
    elif face.name in ('left', 'right'):
        face.rect(0, 0, w, 1, GAPE)
        face.rect(0, h - 1, w, 1, ('belly', 2))
        face.px(w // 2, 2, PORE)
        face.px(w // 2 + (2 if face.name == 'left' else -2), 2, PORE)
    elif face.name == 'front':
        face.rect(0, 0, w, 1, GAPE)
        face.rect(2, h - 1, w - 4, 1, ('belly', 3))
    elif face.name == 'bottom':
        face.rect(0, 0, w, h, ('belly', 1))
        face.rect(1, 1, w - 2, h - 2, ('belly', 2))


def chin_decal(face):
    if face.name == 'bottom':
        face.rect(1, 1, face.w - 2, face.h - 2, ('belly', 3))
    elif face.name in ('left', 'right', 'front'):
        face.rect(0, 0, face.w, face.h, ('belly', 1))


def trunk_decal(face):
    if face.name in ('left', 'right'):
        lateral_line(face, 3, 0 if face.name == 'left' else 1, face.w)
        face.rect(0, face.h - 2, face.w, 2, ('belly', 1))
    elif face.name == 'bottom':
        face.rect(1, 0, face.w - 2, face.h, ('belly', 2))


def tail_decal(face):
    if face.name in ('left', 'right'):
        face.px(1, 2, PORE)
        face.rect(0, face.h - 1, face.w, 1, ('belly', 1))


def rays(face, vertical=True):
    """Fin rays: lighter lines across the membrane, darker towards the edge."""
    if vertical:
        for x in range(0, face.w, 2):
            face.line(x, 0, x, face.h - 2, ('fin', 3))
        face.rect(0, face.h - 1, face.w, 1, ('fin', 0))
    else:
        for y in range(0, face.h, 2):
            face.line(0, y, face.w - 2, y, ('fin', 3))
        face.rect(face.w - 1, 0, 1, face.h, ('fin', 0))


def fin_decal(face):
    if face.name in ('left', 'right'):
        rays(face, vertical=False)


def caudal_decal(face):
    if face.name in ('left', 'right'):
        for y in range(1, face.h - 1, 2):
            face.line(0, y, face.w - 2, y, ('fin', 3))
        face.rect(face.w - 1, 0, 1, face.h, ('fin', 0))
        face.px(face.w - 1, face.h // 2, ('fin', 2))  # the notch of the tail


def dorsal_decal(face):
    if face.name in ('left', 'right'):
        rays(face, vertical=True)


def tooth_up(face):
    """A tooth that stands up from the lower jaw: dark gum at its foot, a light tip."""
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('tooth', 0))
        face.px(0, 0, ('tooth', -1))
    elif face.name == 'top':
        face.px(0, 0, ('tooth', -1))


def tooth_down(face):
    """A tooth that hangs from the upper jaw: gum at the top, tip at the bottom."""
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('tooth', 0))
        face.px(0, face.h - 1, ('tooth', -1))


def stalk_decal(face):
    if face.name in ('left', 'right', 'front', 'back'):
        for y in range(1, face.h, 2):
            face.px(0, y, ('stalk', 3))           # ringed like a fishing rod


def hood_decal(face):
    """The dark esca hood: a lighter rim at the bottom where the bulb comes out, a hole underneath."""
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('stalk', 2))
        face.px(1, 1, GAPE)
    elif face.name in ('left', 'right', 'front', 'back'):
        face.rect(0, face.h - 1, face.w, 1, ('stalk', 3))
        face.px(1, 0, ('stalk', 0))


def lure_decal(face):
    """The lit bulb: a white-hot texel in a cyan glow."""
    face.rect(0, 0, face.w, face.h, ('lure', 1))
    face.px(0, 0, ('lure', 3))
    face.px(1, 0, ('lure', 2))
    face.px(0, 1, ('lure', 2))


def dead_decal(face):
    """The bulb with its light out: dull grey-teal, one faint spot where the glow was."""
    face.rect(0, 0, face.w, face.h, ('dead', 1))
    face.px(0, 0, ('dead', 3))


# ---------------------------------------------------------------------------- model

# Upper teeth (body frame): x, z of the tooth, its length down from the lip (y = 1).
UPPER_TEETH = [
    (-3.5, -11, 2), (-2, -11, 3), (-0.5, -11, 2), (1, -11, 3), (2.5, -11, 2),   # front of the snout
    (-3.5, -9.5, 3), (2.5, -9.5, 3),                                              # its sides
    (-4.5, -7, 2), (3.5, -7, 3), (-4.5, -5.5, 3), (3.5, -5.5, 2), (-4.5, -4, 2), (3.5, -4, 2),
]
# Lower teeth (jaw frame, the jaw top is y = 0): x, z, length up. The front row stands in front of
# the snout when the mouth is shut; the side rows are only seen with the jaw open.
LOWER_TEETH = [
    (-5.5, -8.5, 3), (4.5, -8.5, 2), (-5.5, -7, 2), (4.5, -7, 4),
    (-5.5, -5, 2), (4.5, -5, 3), (-5.5, -3.5, 1), (4.5, -3.5, 1),
]
# The front row of the lower jaw, on its own part leaning forward like an underbite: x, length.
# Uneven on purpose: fangs between stubs, wider gaps on one side.
FRONT_TEETH = [(-5.5, 3), (-4, 2), (-2.5, 4), (-1, 1), (0.5, 3), (2, 2), (3.5, 4), (4.5, 1)]

# The stalk: segment lengths (px) and their rest bend (deg, x, relative to the parent segment).
STALK = [('stalk1', 4, 8), ('stalk2', 5, 52), ('stalk3', 4, 55)]
STALK_ROOT = (0.0, 8.0, -5.0)   # out of the top of the skull


def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 16, 0), parent='root')
    body.cube((-5, -7, -8), (10, 8, 11), 'skin', decal=globe_decal)
    body.cube((-6, -5, -7), (12, 5, 8), 'skin', decal=cheek_decal)     # the head bulges at the sides
    body.cube((-4, -8, -7), (8, 1, 9), 'skin', decal=dome_decal)
    body.cube((-4, -6, -9), (8, 1, 1), 'skin', decal=brow_decal)       # the forehead slopes to the snout
    body.cube((-4, -5, -11), (8, 6, 3), 'skin', decal=snout_decal)
    body.cube((-5, 1, -3), (10, 4, 6), 'belly', decal=belly_decal)
    body.cube((-3.5, -5, 3), (7, 8, 4), 'skin', decal=trunk_decal)
    for x, z, n in UPPER_TEETH:
        body.cube((x, 1, z), (1, n, 1), 'tooth', decal=tooth_down)

    # The jaw rests 3 degrees open, as every clip shows it; shut, the teeth and the belly would
    # meet the jaw's faces in one plane.
    jaw = m.part('jaw', (0, 17, -2), parent='body', rot=(3, 0, 0))
    jaw.cube((-6, 0, -10), (12, 4, 10), 'skin', decal=jaw_decal)
    jaw.cube((-5, 4, -9), (10, 1, 7), 'belly', inflate=(-0.1, 0, 0), decal=chin_decal)   # sides off the belly's planes
    for x, z, n in LOWER_TEETH:
        jaw.cube((x, -n, z), (1, n, 1), 'tooth', decal=tooth_up)
    fangs = m.part('fangs', (0, 17, -11.5), parent='jaw', rot=(14, 0, 0))
    for x, n in FRONT_TEETH:
        fangs.cube((x, -n, -0.5), (1, n, 1), 'tooth', decal=tooth_up)

    tail = m.part('tail', (0, 15, 7), parent='body')
    tail.cube((-2.5, -3, 0), (5, 6, 3), 'skin', decal=tail_decal)
    m.part('tail_fin', (0, 15, 10), parent='tail').cube((-0.5, -4, 0), (1, 8, 2), 'fin', decal=caudal_decal)
    m.part('dorsal', (0, 11.5, 3), parent='body', rot=(-10, 0, 0)).cube((-0.5, -3, 0), (1, 3, 4), 'fin', decal=dorsal_decal)
    # Pectoral fins just behind the cheeks, fanned out and back.
    fr = m.part('fin_r', (-5, 16.5, 1.5), parent='body', rot=(0, -35, 0))
    f = fr.cube((-1, -2, 0), (1, 4, 4), 'fin', decal=fin_decal)
    m.part('fin_l', (5, 16.5, 1.5), parent='body', rot=(0, 35, 0)).cube((0, -2, 0), (1, 4, 4), 'fin', share=f, mirror=True)

    # The stalk: each segment is a 1 x L x 1 rod up its own -y; pivots go along the bent chain.
    p, a, parent = STALK_ROOT, 0.0, 'body'
    for name, length, bend in STALK:
        a += bend
        # The middle segment is 0.1 px wider in x: bent about x, neighbours keep their sides in one plane.
        wide = (0.1, 0, 0) if name == 'stalk2' else 0
        m.part(name, p, parent=parent, rot=(bend, 0, 0)).cube((-0.5, -length, -0.5), (1, length, 1), 'stalk', inflate=wide, decal=stalk_decal)
        r = math.radians(a)
        p = (p[0], p[1] - length * math.cos(r), p[2] - length * math.sin(r))
        parent = name
    # The esca hangs upright from the tip, whatever the bend of the stalk.
    esca = m.part('esca', p, parent=parent, rot=(-a, 0, 0))
    esca.cube((-1.5, -0.3, -1.5), (3, 3, 3), 'stalk', decal=hood_decal)
    bulb = (p[0], p[1] + 1.2 + BULB_DROP, p[2])
    m.part('lure_dark', bulb, parent='esca').cube((-1, -1, -1), (2, 2, 2), 'dead', inflate=0.1, decal=dead_decal)
    m.part('lure', bulb, parent='esca', glow=True).cube((-1, -1, -1), (2, 2, 2), 'lure', inflate=0.25, decal=lure_decal, grain=0.5)

    # Where the game can hang the grabbed player: the middle of the mouth, above the tongue.
    m.socket('socket_mouth', 'jaw', (0, -2, -6))


# ---------------------------------------------------------------------------- poses

LURE_OFF = {'lure': S(0)}   # the lit bulb shrunk away, the dead one inside it shows: the lure is dark
REST = {}   # the jaw's 3 degrees are its rest rotation


def wave(ph, amp=1.0, lag=0.0):
    """sin of a phase in turns (0..1 is one cycle), rounded so a loop ends on its first pose exactly."""
    return round(amp * math.sin(2 * math.pi * (ph - lag)), 3)


def stalk_sway(ph, fore=1.0, side=1.0):
    """The stalk swinging a beat behind the body: each segment a little later and wider than the
    one before; the esca hangs and swings back the other way."""
    return {
        'stalk1': R(wave(ph, 2 * fore), 0, wave(ph, 2 * side, 0.05)),
        'stalk2': R(wave(ph, 3 * fore, 0.1), 0, wave(ph, 3 * side, 0.15)),
        'stalk3': R(wave(ph, 4 * fore, 0.2), 0, wave(ph, 4 * side, 0.25)),
        'esca': R(wave(ph, -6 * fore, 0.3), 0, wave(ph, -6 * side, 0.35)),
    }


def idle_pose(ph):
    return pose(REST, stalk_sway(ph),
                body=M(0, wave(ph, 0.4), 0) + R(wave(ph, 1.5, 0.1), 0, 0),
                jaw=R(wave(ph, 2.5, 0.25) + 2, 0, 0),
                fin_r=R(0, wave(ph * 2, -14), wave(ph * 2, 8)),
                fin_l=R(0, wave(ph * 2, 14), wave(ph * 2, -8)),
                tail=R(0, wave(ph, 5, 0.2), 0),
                tail_fin=R(0, wave(ph, 7, 0.35), 0))


def swim_pose(ph, amp=1.0, dark=False):
    """One beat of the tail: the body yaws against it, the fins tuck in, the stalk is swept back by
    the water and trails behind."""
    p = pose(REST, stalk_sway(ph * 2, 0.6, 0.5),
             body=R(-2, wave(ph, -5 * amp), 0) + M(0, wave(ph * 2, 0.25), 0),
             jaw=R(-2, 0, 0),
             tail=R(0, wave(ph, 22 * amp, 0.1), 0),
             tail_fin=R(0, wave(ph, 30 * amp, 0.3), 0),
             fin_r=R(0, 22 + wave(ph * 2, 6), 0),
             fin_l=R(0, -22 - wave(ph * 2, 6), 0),
             stalk2=R(-14 * amp, 0, 0), stalk3=R(-18 * amp, 0, 0), esca=R(-10 * amp, 0, 0))
    return pose(p, LURE_OFF) if dark else p


def loop_clip(c, name, length, step, fn, ease='linear'):
    clip = c.clip(name, length, loop=True)
    for f in range(0, length + 1, step):
        clip.key(f, fn(f / length), ease)
    return clip


# Ambush: the coil before the lunge, the lunge itself (jaws at their widest, the stalk thrown back)
# and the whiff after it.
COIL = pose(REST, LURE_OFF, body=R(-10, 0, 0) + M(0, 0, 2.5), jaw=R(52, 0, 0),
            tail=R(0, 26, 0), tail_fin=R(0, 30, 0), fin_r=R(0, -20, 18), fin_l=R(0, 20, -18),
            stalk1=R(-6, 0, 0), stalk2=R(-10, 0, 0), stalk3=R(-8, 0, 0), esca=R(8, 0, 0))
LUNGE = pose(REST, LURE_OFF, body=R(-12, 0, 0) + M(0, 0, -4), jaw=R(62, 0, 0),
             tail=R(0, -14, 0), tail_fin=R(0, -22, 0), fin_r=R(0, 30, 6), fin_l=R(0, -30, -6),
             stalk1=R(-14, 0, 0), stalk2=R(-26, 0, 0), stalk3=R(-30, 0, 0), esca=R(-16, 0, 0))
SNAPPED = pose(LUNGE, body=R(14, 0, 0) + M(0, 0, -1), jaw=R(-64, 0, 0), stalk3=R(18, 0, 0), esca=R(20, 0, 0))
DAZED = pose(REST, LURE_OFF, body=R(10, -6, 9) + M(0, 0.6, -3), jaw=R(16, 0, 0),
             tail=R(0, 8, 0), fin_r=R(0, 10, -10), fin_l=R(0, -6, 12),
             stalk1=R(8, 0, 6), stalk2=R(14, 0, 8), stalk3=R(16, 0, 10), esca=R(-10, 0, -8))
NEUTRAL_DARK = pose(REST, LURE_OFF)


def attack_lunge(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    first = pose(REST, body=M(0, 0, 0.8), jaw=R(14, 0, 0), fin_r=R(0, -8, 8), fin_l=R(0, 8, -8),
                 stalk2=R(-4, 0, 0), stalk3=R(-4, 0, 0))
    a.key(0, REST, 'out')
    a.key(3, first, 'linear')                  # the mouth starts to open, the lure still lit
    a.key(4, pose(first, LURE_OFF, jaw=R(4, 0, 0)), 'out')   # the lure goes dark: the tell
    a.key(hit - 2, COIL, 'in')                 # gaping, coiled back, tail cocked
    a.key(hit, LUNGE, 'linear')                # the throw: up to 6 blocks over the next 6 ticks
    a.key(rec - 2, pose(LUNGE, body=M(0, 0, -1), jaw=R(4, 0, 0)), 'in')
    a.key(rec, SNAPPED, 'out')                 # the jaws slam shut on nothing
    a.key(rec + 4, DAZED, 'slow')              # carried on by its own weight, open to hits
    a.key(rec + 13, pose(DAZED, body=R(-4, 4, -6) + M(0, -0.3, 1), jaw=R(-6, 0, 0)), 'slow')
    a.key(end, NEUTRAL_DARK)


# Grab: the jaw clamped on the player, nose dipped to drag them down, thrashing side to side.
GRAB = pose(REST, LURE_OFF, body=R(14, 0, 0) + M(0, 0.5, -1), jaw=R(20, 0, 0),
            fin_r=R(0, 10, 14), fin_l=R(0, -10, -14),
            stalk1=R(-10, 0, 0), stalk2=R(-24, 0, 0), stalk3=R(-26, 0, 0), esca=R(-14, 0, 0))


def grab_shake(s, strength=1.0):
    k = s * strength
    return pose(GRAB,
                body=R(abs(k) * 3, 12 * k, 7 * k),
                jaw=R(-5 * abs(s), 0, 0),
                tail=R(0, -28 * k, 0), tail_fin=R(0, -34 * k, 0),
                fin_r=R(0, 14 * s, 10 * s), fin_l=R(0, 14 * s, -10 * s),
                stalk2=R(0, 0, -12 * k), stalk3=R(0, 0, -16 * k), esca=R(0, 0, 18 * k))


def grab(c):
    """40 ticks at most (mobs.json grab_max_ticks); a loop, so a longer hold in the game still plays."""
    g = c.clip('grab', 40, loop=True)
    g.key(0, grab_shake(0), 'inout')
    side = 1
    for f in range(3, 40, 4):                  # a thrash every 4 ticks, the middle ones the hardest
        g.key(f, grab_shake(side, 1.0 if 9 <= f <= 31 else 0.7), 'inout')
        side = -side
    g.key(40, grab_shake(0))


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}

    loop_clip(c, 'idle', 40, 4, idle_pose)                         # hover: slow fins, the lure sways
    loop_clip(c, 'swim', 20, 2, swim_pose)                         # the tail cycle
    loop_clip(c, 'walk', 20, 2, swim_pose)                         # same cycle under the engine's name
    loop_clip(c, 'drift', 40, 4, lambda ph: pose(                  # creeping closer unseen
        REST, body=R(3, 0, 0), jaw=R(-1, 0, 0),
        fin_r=R(0, wave(ph * 2, -18), wave(ph * 2, 10)), fin_l=R(0, wave(ph * 2, 18), wave(ph * 2, -10)),
        tail=R(0, wave(ph, 4), 0), tail_fin=R(0, wave(ph, 6, 0.2), 0)))
    loop_clip(c, 'retreat', 16, 2, lambda ph: swim_pose(ph, 1.4, dark=True))   # fleeing, lure dark

    attack_lunge(c.attack('attack_lunge', attacks['attack_lunge']))
    grab(c)

    release = c.clip('release', 14)            # the third hit lands: it lets go and flinches back
    jolt = pose(REST, LURE_OFF, body=R(-16, 8, -6) + M(0, -0.5, 3), jaw=R(55, 0, 0),
                fin_r=R(0, -30, 20), fin_l=R(0, 30, -20), tail=R(0, 20, 0), tail_fin=R(0, 26, 0),
                stalk2=R(18, 0, 0), stalk3=R(24, 0, 0), esca=R(-20, 0, 0))
    release.key(0, grab_shake(0), 'out')
    release.key(2, jolt, 'out')
    release.key(6, pose(jolt, body=R(10, -6, 4), jaw=R(-30, 0, 0), stalk2=R(-14, 0, 0), stalk3=R(-18, 0, 0)), 'slow')
    release.key(14, swim_pose(0, 1.4, dark=True))

    hurt = c.clip('hurt', 10)                  # vanilla hurt_time is 10 ticks
    hit = pose(REST, body=R(-10, 8, 6) + M(0, 0, 1.5), jaw=R(24, 0, 0), fin_r=R(0, -24, 16), fin_l=R(0, 24, -16),
               tail=R(0, -16, 0), stalk1=R(6, 0, 0), stalk2=R(14, 0, 0), stalk3=R(20, 0, 0), esca=R(-16, 0, 0))
    hurt.key(0, REST, 'out')
    hurt.key(2, hit, 'slow')
    hurt.key(10, REST)

    death = c.clip('death', 20)                # vanilla death_time is 20 ticks; the light goes out
    spasm = pose(REST, body=R(-12, 0, 0), jaw=R(46, 0, 0), fin_r=R(0, -30, 20), fin_l=R(0, 30, -20),
                 tail=R(0, 24, 0), stalk2=R(12, 0, 0), stalk3=R(16, 0, 0))
    limp = pose(REST, jaw=R(30, 0, 0), fin_r=R(0, 20, -20), fin_l=R(0, -20, 20), tail=R(0, -8, 0),
                stalk1=R(10, 0, 0), stalk2=R(20, 0, 0), stalk3=R(24, 0, 0), esca=R(10, 0, 0))
    death.key(0, REST, 'out')
    death.key(3, spasm, 'inout')
    death.key(5, pose(lerp(spasm, limp, 0.3)), 'linear')
    death.key(6, pose(lerp(spasm, limp, 0.35), LURE_OFF), 'inout')
    death.key(12, pose(limp, LURE_OFF, body=R(0, 0, 100) + M(0, -1.5, 0)), 'inout')   # rolls over ...
    death.key(20, pose(limp, LURE_OFF, body=R(0, 0, 180) + M(0, -3, 0), jaw=R(-6, 0, 0)))   # ... belly up, floating

    trophy_clip(c)


# The trophy pose: rising from the dark with the jaw at its widest, the lure lit and hung in front
# of the teeth, fins spread, the tail cocked. A slow loop: it hangs in the water, the lure sways.
TROPHY_POSE = pose(REST, body=R(-14, 0, 0), jaw=R(48, 0, 0), fin_r=R(0, -26, 20), fin_l=R(0, 26, -20),
                   tail=R(0, 22, 0), tail_fin=R(0, 28, 0))


def trophy_clip(c):
    loop_clip(c, 'trophy', 80, 4, lambda ph: pose(
        TROPHY_POSE, stalk_sway(ph, 0.6, 0.5),
        body=M(0, wave(ph, 0.4), 0), jaw=R(wave(ph, 3, 0.25), 0, 0),
        fin_r=R(0, wave(ph * 2, -8), wave(ph * 2, 5)), fin_l=R(0, wave(ph * 2, 8), wave(ph * 2, -5)),
        tail=R(0, wave(ph, 4, 0.2), 0), tail_fin=R(0, wave(ph, 6, 0.35), 0)))


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['body'], 'drop': ['tail', 'dorsal', 'fin_r', 'fin_l'], 'scale': 0.6},
}
