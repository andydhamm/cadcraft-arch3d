#!/usr/bin/env python3
# Vaulted ceilings and the walls that rise to them (the brief's vault line): under the phase-4 6/12 gable, the south room
# with flatCeiling off has its ceiling on the roof underside, U = 2.4 + 0.5 (y + 0.1): 3.45 at (5,2) and 2.70 at
# (5,0.5), while the flat north-west room stays at 2.4. Every wall bounding the vaulted room rises to meet it along its
# thickness: the interior wall w4 (y 2.95..3.05) to U 3.925..3.975, so (5,3.0,3.9) is wall w4 (it stopped at 2.4
# before), and the south wall w0 from 2.4 at its outer face to 2.475 at its centre line, so (5,0.05,2.45) is wall w0.
# A gable wall keeps its phase-4 shape ((10,0.5,3.0) stays empty) and the wall between the two flat north rooms does not
# rise ((6,5,2.6) stays empty). A library ceiling item placed at (5,2) hangs from the vault: its top is at 3.45.
# A stub fails: no vault answers 2.4 at (5,2) and (5,0.5) and leaves the pendant's top at 2.4; no rise leaves both wall
# probes empty.
# needs: p16-vault-ceiling, p16-vault-walls
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW = R['S'], R['NW']
PTS = [[5, 2], [5, 0.5], [3, 5]]
RISE = [[5, 3.0, 3.9], [5, 0.05, 2.45]]          # w4 and w0 above their 2.4 m tops, under the roof underside
KEEP = [[10, 0.5, 3.0], [6, 5, 2.6]]             # above the east gable wall's top; above w5 (two flat rooms)
QUERY = cmd('build3d', {"ceiling": PTS, "probe": RISE + KEEP})

# 1. Before the vault: flat ceilings, and every wall stops at its 2.4 m top (or the gable's rafters).
rr = run_ok(HOUSE + [GABLE, QUERY], "the house under the gable")
expect_ceilings(rr, PTS, [2.4, 2.4, 2.4], "flat ceilings under the gable")
expect_hits(rr, [(p, None) for p in RISE + KEEP], "before the vault (the walls are 2.4 m high)")
print("ok: before the vault, every ceiling is flat at 2.4 and nothing is above the 2.4 m walls at the probe points")

# 2. The south room vaulted: its ceiling follows the underside, its walls rise, the rest stays as it was.
rr = run_ok(HOUSE + [GABLE, cmd('room', {"id": S, "flatCeiling": False}), QUERY,
                     cmd('library.place', {"item": "light-pendant", "at": [5, 2]}), INSPECT16], "S vaulted")
expect_ceilings(rr, PTS, [3.45, 2.70, 2.4], f"S ({S}) vaulted under the 6/12 gable, NW ({NW}) flat")
expect_hits(rr, [(RISE[0], ('wall', 'w4')), (RISE[1], ('wall', 'w0')), (KEEP[0], None), (KEEP[1], None)],
            "the walls bounding the vault rise to it (w4 to 3.925..3.975, w0 to 2.475 at y = 0.05); the gable wall w1 and "
            "w5 between the flat north rooms do not")
fid = reply_with(rr, 'furnishing', -1, 'library.place').get('furnishing')
insp = inspected(rr)
near(item_top(insp, fid), 3.45, 1e-6, "the pendant placed at (5,2) under the vault: its top")
if room_row(insp, S).get('flatCeiling') is not False:
    fail(f"S's row reports flatCeiling false: {room_row(insp, S).get('flatCeiling')!r}")
print("ok: S vaulted: 3.45 at (5,2), 2.70 at (5,0.5), NW 2.4; w4 and w0 rise to the vault, w1 and w5 do not; "
      "the pendant hangs from 3.45")
print("PASS 03")
