#!/usr/bin/env python3
# Stairs in detail and the carry-over: the phase-5 flight is built from parts, centred on `start` (y 0.5..1.5, not
# 1..2): 15 treads 25 mm thick with a 25 mm nosing (tread 1 spans x 1.975..2.254, z 0.14375..0.16875), 16 risers 19 mm
# thick behind the nosings, two stringers 38 mm thick on the sides whose band runs from 0.3 below the nosing line to
# 0.05 above it (at x 2.6: z 0.284..0.634), the landing (part stair). Under the flight is air ((2.6,1.0,0.3) was solid).
# openRisers removes every riser; stringers "centre" puts one 38 mm stringer under the middle (its top at the tread
# undersides) and lets the treads run full width; stringers "none" puts a 19 mm skirt only on a side against a wall.
# A stub (the old one-piece solid) fails: no tread/riser/stringer parts, (2.6,1.0,0.3) solid, (2.1,1.9,0.16) solid.
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-stair-flight
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

P = [[2.1, 1.0, 0.16], [1.98, 1.0, 0.16], [1.97, 1.0, 0.16], [2.01, 1.0, 0.07], [2.6, 1.0, 0.3], [2.6, 0.51, 0.45],
     [2.6, 0.51, 0.62], [2.6, 0.51, 0.66], [2.1, 0.53, 0.16], [2.1, 0.55, 0.16], [2.1, 1.45, 0.16], [2.1, 1.9, 0.16], [2.1, 1.0, 1.0]]
WANT = ['tread', 'tread', None, 'riser', None, 'stringer', 'stringer', None, 'stringer', 'tread', 'tread', None, None]
A = HOUSE2 + [stair_cmd(**NO_RAILS)]


def parts_are(rows, want, what):
    parts = parts_of(rows)
    for k, v in want.items():
        if parts.get(k) != v:
            fail(f"{what}: parts.{k} is {v}, got {parts.get(k)} in {parts}")
    return parts


# A. The default flight: parts and probes.
rr = run_ok(A + [probe_cmd(P)], "the phase-5 stair built from parts")
parts_are(rr, {"tread": 15, "riser": 16, "stringer": 2, "stair": 1}, "the default stair (two stringers, closed risers, a landing)")
if parts_of(rr).get('skirt'):
    fail(f"no skirt with two stringers: {parts_of(rr)}")
if build3d_of(rr).get('stairs') != 1:
    fail(f"build3d still counts one stair: {build3d_of(rr).get('stairs')}")
expect_hits(rr, list(zip(P, WANT)), "the default flight (treads 25 mm with a 25 mm nosing, 19 mm risers, 38 mm stringers, centred on start)")
print("ok: 15 treads, 16 risers, 2 stringers, the landing; tread 1 at x 1.975..2.254 z 0.144..0.169; stringers at y 0.5..0.538; "
      "air under the flight and beyond y 1.5")

# B. Open risers.
rr = run_ok(A + [cmd('stair.edit', {"id": "s0", "openRisers": True}), probe_cmd([[2.01, 1.0, 0.07], [2.1, 1.0, 0.16]])], "open risers")
if parts_of(rr).get('riser'):
    fail(f"open risers build no riser part: {parts_of(rr)}")
parts_are(rr, {"tread": 15}, "open risers keep the treads")
expect_hits(rr, [([2.01, 1.0, 0.07], None), ([2.1, 1.0, 0.16], 'tread')], "open risers: the riser's place is empty")
print("ok: openRisers true: 0 riser parts, treads stay")

# C. A centre stringer.
rr = run_ok(A + [cmd('stair.edit', {"id": "s0", "stringers": "centre"}), probe_cmd([[2.6, 1.0, 0.35], [2.6, 0.51, 0.45], [2.1, 0.51, 0.16]])],
            "a centre stringer")
parts_are(rr, {"stringer": 1, "tread": 15}, "one centre stringer")
expect_hits(rr, [([2.6, 1.0, 0.35], 'stringer'), ([2.6, 0.51, 0.45], None), ([2.1, 0.51, 0.16], 'tread')],
            "the centre stringer under the middle (z 0.284..0.390 at x 2.6); the sides are open and the treads run full width")
print("ok: stringers centre: one stringer at y 0.981..1.019, treads full width")

# D. No stringers beside a wall: a skirt on the wall side only (the right side at y 0.1 lies on w0's inner face).
rr = run_ok(HOUSE2 + [stair_cmd(start=[2, 0.6], stringers="none", **NO_RAILS), probe_cmd([[2.6, 0.11, 0.6], [2.6, 1.09, 0.6], [2.1, 0.6, 0.16]])],
            "no stringers, one side against the south wall")
parts_are(rr, {"skirt": 1, "tread": 15}, "a skirt on the wall side")
if parts_of(rr).get('stringer'):
    fail(f"stringers none builds no stringer: {parts_of(rr)}")
expect_hits(rr, [([2.6, 0.11, 0.6], 'skirt'), ([2.6, 1.09, 0.6], None), ([2.1, 0.6, 0.16], 'tread')],
            "the skirt (19 mm, 0.25 below .. 0.1 above the nosing line: z 0.334..0.684 at x 2.6) on the wall side; nothing on the open side")
print("ok: stringers none beside w0: one skirt at y 0.1..0.119, the open side bare")

# E. No stringers in the open: nothing on either side.
rr = run_ok(A + [cmd('stair.edit', {"id": "s0", "stringers": "none"}), probe_cmd([[2.6, 0.51, 0.45], [2.6, 1.49, 0.45]])], "no stringers in the open")
parts = parts_of(rr)
if parts.get('skirt') or parts.get('stringer'):
    fail(f"stringers none with both sides open builds no skirt and no stringer: {parts}")
expect_hits(rr, [([2.6, 0.51, 0.45], None), ([2.6, 1.49, 0.45], None)], "both sides open")
print("ok: stringers none in the open: no skirt, no stringer")
print("PASS 03")
