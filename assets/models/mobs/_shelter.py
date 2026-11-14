"""A wooden lean-to for the showcase scenes: a 3 x 3 block plank roof on four posts, 2.5 blocks
high. Showcase only (the shrike waits while the player stands under it); no rig.json, no mob.

Game model px: +x left, +y down, -z front; the root sits on the ground.
"""
from paint import Material
from rig import R

ID = '_shelter'

PALETTES = {
    'plank': Material(['#4a3520', '#5c4229', '#6f5032', '#80603c'], noise=0.4, patch=4),
    'post': Material(['#3a2a1a', '#4a3622', '#5a432b'], noise=0.4, patch=3),
}

TILE = 16          # one block of roof per cube, all sharing one sheet region
HEIGHT = 40        # underside of the roof above the ground, px


def plank_decal(face):
    if face.name in ('top', 'bottom'):
        for y in range(3, face.h, 4):
            face.rect(0, y, face.w, 1, ('plank', 0))
        for x, y in ((3, 1), (11, 5), (6, 9), (13, 13)):
            face.px(x, y, ('plank', 0))
    elif face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('plank', 0))


def post_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(1, 0, 1, face.h, ('post', 0))


def model(m):
    m.part('root', (0, 24, 0))
    roof = m.part('roof', (0, 24 - HEIGHT, 0), parent='root')
    first = None
    for i in (-1, 0, 1):
        for k in (-1, 0, 1):
            o = (i * TILE - TILE / 2, -2, k * TILE - TILE / 2)
            if first is None:
                first = roof.cube(o, (TILE, 2, TILE), 'plank', decal=plank_decal)
            else:
                roof.cube(o, (TILE, 2, TILE), 'plank', share=first)
    posts = m.part('posts', (0, 24, 0), parent='root')
    p = None
    for x in (-22, 19):
        for z in (-22, 19):
            if p is None:
                p = posts.cube((x, -HEIGHT, z), (3, HEIGHT, 3), 'post', decal=post_decal)
            else:
                posts.cube((x, -HEIGHT, z), (3, HEIGHT, 3), 'post', share=p)


def clips(c, mob):
    still = c.clip('idle', 20, loop=True)
    still.key(0, {'roof': R()}).key(20, {'roof': R()})
