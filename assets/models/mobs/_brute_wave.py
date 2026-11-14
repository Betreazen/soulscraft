"""Showcase-only prop for the brute's overhead slam: the shockwave along the ground.

A ring of 24 stone shards and a cracked crater. At rest everything sits under the ground, where
the showcase floor hides it; the wave clip pushes the shards up and out to radius 3 blocks
(data/mobs.json: wave.radius_blocks) and sinks them again. Tick 0 of the clip is the moment the
maul lands. Not a mob (no rig.json). Game model px: +y down, the ground at y 24.
"""
import math

from paint import Material
from rig import M

ID = '_brute_wave'
TEXTURE = (128, 64)
PALETTES = {
    'stone': Material(['#4a4744', '#5e5a55', '#736e67', '#89837a'], noise=0.7, patch=2),
    'dirt': ['#3e2f22', '#4f3c2b', '#614a35'],
    'crack': ['#1c1915', '#2a2520'],
}
SHARDS = 24
RADIUS = 3 * 16      # px
SPREAD = 10          # ticks from the impact to the full radius
HIDE = 9             # px under the ground


def shard_decal(face):
    if face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('stone', -1))
    elif face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 2, face.w, 2, ('dirt', 1))      # earth clinging to the underside
        face.px(1, 0, ('stone', -1))


def crater_decal(face):
    if face.name != 'top':
        return
    face.patch(4, 4, 12, 12, ('dirt', 1), 0.7, 1)
    c = 10
    for a in range(0, 360, 45):                                # cracks radiating from the centre
        r = math.radians(a + 10)
        face.line(c, c, c + round(math.sin(r) * 9), c + round(math.cos(r) * 9), ('crack', 0))
    face.rect(8, 8, 4, 4, ('crack', 1))


def model(m):
    m.part('root', (0, 24, 0))
    crater = m.part('crater', (0, 24 + 4, 0), parent='root')   # 4 px down: hidden at rest
    crater.cube((-10, -1, -10), (20, 1, 20), 'stone', decal=crater_decal)
    first = {}
    for i in range(SHARDS):
        p = m.part('shard%d' % i, (0, 24 + HIDE, 0), parent='root', rot=(0, 360 * i / SHARDS, 0))
        # Every shard stands at its own height, 0.1 px apart, and 0.1 px off its axes: near the
        # centre the shards overlap, and level tops or the faces of two opposite shards would
        # lie in one plane there (z-fighting).
        lift = 0.1 * (i + 1)
        origin, size = ((-3.9, -5 - lift, -1.4), (8, 5, 3)) if i % 2 == 0 else ((-2.9, -3 - lift, -1.4), (6, 3, 3))
        kw = {'share': first[size]} if size in first else {'decal': shard_decal}
        first.setdefault(size, p.cube(origin, size, 'stone', **kw))


def ring(r, up):
    """Shards at radius r px, raised by up px out of hiding; the crater shows while up > 0."""
    pose = {}
    for i in range(SHARDS):
        a = math.radians(360 * i / SHARDS)
        pose['shard%d' % i] = M(math.sin(a) * r, -up, math.cos(a) * r)
    return pose


def clips(c, mob):
    w = c.clip('wave', 16)
    up = M(0, -4, 0)
    w.key(0, dict(ring(6, 0)), 'linear')
    w.key(1, dict(ring(10, HIDE), crater=up), 'linear')
    w.key(SPREAD, dict(ring(RADIUS, HIDE), crater=up), 'out')
    w.key(SPREAD + 3, dict(ring(RADIUS + 4, 2), crater=up), 'linear')
    w.key(16, dict(ring(RADIUS + 6, 0)))
