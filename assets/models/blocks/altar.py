"""The summoning altar (docs/design/06-altar.md): core, pedestal, slab, socket, pillar, brazier,
obelisk, the four assembled levels, the summon items (Rotting Horn and the horns of tiers 2-4),
the soul shard and the hunter's book.

Theme: ancient weathered stone, warm grey-brown so it stands apart from the cold grey of the
mobs; bone inlays and dark iron bands; dim soul fire in the cyan of the mobs' eyes. Each ring
adds a taller corner piece: none, pillars (2), soul braziers, obelisks (3) with glowing runes.

Block frame px: +x east, +y up, -z north, 0..16 per block (see blocks.py).
"""
import json
import os

from blocks import Block, Structure, Texture
from items import Sprite
from paint import Material

SET = 'altar'
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
with open(os.path.join(ROOT, 'data', 'altar.json'), encoding='utf-8') as f:
    DATA = json.load(f)

PALETTES = {
    'stone': Material(['#37332f', '#45403b', '#534d47', '#625b54', '#726a62', '#837a71'], noise=0.75, patch=4, fine=0.15),
    'stone_dark': ['#1e1c1b', '#272422', '#312d2a', '#3b3734'],
    'stone_light': ['#7a7269', '#898077', '#988e84'],
    'moss': ['#2c3522', '#37422a', '#435032'],
    'bone': Material(['#6f6650', '#8c826a', '#a99f84', '#c3b99c', '#d8cfb3'], noise=0.45, patch=3, fine=0.1),
    'iron': Material(['#1c1c20', '#26262b', '#313137', '#3d3d44', '#4b4b53'], noise=0.4, patch=3, fine=0.1),
    'rust': ['#3d2519', '#53301e', '#683d24'],
    'soul': Material(['#0e4148', '#16707a', '#25a7a8', '#5fdccf', '#bff8ee'], noise=0.7, patch=3, fine=0.25),
    'soul_tip': Material(['#25a7a8', '#45c6c0', '#5fdccf', '#8eeadf', '#bff8ee'], noise=0.6, patch=2, fine=0.2),
    'ash': Material(['#151413', '#1d1b1a', '#262422', '#302d2a', '#3b3734'], noise=0.7, patch=2),
    'leather': ['#2a1b12', '#3b281b', '#4e3524', '#62442e'],
    'paper': ['#8e8366', '#b0a585', '#cbc1a0'],
    # The survival slice's items (docs/design/10-survival.md): the soul shard and the horns 2-4.
    'shard': ['#2a6066', '#4a9396', '#7cc4bf', '#aee6dc', '#dcfff6'],
    'grave_bone': ['#4a4944', '#5f5e58', '#77756d', '#8f8d84', '#a9a69c'],
    'gold': ['#6b4a0c', '#a8780f', '#d9a920', '#f2d250', '#fff09a'],
    'horn_dark': ['#16110f', '#241c18', '#33281f', '#45372b', '#58483a'],
    'blaze': ['#7a2a05', '#b84a0a', '#e8780f', '#ffaa2a', '#ffe07a'],
    'abyss': ['#110a18', '#1d1028', '#2c1a3b', '#3f2757', '#563a76'],
    'netherite': ['#2b2629', '#3e3639', '#554b4e', '#6d6265', '#8a7f80'],
}
SOUL = ('soul', 3)
SOUL_CORE = ('soul', 4)
SOUL_DIM = ('soul', 1)
CREVICE = ('stone_dark', 0)
EDGE = ('stone_dark', 2)


# ---------------------------------------------------------------------------- textures

def cracks(f, lines, color=CREVICE):
    for x0, y0, x1, y1 in lines:
        f.line(x0, y0, x1, y1, color)


def moss(f, x, y, w, h, seed):
    f.patch(x, y, w, h, ('moss', 1), 0.5, seed)
    f.patch(x + 1, y, max(1, w - 2), max(1, h - 1), ('moss', 2), 0.45, seed + 3)


def stone_draw(f):
    """Plain weathered stone: a few hairline cracks, one moss patch, worn lighter chips."""
    cracks(f, [(2, 3, 5, 6), (5, 6, 5, 8), (11, 10, 14, 12), (9, 1, 10, 2)])
    f.px(6, 8, EDGE)
    moss(f, 10, 13, 5, 3, 1)
    for x, y in ((3, 11), (12, 4), (7, 14)):
        f.px(x, y, ('stone_light', 0))


def slab_top_draw(f):
    """Dressed flagstones laid like vanilla stone bricks: two rows, the joints of the lower row
    offset by half a stone, so neighbouring slabs run on into one pavement. Each stone a little
    lighter or darker, a worn lit corner, one crack, moss in one joint."""
    stones = ((0, 0, 8, 1), (8, 0, 8, -1), (4, 8, 8, 0), (12, 8, 8, 1))  # x, y, width (wraps), tone
    for i, (sx, sy, w, tone) in enumerate(stones):
        if tone:
            f.patch(sx + 2, sy + 2, w - 3, 5, ('stone', 3 + tone), 0.4, 20 + i)
        f.rect(sx, sy, 1, 8, CREVICE)
        f.px((sx + 1) % 16, sy + 1, ('stone', 5))
    f.rect(0, 0, 16, 1, CREVICE)
    f.rect(0, 8, 16, 1, CREVICE)
    for x in (12, 13, 14, 15, 0, 1, 2, 3):
        f.px(x, 15, ('stone', 1))
    cracks(f, [(10, 3, 13, 6), (6, 11, 8, 13)])
    moss(f, 0, 9, 3, 2, 4)


def slab_side_draw(f):
    """Side of a slab or a plinth (it uses the lower half): a carved groove under the lip,
    darker foot, worn chips. The top rows serve the cap of the core and capitals."""
    f.rect(0, 0, 16, 1, ('stone', 4))
    f.rect(0, 2, 16, 1, CREVICE)
    f.rect(0, 8, 16, 1, ('stone', 4))
    f.rect(0, 10, 16, 1, CREVICE)
    f.rect(0, 15, 16, 1, ('stone_dark', 2))
    for x, y in ((4, 13), (12, 12), (8, 5)):
        f.px(x, y, ('stone_light', 0))
    cracks(f, [(6, 12, 7, 14), (13, 3, 14, 5)])
    moss(f, 1, 14, 4, 2, 7)


def socket_top_draw(f):
    """The socket: a carved ring with a bone inlay and a dark iron plate in the middle, so the
    cell reads as 'something goes here' from a distance."""
    slab_top_draw(f)
    ring = ["....oooooo....",
            "...o.bbbb.o...",
            "..o.b....b.o..",
            ".o.b......b.o.",
            "o.b........b.o",
            "o.b........b.o",
            "o.b........b.o",
            "o.b........b.o",
            "o.b........b.o",
            "o.b........b.o",
            ".o.b......b.o.",
            "..o.b....b.o..",
            "...o.bbbb.o...",
            "....oooooo...."]
    f.rows(ring, {'o': CREVICE, 'b': ('bone', 3)}, 1, 1)
    f.rect(5, 5, 6, 6, ('iron', 2))
    for x, y in ((5, 5), (10, 5), (5, 10), (10, 10)):
        f.px(x, y, ('iron', 4))
    f.rect(7, 7, 2, 2, ('iron', 0))


def core_side_draw(f):
    """The core's side: a bone frame around the soul gem (the gem is its own element at
    x 6..10, y 6..10, rows and columns 6..9 here), runes cut into the stone around it."""
    f.rect(4, 4, 8, 8, ('stone', 1))
    frame = ["..bbbb..",
             ".b....b.",
             "b......b",
             "b......b",
             "b......b",
             "b......b",
             ".b....b.",
             "..bbbb.."]
    f.rows(frame, {'b': ('bone', 3)}, 4, 4)
    for x, y in ((5, 4), (4, 5), (10, 11), (11, 10)):
        f.px(x, y, ('bone', 1))
    # Runes cut above and below: dark grooves with a dim soul glow deep inside.
    for x in (3, 7, 11):
        f.rect(x, 1, 2, 1, CREVICE)
        f.px(x, 2, CREVICE)
        f.px(x + 1, 2, SOUL_DIM)
    for x in (2, 6, 10):
        f.px(x, 13, CREVICE)
        f.rect(x, 14, 3, 1, CREVICE)
        f.px(x + 1, 13, SOUL_DIM)
    f.streak(9, 12, 2, ('stone_dark', 1))


def core_top_draw(f):
    """Top of the core: a round channel where the pedestal stands, bone spokes to the corners."""
    f.rect(0, 0, 16, 1, ('stone', 4))
    f.rect(0, 0, 1, 16, ('stone', 4))
    f.rect(0, 15, 16, 1, ('stone', 1))
    f.rect(15, 0, 1, 16, ('stone', 1))
    ring = ["..oooooo..",
            ".o......o.",
            "o........o",
            "o........o",
            "o........o",
            "o........o",
            "o........o",
            "o........o",
            ".o......o.",
            "..oooooo.."]
    f.rows(ring, {'o': CREVICE}, 3, 3)
    for i in range(1, 3):
        for x, y in ((i, i), (15 - i, i), (i, 15 - i), (15 - i, 15 - i)):
            f.px(x, y, ('bone', 3))
    cracks(f, [(13, 5, 14, 8)])


def bone_draw(f):
    """Bone: faint growth lines, a few darker pits."""
    for y in (3, 9, 13):
        f.rect(0, y, 16, 1, ('bone', 2))
    for x, y in ((4, 6), (11, 2), (8, 11), (2, 14)):
        f.px(x, y, ('bone', 0))


def iron_draw(f):
    """Dark iron: rivets every 8 texels, a little rust running from them."""
    for x, y in ((3, 3), (11, 3), (3, 11), (11, 11)):
        f.px(x, y, ('iron', 4))
        f.streak(x, y + 1, 2, ('rust', 1), tail=('rust', 0))
    f.rect(0, 7, 16, 1, ('iron', 0))


def pillar_draw(f):
    """Fluted shaft: vertical grooves every 3 texels with a lit edge, chipped here and there."""
    for x in range(1, 16, 3):
        f.rect(x, 0, 1, 16, CREVICE)
        f.rect(x + 1, 0, 1, 16, ('stone', 4))
    for x, y in ((5, 6), (11, 12), (2, 2)):
        f.px(x, y, ('stone', 1))
    moss(f, 6, 13, 3, 3, 9)


def pedestal_draw(f):
    """Pedestal shaft: bone ribs running up the stone, iron band above (rows 7..8)."""
    for x in (2, 6, 10, 13):
        f.rect(x, 7, 1, 7, ('bone', 3))
        f.px(x, 13, ('bone', 1))
    f.rect(0, 6, 16, 1, CREVICE)


def pedestal_top_draw(f):
    """The dish the summon item rests over: a dark iron plate with a soul-blue inlay ring."""
    f.rect(3, 3, 10, 10, ('iron', 1))
    for x in range(5, 11):
        f.px(x, 4, SOUL_DIM)
        f.px(x, 11, SOUL_DIM)
        f.px(4, x, SOUL_DIM)
        f.px(11, x, SOUL_DIM)
    f.rect(7, 7, 2, 2, ('iron', 4))


def obelisk_draw(f):
    """Obelisk shaft: large dressed stones, joints every 8 rows, one vertical joint offset."""
    for y in (0, 8):
        f.rect(0, y, 16, 1, CREVICE)
    f.rect(5, 1, 1, 7, CREVICE)
    f.rect(11, 9, 1, 7, CREVICE)
    f.rect(0, 1, 16, 1, ('stone', 4))
    f.rect(0, 9, 16, 1, ('stone', 4))
    moss(f, 1, 13, 3, 3, 11)


def obelisk_base_draw(f):
    """The obelisk's foot: a bone band with skull studs (rows 10..13, the step uses 10..13)."""
    f.rect(0, 10, 16, 1, CREVICE)
    f.rect(0, 11, 16, 2, ('bone', 2))
    for x in (2, 7, 12):
        f.rect(x, 11, 2, 2, ('bone', 4))
        f.px(x, 12, ('bone', 0))
        f.px(x + 1, 12, ('bone', 0))
    f.rect(0, 13, 16, 1, CREVICE)


def rune_draw(f):
    """Rune strip of the obelisk: soul-blue glyphs cut into dark stone; drawn full bright."""
    glyphs = ["....", ".ss.", ".s..", ".ss.", "..s.", "....",
              "s..s", ".ss.", ".s..", "....", ".s.s", ".ss.",
              "..s.", "....", "s.s.", ".s.."]
    f.rect(0, 0, 16, 16, ('stone_dark', 0))
    for y, row in enumerate(glyphs):
        for i, ch in enumerate(row):
            if ch == 's':
                f.px(6 + i, y, SOUL)
    f.px(7, 1, SOUL_CORE)
    f.px(8, 7, SOUL_CORE)
    f.px(6, 14, SOUL_CORE)


def gem_draw(f):
    """Soul gem: a light core, darker rim, one glint."""
    f.rect(0, 0, 16, 16, ('soul', 2))
    for y in range(16):
        for x in range(16):
            if (x % 4 in (0, 3)) or (y % 4 in (0, 3)):
                f.px(x, y, ('soul', 1))
            if x % 4 in (1, 2) and y % 4 in (1, 2):
                f.px(x, y, ('soul', 3))
    for x in range(1, 16, 4):
        for y in range(1, 16, 4):
            f.px(x, y, SOUL_CORE)


def fire_draw(f):
    """Soul fire: deep teal in the bottom rows, brighter up, dark flickers and a few hot texels.
    Element faces take the rows of their height, so the stacked flames darken towards the bowl."""
    for y in range(11, 16):
        for x in range(16):
            if y >= 13 or (x * 5 + y * 3) % 7 < 4:
                f.px(x, y, ('soul', 1) if y < 14 else ('soul', 0))
    for x, y in ((2, 3), (9, 1), (13, 8), (5, 7)):
        f.px(x, y, ('soul', 1))
        f.px(x, y + 1, ('soul', 1))
    for x, y in ((6, 5), (12, 4), (3, 9)):
        f.px(x, y, SOUL_CORE)


def fire_tip_draw(f):
    """Tongue tips: pale cyan, a darker flicker, a white-hot texel or two."""
    for x, y in ((3, 2), (10, 6), (6, 12), (13, 13)):
        f.px(x, y, ('soul_tip', 0))
    for x, y in ((8, 9), (1, 4)):
        f.px(x, y, ('soul_tip', 4))


def ash_draw(f):
    """Cold ash and charred bone in an unlit brazier."""
    for x, y in ((3, 4), (9, 9), (12, 3)):
        f.rect(x, y, 2, 1, ('bone', 0))
    for x, y in ((6, 12), (13, 11)):
        f.px(x, y, ('ash', 4))


TEXTURES = [
    Texture('altar_stone', 'stone', stone_draw),
    Texture('altar_slab_top', 'stone', slab_top_draw),
    Texture('altar_slab_side', 'stone', slab_side_draw),
    Texture('altar_socket_top', 'stone', socket_top_draw),
    Texture('altar_core_side', 'stone', core_side_draw),
    Texture('altar_core_top', 'stone', core_top_draw),
    Texture('altar_bone', 'bone', bone_draw),
    Texture('altar_iron', 'iron', iron_draw),
    Texture('altar_pillar_side', 'stone', pillar_draw),
    Texture('altar_pedestal_side', 'stone', pedestal_draw),
    Texture('altar_pedestal_top', 'iron', pedestal_top_draw),
    Texture('altar_obelisk_side', 'stone', obelisk_draw),
    Texture('altar_obelisk_base', 'stone', obelisk_base_draw),
    Texture('altar_rune', 'stone', rune_draw),
    Texture('altar_gem', 'soul', gem_draw, grain=0),
    Texture('altar_soul_fire', 'soul', fire_draw),
    Texture('altar_soul_fire_tip', 'soul_tip', fire_tip_draw),
    Texture('altar_ash', 'ash', ash_draw),
]

SIDES = ('north', 'south', 'west', 'east')
SLAB = {'up': 'altar_slab_top', 'down': 'altar_stone', 'side': 'altar_slab_side'}
STONE = 'altar_stone'


def ring_box(m, lo, hi, y0, y1, tex, faces=None, **kw):
    """A square box lo..hi on x and z."""
    return m.box((lo, y0, lo), (hi, y1, hi), tex, faces=faces, **kw)


# ---------------------------------------------------------------------------- blocks

def slab(m):
    m.box((0, 0, 0), (16, 8, 16), SLAB)


def socket(m):
    m.box((0, 0, 0), (16, 8, 16), dict(SLAB, up='altar_socket_top'))
    # Four bone studs at the corners: the socket reads from a distance, even in the dark.
    for x, z in ((1, 1), (13, 1), (1, 13), (13, 13)):
        m.box((x, 8, z), (x + 2, 9, z + 2), 'altar_bone', faces=('up',) + SIDES)


def core(m):
    m.box((0, 0, 0), (16, 3, 16), {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
    m.box((1, 3, 1), (15, 13, 15), 'altar_core_side', faces=SIDES)
    m.box((0, 13, 0), (16, 16, 16), {'up': 'altar_core_top', 'down': STONE, 'side': 'altar_slab_side'})
    for x, z in ((0, 0), (14, 0), (0, 14), (14, 14)):
        m.box((x, 3, z), (x + 2, 13, z + 2), 'altar_bone', faces=SIDES)
    # Soul gems, proud of each side by 0.75 px; they glow (light_emission on the element).
    m.box((6, 6, 0.25), (10, 10, 1), 'altar_gem', faces=('north', 'up', 'down', 'west', 'east'), glow=12)
    m.box((6, 6, 15), (10, 10, 15.75), 'altar_gem', faces=('south', 'up', 'down', 'west', 'east'), glow=12)
    m.box((0.25, 6, 6), (1, 10, 10), 'altar_gem', faces=('west', 'up', 'down', 'north', 'south'), glow=12)
    m.box((15, 6, 6), (15.75, 10, 10), 'altar_gem', faces=('east', 'up', 'down', 'north', 'south'), glow=12)


def pedestal(m):
    ring_box(m, 3, 13, 0, 2, {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
    ring_box(m, 5, 11, 2, 9, 'altar_pedestal_side', faces=SIDES)
    ring_box(m, 3, 13, 9, 11, {'up': 'altar_pedestal_top', 'down': 'altar_iron', 'side': 'altar_iron'})
    # Bone claws on the four edges lean out and cradle the floating item.
    m.box((7, 11, 3.5), (9, 16, 5.5), 'altar_bone', faces=('up',) + SIDES, rotation=('x', -22.5, (8, 11, 4.5)))
    m.box((7, 11, 10.5), (9, 16, 12.5), 'altar_bone', faces=('up',) + SIDES, rotation=('x', 22.5, (8, 11, 11.5)))
    m.box((3.5, 11, 7), (5.5, 16, 9), 'altar_bone', faces=('up',) + SIDES, rotation=('z', 22.5, (4.5, 11, 8)))
    m.box((10.5, 11, 7), (12.5, 16, 9), 'altar_bone', faces=('up',) + SIDES, rotation=('z', -22.5, (11.5, 11, 8)))


def pillar(m, part):
    if part == 'bottom':
        ring_box(m, 1, 15, 0, 3, {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
        ring_box(m, 2, 14, 3, 5, STONE, faces=('up',) + SIDES)
        ring_box(m, 3, 13, 5, 16, 'altar_pillar_side', faces=SIDES)
        ring_box(m, 2.5, 13.5, 11, 13, 'altar_iron')
    else:
        ring_box(m, 3, 13, 0, 9, 'altar_pillar_side', faces=SIDES)
        ring_box(m, 2.5, 13.5, 3, 4.5, 'altar_bone')
        ring_box(m, 2, 14, 9, 11, STONE, faces=('down',) + SIDES)
        ring_box(m, 1, 15, 11, 14, {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
        for x, z in ((1.5, 1.5), (12.5, 1.5), (1.5, 12.5), (12.5, 12.5)):
            m.box((x, 14, z), (x + 2, 16, z + 2), 'altar_bone', faces=('up',) + SIDES)
        ring_box(m, 6, 10, 14, 17, 'altar_bone', faces=('up',) + SIDES)


def brazier(m, lit):
    """A standing soul brazier about a block tall: stone foot, four iron legs round a bone
    stem, a wide iron bowl with a bone rim. Lit: soul fire in uneven tongues (full bright);
    unlit: cold ash."""
    ring_box(m, 2, 14, 0, 2, {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
    for x, z in ((3, 3), (11, 3), (3, 11), (11, 11)):
        m.box((x, 2, z), (x + 2, 9, z + 2), 'altar_iron', faces=SIDES)
    ring_box(m, 6, 10, 2, 9, 'altar_bone', faces=SIDES)
    ring_box(m, 3, 13, 9, 11, 'altar_iron', faces=('down',) + SIDES)
    ring_box(m, 1, 15, 11, 14, {'up': 'altar_ash', 'down': 'altar_iron', 'side': 'altar_iron'})
    m.box((1, 14, 1), (15, 15, 2), 'altar_bone', faces=('up', 'north', 'south', 'west', 'east'))
    m.box((1, 14, 14), (15, 15, 15), 'altar_bone', faces=('up', 'north', 'south', 'west', 'east'))
    m.box((1, 14, 2), (2, 15, 14), 'altar_bone', faces=('up', 'west', 'east'))
    m.box((14, 14, 2), (15, 15, 14), 'altar_bone', faces=('up', 'west', 'east'))
    up = ('up',) + SIDES
    if lit:
        m.box((3, 14, 3), (13, 16, 13), 'altar_soul_fire', faces=up, glow=15)
        m.box((4, 16, 4), (11, 19, 12), 'altar_soul_fire', faces=up, glow=15)
        m.box((5, 19, 6), (9, 23, 10), 'altar_soul_fire_tip', faces=up, glow=15)
        m.box((6, 23, 7), (8, 26, 9), 'altar_soul_fire_tip', faces=up, glow=15)
        m.box((11, 16, 7), (12.5, 20, 9), 'altar_soul_fire_tip', faces=up, glow=15)
        m.box((9.5, 19, 4.5), (11, 21, 6), 'altar_soul_fire_tip', faces=up, glow=15)
        m.box((3.5, 16, 10), (5, 18, 12.5), 'altar_soul_fire_tip', faces=up, glow=15)
    else:
        m.box((4, 14, 5), (8, 15, 9), 'altar_ash', faces=up)
        m.box((8, 14, 7), (12, 15, 11), 'altar_ash', faces=up)
        m.box((6, 14, 10), (8, 15, 12), 'altar_ash', faces=up)


def obelisk(m, part):
    if part == 'bottom':
        ring_box(m, 1, 15, 0, 3, {'up': STONE, 'down': STONE, 'side': 'altar_slab_side'})
        ring_box(m, 2, 14, 3, 6, {'up': STONE, 'side': 'altar_obelisk_base'}, faces=('up',) + SIDES)
        ring_box(m, 3, 13, 6, 16, 'altar_obelisk_side', faces=SIDES)
    elif part == 'middle':
        ring_box(m, 3, 13, 0, 14, 'altar_obelisk_side', faces=SIDES)
        ring_box(m, 2.5, 13.5, 14, 16, 'altar_iron', faces=('up', 'down') + SIDES)
        # Rune strips, 0.25 px proud of each face, full bright.
        m.box((6, 2, 2.75), (10, 13, 3), 'altar_rune', faces=('north', 'up', 'down', 'west', 'east'), glow=15)
        m.box((6, 2, 13), (10, 13, 13.25), 'altar_rune', faces=('south', 'up', 'down', 'west', 'east'), glow=15)
        m.box((2.75, 2, 6), (3, 13, 10), 'altar_rune', faces=('west', 'up', 'down', 'north', 'south'), glow=15)
        m.box((13, 2, 6), (13.25, 13, 10), 'altar_rune', faces=('east', 'up', 'down', 'north', 'south'), glow=15)
    else:
        ring_box(m, 4, 12, 0, 8, 'altar_obelisk_side', faces=SIDES)
        ring_box(m, 5, 11, 8, 10, STONE, faces=('up',) + SIDES)
        ring_box(m, 6, 10, 10, 12, STONE, faces=('up',) + SIDES)
        ring_box(m, 7, 9, 12, 14, 'altar_gem', faces=('up',) + SIDES, glow=15)


# ---------------------------------------------------------------------------- item sprites

def pillar_icon(c):
    c.rows(["................",
            "...bbb....bbb...",
            "..ssssssssssss..",
            "..lsssssssssdd..",
            "....ssssssss....",
            ".....lsssssd....",
            ".....lsssssd....",
            ".....lsssssd....",
            "....ibbbbbbbi...",
            ".....lsssssd....",
            ".....lsssssd....",
            ".....lsssssd....",
            "....iiiiiiiii...",
            ".....lsssssd....",
            "...ssssssssss...",
            "..dddddddddddd.."],
           {'s': ('stone', 3), 'l': ('stone', 5), 'd': ('stone', 1), 'b': ('bone', 3), 'i': ('iron', 2)})
    c.edge('#141211')


def obelisk_icon(c):
    c.rows([".......g........",
            "......sgs.......",
            ".....sssss......",
            ".....lsssd......",
            ".....lsrsd......",
            ".....lsrsd......",
            ".....lsssd......",
            "....iiiiiii.....",
            ".....lsrsd......",
            ".....lsrsd......",
            ".....lsssd......",
            "....lssssd......",
            "....bbbbbbb.....",
            "...sssssssss....",
            "..ddddddddddd...",
            "................"],
           {'s': ('stone', 3), 'l': ('stone', 5), 'd': ('stone', 1), 'r': SOUL, 'g': SOUL_CORE,
            'b': ('bone', 3), 'i': ('iron', 2)})
    c.edge('#141211')


# The horn's outline, shared by the four horns (the tiers differ by material, not by shape):
# o outline, h highlight, b body, d shadow, r stain at the bell, m the mouth, i the two bands.
HORN_ROWS = ["..........ooooo.",
             ".........ohhhbbo",
             "........ohbmmmbo",
             "........ohmmmmdo",
             "........obmmmrdo",
             ".......oobbrrdo.",
             "......ohbbbrdo..",
             ".....oiiiibdo...",
             "....ohbbbiio....",
             "...ohbbbdo......",
             "..ohbbbdo.......",
             ".oiiiido........",
             ".ohbbdo.........",
             ".obbdo..........",
             ".oddo...........",
             "..oo............"]
HORN_BAND_GLINTS = ((8, 7), (3, 11))     # the lit texel of each band
HORN_BAND_SHADES = ((9, 8), (5, 11))     # the shaded texel of each band
HORN_MOUTH_GLOW = ((11, 2), (12, 3), (10, 3), (11, 4))


def horn_sprite(c):
    """Гнилой горн: a bone war horn curling up from the mouthpiece (bottom left) to a wide
    flared bell (top right) whose dark mouth faces the viewer; rot creeps from the bell as a
    green-brown stain; two iron bands and a leather strap between them."""
    c.rows(HORN_ROWS,
           {'o': '#1a1411', 'h': ('bone', 4), 'b': ('bone', 2), 'd': ('bone', 0),
            'r': '#55603a', 'm': '#20170f', 'i': ('iron', 2)})
    c.px(12, 3, '#2e2316')
    c.px(13, 4, '#4a4e2c')
    for x, y in HORN_BAND_GLINTS:
        c.px(x, y, ('iron', 4))
    for x, y in ((6, 9), (7, 10), (8, 10), (9, 9)):
        if not c.solid(x, y):
            c.px(x, y, ('leather', 2))


def grave_horn_sprite(c):
    """Горн мертвецов (tier 2): the same horn in old grey bone, cracked by weather, grave dirt
    in the bell, bound with two gold bands."""
    c.rows(HORN_ROWS,
           {'o': '#161514', 'h': ('grave_bone', 4), 'b': ('grave_bone', 2), 'd': ('grave_bone', 0),
            'r': '#3e3a30', 'm': '#121110', 'i': ('gold', 2)})
    for x, y in ((5, 10), (6, 9), (4, 12)):              # hairline cracks in the bone
        c.px(x, y, ('grave_bone', 0))
    c.px(10, 1, ('grave_bone', 3))
    for x, y in HORN_BAND_GLINTS:
        c.px(x, y, ('gold', 4))
    for x, y in HORN_BAND_SHADES:
        c.px(x, y, ('gold', 0))
    c.px(6, 7, ('gold', 3))
    c.px(2, 11, ('gold', 3))


def storm_horn_sprite(c):
    """Горн бури (tier 3): a dark horn with blaze-orange bands; its mouth crackles with embers,
    fire cracks run down the bell and two sparks fly from it."""
    c.rows(HORN_ROWS,
           {'o': '#0d0907', 'h': ('horn_dark', 4), 'b': ('horn_dark', 2), 'd': ('horn_dark', 0),
            'r': ('blaze', 1), 'm': ('blaze', 0), 'i': ('blaze', 2)})
    for x, y in HORN_MOUTH_GLOW:
        c.px(x, y, ('blaze', 2))
    c.px(12, 2, ('blaze', 4))
    c.px(11, 3, ('blaze', 3))
    for x, y in HORN_BAND_GLINTS:
        c.px(x, y, ('blaze', 4))
    for x, y in HORN_BAND_SHADES:
        c.px(x, y, ('blaze', 1))
    c.px(15, 6, ('blaze', 4))                            # sparks
    c.px(14, 8, ('blaze', 3))


def abyss_horn_sprite(c):
    """Горн бездны (tier 4): a black-purple horn with netherite bands; its mouth holds the teal
    glow of soul fire, and soul veins creep down the bell."""
    c.rows(HORN_ROWS,
           {'o': '#07040a', 'h': ('abyss', 4), 'b': ('abyss', 2), 'd': ('abyss', 0),
            'r': SOUL_DIM, 'm': SOUL_DIM, 'i': ('netherite', 2)})
    for x, y in HORN_MOUTH_GLOW:
        c.px(x, y, SOUL)
    c.px(12, 2, SOUL_CORE)
    c.px(11, 3, SOUL_CORE)
    for x, y in HORN_BAND_GLINTS:
        c.px(x, y, ('netherite', 4))
    for x, y in HORN_BAND_SHADES:
        c.px(x, y, ('netherite', 0))


def soul_shard_sprite(c):
    """Осколок души: a jagged pale-teal crystal shard pointing to the top right, a smaller chip
    beside its base, and a faint soul glow in its heart."""
    c.rows(["................",
            "..........oo....",
            ".........ohlo...",
            "........ohllbo..",
            "........ohlcbo..",
            ".......ohlccbdo.",
            ".......ohlccbo..",
            "......ohlccbdo..",
            ".....ohlccbdo...",
            ".....ohcbbdo....",
            "....ohcbbdo..oo.",
            "....ohbbddo.ohlo",
            "...ohbbddo..obdo",
            "...obddoo....oo.",
            "....ooo.........",
            "................"],
           {'o': '#0f2f33', 'h': ('shard', 4), 'l': ('shard', 3), 'c': SOUL, 'b': ('shard', 2),
            'd': ('shard', 1)})
    for x, y in ((10, 6), (9, 7)):                       # the brightest point of the glow
        c.px(x, y, SOUL_CORE)


def book_sprite(c):
    """Книга охотника: a worn leather book, a bone skull clasp with soul-blue eyes, page edges."""
    c.rows(["................",
            "..oooooooooooo..",
            "..olllllllllpo..",
            "..ollhhhhlllpo..",
            "..olhllllhllpo..",
            "..ollbbbbllllpo.",
            "..olbsbbsblllpo.",
            "..olbbbbbbllpo..",
            "..ollbbbblllpo..",
            "..olllbblllllpo.",
            "..ollllllllllpo.",
            "..oillllllllipo.",
            "..olllllllllpo..",
            "..oddddddddddpo.",
            "...oooooooooooo.",
            "................"],
           {'o': '#150e0a', 'l': ('leather', 2), 'h': ('leather', 3), 'd': ('leather', 0),
            'p': ('paper', 1), 'b': ('bone', 3), 's': SOUL, 'i': ('iron', 3)})


SPRITES = [
    Sprite('altar_pillar', pillar_icon, palettes=PALETTES, name='Колонна алтаря'),
    Sprite('altar_obelisk', obelisk_icon, palettes=PALETTES, name='Обелиск'),
    Sprite('rotten_horn', horn_sprite, palettes=PALETTES, name='Гнилой горн'),
    Sprite('grave_horn', grave_horn_sprite, palettes=PALETTES, name='Горн мертвецов'),
    Sprite('storm_horn', storm_horn_sprite, palettes=PALETTES, name='Горн бури'),
    Sprite('abyss_horn', abyss_horn_sprite, palettes=PALETTES, name='Горн бездны'),
    Sprite('soul_shard', soul_shard_sprite, palettes=PALETTES, name='Осколок души'),
    Sprite('hunter_book', book_sprite, palettes=PALETTES, name='Книга охотника'),
]
ICONS = {s.id: s for s in SPRITES}

BLOCKS = [
    Block('altar_core', 'Ядро алтаря', core,
          role='Центр структуры. Считает уровень, показывает призрак, ведёт ивент и учащённый спавн'),
    Block('altar_pedestal', 'Подставка', pedestal,
          role='Стоит на ядре. Держит предмет призыва; правый клик кладёт или забирает предмет'),
    Block('altar_slab', 'Плита алтаря', slab, role='Пол колец'),
    Block('altar_socket', 'Гнездо модификатора', socket,
          role='Отмеченная плита кольца: на неё ставится знамя-модификатор'),
    Block('altar_pillar', 'Колонна алтаря', pillar, part=('bottom', 'top'), on_slab=True,
          item=ICONS['altar_pillar'], role='Углы второго кольца, высота 2'),
    Block('altar_brazier', 'Жаровня', brazier, states={'lit': (False, True)}, on_slab=True,
          role='Углы третьего кольца, горит душевным огнём, пока алтарь активен'),
    Block('altar_obelisk', 'Обелиск', obelisk, part=('bottom', 'middle', 'top'), on_slab=True,
          item=ICONS['altar_obelisk'], role='Углы четвёртого кольца, высота 3'),
]


# ---------------------------------------------------------------------------- levels

def ring(d):
    """Cells at Chebyshev distance d from the core, in a fixed order."""
    if d == 0:
        return [(0, 0)]
    out = []
    for z in range(-d, d + 1):
        for x in range(-d, d + 1):
            if max(abs(x), abs(z)) == d:
                out.append((x, z))
    return out


def outward(x, z):
    if abs(x) >= abs(z):
        return 'east' if x > 0 else 'west'
    return 'south' if z > 0 else 'north'


def level_rings(blocks, level, banners=True):
    """[(ring, kind, block id, (x, y, z), state, facing, on_slab, group)] for rings 1..level."""
    mods = {m['id']: m for m in DATA['modifiers']}
    sample = list(DATA['sample_banners'])
    out = []
    for lv in DATA['levels'][:level]:
        n = lv['level']
        d = n  # level n is a (2n + 1)^2 square: its ring sits at distance n
        sockets = {tuple(s) for s in lv['sockets']}
        if n == 1:
            out.append((n, 'core', 'altar_core', (0, 0, 0), {}, 'north', False, 'core'))
            out.append((n, 'pedestal', 'altar_pedestal', (0, 1, 0), {}, 'north', False, 'pedestal'))
        for x, z in ring(d):
            if (x, z) == (0, 0):
                continue
            kind = 'socket' if (x, z) in sockets else 'slab'
            out.append((n, kind, 'altar_' + kind, (x, 0, z), {}, 'north', False, '%d,%d' % (x, z)))
            if lv['corner'] and abs(x) == d and abs(z) == d:
                out.append((n, 'corner', lv['corner'], (x, 1, z), {}, 'north', True, 'corner %d,%d' % (x, z)))
            if kind == 'socket' and banners:
                kid = sample.pop(0)
                out.append((n, 'banner', mods[kid]['banner'], (x, 1, z), {}, outward(x, z), True, 'banner %d,%d' % (x, z)))
    return out


def structures(blocks):
    out = []
    for lv in DATA['levels']:
        s = Structure('altar_level_%d' % lv['level'], 'Алтарь, уровень %d' % lv['level'])
        for n, kind, bid, at, st, facing, on_slab, group in level_rings(blocks, lv['level']):
            s.piece(blocks[bid], at, st, facing, on_slab, ring=n, kind=kind, group=group)
        out.append(s)
    return out
