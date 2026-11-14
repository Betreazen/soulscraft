"""Mob effect icons of SoulsCraft: the 18x18 sprites the HUD and the inventory draw for an effect.

The engine looks for a content mod's effect icon at assets/<mod>/textures/mob_effect/<name>.png,
18x18 like vanilla's, and draws the missing texture without one (Horizon docs/mods.md, "mod
effects"). The effects themselves are declared by the content mod (mod/sync.ps1 writes
data/soulscraft/horizon/mob_effect/<name>.json); this file only paints their icons.

Vanilla style: a small picture on a transparent field, the subject about 14 px tall in the middle,
a darker shade of its own colour as the outline, light from the top left, no black frame.

build.py calls write(); it writes assets/soulscraft/textures/mob_effect/<id>.png, byte-stable.
"""
import os

import paint

PALETTES = {
    # The bleed's colour in the content mod is 0x8A0303; the drop is shaded around it.
    'blood': ['#3a0505', '#6e0b08', '#8a0303', '#a81810', '#c8352a', '#ef8c78'],
}


def bleeding(f):
    """A big drop of blood, light on its upper left, and a small one falling beside it."""
    rows = [   # 18 x 18
        '..................',
        '..........o.......',
        '.........oro......',
        '.........oro......',
        '........orrro.....',
        '........ohrro.....',
        '.......ohrrrro....',
        '.......ohrrrdo....',
        '......ohsrrrrdo...',
        '......ohhrrrrdo...',
        '.....ohhrrrrrrdo..',
        '.....ohrrrrrrrdo..',
        '..o..ohrrrrrrddo..',
        '.oro.orrrrrrrddo..',
        '.oho..orrrrrddo...',
        '..o....orrdddo....',
        '........ooooo.....',
        '..................',
    ]
    f.rows(rows, {'o': ('blood', 0), 'd': ('blood', 1), 'r': ('blood', 2), 'h': ('blood', 4), 's': ('blood', 5)})
    f.px(11, 12, ('blood', 3))       # a lighter patch in the body of the drop, as on vanilla's
    f.px(10, 13, ('blood', 3))


EFFECTS = {'bleeding': bleeding}
SIZE = 18


def image(draw):
    img = paint.Image(SIZE, SIZE)
    draw(paint.Face(img, 'front', 0, 0, SIZE, SIZE, PALETTES, None))
    return img


def write(ns):
    """Paints every icon into <ns>/textures/mob_effect/; returns the paths."""
    out = []
    folder = os.path.join(ns, 'textures', 'mob_effect')
    os.makedirs(folder, exist_ok=True)
    for eid, draw in sorted(EFFECTS.items()):
        path = os.path.join(folder, eid + '.png')
        image(draw).save(path)
        out.append(path)
    return out
