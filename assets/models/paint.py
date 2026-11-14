"""Procedural pixel textures for SoulsCraft models: palettes, noise, face shading, decals, PNG.

Pure Python (no bpy), so it runs in Blender's bundled Python and in tests alike. Everything is
deterministic: the noise is an integer hash of the texel position and a seed, never random().

A palette is a list of '#rrggbb' colours from dark to light. A face is filled texel by texel: the
noise picks a palette step around the middle, the face shade moves it (top +1, bottom -1, like the
hand-painted vanilla mob textures), and decals are drawn over it by the mob file.

The noise follows vanilla textures: one or two tone shifts within the material, in patches of a
few texels, with only a little per-texel jitter. Scattered single texels of a contrasting colour
read as gravel, so accents (rust, grime, cracks) are placed by decals on purpose: streaks, patches.
"""
import math
import struct
import zlib

# Palette step offset per face: vanilla mob textures are lighter on top and darker below.
FACE_SHADE = {'top': 1, 'bottom': -1, 'front': 0, 'back': 0, 'left': 0, 'right': 0}

# Default noise of a material, in palette steps: `noise` is the amplitude of the patches, `patch`
# their size in texels, `fine` the per-texel jitter on top. Material(...) overrides them.
NOISE = {'noise': 0.85, 'patch': 3, 'fine': 0.2}


def rgb(color):
    """'#rrggbb' or '#rrggbbaa' -> (r, g, b, a)."""
    c = color.lstrip('#')
    a = int(c[6:8], 16) if len(c) == 8 else 255
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), a)


def mix(c1, c2, f):
    """Linear mix of two '#rrggbb' colours, f = 0 gives c1."""
    a, b = rgb(c1), rgb(c2)
    return '#' + ''.join('%02x' % round(a[i] + (b[i] - a[i]) * f) for i in range(3))


def shade(color, f):
    """Lightens towards white for f > 0, darkens towards black for f < 0."""
    r = rgb(color)[:3]
    out = [c + (255 - c) * f if f >= 0 else c * (1 + f) for c in r]
    return '#' + ''.join('%02x' % max(0, min(255, round(c))) for c in out)


class Material(list):
    """A palette with its own noise: Material(['#dark', ..., '#light'], noise=0.4, patch=4).
    It is still a list, so it goes into PALETTES like a plain one; a plain list gets NOISE."""

    def __init__(self, colors, noise=None, patch=None, fine=None):
        super().__init__(colors)
        self.noise = NOISE['noise'] if noise is None else noise
        self.patch = NOISE['patch'] if patch is None else patch
        self.fine = NOISE['fine'] if fine is None else fine


def ramp(color, steps=4, spread=0.22):
    """A palette of `steps` colours around one base colour, dark to light."""
    mid = (steps - 1) / 2
    return [shade(color, (i - mid) * spread) for i in range(steps)]


def hash3(x, y, seed):
    """32-bit integer hash of a texel position: the only source of noise."""
    h = (x * 374761393 + y * 668265263 + seed * 2147483647) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return (h ^ (h >> 16)) & 0xFFFFFFFF


def noise(x, y, seed):
    """Value in [0, 1) for a texel."""
    return hash3(x, y, seed) / 4294967296.0


def patchy(x, y, seed, size):
    """Smooth value noise in [0, 1): hashed values on a lattice every `size` texels, eased between
    them, so neighbouring texels share a value and the noise comes in patches, not specks."""
    gx, gy = x / size, y / size
    x0, y0 = math.floor(gx), math.floor(gy)
    fx, fy = gx - x0, gy - y0
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = noise(x0, y0, seed) + (noise(x0 + 1, y0, seed) - noise(x0, y0, seed)) * fx
    b = noise(x0, y0 + 1, seed) + (noise(x0 + 1, y0 + 1, seed) - noise(x0, y0 + 1, seed)) * fx
    return a + (b - a) * fy


def seed_of(name):
    """A stable seed from a string (FNV-1a), so every material and mob has its own grain."""
    h = 2166136261
    for ch in name.encode('utf-8'):
        h = ((h ^ ch) * 16777619) & 0xFFFFFFFF
    return h


class Image:
    """An RGBA texel grid; (0, 0) is the top left corner, as on the texture sheet."""

    def __init__(self, w, h):
        self.w, self.h = w, h
        self.px = [[(0, 0, 0, 0)] * w for _ in range(h)]

    def set(self, x, y, color):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y][x] = rgb(color) if isinstance(color, str) else color

    def get(self, x, y):
        return self.px[y][x]

    def opaque(self, x, y):
        """True for a texel inside the image with alpha above 0 (the game's test for sprites)."""
        return 0 <= x < self.w and 0 <= y < self.h and self.px[y][x][3] > 0

    def png(self):
        """PNG bytes; zlib level 9 output is fixed for fixed input, so the file is byte-stable."""
        raw = b''.join(b'\x00' + bytes(c for p in row for c in p) for row in self.px)

        def chunk(kind, data):
            body = kind + data
            return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xFFFFFFFF)

        head = struct.pack('>IIBBBBB', self.w, self.h, 8, 6, 0, 0, 0)
        return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', head) + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')

    def save(self, path):
        with open(path, 'wb') as f:
            f.write(self.png())


def load_png(path):
    """Reads an 8-bit RGBA or RGB PNG without interlace (what Image.save writes) into an Image:
    the head icons copy texels of a mob texture, the trophy fit needs the pixels of sprites."""
    with open(path, 'rb') as f:
        data = f.read()
    assert data[:8] == b'\x89PNG\r\n\x1a\n', '%s: not a PNG' % path
    pos, idat, head = 8, b'', None
    while pos < len(data):
        n, kind = struct.unpack('>I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b'IHDR':
            head = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            idat += body
        pos += 12 + n
    w, h, depth, ctype, _, _, interlace = head
    assert depth == 8 and ctype in (2, 6) and not interlace, '%s: only 8-bit RGB(A), no interlace' % path
    bpp = 4 if ctype == 6 else 3
    raw = zlib.decompress(idat)
    stride = w * bpp
    img = Image(w, h)
    prev = bytearray(stride)
    for y in range(h):
        kind = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if kind == 1:
                line[i] = (line[i] + a) & 0xFF
            elif kind == 2:
                line[i] = (line[i] + b) & 0xFF
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 0xFF
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF
        for x in range(w):
            px = tuple(line[x * bpp:(x + 1) * bpp])
            img.px[y][x] = px if bpp == 4 else px + (255,)
        prev = line
    return img


class Face:
    """One face of a cube on the sheet, handed to decal functions.

    Coordinates are face-local texels as laid out on the sheet: x to the right, y down, (0, 0) the
    top left texel of the face. For the four side faces y = 0 is the top edge of the cube and
    x = 0 is the left edge as seen from outside. `name` is front, back, left, right, top or
    bottom in the mob's own terms (left = the mob's left, +x in the game model frame).
    """

    def __init__(self, image, name, u, v, w, h, palettes, cube):
        self.image, self.name, self.u, self.v, self.w, self.h = image, name, u, v, w, h
        self.palettes, self.cube = palettes, cube

    def color(self, mat, step):
        """Palette step of a material, clamped; negative steps count from the light end."""
        pal = self.palettes[mat]
        return pal[max(0, min(len(pal) - 1, step if step >= 0 else len(pal) + step))]

    def px(self, x, y, color):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.image.set(self.u + x, self.v + y, self._c(color))

    def rect(self, x, y, w, h, color):
        for j in range(y, y + h):
            for i in range(x, x + w):
                self.px(i, j, color)

    def outline(self, color, sides='tblr'):
        """A one-texel border: t, b, l, r select the edges."""
        if 't' in sides:
            self.rect(0, 0, self.w, 1, color)
        if 'b' in sides:
            self.rect(0, self.h - 1, self.w, 1, color)
        if 'l' in sides:
            self.rect(0, 0, 1, self.h, color)
        if 'r' in sides:
            self.rect(self.w - 1, 0, 1, self.h, color)

    def get(self, x, y):
        """(r, g, b, a) of a face texel."""
        return self.image.get(self.u + x, self.v + y)

    def line(self, x0, y0, x1, y1, color):
        """A one-texel line, both ends included (Bresenham)."""
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x1 >= x0 else -1), (1 if y1 >= y0 else -1)
        err = dx + dy
        while True:
            self.px(x0, y0, color)
            if x0 == x1 and y0 == y1:
                return
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy

    def rows(self, rows, key, x=0, y=0):
        """A pixel map: rows of characters, key maps a character to a colour; a character that
        is missing from key or maps to None leaves the texel as it is."""
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if key.get(ch) is not None:
                    self.px(x + i, y + j, key[ch])

    def solid(self, x, y):
        """True for a face texel with alpha above 0 (outside the face counts as empty)."""
        return 0 <= x < self.w and 0 <= y < self.h and self.get(x, y)[3] > 0

    def edge(self, color, where=None):
        """Recolours the rim of a sprite: opaque texels with an empty neighbour above, below,
        left or right (the face border counts as empty). where(x, y) limits it to some texels."""
        rim = [(i, j) for j in range(self.h) for i in range(self.w)
               if self.solid(i, j) and (where is None or where(i, j))
               and not all(self.solid(i + a, j + b) for a, b in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
        for i, j in rim:
            self.px(i, j, color)

    def streak(self, x, y, length, color, tail=None):
        """A drip running down from (x, y): rust under a rivet, grime under a slit. tail colours
        the last texel, so the streak fades out."""
        for j in range(length):
            self.px(x, y + j, tail if tail is not None and length > 1 and j == length - 1 else color)

    def patch(self, x, y, w, h, color, density=0.55, seed=0):
        """A blot with a ragged edge inside the box (x, y, w, h): solid in the middle, broken up
        towards the rim by patchy noise, so it reads as one stain and not as scattered dots."""
        s = seed_of(self.name) + seed * 7919
        cx, cy, rx, ry = x + (w - 1) / 2, y + (h - 1) / 2, max(w / 2, 0.5), max(h / 2, 0.5)
        for j in range(y, y + h):
            for i in range(x, x + w):
                d = ((i - cx) / rx) ** 2 + ((j - cy) / ry) ** 2
                if (1 - d) + patchy(self.u + i, self.v + j, s, 2) - 0.5 > 1 - density:
                    self.px(i, j, color)

    def speckle(self, color, density, seed=0):
        """Scattered single texels, deterministic per texel. Use sparingly and with a colour close
        to the base: contrasting specks all over a face read as gravel (prefer patch, streak)."""
        s = seed_of(self.name) + seed
        for y in range(self.h):
            for x in range(self.w):
                if noise(self.u + x, self.v + y, s) < density:
                    self.px(x, y, color)

    def _c(self, color):
        """A colour is '#rrggbb' or (material, step)."""
        if isinstance(color, tuple):
            return self.color(*color)
        return color


def fill(image, face, palettes, mat, seed, grain=1.0):
    """Fills a face with its material: patchy noise picks a step around the middle of the
    palette, a little per-texel jitter breaks up the patch edges, and the face shade shifts it.
    The settings come from the palette (Material) or NOISE; grain scales them (0 = flat colour)."""
    pal = palettes[mat]
    n = len(pal)
    amp = getattr(pal, 'noise', NOISE['noise']) * grain
    size = getattr(pal, 'patch', NOISE['patch'])
    fine = getattr(pal, 'fine', NOISE['fine']) * grain
    mid = (n - 1) / 2 + FACE_SHADE[face.name] * 0.9
    s = seed_of(mat) ^ seed
    for y in range(face.h):
        for x in range(face.w):
            u, v = face.u + x, face.v + y
            r = (patchy(u, v, s, size) - 0.5) * 2 * amp + (noise(u, v, s ^ 0x5BD1E995) - 0.5) * 2 * fine
            # A soft vertical gradient on side faces: texels near the top edge catch more light.
            g = 0.0
            if face.name not in ('top', 'bottom') and face.h > 3:
                g = 0.35 - 0.7 * y / (face.h - 1)
            step = int(round(mid + r + g))
            image.set(u, v, pal[max(0, min(n - 1, step))])


def patchy_tile(x, y, seed, size, period):
    """patchy() that repeats every `period` texels: the lattice has a whole number of cells per
    period (about period / size) and wraps, so a block texture tiles without a seam."""
    n = max(1, round(period / size))
    gx, gy = (x % period) * n / period, (y % period) * n / period
    x0, y0 = math.floor(gx), math.floor(gy)
    fx, fy = gx - x0, gy - y0
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)

    def at(i, j):
        return noise(i % n, j % n, seed)
    a = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * fx
    b = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * fx
    return a + (b - a) * fy


def fill_tile(image, face, palettes, mat, seed, grain=1.0, period=16):
    """A block texture fill: like fill(), but without face shade or gradient (the game shades
    block faces by direction itself) and seamless over `period` texels, so a full block face
    tiles with its neighbours like vanilla stone."""
    pal = palettes[mat]
    n = len(pal)
    amp = getattr(pal, 'noise', NOISE['noise']) * grain
    size = getattr(pal, 'patch', NOISE['patch'])
    fine = getattr(pal, 'fine', NOISE['fine']) * grain
    mid = (n - 1) / 2
    s = seed_of(mat) ^ seed
    for y in range(face.h):
        for x in range(face.w):
            u, v = face.u + x, face.v + y
            r = (patchy_tile(u, v, s, size, period) - 0.5) * 2 * amp
            r += (noise(u % period, v % period, s ^ 0x5BD1E995) - 0.5) * 2 * fine
            step = int(round(mid + r))
            image.set(u, v, pal[max(0, min(n - 1, step))])
