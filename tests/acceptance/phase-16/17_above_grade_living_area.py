#!/usr/bin/env python3
# The above-grade area follows Chief's living-area convention (the operator's call on the phase-15 checkpoint's decision
# 3): Open Below, Garage, Porch and Deck rooms never count. The phase-1 house on Level 1 at 0 (no terrain: above grade):
# rooms S 27.93, NW 28.3725 and NE 18.6725 m² (interior), 74.975 m² in all. Retyping NE a garage leaves 56.3025, NW a
# porch then leaves 27.93, S a deck then leaves 0; retyping NW Open Below alone leaves 46.6025. The level row's
# aboveGradeArea agrees with the total each time.
# A stub fails: today every room on an above-grade floor level counts (74.975 throughout).
# needs: p16-rw-above-grade-living
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
INSPECT = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True})
retype = lambda k, t: cmd('room', {"id": R[k], "type": t})
rr = run_ok(HOUSE + [INSPECT, retype('NE', 'garage'), INSPECT, retype('NW', 'porch'), INSPECT, retype('S', 'deck'), INSPECT],
            "the phase-1 house, its rooms retyped garage, porch and deck in turn")
for k, (want, what) in enumerate([(74.975, "three living rooms"), (56.3025, "NE a garage"), (27.93, "NW a porch too"),
                                  (0.0, "S a deck too")]):
    insp = inspected(rr, k)
    near(insp.get('aboveGradeArea'), want, 1e-6, f"aboveGradeArea with {what}")
    near(level_row(insp, 0).get('aboveGradeArea'), want, 1e-6, f"Level 1's aboveGradeArea with {what}")
rr = run_ok(HOUSE + [retype('NW', 'openBelow'), INSPECT], "the phase-1 house with NW Open Below")
near(inspected(rr).get('aboveGradeArea'), 46.6025, 1e-6, "aboveGradeArea with NW Open Below")
print("ok: aboveGradeArea 74.975 -> 56.3025 (garage) -> 27.93 (porch) -> 0 (deck); 46.6025 with an Open Below room")
print("PASS 17")
