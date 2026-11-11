#!/usr/bin/env python3
# Shower doors and wall niches (the brief's niche number: "a niche 0.6 wide, 0.9 high, sill 1.0, depth 0.1 from the
# inside face of the phase-1 north wall at x = 4: nothing at (4.0,7.95,1.45), the wall at (4.0,8.05,1.45)"). The north
# wall w2 runs from (10,8) to (0,8), its inside face y = 7.9 is its left face: the niche is cut exactly 0.6 x 0.9 (no
# rough allowance), so 5 cm and 3 cm beside it and 3 cm above it the wall is solid, below the sill too, and a 1 mm
# back skin lines the recess; a 0.1 niche in the 0.1 m interior wall w5 is refused (not 12 mm thicker) and 0.088 is
# accepted; a shower door (style "shower", 0.7 x 1.9, frameless) in a 2.0 m glass wall probes as the door's glass,
# with the wall's glass 3 cm beside it and 5 cm above it; a shower door in a stud wall is refused.
# A stub fails: a through hole, a hole with the 0.12 rough allowance, no niche row, no shower leaf.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3, p18-shower-niche-1, p18-shower-niche-2, p18-shower-niche-3, p18-shower-niche-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

NICHE = cmd('niche', {"wall": "w2", "point": [4, 7.9], "width": 0.6, "height": 0.9, "sill": 1.0, "depth": 0.1})
PN = [[4.0, 7.95, 1.45], [4.0, 8.05, 1.45], [4.0, 7.95, 0.95], [3.65, 7.95, 1.45], [4.33, 7.95, 1.45], [4.0, 7.95, 1.93], [4.0, 7.9995, 1.45]]

rr = run_ok(HOUSE + [NICHE, INSPECT18, probe_cmd(PN)], "a niche in the north wall")
ops = [o for o in openings_rows(inspected(rr)) if o.get('kind') == 'niche']
if len(ops) != 1:
    fail(f"one niche opening row: {[(o.get('id'), o.get('kind')) for o in openings_rows(inspected(rr))]}")
n = ops[0]
near(n.get('depth'), 0.1, 1e-9, "the niche's depth")
if n.get('face') != 'left':
    fail(f"the niche's face is the room side (left of (10,8)->(0,8)): {n.get('face')!r}")
near(n.get('width'), 0.6, 1e-9, "width")
near(n.get('height'), 0.9, 1e-9, "height")
g = probed(rr, PN, what="niche")
nohit(g[0], "inside the recess (4.0, 7.95, 1.45)")
hit(g[1], 'wall', 'w2', "the wall behind the recess (4.0, 8.05, 1.45)")
hit(g[2], 'wall', 'w2', "below the sill")
hit(g[3], 'wall', 'w2', "5 cm beside the niche")
hit(g[4], 'wall', 'w2', "3 cm beside the niche (no rough allowance)")
hit(g[5], 'wall', 'w2', "3 cm above the niche's top")
hit(g[6], 'wall', 'w2', "the 1 mm back skin at the recess bottom")
print("ok: the niche is cut exactly 0.6 x 0.9 x 0.1 from the inside face")

refused(HOUSE + [cmd('niche', {"wall": "w5", "offset": 2.0, "depth": 0.1})], "a 0.1 niche in a 0.1 wall")
run_ok(HOUSE + [cmd('niche', {"wall": "w5", "offset": 2.0, "depth": 0.088})], "a 0.088 niche in a 0.1 wall")
print("ok: the 12 mm backing rule")

GW = cmd('wall', {"points": [[1, 1], [3, 1]], "type": "glass"})
SH = cmd('door', {"wall": "w6", "offset": 1.0, "style": "shower"})
PS = [[2.0, 1.0, 1.0], [2.0, 1.0, 1.95], [1.62, 1.0, 1.0]]
rr = run_ok(HOUSE + [GW, SH, INSPECT18, probe_cmd(PS)], "a shower door in a glass wall")
doors = [o for o in openings_rows(inspected(rr)) if o.get('style') == 'shower']
if len(doors) != 1 or doors[0].get('host') not in ('w6', 6):
    fail(f"one shower door on w6: {[(o.get('id'), o.get('style'), o.get('host')) for o in openings_rows(inspected(rr))]}")
near(doors[0].get('width'), 0.7, 1e-9, "a shower door's default width")
near(doors[0].get('height'), 1.9, 1e-9, "a shower door's default height")
DID = doors[0]['id']
g = probed(rr, PS, what="shower door")
hit(g[0], 'glass', DID, "the shower leaf at the door's centre")
hit(g[1], 'glass', 'w6', "the wall's glass 5 cm above the door (cut exactly 1.9 high)")
hit(g[2], 'glass', 'w6', "the wall's glass 3 cm beside the door")
refused(HOUSE + [cmd('door', {"wall": "w0", "offset": 3, "style": "shower"})], "a shower door in a stud wall")
print("ok: a frameless shower door in glass; refused in a stud wall")
print("PASS 15")
