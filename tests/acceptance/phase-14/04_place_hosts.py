#!/usr/bin/env python3
# Placing: a floor item placed at (2,2) on a level has its footprint centred there and its bottom on that level's floor;
# a wall item clicked 0.2 m from a wall face ends with its back on that face, at its catalog elevation; a ceiling item in
# a 2.4 m room has its top at 2.4. A furnishing is a probeable solid on its category's layer; size, elevation and rotation
# edit in place; move, multiple copy at an interval, rotate and mirror work on its handle as on other objects, and
# furnishing.centre centres it in its room. Each is one undo step.
# needs: p14-doc-furnishing, p14-io-gltf-read, p14-io-library-files, p14-cmd-library, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-io-glb-furnishing, p14-cmd-furnishing-transform, p14-inspect, p14-arch-furnishing-plan
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
by_id = {s['id']: s for s in SAMPLE}
# ROOM: the 4 x 4 m room r0 (inner faces x, y = 0 and 4; w0 along y = -0.1, inner face y = 0), 2.4 m high; a second level.
TWO = ROOM + [cmd('level', {"above": 0}),
              cmd('wall', {"points": [[-0.1, -0.1], [4.1, -0.1], [4.1, 4.1], [-0.1, 4.1], [-0.1, -0.1]], "thickness": 0.2,
                           "level": "Level 2", "exterior": True}),
              cmd('roomdetect', {"level": 1})]
PLACE = [cmd('library.place', {"item": "table-dining-150", "at": [2, 2], "level": "Level 2"}),        # f0
         cmd('library.place', {"item": "armchair", "at": [1, 1]}),                                     # f1
         cmd('library.place', {"item": "towel-rack", "at": [2, 0.2]}),                                 # f2
         cmd('library.place', {"item": "light-pendant", "at": [3, 3]}),                                # f3
         cmd('library.place', {"item": "toilet", "at": [3.5, 1]}),                                     # f4
         cmd('library.place', {"item": "refrigerator-small", "at": [0.5, 3.5]}),                       # f5
         cmd('library.place', {"item": "lamp-floor", "at": [3.5, 3.5]})]                               # f6
rr, (j, binc) = export_glb(TWO + PLACE + [INSPECT14, cmd('build3d', {"probe": [[1.0, 1.0, 0.3], [2.0, 2.0, 0.3]]})], tmp)
insp = inspected(rr)
levels = {l['name']: l for l in objects_named(insp, 'levels', 'levels')}
z2 = num(levels['Level 2']['elevation'], 'Level 2 elevation')
fs = {f['id']: f for f in furnishings_of(insp)}
if sorted(fs) != [f'f{k}' for k in range(7)]:
    fail(f"furnishings {sorted(fs)}")

t = fs['f0']
near(t['at'][0], 2.0, 1e-6, "table at x"); near(t['at'][1], 2.0, 1e-6, "table at y")
lo, hi = drawing_box(j, 'furnishing-f0')
near((lo[0] + hi[0]) / 2, 2.0, 1e-4, "table footprint centre x"); near((lo[1] + hi[1]) / 2, 2.0, 1e-4, "table footprint centre y")
near(lo[2], z2, 1e-4, "table bottom on Level 2's floor")
near(hi[2] - lo[2], by_id['table-dining-150']['size'][2], 1e-4, "table height")
lo, hi = drawing_box(j, 'furnishing-f1')
near(lo[2], 0.0, 1e-4, "armchair bottom on Level 1's floor")
print(f"ok: floor items centred at the click, bottoms on their level's floor (Level 2 at {z2})")

w = fs['f2']; rack = by_id['towel-rack']
if not isinstance(w.get('host'), dict) or w['host'].get('kind') != 'wall' or w['host'].get('wall') != 'w0':
    fail(f"the towel rack's host: {w.get('host')}")
lo, hi = drawing_box(j, 'furnishing-f2')
near(lo[1], 0.0, 1e-4, "the towel rack's back on w0's inner face (y = 0)")
near(hi[1] - lo[1], rack['size'][1], 1e-4, "the towel rack's depth into the room")
near(lo[2], rack['elevation'], 1e-4, "the towel rack at its catalog elevation")
near(num(w['at'][1], 'rack at y') - rack['size'][1] / 2, 0.0, 1e-6, "rack back from the record")
print("ok: a wall item clicked 0.2 m off the face has its back on it")

c = fs['f3']
near(num(c['elevation'], 'pendant elevation') + num(c['size'][2], 'pendant height'), 2.4, 1e-6, "pendant top")
lo, hi = drawing_box(j, 'furnishing-f3')
near(hi[2], 2.4, 1e-4, "pendant top at the 2.4 m ceiling (GLB)")
print("ok: a ceiling item hangs with its top at the ceiling")

layers = {k: fs[k].get('layer') for k in fs}
want = {'f0': 'I-FURN', 'f1': 'I-FURN', 'f4': 'P-FLOR-FIXT', 'f5': 'A-FLOR-APPL', 'f6': 'E-LITE'}
for k, v in want.items():
    if layers[k] != v:
        fail(f"{k} ({fs[k]['category']}, {fs[k]['symbolType']}) on layer {layers[k]!r}, expected {v}")
pr = probes(rr)['probe']
if part(pr[0]) != 'furnishing' or pr[0].get('id') != 'f1':
    fail(f"a probe inside the armchair: {pr[0]}")
if pr[1] is not None and part(pr[1]) == 'furnishing':
    fail(f"a probe at (2, 2, 0.3) on Level 1 is inside a furnishing: {pr[1]} (the table is on Level 2)")
print(f"ok: furnishings are probeable solids on their layers {sorted(set(layers.values()))}")

# Edits in place, then the generic transforms on its handle.
h1 = reply_with(rr, 'furnishing', 1)['handle']
EDIT = [cmd('furnishing.edit', {"id": "f1", "size": [0.8, 0.7, 1.0], "elevation": 0.1, "rotation": 90}),
        cmd('move', {"handles": [h1], "delta": [0.5, 0.0]}),
        cmd('copy', {"handles": [h1], "delta": [0.0, 0.6], "count": 3}),
        cmd('rotate', {"handles": [h1], "base": [1.5, 1.0], "angle": 90}),
        cmd('mirror', {"handles": [h1], "p1": [0, 2], "p2": [4, 2]}),
        INSPECT14]
rc, rr, so, se = run(TWO + PLACE + EDIT)
fs = {f['id']: f for f in furnishings_of(inspected(rr))}
f1 = fs['f1']
for k, want in enumerate([0.8, 0.7, 1.0]):
    near(f1['size'][k], want, 1e-9, f"edited size {'wdh'[k]}")
near(f1['elevation'], 0.1, 1e-9, "edited elevation")
near(f1['at'][0], 1.5, 1e-6, "f1 x after move, rotate about its own centre")
near(f1['at'][1], 3.0, 1e-6, "f1 y after mirroring across y = 2")
copies = sorted((k for k in fs if fs[k]['item'] == 'armchair' and k != 'f1'), key=lambda k: fs[k]['at'][1])
if len(copies) != 3:
    fail(f"copy count 3 made {len(copies)} armchairs: {copies}")
for n, k in enumerate(copies):
    near(fs[k]['at'][0], 1.5, 1e-6, f"copy {n + 1} x"); near(fs[k]['at'][1], 1.0 + 0.6 * (n + 1), 1e-6, f"copy {n + 1} y")
if fs['f1'].get('mirrored') is not True:
    fail(f"mirror did not mark f1 mirrored: {fs['f1']}")
print("ok: edits in place; move, copy x3 at 0.6 m, rotate and mirror on a furnishing handle")

rc, rr, so, se = run(TWO + PLACE + [cmd('furnishing.centre', {"id": "f1", "on": "room"}), INSPECT14, 'undo {}', INSPECT14])
a, b = furnishings_of(inspected(rr, -2)), furnishings_of(inspected(rr, -1))
fa = {f['id']: f for f in a}['f1']; fb = {f['id']: f for f in b}['f1']
near(fa['at'][0], 2.0, 1e-6, "centred x"); near(fa['at'][1], 2.0, 1e-6, "centred y")
near(fb['at'][0], 1.0, 1e-6, "undo restores x")
print("ok: furnishing.centre centres in the room; undo restores")
print("PASS 04")
