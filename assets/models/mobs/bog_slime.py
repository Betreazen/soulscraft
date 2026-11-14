"""Bog slime (soulscraft:bog_slime, Топяная слизь): an opaque cube of bog mud.

One model for all sizes. At size 1 it is the 8 x 8 x 8 px outer cube of the vanilla slime; the
mod scales the whole model by Size (1, 2, 4) the way the vanilla slime renderer does, so a size 4
slime is 2 blocks tall. The Horizon draw frame carries no scale of its own (research 5.4), the
mod applies it.

No translucency in the game (research 1.4), so the vanilla "core inside the jelly" is shown
another way: a dark hollow on the front (the "mouth") where the dark inner mud, roots and a bone
are visible two pixels deep. Moss and a reed on top, roots sticking out of the sides and the back,
bubbles on the skin, two beady eyes.

Squash and stretch. The cube is built like the magma cube: three
horizontal slabs (lower, mid, upper) stacked on each other and a dark core inside them.
Stretch pulls the slabs apart and the core shows in the gaps; squash sinks them into each other
(upper and lower are inflated a little more than mid, so overlapping faces never meet in one
plane), and four mud skirts hidden inside the lower slab slide out on the ground, which makes
the squashed slime wider. The rig has a part scale (S) now, but the slabs stay: a scaled part
stretches its texels (one cube pixel is one texel), and the open slabs show the core, which a
scaled cube would hide.

The jump height is not in the clips: in the game the slime jumps by physics (vanilla slime
movement), so no clip lifts the root. The showcase lifts the actor itself.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front.
"""
import math

from paint import Material
from rig import M, R, pose

ID = 'bog_slime'

PALETTES = {
    'mud': Material(['#2a2416', '#3a321f', '#4a4029', '#5b4f33', '#6c5f3e'], noise=0.8, patch=3),
    'mud_wet': Material(['#1b1810', '#252117', '#302a1d', '#3c3524', '#4a422e'], noise=0.6, patch=3),
    'gut': Material(['#060504', '#0c0a07', '#14100c', '#1c1711'], noise=0.5, patch=2),
    'moss': Material(['#22351a', '#2d461f', '#3a5a25', '#4b702c', '#5f8636'], noise=0.9, patch=2),
    'root': Material(['#24180e', '#342314', '#47311d', '#5b4027', '#6f5134'], noise=0.5, patch=2),
    'bubble': ['#3e4026', '#535636', '#6a6d45', '#878a5c'],
    'eye': ['#070605', '#100e0b', '#1a1712'],
    'bone': ['#5f5947', '#7a735d', '#958d74', '#ada58a'],
    'reed': ['#3b4a1c', '#4d5f24', '#62782d', '#7a9038'],
}
GLINT = '#cfd49a'           # the wet light in a beady eye
SHINE = ('mud', -1)          # wet mud catching the light
WET = ('mud_wet', 1)

# Slab sizes, px: lower 2, mid 3, upper 3; the hollow is cut into the mid slab.
UPPER_INFLATE = 0.3
LOWER_INFLATE = 0.2


# ---------------------------------------------------------------------------- decals

def bubble(face, x, y):
    """A 2 x 2 bubble in the mud skin: lit on the top left, dark rim on the bottom right."""
    face.px(x, y, ('bubble', 3))
    face.px(x + 1, y, ('bubble', 2))
    face.px(x, y + 1, ('bubble', 2))
    face.px(x + 1, y + 1, ('bubble', 0))


def moss_fringe(face, cols, seed=0):
    """Moss hanging over the top edge of a side face: the first row, and a drip here and there."""
    for x in cols:
        if 0 <= x < face.w:
            face.px(x, 0, ('moss', 2 if (x + seed) % 3 else 1))
            if (x * 7 + seed) % 4 == 0:
                face.px(x, 1, ('moss', 1))


def upper_decal(face):
    n = face.name
    if n == 'front':
        moss_fringe(face, range(0, 4), 1)       # moss_a hangs over the front right corner
        face.px(4, 0, ('moss', 1))
        face.rect(0, 2, 8, 1, ('mud', 1))        # the lip over the hollow, in shadow
        face.px(4, 1, SHINE)                     # wet between the eyes
        face.streak(7, 1, 2, WET)
    elif n == 'right':                            # x = 0 is the back edge here
        moss_fringe(face, range(4, 8), 2)
        face.rect(2, 1, 2, 2, ('gut', 2))         # where root_a comes out
        face.px(3, 1, ('root', 1))
        face.px(6, 2, SHINE)
    elif n == 'left':                             # x = 0 is the front edge here
        moss_fringe(face, (0, 1, 5, 6, 7), 3)
        bubble(face, 2, 1)
        face.streak(6, 1, 2, WET)
    elif n == 'back':                             # x = 0 is the mob's left
        moss_fringe(face, range(1, 5), 4)
        face.rect(5, 0, 1, 1, ('gut', 2))         # root_c comes out here
        face.streak(1, 1, 2, WET)
        bubble(face, 6, 1)
    elif n == 'top':                              # bottom row is the front edge
        face.patch(4, 1, 4, 3, ('mud_wet', 3), 0.55, 2)   # a puddle at the back
        face.px(5, 2, SHINE)
        face.px(1, 1, ('bubble', 2))
        face.px(6, 6, ('bubble', 3))
    elif n == 'bottom':                           # the roof of the hollow
        face.rect(0, 0, face.w, face.h, ('gut', 1))
        face.px(2, 1, ('root', 1))


def mid_decal(face):
    n = face.name
    if n == 'left':                               # z -1..4, x = 0 at the front end
        face.streak(1, 0, 2, WET)
        face.px(4, 2, SHINE)
    elif n == 'right':                            # x = 0 at the back end
        bubble(face, 1, 0)
        face.streak(3, 0, 3, WET)
    elif n == 'back':
        face.rect(0, 0, face.w, 1, ('mud', 1))
        face.px(2, 1, ('gut', 2))                 # where root_b goes in on the left
        bubble(face, 4, 1)
        face.streak(7, 0, 2, WET)


def cheek_r_decal(face):
    """The mob's right cheek of the hollow, 1 px wide; its left face looks into the hollow."""
    if face.name == 'left':
        face.rect(0, 0, face.w, face.h, ('gut', 2))
        face.rect(0, 0, 1, face.h, ('mud', 0))    # the lit lip at the front edge
    elif face.name == 'front':
        face.rect(0, 0, 1, face.h, ('mud', 1))
        face.px(0, 2, WET)


def cheek_l_decal(face):
    """The mob's left cheek of the hollow, 2 px wide; its right face looks into the hollow."""
    if face.name == 'right':
        face.rect(0, 0, face.w, face.h, ('gut', 2))
        face.rect(face.w - 1, 0, 1, face.h, ('mud', 0))
    elif face.name == 'front':
        face.px(0, 0, ('mud', 1))
        face.px(0, 1, ('mud', 1))
        face.px(0, 2, ('mud', 0))
        face.px(1, 1, SHINE)


def lower_decal(face):
    n = face.name
    if n in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, WET)  # wet where it sits in the bog
        if n == 'front':
            face.px(3, 0, ('mud_wet', 2))             # wet under the hollow, where it drools
            face.px(3, 1, ('mud_wet', 0))
        if n == 'left':
            bubble(face, 5, 0)
        if n == 'right':
            face.px(2, 0, SHINE)
    elif n == 'top':                               # the floor of the hollow at the front rows
        face.rect(0, 0, face.w, face.h, WET)
        face.rect(1, 5, 6, 3, ('gut', 1))
        face.px(2, 6, ('gut', 3))
    elif n == 'bottom':
        face.rect(0, 0, face.w, face.h, ('mud_wet', 0))


def core_decal(face):
    """The dark inner mud: root fibres in it (seen in the hollow and in the stretch gaps)."""
    n = face.name
    if n == 'front':                               # rows 1..3 show in the hollow
        face.px(1, 2, ('root', 3))
        face.px(4, 1, ('root', 2))
        face.px(5, 3, ('bubble', 2))               # a wet glint deep inside
        face.px(0, 3, ('root', 1))
        face.px(3, 3, ('gut', 0))
    elif n in ('left', 'right', 'back'):
        face.line(0, 1, face.w - 1, 0, ('root', 1))
        face.line(1, 3, face.w - 2, 4, ('root', 2))
        face.px(face.w // 2, 2, ('bubble', 1))
    elif n == 'top':
        face.line(0, 1, face.w - 1, 3, ('root', 1))
        face.px(2, 3, ('bubble', 2))


def eye_decal(face):
    if face.name == 'front':
        face.px(0, 0, GLINT)
        face.px(1, 0, ('eye', 2))
        face.px(0, 1, ('eye', 1))
        face.px(1, 1, ('eye', 0))
    else:
        face.rect(0, 0, face.w, face.h, ('eye', 1))


def moss_decal(face):
    if face.name == 'top':
        face.patch(0, 0, face.w, face.h, ('moss', 3), 0.35, face.w)
        face.px(1, 1, ('moss', 4))
        face.px(face.w - 2, face.h - 1, ('moss', 4))
    elif face.name != 'bottom':
        face.rect(0, face.h - 1, face.w, 1, ('moss', 1))


def root_decal(face):
    if face.name in ('top', 'front', 'back', 'left', 'right'):
        face.px(0, 0, ('root', 3))
    # the cut end catches the light: a pale ring on the small end faces
    if face.w == 1 and face.h == 1:
        face.px(0, 0, ('root', 4))


def pale_root_decal(face):
    """Roots inside the hollow are lighter, so they read against the dark."""
    face.rect(0, 0, face.w, face.h, ('root', 3))
    face.px(0, 0, ('root', 4))


def bone_decal(face):
    if face.name in ('left', 'right'):
        face.px(0, 0, ('bone', 3))


def bubble_decal(face):
    face.rect(0, 0, face.w, face.h, ('bubble', 1))
    face.px(0, 0, ('bubble', 3))
    if face.w > 1 and face.h > 1:
        face.px(face.w - 1, face.h - 1, ('bubble', 0))


def skirt_decal(face):
    if face.name == 'top':
        face.px(1, 1, SHINE)
        face.px(face.w - 2, face.h - 2, ('mud_wet', 3))
    elif face.name != 'bottom':
        face.rect(0, 0, face.w, 1, ('mud_wet', 2))


def reed_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.px(0, 0, ('reed', 3))
        face.px(0, face.h - 1, ('reed', 0))


# ---------------------------------------------------------------------------- model

def tip(pivot, length, rot_z_deg):
    """End of a root that leaves pivot along -x (rot_z > 0 lifts it), game px."""
    a = math.radians(rot_z_deg)
    return (pivot[0] - length * math.cos(a), pivot[1] - length * math.sin(a), pivot[2])


def model(m):
    m.part('root', (0, 24, 0))

    # Lower slab: 8 x 2 x 8, inflated, its bottom on the ground. Four mud skirts hide in it.
    lower = m.part('lower', (0, 24, 0), parent='root')
    lower.cube((-4, -2 - LOWER_INFLATE, -4), (8, 2, 8), 'mud', inflate=LOWER_INFLATE, decal=lower_decal)
    # The side skirts ride 0.1 px higher: slid out and down, their bottoms stop just above the
    # lower slab's bottom instead of in its plane.
    sk = m.part('skirt_l', (0, 24, 0), parent='lower').cube((1, -1.35, -3.5), (3, 1, 7), 'mud_wet', decal=skirt_decal)
    m.part('skirt_r', (0, 24, 0), parent='lower').cube((-4, -1.35, -3.5), (3, 1, 7), 'mud_wet', share=sk, mirror=True)
    # The front and back skirts lie 0.1 px lower than the side ones, and 0.1 px narrower than the
    # core each side: where they cross, no two faces share a plane (z-fighting).
    sf = m.part('skirt_f', (0, 24, 0), parent='lower').cube((-3, -1.15, -4), (6, 1, 3), 'mud_wet', inflate=(-0.1, 0, 0),
                                                           decal=skirt_decal)
    m.part('skirt_b', (0, 24, 0), parent='lower').cube((-3, -1.15, 1), (6, 1, 3), 'mud_wet', inflate=(-0.1, 0, 0), share=sf)

    # Mid slab, y 19..22: the back block and two cheeks around the hollow on the front.
    mid = m.part('mid', (0, 22, 0), parent='lower')
    mid.cube((-4, -3, -1), (8, 3, 5), 'mud', decal=mid_decal)
    mid.cube((-4, -3, -4), (1, 3, 3), 'mud', decal=cheek_r_decal)
    mid.cube((2, -3, -4), (2, 3, 3), 'mud', decal=cheek_l_decal)
    # In the hollow (x -3..2, 2 px deep): a hanging root, a root across, a bone on the floor.
    mid.cube((-2, -2.7, -3.5), (1, 2, 1), 'root', decal=pale_root_decal)
    mid.cube((-1, -2, -2.8), (3, 1, 1), 'root', decal=pale_root_decal)
    # Its end 0.1 px inside the hanging root; its top never in the lower slab's top when squashed.
    mid.cube((-3.1, -1.05, -3.4), (2, 1, 1), 'bone', decal=bone_decal)

    # The core: dark inner mud, the back wall of the hollow; it fills the gaps of the stretch.
    core = m.part('core', (0, 22, 0), parent='mid')
    core.cube((-3, -4, -2), (6, 5, 5), 'gut', inflate=(0, -0.1, 0), decal=core_decal)   # never in a slab's plane

    # Upper slab, y 16..19 plus inflate: eyes, moss, a reed, the roots of the back and side.
    upper = m.part('upper', (0, 19, 0), parent='mid')
    upper.cube((-4, -3, -4), (8, 3, 8), 'mud', inflate=UPPER_INFLATE, decal=upper_decal)
    e = upper.cube((-3, -2.5, -4.8), (2, 2, 1), 'eye', decal=eye_decal)
    upper.cube((1, -2.5, -4.8), (2, 2, 1), 'eye', share=e)
    upper.cube((-4, -4, -4), (4, 1, 4), 'moss', inflate=0.4, decal=moss_decal)
    upper.cube((1, -4, 1), (3, 1, 3), 'moss', inflate=0.4, decal=moss_decal)
    upper.cube((-3, -5, -3), (2, 1, 2), 'moss', inflate=0.2, decal=moss_decal)
    upper.cube((2, -7, 2), (1, 3, 1), 'reed', decal=reed_decal)
    upper.cube((3, -6, 3), (1, 2, 1), 'reed', decal=reed_decal)

    # Roots. root_a leaves the right side and bends up; root_b dives from the left side into
    # the ground like a buttress; root_c sticks out of the back and up, with a twig.
    pa = (-4.3, 17.5, 1.0)
    m.part('root_a', pa, parent='upper', rot=(0, 0, 20)).cube((-3, -0.5, -0.5), (3, 1, 1), 'root', decal=root_decal)
    m.part('root_a2', tip(pa, 3, 20), parent='root_a', rot=(0, 0, 35)).cube((-2, -0.5, -0.5), (2, 1, 1), 'root',
                                                                             inflate=(0, 0, 0.1), decal=root_decal)
    m.part('root_b', (4, 21, 2.5), parent='mid', rot=(0, 0, 35)).cube((0, -0.5, -0.5), (4, 1, 1), 'root',
                                                                        decal=root_decal)
    pc = (-1.5, 16.2, UPPER_INFLATE + 4)
    m.part('root_c', pc, parent='upper', rot=(35, 0, 0)).cube((-0.5, -0.5, 0), (1, 1, 4), 'root', decal=root_decal)
    a = math.radians(35)
    m.part('root_c2', (pc[0], pc[1] - 2.5 * math.sin(a), pc[2] + 2.5 * math.cos(a)), parent='root_c',
           rot=(0, 0, -40)).cube((0, -0.5, -0.5), (2, 1, 1), 'root', decal=root_decal)

    # Bubbles that grow and pop in the idle: one on the left side, one on top.
    m.part('bubble_a', (4, 20.5, 0.5), parent='mid').cube((-0.25, -1, -1), (1, 2, 2), 'bubble', decal=bubble_decal)
    m.part('bubble_b', (-2.5, 16 - UPPER_INFLATE, 2.5), parent='upper').cube((-0.5, -0.6, -0.5), (1, 1, 1), 'bubble',
                                                                             decal=bubble_decal)


# ---------------------------------------------------------------------------- poses

SKIRT_OUT = 2.0   # px a skirt slides out per unit of spread


def squash(k, spread=None):
    """Squashed down by k (1 = 2.25 px lower, the mouth closed), wider by the skirts."""
    s = k if spread is None else spread
    out = SKIRT_OUT * s
    down = 0.25 if s > 0 else 0   # the skirts sit on the ground once they are out
    return {
        'mid': M(0, 1.0 * k, 0),
        'upper': M(0, 1.25 * k, 0),
        'skirt_l': M(out, down, 0), 'skirt_r': M(-out, down, 0),
        'skirt_f': M(0, down, -out), 'skirt_b': M(0, down, out),
        'root_b': R(0, 0, -14 * k),          # the buttress root lifts instead of going into the ground
    }


def stretch(k):
    """Stretched up by k (1 = 2.45 px taller): the slabs part, the core shows in the gaps, the
    roots trail."""
    return {
        'mid': M(0, -1.2 * k, 0),
        'upper': M(0, -1.25 * k, 0),
        'root_a': R(0, 0, -18 * k),
        'root_a2': R(0, 0, -10 * k),
        'root_c': R(-14 * k, 0, 0),
        'root_b': R(0, 0, 10 * k),
    }


def tremble(sign, k=1.0):
    return {'upper': R(0, 0, 2.5 * sign * k) + M(0.25 * sign * k, 0, 0), 'mid': R(0, 0, -1.2 * sign * k)}


def idle_clip(c):
    idle = c.clip('idle', 40, loop=True)   # a subtle wobble; a bubble grows and pops
    idle.key(0, {})
    idle.key(10, pose(tremble(1, 0.8), bubble_a=M(0.3, 0, 0), bubble_b=M(0, -0.2, 0)))
    idle.key(20, pose(squash(0.12, 0), {'upper': R(1.5, 0, 0)}, bubble_a=M(0.6, 0, 0), bubble_b=M(0, -0.4, 0)))
    idle.key(26, pose(squash(0.06, 0), bubble_a=M(0.8, 0, 0), bubble_b=M(0, -0.3, 0)), 'snap')
    idle.key(27, pose(tremble(-0.4), bubble_a=M(-0.8, 0, 0), bubble_b=M(0, -0.3, 0)), 'out')  # pop
    idle.key(32, pose(tremble(-1, 0.8), bubble_a=M(-0.6, 0, 0), bubble_b=M(0, 0.4, 0)))
    idle.key(40, {})


def hop_clip(c):
    hop = c.clip('hop', 16)   # the small hop it moves by; the height comes from physics
    hop.key(0, {}, 'out')
    hop.key(3, squash(0.45, 0.3), 'in')
    hop.key(5, stretch(0.7), 'out')                # takeoff
    hop.key(9, stretch(0.3), 'inout')               # top of the hop
    hop.key(12, stretch(0.5), 'in')                 # falling
    hop.key(13, squash(0.6, 0.6), 'out')            # lands
    hop.key(15, stretch(0.1), 'inout')
    hop.key(16, {})


def attack_jump(a):
    """High jump. 12 ticks of windup: it sinks down and spreads out wide, then trembles (the
    tell, with the shadow at the landing spot). The flight is the clip jump_air: the game plays
    this clip up to the hit marker, jump_air while airborne, and this clip again from hit on at
    the landing. hit = the landing: flattened, the mud thrown out; recover = 15 ticks stuck flat."""
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    a.key(0, {}, 'out')
    a.key(3, squash(0.5, 0.6), 'inout')
    a.key(7, squash(0.9, 1.15), 'inout')
    a.key(9, pose(squash(1.0, 1.25), tremble(1)), 'linear')
    a.key(10, pose(squash(1.0, 1.25), tremble(-1)), 'linear')
    a.key(11, pose(squash(1.05, 1.3), tremble(1)), 'linear')   # deepest: here it leaves the ground
    a.key(hit, squash(1.15, 1.6), 'out')                        # the landing: flat, mud thrown out
    a.key(rec, squash(0.95, 1.45), 'hold')                      # stuck in its own mud: the window
    a.key(rec + 9, squash(0.85, 1.35), 'slow')
    a.key(end, {})


def jump_air_clip(c):
    air = c.clip('jump_air', 20)   # airborne between the windup and the landing of attack_jump
    air.key(0, squash(1.05, 1.3), 'out')
    air.key(3, stretch(1.0), 'out')                 # pulls itself up out of the mud
    air.key(10, stretch(0.35), 'inout')             # the top of the arc
    air.key(17, stretch(0.75), 'in')                # falling, stretched towards the ground
    air.key(20, stretch(0.8))


def stick_clip(c):
    stick = c.clip('stick', 14)   # the passive on contact: it leans in and the mud grabs the feet
    lean = pose(squash(0.35, 0.4), {'upper': R(10, 0, 0) + M(0, 0, -0.5), 'mid': R(5, 0, 0)},
                skirt_f=M(0, 0.25, -3.5))
    stick.key(0, {}, 'out')
    stick.key(3, lean, 'out')
    stick.key(5, pose(lean, upper=R(2, 0, 0)), 'hold')
    stick.key(10, pose(lean, upper=R(-2, 0, 0)), 'slow')
    stick.key(14, {})


def hurt_clip(c):
    hurt = c.clip('hurt', 10)   # vanilla hurt_time is 10 ticks
    hurt.key(0, {}, 'out')
    hurt.key(2, pose(squash(0.45, 0.35), upper=R(-8, 0, 3) + M(0, 0, 0.8), mid=R(-3, 0, 0)), 'back')
    hurt.key(5, pose(stretch(0.3), upper=R(4, 0, -2)), 'inout')
    hurt.key(8, pose(squash(0.12, 0), upper=R(-1, 0, 1)), 'inout')
    hurt.key(10, {})


def death_clip(c):
    """Vanilla death_time is 20 ticks. It sags, swells and bursts: the upper slab pops off, the
    roots fly out, the mud spreads. The game spawns the smaller slimes (split) at the burst."""
    death = c.clip('death', 20)
    death.key(0, {}, 'out')
    death.key(3, pose(squash(0.6, 0.5), tremble(1, 0.6)), 'inout')
    death.key(6, pose(squash(1.0, 1.0), tremble(-1)), 'in')
    death.key(8, stretch(0.9), 'out')                         # swells
    burst = pose(squash(0.8, 1.9), {
        'upper': M(0, -4.5, 0.5) + R(-14, 0, 10),
        'root_a': M(-1.5, -0.5, 0) + R(0, 0, 30),
        'root_c': M(0, -1, 1.5) + R(30, 0, 0),
        'root_b': M(1.5, 0.5, 0) + R(0, 0, 20),
        'bubble_a': M(2, -1, 0),
        'bubble_b': M(0, -2, 0),
    })
    death.key(10, burst, 'out')                               # bursts: here it splits
    death.key(14, pose(burst, upper=M(0, 1.5, 0.5) + R(-6, 0, 6)), 'in')
    puddle = pose(squash(1.4, 2.1), {
        'upper': R(-4, 0, 6) + M(0.5, 0.3, 0.6),
        'root_a': M(-1.5, 2.5, 0) + R(0, 0, -12),
        'root_c': M(0, 1, 1.5) + R(-30, 0, 0),
        'root_b': M(1.5, 1.2, 0) + R(0, 0, -6),
        'bubble_a': M(-1, 0, 0),
    })
    death.key(17, puddle, 'back')                             # falls back flat: a puddle
    death.key(20, puddle)


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}
    idle_clip(c)
    hop_clip(c)
    attack_jump(c.attack('attack_jump', attacks['attack_jump']))
    jump_air_clip(c)
    stick_clip(c)
    hurt_clip(c)
    death_clip(c)
    trophy_clip(c)


# The trophy pose: bursting up out of the mud, the skirts flung wide on the surface, the slabs
# stretched apart so the core shows. A slow loop: it heaves, a bubble rises through it.
def trophy_clip(c):
    up = pose(squash(0, 1.3), stretch(0.7))
    t = c.clip('trophy', 80, loop=True)
    t.key(0, up)
    t.key(40, pose(squash(0.25, 1.45), stretch(0.35), tremble(1, 0.5), bubble_a=M(0.6, 0, 0), bubble_b=M(0, -0.4, 0)))
    t.key(80, up)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['root'], 'scale': 0.65},
}
