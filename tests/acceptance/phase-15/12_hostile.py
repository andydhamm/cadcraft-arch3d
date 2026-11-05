#!/usr/bin/env python3
# Hostile input to every new command is refused with an error line, and nothing is placed, connected, dimmed, saved or moved.
# needs: p15-cmd-electrical, p15-cmd-connections, p15-cmd-outlets-auto, p15-cmd-light-sets, p15-cmd-electrical-view, p15-cmd-levels, p15-cmd-export-view, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

NAN = float('nan')
SETUP = ROOM64 + [cmd('lightfixture', {"room": "r0", "rows": 1, "cols": 1}),
                  cmd('switch', {"kind": "single-pole", "wall": "w3", "offset": 1.0, "side": "left"})]
H = [('outlet', {"kind": "duplex", "wall": "w0", "offset": "1"}), ('outlet', {"kind": "duplex", "wall": "w0", "offset": 1e308, "side": "left"}),
     ('outlet', {"kind": "duplex", "wall": "w0", "point": [NAN, 0], "side": "left"}), ('outlet', {"kind": ["duplex"], "wall": "w0", "offset": 1}),
     ('outlet', {"kind": "duplex", "wall": "w0", "offset": 1, "side": "left", "height": 99}), ('outlet', {"kind": "duplex", "wall": "w0", "offset": 1, "side": "left", "level": "Moon"}),
     ('switch', {"kind": "dimmer", "wall": "r0", "offset": 1, "side": "left"}), ('switch', {}),
     ('electrical.edit', {"id": "el9", "height": 1.0}), ('electrical.edit', {"id": "el0", "offset": -1}),
     ('electrical.delete', {"id": "lf0"}), ('connection', {"from": "el0", "to": "lf0"}),
     ('connection', {"from": "el0", "to": ["lf0", "lf0"]}), ('connection.edit', {"id": "cn5", "bulge": 0.1}),
     ('connection.delete', {"id": "el0"}), ('circuit.brightness', {"switch": "el0", "brightness": NAN}),
     ('circuit.brightness', {"switch": "el0", "brightness": -0.1}), ('outlets.auto', {"room": 0}),
     ('outlets.auto', {}), ('lightset', {"name": "A", "lights": []}), ('lightset', {"name": "A", "lights": {"lf0": {"on": "yes"}}}),
     ('lightset', {"delete": "Nope"}), ('view.electrical', {"dim": 500}), ('level.show', {"level": 7}),
     ('level', {"name": "X", "elevation": NAN}), ('level', {"name": "X", "kind": "attic"}),
     ('level.edit', {"level": 9, "elevation": 1.0}), ('level.edit', {"level": 0, "ceiling": -2}),
     ('wall', {"points": [[0, 0], [1, 0]], "thickness": 0.1, "topLevel": "Nowhere"}),
     ('terrain.modify', {"path": {"kind": "moat", "polygon": [[0, 0], [1, 0], [1, 1]]}}),
     ('render', {"out": "/tmp/x.png", "style": "watercolor"}), ('export', {"path": "/tmp/x.png", "view": "v9"})]
rc, rr, so, se = run(SETUP + [INSPECT15])
before = inspected(rr)
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT15], expect_ok=False)
if 'panicked' in se:
    fail(f"a hostile command panicked: {se[-400:]}")
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:40]))
after = inspected(rr)
for key in ('electrical', 'connections', 'circuits', 'levels', 'lightSets'):
    if json.dumps(before.get(key)) != json.dumps(after.get(key)):
        fail(f"refused commands changed '{key}': {json.dumps(before.get(key))[:200]} -> {json.dumps(after.get(key))[:200]}")
if [l.get('brightness') for l in lights_of([before])] != [l.get('brightness') for l in lights_of(rr)]:
    fail("a refused brightness changed a light")
print(f"ok: {len(H)} hostile commands refused; nothing changed")
print("PASS 12")
