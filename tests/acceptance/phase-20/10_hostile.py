#!/usr/bin/env python3
# Carried forward: hostile input. Every malformed plan-view, export, dimension, auto-dimension, driving, room-label and
# inspect request is refused with an error line, nothing panics, and nothing is half-applied: plan views, tabs, auto
# rows, dimensions, walls, openings and rooms are unchanged afterwards. A positive control then drives w5 with the same
# dimension, so the refusal of the unattached w1 comes from the attachment test, not from a dimdrive that refuses all.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-plan-views, p20-view-export, p20-dims-model, p20-auto-dims-1, p20-auto-dims-2, p20-dims-drive, p20-room-labels
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()
R = house_rooms()
SET = HOUSE + [PV_WINDOW, cmd('dimauto', {"level": 0})]
rr = run_ok(SET + [INSPECT20], "the set-up")
h = pv_auto_row(pv_auto(rr), 'north', 'walls')['handles'][0]     # 6.1: w3's outer face to w5's centre line
svg = os.path.join(tmp, 'x.svg')
P = lambda **kw: cmd('planview', kw)
HOSTILE = [
    'planview {}', P(name=""), P(name=7), P(name="x" * 65), P(name="Working"), P(name="working"),
    P(name="A", **{"from": "Nope"}), P(name="A", level=9), P(name="A", kind="attic"),
    P(name="A", layers={"rules": "all"}), P(name="A", layers={"rules": [{"pattern": "*", "transparency": 95}]}),
    P(name="A", layers={"rules": [{"pattern": "x" * 300}]}), P(name="A", reference={"level": 5}),
    P(name="A", zoom={"center": [0, 0], "width": -1, "height": 2}), P(name="A", labels={"area": "yes"}),
    P(name="A", annotation={"layer": "bad*name"}),
    cmd('planview.open', {"name": "Nope"}), cmd('planview.edit', {"name": "Floor Plan", "rename": "Working"}),
    cmd('planview.delete', {"name": "Nope"}), cmd('planview.close', {"name": "Nope"}),
    cmd('export', {"path": svg, "planView": "Nope"}), cmd('export', {"path": svg, "planView": "Site", "level": 0}),
    cmd('dimauto', {"level": 9}), cmd('dimauto', {"level": 0, "rows": ["sideways"]}),
    cmd('dimdrive', {"dimension": "ZZ", "value": 1}),
    cmd('dimdrive', {"dimension": h, "value": -1, "object": "w5"}),
    cmd('dimdrive', {"dimension": h, "value": 1e308, "object": "w5"}),
    cmd('dimdrive', {"dimension": h, "value": "5", "object": "w5"}),
    cmd('dimdrive', {"dimension": h, "value": 5, "object": "w1"}),
    cmd('room.label', {"room": "r99", "area": "standard"}), cmd('room.label', {"room": R['S'], "area": "gross"}),
    cmd('arch.inspect', {"plan": "Nope"}), cmd('arch.inspect', {"plan": 3}),
]
rc, rr, so, se = run(SET + [INSPECT20] + HOSTILE + [INSPECT20], expect_ok=False)
errs = errors(se)
if len(errs) != len(HOSTILE):
    fail(f"each of the {len(HOSTILE)} hostile requests is refused with one error line, got {len(errs)}: {errs[:40]}")
if os.path.exists(svg):
    fail("a refused export wrote a file")
a, b = inspected(rr, 0), inspected(rr, -1)
for key in ('planViews', 'openPlanViews', 'activePlanView', 'autoDims', 'dimensions', 'walls', 'openings', 'rooms'):
    if json.dumps(a.get(key), sort_keys=True) != json.dumps(b.get(key), sort_keys=True):
        fail(f"a refused request changed {key}: {json.dumps(a.get(key))[:300]} -> {json.dumps(b.get(key))[:300]}")
if not isinstance(a.get('planViews'), list) or not isinstance(a.get('autoDims'), list):
    fail("arch.inspect reports planViews and autoDims")
print(f"ok: {len(HOSTILE)} hostile requests refused, nothing changed")

# Positive control: the same dimension drives w5, which it locates.
rr = run_ok(SET + [cmd('dimdrive', {"dimension": h, "value": 5, "object": "w5"}), INSPECT20], "driving w5 by 6.1 -> 5")
w5 = pv_wall(inspected(rr), 'w5')
if any(abs(pv_xy(w5.get(k), 'w5')[0] - 4.9) > 1e-6 for k in ('start', 'end')) or \
        [pv_xy(w5.get(k), 'w5')[1] for k in ('start', 'end')] != [3.0, 8.0]:
    fail(f"dimdrive moves w5 to x = 4.9 (6.0 - 1.1), y still 3 and 8: {w5}")
print("ok: the positive control moves w5 to x = 4.9")
print("PASS 10")
