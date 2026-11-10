#!/usr/bin/env python3
# Landing walls and ceilings stop at the level above, and an Open Below foyer opens it (the phase-15 checkpoint,
# decision 1). The split test house: Lower (level 0) at 0, Main at 2.7, Entry (a landing) at 1.35; the front wall w0 spans
# Lower's floor to Main's ceiling (5.1), Lower and Main have their own 2.4 m rings, and the landing is a box of 0.1 m walls
# (3,0)-(3,3)-(6,3)-(6,0) on Entry at the default height (2.7: their top would be 4.05, through Main's floor into Main's
# room). Every level detects its rooms: the landing room is x 3.05..5.95, y 0.1..2.95.
#  1. Under Main's floor: the landing's ceiling is clipped to Main's slab underside (2.7 - 0.3 = 2.4, not 1.35 + 2.4 =
#     3.75); the landing's walls stop at Main's floor (2.7): w7 is hit at (3,1.5,2.0) and nothing is at (3,1.5,3.5); the
#     landing's ceiling solid hangs at 2.38..2.4 and nothing is left at 3.74. Lower's ceiling under the landing stops at
#     the landing's slab underside (1.35 - 0.3 = 1.05; solid at 1.04) and stays 2.4 elsewhere ((8,6)).
#  2. An Open Below foyer: `room {"points": the landing's ring, "type": "openBelow", "level": "Main"}` cuts its ring out of
#     Main's room (a hole; 76.44 - 2.9 x 2.85 = 68.175 m²); Main's floor opens over the landing (nothing at (4.5,1.5,2.55)),
#     the landing's ceiling is the foyer's (2.7 + 2.4 = 5.1; its solid at 5.09 is the Open Below room's ceiling), nothing
#     hangs at 2.39 any more, and the landing wall beside the open foyer keeps its height (hit at (3,1.5,3.5)).
# A stub fails: today the landing's ceiling is 3.75 with its solid at 3.74, w7 reaches 4.05, Lower's ceiling is 2.4 under
# the landing, and a drawn Open Below room overlaps Main's room without opening its floor.
# needs: p16-rw-overhead-arch, p16-rw-overhead-3d, p16-rw-open-below-rooms
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

RING = [[10, 0], [10, 8], [0, 8], [0, 0]]
HOUSE15 = [cmd('level.edit', {"level": 0, "name": "Lower"}),
           cmd('level', {"name": "Main", "elevation": 2.7}),
           cmd('level', {"name": "Entry", "elevation": 1.35, "kind": "landing"}),
           cmd('wall', {"points": [[0, 0], [10, 0]], "thickness": 0.2, "exterior": True, "level": "Lower", "topLevel": "Main"}),
           cmd('wall', {"points": RING, "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Lower"}),
           cmd('wall', {"points": RING, "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Main"}),
           cmd('wall', {"points": [[3, 0], [3, 3], [6, 3], [6, 0]], "thickness": 0.1, "level": "Entry"}),
           cmd('roomdetect', {})]
INSPECT = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True})
FOYER = [[3.05, 0.1], [5.95, 0.1], [5.95, 2.95], [3.05, 2.95], [3.05, 0.1]]
P = [4.5, 1.5]


def holds(poly, x, y):
    inside = False
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            inside = not inside
    return inside


def room_holding(insp, lrow, x, y, what):
    """The one room on level `lrow` whose polygon, less its holes, holds (x, y)."""
    hits = [r for r in objects_named(insp, 'rooms', 'rooms') if on_level(r.get('level'), lrow)
            and holds(r.get('polygon') or [], x, y) and not any(holds(h, x, y) for h in r.get('holeRings') or [])]
    if len(hits) != 1:
        fail(f"{what}: one room on {lrow.get('name')} holds ({x}, {y}), found {[(r.get('id'), r.get('type')) for r in hits]}")
    return hits[0]


rr = run_ok(HOUSE15 + [INSPECT], "the split test house")
insp = inspected(rr)
lower, main, entry = (level_row(insp, n) for n in ("Lower", "Main", "Entry"))
LAND = room_holding(insp, entry, *P, "the landing")['id']
LOW = room_holding(insp, lower, *P, "Lower's room")['id']
w7 = [w for w in objects_named(insp, 'walls', 'walls') if on_level(w.get('level'), entry) and w.get('start') == [3.0, 0.0]]
if len(w7) != 1:
    fail(f"the landing's wall from (3,0): {[(w.get('id'), w.get('level'), w.get('start')) for w in objects_named(insp, 'walls', 'walls')]}")
W7 = w7[0]['id']

# 1. Under Main's floor.
PR1 = [[3.0, 1.5, 2.0], [3.0, 1.5, 3.5], [4.5, 1.5, 2.39], [4.5, 1.5, 3.74], [4.5, 1.5, 1.04]]
rr = run_ok(HOUSE15 + [ceiling_cmd([P], "Entry"), ceiling_cmd([P, [8, 6]], "Lower"), probe_cmd(PR1)], "the landing under Main's floor")
expect_ceilings(rr, [P], [2.4], "the landing's ceiling under Main's floor (Main's slab underside)", nth=0)
expect_ceilings(rr, [P, [8, 6]], [1.05, 2.4], "Lower's ceiling: the landing's slab underside under it, 2.4 elsewhere", nth=1)
expect_hits(rr, [(PR1[0], ('wall', W7)), (PR1[1], None), (PR1[2], ('ceiling', LAND)), (PR1[3], None), (PR1[4], ('ceiling', LOW))],
            "the landing's wall stops at Main's floor (2.7), its ceiling solid at 2.38..2.4, Lower's at 1.03..1.05 under it")
print(f"ok: under Main's floor the landing ({LAND}) has its ceiling at 2.4 and {W7} stops at 2.7; Lower ({LOW}) stops at 1.05 under the landing")

# 2. An Open Below foyer over the landing.
OPEN = cmd('room', {"points": FOYER, "type": "openBelow", "level": "Main"})
PR2 = [[4.5, 1.5, 2.55], [4.5, 1.5, 2.39], [4.5, 1.5, 5.09], [3.0, 1.5, 3.5]]
rr = run_ok(HOUSE15 + [OPEN, INSPECT, ceiling_cmd([P], "Entry"), ceiling_cmd([P], "Lower"), probe_cmd(PR2)], "an Open Below foyer on Main")
insp = inspected(rr)
ob = room_holding(insp, main, *P, "the foyer on Main")
if ob.get('type') != 'openBelow':
    fail(f"the room on Main over the landing is the Open Below foyer: {ob.get('id')} is {ob.get('type')}")
big = room_holding(insp, main, 8, 6, "Main's room")
hr = big.get('holeRings') or []
if len(hr) != 1:
    fail(f"the foyer is a hole in Main's room {big.get('id')}: holeRings {hr!r}")
expect_box(hr[0], [3.05, 0.1, 5.95, 2.95], f"{big.get('id')}'s hole (the foyer)")
near(big.get('interiorArea'), 76.44 - 2.9 * 2.85, 1e-6, f"Main's room {big.get('id')} less the foyer")
expect_ceilings(rr, [P], [5.1], "the landing's ceiling under the Open Below foyer (the foyer's, 2.7 + 2.4)", nth=0)
expect_ceilings(rr, [P], [1.05], "Lower's ceiling under the landing (the landing's floor is still there)", nth=1)
expect_hits(rr, [(PR2[0], None), (PR2[1], None), (PR2[2], ('ceiling', ob.get('id'))), (PR2[3], ('wall', W7))],
            "Main's floor opens over the foyer, the foyer's ceiling hangs at 5.08..5.1, the wall beside it keeps its height")
print(f"ok: the Open Below foyer {ob.get('id')} is a hole in {big.get('id')} (68.175 m²), opens Main's floor and lifts the landing's ceiling to 5.1")
print("PASS 15")
