"""Diorama trophies (docs/design/09-trophies.md, round 6): one block per mob, <mob>_trophy, a
miniature of the mob's home on a dark wooden plinth with brass corners and an engraved brass
plaque. The block model (this file) is the plinth and the scene; the mob figure on it is drawn by
the block-entity renderer from the mob's rig in its `trophy` clip (trophies.py fits it into
SCENES[mob]['figure'] and writes assets/rigs/<mob>.trophy.json).

What else a scene gives the mod (trophies.py copies it into the trophy file):
    light       the block's light level (research trophy-effects.md 5: one value per block)
    particles   what the block entity's server tick spawns, own particle types of PARTICLES:
                {type, every (mean ticks between two), box (block px, north-facing model)}

Glow: elements with light_emission (soul flames, glowing caps, magma cracks, gold, lantern); the
figure's own glow parts (eyes, the lure, the glint) are full-bright in the rig.

Frame px: +x east, +y up, -z north, from the north-west bottom corner; models face north, the
plaque is on the north face. Nothing leaves the block; no translucent geometry (mist, bubbles and
snow are particles, which the engine draws blended).
"""
import json
import os

import paint
from blocks import Block, Texture
from paint import Material

SET = 'trophies'
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
    MOBS = {m['id']: m for m in json.load(f)['mobs']}

PALETTES = {
    'wood': Material(['#1f140d', '#2a1b12', '#352217', '#422b1d', '#4f3424'], noise=0.4, patch=4, fine=0.08),
    'brass': Material(['#4a3518', '#6b4f22', '#8c6a2e', '#ad8a42', '#cfae62'], noise=0.25, patch=3, fine=0.05),
    'stone': Material(['#34312e', '#423e3a', '#504b46', '#5f5953', '#6f6861'], noise=0.6, patch=3, fine=0.12),
    'stone_dark': ['#191716', '#211f1d', '#2a2725', '#34302d'],
    'bone': Material(['#6f6650', '#8c826a', '#a99f84', '#c3b99c', '#d8cfb3'], noise=0.4, patch=3, fine=0.1),
    'iron': Material(['#1c1c20', '#2a2a30', '#3a3a41', '#4c4c54', '#62626b'], noise=0.35, patch=3, fine=0.08),
    'rust': ['#3b2316', '#53301e', '#6e4024', '#8a542c'],
    'soil': Material(['#1d1610', '#261d15', '#30251b', '#3a2d21'], noise=0.7, patch=2, fine=0.2),
    'grass': Material(['#202a17', '#28341c', '#313f22', '#3b4a28'], noise=0.7, patch=2, fine=0.2),
    'wax': ['#8d8670', '#a8a189', '#c2bba2', '#d6d0b9'],
    'soulfire': ['#3fd4e0', '#7befef', '#c6fbf6', '#f2fffd'],
    'bark': Material(['#20170f', '#2b1f14', '#36281a', '#433221'], noise=0.6, patch=2, fine=0.15),
    'log_end': ['#4a3622', '#5d452c', '#725638', '#866846'],
    'moss': Material(['#1f2d15', '#28391a', '#32461f', '#3d5425'], noise=0.7, patch=2, fine=0.25),
    'leaf': ['#4a2c13', '#62401c', '#7a5226', '#8f6530'],
    'fern': ['#1f3a1a', '#2a4a21', '#355b28', '#426c31'],
    'cap': ['#5a2318', '#74301f', '#8c3f28', '#a65233'],
    'glowcap': ['#2fa58f', '#58cbb0', '#9ae8cf', '#d8fff0'],
    'stem': ['#8d8670', '#a49d86', '#bcb59c'],
    'rope': ['#5a4a30', '#6f5c3d', '#85704b'],
    'sand': Material(['#7e6c4b', '#917d57', '#a48e63', '#b49f72'], noise=0.55, patch=3, fine=0.2),
    'sandstone': Material(['#6e5d42', '#806d4e', '#927e5b', '#a38f69'], noise=0.5, patch=3, fine=0.1),
    'shaft': ['#4a3420', '#5f4429', '#745434'],
    'fletch': ['#5a1f1a', '#7a2a22', '#9a3a2c'],
    'lamp': ['#d98a2b', '#f0b247', '#ffd677', '#fff1c4'],
    'rock': Material(['#26231f', '#302c28', '#3b3631', '#47413b', '#544d46'], noise=0.7, patch=2, fine=0.2),
    'scorch': ['#141110', '#1c1816', '#26201c', '#30281f'],
    'magma': ['#a8320c', '#d9561a', '#f88a2c', '#ffc35a'],
    'snow': Material(['#9fabb8', '#b5c0cb', '#cad3dc', '#dde4ea', '#eef2f5'], noise=0.35, patch=3, fine=0.1),
    'cliff': Material(['#2d3035', '#383c42', '#454950', '#53575e'], noise=0.6, patch=3, fine=0.15),
    'deadwood': Material(['#4b4740', '#5d5850', '#706a60', '#857e72'], noise=0.5, patch=2, fine=0.15),
    'feather': ['#16151b', '#201f27', '#2c2a35'],
    'ice': ['#7fc4e8', '#a9dcf3', '#d2eefa', '#f2fbff'],
    'silt': Material(['#14191c', '#192024', '#1f282d', '#263137'], noise=0.6, patch=3, fine=0.2),
    'seastone': Material(['#1a2124', '#222b2f', '#2b363b', '#354248'], noise=0.6, patch=2, fine=0.15),
    'kelp': ['#1c2f17', '#24401c', '#2e5122', '#396229'],
    'shell': ['#8a7d72', '#a6988b', '#c0b3a5', '#d8ccbe'],
    'chest': Material(['#2b1a10', '#382316', '#462d1c', '#553823'], noise=0.4, patch=3, fine=0.1),
    'gold': ['#b8851e', '#e0aa2e', '#f6d257', '#fff3a8'],
    'anemone': ['#b33a6e', '#d85a8c', '#f08ab0', '#ffc6dc'],
    'mud': Material(['#18130e', '#201912', '#292017', '#32281c'], noise=0.6, patch=3, fine=0.2),
    'pool': ['#0f1310', '#151b16', '#1c241d'],
    'lily': ['#26431c', '#325723', '#3f6b2b', '#4c7f34'],
    'reed': ['#3d4a22', '#4c5b2a', '#5d6d33'],
    'cattail': ['#3a2414', '#4b2f1a', '#5c3a21'],
    'marshglow': ['#9fbf2a', '#c3e040', '#e3f77a', '#f8ffc4'],
    'flower': ['#c9c3b0', '#e2dccb', '#f4f0e4'],
}
CREVICE = ('stone_dark', 0)
UP = ('up', 'north', 'south', 'west', 'east')      # an element standing on something
ALL = ('down', 'up', 'north', 'south', 'west', 'east')

# ---------------------------------------------------------------------------- the plinth

PLINTH_TOP = 3      # the scene's ground starts here
PLAQUE = ((4, 0.75, 0), (12, 2.25, 0.5))   # the brass plaque, front at z = 0: the mod writes the
                                           # mob's name and the owner's kills over it


def plinth(m):
    """Dark varnished wood, inset 0.25 px so the four brass corner brackets stand proud of it,
    a moulded foot and a lip under the ground; the engraved plaque on the north face."""
    m.box((0.25, 0, 0.25), (15.75, 1, 15.75), {'up': 'dio_wood', 'down': 'dio_wood', 'side': 'dio_foot'})
    m.box((0.75, 1, 0.75), (15.25, 2.5, 15.25), 'dio_panel', faces=('north', 'south', 'west', 'east'))
    m.box((0.25, 2.5, 0.25), (15.75, PLINTH_TOP, 15.75), {'up': 'dio_wood', 'down': 'dio_wood', 'side': 'dio_lip'},
          faces=UP + ('down',))
    for x in (0, 14.25):
        for z in (0, 14.25):
            m.box((x, 0, z), (x + 1.75, PLINTH_TOP + 0.25, z + 1.75), 'dio_brass_corner', faces=UP)
    m.box(PLAQUE[0], PLAQUE[1], {'north': 'dio_plaque', 'side': 'dio_brass', 'end': 'dio_brass'},
          faces=('north', 'up', 'down', 'west', 'east'))


def ground(m, tex, side, top=4.0):
    """The scene's ground: a slab inside a 0.5 px wooden rim."""
    m.box((0.75, PLINTH_TOP, 0.75), (15.25, top, 15.25), {'up': tex, 'side': side}, faces=UP)


def candle(m, x, z, y, h):
    """A bone-white candle with a soul flame on top (light_emission 15)."""
    m.box((x, y, z), (x + 1, y + h, z + 1), 'dio_wax', faces=UP)
    m.box((x + 0.3, y + h, z + 0.3), (x + 0.7, y + h + 1, z + 0.7), 'dio_soulfire', faces=UP, glow=15)


def mushroom(m, x, z, y, stem, cap, glow):
    w = cap / 2
    m.box((x - 0.25, y, z - 0.25), (x + 0.25, y + stem, z + 0.25), 'dio_stem', faces=UP)
    m.box((x - w, y + stem, z - w), (x + w, y + stem + 0.6, z + w), 'dio_glowcap' if glow else 'dio_cap', faces=ALL,
          glow=12 if glow else 0)


# ---------------------------------------------------------------------------- scenes

def revenant(m):
    """A graveyard at night: a grave mound with a headstone and a leaning second stone, soul
    candles, a broken sword stuck in the earth with its snapped point beside it, a skull."""
    ground(m, 'dio_grave', 'dio_soil_side')
    m.box((1.5, 4, 10), (7.5, 4.75, 15), 'dio_grave', faces=UP)                      # the mound
    m.box((2.5, 4.75, 12.75), (6.5, 11, 14), 'dio_headstone', faces=UP)               # headstone
    m.box((3, 11, 12.75), (6, 11.75, 14), 'dio_headstone', faces=UP)
    m.box((10.5, 4, 12.5), (14, 8.5, 13.5), 'dio_headstone', faces=ALL, rotation=('x', -22.5, (12.25, 4, 13)))
    m.box((10, 4, 9.5), (15, 4.5, 14.75), 'dio_grave', faces=UP)                     # the second grave
    for x, z, y, h in ((2, 10.75, 4.75, 2.25), (6.25, 11, 4.75, 1.5), (1.75, 2, 4, 1.75), (13.25, 10, 4.5, 1.25)):
        candle(m, x, z, y, h)
    # The broken sword: hilt up, the blade snapped a hand's width into the ground.
    sw = ('z', 22.5, (12.6, 4, 4.75))
    m.box((12.25, 4, 4.5), (12.95, 7.5, 5), 'dio_blade', faces=UP, rotation=sw)
    m.box((11, 7.5, 4.35), (14.2, 8.1, 5.15), 'dio_iron', faces=ALL, rotation=sw)
    m.box((12.2, 8.1, 4.45), (13, 10, 5.05), 'dio_grip', faces=UP, rotation=sw)
    m.box((12.1, 10, 4.35), (13.1, 10.7, 5.15), 'dio_iron', faces=UP, rotation=sw)
    m.box((8.5, 4, 1.75), (11.5, 4.35, 2.45), 'dio_blade', faces=UP, rotation=('y', -22.5, (10, 4, 2.1)))
    # A skull with dark sockets, a thigh bone.
    m.box((3.5, 4, 3), (5.5, 5.6, 4.75), 'dio_bone', faces=UP)
    for x in (3.8, 4.7):
        m.box((x, 4.6, 2.9), (x + 0.5, 5.1, 3.05), 'dio_socket', faces=('north', 'up', 'down', 'west', 'east'))
    m.box((5.75, 4, 1.25), (9, 4.45, 1.85), 'dio_bone', faces=UP, rotation=('y', 22.5, (7.4, 4, 1.55)))
    # Dead grass tufts.
    for x, z, h in ((8, 13, 1.5), (14, 6.5, 1.25), (1.25, 7, 1.75), (9.5, 8.5, 1)):
        m.box((x, 4, z), (x + 0.5, 4 + h, z + 0.5), 'dio_tuft', faces=UP)


def stalker(m):
    """A forest floor at dusk: a fallen log with moss and glowing caps, ferns, a snare on a bent
    sapling, a ribcage of something it caught, fallen leaves."""
    ground(m, 'dio_forest', 'dio_soil_side')
    m.box((1, 4, 12), (13, 6.75, 14.5), {'side': 'dio_bark', 'up': 'dio_bark', 'west': 'dio_log_end', 'east': 'dio_log_end'},
          faces=UP)
    m.box((3, 6.75, 12.5), (8.5, 7.1, 14), 'dio_moss', faces=UP)
    mushroom(m, 10, 13, 6.75, 1, 1.5, True)
    mushroom(m, 11.75, 13.6, 6.75, 0.6, 1, True)
    mushroom(m, 13.5, 5.25, 4, 1.5, 2, True)
    mushroom(m, 14, 7, 4, 0.8, 1.2, False)
    mushroom(m, 2.25, 13.75, 4, 1.2, 1.4, True)
    # Ferns: fronds leaning out from a clump.
    for ang, ax, dx, dz in ((45, 'z', 0, 0), (-45, 'z', 0.6, 0.4), (22.5, 'x', 0.3, -0.2), (-22.5, 'x', 0.35, 0.6)):
        x, z = 2.4 + dx, 4.4 + dz
        m.box((x, 4, z), (x + 0.5, 8, z + 0.5), 'dio_fern', faces=ALL, rotation=(ax, ang, (x + 0.25, 4, z + 0.25)))
    # The snare: a bent sapling, the cord hanging, the noose on the ground.
    m.box((14, 4, 9.5), (14.6, 11, 10.1), 'dio_bark', faces=UP, rotation=('z', 22.5, (14.3, 4, 9.8)))
    m.box((11.6, 6, 9.65), (11.85, 10.4, 9.9), 'dio_rope', faces=ALL)
    m.box((10.25, 4, 8.25), (13.25, 4.3, 8.55), 'dio_rope', faces=UP)
    m.box((10.25, 4, 10.95), (13.25, 4.3, 11.25), 'dio_rope', faces=UP)
    m.box((10.25, 4, 8.55), (10.55, 4.35, 10.95), 'dio_rope', faces=UP)
    m.box((12.95, 4, 8.55), (13.25, 4.35, 10.95), 'dio_rope', faces=UP)
    # A ribcage: a spine and four ribs.
    m.box((4.5, 4, 1.25), (9.5, 4.6, 1.85), 'dio_bone', faces=UP)
    for i, x in enumerate((5, 6.25, 7.5, 8.75)):
        m.box((x, 4, 1.85), (x + 0.45, 5.4 - 0.2 * i, 3.6), 'dio_bone', faces=UP + ('down',), rotation=('x', -22.5, (x, 4, 1.85)))
    # Leaves.
    for x, z, y in ((7, 6, 4), (3.5, 9, 4), (12, 3, 4), (9, 10, 4), (5.5, 3.5, 4)):
        m.box((x, y, z), (x + 1, y + 0.2, z + 0.8), 'dio_leaf', faces=UP)


def marksman(m):
    """A ruined ledge in the badlands: a low sandstone platform, the broken stump of a wall with
    an arrow slit behind, a hooded lantern on the corner, spent bolts in the sand and the stone."""
    ground(m, 'dio_sand', 'dio_sand_side')
    top = {'side': 'dio_brick', 'up': 'dio_ruin_top'}
    m.box((1, 4, 5.5), (15, LEDGE, 15), top, faces=UP)
    m.box((10.5, 4, 3.5), (15, 5, 5.5), top, faces=UP)                               # a broken step
    # The wall behind: three heights, broken.
    m.box((1, LEDGE, 13), (6, 14.5, 15), top, faces=UP)
    m.box((1, 14.5, 13.5), (3.5, 15.5, 15), top, faces=UP)
    m.box((6, LEDGE, 13.5), (10.5, 10.5, 15), top, faces=UP)
    m.box((10.5, LEDGE, 14), (15, 7.75, 15), top, faces=UP)
    m.box((3, 9, 12.9), (4, 12.5, 13), 'dio_slit', faces=('north', 'up', 'down', 'west', 'east'))
    # Fallen blocks.
    m.box((6, 4, 1.5), (8.5, 5.5, 3.5), 'dio_brick', faces=UP, rotation=('y', 22.5, (7.25, 4, 2.5)))
    m.box((12, LEDGE, 11), (13.75, LEDGE + 1.1, 12.75), 'dio_brick', faces=UP)
    m.box((1.5, 4, 3), (6, 4.6, 5.75), 'dio_sand', faces=UP)                         # sand drift
    # The lantern on the ledge corner.
    lx, lz, ly = 1.75, 6, LEDGE
    m.box((lx, ly, lz), (lx + 1.6, ly + 0.4, lz + 1.6), 'dio_iron', faces=UP)
    m.box((lx + 0.3, ly + 0.4, lz + 0.3), (lx + 1.3, ly + 1.8, lz + 1.3), 'dio_lamp', faces=('north', 'south', 'west', 'east'), glow=14)
    m.box((lx, ly + 1.8, lz), (lx + 1.6, ly + 2.2, lz + 1.6), 'dio_iron', faces=ALL)
    m.box((lx + 0.6, ly + 2.2, lz + 0.6), (lx + 1, ly + 2.8, lz + 1), 'dio_iron', faces=UP)
    for x in (lx + 0.1, lx + 1.25):
        for z in (lz + 0.1, lz + 1.25):
            m.box((x, ly + 0.4, z), (x + 0.25, ly + 1.8, z + 0.25), 'dio_iron', faces=('north', 'south', 'west', 'east'))
    # Spent bolts: in the sand, in the ledge face, one lying flat.
    for x, z, ax, ang in ((3, 2, 'x', -45), (10, 3, 'z', 22.5), (13.5, 2, 'x', 22.5)):
        bolt(m, x, 4, z, ax, ang)
    m.box((8, 5, 4.7), (8.4, 5.4, 5.5), 'dio_shaft', faces=('north', 'up', 'down', 'west', 'east'))
    m.box((7.85, 4.85, 4.3), (8.55, 5.55, 4.7), 'dio_fletch', faces=ALL)
    m.box((4.5, 4, 0.9), (8.5, 4.3, 1.2), 'dio_shaft', faces=UP)
    # A bleached rib on the ledge and a dry shrub in the sand.
    m.box((7, LEDGE, 12), (9, LEDGE + 0.4, 12.5), 'dio_bone', faces=UP)
    for ang, z in ((-22.5, 4.6), (22.5, 4.75)):
        m.box((2.2, 4, z), (2.5, 6.5, z + 0.3), 'dio_twig', faces=ALL, rotation=('z', ang, (2.35, 4, z + 0.15)))


LEDGE = 6     # top of the marksman's ledge, px


def bolt(m, x, y, z, axis, ang):
    """A crossbow bolt stuck in the ground: a shaft 3 px out, red fletching at the end."""
    r = (axis, ang, (x + 0.2, y, z + 0.2))
    m.box((x, y, z), (x + 0.4, y + 2.6, z + 0.4), 'dio_shaft', faces=UP, rotation=r)
    m.box((x - 0.15, y + 2.6, z - 0.15), (x + 0.55, y + 3.3, z + 0.55), 'dio_fletch', faces=ALL, rotation=r)


def brute(m):
    """Where it slammed: cracked rock with a wide scorched crater, glowing cracks where the maul
    struck, the brute roaring in it with the maul on its shoulder, the shattered wall it rammed
    behind, rubble."""
    # The ground with the crater sunk into it down to the plinth: the brute stands in it. The
    # glowing cracks are plates 0.05 px proud of the scorched floor that do not overlap one
    # another; the feet stand on them (CRATER).
    side = {'up': 'dio_rock', 'side': 'dio_rock_side'}
    for frm, to in (((0.75, 3, 0.75), (15.25, 4, 1.5)), ((0.75, 3, 11.75), (15.25, 4, 15.25)),
                    ((0.75, 3, 1.5), (2.5, 4, 11.75)), ((14.5, 3, 1.5), (15.25, 4, 11.75))):
        m.box(frm, to, side, faces=UP)
    m.box((2.5, 3, 1.5), (14.5, CRATER - 0.05, 11.75), 'dio_scorch', faces=('up',))  # the crater floor
    for frm, to in (((1, 3, 0.9), (3.25, 4.75, 2.75)), ((13.25, 3, 0.9), (15.1, 4.6, 2.5)),
                    ((1.25, 3, 9.75), (3, 4.6, 12.25)), ((13.5, 3, 10), (15.1, 5, 12.25))):
        m.box(frm, to, 'dio_rock', faces=UP)                                            # its broken rim
    for frm, to in (((5, 3, 5.75), (7.75, CRATER, 6.25)), ((8.25, 3, 5.75), (11, CRATER, 6.25)),
                    ((7.75, 3, 3), (8.25, CRATER, 9.5)), ((4, 3, 8.5), (7.75, CRATER, 9)),
                    ((8.25, 3, 3.5), (11.5, CRATER, 4))):
        m.box(frm, to, 'dio_magma', faces=UP, glow=13)                                 # the cracks glow
    # The shattered wall: three jagged pieces.
    m.box((1, 4, 12.5), (5.5, 12.5, 15), 'dio_rubble_brick', faces=UP)
    m.box((1, 12.5, 13), (3.5, 14, 15), 'dio_rubble_brick', faces=UP)
    m.box((5.5, 4, 13), (9.5, 6.5, 15), 'dio_rubble_brick', faces=UP)
    m.box((9.5, 4, 12.25), (15, 10, 15), 'dio_rubble_brick', faces=UP)
    m.box((12, 10, 12.75), (15, 11.5, 15), 'dio_rubble_brick', faces=UP)
    # Thrown blocks and chips.
    m.box((1.25, CRATER - 0.05, 5), (3.25, 4.8, 7), 'dio_rubble_brick', faces=UP, rotation=('y', -22.5, (2.25, 4, 6)))
    m.box((6, 4, 11.9), (8, 4.9, 12.9), 'dio_rubble_brick', faces=UP)
    for x, z, y in ((4.5, 2, CRATER - 0.05), (12, 9.75, CRATER - 0.05), (1.25, 3.5, 4), (9.25, 11.85, 4)):
        m.box((x, y, z), (x + 0.75, y + 0.5, z + 0.75), 'dio_rock', faces=UP)


CRATER = 3.25   # the brute's crater floor, px


def shrike(m):
    """A wind-scoured crag under snow: a dead tree rising at the back, its bare branch reaching
    across where the shrike has landed, ice crystals catching the light, black feathers and a
    prey's skull in the snow."""
    ground(m, 'dio_snow', 'dio_cliff_side')
    rock = {'side': 'dio_cliff', 'up': 'dio_snow'}
    m.box((1, 4, 10), (6.5, 6, 15), rock, faces=UP)                                  # the crag, stacked
    m.box((1.5, 6, 11.5), (4.5, 8, 14.5), rock, faces=UP)
    m.box((1.75, 8, 12.5), (3.5, 9, 14), rock, faces=UP)
    m.box((4.5, 6, 12), (6, 6.4, 14.5), 'dio_snow', faces=UP)
    m.box((9.5, 4, 11.5), (15, 5.5, 15), rock, faces=UP)
    m.box((13, 4, 7), (15, 4.75, 11.5), rock, faces=UP)
    # The dead tree: a trunk on the right rock, the branch across, twigs.
    m.box((11.75, 5.5, 12.25), (13.25, 15.5, 13.75), 'dio_deadwood', faces=UP)
    m.box((12.25, 15.5, 12.75), (12.75, 16, 13.25), 'dio_deadwood', faces=UP)
    m.box((11.9, 11, 11.5), (12.6, 11.6, 12.25), 'dio_deadwood', faces=ALL)          # a stub
    m.box((4, BRANCH - 1, 7.4), (11.5, BRANCH, 8.6), 'dio_deadwood', faces=ALL)
    m.box((11.5, BRANCH - 0.9, 7.6), (12.35, BRANCH - 0.1, 12.25), 'dio_deadwood', faces=ALL)   # bends into the trunk
    m.box((4.5, BRANCH, 7.6), (6.5, BRANCH + 0.3, 8.4), 'dio_snow', faces=UP)
    m.box((4.5, BRANCH - 0.5, 7.7), (4.9, BRANCH + 2.5, 8.1), 'dio_deadwood', faces=ALL, rotation=('z', 45, (4.7, BRANCH, 7.9)))
    m.box((3, BRANCH - 1.5, 7.75), (4, BRANCH - 0.6, 8.25), 'dio_deadwood', faces=ALL)
    m.box((13.25, 13, 12.75), (14.25, 13.5, 13.25), 'dio_deadwood', faces=ALL)
    m.box((13.75, 13.5, 12.75), (14.25, 14.75, 13.25), 'dio_deadwood', faces=UP)
    # Ice crystals with a cold glow.
    for x, z, y, h, ang in ((5, 10.5, 6, 1.8, 22.5), (2.5, 11.75, 8, 1.2, -22.5), (14, 8, 4.75, 1.5, 22.5),
                            (10, 12, 5.5, 1, -22.5)):
        m.box((x, y, z), (x + 0.6, y + h, z + 0.6), 'dio_ice', faces=UP, glow=7, rotation=('z', ang, (x + 0.3, y, z + 0.3)))
    # Feathers and a skull.
    for x, z, ang in ((5, 3, 22.5), (9.5, 5, -45), (2.5, 6, -22.5), (7.5, 9.5, 45)):
        m.box((x, 4, z), (x + 0.6, 4.2, z + 2), 'dio_feather', faces=UP, rotation=('y', ang, (x + 0.3, 4, z + 1)))
    m.box((11.5, 4, 2), (13, 5.1, 3.4), 'dio_bone', faces=UP)
    m.box((11.8, 4.4, 1.9), (12.2, 4.8, 2), 'dio_socket', faces=('north', 'up', 'down', 'west', 'east'))
    m.box((12.35, 4.4, 1.9), (12.75, 4.8, 2), 'dio_socket', faces=('north', 'up', 'down', 'west', 'east'))


BRANCH = 7.25   # top of the shrike's branch, px


def chest(m):
    """The sunken chest, square to the block so its halves line up: a box of planks 0.5 px thick
    sunk 0.5 px into the silt, iron straps and a latch on the front, gold heaped inside (the only
    glow), a few coins spilled in front; the lid hinged on the back top edge, opened 45 degrees,
    with its own straps. Straps stand 0.1 px proud of the planks."""
    x0, x1, z0, z1, y0, y1 = CHEST
    wall = {'side': 'dio_chest', 'end': 'dio_chest_end'}
    m.box((x0, y0, z0), (x1, y1, z0 + 0.5), wall, faces=UP)                          # front
    m.box((x0, y0, z1 - 0.5), (x1, y1, z1), wall, faces=UP)                          # back
    m.box((x0, y0, z0 + 0.5), (x0 + 0.5, y1, z1 - 0.5), wall, faces=('up', 'west', 'east'))
    m.box((x1 - 0.5, y0, z0 + 0.5), (x1, y1, z1 - 0.5), wall, faces=('up', 'west', 'east'))
    for x in (x0 + 0.75, x1 - 1.25):                                                 # straps
        m.box((x, y0 + 0.5, z0 - 0.1), (x + 0.5, y1, z0), 'dio_iron', faces=('north', 'up', 'west', 'east'))
    mid = (x0 + x1) / 2
    m.box((mid - 0.35, y1 - 1.3, z0 - 0.2), (mid + 0.35, y1 - 0.2, z0), 'dio_iron', faces=('north', 'up', 'down', 'west', 'east'))
    # Gold heaped inside, higher in the middle, a little over the rim.
    m.box((x0 + 0.5, y0, z0 + 0.5), (x1 - 0.5, y1 - 0.9, z1 - 0.5), 'dio_gold', faces=('up',), glow=10)
    m.box((x0 + 1, y1 - 0.9, z0 + 0.8), (x1 - 1.2, y1 - 0.3, z1 - 0.9), 'dio_gold', faces=UP, glow=10)
    m.box((mid - 0.9, y1 - 0.3, z0 + 1.1), (mid + 0.6, y1 + 0.2, z1 - 1.3), 'dio_gold', faces=UP, glow=10)
    for x, z, w in ((x0 + 0.8, z0 - 1.3, 0.8), (mid + 0.6, z0 - 0.9, 0.7), (x1 + 0.2, z0 + 0.6, 0.6)):
        m.box((x, 4, z), (x + w, 4.15, z + w), 'dio_gold', faces=UP, glow=10)        # spilled coins
    # The lid: planks and two straps, hinged on the back top edge, 45 degrees open.
    lid = ('x', 45, (mid, y1, z1))
    m.box((x0, y1, z0), (x1, y1 + 0.75, z1), {'side': 'dio_chest', 'end': 'dio_chest_end'}, faces=ALL, rotation=lid)
    for x in (x0 + 0.75, x1 - 1.25):
        m.box((x, y1 + 0.75, z0 + 0.25), (x + 0.5, y1 + 0.85, z1 - 0.25), 'dio_iron', faces=('up', 'north', 'south', 'west', 'east'),
              rotation=lid)
        m.box((x, y1, z0 - 0.1), (x + 0.5, y1 + 0.85, z0), 'dio_iron', faces=('north', 'up', 'down', 'west', 'east'), rotation=lid)


CHEST = (1.75, 6.75, 11.25, 14.25, 3.5, 6.5)   # x0, x1, z0, z1, y0 (0.5 px in the silt), y1 (the rim)


def lurker(m):
    """The seabed in the dark: dark silt and stones, kelp, a sunken chest with its lid ajar and
    the gold showing, a rusted anchor, shells, glowing anemones."""
    ground(m, 'dio_silt', 'dio_silt_side')
    chest(m)
    # The anchor, stuck at an angle.
    an = ('z', -22.5, (13, 4, 5))
    m.box((12.6, 4, 4.6), (13.4, 11, 5.4), 'dio_rust', faces=UP, rotation=an)
    m.box((10.5, 4.6, 4.65), (15.5, 5.4, 5.35), 'dio_rust', faces=ALL, rotation=an)
    m.box((11.5, 10, 4.7), (14.5, 10.6, 5.3), 'dio_rust', faces=ALL, rotation=an)
    m.box((12.5, 11, 4.75), (13.5, 12, 5.25), 'dio_rust', faces=ALL, rotation=an)
    # Kelp: three strands with blades.
    for x, z, h in ((13, 12.5, 11.5), (10.5, 13.75, 9), (13.5, 9.5, 9)):
        m.box((x, 4, z), (x + 0.7, 4 + h, z + 0.5), 'dio_kelp', faces=UP)
        for i, y in enumerate(range(6, int(3 + h), 2)):
            dx = 0.7 if i % 2 == 0 else -0.9
            m.box((x + dx, y, z + 0.1), (x + dx + 0.9, y + 1.2, z + 0.4), 'dio_kelp', faces=ALL)
    # Stones, shells, anemones.
    m.box((9, 4, 10.5), (12, 5.5, 13), 'dio_seastone', faces=UP)
    m.box((1, 4, 1.5), (3.5, 5, 3.5), 'dio_seastone', faces=UP)
    for x, z in ((6, 2), (9.5, 3), (4, 8.5)):
        m.box((x, 4, z), (x + 1.2, 4.4, z + 1), 'dio_shell', faces=UP)
    for x, z, y in ((9.75, 11, 5.5), (2, 2, 5), (14, 8, 4)):
        m.box((x, y, z), (x + 0.8, y + 0.9, z + 0.8), 'dio_anemone', faces=UP, glow=9)
        m.box((x + 0.2, y + 0.9, z + 0.2), (x + 0.6, y + 1.4, z + 0.6), 'dio_anemone', faces=UP, glow=9)


def bog_slime(m):
    """A bog: thick mud with dark pools, lily pads and a flower, reeds and cattails, a half-sunk
    log with glowing marsh fungus, roots arching out of the mud, mud bubbles."""
    m.box((0.75, PLINTH_TOP, 0.75), (15.25, MUD, 15.25), {'up': 'dio_mud', 'side': 'dio_mud_side'}, faces=UP)
    for frm, to in (((1.5, MUD, 1.5), (5.5, MUD + 0.1, 5)), ((10.5, MUD, 1), (14.5, MUD + 0.1, 4.5))):
        m.box(frm, to, 'dio_pool', faces=UP)
    for x, z in ((2.25, 2.25), (12, 2), (3.5, 3.75)):
        m.box((x, MUD + 0.1, z), (x + 1.6, MUD + 0.3, z + 1.4), 'dio_lily', faces=UP)
    m.box((12.6, MUD + 0.3, 2.5), (13.2, MUD + 0.8, 3.1), 'dio_flower', faces=UP)
    # The log at the back with fungus.
    m.box((6, MUD - 0.5, 12), (15, MUD + 2, 14.5), {'side': 'dio_bark', 'up': 'dio_bark', 'west': 'dio_log_end', 'east': 'dio_log_end'},
          faces=UP)
    for x, z, y in ((8, 12, MUD + 0.6), (11, 12, MUD + 1.1), (13.5, 14.7, MUD + 0.8)):
        m.box((x, y, z - 0.6), (x + 1.4, y + 0.4, z), 'dio_marshglow', faces=ALL, glow=12)
    # Roots arching out of the mud.
    m.box((1, MUD - 0.5, 9.5), (6, MUD + 0.8, 10.3), 'dio_bark', faces=UP, rotation=('z', 22.5, (3.5, MUD, 9.9)))
    m.box((13, MUD - 0.5, 6), (13.8, MUD + 0.9, 10.5), 'dio_bark', faces=UP, rotation=('x', -22.5, (13.4, MUD, 8.25)))
    # Reeds and cattails.
    for x, z, h in ((1.5, 12.5, 9), (2.5, 13.75, 7.5), (3.6, 12.8, 8.5), (14.5, 9, 6.5)):
        m.box((x, MUD, z), (x + 0.4, MUD + h, z + 0.4), 'dio_reed', faces=UP)
        m.box((x - 0.15, MUD + h - 1.8, z - 0.15), (x + 0.55, MUD + h - 0.4, z + 0.55), 'dio_cattail', faces=ALL)
    # Mud thrown up in a ring where it burst out.
    for frm, to in (((3, MUD, 4.25), (5.75, MUD + 0.6, 6)), ((10, MUD, 4.5), (12.25, MUD + 0.8, 6.5)),
                    ((2.5, MUD, 8), (4.25, MUD + 0.5, 10.25)), ((10.5, MUD, 9), (12.5, MUD + 0.7, 11)),
                    ((6, MUD, 3), (8.5, MUD + 0.4, 4.25))):
        m.box(frm, to, 'dio_mud', faces=UP)
    # Mud bubbles (the particles pop above them).
    for x, z, s in ((9, 2.25, 0.8), (6.5, 1.5, 0.6), (13.5, 6, 0.5)):
        m.box((x, MUD, z), (x + s, MUD + s * 0.6, z + s), 'dio_mud', faces=UP)


MUD = 5   # top of the bog's mud, px

# Each scene: build; the figure's box (block px from the bottom centre, as trophies.py fits it:
# lo, hi, and the floor its lowest point stands on, below the ground when it sinks in), its
# turn from facing the plaque (degrees), the block light, particles (PARTICLES types; box in
# block px of the north-facing model), what the scene is.
SCENES = {
    'revenant': dict(build=revenant, light=4, turn=-20, flags=(),
                     figure=dict(lo=(-5.5, 4, -5), hi=(4.75, 15.75, 3.75), floor=4),
                     particles=[dict(type='mist', every=6, box=((1.5, 4.2, 1.5), (14.5, 5, 14.5))),
                                dict(type='wisp', every=12, box=((1.5, 6.5, 2), (7, 7.5, 11.5)))],
                     text='Кладбище ночью: могила с плитой, свечи с огнём душ, сломанный меч в земле, череп'),
    'stalker': dict(build=stalker, light=3, turn=15, flags=(),
                    figure=dict(lo=(-5.5, 4, -6), hi=(5, 15.75, 3.75), floor=4),
                    particles=[dict(type='spore', every=10, box=((1.5, 5, 1.5), (14.5, 11, 12))),
                               dict(type='mist', every=12, box=((1.5, 4.2, 1.5), (14.5, 4.8, 11)))],
                    text='Лесная подстилка в сумерках: упавший ствол со светящимися грибами, папоротник, силок, рёбра добычи'),
    'marksman': dict(build=marksman, light=3, turn=30, flags=(),
                     figure=dict(lo=(-5.75, LEDGE, -2.25), hi=(6.5, 15.75, 4.75), floor=LEDGE),
                     particles=[dict(type='dust', every=7, box=((0.5, 4.5, 1), (4, 9, 7)))],
                     text='Руины в пустоши: уступ из песчаника, стена с бойницей, фонарь, болты в песке'),
    'brute': dict(build=brute, light=5, turn=-15, flags=('rage',),
                  figure=dict(lo=(-7.5, CRATER, -6.5), hi=(7.5, 15.75, 4.25), floor=CRATER),
                  particles=[dict(type='ember', every=5, box=((4, 3.5, 3), (12, 4, 10))),
                             dict(type='smoke', every=16, box=((4, 3.8, 3), (12, 4.4, 10)))],
                  text='Место удара: треснувший камень, воронка со светящимися трещинами, разбитая стена'),
    'shrike': dict(build=shrike, light=1, turn=25, flags=(),
                   figure=dict(lo=(-6.5, 4.5, -5.5), hi=(3.25, 15.75, 3.5), floor=BRANCH, feet=('leg_r', 'leg_l')),
                   particles=[dict(type='snow', every=3, box=((1, 15.5, 1), (15, 16, 15))),
                              dict(type='mist', every=14, box=((1, 4.2, 1), (15, 5, 8)))],
                   text='Скала под снегом: мёртвое дерево, на ветке которого сел сорокопут, лёд, перья и череп добычи'),
    'lurker': dict(build=lurker, light=2, turn=-20, flags=(),
                   figure=dict(lo=(-6.5, 5.5, -6.5), hi=(5.5, 15.75, 3.5), floor=5.5),
                   particles=[dict(type='bubble', every=8, box=((2.5, 6.8, 11.75), (6, 7.3, 13.75))),
                              dict(type='bubble', every=14, box=((2, 5, 1.5), (14, 6, 12))),
                              dict(type='glimmer', every=10, box=((1, 5, 1), (15, 14, 15)))],
                   text='Морское дно во тьме: ил, водоросли, приоткрытый затонувший сундук с золотом, якорь, ракушки, светящиеся актинии'),
    'bog_slime': dict(build=bog_slime, light=3, turn=10, flags=(),
                      figure=dict(lo=(-6.25, MUD - 1.25, -6.5), hi=(5.75, 15.75, 3.75), floor=MUD - 1.25),
                      particles=[dict(type='mud_bubble', every=9, box=((4.5, MUD + 0.2, 1), (11, MUD + 0.4, 9))),
                                 dict(type='mist', every=7, box=((1, MUD + 0.2, 1), (15, MUD + 1, 15))),
                                 dict(type='spore', every=20, box=((5, 7, 10), (15, 10, 15)))],
                      text='Топь: густая грязь с омутами, кувшинки, рогоз, полузатонувшее бревно со светящимися грибами, корни'),
}
assert set(SCENES) <= set(MOBS), 'a scene for a mob that is not in data/mobs.json'

# ---------------------------------------------------------------------------- particles

# Own particle types (engine API 103, NM: a sprite set and a declarative horizon:behaviour):
# build_blocks.py writes assets/soulscraft/particles/<id>.json and textures/particle/<id>.png.
# Sizes are billboard half-widths in blocks: a diorama is small, vanilla smoke would be a cloud.
# behaviour fields as the engine reads them (velocity blocks/tick, gravity, friction, lifetime
# ticks, size and alpha [start, end]). The sprite: 8 x 8, colours with alpha (the engine blends
# particles; below alpha 0.1 it cuts).
PARTICLES = {
    'mist': dict(name='Туман', colour='#b9c4cc', shape='soft',
                 behaviour=dict(velocity=[0.0, 0.0015, 0.0], velocity_jitter=[0.004, 0.0005, 0.004], gravity=0.0, friction=1.0,
                                lifetime=60, size=[0.07, 0.14], alpha=[0.4, 0.0])),
    'wisp': dict(name='Огонёк души', colour='#7befef', shape='core',
                 behaviour=dict(velocity=[0.0, 0.012, 0.0], velocity_jitter=[0.002, 0.002, 0.002], gravity=0.0, friction=1.0,
                                lifetime=30, size=[0.035, 0.01], alpha=[1.0, 0.0])),
    'ember': dict(name='Искра', colour='#ff9a3a', shape='core',
                  behaviour=dict(velocity=[0.0, 0.014, 0.0], velocity_jitter=[0.004, 0.004, 0.004], gravity=0.0, friction=0.98,
                                 lifetime=26, size=[0.03, 0.008], alpha=[1.0, 0.2])),
    'smoke': dict(name='Дым', colour='#2a2622', shape='soft',
                  behaviour=dict(velocity=[0.0, 0.008, 0.0], velocity_jitter=[0.002, 0.001, 0.002], gravity=0.0, friction=1.0,
                                 lifetime=50, size=[0.05, 0.13], alpha=[0.5, 0.0])),
    'snow': dict(name='Снег', colour='#f2f6fa', shape='dot',
                 behaviour=dict(velocity=[0.0, -0.012, 0.0], velocity_jitter=[0.004, 0.002, 0.004], gravity=0.0, friction=1.0,
                                lifetime=55, size=[0.025, 0.02], alpha=[1.0, 0.6])),
    'spore': dict(name='Спора', colour='#c8e070', shape='core',
                  behaviour=dict(velocity=[0.0, 0.002, 0.0], velocity_jitter=[0.003, 0.003, 0.003], gravity=0.0, friction=1.0,
                                 lifetime=70, size=[0.022, 0.016], alpha=[1.0, 0.0])),
    'dust': dict(name='Песок', colour='#c9b183', shape='dot',
                 behaviour=dict(velocity=[0.008, 0.001, 0.0], velocity_jitter=[0.002, 0.002, 0.002], gravity=0.0, friction=1.0,
                                lifetime=40, size=[0.022, 0.014], alpha=[0.8, 0.0])),
    'bubble': dict(name='Пузырь', colour='#9fd4ee', shape='ring',
                   behaviour=dict(velocity=[0.0, 0.02, 0.0], velocity_jitter=[0.002, 0.001, 0.002], gravity=0.0, friction=1.0,
                                  lifetime=28, size=[0.02, 0.03], alpha=[0.9, 0.6])),
    'mud_bubble': dict(name='Пузырь грязи', colour='#5a4a36', shape='ring',
                       behaviour=dict(velocity=[0.0, 0.002, 0.0], gravity=0.0, friction=1.0,
                                      lifetime=12, size=[0.02, 0.05], alpha=[1.0, 0.0])),
    'glimmer': dict(name='Планктон', colour='#9ff0e6', shape='core',
                    behaviour=dict(velocity=[0.0, 0.002, 0.0], velocity_jitter=[0.002, 0.002, 0.002], gravity=0.0, friction=1.0,
                                   lifetime=80, size=[0.018, 0.012], alpha=[1.0, 0.0])),
}
for _s in SCENES.values():
    for _p in _s['particles']:
        assert _p['type'] in PARTICLES, _p['type']


def particle_image(spec):
    """The 8 x 8 sprite of a particle type: soft (a blurred blob), core (a bright centre with a
    falloff), dot (a small solid dot), ring (a bubble: a rim, a highlight, a clear middle)."""
    img = paint.Image(8, 8)
    c = spec['colour']
    r, g, b = int(c[1:3], 16), int(c[3:5], 16), int(c[5:7], 16)
    for y in range(8):
        for x in range(8):
            d = ((x - 3.5) ** 2 + (y - 3.5) ** 2) ** 0.5
            a, k = 0.0, 1.0
            if spec['shape'] == 'soft':
                a = max(0.0, 1 - d / 4.0) ** 1.3
            elif spec['shape'] == 'core':
                a = max(0.0, 1 - d / 4.0) ** 1.6
                k = 1 + 0.6 * max(0.0, 1 - d / 1.6)
            elif spec['shape'] == 'dot':
                a = 1.0 if d < 2.3 else (0.5 if d < 3.2 else 0.0)
            elif spec['shape'] == 'ring':
                a = 1.0 if 2.4 <= d < 3.6 else (0.25 if d < 2.4 else 0.0)
                k = 1.5 if (x, y) in ((2, 2), (2, 3), (3, 2)) else 1.0
            if a >= 0.1:
                img.set(x, y, (min(255, int(r * k)), min(255, int(g * k)), min(255, int(b * k)), int(round(255 * min(1.0, a)))))
    return img


def particle_json(pid):
    return {'textures': ['soulscraft:%s' % pid], 'horizon:behaviour': dict(PARTICLES[pid]['behaviour'], animation='first')}


# ---------------------------------------------------------------------------- textures

def foot_draw(f):
    """Plinth foot, rows 15 (1 px): a dark moulding."""
    f.rect(0, 15, 16, 1, ('wood', 0))


def panel_draw(f):
    """Plinth sides, rows 13.5..15: a light bead along the top, recessed panels."""
    f.rect(0, 13, 16, 1, ('wood', 4))
    for x in (2, 13):
        f.rect(x, 14, 1, 1, ('wood', 0))


def lip_draw(f):
    f.rect(0, 13, 16, 1, ('wood', 3))


def wood_draw(f):
    for y in (3, 7, 11, 15):
        f.rect(0, y, 16, 1, ('wood', 0))
    for y, x in ((0, 6), (4, 12), (8, 3), (12, 9)):
        f.rect(x, y, 1, 3, ('wood', 0))


def brass_corner_draw(f):
    """Brass bracket: bevel light top-left, a rivet where the face shows."""
    f.rect(0, 0, 16, 16, ('brass', 3))
    for i in range(16):
        f.px(i, 0, ('brass', 4))
        f.px(0, i, ('brass', 4))
    for x, y in ((0, 13), (1, 13), (14, 13), (15, 13), (0, 12), (15, 12)):
        f.px(x, y, ('brass', 4))
    for x, y in ((1, 14), (14, 14)):
        f.px(x, y, ('brass', 1))


def plaque_draw(f):
    """The engraved plaque (x 4..12, rows 13.75..15.25 of the north face): a cut border round a
    clean field, where the mod writes the mob's name and the kills (trophy::plaque_scale keeps
    the text inside the border; the two dark marks that stood in the field covered the letters)."""
    f.rect(0, 0, 16, 16, ('brass', 3))
    for x in range(4, 12):
        f.px(x, 13, ('brass', 4))
        f.px(x, 15, ('brass', 1))
    for x in (4, 11):
        f.px(x, 14, ('brass', 1))


def grave_draw(f):
    """Grave soil: dark earth, tufts of dead grass, a bone chip."""
    for x, y, w, h in ((0, 0, 5, 4), (10, 2, 6, 5), (3, 10, 5, 4), (12, 12, 4, 4)):
        f.patch(x, y, w, h, ('grass', 1), 0.55, x + y)
    for x, y in ((8, 6), (13, 9)):
        f.px(x, y, ('bone', 2))


def soil_side_draw(f):
    """The cut edge of the ground (rows 12..13 for a 1 px slab, the rest for taller pieces)."""
    for y in range(0, 16, 3):
        f.px((y * 5) % 16, y, ('soil', 3))
    f.rect(0, 12, 16, 1, ('grass', 2))


def headstone_draw(f):
    """Weathered headstone: a cross cut in, cracks, moss on the lower edge."""
    f.rect(7, 3, 2, 7, CREVICE)
    f.rect(5, 5, 6, 2, CREVICE)
    f.line(11, 1, 13, 6, ('stone', 1))
    f.patch(0, 12, 6, 4, ('grass', 2), 0.5, 3)
    f.patch(10, 13, 6, 3, ('grass', 1), 0.5, 7)


def blade_draw(f):
    for y in range(16):
        f.px(8, y, ('iron', 4))
    f.patch(4, 9, 3, 4, ('rust', 2), 0.5, 2)


def grip_draw(f):
    for y in range(0, 16, 2):
        f.rect(0, y, 16, 1, ('rust', 0))


def socket_draw(f):
    f.rect(0, 0, 16, 16, '#0b0908')


def forest_draw(f):
    """Forest floor: dark moss and fallen leaves in clumps, a twig."""
    for x, y, w, h in ((1, 1, 5, 4), (9, 8, 5, 5), (3, 11, 4, 3), (12, 1, 3, 3)):
        f.patch(x, y, w, h, ('leaf', 1 + (x + y) % 3), 0.5, x * 3 + y)
    f.line(5, 6, 10, 4, ('bark', 3))


def bark_draw(f):
    for x in (1, 5, 8, 12, 14):
        f.rect(x, 0, 1, 16, ('bark', 0))
    for x, y in ((3, 4), (10, 9), (6, 13)):
        f.rect(x, y, 2, 1, ('bark', 3))


def log_end_draw(f):
    f.outline(('bark', 1))
    for r in (2, 5):
        f.rect(8 - r, 8 - r, 2 * r, 1, ('log_end', 1))
        f.rect(8 - r, 8 + r - 1, 2 * r, 1, ('log_end', 1))


def fern_draw(f):
    for y in range(0, 16, 2):
        f.px(7, y, ('fern', 0))
        f.px(8, y + 1, ('fern', 3))


def cap_draw(f):
    for x, y in ((3, 2), (9, 5), (5, 10), (12, 12)):
        f.px(x, y, ('stem', 2))


def glowcap_draw(f):
    for x, y in ((3, 2), (9, 5), (5, 10), (12, 12)):
        f.px(x, y, ('glowcap', 3))


def sand_draw(f):
    for y in (2, 6, 10, 14):
        for x in range(16):
            if (x + y * 3) % 7 < 4:
                f.px(x, y, ('sand', 0))


def sand_side_draw(f):
    f.rect(0, 12, 16, 1, ('sand', 3))


def brick_draw(f):
    """Sandstone bricks, 8 x 4, offset rows, chipped corners."""
    for y in (3, 7, 11, 15):
        f.rect(0, y, 16, 1, ('sandstone', 0))
    for row, y in enumerate((0, 4, 8, 12)):
        for x in ((7, 15) if row % 2 == 0 else (3, 11)):
            f.rect(x, y, 1, 3, ('sandstone', 0))
    f.patch(9, 5, 3, 2, ('sandstone', 3), 0.5, 4)
    f.line(1, 9, 4, 10, CREVICE)


def ruin_top_draw(f):
    f.patch(2, 2, 6, 5, ('sand', 2), 0.5, 1)
    f.patch(9, 9, 5, 5, ('sand', 1), 0.5, 2)
    f.line(3, 12, 7, 9, CREVICE)


def slit_draw(f):
    f.rect(0, 0, 16, 16, '#0d0b09')


def fletch_draw(f):
    f.rect(0, 0, 16, 16, ('fletch', 1))


def rock_draw(f):
    """Cracked dark rock: crevices, a lighter worn patch."""
    f.line(1, 3, 6, 5, CREVICE)
    f.line(6, 5, 9, 4, CREVICE)
    f.line(10, 10, 14, 15, CREVICE)
    f.patch(9, 1, 5, 4, ('rock', 4), 0.45, 3)


def rock_side_draw(f):
    f.rect(0, 12, 16, 1, ('rock', 4))
    f.line(3, 13, 5, 15, CREVICE)


def scorch_draw(f):
    for x, y in ((3, 4), (10, 2), (6, 11), (13, 9)):
        f.px(x, y, ('magma', 0))


def magma_draw(f):
    for x, y in ((2, 2), (7, 9), (12, 4), (4, 13), (14, 12)):
        f.px(x, y, ('magma', 3))


def rubble_draw(f):
    """Broken dark stone bricks with soot."""
    for y in (3, 7, 11, 15):
        f.rect(0, y, 16, 1, CREVICE)
    for row, y in enumerate((0, 4, 8, 12)):
        for x in ((5, 13) if row % 2 == 0 else (1, 9)):
            f.rect(x, y, 1, 3, CREVICE)
    f.patch(2, 9, 5, 4, ('scorch', 2), 0.55, 1)
    f.line(10, 1, 13, 6, CREVICE)


def snow_draw(f):
    f.patch(3, 4, 4, 3, ('snow', 1), 0.5, 1)
    f.patch(10, 10, 4, 3, ('snow', 1), 0.5, 2)
    for x, y in ((12, 3), (5, 12)):
        f.px(x, y, ('snow', 4))


def cliff_draw(f):
    f.line(2, 0, 4, 7, CREVICE)
    f.line(11, 6, 13, 15, CREVICE)
    f.rect(0, 0, 16, 1, ('snow', 3))
    f.patch(5, 0, 6, 2, ('snow', 2), 0.6, 1)


def cliff_side_draw(f):
    f.rect(0, 12, 16, 1, ('snow', 3))


def deadwood_draw(f):
    for x in (2, 7, 12):
        f.rect(x, 0, 1, 16, ('deadwood', 0))
    f.px(9, 6, ('deadwood', 4))


def feather_draw(f):
    for y in range(16):
        f.px(7, y, ('feather', 2))


def ice_draw(f):
    f.rect(0, 0, 16, 16, ('ice', 2))
    for x in range(0, 16, 3):
        f.px(x, (x * 7) % 16, ('ice', 3))


def silt_draw(f):
    for x, y in ((3, 4), (11, 2), (7, 10), (13, 13), (1, 13)):
        f.px(x, y, ('shell', 0))
    f.patch(5, 5, 5, 3, ('silt', 0), 0.5, 2)


def silt_side_draw(f):
    f.rect(0, 12, 16, 1, ('silt', 3))


def kelp_draw(f):
    for y in range(0, 16, 3):
        f.rect(0, y, 16, 1, ('kelp', 3))


def shell_draw(f):
    for x in range(0, 16, 2):
        f.rect(x, 0, 1, 16, ('shell', 3))


def chest_draw(f):
    """Sunken chest planks with iron bands and green slime of the sea."""
    for y in (4, 9, 14):
        f.rect(0, y, 16, 1, ('chest', 0))
    for x in (1, 14):
        f.rect(x, 0, 1, 16, ('iron', 2))
    f.patch(3, 11, 6, 4, ('kelp', 1), 0.45, 2)


def chest_end_draw(f):
    """Top and bottom of the chest's planks and lid: grain along x, a dark seam every 4 rows."""
    for y in (3, 7, 11, 15):
        f.rect(0, y, 16, 1, ('chest', 0))


def gold_draw(f):
    for x, y in ((2, 3), (6, 1), (10, 5), (13, 2), (4, 9), (11, 12)):
        f.px(x, y, ('gold', 3))


def rust_draw(f):
    f.rect(0, 0, 16, 16, ('rust', 1))
    f.patch(3, 2, 5, 6, ('rust', 2), 0.5, 1)
    f.patch(9, 9, 5, 5, ('iron', 2), 0.5, 2)


def mud_draw(f):
    f.patch(2, 3, 5, 4, ('mud', 3), 0.5, 1)
    f.patch(10, 9, 4, 4, ('moss', 2), 0.55, 2)
    f.patch(4, 11, 3, 3, ('moss', 1), 0.55, 3)


def mud_side_draw(f):
    f.rect(0, 11, 16, 1, ('moss', 2))


def lily_draw(f):
    f.line(8, 8, 15, 4, ('pool', 1))
    for x, y in ((4, 4), (11, 11)):
        f.px(x, y, ('lily', 3))


def reed_draw(f):
    f.rect(7, 0, 1, 16, ('reed', 2))


TEXTURES = [
    Texture('dio_wood', 'wood', wood_draw),
    Texture('dio_foot', 'wood', foot_draw),
    Texture('dio_panel', 'wood', panel_draw),
    Texture('dio_lip', 'wood', lip_draw),
    Texture('dio_brass', 'brass', grain=0.5),
    Texture('dio_brass_corner', 'brass', brass_corner_draw, grain=0),
    Texture('dio_plaque', 'brass', plaque_draw, grain=0),
    Texture('dio_grave', 'soil', grave_draw),
    Texture('dio_soil_side', 'soil', soil_side_draw),
    Texture('dio_headstone', 'stone', headstone_draw),
    Texture('dio_wax', 'wax'),
    Texture('dio_soulfire', 'soulfire', grain=0.5),
    Texture('dio_blade', 'iron', blade_draw),
    Texture('dio_iron', 'iron'),
    Texture('dio_grip', 'rust', grip_draw),
    Texture('dio_bone', 'bone'),
    Texture('dio_socket', 'stone_dark', socket_draw),
    Texture('dio_tuft', 'grass'),
    Texture('dio_forest', 'moss', forest_draw),
    Texture('dio_bark', 'bark', bark_draw),
    Texture('dio_log_end', 'log_end', log_end_draw),
    Texture('dio_moss', 'moss'),
    Texture('dio_fern', 'fern', fern_draw),
    Texture('dio_stem', 'stem'),
    Texture('dio_cap', 'cap', cap_draw),
    Texture('dio_glowcap', 'glowcap', glowcap_draw, grain=0.5),
    Texture('dio_rope', 'rope'),
    Texture('dio_leaf', 'leaf'),
    Texture('dio_sand', 'sand', sand_draw),
    Texture('dio_sand_side', 'sand', sand_side_draw),
    Texture('dio_brick', 'sandstone', brick_draw),
    Texture('dio_ruin_top', 'sandstone', ruin_top_draw),
    Texture('dio_slit', 'stone_dark', slit_draw),
    Texture('dio_lamp', 'lamp', grain=0.5),
    Texture('dio_shaft', 'shaft'),
    Texture('dio_fletch', 'fletch', fletch_draw),
    Texture('dio_twig', 'deadwood'),
    Texture('dio_rock', 'rock', rock_draw),
    Texture('dio_rock_side', 'rock', rock_side_draw),
    Texture('dio_scorch', 'scorch', scorch_draw),
    Texture('dio_magma', 'magma', magma_draw, grain=0.6),
    Texture('dio_rubble_brick', 'stone', rubble_draw),
    Texture('dio_snow', 'snow', snow_draw),
    Texture('dio_cliff', 'cliff', cliff_draw),
    Texture('dio_cliff_side', 'cliff', cliff_side_draw),
    Texture('dio_deadwood', 'deadwood', deadwood_draw),
    Texture('dio_feather', 'feather', feather_draw),
    Texture('dio_ice', 'ice', ice_draw, grain=0.5),
    Texture('dio_silt', 'silt', silt_draw),
    Texture('dio_silt_side', 'silt', silt_side_draw),
    Texture('dio_seastone', 'seastone'),
    Texture('dio_kelp', 'kelp', kelp_draw),
    Texture('dio_shell', 'shell', shell_draw),
    Texture('dio_chest', 'chest', chest_draw),
    Texture('dio_chest_end', 'chest', chest_end_draw),
    Texture('dio_gold', 'gold', gold_draw, grain=0.6),
    Texture('dio_rust', 'rust', rust_draw),
    Texture('dio_anemone', 'anemone', grain=0.6),
    Texture('dio_mud', 'mud', mud_draw),
    Texture('dio_mud_side', 'mud', mud_side_draw),
    Texture('dio_pool', 'pool'),
    Texture('dio_lily', 'lily', lily_draw),
    Texture('dio_flower', 'flower'),
    Texture('dio_reed', 'reed', reed_draw),
    Texture('dio_cattail', 'cattail'),
    Texture('dio_marshglow', 'marshglow', grain=0.5),
]


# ---------------------------------------------------------------------------- blocks

def scene_builder(scene):
    def build(m):
        plinth(m)
        scene['build'](m)
    return build


BLOCKS = [Block(mob + '_trophy', 'Диорама: ' + MOBS[mob]['name'], scene_builder(s), facing=True,
                role='Трофей: %s. Фигурка моба в позе трофея рисуется отрисовкой block entity, свет %d' % (s['text'], s['light']))
          for mob, s in SCENES.items()]
