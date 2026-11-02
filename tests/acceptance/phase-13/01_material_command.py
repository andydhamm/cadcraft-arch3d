# Materials: the `material` command sets a wall's exterior finish, a roof's covering and a room's floor finish, refuses
# what a target cannot take (and unknown ids) without writing anything, undoes, and the finishes survive a DXF round trip.
# needs: p13-doc-finish
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp(); saved = os.path.join(tmp, 'finished.dxf')
cmds = ROOM + [cmd('roofauto', {"pitchRise": 6, "pitchRun": 12, "overhang": 0.3}),
               cmd('material', {"walls": "exterior", "finish": "siding"}),
               cmd('material', {"walls": ["w0"], "finish": "brick"}),
               cmd('material', {"roofs": "all", "finish": "shingles"}),
               cmd('material', {"rooms": ["r0"], "finish": "wood"}),
               INSPECT_ALL]
rc, rr, so, se = run(cmds, save=saved)
mats = [r for r in rr if isinstance(r, dict) and 'target' in r and 'ids' in r]
if [m.get('target') for m in mats] != ['walls', 'walls', 'roofs', 'rooms']:
    fail(f"material replies name their target (walls, walls, roofs, rooms): {mats}")
if sorted(mats[0].get('ids') or []) != ['w0', 'w1', 'w2', 'w3'] or mats[1].get('ids') != ['w0'] or mats[1].get('finish') != 'brick':
    fail(f"'exterior' finishes the four ring walls, then w0 alone is brick: {mats[:2]}")
ins = inspected(rr)
walls = {w['id']: w for w in objects_named(ins, 'walls', 'walls')}
want = {'w0': 'brick', 'w1': 'siding', 'w2': 'siding', 'w3': 'siding'}
got = {k: walls[k].get('finish') for k in want if k in walls}
if got != want:
    fail(f"arch.inspect wall finishes: expected {want}, got {got}")
roofs = objects_named(ins, 'roofs', 'roofs')
if not roofs or any(r.get('covering') != 'shingles' for r in roofs):
    fail(f"every roof is covered with shingles: {[(r.get('id'), r.get('covering')) for r in roofs]}")
rooms = objects_named(ins, 'rooms', 'rooms')
if by_id(rooms, 'r0', 'rooms').get('floorFinish') != 'wood':
    fail(f"room r0's floor is wood: {[(r.get('id'), r.get('floorFinish')) for r in rooms]}")
print("ok: walls siding with w0 brick, the roof shingles, r0's floor wood")

# Refusals: nothing is written.
bad = [({"roofs": "all", "finish": "brick"}, "brick on a roof"),
       ({"rooms": "all", "finish": "shingles"}, "shingles on a floor"),
       ({"walls": ["w0"], "finish": "marble"}, "an unknown finish"),
       ({"walls": ["w0", "w99"], "finish": "wood"}, "an unknown wall (with a known one)"),
       ({"walls": ["w0"], "roofs": "all", "finish": "wood"}, "two targets"),
       ({"finish": "wood"}, "no target"),
       ({"walls": [], "finish": "wood"}, "an empty list")]
for params, what in bad:
    rc, rr2, so2, se2 = run(cmds + [cmd('material', params), INSPECT_ALL], expect_ok=False)
    if not was_refused(rc, se2):
        fail(f"material refuses {what}: {params}")
    w2 = {w['id']: w.get('finish') for w in objects_named(inspected(rr2), 'walls', 'walls')}
    if w2 != {'w0': 'brick', 'w1': 'siding', 'w2': 'siding', 'w3': 'siding'}:
        fail(f"a refused material ({what}) changed nothing: {w2}")
print(f"ok: {len(bad)} refusals write nothing")

# none clears; undo restores.
rc, rr3, so3, se3 = run(cmds + [cmd('material', {"walls": ["w1"], "finish": "none"}), INSPECT_ALL, 'undo {}', INSPECT_ALL])
a, b = inspected(rr3, -2), inspected(rr3, -1)
if by_id(a['walls'], 'w1', 'walls').get('finish') is not None:
    fail("finish none clears w1's finish")
if by_id(b['walls'], 'w1', 'walls').get('finish') != 'siding':
    fail("undo after material restores w1's siding")
print("ok: none clears, undo restores")

# The DXF round trip keeps them.
rc, rr4, so4, se4 = run([INSPECT_ALL], file=saved)
ins4 = inspected(rr4)
got4 = {w['id']: w.get('finish') for w in ins4['walls']}
if got4 != want or any(r.get('covering') != 'shingles' for r in objects_named(ins4, 'roofs', 'roofs')) \
        or by_id(objects_named(ins4, 'rooms', 'rooms'), 'r0', 'rooms').get('floorFinish') != 'wood':
    fail(f"the finishes survive save and open: walls {got4}")
print("ok: finishes survive the DXF round trip")
print("PASS 01")
