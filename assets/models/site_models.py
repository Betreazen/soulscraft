"""Self-contained models for the project site (site.toml in the repository root): assets/site/.

The models of assets/models/<mob>.glb carry no held items: the showcase hangs them on the hand
sockets at run time (viewer.js Actor.hold). The site shows a glb as it is, so build.py writes a
second copy of every mob here with its items baked in:

    assets/site/<mob>.glb        the mob as in assets/models, plus the items of its rig.json
                                 sockets (m.hold): the Blender preview objects that build.py
                                 already placed under the socket Empties by the game chain
                                 (items.preview_object, K^T @ held_matrix, the same matrix the
                                 showcase uses). Every clip, node animation, no skin. Parts shown
                                 only under a mob state flag (the brute's rage eyes) are left out,
                                 as the showcase hides them while no flag is on.
    assets/site/<structure>.glb  a copy of assets/models/<structure>.glb (altar_level_4)

Mobs are exported by export_mobs() while build.py still has their scenes, structures are copied
by copy_structures() after build_blocks.py wrote them (the module is not called site.py: that
name is taken by the standard library). check() then holds every file to the
site's limits (SIZE_LIMIT each, TOTAL_LIMIT together).
"""
import glob
import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
SITE = os.path.join(ROOT, 'assets', 'site')
STRUCTURES = ('altar_level_4',)
SIZE_LIMIT = 4 * 1024 * 1024    # bytes per model, site.toml
TOTAL_LIMIT = 16 * 1024 * 1024  # bytes for every model of site.toml together


def clean():
    os.makedirs(SITE, exist_ok=True)
    for old in glob.glob(os.path.join(SITE, '*.glb')):
        os.remove(old)


def shown_by_default(ob):
    """False for a part (or a child of a part) that shows only while a mob state flag is on."""
    while ob is not None:
        state = ob.get('soulscraft_state')
        if state and not state.startswith('!'):
            return False
        ob = ob.parent
    return True


def site_objects(objs):
    """The parts and sockets of build_objects plus the held item previews under the sockets."""
    chosen = [ob for ob in objs.values() if shown_by_default(ob)]
    held = [ch for ob in chosen for ch in ob.children if ch.get('soulscraft_preview')]
    return chosen + held


def export_mobs(built, export_glb):
    """built: [(scene, model, objs)] of build.py; export_glb(scene, objects, path) writes one glb."""
    clean()
    for scene, model, objs in built:
        if model.id.startswith('_'):
            continue
        export_glb(scene, site_objects(objs), os.path.join(SITE, model.id + '.glb'))


def copy_structures():
    for name in STRUCTURES:
        shutil.copyfile(os.path.join(HERE, name + '.glb'), os.path.join(SITE, name + '.glb'))


def check():
    files = sorted(glob.glob(os.path.join(SITE, '*.glb')))
    sizes = {os.path.basename(p): os.path.getsize(p) for p in files}
    big = ['%s %d bytes' % (n, s) for n, s in sizes.items() if s > SIZE_LIMIT]
    assert not big, 'site models over %d bytes: %s' % (SIZE_LIMIT, ', '.join(big))
    total = sum(sizes.values())
    assert total <= TOTAL_LIMIT, 'site models: %d bytes together, the limit is %d' % (total, TOTAL_LIMIT)
    return sizes
