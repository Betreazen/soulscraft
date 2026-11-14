"""Builds the SoulsCraft mob models from code: Blender scenes, glTF, rig.json, textures, items.

Run from the repository root (Blender 5.2):

    blender --background --factory-startup --python assets/models/build.py

Every file mobs/<id>.py is one model (files starting with '_' are showcase-only figures, such
as the player). A mob file defines:

    ID        the mob id of data/mobs.json ('revenant'), or '_name' for a showcase-only figure
    PALETTES  {material: ['#dark', ..., '#light']}
    TEXTURE   optional (w, h) of the sheet; by default 64x64, or 128x128 if the cubes need it
    model(m)  parts, cubes, sockets and held items (rig.Model)
    clips(c, mob)  clips with keys (rig.Clips); mob is the data/mobs.json entry or None
    ITEMS     optional list of items.Item to write as item models
    SPRITES   optional list of items.Sprite (icons, item/generated; weapons, item/handheld)
    ARMOR     optional list of items.Equipment (vanilla-shape armour textures)

Writes, per model:
    assets/models/<id>.glb                 meshes, texture (NEAREST), every clip as an animation
    assets/rigs/<id>.rig.json              the rig for the game (not for '_' figures)
    assets/textures/entity/<id>.png        the texture ('_' figures: assets/models/textures/)
    assets/soulscraft/models/item/*.json   item models, with their textures in textures/item/
    assets/soulscraft/equipment/*.json     armour sets, textures in textures/entity/equipment/
    assets/soulscraft/textures/mob_effect/*.png  mob effect icons, 18x18 (effects.py)
    assets/models/mobs.blend               all models, one scene each
    assets/site/<id>.glb                   the mob with its held items baked in, for the site
                                           (site_models.py; also copies altar_level_4.glb there)
and then runs embed.py for docs/showcase/js. The build stops on any mismatch between the clips
and the attack ticks of data/mobs.json, and on z-fighting (zfight.py: coplanar overlapping cube
faces in the rest pose or on any tick). It is deterministic: a rerun gives the same bytes.
"""
import glob
import json
import os
import runpy
import sys
import types

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)

import effects  # noqa: E402
import items  # noqa: E402
import paint  # noqa: E402
import rig  # noqa: E402
import site_models  # noqa: E402
import trophies  # noqa: E402
import zfight  # noqa: E402

ASSETS = os.path.join(ROOT, 'assets')
NS = os.path.join(ASSETS, 'soulscraft')


def out(*parts):
    path = os.path.join(*parts)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    return path


def write_text(path, text):
    with open(out(path), 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)


def write_json(path, obj):
    write_text(path, rig.dumps(obj) + '\n')


def load_mobs():
    with open(os.path.join(ROOT, 'data', 'mobs.json'), encoding='utf-8') as f:
        data = json.load(f)
    return {m['id']: m for m in data['mobs']}


def mob_files():
    return sorted(glob.glob(os.path.join(HERE, 'mobs', '*.py')))


def check_against_data(mod_id, clips, mob):
    """Every attack and move of the mob has its clip; attack clips carry exactly the markers of
    their ticks in data/mobs.json, end there, and have a key on every marker. A grab (all ticks
    0) is a looping clip of grab_max_ticks."""
    if mob is None:
        return
    for a in mob['attacks']:
        name = a['clip']
        assert name in clips, '%s: no clip %s for attack %s' % (mod_id, name, a['id'])
        c = clips[name]
        if rig.attack_end(a) == 0:
            # A state such as a grab, not a timed attack: its clip loops, one loop is the longest hold.
            if 'grab_max_ticks' in a:
                assert c.loop and c.length == a['grab_max_ticks'], '%s/%s: a %d-tick loop, data/mobs.json grab_max_ticks says %d' % (
                    mod_id, name, c.length, a['grab_max_ticks'])
            continue
        want = rig.attack_markers(a)
        assert c.markers == want, '%s/%s: markers %s, data/mobs.json says %s' % (mod_id, name, c.markers, want)
        assert c.length == want['end'], '%s/%s: length %d, data/mobs.json says %d' % (mod_id, name, c.length, want['end'])
    for mv in mob.get('moves', []):
        assert mv['clip'] in clips, '%s: no clip %s for move %s' % (mod_id, mv['clip'], mv['id'])


def check_blender_markers(model, clips, mob):
    """Reads the markers back from the Blender actions: what the .blend shows is what was checked."""
    if mob is None:
        return
    for a in mob['attacks']:
        if rig.attack_end(a) == 0:
            continue
        act = bpy.data.actions[rig.obj_name(model, a['clip'])]
        got = {m.name: m.frame for m in act.pose_markers}
        assert got == rig.attack_markers(a), '%s: Blender markers %s' % (a['clip'], got)
        assert tuple(act.frame_range) == (0, rig.attack_end(a))


def build_items(mod, seed):
    """Item models, flat icons and armour sets of a mob file."""
    built = {}
    for it in getattr(mod, 'ITEMS', []):
        it.pack()
        tex = out(NS, 'textures', 'item', it.id + '.png')
        it.paint(seed ^ paint.seed_of(it.id)).save(tex)
        write_json(os.path.join(NS, 'models', 'item', it.id + '.json'), it.json())
        built[it.id] = tex
    for sp in getattr(mod, 'SPRITES', []):
        sp.image().save(out(NS, 'textures', 'item', sp.id + '.png'))
        write_json(os.path.join(NS, 'models', 'item', sp.id + '.json'), sp.json())
    for eq in getattr(mod, 'ARMOR', []):
        eq.paint(seed ^ paint.seed_of(eq.id)).save(out(NS, 'textures', 'entity', 'equipment', eq.layer, eq.id + '.png'))
        write_json(os.path.join(NS, 'equipment', eq.id + '.json'), eq.json())
    return built


ZFIGHTS = []  # coplanar overlapping faces found while building (zfight.py); the build fails on any
TROPHY_SPECS = {}  # mob id -> the mob file's TROPHY (trophies.py), for every mob with a rig


def build_mob(path, mobs):
    mod = runpy.run_path(path)
    mod_id = mod['ID']
    assert os.path.splitext(os.path.basename(path))[0] == mod_id, '%s: ID must match the file name' % path
    showcase_only = mod_id.startswith('_')
    mob = None if showcase_only else mobs.get(mod_id)
    assert showcase_only or mob, '%s is not in data/mobs.json' % mod_id

    m_ = types.SimpleNamespace(**mod)
    seed = paint.seed_of(mod_id)

    model = rig.Model(mod_id, mod['PALETTES'], mod.get('TEXTURE'))
    mod['model'](model)
    cubes = [c for p in model.parts.values() for c in p.cubes]
    sheet = rig.pack(cubes, model.texture)
    img = rig.paint_sheet(model, cubes, sheet, seed)
    if showcase_only:
        tex_path = out(HERE, 'textures', mod_id + '.png')
    else:
        tex_path = out(ASSETS, 'textures', 'entity', mod_id + '.png')
    img.save(tex_path)
    item_tex = build_items(m_, seed)

    clips = rig.Clips()
    mod['clips'](clips, mob)
    check_against_data(mod_id, clips.clips, mob)

    scene = bpy.data.scenes.new(mod_id)
    scene.render.fps = rig.FPS
    scene.frame_start = 0
    scene.frame_end = max([c.length for c in clips.clips.values()] or [0])
    image = bpy.data.images.load(tex_path)
    image.name = mod_id
    objs = rig.build_objects(model, scene, sheet, image)
    for s in model.sockets.values():
        if s.item:
            tex = item_tex.get(s.item.id) or os.path.join(NS, 'textures', 'item', s.item.id + '.png')
            items.preview_object(s.item, objs[s.name], s.hand, tex, rig.obj_name(model, 'held_' + s.item.id))
    rig.key_clips(model, clips.clips, objs)
    check_blender_markers(model, clips.clips, mob)
    samples = rig.sample_clips(model, clips.clips, objs, scene)
    ZFIGHTS.extend(zfight.report(mod_id, zfight.check_model(model, clips.clips, samples)))
    for it in getattr(m_, 'ITEMS', []):
        ZFIGHTS.extend(zfight.report(mod_id + '/' + it.id, zfight.check_item(it)))
    if not showcase_only:
        TROPHY_SPECS[mod_id] = mod.get('TROPHY', {})
        sprite = 'soulscraft:entity/' + mod_id
        write_json(os.path.join(ASSETS, 'rigs', mod_id + '.rig.json'), rig.rig_json(model, clips.clips, samples, sheet, sprite))
    return scene, model, objs


def export_glb(scene, objects, path):
    """NLA_TRACKS bakes every clip; the exporter then drops a baked channel that does not change
    over the clip, even when it differs from the rest pose (a leg held bent through idle), unless
    export_optimize_animation_keep_anim_object is set. With it, such a channel keeps its first and
    last sample, so every part's location, rotation and scale is in every clip."""
    bpy.context.window.scene = scene
    for ob in scene.objects:
        ob.select_set(False)
    for ob in objects:
        ob.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=path, export_format='GLB', use_active_scene=True,
        use_selection=True, export_apply=False, export_yup=True, export_texcoords=True, export_normals=True,
        export_materials='EXPORT', export_image_format='AUTO', export_animations=True,
        export_animation_mode='NLA_TRACKS', export_force_sampling=True, export_frame_step=1,
        export_optimize_animation_size=True, export_optimize_animation_keep_anim_object=True,
        export_skins=False, export_morph=False, export_cameras=False, export_lights=False, export_extras=False)
    for ob in objects:
        ob.select_set(False)


def relative_images():
    """Images are saved with paths relative to the .blend, so the file is the same on any machine."""
    for img in bpy.data.images:
        if img.filepath:
            img.filepath = bpy.path.relpath(bpy.path.abspath(img.filepath), start=HERE).replace('\\', '/')


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    default = bpy.data.scenes[0]
    mobs = load_mobs()
    for old in glob.glob(os.path.join(HERE, '*.glb')):
        os.remove(old)
    built = [build_mob(p, mobs) for p in mob_files()]
    assert not ZFIGHTS, 'z-fighting: coplanar overlapping faces (zfight.py):\n' + '\n'.join(ZFIGHTS)
    # Trophies from the rig.json just written: heads and stuffed mobs, each fitted in one block.
    for old in glob.glob(os.path.join(ASSETS, 'rigs', '*.trophy.json')):
        os.remove(old)
    bad = trophies.build_all(ROOT, HERE, TROPHY_SPECS, mobs)
    assert not bad, 'trophies: out of the block or z-fighting with the base:\n' + '\n'.join(bad[:40])
    effects.write(NS)  # mob effect icons (effects.py)
    bpy.data.scenes.remove(default)
    for scene, model, objs in built:
        export_glb(scene, list(objs.values()), os.path.join(HERE, model.id + '.glb'))
    site_models.export_mobs(built, export_glb)  # assets/site: the mobs with their held items
    relative_images()
    bpy.context.window.scene = built[0][0]
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, 'mobs.blend'), compress=True, relative_remap=True)
    for stale in glob.glob(os.path.join(HERE, '*.blend1')):
        os.remove(stale)
    # Blocks next (blocks/*.py, build_blocks.py): their own blocks.blend, glb and game files.
    runpy.run_path(os.path.join(HERE, 'build_blocks.py'), run_name='build_blocks')['main'](run_embed=False)
    site_models.copy_structures()
    site_sizes = site_models.check()
    runpy.run_path(os.path.join(HERE, 'embed.py'), run_name='__main__')
    print('build.py: %d models: %s' % (len(built), ', '.join(m.id for _, m, _ in built)))
    print('build.py: assets/site: %s' % ', '.join('%s %d' % kv for kv in site_sizes.items()))


main()
