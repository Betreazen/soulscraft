"""The tier rewards (docs/design/11-summons-rewards.md, «Предметы ступеней»): the Revenant's Blade,
the Brute's Maul and the Abyss Heart, three 16 x 16 item sprites. No blocks: the file only adds
SPRITES to the altar set, as altar.py adds the horns and the shard.

The two weapons are handheld sprites in the vanilla tool layout (handle bottom left, tip or head
top right, item px (4, 4) in the fist). Along the diagonal a = x - y runs from the handle to the
tip, d = x + y - 15 runs across it (negative d is the upper, lit side), as in the revenant's
rusty sword (mobs/revenant.py, sword_sprite).

The maul in the brute's hands stays the element model (mobs/brute.py, soulscraft:item/brute_maul);
this sprite, brute_maul_sprite, is how the player sees the same maul (mod/sync.ps1 picks it).
"""
from items import HANDHELD_PARENT, Sprite

SET = 'altar'

PALETTES = {
    # The revenant's blade: dark steel, a worn bronze guard and pommel, a black leather grip.
    'steel': ['#24272b', '#363a3f', '#4b5056', '#646a71', '#868d94', '#b3b9be'],
    'bronze': ['#3d2a12', '#5e431e', '#80602d', '#a07d42', '#bf9c5e'],
    'verdigris': ['#3c5c4f', '#4f7a68'],
    'grip': ['#24180f', '#3f2b1d', '#5a3f2a'],
    # The brute's maul (mobs/brute.py palettes): grey stone, dark iron bands, a dark wooden haft.
    'stone': ['#2f2f2d', '#454542', '#5b5b57', '#72726d', '#8a8a84', '#a3a29b'],
    'iron': ['#1d1d1f', '#2c2c2f', '#3e3e42', '#55555a', '#6e6e73'],
    'wood': ['#21170f', '#2f2217', '#433021', '#58402b'],
    'leather': ['#2a1c14', '#3d2a1d', '#553b28'],
    # The abyss heart: deep blue-violet, lit from inside by a faint soul glow.
    'abyss': ['#0d0820', '#170f38', '#22175a', '#30227a', '#44339c', '#5f4cc0'],
    'glow': ['#6b5fd8', '#8f86f0', '#b9b3ff', '#e4e1ff'],
}
SOUL_EYE = '#9cf5e4'          # the revenant's eye colour (mobs/revenant.py EYE)


def blade_sprite(c):
    """Клинок латника: the revenant's straight sword made whole, a long dark-steel blade with a
    fuller and a lit upper edge, a wide bronze crossguard worn green at the ends with a soul-blue
    stone in its middle, a wrapped leather grip and a bronze pommel."""
    guard, grip, pommel = range(-6, -3), range(-10, -6), range(-14, -10)

    def in_grip(x, y):
        return x - y in grip and x + y - 15 in (-1, 0)

    for y in range(16):
        for x in range(16):
            a, d = x - y, x + y - 15
            if a >= -3 and -2 <= d <= 2:
                fuller = d == 0 and -2 <= a <= 9
                c.px(x, y, ('steel', 2) if fuller else ('steel', 4) if d == -1
                     else ('steel', 3) if d == 0 else ('steel', 1))
            elif a in guard and -5 <= d <= 5:
                c.px(x, y, ('bronze', 3) if a == -5 else ('bronze', 1))
            elif in_grip(x, y):
                wrap = y % 2 == 0                            # the leather wound round it
                c.px(x, y, ('grip', 2) if wrap else ('grip', 1))
            elif a in pommel and -2 <= d <= 2:
                c.px(x, y, ('bronze', 4) if d < 0 else ('bronze', 2))
    c.edge('#121314', where=lambda x, y: not in_grip(x, y))
    c.px(14, 0, ('steel', 5))                                # the glint at the point
    c.px(13, 1, ('steel', 5))
    for x, y, col in ((3, 6, ('verdigris', 1)), (8, 11, ('verdigris', 0)),
                      (7, 12, ('verdigris', 1))):            # the guard's worn ends
        if c.solid(x, y):
            c.px(x, y, col)
    c.px(5, 10, SOUL_EYE)                                    # the stone in the guard
    c.px(11, 3, ('steel', 1))                                # a nick in the edge
    c.px(10, 3, '#121314')


# The head: a along the haft, d across it (only a + d odd is a texel).
MAUL_HEAD_A, MAUL_HEAD_D = range(2, 10), range(-6, 8)
MAUL_CHIPS = ((6, -3, ('stone', 4)), (5, -4, ('stone', 2)), (6, 3, ('stone', 1)), (5, 4, ('stone', 1)),
              (4, 3, ('stone', 1)), (7, 4, ('stone', 3)))      # (a, d, colour): a chip and two cracks in the stone


def maul_head(a, d):
    """The colour of a texel of the maul's stone head (a along the haft, d across it): the top
    face towards the tip and the striking face to the upper left lit, the far end and the
    underside in shade, the iron band round its middle where the haft goes through."""
    if d in (-1, 0):
        return ('iron', 4) if a == MAUL_HEAD_A[-2] else ('iron', 3) if d == -1 else ('iron', 2)
    if a == MAUL_HEAD_A[-2]:
        return ('stone', 5) if d < 0 else ('stone', 4)
    if d == MAUL_HEAD_D[1]:
        return ('stone', 5)
    if d == MAUL_HEAD_D[-2] or a == MAUL_HEAD_A[1]:
        return ('stone', 1)
    return ('stone', 3) if d < 0 else ('stone', 2)


def maul_sprite(c):
    """Молот громилы: the brute's two-handed maul, a big block of grey stone across the top right
    with an iron band round its middle, on a dark wooden haft that runs down to the bottom left
    with an iron collar under the head, a leather wrap and an iron butt cap."""
    def in_haft(a, d):
        return -14 <= a <= 1 and d in (-1, 0)

    def bare(a, d):
        """The haft's wood and leather, drawn without an outline like a vanilla tool's handle."""
        return in_haft(a, d) and -12 <= a <= -1

    for y in range(16):
        for x in range(16):
            a, d = x - y, x + y - 15
            if a in MAUL_HEAD_A and d in MAUL_HEAD_D:
                c.px(x, y, maul_head(a, d))
            elif in_haft(a, d):
                if a in (0, 1):
                    c.px(x, y, ('iron', 3) if d == -1 else ('iron', 2))      # collar
                elif -11 <= a <= -6:
                    c.px(x, y, ('leather', 2) if y % 2 else ('leather', 1))
                elif a <= -13:
                    c.px(x, y, ('iron', 3))                                  # butt cap
                else:
                    c.px(x, y, ('wood', 3) if d == -1 else ('wood', 2))
    c.edge('#0f0f10', where=lambda x, y: not bare(x - y, x + y - 15))
    for a, d, col in MAUL_CHIPS:
        x, y = (a + d + 15) // 2, (d - a + 15) // 2
        c.px(x, y, col)


HEART_ROWS = ["................",
              "................",
              "...xxxx..xxxx...",
              "..xxxxxxxxxxxx..",
              ".xxxxxxxxxxxxxx.",
              ".xxxxxxxxxxxxxx.",
              ".xxxxxxxxxxxxxx.",
              ".xxxxxxxxxxxxxx.",
              "..xxxxxxxxxxxx..",
              "...xxxxxxxxxx...",
              "....xxxxxxxx....",
              ".....xxxxxx.....",
              "......xxxx......",
              ".......xx.......",
              "................",
              "................"]
HEART_CORE = (7.0, 7.5)       # where the glow inside is brightest


def heart_sprite(c):
    """Сердце бездны: a heart-shaped dark gem, deep blue-violet, lit from the inside by a faint
    glow round its heart and darker towards the rim and the point; a pale glint on the left lobe
    and a facet line down the cleft."""
    c.rows(HEART_ROWS, {'x': ('abyss', 1)})
    cx, cy = HEART_CORE
    for y in range(16):
        for x in range(16):
            if not c.solid(x, y):
                continue
            r = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            step = ('glow', 1) if r < 1 else ('glow', 0) if r < 2 else ('abyss', 5) if r < 3                 else ('abyss', 4) if r < 4.3 else ('abyss', 3) if r < 5.5 else ('abyss', 2)
            c.px(x, y, step)
    c.edge('#07040f')
    for x, y in ((7, 3), (7, 4), (8, 4)):                    # the cleft between the lobes
        c.px(x, y, ('abyss', 1))
    for x, y in ((11, 9), (10, 10), (9, 11), (12, 8)):       # the shadow side towards the point
        c.px(x, y, ('abyss', 1))
    c.px(3, 4, ('glow', 2))                                  # the glint on the left lobe
    c.px(4, 3, ('glow', 1))
    c.px(7, 7, ('glow', 3))                                  # the heart of the glow


SPRITES = [
    Sprite('revenant_blade', blade_sprite, parent=HANDHELD_PARENT, palettes=PALETTES, name='Клинок латника'),
    Sprite('brute_maul_sprite', maul_sprite, parent=HANDHELD_PARENT, palettes=PALETTES, name='Молот громилы'),
    Sprite('abyss_heart', heart_sprite, palettes=PALETTES, name='Сердце бездны'),
]
