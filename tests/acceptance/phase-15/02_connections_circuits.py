#!/usr/bin/env python3
# Connections: a switch's lights form a circuit; a second (three-way) switch to the same lights joins that circuit;
# circuit.brightness dims every light on it; the plan draws each connection as an arc on E-LITE whose bulge reshapes;
# bad connections are refused; deleting one ends the circuit.
# needs: p15-doc-electrical, p15-arch-connections, p15-cmd-electrical, p15-cmd-connections, p15-render-electrical, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
BASE = ROOM64 + [cmd('lightfixture', {"room": "r0", "rows": 1, "cols": 2}),                       # lf0, lf1
                 cmd('switch', {"kind": "three-way", "wall": "w3", "offset": 1.0, "side": "left"}),   # el0
                 cmd('switch', {"kind": "three-way", "wall": "w1", "offset": 3.0, "side": "left"}),   # el1
                 cmd('outlet', {"kind": "duplex", "wall": "w0", "offset": 1.0, "side": "left"}),      # el2
                 cmd('connection', {"from": "el0", "to": ["lf0", "lf1"]}),                            # cn0
                 cmd('connection', {"from": "el1", "to": ["lf0", "lf1"], "bulge": 0.25})]             # cn1
saved = os.path.join(tmp, 'c.dxf')
rc, rr, so, se = run(BASE + [cmd('circuit.brightness', {"switch": "el1", "brightness": 0.4}), INSPECT15], save=saved)
ins = inspected(rr)
cons = rows_of(ins, 'connections')
if [(c['id'], c['from'], sorted(c['to'])) for c in cons] != [('cn0', 'el0', ['lf0', 'lf1']), ('cn1', 'el1', ['lf0', 'lf1'])]:
    fail(f"connections: {cons}")
near(cons[1].get('bulge'), 0.25, 1e-9, "cn1's bulge")
circ = rows_of(ins, 'circuits')
if len(circ) != 1 or sorted(circ[0]['switches']) != ['el0', 'el1'] or sorted(circ[0]['lights']) != ['lf0', 'lf1']:
    fail(f"one circuit of both switches and both lights: {circ}")
lights = {l['id']: l for l in lights_of(rr)}
for k in ('lf0', 'lf1'):
    near(lights[k].get('brightness'), 0.4, 1e-9, f"{k} dimmed with its circuit")
near(circ[0].get('brightness'), 0.4, 1e-9, "the circuit's brightness")


def lite_curves(path):
    """ARC / LWPOLYLINE entities on E-LITE in a saved plan (entities() gives (kind, layer, pairs))."""
    return [e for e in entities(dxf_text(path)) if e[1] == 'E-LITE' and e[0] in ('ARC', 'LWPOLYLINE')]


# The switch and light symbols are on E-LITE too: count what the two connections add to the same plan without them.
bare = os.path.join(tmp, 'bare.dxf')
run(BASE[:-2] + [INSPECT15], save=bare)
arcs = len(lite_curves(saved)) - len(lite_curves(bare))
if arcs < 4:
    fail(f"each switch-to-light leg is an arc on E-LITE in the plan: the connections add {arcs} ARC/LWPOLYLINE entities there")
print("ok: two three-way switches share one circuit of two lights; circuit.brightness dims both; arcs on E-LITE")

rc, rr, so, se = run(BASE + [cmd('connection.edit', {"id": "cn0", "bulge": -0.5}), INSPECT15])
near(rows_of(inspected(rr), 'connections')[0].get('bulge'), -0.5, 1e-9, "a reshaped arc")
rc, rr, so, se = run(BASE + [cmd('connection.delete', {"id": "cn1"}), cmd('connection.delete', {"id": "cn0"}), INSPECT15])
if rows_of(inspected(rr), 'circuits'):
    fail(f"no circuit once both connections are gone: {rows_of(inspected(rr), 'circuits')}")
for bad, why in ((cmd('connection', {"from": "el2", "to": ["lf0"]}), "from an outlet"),
                 (cmd('connection', {"from": "el0", "to": ["el1"]}), "to a switch"),
                 (cmd('connection', {"from": "el0", "to": []}), "to no light"),
                 (cmd('connection', {"from": "el0", "to": ["lf9"]}), "to a missing light"),
                 (cmd('connection', {"from": "el0", "to": ["lf0"], "bulge": 3}), "a bulge out of -1..1"),
                 (cmd('circuit.brightness', {"switch": "el0", "brightness": 1.5}), "a brightness above 1"),
                 (cmd('circuit.brightness', {"switch": "el2", "brightness": 0.5}), "dimming an outlet")):
    rc, rr, so, se = run(BASE + [bad], expect_ok=False)
    if not errors(se):
        fail(f"a connection {why} is refused")
print("ok: bulge edits, deletes end the circuit, and seven bad connections are refused")
print("PASS 02")
