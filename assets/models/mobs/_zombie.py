"""A plain zombie figure for the altar scenes of the showcase (the Rotting Horn calls zombies).
Showcase only: no rig.json, not in data/mobs.json; the game spawns vanilla zombies.

Our own box humanoid, not the vanilla model or texture: grey-green rotting skin, a torn
brown smock and dark rags, sunken glowing-less eyes. Head 8, body 8 x 12 x 4, limbs 4 x 12 x 4,
arms held forward as zombies do. Game model px, see rig.py.
"""
from paint import Material
from rig import M, R, pose

ID = '_zombie'

PALETTES = {
    'skin': Material(['#3b4a37', '#4b5c45', '#5c6e53', '#6d8062'], noise=0.7, patch=3),
    'smock': Material(['#3a2c1f', '#4a3827', '#5a4430', '#6a5039'], noise=0.6, patch=3),
    'rags': ['#232126', '#2f2c33', '#3b3840'],
    'dark': ['#0f0d0c', '#1c1916'],
    'rot': ['#2c3324', '#36402b'],
}


def head_decal(face):
    if face.name == 'front':
        face.rect(1, 3, 2, 2, ('dark', 0))
        face.rect(5, 3, 2, 2, ('dark', 0))
        face.px(2, 4, '#5a2a22')
        face.px(5, 4, '#5a2a22')
        face.rect(2, 6, 4, 1, ('dark', 1))
        face.px(3, 7, ('skin', 0))
        face.patch(0, 0, 4, 2, ('rot', 1), 0.5, 1)
    elif face.name == 'top':
        face.patch(2, 2, 4, 4, ('rot', 0), 0.5, 2)
    elif face.name in ('left', 'right'):
        face.px(5, 4, ('dark', 1))


def body_decal(face):
    if face.name in ('front', 'back'):
        face.rect(0, 9, face.w, 1, ('rags', 1))
        face.rect(0, 10, face.w, 2, ('rags', 0))
        face.px(2, 11, ('skin', 1))
        face.px(5, 10, ('skin', 1))   # torn holes show skin
        if face.name == 'front':
            face.rect(3, 0, 2, 3, ('skin', 1))
            face.patch(4, 4, 3, 3, ('smock', 0), 0.5, 3)


def arm_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 3, ('smock', 1))
        face.px(1, 3, ('smock', 1))


def leg_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 7, ('rags', 1))
        face.px(2, 7, ('rags', 1))
        face.px(1, 5, ('skin', 1))
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('dark', 1))


def model(m):
    m.part('root', (0, 24, 0))
    body = m.part('body', (0, 12, 0), parent='root')
    # 0.1 px wider than head and limbs, so a swung limb never lies in the body's side planes.
    body.cube((-4, -12, -2), (8, 12, 4), 'smock', inflate=0.1, decal=body_decal)
    head = m.part('head', (0, 0, 0), parent='body')
    head.cube((-4, -8, -4), (8, 8, 8), 'skin', decal=head_decal)
    arm_r = m.part('arm_r', (-5, 2, 0), parent='body')
    a = arm_r.cube((-3, -2, -2), (4, 12, 4), 'skin', decal=arm_decal)
    arm_l = m.part('arm_l', (5, 2, 0), parent='body')
    arm_l.cube((-1, -2, -2), (4, 12, 4), 'skin', share=a, mirror=True)
    leg_r = m.part('leg_r', (-2, 12, 0), parent='root')
    b = leg_r.cube((-2, 0, -2), (4, 12, 4), 'rags', decal=leg_decal)
    leg_l = m.part('leg_l', (2, 12, 0), parent='root')
    leg_l.cube((-2, 0, -2), (4, 12, 4), 'rags', share=b, mirror=True)


ARMS = dict(arm_r=R(-85, 4, 0), arm_l=R(-85, -4, 0), head=R(4, 0, 0), body=R(4, 0, 0))


def clips(c, mob):
    idle = c.clip('idle', 40, loop=True)
    idle.key(0, ARMS)
    idle.key(20, pose(ARMS, arm_r=R(-80, 6, 0), arm_l=R(-88, -2, 0), head=R(8, 6, 4)))
    idle.key(40, ARMS)

    walk = c.clip('walk', 24, loop=True)  # a shamble: short steps, a sway of the body
    for f, s in ((0, 1), (6, 0), (12, -1), (18, 0), (24, 1)):
        walk.key(f, pose(ARMS, leg_l=R(-22 * s, 0, 0), leg_r=R(22 * s, 0, 0), arm_r=R(-85 + 6 * s, 4, 0),
                         arm_l=R(-85 - 6 * s, -4, 0), body=R(4, 0, 4 * s), head=R(4, 0, -3 * s)))

    swing = c.clip('swing', 12)  # both arms come down on whoever is in front
    swing.key(0, ARMS, 'out')
    swing.key(5, pose(ARMS, arm_r=R(-130, 4, 0), arm_l=R(-130, -4, 0), body=R(-6, 0, 0)), 'in')
    swing.key(7, pose(ARMS, arm_r=R(-50, 4, 0), arm_l=R(-50, -4, 0), body=R(14, 0, 0)), 'slow')
    swing.key(12, ARMS)

    rise = c.clip('rise', 20)  # climbing out of the ground: the scene lifts the root from below
    low = pose(ARMS, body=R(40, 0, 0), head=R(-30, 0, 0), arm_r=R(-150, 10, 10), arm_l=R(-150, -10, -10),
               leg_l=R(-60, 0, 0), leg_r=R(-20, 0, 0), root=M(0, 4, 0))
    rise.key(0, low, 'out')
    rise.key(12, pose(ARMS, body=R(20, 0, 0), arm_r=R(-120, 4, 6), arm_l=R(-100, -4, -6)), 'slow')
    rise.key(20, ARMS)
