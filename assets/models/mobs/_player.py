"""Player figure for the showcase scenes (showcase only: no rig.json, not in data/mobs.json).

Steve-like proportions (head 8, body 8 x 12 x 4, limbs 4 x 12 x 4) in our own colours, no Mojang
texture. A wooden shield on the outer side of the left forearm, like the revenant's, so the
block pose reads the same way. Game model px, see rig.py.
"""
from rig import M, R, pose

ID = '_player'

PALETTES = {
    'skin': ['#8a6248', '#a5775a', '#ba8a6a', '#cc9d7c'],
    'hair': ['#2a1d14', '#3a281b', '#4a3423'],
    'tunic': ['#2f3d4f', '#3c4d63', '#4b5f78', '#5a7090'],
    'trousers': ['#2c2a33', '#3a3742', '#494552'],
    'boots': ['#2a1f17', '#3a2b1f', '#4a3828'],
    'wood': ['#4a3320', '#5e432a', '#735334', '#86623d'],
    'iron': ['#4e5258', '#6a6f76', '#868b92', '#a3a8ae'],
    'dark': ['#141414', '#202020'],
}


def head_decal(face):
    if face.name == 'front':
        face.rect(0, 0, 8, 2, ('hair', 1))
        face.px(0, 2, ('hair', 1))
        face.px(7, 2, ('hair', 1))
        face.rect(1, 4, 2, 1, '#e8e8e8')
        face.rect(5, 4, 2, 1, '#e8e8e8')
        face.px(2, 4, '#2b3a5a')
        face.px(5, 4, '#2b3a5a')
        face.rect(3, 6, 2, 1, ('skin', 0))
    elif face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('hair', 1))
    elif face.name in ('left', 'right', 'back'):
        face.rect(0, 0, face.w, 3 if face.name != 'back' else 6, ('hair', 1))


def body_decal(face):
    if face.name == 'front':
        face.rect(0, 10, 8, 1, ('boots', 1))  # belt
        face.px(3, 10, '#a08850')
        face.rect(3, 0, 2, 2, ('tunic', 0))   # collar


def arm_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 4, ('tunic', 1))  # sleeve; skin below


def leg_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 9, face.w, 3, ('boots', 1))
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('boots', 0))


def shield_decal(face):
    if face.name == 'left':
        face.outline(('iron', 1))
        face.rect(4, 1, 2, face.h - 2, ('iron', 0))
        face.px(4, 4, ('iron', 2))
    elif face.name == 'right':
        face.rect(0, 4, face.w, 2, ('boots', 1))


def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 12, 0), parent='root')
    # 0.1 px wider and deeper than the head, arms and legs: their sides lie in the body's planes,
    # and a pitched head or a swung limb would overlap them there (z-fighting).
    body.cube((-4, -12, -2), (8, 12, 4), 'tunic', inflate=0.1, decal=body_decal)
    head = m.part('head', (0, 0, 0), parent='body')
    head.cube((-4, -8, -4), (8, 8, 8), 'skin', decal=head_decal)
    arm_r = m.part('arm_r', (-5, 2, 0), parent='body')
    a = arm_r.cube((-3, -2, -2), (4, 12, 4), 'skin', decal=arm_decal)
    arm_l = m.part('arm_l', (5, 2, 0), parent='body')
    arm_l.cube((-1, -2, -2), (4, 12, 4), 'skin', share=a, mirror=True)
    shield = m.part('shield', (8.5, 7, 0), parent='arm_l')
    shield.cube((0, -6, -5), (1, 11, 10), 'wood', decal=shield_decal)
    leg_r = m.part('leg_r', (-2, 12, 0), parent='root')
    b = leg_r.cube((-2, 0, -2), (4, 12, 4), 'trousers', decal=leg_decal)
    leg_l = m.part('leg_l', (2, 12, 0), parent='root')
    leg_l.cube((-2, 0, -2), (4, 12, 4), 'trousers', share=b, mirror=True)
    m.socket('socket_hand_r')
    m.socket('socket_hand_l')


READY = dict(body=R(4, 0, 0), arm_l=R(-20, 10, -4), arm_r=R(-10, 0, 8))
GUARD = dict(body=R(8, 8, 0), head=R(6, -8, 0), arm_l=R(-75, 48, 0), arm_r=R(-14, 0, 10),
             leg_l=R(-10, 0, 0), leg_r=R(10, 0, 0))


def clips(c, mob):
    idle = c.clip('idle', 40, loop=True)
    idle.key(0, READY)
    idle.key(20, pose(READY, body=R(1, 0, 0) + M(0, 0.2, 0), arm_r=R(-2, 0, 1)))
    idle.key(40, READY)

    walk = c.clip('walk', 20, loop=True)
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        walk.key(f, pose(READY, leg_l=R(-28 * s, 0, 0), leg_r=R(28 * s, 0, 0), arm_r=R(24 * s, 0, 0),
                         arm_l=R(-10 * s, 0, 0), body=M(0, -0.5 if s == 0 else 0, 0)))

    block = c.clip('block', 20, loop=True)
    block.key(0, GUARD)
    block.key(10, pose(GUARD, body=M(0, 0.2, 0)))
    block.key(20, GUARD)

    dodge = c.clip('dodge', 8)  # a side roll-step; the scene moves the figure
    low = pose(READY, body=R(30, 0, -18) + M(0, 3, 0), head=R(-10, 0, 0), arm_l=R(-40, 0, -30), arm_r=R(-40, 0, 40),
               leg_l=R(-50, 0, -14), leg_r=R(20, 0, -14), root=M(0, 2, 0))
    dodge.key(0, READY, 'out')
    dodge.key(3, low, 'inout')
    dodge.key(8, READY)

    hurt = c.clip('hurt', 10)
    hurt.key(0, READY, 'out')
    hurt.key(2, pose(READY, body=R(-18, 0, 0), head=R(-20, 0, 0), arm_r=R(-40, 0, 26), arm_l=R(-30, 0, -26)), 'slow')
    hurt.key(10, READY)

    knocked = c.clip('knocked', 14)  # hit through the shield: thrown back, guard broken
    thrown = pose(READY, body=R(-26, 0, 0), head=R(-24, 0, 0), arm_l=R(-110, 20, -30), arm_r=R(-60, 0, 40),
                  leg_l=R(-30, 0, 0), leg_r=R(10, 0, 0), root=M(0, -1, 0))
    knocked.key(0, GUARD, 'out')
    knocked.key(3, thrown, 'slow')
    knocked.key(9, pose(READY, body=R(10, 0, 0), leg_l=R(-12, 0, 0)), 'inout')
    knocked.key(14, READY)

    swing = c.clip('swing', 8)  # a plain sword-arm swing into the revenant's open window
    swing.key(0, READY, 'out')
    swing.key(3, pose(READY, arm_r=R(-150, 0, 10), body=R(-6, 14, 0)), 'in')
    swing.key(5, pose(READY, arm_r=R(-30, -20, -10), body=R(16, -18, 0)), 'slow')
    swing.key(8, READY)

    # A jump: crouch, legs tucked at the top, land. The root stays on the ground, as in the game;
    # a scene lifts the figure with `lift` (SC.hop over clip ticks 2..11, about 0.8 block).
    jump = c.clip('jump', 14)
    crouch = pose(READY, body=R(18, 0, 0) + M(0, 1.5, 0), leg_l=R(-30, 0, 0), leg_r=R(-30, 0, 0),
                  arm_r=R(30, 0, 10), arm_l=R(20, 0, -10), root=M(0, 1.5, 0))
    tuck = pose(READY, body=R(8, 0, 0), leg_l=R(-40, 0, 0), leg_r=R(-10, 0, 0), arm_r=R(-60, 0, 20),
                arm_l=R(-50, 0, -30))
    jump.key(0, READY, 'out')
    jump.key(2, crouch, 'out')
    jump.key(7, tuck, 'inout')                       # the top of the jump
    jump.key(11, crouch, 'out')
    jump.key(14, READY)

    # Puts an item down in front, at chest height and a little up (the horn on the pedestal):
    # the right arm reaches out, holds, then comes back.
    place = c.clip('place', 20)
    reach = pose(READY, arm_r=R(-105, 10, 4), body=R(10, 6, 0), head=R(12, 0, 0))
    place.key(0, READY, 'out')
    place.key(6, reach, 'inout')
    place.key(12, pose(reach, arm_r=R(-98, 10, 4)), 'slow')
    place.key(20, READY)
