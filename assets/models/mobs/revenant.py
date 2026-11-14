"""Revenant (soulscraft:revenant, Латник-мертвец): a dead man-at-arms in rusty half-plate.

Bucket helmet with an eye slit, pauldrons, a round dented shield on the left forearm and a short
rusty sword in the right hand (socket_hand_r, vanilla handheld chain). Zombie-sized: 32 px from
the feet to the top of the head, the helmet and ridge add two. Stands bladed, shield forward.

Game model px throughout (see rig.py): +x is the mob's left, +y down, -z its front.
Pose angles in degrees: limb x < 0 swings forward/up, body x > 0 leans forward, y > 0 turns the
front to the mob's right (left shoulder forward), right arm z > 0 lifts it out sideways.

Rust is placed, not sprinkled: streaks running down from rivets and slits, a few patches where
water stands (lower edges, the top of the helmet), brown stain around the orange so it fades
into the steel. The plates themselves only carry the patchy two-tone noise of paint.fill.
"""
from items import HANDHELD_PARENT, HELMET, Equipment, Item, Sprite
from paint import Material
from rig import M, R, lerp, pose

ID = 'revenant'

PALETTES = {
    'iron': Material(['#33302d', '#4a4541', '#615b55', '#79716a', '#8e867d'], noise=0.9, patch=3),
    'iron_dark': ['#262422', '#363330', '#48443f', '#5a554f'],
    'rust': ['#45261a', '#653620', '#834a29', '#9c5f35'],
    'stain': ['#43372f', '#54443a', '#665246'],     # steel going brown around the rust
    'flesh': ['#353f31', '#4a5642', '#5f6c55', '#76846a'],
    'cloth': ['#261716', '#382120', '#4c2c29', '#5e3632'],
    'cloth_dark': ['#1c1a19', '#2a2725', '#383431', '#46413d'],
    'leather': ['#2b1e15', '#3f2c1e', '#553c29', '#6a4c34'],
    'wood': Material(['#36261a', '#4c3622', '#634729', '#795833'], noise=0.45, patch=4),
    'slit': ['#070606', '#0d0c0b'],
    'blade': ['#6a645e', '#8a837b', '#a9a197', '#c4bcb1', '#dad3c8'],
    'brass': ['#5a4724', '#7a6233', '#987b44'],
}
EYE = '#9cf5e4'
EYE_CORE = '#e2fff8'
RUST = ('rust', 1)
RUST_LIGHT = ('rust', 2)
STAIN = ('stain', 1)


# ---------------------------------------------------------------------------- decals

def rivets(face, color=('iron', -1), inset=0):
    w, h = face.w, face.h
    for x, y in ((inset, inset), (w - 1 - inset, inset), (inset, h - 1 - inset), (w - 1 - inset, h - 1 - inset)):
        face.px(x, y, color)


def rust_streak(face, x, y, length):
    """Rust running down from a rivet or a slit: orange at the source, brown stain below it."""
    face.streak(x, y, length, RUST, tail=STAIN)


def rust_patch(face, x, y, w, h, seed=0):
    """A patch of rust: a ragged brown stain with orange in its middle."""
    face.patch(x, y, w, h, STAIN, 0.6, seed)
    if w >= 3 and h >= 2:
        face.patch(x + 1, y + (1 if h > 2 else 0), w - 2, max(1, h - 2), RUST, 0.55, seed + 1)


def rust_edge(face, seed=0):
    """Rust along the lower edge of a plate, where the water stands: broken up, not a line."""
    face.patch(0, face.h - 2, face.w, 3, STAIN, 0.4, seed)


def helmet_decal(face):
    if face.name == 'front':
        face.rect(1, 3, 6, 1, ('slit', 0))          # eye slit
        face.rect(3, 4, 2, 5, ('iron', -2))         # the ridge down the face plate
        for x, y in ((1, 5), (2, 6), (1, 7), (6, 5), (5, 6), (6, 7)):
            face.px(x, y, ('slit', 1))              # breathing holes on both cheeks
        face.rect(0, 2, 8, 1, ('iron', -1))          # brow band catches the light
        rust_streak(face, 2, 4, 2)                   # rust weeping from the slit
        rust_streak(face, 5, 4, 1)
        face.px(5, 8, STAIN)
        face.px(6, 8, RUST)
        rivets(face)
    elif face.name in ('left', 'right', 'back'):
        face.rect(0, 2, face.w, 1, ('iron', -1))
        face.px(1, 5, ('iron', 0))
        face.px(face.w - 2, 5, ('iron', 0))
        if face.name == 'left':
            rust_streak(face, 1, 6, 3)               # under the rivet
            rust_patch(face, 4, 6, 4, 3, 2)
        elif face.name == 'right':
            rust_streak(face, face.w - 2, 6, 2)
            rust_edge(face, 3)
        else:
            rust_patch(face, 1, 5, 5, 4, 4)
    elif face.name == 'top':
        face.outline(('iron', -2))
        rust_patch(face, 2, 2, 4, 4, 5)              # rain pools on the crown
    elif face.name == 'bottom':
        face.rect(1, 1, face.w - 2, face.h - 2, ('slit', 1))  # the open bottom of the bucket


def ridge_decal(face):
    if face.name == 'top':
        face.rect(0, 0, face.w, face.h, ('iron', -1))
        face.rect(0, 5, face.w, 2, STAIN)
        face.px(0, 6, RUST)


def eyes_decal(face):
    face.rect(0, 0, face.w, face.h, ('slit', 0))
    face.px(1, 0, EYE)
    face.px(4, 0, EYE)


def breastplate_decal(face):
    if face.name == 'front':
        face.rect(3, 0, 2, face.h, ('iron', -1))     # central ridge
        face.px(3, face.h - 1, ('iron', 1))
        face.rect(0, 0, face.w, 1, ('iron', -2))      # rolled neck edge
        rivets(face, ('iron', -1))
        rust_streak(face, 0, 1, 3)                    # under the rivets
        rust_streak(face, 7, 1, 2)
        face.px(5, 4, ('iron_dark', 0))               # a dent, rust starting in it
        face.px(6, 5, ('iron', -1))
        face.px(5, 5, STAIN)
        face.px(1, 5, STAIN)
    elif face.name == 'back':
        face.rect(0, 0, face.w, 1, ('iron', -2))
        face.rect(3, 1, 2, 4, ('leather', 1))          # straps crossing the back
        rust_edge(face, 11)
    elif face.name in ('left', 'right'):
        face.rect(0, 1, face.w, 1, ('leather', 1))
        rust_streak(face, 1, 2, 2)


def fauld_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 1, face.w, 1, ('iron_dark', 0))  # overlapping lames
        face.rect(0, 3, face.w, 1, ('iron_dark', 1))
        if face.name == 'front':
            face.streak(2, 2, 1, STAIN)               # stains from under the lames
            face.streak(6, 4, 1, STAIN)
            face.px(5, 2, RUST)


def belt_decal(face):
    if face.name == 'front':
        face.rect(3, 0, 2, 1, ('brass', 2))
        face.px(3, 0, ('brass', 1))


def pauldron_decal(face):
    if face.name == 'top':
        face.outline(('iron', -1), 'tb')
        rust_patch(face, 1, 1, 3, 3, 31)
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('iron_dark', 0))
        face.px(1, 0, ('iron', -1))
    if face.name in ('left', 'right'):
        face.px(face.w // 2, 1, ('iron', -1))          # rivet on the outer side, stain under it
        face.px(face.w // 2, 2, STAIN)


def lame_decal(face):
    face.rect(0, face.h - 1, face.w, 1, ('iron_dark', 0))


def arm_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 10, face.w, 2, ('flesh', 1))      # bony knuckles below the bracer
        face.px(1, 11, ('flesh', 0))
        face.rect(0, 0, face.w, 1, ('cloth', 1))        # a rag of the gambeson sleeve
        face.patch(0, 1, face.w, 3, ('flesh', 0), 0.35, 51)  # rot under the rag
    if face.name == 'bottom':
        face.rect(0, 0, face.w, face.h, ('flesh', 0))


def bracer_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 0, face.w, 1, ('iron', -1))
        face.rect(0, 2, face.w, 1, ('leather', 1))     # strap
        rust_streak(face, 1, 3, 2)                       # under the strap buckle


def leg_decal(face):
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, 11, face.w, 1, ('leather', 0))     # boot sole
        face.patch(0, 8, face.w, 3, ('cloth_dark', 0), 0.4, 71)  # mud on the boots


def greave_decal(face):
    if face.name == 'front':
        face.rect(1, 0, 2, face.h, ('iron', -1))
    if face.name in ('front', 'back', 'left', 'right'):
        face.rect(0, face.h - 1, face.w, 1, ('iron_dark', 0))
        rust_edge(face, 81)


def knee_decal(face):
    if face.name == 'front':
        face.px(1, 1, STAIN)
        face.px(2, 1, RUST)


def shield_face(face):
    """The outer face of the round shield: planks, an iron rim on the corners, a dent."""
    if face.name != 'left':
        if face.name == 'right':  # inner side: bare planks and the arm strap
            face.rect(0, 3, face.w, 2, ('leather', 1))
        return
    face.rect(2, 0, 1, face.h, ('wood', 0))
    face.rect(5, 0, 1, face.h, ('wood', 0))
    for x, y in ((0, 0), (7, 0), (0, 7), (7, 7)):
        face.px(x, y, ('iron', 1))
    for x, y in ((1, 0), (0, 1), (6, 0), (7, 1), (0, 6), (1, 7), (7, 6), (6, 7)):
        face.px(x, y, ('iron', 2))
    face.px(5, 2, ('wood', 0))                         # the dent: a dark crescent, a lit edge
    face.px(6, 3, ('wood', 0))
    face.px(6, 2, ('wood', -1))
    face.px(1, 6, STAIN)                               # rust from the rim bleeding into the wood


def rim_decal(face):
    if face.name in ('left', 'right'):
        face.patch(0, 0, face.w, face.h, STAIN, 0.3, 91)
    if face.name == 'left':
        face.px(face.w // 2, face.h // 2, RUST)


def boss_decal(face):
    if face.name == 'left':
        face.px(1, 1, ('iron', -1))
        face.px(2, 2, RUST)
        face.px(1, 2, STAIN)


# ---------------------------------------------------------------------------- items

def helmet_cubes(cube):
    """The bucket helmet as head-local cubes; the mob wears them, the dropped helmet is made of
    the same cubes. cube(origin, size, mat, **kw) adds one."""
    return [
        cube((-4, -9, -4), (8, 9, 8), 'iron', inflate=0.5, decal=helmet_decal),
        cube((-0.5, -10, -4), (1, 1, 8), 'iron_dark', decal=ridge_decal),
    ]


OUTLINE = '#1e1b19'


def in_grip(x, y):
    """The leather grip texels of the sword sprite: a two-texel band down to the bottom left."""
    return -10 <= x - y <= -6 and -1 <= x + y - 15 <= 0


def sword_sprite(c):
    """16 x 16 sprite of the short rusty sword, vanilla sword layout: the blade on the diagonal
    from the guard to the tip in the top right corner, a dark rim, a lit edge on the upper side,
    the crossguard across the diagonal, the grip and pommel towards the bottom left. a runs along
    the blade (x - y), d across it (x + y - 15)."""
    for y in range(16):
        for x in range(16):
            a, d = x - y, x + y - 15
            if a >= -2 and -2 <= d <= 2:
                c.px(x, y, ('blade', -1) if d == -1 else ('blade', 2) if d == 0 else ('blade', 1))
            elif -5 <= a <= -3 and -5 <= d <= 4:
                c.px(x, y, ('iron', 3) if a == -4 else ('iron', 2))      # crossguard
            elif in_grip(x, y):
                c.px(x, y, ('leather', 2) if d == -1 else ('leather', 1))
            elif -13 <= a <= -11 and -2 <= d <= 1:
                c.px(x, y, ('iron', 3))                                  # pommel
    c.edge(OUTLINE, where=lambda x, y: not in_grip(x, y))
    # Rust creeps up from the guard along the dark side, a stain half way, pits near the tip.
    for x, y, col in ((7, 9, ('rust', 0)), (7, 8, RUST), (8, 8, STAIN), (8, 7, RUST), (10, 5, STAIN),
                      (11, 5, RUST), (11, 4, STAIN), (13, 2, STAIN)):
        if c.solid(x, y):
            c.px(x, y, col)
    c.px(10, 3, '#00000000')  # a notch bitten out of the edge
    c.px(11, 3, OUTLINE)


RUSTY_SWORD = Sprite('rusty_sword', sword_sprite, parent=HANDHELD_PARENT, palettes=PALETTES, name='Ржавый меч')


def rusty_helmet():
    from rig import Cube
    it = Item('rusty_helmet', PALETTES, HELMET, texture=32, name='Ржавый шлем')
    it.from_head_cubes(helmet_cubes(lambda o, s, mat, **kw: Cube(o, s, mat, **kw)))
    return it


# 16 x 16 inventory icon of the rusty chestplate.
CHEST_ROWS = [
    '................',
    '..##........##..',
    '.#ll#......#ll#.',
    '.#lii######iil#.',
    '.#iiiiillliiii#.',
    '..#iiiillliii#..',
    '...#iirilliii#..',
    '...#iiiilliri#..',
    '...#iiiilliii#..',
    '...#irii..iii#..',
    '...#iiiilliii#..',
    '...#iiiilliir#..',
    '...#dddddddddd#.',
    '...#iriiiiriii#.',
    '....##########..',
    '................',
]
CHEST_KEY = {'#': '#2c2927', 'i': '#5b544e', 'l': '#90857a', 'r': '#8c4a22', 'd': '#433e3a'}


def chest_decal(face):
    """The vanilla 64 x 32 armour layer: breastplate on the body, plates on the arms."""
    if face.cube.size == (8, 12, 4):  # body
        if face.name == 'front':
            face.rect(3, 0, 2, 9, ('iron', -1))
            face.rect(0, 0, face.w, 1, ('iron', -2))
            face.rect(0, 9, face.w, 1, ('iron_dark', 0))
            face.rect(0, 10, face.w, 1, ('leather', 1))
            face.rect(3, 10, 2, 1, ('brass', 2))
            rivets(face, ('iron', -1))
            rust_streak(face, 0, 1, 3)
            rust_streak(face, 7, 1, 2)
            rust_patch(face, 5, 5, 3, 3, 111)
        if face.name == 'back':
            face.rect(0, 0, face.w, 1, ('iron', -2))
            face.rect(3, 1, 2, 5, ('leather', 1))
            face.rect(0, 10, face.w, 1, ('leather', 1))
            face.patch(0, 7, face.w, 3, STAIN, 0.4, 112)
    else:  # arm: a pauldron on top, bare below the plates
        if face.name in ('front', 'back', 'left', 'right'):
            face.rect(0, 3, face.w, 1, ('iron_dark', 0))
            face.px(1, 4, RUST)
            face.px(1, 5, STAIN)
            for y in range(6, 12):
                face.rect(0, y, face.w, 1, '#00000000')
        if face.name == 'bottom':
            face.rect(0, 0, face.w, face.h, '#00000000')


def rusty_set():
    eq = Equipment('rusty', PALETTES)
    eq.region('body', 'iron', chest_decal)
    eq.region('arm', 'iron', chest_decal)
    return eq


ITEMS = [rusty_helmet()]
SPRITES = [RUSTY_SWORD, Sprite('rusty_chestplate', rows=CHEST_ROWS, key=CHEST_KEY, name='Ржавый нагрудник')]
ARMOR = [rusty_set()]


# ---------------------------------------------------------------------------- model

def model(m):
    root = m.part('root', (0, 24, 0))
    body = m.part('body', (0, 12, 0), parent='root')
    body.cube((-4, -12, -2), (8, 7, 4), 'iron', inflate=0.75, decal=breastplate_decal)
    body.cube((-4, -5, -2), (8, 5, 4), 'iron', inflate=0.5, decal=fauld_decal)
    body.cube((-4, -2, -2), (8, 1, 4), 'leather', inflate=0.6, decal=belt_decal)

    head = m.part('head', (0, 0, 0), parent='body')
    helmet_cubes(head.cube)
    head.cube((-3, -6, -4.6), (6, 1, 0), 'slit', glow=True, decal=eyes_decal, faces=('front',))

    arm_r = m.part('arm_r', (-5, 2, 0), parent='body')
    a = arm_r.cube((-3, -2, -2), (4, 12, 4), 'flesh', decal=arm_decal)
    b = arm_r.cube((-3, 4, -2), (4, 5, 4), 'iron', inflate=0.35, decal=bracer_decal)
    # Pauldron sides 0.1 px out: its inner side would lie flush with the arm's (z-fighting).
    c = arm_r.cube((-4, -3, -3), (5, 3, 6), 'iron', inflate=(0.1, 0, 0), decal=pauldron_decal)
    # The lame is 0.1 px thinner: its bottom met the shield rim's bottom in one plane.
    d = arm_r.cube((-4, 0, -2.5), (4, 2, 5), 'iron', inflate=(0, -0.1, 0), decal=lame_decal)

    arm_l = m.part('arm_l', (5, 2, 0), parent='body')
    arm_l.cube((-1, -2, -2), (4, 12, 4), 'flesh', share=a, mirror=True)
    arm_l.cube((-1, 4, -2), (4, 5, 4), 'iron', inflate=0.35, share=b, mirror=True)
    arm_l.cube((-1, -3, -3), (5, 3, 6), 'iron', inflate=(0.1, 0, 0), share=c, mirror=True)
    arm_l.cube((0, 0, -2.5), (4, 2, 5), 'iron', inflate=(0, -0.1, 0), share=d, mirror=True)

    # Round shield strapped to the outer side of the left forearm, its face to +x.
    shield = m.part('shield', (8.5, 8, 0), parent='arm_l')
    shield.cube((0, -4, -4), (1, 8, 8), 'wood', decal=shield_face)
    shield.cube((0, -5, -3), (1, 1, 6), 'iron', decal=rim_decal)
    shield.cube((0, 4, -3), (1, 1, 6), 'iron', decal=rim_decal)
    shield.cube((0, -3, -5), (1, 6, 1), 'iron', decal=rim_decal)
    shield.cube((0, -3, 4), (1, 6, 1), 'iron', decal=rim_decal)
    shield.cube((1, -1.5, -1.5), (1, 3, 3), 'iron', decal=boss_decal)

    leg_r = m.part('leg_r', (-2, 12, 0), parent='root')
    e = leg_r.cube((-2, 0, -2), (4, 12, 4), 'cloth_dark', decal=leg_decal)
    f = leg_r.cube((-2, 6, -2), (4, 6, 4), 'iron', inflate=0.4, decal=greave_decal)
    g = leg_r.cube((-2, 4.5, -3), (4, 2, 1), 'iron_dark', decal=knee_decal)
    leg_l = m.part('leg_l', (2, 12, 0), parent='root')
    leg_l.cube((-2, 0, -2), (4, 12, 4), 'cloth_dark', share=e, mirror=True)
    # 0.1 px deeper than the right greave: at rest the two overlap at the middle in one plane.
    leg_l.cube((-2, 6, -2), (4, 6, 4), 'iron', inflate=(0.4, 0.5, 0.5), share=f, mirror=True)
    leg_l.cube((-2, 4.5, -3), (4, 2, 1), 'iron_dark', share=g, mirror=True)

    m.socket('socket_hand_r')
    m.socket('socket_hand_l')
    m.socket('socket_head', 'head', (0, -9.5, 0))
    m.socket('socket_back', 'body', (0, -6, 2.75))
    m.hold('socket_hand_r', RUSTY_SWORD)


# ---------------------------------------------------------------------------- poses

# Bladed stance: left shoulder and shield to the front, sword low and back, weight forward.
STANCE = dict(
    body=R(8, 30, 0),
    head=R(-8, -26, 0),
    arm_l=R(-35, 28, 0),
    arm_r=R(-12, -8, 12),
    leg_l=R(-14, 18, -3),
    leg_r=R(14, 18, 3),
)
# Shield up: the left forearm across the chest, the face of the shield to the front, chin down.
BLOCK = pose(STANCE, body=R(4, 0, 0), head=R(10, 4, 0), arm_l=R(-55, 22, 0), arm_r=R(-6, 0, 4))


def attack_thrust(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    # Cocked: the right shoulder far back, the hand drawn back past the hip, the blade level.
    back = pose(STANCE, body=R(-8, 34, 0) + M(0, 0, 1.5), head=R(6, -30, 0), arm_r=R(22, 34, 28),
                arm_l=R(20, -22, -12), leg_r=R(8, 0, 0), leg_l=R(-6, 0, 0))
    lunge = pose(STANCE, body=R(22, -48, 0) + M(0, 0.5, -2), head=R(-18, 46, 0), arm_r=R(-20, 16, -6),
                 arm_l=R(30, -24, -14), leg_l=R(-22, 0, 0), leg_r=R(20, 0, 0), root=M(0, 0, -1))
    a.key(0, STANCE, 'out')
    a.key(hit - 2, back, 'in')        # anticipation: sword drawn back, shield dropped; then the release
    a.key(hit, lunge, 'out')
    a.key(rec, pose(lunge, arm_r=R(-6, 0, 0), body=R(4, 0, 0)), 'hold')
    a.key(rec + 3, pose(lunge, arm_r=R(-4, 0, 0), body=R(3, 0, 0)), 'slow')
    a.key(end, STANCE)


def attack_slash(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    over = pose(STANCE, body=R(-10, 40, -6), head=R(6, -34, 0), arm_r=R(-100, 20, 34), arm_l=R(30, -20, -18),
                leg_l=R(-6, 0, 0))
    cut = pose(STANCE, body=R(24, -38, 6) + M(0, 0.5, -1), head=R(-16, 34, 0), arm_r=R(-48, -30, -22),
               arm_l=R(32, -30, -16), leg_l=R(-22, 0, 0), leg_r=R(18, 0, 0))
    a.key(0, STANCE, 'out')
    a.key(hit - 4, over, 'inout')     # sword over the shoulder, held long enough to read
    a.key(hit - 2, pose(over, arm_r=R(-8, 0, 4), body=R(-3, 4, 0)), 'in')
    a.key(hit, cut, 'out')
    a.key(rec, pose(cut, arm_r=R(40, -8, -8), body=R(4, -8, 0)), 'hold')
    a.key(rec + 4, pose(cut, arm_r=R(36, -6, -6), body=R(2, -6, 0)), 'slow')
    a.key(end, STANCE)


def attack_overhead(a):
    hold, hit, rec, end = a.at('hold'), a.at('hit'), a.at('recover'), a.at('end')
    raised = pose(STANCE, body=R(-16, 12, 0) + M(0, -0.5, 1), head=R(8, -10, 0), arm_r=R(-125, -6, -14),
                  arm_l=R(28, -26, -30), leg_l=R(-8, 0, 0), leg_r=R(4, 0, 0))
    down = pose(STANCE, body=R(30, -22, 0) + M(0, 1.5, -2), head=R(-24, 22, 0), arm_r=R(-40, 0, -12),
                arm_l=R(30, -26, -26), leg_l=R(-30, 0, 0), leg_r=R(22, 0, 0), root=M(0, 0, -1))
    stuck = pose(down, arm_r=R(46, 0, 0), body=R(8, 0, 0) + M(0, 0.5, 0))
    a.key(0, STANCE, 'out')
    a.key(hold - 3, pose(raised, arm_r=R(-6, 0, 0)), 'inout')   # up fast, a little past the top
    a.key(hold, raised, 'snap')       # frozen over the head; the release comes late and fast
    a.key(hit, down, 'out')
    a.key(rec, stuck, 'hold')         # the blade bites the ground: the long window
    a.key(rec + 7, pose(stuck, body=R(-2, 0, 0)), 'slow')
    a.key(end, STANCE)


def shield_bash(a):
    hit, rec, end = a.at('hit'), a.at('recover'), a.at('end')
    pull = pose(STANCE, body=R(-6, -12, 0) + M(0, 0, 1), head=R(4, 6, 0), arm_l=R(-40, 52, 0), arm_r=R(24, 0, 12),
                leg_l=R(4, 0, 0))
    bash = pose(STANCE, body=R(20, 34, 0) + M(0, 0.5, -2), head=R(-14, -18, 0), arm_l=R(-60, 10, 0),
                arm_r=R(30, 0, 16), leg_l=R(-28, 0, 0), leg_r=R(20, 0, 0), root=M(0, 0, -1.5))
    a.key(0, STANCE, 'out')
    a.key(hit - 3, pull, 'inout')     # shield drawn to the chest
    a.key(hit - 2, pose(pull, body=R(-2, -4, 0)), 'in')
    a.key(hit, bash, 'out')
    a.key(rec, pose(bash, body=R(2, 4, 0)), 'slow')
    a.key(end, STANCE)


def clips(c, mob):
    attacks = {x['clip']: x for x in mob['attacks']}

    idle = c.clip('idle', 40, loop=True)
    idle.key(0, STANCE)
    idle.key(20, pose(STANCE, body=R(2, 0, 0) + M(0, 0.3, 0), head=R(-2, 2, 0), arm_r=R(-3, 0, 2), arm_l=R(-2, 0, 0)))
    idle.key(40, STANCE)

    walk = c.clip('walk', 20, loop=True)   # one step cycle; the game drives it by walk_position
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        bob = M(0, -0.6 if s == 0 else 0, 0)
        walk.key(f, pose(STANCE, leg_l=R(-26 * s, 0, 0), leg_r=R(26 * s, 0, 0), arm_r=R(16 * s, 0, 0),
                         arm_l=R(-4 * s, 0, 0), body=bob + R(0, 3 * s, 0), head=R(0, -3 * s, 0)))

    block = c.clip('block', 20, loop=True)
    block.key(0, BLOCK)
    block.key(10, pose(BLOCK, body=R(1, 0, 0) + M(0, 0.2, 0), arm_l=R(-2, 0, 0)))
    block.key(20, BLOCK)

    strafe = c.clip('circle_strafe', 20, loop=True)  # side steps to the mob's right, shield up
    for f, s in ((0, 1), (5, 0), (10, -1), (15, 0), (20, 1)):
        strafe.key(f, pose(BLOCK, leg_l=R(-8 * s, 0, -10 * s), leg_r=R(8 * s, 0, -10 * s),
                           body=M(0, -0.4 if s == 0 else 0, 0) + R(0, 0, 2 * s)))

    back = c.clip('backstep', 12)
    hop = pose(BLOCK, body=R(-10, 0, 0), leg_l=R(-20, 0, 0), leg_r=R(-34, 0, 0), root=M(0, -2, 0))
    back.key(0, STANCE, 'out')
    back.key(3, pose(BLOCK, body=R(6, 0, 0), leg_l=R(-10, 0, 0), leg_r=R(12, 0, 0), root=M(0, 0.5, 0)), 'out')
    back.key(7, hop, 'in')
    back.key(9, pose(BLOCK, body=R(8, 0, 0), root=M(0, 0.6, 0)), 'inout')
    back.key(12, BLOCK)

    for name, fn in (('attack_thrust', attack_thrust), ('attack_slash', attack_slash),
                     ('attack_overhead', attack_overhead), ('shield_bash', shield_bash)):
        fn(c.attack(name, attacks[name]))

    hurt = c.clip('hurt', 10)   # vanilla hurt_time is 10 ticks
    hurt.key(0, STANCE, 'out')
    hurt.key(2, pose(STANCE, body=R(-16, -6, 4) + M(0, 0, 1), head=R(-20, 8, 0), arm_r=R(-30, 0, 20),
                     arm_l=R(-10, 0, -14), root=M(0, 0, 1)), 'slow')
    hurt.key(10, STANCE)

    death = c.clip('death', 20)  # vanilla death_time is 20 ticks; the mod topples the model itself
    sag = pose(STANCE, body=R(26, 10, 0), head=R(30, -10, 0), arm_r=R(30, 0, 30), arm_l=R(10, 0, -40),
               leg_l=R(-10, 0, 0), leg_r=R(6, 0, 0))
    death.key(0, STANCE, 'out')
    death.key(5, sag, 'in')
    death.key(18, pose(sag, root=R(0, 0, 88)), 'back')
    death.key(20, pose(sag, root=R(0, 0, 90)))

    trophy_clip(c)


# The trophy pose (the diorama, docs/design/09-trophies.md): the shield raised, the sword cocked
# high for the delayed overhead, weight on the back foot. A slow loop: it breathes, the blade
# trembles over the head.
TROPHY_POSE = pose(STANCE, body=R(-10, 4, 0) + M(0, -0.4, 1), head=R(10, -6, 0), arm_r=R(-138, -6, -14),
                   arm_l=R(-55, 22, 0), leg_l=R(-10, 0, 0), leg_r=R(8, 0, 0))


def trophy_clip(c):
    t = c.clip('trophy', 80, loop=True)
    t.key(0, TROPHY_POSE)
    t.key(40, pose(TROPHY_POSE, body=R(1.5, 0, 0) + M(0, 0.3, 0), head=R(-2, 3, 0), arm_r=R(-4, 0, 1), arm_l=R(2, 0, 0)))
    t.key(80, TROPHY_POSE)


# Trophies (trophies.py, docs/design/09-trophies.md): the head block; the diorama shows the clip
# `trophy` (the scene, light and particles are in blocks/dioramas.py).
TROPHY = {
    'head': {'parts': ['head'], 'scale': 0.9},
}
