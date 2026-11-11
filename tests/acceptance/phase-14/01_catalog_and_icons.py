#!/usr/bin/env python3
# The library opens on nine categories, each with an icon; every sample item is listed with its licence and source and
# loads (each 3D symbol places and builds); the existing catalogues are items (doors and windows, plants, light fixtures,
# finishes, cabinet and fixture kinds, front styles); two icon renders of one item are byte-identical PNGs, 128 x 128.
# needs: p14-doc-furnishing, p14-arch-library, p14-io-gltf-read, p14-io-library-files, p14-cmd-library, p14-apps-library-hooks, p14-arch3d-icon, p14-cmd-icons, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-cmd-user-catalog, p18-room-finishes
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

# A user item first, so the User catalog has one (it starts empty).
rc, rr, so, se = run(ROOM + [cmd('library.place', {"item": "chair-dining-wood", "at": [1.0, 1.0]}),
                             cmd('library.add', {"objects": ["f0"], "name": "One chair"}),
                             'library {}'])
lib = lib_reply(rr)
cats = {c.get('name'): c for c in lib['categories'] if isinstance(c, dict)}
if [c.get('name') for c in lib['categories']] != CATEGORIES:
    fail(f"categories {[c.get('name') for c in lib['categories']]}, expected {CATEGORIES} in that order")
for name in CATEGORIES:
    c = cats[name]
    if not isinstance(c.get('items'), int) or c['items'] < 1:
        fail(f"category {name} has {c.get('items')!r} items")
    if not isinstance(c.get('icon'), str) or not c['icon']:
        fail(f"category {name} has no icon item: {c}")
print(f"ok: nine categories: {[(n, cats[n]['items']) for n in CATEGORIES]}")

items = {i.get('id'): i for i in lib['items'] if isinstance(i, dict)}
missing = sorted(SAMPLE_IDS - set(items))
if missing:
    fail(f"sample items missing from the library: {missing}")
for s in SAMPLE:
    it = items[s['id']]
    for k in ('name', 'category', 'licence', 'source', 'catalog'):
        if not it.get(k):
            fail(f"{s['id']}: no {k} in the library record {it}")
    if it['licence'] != s['licence'] or it['category'] != s['category']:
        fail(f"{s['id']}: licence/category {it['licence']}/{it['category']}, catalog.json says {s['licence']}/{s['category']}")
    if it['catalog'] != 'sample':
        fail(f"{s['id']}: catalog {it['catalog']!r}, expected 'sample'")
if not any(i.get('catalog') == 'user' and i.get('category') == 'User catalog' for i in items.values()):
    fail("the user item made by library.add is not in the User catalog")
print(f"ok: {len(SAMPLE)} sample items, each with its licence and source")

kinds = {}
for i in items.values():
    kinds.setdefault(i.get('kind'), []).append(i)
for k, cat in (('door', 'Doors and windows'), ('window', 'Doors and windows'), ('plant', 'Plants'), ('lightFixture', 'Lighting and electrical'),
               ('finish', 'Materials'), ('cabinet', 'Cabinet parts'), ('fixture', 'Appliances and plumbing fixtures'), ('frontStyle', 'Cabinet parts')):
    ks = kinds.get(k, [])
    if not ks or any(i.get('category') != cat for i in ks):
        fail(f"existing catalogue items of kind {k} missing or outside {cat}: {[(i.get('id'), i.get('category')) for i in ks][:6]}")
finishes = sorted(i['name'].lower() for i in kinds['finish'])
# Phase 18 adds concrete, tile, carpet, drywall and plaster (room-type finishes): one item per finish, exactly these.
if finishes != ['brick', 'carpet', 'concrete', 'drywall', 'plaster', 'shingles', 'siding', 'tile', 'wood']:
    fail(f"the nine finishes as library items: {finishes}")
styles = sorted(i['id'] for i in kinds['frontStyle'])
for want in ('frontstyle:door-slab', 'frontstyle:door-shaker', 'frontstyle:door-raised-panel',
             'frontstyle:drawer-slab', 'frontstyle:drawer-shaker', 'frontstyle:drawer-raised-panel'):
    if want not in styles:
        fail(f"front style {want} not in the library: {styles}")
print(f"ok: existing catalogues are items ({', '.join(f'{k} {len(v)}' for k, v in sorted(kinds.items(), key=lambda kv: str(kv[0])))})")

# Every 3D symbol of the sample set loads: placed side by side in a big room, each builds as its own furnishing node.
symbols = [s for s in SAMPLE if s['file'].endswith('.glb') and s['symbolType'] != 'pull']   # pulls go on fronts (06)
big = room_cmds(0, 0, 30, 30, 3.0)
place, x, y = [], 1.5, 1.5
for s in symbols:
    at = [x, y]
    if s['placement'] == 'wall':
        at = [x, 0.2]
    place.append(cmd('library.place', {"item": s['id'], "at": at}))
    x += 2.5
    if x > 28:
        x, y = 1.5, y + 3.0
tmp = tempfile.mkdtemp()
rr, (j, binc) = export_glb(big + place, tmp, 'all.glb')
placed = [r for r in rr if isinstance(r, dict) and isinstance(r.get('furnishing'), str)]
if len(placed) != len(symbols):
    fail(f"{len(placed)} of {len(symbols)} symbols placed; replies: {json.dumps(rr)[-600:]}")
names = set(node_names(j))
for r in placed:
    if f"furnishing-{r['furnishing']}" not in names:
        fail(f"no GLB node furnishing-{r['furnishing']}")
print(f"ok: all {len(symbols)} sample symbols place and build (furnishing-f0..f{len(symbols) - 1})")

# Icons: 128 x 128 PNGs, byte-identical twice (no cache), one per category.
ic = [cmd('library.icon', {"item": i, "cache": False}) for i in ('sofa-chaise', 'sofa-chaise', 'material-oak', 'own:backdrop-sky', 'finish:brick')]
ic += [cmd('library.icon', {"category": c, "cache": False}) for c in CATEGORIES]
rc, rr, so, se = run(ic)
pngs = [r for r in rr if isinstance(r, dict) and 'png' in r]
if len(pngs) != len(ic):
    fail(f"{len(pngs)} icons for {len(ic)} requests: {se[-400:]}")
blobs = []
for k in range(len(ic)):
    data, r = icon_png(rr, k)
    p = os.path.join(tmp, f'icon{k}.png')
    open(p, 'wb').write(data)
    w, h, nch, rows = png(p)
    if (w, h) != (128, 128):
        fail(f"icon {k} is {w} x {h}, not 128 x 128")
    if k < 5 and len({bytes(r2) for r2 in rows}) < 8:
        fail(f"icon {k} is (nearly) blank")
    blobs.append(data)
if blobs[0] != blobs[1]:
    fail("two renders of the sofa's icon differ")
if len({hashlib.sha256(b).hexdigest() for b in blobs[1:5]}) != 4:
    fail("the sofa, oak, sky and brick icons are not four different images")
for k, name in ((0, 'icon-sofa.png'), (2, 'icon-oak.png'), (3, 'icon-sky.png')):
    open(os.path.join(tmp, name), 'wb').write(blobs[k])
    artifact(os.path.join(tmp, name), name)
print("ok: icons are 128 x 128 PNGs, byte-identical twice, and every category has one")
print("PASS 01")
