"""Mob heads (docs/design/09-trophies.md): <mob>_head (state rotation 0..15, on the floor) and
<mob>_wall_head (facing). Models without elements, the block entity draws the head from the mob's
rig; the item is a flat icon cut from the front face of the head's main cube on the mob texture.
The trophy proper, the diorama, is blocks/dioramas.py.
"""
import json
import os

import paint
from blocks import Block, Texture
from items import Sprite
from paint import Material

SET = 'trophies'
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
    MOBS = json.load(f)['mobs']

PALETTES = {
    'bone': Material(['#6f6650', '#8c826a', '#a99f84', '#c3b99c', '#d8cfb3'], noise=0.45, patch=3, fine=0.1),
}
TEXTURES = [Texture('trophy_bone', 'bone')]   # the heads' break particles


# ---------------------------------------------------------------------------- heads

def no_elements(m):
    """A mob head: the block entity draws it; the model gives only the break particles."""


def head_icon(mob_id):
    """The head item's icon: the front face of the head's main cube cut from the mob texture,
    scaled up by a whole number to fill the 16 x 16 sprite, outlined dark."""
    path = os.path.join(ROOT, 'assets', 'rigs', mob_id + '.trophy.json')
    with open(path, encoding='utf-8') as f:
        head = json.load(f)['head']
    part, index = head['icon']
    cube = next(p for p in head['parts'] if p['name'] == part)['cubes'][index]
    tex = paint.load_png(os.path.join(ROOT, 'assets', 'textures', 'entity', mob_id + '.png'))
    w, h, d = cube['size']
    u, v = cube['uv'][0] + d, cube['uv'][1] + d
    k = max(1, min(14 // max(w, 1), 14 // max(h, 1)))
    x0, y0 = (16 - w * k) // 2, (16 - h * k) // 2

    def draw(c):
        for y in range(h * k):
            for x in range(w * k):
                c.px(x0 + x, y0 + y, '#%02x%02x%02x' % tex.get(u + x // k, v + y // k)[:3])
        c.edge('#120e0c')
    return Sprite(mob_id + '_head', draw, name='Голова: ' + MOBS_BY[mob_id]['name'])


MOBS_BY = {m['id']: m for m in MOBS}
HEADS = [m['id'] for m in MOBS if os.path.exists(os.path.join(ROOT, 'assets', 'rigs', m['id'] + '.trophy.json'))]
ICONS = {mid: head_icon(mid) for mid in HEADS}
SPRITES = list(ICONS.values())

BLOCKS = []
for mid in HEADS:
    name = MOBS_BY[mid]['name']
    BLOCKS.append(Block(mid + '_head', 'Голова: ' + name, no_elements, entity_states={'rotation': tuple(range(16))},
                        particle='trophy_bone', item=ICONS[mid], role='Голова на полу, 16 направлений'))
    BLOCKS.append(Block(mid + '_wall_head', 'Голова на стене: ' + name, no_elements, facing=True, particle='trophy_bone',
                        item=ICONS[mid], role='Голова на стене'))
