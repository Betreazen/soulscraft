"""Showcase-only prop for the brute's charge scene: a stone brick wall, 5 blocks wide, 3 high,
1 thick. Not a mob (no rig.json). The scene places it as an actor; wall_hit jolts the top row
when the brute rams it. Game model px, like the mob files: +y down, feet at y 24.
"""
from paint import Material
from rig import M, R

ID = '_brute_wall'
TEXTURE = (128, 128)
PALETTES = {
    'brick': Material(['#55534f', '#68665f', '#7a7870', '#8c8a81'], noise=0.6, patch=3),
    'mortar': ['#3b3a37', '#45443f'],
    'moss': ['#3c4a2a', '#4d5e33'],
}


def bricks(face, offset=0, moss=False, crack=False):
    """Stone bricks: two courses of two bricks per 16 px face, mortar between, staggered."""
    if face.name in ('top', 'bottom'):
        face.outline(('mortar', 1))
        return
    for y in (0, 8):
        face.rect(0, y, face.w, 1, ('mortar', 0))
    for y0, shift in ((0, offset), (8, offset + 8)):
        for x in (shift % 16, (shift + 8) % 16):
            face.rect(x, y0, 1, 8, ('mortar', 0))
    if moss:
        face.patch(0, 12, 9, 4, ('moss', 1), 0.55, 3)
        face.patch(10, 13, 6, 3, ('moss', 0), 0.5, 4)
    if crack:
        face.line(3, 2, 7, 6, ('mortar', 0))
        face.line(7, 6, 6, 10, ('mortar', 0))
        face.line(6, 10, 10, 14, ('mortar', 0))


def plain(face):
    bricks(face)


def mossy(face):
    bricks(face, 4, moss=True)


def cracked(face):
    bricks(face, 0, crack=True)


LAYOUT = ['pmcpm', 'mpcpc', 'cpmmp']   # bottom row first: plain, mossy, cracked


def model(m):
    m.part('root', (0, 24, 0))
    low = m.part('wall', (0, 24, 0), parent='root')
    top = m.part('wall_top', (0, -8, 0), parent='wall')
    first = {}
    decals = {'p': plain, 'm': mossy, 'c': cracked}
    for row, kinds in enumerate(LAYOUT):
        part, y = (top, -16) if row == 2 else (low, -16 * (row + 1))   # part-local top of the course
        for col, k in enumerate(kinds):
            kw = {'share': first[k]} if k in first else {'decal': decals[k]}
            first.setdefault(k, part.cube((-40 + 16 * col, y, -8), (16, 16, 16), 'brick', **kw))


def clips(c, mob):
    w = c.clip('wall', 2)
    w.key(0, {}).key(2, {})
    hit = c.clip('wall_hit', 10)   # the ram lands: the top course jolts back and settles
    hit.key(0, {}, 'out')
    hit.key(1, {'wall_top': M(0, -0.6, 1.2) + R(-3, 0, 0), 'wall': M(0, 0, 0.4)}, 'back')
    hit.key(5, {'wall_top': M(0, 0, 0.3)}, 'slow')
    hit.key(10, {})
