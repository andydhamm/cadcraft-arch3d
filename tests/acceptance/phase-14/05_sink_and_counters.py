#!/usr/bin/env python3
# Counter hosts: a sink clicked over a base cabinet goes into it, leaving a counter hole of the sink's cut-out size and a
# false top front, and stays its own selectable furnishing; a faucet clicked on the sink snaps to the sink's back centre;
# a dishwasher clicked in a 0.6 m gap between two base cabinets of its depth gets the counter carried over it (one
# continuous counter over all three); a range (taller than the space under a counter) in a gap does not; neighbouring
# base cabinets share one counter.
# needs: p14-doc-furnishing, p14-doc-cabinet-parts, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-arch3d-cabinet-parts, p14-io-glb-furnishing, p14-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
by_id = {s['id']: s for s in SAMPLE}


def covered(j, binc, name, x, y):
    """Whether a node's triangles, seen from above, cover (x, y) (positions are in drawing axes, see drawing_box)."""
    for m in node_meshes(j, name):
        for prim in m['primitives']:
            pos = [(p[0], p[1]) for p in _acc(j, binc, prim['attributes']['POSITION'])]
            idx = [t[0] for t in _acc(j, binc, prim['indices'])] if 'indices' in prim else list(range(len(pos)))
            for t in range(0, len(idx) - 2, 3):
                (ax, ay), (bx, by), (cx, cy) = pos[idx[t]], pos[idx[t + 1]], pos[idx[t + 2]]
                d1 = (x - bx) * (ay - by) - (ax - bx) * (y - by)
                d2 = (x - cx) * (by - cy) - (bx - cx) * (y - cy)
                d3 = (x - ax) * (cy - ay) - (cx - ax) * (y - ay)
                area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
                if abs(area) < 1e-12:
                    continue
                if (d1 >= 0 and d2 >= 0 and d3 >= 0) or (d1 <= 0 and d2 <= 0 and d3 <= 0):
                    return True
    return False


# ---- the sink: ROOM's w0 has its inner face on y = 0; a 0.9 m base cabinet centred at x = 0.9 (x 0.45..1.35, back on
# y = 0, 0.6 deep, counter 0.876..0.914 to y = 0.625).
SINK = ROOM + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.9, "fronts": "drawerOverDoor"}),
               cmd('library.place', {"item": "own:sink-kitchen", "at": [0.95, 0.35]}),
               cmd('library.place', {"item": "faucet-kitchen-chrome", "at": [0.9, 0.3]}),
               INSPECT14]
rr, (j, binc) = export_glb(SINK, tmp, 'sink.glb')
insp = inspected(rr)
fs = {f['id']: f for f in furnishings_of(insp)}
cab = {c['id']: c for c in objects_named(insp, 'cabinets', 'cabinets')}['cb0']
sink, faucet = fs.get('f0'), fs.get('f1')
if not sink or (sink.get('host') or {}).get('kind') != 'cabinet' or sink['host'].get('cabinet') != 'cb0':
    fail(f"the sink's host: {sink and sink.get('host')}")
near(sink['at'][0], 0.9, 1e-6, "sink centred on the cabinet (x)")
near(sink['at'][1], 0.3, 1e-6, "sink centred on the cabinet's depth (y)")
cw, cd = by_id['own:sink-kitchen']['cutout']
if cab.get('falseFront') is not True:
    fail(f"the cabinet under the sink has no false front: {cab}")
if [round(v, 6) for v in (cab.get('cutout') or [])] != [round(cw, 6), round(cd, 6)]:
    fail(f"the cabinet reports cut-out {cab.get('cutout')}, the sink's is {[cw, cd]}")
node = 'cabinet-cb0-counter'
e = 0.003
inside = [(0.9, 0.3), (0.9 - cw / 2 + e, 0.3), (0.9 + cw / 2 - e, 0.3), (0.9, 0.3 - cd / 2 + e), (0.9, 0.3 + cd / 2 - e)]
outside = [(0.9 - cw / 2 - e, 0.3), (0.9 + cw / 2 + e, 0.3), (0.9, 0.3 - cd / 2 - e), (0.9, 0.3 + cd / 2 + e), (0.5, 0.6)]
for x, y in inside:
    if covered(j, binc, node, x, y):
        fail(f"the counter covers ({x:.3f}, {y:.3f}), inside the {cw} x {cd} cut-out")
for x, y in outside:
    if not covered(j, binc, node, x, y):
        fail(f"the counter does not cover ({x:.3f}, {y:.3f}), outside the cut-out")
print(f"ok: the sink drops into cb0: a {cw} x {cd} counter hole and a false front")
if not faucet or (faucet.get('host') or {}).get('kind') != 'sink' or faucet['host'].get('furnishing') != 'f0':
    fail(f"the faucet's host: {faucet and faucet.get('host')}")
back = 0.3 - by_id['own:sink-kitchen']['size'][1] / 2
near(faucet['at'][0], 0.9, 1e-6, "faucet on the sink's centre line")
near(faucet['at'][1] - faucet['size'][1] / 2, back, 1e-6, "faucet's back on the sink's back edge")
near(faucet['elevation'], sink['elevation'] + sink['size'][2], 1e-6, "faucet on the sink's top")
print("ok: the faucet snaps to the sink's back centre")
rc, rr, so, se = run(SINK + [cmd('furnishing.delete', {"id": "f0"}), INSPECT14])
cab2 = {c['id']: c for c in objects_named(inspected(rr), 'cabinets', 'cabinets')}['cb0']
if cab2.get('falseFront') is not False or cab2.get('cutout') not in (None, []):
    fail(f"deleting the sink leaves cb0 with falseFront {cab2.get('falseFront')!r}, cutout {cab2.get('cutout')!r}")
print("ok: the sink is its own object; deleting it closes the counter")

# ---- the dishwasher: base cabinets x 0.2..0.8 (cb0) and 1.4..2.0 (cb1), a 0.6 m gap; cb2 x 2.0..2.6 touches cb1.
GAP = ROOM + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 0.6, "width": 0.6}),
              cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.8, "width": 0.6}),
              cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 2.4, "width": 0.6})]
rr, (j, binc) = export_glb(GAP + [cmd('library.place', {"item": "own:dishwasher-60", "at": [1.1, 0.4]}), INSPECT14], tmp, 'dw.glb')
dw = {f['id']: f for f in furnishings_of(inspected(rr))}['f0']
if (dw.get('host') or {}).get('kind') != 'gap' or {dw['host'].get('left'), dw['host'].get('right')} != {'cb0', 'cb1'}:
    fail(f"the dishwasher's host: {dw.get('host')}")
near(dw['at'][0], 1.1, 1e-6, "dishwasher centred in the gap")
near(dw['at'][1], 0.3, 1e-6, "dishwasher in line with the cabinets")
counters = [n for n in node_names(j) if n and n.startswith('cabinet-') and n.endswith('-counter')]
if counters != ['cabinet-cb0-counter']:
    fail(f"one continuous counter expected, GLB counter nodes: {counters}")
lo, hi = drawing_box(j, 'cabinet-cb0-counter')
near(lo[0], 0.2, 1e-4, "counter run start"); near(hi[0], 2.6, 1e-4, "counter run end")
for x in (0.5, 1.1, 1.7, 2.3):
    if not covered(j, binc, 'cabinet-cb0-counter', x, 0.3):
        fail(f"the counter run does not cover x = {x}")
print("ok: the dishwasher in the gap carries one counter over cb0, the gap, cb1 and cb2")
rr, (j, binc) = export_glb(GAP + [cmd('library.place', {"item": "own:range-30", "at": [1.1, 0.4]})], tmp, 'range.glb')
counters = sorted(n for n in node_names(j) if n and n.startswith('cabinet-') and n.endswith('-counter'))
if counters != ['cabinet-cb0-counter', 'cabinet-cb1-counter']:
    fail(f"a range in the gap: counter nodes {counters}, expected cb0's and the cb1+cb2 run's")
if covered(j, binc, 'cabinet-cb0-counter', 1.1, 0.3) or covered(j, binc, 'cabinet-cb1-counter', 1.1, 0.3):
    fail("a counter runs over the range")
print("ok: a range in a gap is not counter-topped")
print("PASS 05")
