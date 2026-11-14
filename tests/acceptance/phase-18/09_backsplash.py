#!/usr/bin/env python3
# Backsplash (the brief's bullet and its number: "over a counter run with wall cabinets above part of it, the
# backsplash's top equals the cabinets' bottom over that part and the counter top + 0.45 elsewhere (± 1e-6)"). Three
# 0.6 base cabinets on the kitchen's south wall (x 0.6..2.4, counter top 0.914) and one 0.6 wall cabinet over the first
# (x 0.6..1.2, bottom 1.372): `backsplash {run: cb0, material: material-marble}` covers the wall face y = 0 over
# x 0.6..2.4, 10 mm thick, from 0.914 up to 1.372 under the wall cabinet and to 1.364 elsewhere; probes find it inside
# the 10 mm and not 15 mm out; a cabinet with `backsplash: false` drops its stretch (from every backsplash, so it is
# switched on again before the wall one is made); `top: ceiling` raises every top to 2.4; `backsplash {wall: w0,
# height: 0.3}` covers the same face with tops 1.372 / 1.214; `backsplash {}` and a wall cabinet's run are refused.
# A stub fails: no backsplash rows, no segments, one top everywhere, no probe hits.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-backsplash
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

BASES = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.6, 0.6]})
WALLC = cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "width": 0.6})
BS = cmd('backsplash', {"run": "cb0", "material": "material-marble"})
P = [[1.5, 0.005, 1.1], [1.5, 0.015, 1.1], [0.9, 0.005, 1.368], [1.5, 0.005, 1.368]]


def tops_over(row, x, what):
    """The tops of the segments of a backsplash row whose x-extent holds x (on the y = 0 face)."""
    tops = []
    for s in row.get('segments') or []:
        a, b = s.get('from'), s.get('to')
        if not (isinstance(a, list) and isinstance(b, list)):
            fail(f"{what}: a segment has from/to points: {s}")
        if min(a[0], b[0]) - 1e-9 <= x <= max(a[0], b[0]) + 1e-9 and abs(a[1]) < 1e-6 and abs(b[1]) < 1e-6:
            near(s.get('bottom'), 0.914, 1e-6, f"{what}: the segment bottom (the counter top)")
            tops.append(num(s.get('top'), f"{what}: a segment top"))
    if not tops:
        fail(f"{what}: no segment on y = 0 covers x = {x}: {json.dumps(row.get('segments'))[:400]}")
    return tops


rr = run_ok(KITCHEN + [BASES, WALLC, BS, INSPECT18, probe_cmd(P)], "a backsplash over a run with one wall cabinet")
insp = inspected(rr)
rep = reply_with(rr, 'backsplash', -1, 'backsplash')
B0 = rep.get('backsplash')
row = by_id(backsplashes_of(insp), B0, 'arch.inspect backsplashes')
if row.get('material') != 'item:material-marble':
    fail(f"the backsplash's material: item:material-marble, got {row.get('material')!r}")
near(row.get('thickness'), 0.01, 1e-9, "the backsplash thickness")
xs = [q for s in row['segments'] for q in (s['from'][0], s['to'][0])]
near(min(xs), 0.6, 1e-6, "the segments start at the run's start")
near(max(xs), 2.4, 1e-6, "the segments end at the run's end")
for x, want in ((0.9, 1.372), (1.5, 1.364), (2.1, 1.364)):
    for t in tops_over(row, x, f"x = {x}"):
        near(t, want, 1e-6, f"the backsplash top at x = {x}")
g = probed(rr, P, what="backsplash")
hit(g[0], 'backsplash', B0, "inside the 10 mm backsplash at z 1.1")
nohit(g[1], "15 mm out from the wall face")
hit(g[2], 'backsplash', B0, "under the wall cabinet at z 1.368 (top 1.372)")
nohit(g[3], "beside the wall cabinet at z 1.368 (top 1.364)")
print(f"ok: {B0} tops 1.372 under the wall cabinet, 1.364 elsewhere, 10 mm thick")

rr = run_ok(KITCHEN + [BASES, WALLC, BS, cmd('cabinet.edit', {"id": "cb2", "backsplash": False}), INSPECT18, probe_cmd([[2.1, 0.005, 1.1]]),
                       cmd('backsplash.edit', {"id": B0, "top": "ceiling"}), INSPECT18, probe_cmd([[1.5, 0.005, 2.3]]),
                       cmd('cabinet.edit', {"id": "cb2", "backsplash": True}),
                       cmd('backsplash', {"wall": "w0", "height": 0.3}), INSPECT18], "per-cabinet off, to the ceiling, by wall")
row = by_id(backsplashes_of(inspected(rr, 0)), B0, 'backsplashes')
if max(q for s in row['segments'] for q in (s['from'][0], s['to'][0])) > 1.8 + 1e-6:
    fail(f"cb2 with backsplash off drops its stretch (nothing beyond x 1.8): {json.dumps(row['segments'])[:300]}")
if by_id(cabinets_of(inspected(rr, 0)), 'cb2', 'cabinets').get('backsplash') is not False:
    fail("cb2's row reports backsplash false")
nohit(probed(rr, [[2.1, 0.005, 1.1]], nth=0, what="off")[0], "behind cb2 once its backsplash is off")
row = by_id(backsplashes_of(inspected(rr, 1)), B0, 'backsplashes')
if row.get('toCeiling') is not True:
    fail("top: ceiling sets toCeiling")
for s in row['segments']:
    near(s.get('top'), 2.4, 1e-6, "a segment top at the ceiling")
hit(probed(rr, [[1.5, 0.005, 2.3]], nth=1, what="ceiling")[0], 'backsplash', B0, "the backsplash at z 2.3 once it reaches the ceiling")
if by_id(cabinets_of(inspected(rr, 2)), 'cb2', 'cabinets').get('backsplash') is not True:
    fail("cb2's row reports backsplash true once it is switched on again")
rows = backsplashes_of(inspected(rr, 2))
b1 = [r for r in rows if r.get('id') != B0]
if len(b1) != 1 or b1[0].get('wall') != 'w0':
    fail(f"`backsplash {{wall: w0}}` makes a second backsplash on w0: {[(r.get('id'), r.get('wall')) for r in rows]}")
for x, want in ((0.9, 1.372), (2.1, 1.214)):
    for t in tops_over(b1[0], x, f"wall backsplash at x = {x}"):
        near(t, want, 1e-6, f"the wall backsplash's top at x = {x}")
refused(KITCHEN + [BASES, cmd('backsplash', {})], "backsplash with neither run nor wall")
refused(KITCHEN + [BASES, WALLC, cmd('backsplash', {"run": "cb3"})], "a backsplash on a wall cabinet's run")
print("ok: per-cabinet off, to the ceiling, by wall (1.372 / 1.214); refusals")
print("PASS 09")
