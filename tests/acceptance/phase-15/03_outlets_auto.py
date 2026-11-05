#!/usr/bin/env python3
# outlets.auto: duplex outlets along every usable wall of a room (doorways break the wall; windows do not) so that no point of
# it is more than 1.8 m, measured along the wall, from an outlet; in a kitchen, GFCI outlets over the counters at most 1.2 m
# apart and within 0.6 m of each counter end, 0.15 m above the counter top, and the wall behind the counter is served by them.
# needs: p15-arch-outlets-auto, p15-cmd-electrical, p15-cmd-outlets-auto, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

RING = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)]      # the inner faces, counter-clockwise (edge k = wall wk)
# A door on w0 centred at x = 2.9 (offset 3.0 from w0's start at x = -0.1), 0.9 wide; a window on w2 (does not break the wall).
OPEN = [cmd('door', {"wall": "w0", "offset": 3.0, "width": 0.9}), cmd('window', {"wall": "w2", "offset": 3.0, "width": 1.2})]
rc, rr, so, se = run(ROOM64 + OPEN + [cmd('outlets.auto', {"room": "r0"}), INSPECT15])
auto = reply_with(rr, 'placed', -1, 'outlets.auto')
ins = inspected(rr)
outs = [e for e in rows_of(ins, 'electrical') if e['id'] in auto['placed']]
if not outs or any(e['kind'] != 'duplex' or abs(e['z'] - 0.3) > 1e-6 or e['layer'] != 'E-POWR' for e in outs):
    fail(f"outlets.auto places duplex outlets at 0.3 m on E-POWR: {outs}")
for e in outs:
    if 2.45 - 1e-6 < e['at'][0] < 3.35 + 1e-6 and abs(e['at'][1]) < 1e-6:
        fail(f"{e['id']} is in the doorway: {e['at']}")
runs = ring_runs(RING, [(0, 2.45, 3.35)])
worst = max(run_gap(r, [tuple(e['at']) for e in outs]) for r in runs)
if worst > 1.8 + 1e-6:
    fail(f"a point of usable wall is {worst:.3f} m from an outlet (limit 1.8); outlets at {[e['at'] for e in outs]}")
g = [x for x in rows_of(ins, 'outletGaps') if x['room'] == 'r0']
if not g or g[0]['maxGap'] > 1.8 + 1e-6 or abs(g[0]['maxGap'] - worst) > 0.06:
    fail(f"outletGaps reports the same worst gap ({worst:.3f}): {g}")
if auto.get('kitchen') is not False:
    fail(f"r0 is not a kitchen: {auto}")
print(f"ok: {len(outs)} duplex outlets, worst gap {worst:.2f} m along the wall, none in the doorway")

KITCHEN = [cmd('room', {"id": "r0", "type": "kitchen"}),
           cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.9, 0.6]}),
           cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 1.5, "width": 0.6})]
rc, rr, so, se = run(ROOM64 + KITCHEN + [cmd('outlets.auto', {"room": "r0"}), INSPECT15])
auto = reply_with(rr, 'placed', -1, 'outlets.auto')
ins = inspected(rr)
if auto.get('kitchen') is not True:
    fail(f"a kitchen room: {auto}")
cabs = rows_of(ins, 'cabinets')
placed = [e for e in rows_of(ins, 'electrical') if e['id'] in auto['placed']]
for wall in ('w0', 'w1'):
    spans = sorted(tuple(c['alongWall']) for c in cabs if c.get('wall') == wall and c.get('kind') == 'base')
    s0, s1 = spans[0][0], max(s for _, s in spans)
    top = max(num(c['counterTop'], 'counterTop') for c in cabs if c.get('wall') == wall)
    g = sorted(e['offset'] for e in placed if e['wall'] == wall and e['kind'] == 'gfci')
    if not g:
        fail(f"GFCI outlets over the {wall} counter: {placed}")
    if g[0] - s0 > 0.6 + 1e-6 or s1 - g[-1] > 0.6 + 1e-6 or any(b - a > 1.2 + 1e-6 for a, b in zip(g, g[1:])):
        fail(f"{wall} counter {s0:.2f}..{s1:.2f}: GFCI offsets {g} (<= 0.6 from each end, <= 1.2 apart)")
    for e in placed:
        if e['wall'] == wall and e['kind'] == 'gfci':
            near(e['z'], top + 0.15, 1e-6, f"{e['id']} 0.15 above the {wall} counter")
gap = [x for x in rows_of(ins, 'outletGaps') if x['room'] == 'r0'][0]
if gap['maxGap'] > 1.8 + 1e-6 or not gap.get('counter') or gap['counter']['maxSpacing'] > 1.2 + 1e-6:
    fail(f"the kitchen's reported gaps: {gap}")
print("ok: kitchen GFCI outlets over both counters (<= 1.2 m apart, <= 0.6 m from the ends, counter top + 0.15)")
rc, rr, so, se = run(ROOM64 + [cmd('outlets.auto', {"room": "r7"})], expect_ok=False)
if not errors(se):
    fail("outlets.auto on a missing room is refused")
print("PASS 03")
