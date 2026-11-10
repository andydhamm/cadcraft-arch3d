#!/usr/bin/env python3
# A wall that runs under a flight stops at the flight's underside, 0.3 m (settable: the stair's `underside`) under the
# line through the tread nosings, measured vertically, cut into its elevation profile like a gable clip. A stair
# starting at (1,1) heading +x, 0.9 m wide (y 0.55..1.45) over a 0.1 m wall (3,0)-(3,3): at x = 3.0 (2.0 m from the
# start, under tread 8) the nosing line is 0.16875 + 2.025 x 0.16875 / 0.254 = 1.5141, so the wall stops at 1.2141:
# wall at (3.0,1.0,1.15), nothing at (3.0,1.0,1.27); outside the flight it keeps its full 2.7 m (wall at (3.0,2.5,2.0)
# and (3.0,0.4,2.0)). Closed risers count 16 riser parts, open risers 0. underside 0.5 lowers the cut to 1.0141.
# A stub (no cut) fails on (3.0,1.0,1.27).
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-stair-flight, p17-wall-under-flight
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

WALL = cmd('wall', {"points": [[3, 0], [3, 3]], "thickness": 0.1})
STAIR = cmd('stair', {"start": [1, 1], "direction": 0, "width": 0.9, "fromLevel": 0, "toLevel": 1})
W = HOUSE2 + [WALL, STAIR]
P = [[3.0, 1.0, 1.15], [3.0, 1.0, 1.27], [3.0, 2.5, 2.0], [3.0, 0.4, 2.0]]
CUT = 0.16875 + 2.025 * 0.16875 / 0.254 - 0.3       # 1.2140994

rr = run_ok(W + [probe_cmd(P)], "a 0.1 m wall under the flight")
expect_hits(rr, [(P[0], 'wall'), (P[1], None), (P[2], 'wall'), (P[3], 'wall')],
            f"the wall stops at {CUT:.4f} under the flight (nosing line 1.5141 at x 3.0) and keeps 2.7 m beside it")
parts = parts_of(rr)
if parts.get('riser') != 16:
    fail(f"closed risers by default: parts.riser 16, got {parts.get('riser')} in {parts}")
print(f"ok: wall at (3,1,1.15), air at (3,1,1.27) (cut {CUT:.4f}), full height at y 0.4 and 2.5; 16 risers")

rr = run_ok(W + [cmd('stair.edit', {"id": "s0", "openRisers": True}), probe_cmd(P[:2])], "open risers over the wall")
if parts_of(rr).get('riser'):
    fail(f"open risers: 0 riser parts, got {parts_of(rr).get('riser')}")
expect_hits(rr, [(P[0], 'wall'), (P[1], None)], "the cut does not depend on the risers")
print("ok: open risers: 0 riser parts, the cut stays")

rr = run_ok(W + [cmd('stair.edit', {"id": "s0", "underside": 0.5}), probe_cmd([[3.0, 1.0, 1.05], [3.0, 1.0, 0.95]])], "underside 0.5")
expect_hits(rr, [([3.0, 1.0, 1.05], None), ([3.0, 1.0, 0.95], 'wall')], f"underside 0.5 lowers the cut to {CUT - 0.2:.4f}")
print(f"ok: underside 0.5: the wall stops at {CUT - 0.2:.4f}")
print("PASS 04")
