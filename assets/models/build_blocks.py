"""Builds the SoulsCraft blocks from code: block models, blockstates, item models, textures,
Blender scenes, glTF for the showcase. build.py runs it after the mobs; it also runs alone:

    blender --background --factory-startup --python assets/models/build_blocks.py

Every file blocks/<name>.py adds to a block set (its SET, 'altar'). A block file defines:

    SET          the set it belongs to (the glb blocks_<set>.glb holds every model of the set)
    PALETTES     {material: ['#dark', ..., '#light']} or paint.Material
    TEXTURES     [blocks.Texture]: 16 x 16 block textures, soulscraft:block/<id>
    BLOCKS       [blocks.Block]
    SPRITES      optional [items.Sprite]: block item icons and plain items (the summon item)
    structures(blocks)  optional: [blocks.Structure] built from the blocks of every file

Writes:
    assets/soulscraft/models/block/<model>.json, blockstates/<block>.json, models/item/<id>.json
    assets/soulscraft/textures/block/<tex>.png, textures/item/<sprite>.png
    assets/models/blocks_<set>.glb        every model of the set as a node named by its model id,
                                          at the origin (the block's north-west bottom corner)
    assets/models/<structure>.glb         a structure (altar_level_1..4): linked copies of models
    assets/models/blocks.blend            a scene per block, per structure, and the export scenes
    docs/showcase/js/blocks.js            the block registry for the showcase (SC.blocks)
and runs embed.py. The build stops on a model outside the engine limits (blocks.Model.check) and
on z-fighting inside a model, a block's assembled segments, or a structure. Deterministic.
"""
import glob
import json
import os
import runpy
import sys

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
sys.dont_write_bytecode = True
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import blocks  # noqa: E402
import paint  # noqa: E402
import rig  # noqa: E402
import zfight  # noqa: E402

NS = os.path.join(ROOT, 'assets', 'soulscraft')
JS = os.path.join(ROOT, 'docs', 'showcase', 'js')
GAP = 1.5  # blocks between the variants of a block in its Blender scene


def out(*parts):
    path = os.path.join(*parts)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    return path


def write_json(path, obj):
    with open(out(path), 'w', encoding='utf-8', newline='\n') as f:
        f.write(rig.dumps(obj) + '\n')


def load_sets():
    """Block files in name order: [(name, module dict)]."""
    return [(os.path.splitext(os.path.basename(p))[0], runpy.run_path(p))
            for p in sorted(glob.glob(os.path.join(HERE, 'blocks', '*.py')))]


class Built:
    def __init__(self):
        self.blocks = {}      # id -> Block
        self.set_of = {}      # block id -> set
        self.models = {}      # model id -> Model
        self.model_set = {}   # model id -> set
        self.textures = {}    # id -> (Texture, palettes, set)
        self.sprites = []
        self.structures = []
        self.zfights = []
        self.particles = {}   # id -> the block file (module dict) whose PARTICLES defines it


def collect(sets):
    b = Built()
    for name, mod in sets:
        s = mod['SET']
        for t in mod.get('TEXTURES', []):
            assert t.id not in b.textures, 'texture %s twice' % t.id
            b.textures[t.id] = (t, mod['PALETTES'], s)
        for blk in mod.get('BLOCKS', []):
            assert blk.id not in b.blocks, 'block %s twice' % blk.id
            b.blocks[blk.id] = blk
            b.set_of[blk.id] = s
            for mid, m in blk.models().items():
                assert mid not in b.models, 'model %s twice' % mid
                b.models[mid] = m
                b.model_set[mid] = s
        b.sprites.extend(mod.get('SPRITES', []))
        for pid in mod.get('PARTICLES', {}):
            assert pid not in b.particles, 'particle %s twice' % pid
            b.particles[pid] = mod
    for name, mod in sets:
        if 'structures' in mod:
            b.structures.extend(mod['structures'](b.blocks))
    for mid, m in b.models.items():
        for t in m.textures():
            assert t in b.textures, '%s: no texture %s' % (mid, t)
    return b


def write_game_files(b):
    for old in glob.glob(os.path.join(NS, 'models', 'block', '*.json')) + glob.glob(os.path.join(NS, 'blockstates', '*.json')) \
            + glob.glob(os.path.join(NS, 'textures', 'block', '*.png')) + glob.glob(os.path.join(NS, 'particles', '*.json')) \
            + glob.glob(os.path.join(NS, 'textures', 'particle', '*.png')):
        os.remove(old)
    for pid, mod in b.particles.items():
        mod['particle_image'](mod['PARTICLES'][pid]).save(out(NS, 'textures', 'particle', pid + '.png'))
        write_json(os.path.join(NS, 'particles', pid + '.json'), mod['particle_json'](pid))
    for tid, (t, pal, s) in b.textures.items():
        t.image(pal, paint.seed_of(s)).save(out(NS, 'textures', 'block', tid + '.png'))
    for mid, m in b.models.items():
        write_json(os.path.join(NS, 'models', 'block', mid + '.json'), m.json())
    for bid, blk in b.blocks.items():
        write_json(os.path.join(NS, 'blockstates', bid + '.json'), blk.blockstate())
        write_json(os.path.join(NS, 'models', 'item', bid + '.json'), blk.item_json())
    for sp in b.sprites:
        sp.image().save(out(NS, 'textures', 'item', sp.id + '.png'))
        write_json(os.path.join(NS, 'models', 'item', sp.id + '.json'), sp.json())


def block_layout(blk):
    """The block's pieces side by side, segments stacked: [(model id, (x, y, z), 0)]."""
    combos = blk.combos()
    columns = []
    for st in combos:
        key = tuple((k, v) for k, v in st.items() if k != 'part')
        if key not in columns:
            columns.append(key)
    placed = []
    for i, key in enumerate(columns):
        st = dict(key)
        for j, p in enumerate(blk.part or (None,)):
            if p is not None:
                st['part'] = p
            placed.append((blk.resolve(st)[0], (i * 2, j, 0), 0))
    return placed


def check_zfight(b):
    found = blocks.check_models(b.models.values())
    b.zfights.extend(zfight.report('models', found))
    for bid, blk in b.blocks.items():
        placed = block_layout(blk)
        if blk.on_slab:  # the same piece standing on an altar slab, as in a structure
            placed = [(mid + '_on_slab', at, y) for mid, at, y in placed]
            placed += [('altar_slab', (at[0], at[1] - 1, at[2]), 0) for mid, at, y in placed if at[1] == 0 and 'altar_slab' in b.models]
        b.zfights.extend(zfight.report(bid, blocks.check_placed(placed, b.models)))
    for s in b.structures:
        placed = [(mid, at, y) for _, mid, at, y, _ in s.resolved(b.blocks)]
        b.zfights.extend(zfight.report(s.id, blocks.check_placed(placed, b.models)))


# ---------------------------------------------------------------------------- Blender

class Scenes:
    def __init__(self, b):
        self.b = b
        self.materials = {}
        self.meshes = {}

    def material(self, tex, glow):
        key = (tex, glow)
        if key not in self.materials:
            path = os.path.join(NS, 'textures', 'block', tex + '.png')
            img = bpy.data.images.load(path, check_existing=True)
            img.name = 'block:' + tex
            self.materials[key] = rig.texture_material('block:%s%s' % (tex, ':glow' if glow else ''), img, glow)
        return self.materials[key]

    def mesh(self, mid):
        if mid not in self.meshes:
            self.meshes[mid] = blocks.make_mesh(self.b.models[mid], self.material)
        return self.meshes[mid]

    def scene(self, name):
        sc = bpy.data.scenes.new(name)
        sc.render.fps = rig.FPS
        return sc

    def place(self, sc, name, mid, at, y_rot, drop=0.0):
        ob = bpy.data.objects.new(name, self.mesh(mid))
        sc.collection.objects.link(ob)
        ob.matrix_world = blocks.blender_matrix(at, y_rot, drop)
        return ob


def sprite_object(sc, sprite, name, matrix):
    """An extruded item sprite (the summon item over the pedestal) as a Blender object."""
    from mathutils import Matrix
    path = os.path.join(NS, 'textures', 'item', sprite.id + '.png')
    img = bpy.data.images.load(path, check_existing=True)
    mat = bpy.data.materials.get('item:' + sprite.id) or rig.texture_material('item:' + sprite.id, img)
    verts, faces, uvs = [], [], []
    for corners, uv, n in sprite.quads():
        base = len(verts)
        pts = [tuple(c / 16 for c in p) for p in corners]
        a, b_, c = pts[0], pts[1], pts[2]
        cr = ((b_[1] - a[1]) * (c[2] - a[2]) - (b_[2] - a[2]) * (c[1] - a[1]),
              (b_[2] - a[2]) * (c[0] - a[0]) - (b_[0] - a[0]) * (c[2] - a[2]),
              (b_[0] - a[0]) * (c[1] - a[1]) - (b_[1] - a[1]) * (c[0] - a[0]))
        if sum(cr[i] * n[i] for i in range(3)) < 0:
            pts, uv = pts[::-1], uv[::-1]
        verts.extend(pts)
        faces.append(tuple(range(base, base + 4)))
        uvs.append([(u / sprite.size, 1 - v / sprite.size) for u, v in uv])
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    layer = me.uv_layers.new(name='UVMap')
    for poly, uv in zip(me.polygons, uvs):
        poly.use_smooth = False
        for li, corner in zip(poly.loop_indices, uv):
            layer.data[li].uv = corner
    me.materials.append(mat)
    ob = bpy.data.objects.new(name, me)
    sc.collection.objects.link(ob)
    # item space (px 0..16, y up) is the MC frame: centre it, scale, then into Blender (z up).
    k = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
    ob.matrix_world = k @ matrix @ Matrix.Translation((-0.5, -0.5, -0.5))
    return ob


def pedestal_item_matrix(data, at):
    """The summon item over a pedestal at cell `at` (MC frame, blocks): data/altar.json
    pedestal_item; standing upright, turned 30 degrees so the extrusion shows."""
    import math
    from mathutils import Matrix
    p = data['pedestal_item']
    return (Matrix.Translation((at[0], at[1] + p['height_px'] / 16, at[2])) @ Matrix.Rotation(math.radians(30), 4, 'Y')
            @ Matrix.Diagonal((p['scale'], p['scale'], p['scale'], 1)))


def build_scenes(b):
    s = Scenes(b)
    sprites = {sp.id: sp for sp in b.sprites}
    with open(os.path.join(ROOT, 'data', 'altar.json'), encoding='utf-8') as f:
        altar = json.load(f)
    scenes = {}
    for bid, blk in b.blocks.items():
        if not any(m.elements for m in blk.models().values()):
            continue  # drawn by its block entity (a mob head): nothing to show here
        sc = s.scene(bid)
        sc['soulscraft_set'] = b.set_of[bid]
        for mid, at, y in block_layout(blk):
            s.place(sc, '%s:%s' % (bid, mid), mid, at, y)
        if bid == 'altar_pedestal' and 'rotten_horn' in sprites:
            sprite_object(sc, sprites['rotten_horn'], 'altar_pedestal:rotten_horn', pedestal_item_matrix(altar, (0, 0, 0)))
        scenes[bid] = sc
    for st in b.structures:
        sc = s.scene(st.id)
        sc['soulscraft_set'] = 'structure'
        for blk, mid, at, y, meta in st.resolved(b.blocks):
            s.place(sc, '%s:%s@%d,%d,%d' % ((st.id, mid) + tuple(at)), mid, at, y)
        scenes[st.id] = sc
    exports = {}
    for set_name in sorted(set(b.model_set.values())):
        sc = s.scene('export:blocks_' + set_name)
        for mid in b.models:
            if b.model_set[mid] == set_name and not mid.endswith('_on_slab') and b.models[mid].elements:
                ob = bpy.data.objects.new(mid, s.mesh(mid))
                sc.collection.objects.link(ob)
        exports['blocks_' + set_name] = sc
    for st in b.structures:
        exports[st.id] = scenes[st.id]
    return scenes, exports


def export_glb(scene, name):
    bpy.context.window.scene = scene
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(HERE, name + '.glb'), export_format='GLB', use_active_scene=True,
        use_selection=False, export_apply=False, export_yup=True, export_texcoords=True, export_normals=True,
        export_materials='EXPORT', export_image_format='AUTO', export_animations=False,
        export_skins=False, export_morph=False, export_cameras=False, export_lights=False, export_extras=False)


def showcase_registry(b):
    """docs/showcase/js/blocks.js: blocks, models (glb, bounds, glow) and structures."""
    reg_blocks = []
    for bid, blk in b.blocks.items():
        reg_blocks.append({
            'id': bid, 'name': blk.name, 'role': blk.role, 'set': b.set_of[bid], 'part': list(blk.part) if blk.part else None,
            'states': {k: [v for v in vals] for k, vals in blk.states.items()}, 'facing': blk.facing, 'on_slab': blk.on_slab,
            'item': 'block' if blk.item == 'block' else 'sprite',
            'models': [m for m, mm in blk.models().items() if not m.endswith('_on_slab') and mm.elements],
            'entity': not any(mm.elements for mm in blk.models().values()),
        })
    reg_models = {}
    for mid, m in b.models.items():
        if mid.endswith('_on_slab') or not m.elements:
            continue
        lo, hi = m.bounds()
        reg_models[mid] = {'glb': 'blocks_' + b.model_set[mid], 'bounds': [[blocks.rnd(c) for c in lo], [blocks.rnd(c) for c in hi]],
                           'glow': any(e.glow for e in m.elements)}
    reg_structs = {}
    for st in b.structures:
        placed = []
        for blk, mid, at, y, meta in st.resolved(b.blocks):
            base = mid[:-len('_on_slab')] if mid.endswith('_on_slab') else mid
            e = {'block': blk.id, 'model': base, 'at': list(at), 'rot': y, 'drop': blocks.SLAB_DROP if mid.endswith('_on_slab') else 0}
            e.update(meta)
            placed.append(e)
        reg_structs[st.id] = {'name': st.name, 'placed': placed}
    import base64
    reg_particles = {}
    for pid, mod in b.particles.items():
        spec = mod['PARTICLES'][pid]
        reg_particles[pid] = {'name': spec['name'], 'behaviour': spec['behaviour'],
                              'png': base64.b64encode(mod['particle_image'](spec).png()).decode('ascii')}
    data = {'blocks': reg_blocks, 'models': reg_models, 'structures': reg_structs, 'slab_drop': blocks.SLAB_DROP,
            'particles': reg_particles}
    with open(out(JS, 'blocks.js'), 'w', encoding='utf-8', newline='\n') as f:
        f.write("'use strict';\n// Generated by assets/models/build_blocks.py. Do not edit by hand.\n")
        f.write('SC.blocks(%s);\n' % json.dumps(data, ensure_ascii=False, separators=(',', ':')))


def relative_images():
    for img in bpy.data.images:
        if img.filepath:
            img.filepath = bpy.path.relpath(bpy.path.abspath(img.filepath), start=HERE).replace('\\', '/')


def main(run_embed=True):
    b = collect(load_sets())
    if not b.blocks:
        print('build_blocks.py: no blocks in blocks/*.py')
        return
    write_game_files(b)
    check_zfight(b)
    assert not b.zfights, 'z-fighting in blocks: coplanar overlapping faces (zfight.py):\n' + '\n'.join(b.zfights)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    default = bpy.data.scenes[0]
    for old in glob.glob(os.path.join(HERE, 'blocks_*.glb')) + glob.glob(os.path.join(HERE, 'altar_level_*.glb')):
        os.remove(old)
    scenes, exports = build_scenes(b)
    bpy.data.scenes.remove(default)
    for name, sc in exports.items():
        export_glb(sc, name)
    relative_images()
    bpy.context.window.scene = scenes[b.structures[-1].id] if b.structures else next(iter(scenes.values()))
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, 'blocks.blend'), compress=True, relative_remap=True)
    for stale in glob.glob(os.path.join(HERE, '*.blend1')):
        os.remove(stale)
    showcase_registry(b)
    if run_embed:
        runpy.run_path(os.path.join(HERE, 'embed.py'), run_name='__main__')
    print('build_blocks.py: %d blocks, %d models, %d textures, %d structures' % (len(b.blocks), len(b.models), len(b.textures), len(b.structures)))


if __name__ == '__main__':
    main()
