#!/usr/bin/env python3
# Stair wells decide up and down by elevation and measure headroom from the stair's own base (the phase-15 checkpoint,
# decision 1). The phase-5 stair (start (2,1) heading +y, 1.0 wide: 16 risers of 0.16875, 15 treads of 0.254, run 3.81)
# opens the upper floor from the first riser whose top is less than 2.032 under the upper slab's underside, counted from
# the stair's base. With the upper floor 2.7 above the base and 0.3 thick that is riser 3 (3 x 0.16875 > 2.4 - 2.032), so
# the well is x 1.5..2.5, y 1.508..4.81, whatever the levels' elevations and the order they were made in:
#   A. Main made first (index 0, renamed, at 2.7) and Lower after it (index 1, at 0): stair Lower -> Main;
#   B. the same levels with the stair given from Main to Lower: it is stored lower level first, with the same well;
#   C. three stacked levels at 0, 2.7 and 5.4: a stair from level 1 to level 2 (base 2.7).
# Each well is an Open Below room on the upper level (the stair's wellRoom) and a hole in the upper room (76.44 - 3.302).
# roofauto with no level builds over the highest level by elevation: a basement added after Level 1 does not take the
# roof (its eaves stay at 2.4, not -0.3).
# A stub fails: A and C answer "well": false today (C: measured from z = 0 no riser is low enough), B is refused (a
# negative rise), and the roof lands on the basement.
# needs: p16-rw-stair-wells, p16-rw-roof-top-level
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

RING = {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True}
INSPECT = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "stairs": True, "roofs": True})
STAIR = {"start": [2, 1], "direction": 90, "width": 1.0}
WELL = [1.5, 1.508, 2.5, 4.81]


def ring(level):
    return cmd('wall', dict(RING, level=level))


def holds(poly, x, y):
    inside = False
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            inside = not inside
    return inside


def expect_well(cmds, lower, upper, what):
    rr = run_ok(cmds + [INSPECT], what)
    rep = reply_with(rr, 'well', -1, f"{what}: the stair's reply")
    if rep.get('well') is not True:
        fail(f"{what}: the stair cuts a well into {upper} (reply \"well\": true): {json.dumps(rep)[:300]}")
    insp = inspected(rr)
    lo, up = level_row(insp, lower), level_row(insp, upper)
    st = objects_named(insp, 'stairs', 'stairs')
    if len(st) != 1:
        fail(f"{what}: one stair: {json.dumps(st)[:300]}")
    s = st[0]
    if s.get('fromLevel') != lo.get('index') or s.get('toLevel') != up.get('index'):
        fail(f"{what}: the stair is stored from the lower level {lower} (index {lo.get('index')}) to {upper} "
             f"(index {up.get('index')}): fromLevel {s.get('fromLevel')}, toLevel {s.get('toLevel')}")
    expect_box(s.get('well'), WELL, f"{what}: the well")
    rooms = objects_named(insp, 'rooms', 'rooms')
    wr = [r for r in rooms if r.get('id') == s.get('wellRoom')]
    if len(wr) != 1 or wr[0].get('type') != 'openBelow' or not on_level(wr[0].get('level'), up):
        fail(f"{what}: the stair's wellRoom {s.get('wellRoom')!r} is an Open Below room on {upper}: "
             f"{[(r.get('id'), r.get('type'), r.get('level')) for r in rooms]}")
    host = [r for r in rooms if on_level(r.get('level'), up) and r.get('type') != 'openBelow' and holds(r.get('polygon') or [], 5, 6)]
    if len(host) != 1:
        fail(f"{what}: one room on {upper} around the well: {[(r.get('id'), r.get('type'), r.get('level')) for r in rooms]}")
    hr = host[0].get('holeRings') or []
    if len(hr) != 1:
        fail(f"{what}: the well is a hole in {host[0].get('id')}: holeRings {hr!r}")
    expect_box(hr[0], WELL, f"{what}: {host[0].get('id')}'s hole")
    near(host[0].get('interiorArea'), 76.44 - 3.302, 1e-6, f"{what}: {host[0].get('id')}'s interior area (76.44 less the well)")
    return insp


# A and B: Main is index 0 at 2.7, Lower index 1 at 0.
AB = [cmd('level.edit', {"level": 0, "name": "Main", "elevation": 2.7}), cmd('level', {"name": "Lower", "elevation": 0}),
      ring("Lower"), ring("Main"), cmd('roomdetect', {})]
expect_well(AB + [cmd('stair', dict(STAIR, fromLevel="Lower", toLevel="Main"))], "Lower", "Main",
            "A (Lower made after Main, stair Lower -> Main)")
print("ok: A: Main (index 0) above Lower (index 1): the stair Lower -> Main cuts its well y 1.508..4.81 into Main")
expect_well(AB + [cmd('stair', dict(STAIR, fromLevel="Main", toLevel="Lower"))], "Lower", "Main",
            "B (the stair given from Main down to Lower)")
print("ok: B: given Main -> Lower, the stair is stored Lower -> Main with the same well")

# C: three stacked levels; the stair from level 1 (2.7) to level 2 (5.4).
C = [ring(0), cmd('level', {"above": 0}), ring(1), cmd('level', {"above": 1}), ring(2), cmd('roomdetect', {}),
     cmd('stair', dict(STAIR, fromLevel=1, toLevel=2))]
insp = expect_well(C, 1, 2, "C (levels 0, 2.7, 5.4; the stair from level 1 to level 2)")
near(level_row(insp, 2).get('elevation'), 5.4, 1e-9, "C: level 2's elevation")
print("ok: C: a stair from 2.7 to 5.4 measures headroom from its base: the well starts 0.508 from its start")

# The roof goes on the highest level by elevation.
rr = run_ok([ring(0), cmd('level', {"name": "Basement", "elevation": -2.7}), ring("Basement"),
             cmd('roofauto', {"pitchRise": 6, "pitchRun": 12}), INSPECT], "a basement added after Level 1, then roofauto")
insp = inspected(rr)
roofs = objects_named(insp, 'roofs', 'roofs')
l1 = level_row(insp, 0)
eaves = [e for r in roofs for e in (r.get('edges') or []) if isinstance(e, dict) and e.get('kind') == 'eave']
if len(roofs) != 1 or roofs[0].get('level') != l1.get('index') or not eaves:
    fail(f"roofauto builds over {l1.get('name')} (index {l1.get('index')}), the highest level by elevation: "
         f"{[(r.get('id'), r.get('level')) for r in roofs]}")
for e in eaves:
    near(e.get('za'), 2.4, 1e-6, "an eave of the roof over Level 1 (its walls' tops)")
print("ok: roofauto builds over Level 1 (eaves 2.4), not over the basement added after it")
print("PASS 14")
