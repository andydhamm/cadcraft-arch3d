#!/usr/bin/env python3
# Split levels built properly: a level's elevation is free (Entry, a landing, half a storey between Lower and Main); a wall
# spans from one level's floor to another's ceiling; the front door sits on the landing; half flights go up and down from
# it. Rooms are detected on each level, the 3D build and the exports carry the offset levels and the spanning wall, and the
# incremental rebuild stays the full build when a level moves.
# needs: p15-doc-levels, p15-cmd-levels, p15-arch-spanning-walls, p15-arch3d-spanning-walls, p15-io-split-levels, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
# Lower (index 0, elevation 0), Main at 2.7, Entry (a landing) at 1.35. The front wall w0 (y = 0) spans Lower's floor to
# Main's ceiling (2.7 + 2.4 = 5.1); the other three exterior walls are Lower's, and Main has its own ring on top of them.
HOUSE = [cmd('level', {"name": "Main", "elevation": 2.7}),
         cmd('level', {"name": "Entry", "elevation": 1.35, "kind": "landing"}),
         cmd('wall', {"points": [[0, 0], [10, 0]], "thickness": 0.2, "exterior": True, "level": "Lower", "topLevel": "Main"}),   # w0
         cmd('wall', {"points": [[10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True,
                      "level": "Lower"}),                                                                                      # w1..w3
         cmd('wall', {"points": [[10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True,
                      "level": "Main"}),                                                                                       # w4..w6
         cmd('wall', {"points": [[3, 0], [3, 3], [6, 3], [6, 0]], "thickness": 0.1, "height": 2.4, "level": "Entry"}),       # w7..w9
         cmd('door', {"wall": "w0", "offset": 4.6, "width": 0.9, "level": "Entry"}),                                          # o0
         cmd('stair', {"start": [4.5, 2.9], "direction": 90, "width": 1.0, "fromLevel": "Entry", "toLevel": "Main"}),
         cmd('stair', {"start": [5.5, 0.3], "direction": 90, "width": 1.0, "fromLevel": "Lower", "toLevel": "Entry"}),
         cmd('roomdetect', {"level": "Lower"}), cmd('roomdetect', {"level": "Main"}), cmd('roomdetect', {"level": "Entry"})]
# The first command renames level 0, the default level, to Lower.
HOUSE.insert(0, cmd('level.edit', {"level": 0, "name": "Lower"}))
PROBE = cmd('build3d', {"probe": [[2.0, 0.0, 0.5], [2.0, 0.0, 2.5], [2.0, 0.0, 4.9], [2.0, 0.0, 5.3],    # the spanning wall
                                  [4.6, 0.0, 1.0], [4.6, 0.0, 2.0]]})                                  # below / in the door
rc, rr, so, se = run(HOUSE + [INSPECT15, PROBE])
ins = inspected(rr)
lv = {l['name']: l for l in rows_of(ins, 'levels')}
for name, z, kind in (("Lower", 0.0, "floor"), ("Main", 2.7, "floor"), ("Entry", 1.35, "landing")):
    if name not in lv:
        fail(f"level {name}: {sorted(lv)}")
    near(lv[name]['elevation'], z, 1e-9, f"{name}'s elevation")
    if lv[name].get('kind') != kind:
        fail(f"{name} is a {kind}: {lv[name]}")
w0 = [w for w in rows_of(ins, 'walls') if w['id'] == 'w0'][0]
near(w0['bottom'], 0.0, 1e-9, "w0's bottom")
near(w0['top'], 5.1, 1e-9, "w0's top (Main's ceiling)")
hits = probes(rr)['probe']
if [part(h) for h in hits[:3]] != ['wall', 'wall', 'wall'] or part(hits[3]) == 'wall':
    fail(f"the spanning wall is solid from 0 to 5.1 m: {[part(h) for h in hits[:4]]}")
if part(hits[4]) != 'wall' or part(hits[5]) == 'wall':
    fail(f"the front door opens from the landing (sill 1.35): below {part(hits[4])}, in it {part(hits[5])}")
stairs = rows_of(ins, 'stairs') if 'stairs' in ins else []
rooms = rows_of(ins, 'rooms')
per = {}
for r in rooms:
    per.setdefault(r.get('level'), []).append(r)      # a room row names its level (phase 3: "level": "Level 2")
for name in ('Lower', 'Main', 'Entry'):
    if not per.get(name):
        fail(f"rooms detected on level {name}: {sorted(map(str, per))}")
print("ok: free level elevations (0, 1.35 landing, 2.7), a wall from Lower's floor to Main's ceiling, the door on the landing, rooms on every level")

rc, rr, so, se = run(HOUSE + [cmd('build3d', {}), cmd('build3d', {"digest": True})])
full = [r for r in rr if isinstance(r, dict) and 'digest' in r][-1]['digest']
rc, rr, so, se = run(HOUSE + [cmd('build3d', {}), cmd('level.edit', {"level": "Main", "elevation": 2.8}),
                              cmd('build3d', {"incremental": True, "digest": True}), PROBE.replace('5.3]', '5.15]')])
inc = [r for r in rr if isinstance(r, dict) and 'digest' in r][-1]['digest']
rc2, rr2, so2, se2 = run(HOUSE + [cmd('level.edit', {"level": "Main", "elevation": 2.8}), cmd('build3d', {"digest": True})])
full2 = [r for r in rr2 if isinstance(r, dict) and 'digest' in r][-1]['digest']
if inc != full2:
    fail("after moving Main to 2.8 the incremental build is the full build")
if part(probes(rr)['probe'][3]) != 'wall':
    fail("the spanning wall follows Main's ceiling up to 5.2")
print("ok: moving a level rebuilds the spanning wall; incremental == full")

g, i, o = (os.path.join(tmp, n) for n in ('s.glb', 's.ifc', 's.obj'))
for path in (g, i, o):
    export(HOUSE, path)
validate_glb(g)
_, inst = step_file(i)                               # (header, {n: (ENTITY, args)})
storeys = sorted(float(s[1][9]) for s in of_type(inst, 'IFCBUILDINGSTOREY') if len(s[1]) > 9)
if [round(z, 6) for z in storeys] != [0.0, 1.35, 2.7]:
    fail(f"three IFC storeys at 0, 1.35 and 2.7: {storeys}")
if os.path.getsize(o) < 1000:
    fail("the OBJ export is written")
print("ok: GLB validates; IFC storeys at 0, 1.35, 2.7; OBJ written")
print("PASS 06")
