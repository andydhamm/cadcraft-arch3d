#!/usr/bin/env python3
# Openings in detail, part 2: window tops. Round: a 0.9 x 1.5 window, sill 0.6, at x = 4 on the north wall has a half
# circle of radius 0.45 above its spring line 1.65, and its rough opening is cut to that shape: inside the wall's
# thickness (4.0,8.0,2.0) finds no wall (under the arc's crown, rough top 2.16) and (3.6,8.0,2.05) finds it (the rough
# arc is at 1.9664 there; a square head would reach 2.22). Segment: a 0.9 x 1.2 window, sill 0.9, rise 0.15 at x 7.5:
# the rough arc (R 0.81 about z 1.35) is at 2.0235 at x 7.05, so (7.05,8.0,2.05) is wall and (7.5,8.0,2.1) is not. Roof:
# a 1.0 m window at y 1.5 in the west gable wall, sill 1.2, under phase 4's 6/12 gable (underside 2.4 + 0.5 (y + 0.1))
# over phase 16's vaulted south room: its top follows the underside less 0.15, 3.05 at y 1.5 and 2.825 at y 1.05, so
# (0,1.5,3.0) finds no wall and (0,1.05,2.9) finds it, and its derived height is 3.05 - 0.12 - 1.2 = 1.73; roofOffset
# 0.25 lowers the top to 2.95. A roof top on an eave wall and a round top too short for its half circle are refused.
# A stub (square holes) fails (a), (b) and (c).
# needs: p16-vault-ceiling, p17-observables, p17-commands, p17-scene-plumbing, p17-grilles-doors, p17-window-tops
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

R = house_rooms()

# (a) Round.
ROUND = cmd('window', {"wall": "w2", "offset": 6.0, "width": 0.9, "height": 1.5, "sill": 0.6, "top": "round"})
rr = run_ok(HOUSE + [ROUND, INSPECT17, probe_cmd([[4.0, 8.0, 2.0], [3.6, 8.0, 2.05], [3.6, 8.0, 1.9]])], "a round-top window")
w = openings_of(inspected(rr))[0]
if w.get('top') != 'round':
    fail(f"the window row reports top round: {w.get('top')!r}")
expect_walls(rr, [([4.0, 8.0, 2.0], False), ([3.6, 8.0, 2.05], True), ([3.6, 8.0, 1.9], False)],
             "the rough opening follows the half circle (radius 0.51 about z 1.65: 1.9664 at x 3.6)")
print("ok: round top: no wall at (4,8,2.0), wall at (3.6,8,2.05), no wall at (3.6,8,1.9)")

# (b) Segment.
SEG = cmd('window', {"wall": "w2", "offset": 2.5, "width": 0.9, "height": 1.2, "sill": 0.9, "top": "segment", "topRise": 0.15})
rr = run_ok(HOUSE + [SEG, INSPECT17, probe_cmd([[7.5, 8.0, 2.1], [7.05, 8.0, 2.05], [7.05, 8.0, 1.95]])], "a segment-top window")
w = openings_of(inspected(rr))[0]
if w.get('top') != 'segment':
    fail(f"the window row reports top segment: {w.get('top')!r}")
near(w.get('topRise'), 0.15, 1e-9, "the segment's rise")
expect_walls(rr, [([7.5, 8.0, 2.1], False), ([7.05, 8.0, 2.05], True), ([7.05, 8.0, 1.95], False)],
             "the rough opening follows the arc (R 0.81 about z 1.35: 2.0235 at x 7.05, 2.16 at the centre)")
print("ok: segment top: no wall at (7.5,8,2.1), wall at (7.05,8,2.05)")

# (c) Roof top in the west gable wall (w3 runs (0,8) -> (0,0): offset 6.5 is y = 1.5).
ROOF = cmd('window', {"wall": "w3", "offset": 6.5, "width": 1.0, "sill": 1.2, "top": "roof"})
VAULT = cmd('room', {"id": R['S'], "flatCeiling": False})
rr = run_ok(HOUSE + [GABLE, VAULT, ROOF, INSPECT17, probe_cmd([[0, 1.5, 3.0], [0, 1.05, 2.9], [0, 1.5, 2.5]])], "a roof-top window")
w = openings_of(inspected(rr))[0]
if w.get('top') != 'roof':
    fail(f"the window row reports top roof: {w.get('top')!r}")
near(w.get('height'), 1.73, 1e-6, "the derived height (3.05 - 0.12 - 1.2)")
near(w.get('roofOffset'), 0.15, 1e-9, "the default roofOffset")
expect_walls(rr, [([0, 1.5, 3.0], False), ([0, 1.05, 2.9], True), ([0, 1.5, 2.5], False)],
             "the top follows the roof underside less 0.15: 3.05 at y 1.5, 2.825 at y 1.05")
rr = run_ok(HOUSE + [GABLE, VAULT, cmd('window', {"wall": "w3", "offset": 6.5, "width": 1.0, "sill": 1.2, "top": "roof", "roofOffset": 0.25}),
                     INSPECT17, probe_cmd([[0, 1.5, 3.0], [0, 1.5, 2.9]])], "roofOffset 0.25")
near(openings_of(inspected(rr))[0].get('height'), 1.63, 1e-6, "the derived height with roofOffset 0.25")
expect_walls(rr, [([0, 1.5, 3.0], True), ([0, 1.5, 2.9], False)], "roofOffset 0.25 keeps the wall down to 2.95")
print("ok: roof top: height 1.73, no wall at (0,1.5,3.0), wall at (0,1.05,2.9); roofOffset 0.25 -> wall at (0,1.5,3.0)")

# (d) Refused, nothing added.
BAD = [cmd('window', {"wall": "w0", "offset": 5, "top": "roof"}),
       cmd('window', {"wall": "w2", "offset": 2.5, "width": 0.9, "height": 0.3, "sill": 0.9, "top": "round"})]
rc, rr, so, se = run(HOUSE + [GABLE] + BAD + [INSPECT17], expect_ok=False)
errs = errors(se)
if len(errs) != len(BAD):
    fail(f"a roof top on an eave wall and a round top shorter than width/2 + 0.1 are refused ({len(BAD)} error lines): {errs}")
if openings_of(inspected(rr)):
    fail(f"the refused windows were not placed: {[(o.get('id'), o.get('top')) for o in openings_of(inspected(rr))]}")
print("ok: a roof top on the eave wall w0 and a 0.3 m round top are refused")
print("PASS 10")
