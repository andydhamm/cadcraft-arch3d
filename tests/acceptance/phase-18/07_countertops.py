#!/usr/bin/env python3
# Countertops you can shape, first half (the brief's detach, typed edge moves, from scratch, cut-outs carry over, and
# the numbers "detaching an automatic counter keeps its area (± 1e-9); moving one edge of length L outward by 0.3 adds
# 0.3 · L"). An island of two 0.9 base cabinets (back on y = 2.0, x 1.55..3.35, counter to y 2.625, top 0.914) with a
# sink in the first: its automatic counter is one countertops row (id cb0, auto) of area 1.125; countertop.detach gives
# ct0 with the same polygon and area 1.125; the sink's cut-out carries over, checked on geometry (a probe there answers
# the sink's furnishing box): ct0's row has one hole, the cut-out rectangle centred on the sink, and the painted ct0's GLB
# node does not cover the hole; moving the front edge (L 1.8) out by 0.3 gives 1.665 and probes find the counter in the
# new overhang; `countertop {polygon, elevation, thickness}` draws one from scratch; delete brings the automatic counter
# back and undo the custom one.
# A stub fails: no countertops rows, an area that changes on detach, no hole, no probe in the overhang.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-countertops
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

ISLAND = cmd('cabinet', {"kind": "base", "at": [2.0, 2.0], "rotation": 0, "widths": [0.9, 0.9]})
SINK = cmd('library.place', {"item": "own:sink-kitchen", "at": [2.0, 2.3]})
DETACH = cmd('countertop.detach', {"run": "cb1"})
tmp = tempfile.mkdtemp()

# 1. The automatic counter, then the detached one.
rr = run_ok(KITCHEN + [ISLAND, SINK, INSPECT18, DETACH, INSPECT18], "an island with a sink, detached")
i0, i1 = inspected(rr, 0), inspected(rr, 1)
auto = countertops_of(i0, True)
if len(auto) != 1 or auto[0].get('id') != 'cb0' or countertops_of(i0, False):
    fail(f"one automatic countertops row owned by cb0 before the detach: {[(r.get('id'), r.get('auto')) for r in countertops_of(i0)]}")
near(auto[0].get('area'), 1.125, 1e-9, "the island's automatic counter area (1.8 x 0.625)")
rep = reply_with(rr, 'countertop', -1, 'countertop.detach')
CT = rep.get('countertop')
if sorted(rep.get('cabinets') or []) != ['cb0', 'cb1']:
    fail(f"the detached countertop claims both cabinets: {rep}")
if countertops_of(i1, True):
    fail(f"no automatic row remains after the detach: {[(r.get('id'), r.get('auto')) for r in countertops_of(i1)]}")
row = ct_row(i1, CT)
near(row.get('area'), 1.125, 1e-9, "the detached countertop keeps its area")
same_vertex_set(row.get('polygon'), [(1.55, 2.0), (3.35, 2.0), (3.35, 2.625), (1.55, 2.625)], 1e-9, "the detached polygon")
near(row.get('top'), 0.914, 1e-9, "the detached top")
print(f"ok: {CT} keeps the 1.125 outline")

# 2. The cut-out carries over: the hole in the row and in the geometry.
f = [x for x in furnishings_of(i1) if 'sink' in str(x.get('item', ''))]
if len(f) != 1:
    fail(f"one placed sink: {[x.get('item') for x in furnishings_of(i1)]}")
at, co = f[0].get('at'), f[0].get('cutout')
if not (isinstance(at, list) and isinstance(co, list) and len(co) == 2):
    fail(f"the sink row carries `at` and `cutout`: {f[0]}")
holes = row.get('holes')
if not isinstance(holes, list) or len(holes) != 1:
    fail(f"ct0 has exactly one cut-out hole: {json.dumps(holes)[:300]}")
w, d = co[0], co[1]
turned = abs(math.sin(math.radians(num(f[0].get('rotation', 0), 'rotation')))) > 0.5
if turned:
    w, d = d, w
want = [(at[0] - w / 2, at[1] - d / 2), (at[0] + w / 2, at[1] - d / 2), (at[0] + w / 2, at[1] + d / 2), (at[0] - w / 2, at[1] + d / 2)]
same_vertex_set(holes[0], want, 1e-9, "the cut-out hole centred on the sink")
PAINT = cmd('paint.material', {"material": "#c04020", "target": f"countertop/{CT}", "scope": "component"})
rr2, (j, binc) = export_glb(KITCHEN + [ISLAND, SINK, DETACH, PAINT], tmp, 'island.glb')
node = f'countertop-{CT}'
if node not in node_names(j):
    fail(f"the painted countertop is GLB node {node}: {node_names(j)[:40]}")
cx, cy = at[0], at[1]
for x, y, inside in ((cx, cy, False), (cx - w / 2 + 0.003, cy, False), (cx + w / 2 - 0.003, cy, False), (cx, cy - d / 2 + 0.003, False),
                     (cx, cy + d / 2 - 0.003, False), (cx - w / 2 - 0.003, cy, True), (cx + w / 2 + 0.003, cy, True),
                     (cx, cy - d / 2 - 0.003, True), (cx, cy + d / 2 + 0.003, True)):
    if covered(j, binc, node, x, y) != inside:
        fail(f"{node} {'covers' if inside else 'does not cover'} ({x:.3f}, {y:.3f}) (the sink's cut-out carries over)")
print("ok: the sink's cut-out is ct0's one hole, open in the GLB")

# 3. The front edge moves out 0.3.
i, e = edge_index(row, lambda mx, my: abs(my - 2.625) < 1e-6, "the front edge")
near(e.get('length'), 1.8, 1e-9, "the front edge's length")
MOVE = cmd('countertop.edit', {"id": CT, "edge": i, "move": 0.3})
rr = run_ok(KITCHEN + [ISLAND, SINK, DETACH, MOVE, INSPECT18, probe_cmd([[2.45, 2.8, 0.9], [2.45, 2.95, 0.9]])], "the front edge moved 0.3")
near(reply_with(rr, 'area', -1, 'countertop.edit').get('area'), 1.665, 1e-6, "the area after a 0.3 move of the 1.8 edge (+0.54)")
near(ct_row(inspected(rr), CT).get('area'), 1.665, 1e-6, "the inspected area")
g = probed(rr, [[2.45, 2.8, 0.9], [2.45, 2.95, 0.9]], what="overhang")
hit(g[0], 'counter', CT, "the counter in the new overhang (y 2.625..2.925, z 0.876..0.914)")
nohit(g[1], "beyond the moved edge at y 2.95")
print("ok: +0.3 x 1.8 = 1.665; the overhang probes")

# 4. From scratch; delete and undo.
NEW = cmd('countertop', {"polygon": [[5, 0.5], [5.6, 0.5], [5.6, 1.5], [5, 1.5]], "elevation": 0.914, "thickness": 0.03})
rr = run_ok(KITCHEN + [NEW, INSPECT18, probe_cmd([[5.3, 1.0, 0.9], [5.3, 1.0, 0.88]])], "a countertop from scratch")
rep = reply_with(rr, 'countertop', -1, 'countertop')
near(rep.get('area'), 0.6, 1e-9, "the drawn countertop's area")
g = probed(rr, [[5.3, 1.0, 0.9], [5.3, 1.0, 0.88]], what="from scratch")
hit(g[0], 'counter', rep.get('countertop'), "inside the 0.03 slab under 0.914")
nohit(g[1], "below the slab's bottom 0.884")
rr = run_ok(KITCHEN + [ISLAND, SINK, DETACH, cmd('countertop.delete', {"id": CT}), INSPECT18, 'undo {}', INSPECT18], "delete and undo")
auto = countertops_of(inspected(rr, 0), True)
if len(auto) != 1 or auto[0].get('id') != 'cb0' or countertops_of(inspected(rr, 0), False):
    fail(f"deleting the countertop brings the automatic counter back: {[(r.get('id'), r.get('auto')) for r in countertops_of(inspected(rr, 0))]}")
near(auto[0].get('area'), 1.125, 1e-9, "the automatic counter's area after the delete")
if not [r for r in countertops_of(inspected(rr, 1), False) if r.get('id') == CT]:
    fail("undo restores the custom countertop")
print("ok: from scratch 0.6; delete restores the automatic counter, undo the custom one")
print("PASS 07")
