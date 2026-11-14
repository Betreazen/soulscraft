"""Banners of the altar modifiers (06-altar.md, «Модификаторы»): one block per banner, standing
on a socket. Two segments like a door (part bottom, top), facing (the cloth faces the way the
banner looks; models face north), on_slab (it stands on the socket's top).

The cloth is a 1 px thick element in front of the pole, 12 px wide, from 8 px up the bottom
segment to 13 px up the top one, with a stepped point below. Its picture is painted on one
16 x 32 canvas (row 0 is the top of the top segment) and cut into banner_<kind>_top and
banner_<kind>_bottom; the north face shows it as painted, the south face mirrored, as a vanilla
banner's back does. Emblems are our own pixel art, outlined by code.

Block frame px: +x east, +y up, -z north.
"""
import json
import os

import paint
from blocks import Block, Texture
from items import Sprite

SET = 'altar'
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
with open(os.path.join(ROOT, 'data', 'altar.json'), encoding='utf-8') as f:
    DATA = json.load(f)

PALETTES = {
    'pole': paint.Material(['#22170f', '#2f2015', '#3d2a1c', '#4c3523'], noise=0.4, patch=4, fine=0.1),
    'stone': paint.Material(['#2e2b29', '#3b3734', '#494440', '#57514c', '#665f59', '#766e67'], noise=0.75, patch=4, fine=0.15),
    'iron': paint.Material(['#1c1c20', '#26262b', '#313137', '#3d3d44', '#4b4b53'], noise=0.4, patch=3, fine=0.1),
    'bone': paint.Material(['#6f6650', '#8c826a', '#a99f84', '#c3b99c', '#d8cfb3'], noise=0.45, patch=3, fine=0.1),
}
OUTLINE = '#120e0c'
STITCH = '#c3b99c'

# kind -> cloth palette (dark to light), emblem colours e (main), h (light), s (shade or second),
# and the emblem map: rows of e/h/s/d (d = dark detail), '.' empty. The outline is added by code;
# capital letters are colours whose texels get no outline (thin lines side by side).
KINDS = {
    'weight': (['#33100f', '#481818', '#5e2120', '#722b29'], {'e': '#8a8a92', 'h': '#bcbcc4', 's': '#55555d', 'd': '#2b2b30'}, [
        "...hhh...",
        "..h...e..",
        "..h...e..",
        "...eee...",
        ".heeeeee.",
        "hheeeeees",
        "heeeeeees",
        "heeeeeess",
        ".esssssss",
        "..sssss..",
    ]),
    'mercy': (['#7e7a6c', '#959181', '#ada997', '#c2bea9'], {'e': '#3f8088', 'h': '#6fb9b9', 's': '#2a5960', 'd': '#1d3d42'}, [
        ".h.h.h...",
        ".e.e.e.h.",
        ".e.e.e.e.",
        ".e.e.e.e.",
        ".eeeeeee.",
        ".eeeeeee.",
        "heeeeeee.",
        ".heeeeee.",
        "..eeeess.",
        "...ssss..",
    ]),
    'horde': (['#4f240f', '#6a3216', '#83431e', '#9a5426'], {'e': '#a9a9b2', 'h': '#dcdce4', 'S': '#d0ad74', 'D': '#8a6a3e'}, [
        ".h..h..h.",
        "heeheehee",
        ".e..e..e.",
        ".S..S..S.",
        ".S..S..S.",
        ".S..S..S.",
        ".S..S..S.",
        ".S..S..S.",
        ".D..D..D.",
    ]),
    'calm': (['#121a31', '#1a2442', '#233054', '#2d3d66'], {'e': '#d6d0a6', 'h': '#f4efca', 's': '#a39c76', 'd': '#8f8a68'}, [
        "...heee..",
        "..hee....",
        ".hee.....",
        ".he......",
        ".ee....h.",
        ".ee......",
        ".ses.....",
        "..sse....",
        "...ssee..",
    ]),
    'haste': (['#271433', '#361c44', '#462656', '#562f68'], {'e': '#e6c240', 'h': '#fbe88a', 's': '#a8842a', 'd': '#6b521a'}, [
        ".....hhe",
        "....hee.",
        "...hee..",
        "..hee...",
        ".heeeee.",
        "....es..",
        "...es...",
        "..es....",
        ".es.....",
        "es......",
    ]),
    'rest': (['#152c1a', '#1d3a23', '#26492c', '#305835'], {'e': '#e3842c', 'h': '#f8c45a', 's': '#5c3b22', 'd': '#3a2414'}, [
        "....h....",
        "...he....",
        "..hee.h..",
        "..eeeeh..",
        ".eeehee..",
        ".eehhhee.",
        "s.eeeee.s",
        ".sss.sss.",
        "d.dsssd.d",
    ]),
    'resilience': (['#262c32', '#323941', '#3f4750', '#4c555f'], {'e': '#b4a98b', 'h': '#ddd3b5', 's': '#7f765f', 'd': '#6d2724'}, [
        "eeeeeeeee",
        "ehhhdhhhe",
        "ehhhdhhhe",
        "eddddddde",
        "ehhhdhhse",
        ".ehhdhse.",
        ".eehdse..",
        "..eedss..",
        "...ees...",
    ]),
    'kin_undead': (['#1b271e', '#243427', '#2e4131', '#384f3b'], {'e': '#cfc5a6', 'h': '#ece4cb', 's': '#9a8f74', 'd': '#121a14'}, [
        "..hhhhh..",
        ".heeeeee.",
        "heeeeeees",
        "eddeeedds",
        "eddeeedds",
        "eeeedeees",
        ".seeeees.",
        "..eseses.",
        "..s.s.s..",
    ]),
    'kin_beasts': (['#35220f', '#462e16', '#563a1e', '#674626'], {'e': '#d2c8a8', 'h': '#eee6cc', 's': '#9c9175', 'd': '#2a1a0d'}, [
        ".he...he.",
        ".es...es.",
        "he.....he",
        "es.....es",
        "...heh...",
        "..heeee..",
        ".heeeees.",
        ".eeeeees.",
        "..sesss..",
    ]),
    'kin_slimes': (['#233c18', '#2d4d1d', '#385f23', '#44712a'], {'e': '#94d457', 'h': '#c8f08a', 's': '#5f9a35', 'd': '#1d3412'}, [
        "hhhhhhhh.",
        "heeeeeees",
        "heddeddes",
        "heddeddes",
        "heeeeeees",
        "heeddddes",
        "heeeeeees",
        "essssssss",
    ]),
    'kin_flyers': (['#2a3850', '#344661', '#405574', '#4c6486'], {'e': '#e2ded2', 'h': '#ffffff', 's': '#9a968a', 'd': '#6c695f'}, [
        "hh.......",
        "heh......",
        "heeh.....",
        "heeeh....",
        "heeeehh..",
        ".eeeeeehh",
        ".seeeeee.",
        "..ssesese",
        "...s.s.s.",
    ]),
    'kin_water': (['#0e2b30', '#13393f', '#19474e', '#20565e'], {'e': '#7cccd4', 'h': '#c0f0f0', 's': '#3d8c96', 'd': '#0b2024'}, [
        "...hhh...",
        "..heeeh.s",
        ".heedeess",
        "heeeeeess",
        "eeeeeeess",
        ".seeeesss",
        "..ssss..s",
    ]),
}


def emblem(face, kind, ox, oy):
    """Draws the emblem with its top left at (ox, oy) of the face, then a dark outline around
    every emblem texel (4-neighbours), like the black edge of vanilla icons. Texels given by a
    capital letter get no outline of their own, so thin parallel lines keep the cloth between."""
    _, colors, rows = KINDS[kind]
    cells, chars = {}, {}
    for j, row in enumerate(rows):
        for i, ch in enumerate(row):
            if ch != '.':
                cells[(ox + i, oy + j)] = colors[ch]
                chars[(ox + i, oy + j)] = ch

    def plain(p):
        return chars[p].isupper()  # capital letters: thin lines that get no outline (spear shafts)
    for (x, y), c in cells.items():
        face.px(x, y, c)
    for (x, y) in [p for p in cells if not plain(p)]:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            p = (x + dx, y + dy)
            if p not in cells:
                face.px(p[0], p[1], OUTLINE)


def emblem_size(kind):
    rows = KINDS[kind][2]
    return max(len(r) for r in rows), len(rows)


def cloth_canvas(kind):
    """The 16 x 32 painting: the cloth spans columns 2..13 and rows 3..23, the point rows 24..27
    (columns 4..11, then 6..9). Trim: a dark border, bone stitching under the top hem, a band
    near the bottom, the emblem centred in between."""
    cloth = KINDS[kind][0]
    pal = dict(PALETTES, cloth=paint.Material(cloth, noise=0.5, patch=3, fine=0.12))
    img = paint.Image(16, 32)
    f = paint.Face(img, 'front', 0, 0, 16, 32, pal, None)
    paint.fill_tile(img, f, pal, 'cloth', paint.seed_of('banner_' + kind), 1.0, period=16)
    # Folds: two darker vertical stripes, as hanging cloth catches less light in its creases.
    for x in (5, 10):
        for y in range(4, 28):
            if (y * 7 + x) % 11 < 8:
                f.px(x, y, ('cloth', 0) if y % 5 else ('cloth', 1))
    f.rect(2, 3, 12, 1, ('cloth', 0))
    f.rect(2, 4, 12, 1, ('cloth', 3))
    for x in range(3, 13, 2):
        f.px(x, 5, STITCH)
    for y in range(3, 24):
        f.px(2, y, ('cloth', 0))
        f.px(13, y, ('cloth', 0))
    f.rect(3, 20, 10, 1, ('cloth', 0))
    for x in range(3, 13):
        f.px(x, 21, STITCH if x % 3 == 0 else ('cloth', 3))
    f.rect(3, 22, 10, 1, ('cloth', 0))
    f.rect(4, 24, 8, 2, ('cloth', 1))
    f.rect(6, 26, 4, 2, ('cloth', 1))
    f.rect(4, 25, 8, 1, ('cloth', 0))
    f.rect(6, 27, 4, 1, ('cloth', 0))
    w, h = emblem_size(kind)
    emblem(f, kind, 2 + (12 - w) // 2, 7 + (12 - h) // 2)
    return img


def cut(img, top):
    out = paint.Image(16, 16)
    for y in range(16):
        for x in range(16):
            out.px[y][x] = img.px[y + (0 if top else 16)][x]
    return out


def pole_draw(f):
    for x in (3, 11):
        f.rect(x, 0, 1, 16, ('pole', 0))
    for y in (5, 13):
        f.px(7, y, ('pole', 3))


def icon(kind):
    def draw(c):
        cloth = KINDS[kind][0]
        c.rect(1, 1, 14, 1, PALETTES['pole'][2])
        c.rect(7, 0, 2, 1, PALETTES['bone'][3])
        c.rect(2, 2, 12, 12, cloth[2])
        for y in range(2, 14):
            c.px(2, y, cloth[0])
            c.px(13, y, cloth[0])
        c.rect(4, 14, 8, 1, cloth[1])
        c.rect(6, 15, 4, 1, cloth[1])
        w, h = emblem_size(kind)
        emblem(c, kind, 2 + (12 - w) // 2, 2 + (12 - h) // 2)
        c.edge(OUTLINE)
    return draw


SIDES = ('north', 'south', 'west', 'east')


def banner_build(kind):
    top_tex, bottom_tex = 'banner_%s_top' % kind, 'banner_%s_bottom' % kind

    def build(m, part):
        if part == 'bottom':
            m.box((4, 0, 4), (12, 2, 12), 'altar_stone')
            m.box((5, 2, 5), (11, 3, 11), 'altar_iron', faces=('up',) + SIDES)
            m.box((7, 3, 7), (9, 16, 9), 'banner_pole', faces=SIDES)
            m.box((2, 8, 6), (14, 16, 7), bottom_tex, faces=('down',) + SIDES)
            m.box((4, 6, 6), (12, 8, 7), bottom_tex, faces=('down',) + SIDES)
            m.box((6, 4, 6), (10, 6, 7), bottom_tex, faces=('down',) + SIDES)
        else:
            m.box((7, 0, 7), (9, 15, 9), 'banner_pole', faces=SIDES)
            m.box((2, 0, 6), (14, 13, 7), top_tex, faces=SIDES)
            m.box((1, 13, 5.5), (15, 14.5, 7.5), 'banner_pole', faces=('up', 'down', 'north', 'south'))
            m.box((0, 12.75, 5.25), (1, 14.75, 7.75), 'altar_bone')
            m.box((15, 12.75, 5.25), (16, 14.75, 7.75), 'altar_bone')
            m.box((6.5, 15, 6.5), (9.5, 17, 9.5), 'altar_bone')
    return build


TEXTURES = [Texture('banner_pole', 'pole', pole_draw)]
SPRITES = []
BLOCKS = []
for _m in DATA['modifiers']:
    _kind = _m['id']
    _img = cloth_canvas(_kind)
    TEXTURES.append(Texture('banner_%s_top' % _kind, image=cut(_img, True)))
    TEXTURES.append(Texture('banner_%s_bottom' % _kind, image=cut(_img, False)))
    _sp = Sprite(_m['banner'], icon(_kind), palettes=PALETTES, name=_m['name'])
    SPRITES.append(_sp)
    BLOCKS.append(Block(_m['banner'], _m['name'], banner_build(_kind), part=('bottom', 'top'), facing=True,
                        on_slab=True, item=_sp, role=_m['text']))
CANVAS = {m['id']: cloth_canvas(m['id']) for m in DATA['modifiers']}  # for the showcase pages
