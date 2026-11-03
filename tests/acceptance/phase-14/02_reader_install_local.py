#!/usr/bin/env python3
# The glTF reader: a symbol turns from glTF's Y up to Z up and scales to the size it is given; base-colour textures come
# through to the exported GLB; a mirrored furnishing is its mesh reflected. library.install adds a converted folder as a
# read-only catalog (installing again updates it; an item with no licence is skipped); library.local reads a folder of
# .glb files with one licence, sizes from the mesh bounds and categories and folders from folder names; Draco, meshopt and
# KTX2 files are skipped with their reason, and a local catalog without a licence is refused.
# needs: p14-doc-furnishing, p14-io-gltf-read, p14-io-library-files, p14-cmd-library, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-io-glb-furnishing, p14-cmd-furnishing-transform, p14-inspect, p14-cmd-user-catalog
import os, shutil, struct
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
SAMPLE_DIR = os.path.join(ROOT, 'assets', 'library', 'sample')
by_id = {s['id']: s for s in SAMPLE}

# ---- size, axes, textures, mirror ----
cmds = room_cmds(0, 0, 8, 8) + [cmd('library.place', {"item": "table-dining-150", "at": [2, 2], "size": [2.0, 1.0, 0.8]}),
                                cmd('library.place', {"item": "rug-oriental", "at": [5, 5]}),
                                cmd('library.place', {"item": "armchair", "at": [6, 2]})]
rr, (j, binc) = export_glb(cmds, tmp, 'a.glb')
lo, hi = drawing_box(j, 'furnishing-f0')
for k, (want, c) in enumerate(zip([2.0, 1.0, 0.8], [2.0, 2.0, 0.4])):
    near(hi[k] - lo[k], want, 1e-4, f"table extent along {'xyz'[k]}")
    if k < 2:
        near((hi[k] + lo[k]) / 2, c, 1e-4, f"table centre {'xy'[k]}")
near(lo[2], 0.0, 1e-4, "table bottom")
print("ok: a symbol is Z up at the size it is given (2.0 x 1.0 x 0.8 m at (2, 2))")
rug = by_id['rug-oriental']
lo, hi = drawing_box(j, 'furnishing-f1')
for k in range(3):
    near(hi[k] - lo[k], rug['size'][k], 1e-4, f"rug extent along {'xyz'[k]} (catalog size)")
mats = set()
for m in node_meshes(j, 'furnishing-f1'):
    for prim in m['primitives']:
        mats.add(prim.get('material'))
textured = [k for k in mats if k is not None and 'baseColorTexture' in j['materials'][k].get('pbrMetallicRoughness', {})]
if not textured:
    fail(f"the rug's materials carry no baseColorTexture: {[j['materials'][k] for k in mats if k is not None][:3]}")
print(f"ok: the rug keeps its texture ({len(textured)} textured material(s))")
before = node_positions(j, binc, 'furnishing-f2')
rr, (j2, binc2) = export_glb(cmds + [cmd('furnishing.edit', {"id": "f2", "mirrored": True})], tmp, 'b.glb')
after = node_positions(j2, binc2, 'furnishing-f2')
if geometry_key([(2 * 6.0 - x, y, z) for x, y, z in before]) != geometry_key(after):
    fail("a mirrored armchair is not its mesh reflected across its own x = 6 plane")
print("ok: mirrored is the mesh reflected")

# ---- library.install ----
kit = os.path.join(tmp, 'kit')
os.makedirs(kit)
shutil.copy(os.path.join(SAMPLE_DIR, 'chair-dining-wood.glb'), os.path.join(kit, 'kit-chair.glb'))
shutil.copy(os.path.join(SAMPLE_DIR, 'table-bedside.glb'), os.path.join(kit, 'kit-table.glb'))
base = {k: by_id['chair-dining-wood'][k] for k in ('category', 'symbolType', 'placement', 'size', 'keywords', 'source', 'author')}


def write_kit(name):
    items = [dict(base, id='kit-chair', name=name, licence='CC0-1.0', file='kit-chair.glb', attribution=''),
             dict(base, id='kit-table', name='Unlicensed table', file='kit-table.glb', attribution='')]   # no licence
    json.dump({"version": 1, "items": items}, open(os.path.join(kit, 'catalog.json'), 'w'))


write_kit('Kit chair')
rc, rr, so, se = run([cmd('library.install', {"from": kit}), cmd('library.item', {"id": "kit-chair"})])
inst = reply_with(rr, 'catalog', 0, 'library.install')
if inst.get('items') != 1 or not any(s.get('file') == 'kit-table.glb' or 'kit-table' in json.dumps(s) for s in inst.get('skipped', [])):
    fail(f"install: expected 1 item and kit-table skipped (no licence): {inst}")
rec = reply_with(rr, 'item', -1, 'library.item')['item']
if rec.get('name') != 'Kit chair' or rec.get('readOnly') is not True or not str(rec.get('catalog', '')).startswith('installed'):
    fail(f"the installed item: {rec}")
rc, rr, so, se = run([cmd('library.edit', {"item": "kit-chair", "name": "Mine now"})], expect_ok=False)
if not errors(se):
    fail("library.edit of an installed (read-only) item was not refused")
write_kit('Kit chair, v2')
rc, rr, so, se = run([cmd('library.install', {"from": kit}), cmd('library.item', {"id": "kit-chair"}),
                      cmd('library.place', {"item": "kit-chair", "at": [1, 1]})])
rec = reply_with(rr, 'item', -1, 'library.item')['item']
if rec.get('name') != 'Kit chair, v2':
    fail(f"installing again did not update the catalog: {rec.get('name')!r}")
print("ok: library.install adds a read-only catalog, skips the unlicensed item and updates when installed again")

# ---- library.local ----
loc = os.path.join(tmp, 'simhome')
os.makedirs(os.path.join(loc, 'Furniture', 'Chairs'))
os.makedirs(os.path.join(loc, 'Lamps'))
os.makedirs(os.path.join(loc, 'odd'))
shutil.copy(os.path.join(SAMPLE_DIR, 'armchair.glb'), os.path.join(loc, 'Furniture', 'Chairs', 'Club chair.glb'))
shutil.copy(os.path.join(SAMPLE_DIR, 'lamp-floor.glb'), os.path.join(loc, 'Lamps', 'Tall lamp.glb'))


def glb_bytes(js):
    body = json.dumps(js).encode()
    body += b' ' * (-len(body) % 4)
    return b'glTF' + struct.pack('<II', 2, 12 + 8 + len(body)) + struct.pack('<I', len(body)) + b'JSON' + body


open(os.path.join(loc, 'odd', 'draco.glb'), 'wb').write(glb_bytes({
    "asset": {"version": "2.0"}, "extensionsUsed": ["KHR_draco_mesh_compression"],
    "extensionsRequired": ["KHR_draco_mesh_compression"], "meshes": [], "nodes": [], "scenes": [{"nodes": []}]}))
open(os.path.join(loc, 'odd', 'ktx2.glb'), 'wb').write(glb_bytes({
    "asset": {"version": "2.0"}, "extensionsUsed": ["KHR_texture_basisu"], "extensionsRequired": ["KHR_texture_basisu"],
    "meshes": [], "nodes": [], "scenes": [{"nodes": []}]}))
rc, rr, so, se = run([cmd('library.local', {"path": loc, "licence": "CC-BY-4.0"}),
                      cmd('library', {"chips": [{"kind": "source", "value": "local"}]})])
lr = reply_with(rr, 'catalog', 0, 'library.local')
skipped = {os.path.basename(str(s.get('file'))): str(s.get('reason')) for s in lr.get('skipped', [])}
if lr.get('items') != 2 or set(skipped) != {'draco.glb', 'ktx2.glb'}:
    fail(f"library.local: expected 2 items and draco.glb, ktx2.glb skipped: {lr}")
if 'draco' not in skipped['draco.glb'].lower() or ('ktx2' not in skipped['ktx2.glb'].lower() and 'basisu' not in skipped['ktx2.glb'].lower()):
    fail(f"skip reasons do not name the extension: {skipped}")
loc_items = {i['name']: i for i in lib_reply(rr)['items']}
chair, lamp = loc_items.get('Club chair'), loc_items.get('Tall lamp')
if not chair or not lamp:
    fail(f"local items by name: {sorted(loc_items)}")
if chair.get('category') != 'Furniture' or chair.get('folder') != 'Chairs' or chair.get('licence') != 'CC-BY-4.0':
    fail(f"the local chair: category {chair.get('category')!r}, folder {chair.get('folder')!r}, licence {chair.get('licence')!r}")
if lamp.get('folder') != 'Lamps':
    fail(f"the local lamp's folder: {lamp.get('folder')!r}")
for k in range(3):
    near(chair['size'][k], by_id['armchair']['size'][k], 1e-3, f"local chair size {'wdh'[k]} from the mesh bounds")
rc, rr, so, se = run([cmd('library.local', {"path": loc})], expect_ok=False)
if not errors(se):
    fail("library.local without a licence was not refused")
print("ok: library.local reads .glb items with one licence, sizes from the bounds, folders as categories; Draco/KTX2 skipped")
print("PASS 02")
