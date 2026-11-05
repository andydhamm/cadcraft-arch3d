#!/usr/bin/env python3
# Outlets and switches: placed on a wall face at the standard heights (outlet 0.3 m, switch 1.2 m, counter outlet 0.15 m above
# the counter top), on E-POWR / E-LITE, with a plan symbol (a DXF proxy on its layer) and a 3D cover plate (probe part
# `electrical`, GLB node electrical-el<n>); every kind is accepted; bad placements are refused.
# needs: p15-doc-electrical, p15-arch-electrical, p15-cmd-electrical, p15-arch3d-electrical, p15-render-electrical, p15-io-electrical, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
O = lambda **k: cmd('outlet', dict({"side": "left"}, **k))
S = lambda **k: cmd('switch', dict({"side": "left"}, **k))
DEV = [O(kind="duplex", wall="w0", offset=1.0),                       # el0: inner face y = 0, x = 0.9
       O(kind="gfci", wall="w0", offset=2.0),                         # el1
       O(kind="240v", wall="w1", offset=1.0),                         # el2: inner face x = 6
       S(kind="single-pole", wall="w3", offset=1.0),                  # el3: inner face x = 0 (w3 runs (−0.1,4.1) -> (−0.1,−0.1))
       S(kind="three-way", wall="w3", offset=1.6),                    # el4
       S(kind="dimmer", wall="w2", offset=2.0),                       # el5: inner face y = 4
       cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 4.0, "width": 0.9}),
       O(kind="gfci", wall="w0", offset=4.3, counter=True)]           # el6 over the counter
saved = os.path.join(tmp, 'e.dxf')
rc, rr, so, se = run(ROOM64 + DEV + [INSPECT15, cmd('build3d', {"probe": [[0.9, 0.003, 0.3], [0.003, 3.1, 1.2], [0.9, 0.003, 1.5]]})],
                     save=saved)
devs = [r for r in rr if isinstance(r, dict) and 'device' in r]
if len(devs) != 7:
    fail(f"seven devices placed, got {len(devs)}: {devs}")
want = [("duplex", "E-POWR", (0.9, 0.0), 0.3), ("gfci", "E-POWR", (1.9, 0.0), 0.3), ("240v", "E-POWR", (6.0, 0.9), 0.3),
        ("single-pole", "E-LITE", (0.0, 3.1), 1.2), ("three-way", "E-LITE", (0.0, 2.5), 1.2), ("dimmer", "E-LITE", (4.1, 4.0), 1.2)]
for d, (kind, layer, at, z) in zip(devs, want):
    if d.get('kind') != kind or d.get('layer') != layer:
        fail(f"{d.get('device')}: kind {d.get('kind')} on {d.get('layer')}, expected {kind} on {layer}")
    near(d['at'][0], at[0], 1e-6, f"{d['device']} x")
    near(d['at'][1], at[1], 1e-6, f"{d['device']} y")
    near(d['z'], z, 1e-6, f"{d['device']} height")
    if d.get('room') != 'r0':
        fail(f"{d['device']} is in room r0: {d.get('room')}")
ins = inspected(rr)
cab = rows_of(ins, 'cabinets')[0]
near(devs[6]['z'], num(cab.get('counterTop'), 'counterTop') + 0.15, 1e-6, "the counter outlet 0.15 above the counter top")
el = rows_of(ins, 'electrical')
if [e['id'] for e in el] != [d['device'] for d in devs]:
    fail(f"arch.inspect lists the devices: {[e.get('id') for e in el]}")
hits = probes(rr)
if part(hits[0]) != 'electrical' or part(hits[1]) != 'electrical':
    fail(f"the cover plates probe as electrical: {hits[:2]}")
if part(hits[2]) == 'electrical':
    fail(f"no plate 1.2 m above the outlet: {hits[2]}")
print("ok: six kinds on their wall faces at 0.3 / 1.2 m on E-POWR / E-LITE, a counter outlet 0.15 above the counter, plates probe")

text = dxf_text(saved)
layers = {str(v).strip() for p in entities(text) for c, v in p if str(c).strip() == "8"}
for l in ('E-POWR', 'E-LITE'):
    if l not in layers:
        fail(f"the saved plan has a proxy on {l}: layers {sorted(layers)}")
g = os.path.join(tmp, 'e.glb')
export(ROOM64 + DEV, g)
j, binc = glb(g)
for k in range(7):
    if f'electrical-el{k}' not in set(node_names(j)):
        fail(f"GLB node electrical-el{k}: {sorted(n for n in node_names(j) if n and n.startswith('electrical'))}")
print("ok: plan proxies on E-POWR and E-LITE; GLB nodes electrical-el0..6")

for bad, why in ((O(kind="duplex", wall="w0", offset=9.0), "past the wall's end"),
                 (O(kind="quad", wall="w0", offset=1.0), "an unknown kind"),
                 (S(kind="dimmer", wall="w0", offset=1.0, height=-1), "a negative height"),
                 (O(kind="duplex", wall="w9", offset=1.0), "an unknown wall"),
                 (O(kind="gfci", wall="w2", offset=1.0, counter=True), "a counter outlet with no counter"),
                 (cmd('outlet', {"kind": "duplex", "wall": "w0", "offset": 1.0, "side": "up"}), "a bad side")):
    rc, rr, so, se = run(ROOM64 + [bad, INSPECT15], expect_ok=False)
    if not errors(se) or rows_of(inspected(rr), 'electrical'):
        fail(f"{why} is refused and places nothing (rc {rc}): {errors(se)[:2]}")
rc, rr, so, se = run(ROOM64 + [cmd('door', {"wall": "w0", "offset": 3.0, "width": 0.9}), O(kind="duplex", wall="w0", offset=3.0),
                               INSPECT15], expect_ok=False)
if not errors(se) or rows_of(inspected(rr), 'electrical'):
    fail("an outlet in a doorway is refused")
print("ok: past the end, unknown kind/wall/side, negative height, no counter and a doorway are refused")
print("PASS 01")
