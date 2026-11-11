#!/usr/bin/env python3
# Countertop shapes (the brief's arcs with a sagitta and a radius grip, edge profiles, waterfall ends, and the numbers:
# "setting that edge to an arc with sagitta s adds the circular segment's area (chord L, R = (L²/4 + s²)/(2s), area
# R²·acos((R − s)/R) − (R − s)·√(2Rs − s²)) ± 1e-6. A waterfall end is hit by a probe 0.3 m above the floor inside its
# thickness"). The check-07 island detached, its front edge (L 1.8, now at y 2.925) moved 0.3: sagitta 0.2 adds the
# segment (R 2.125), the radius form gives the same sagitta, a negative sagitta subtracts; the end edge at x 3.35 turned
# into a waterfall is a panel of the counter's 0.038 thickness from the floor (outer face 1 mm proud of the edge line:
# x 3.313..3.351) hit at z 0.3 under the seating overhang; a waterfall on an arc is refused; a bullnose keeps the outline
# and rounds the edge: the corner a square edge fills is empty and the nose at mid-thickness is counter.
# A stub fails: no segment area, no panel, a square slab under a "bullnose".
# needs: p18-countertops, p18-countertop-shapes
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

ISLAND = cmd('cabinet', {"kind": "base", "at": [2.0, 2.0], "rotation": 0, "widths": [0.9, 0.9]})
DETACH = cmd('countertop.detach', {"run": "cb1"})
insp = inspected(run_ok(KITCHEN + [ISLAND, DETACH, INSPECT18], "the detached island"))
CT = countertops_of(insp, False)[0]['id'] if countertops_of(insp, False) else fail("a custom countertop after the detach")
row = ct_row(insp, CT)
I, _ = edge_index(row, lambda mx, my: abs(my - 2.625) < 1e-6, "the front edge")
MOVE = cmd('countertop.edit', {"id": CT, "edge": I, "move": 0.3})
BASE = KITCHEN + [ISLAND, DETACH, MOVE]
row = ct_row(inspected(run_ok(BASE + [INSPECT18], "moved")), CT)
J, _ = edge_index(row, lambda mx, my: abs(mx - 3.35) < 1e-6, "the end edge at x 3.35")
A0 = 1.665


def edit(**kw):
    p = {"id": CT}
    p.update(kw)
    return cmd('countertop.edit', p)


# 1. Arcs.
rr = run_ok(BASE + [edit(edge=I, sagitta=0.2), INSPECT18, probe_cmd([[2.45, 3.075, 0.9], [2.45, 3.15, 0.9]]),
                    edit(edge=I, sagitta=0), edit(edge=I, radius=2.125), INSPECT18,
                    edit(edge=I, sagitta=-0.1), INSPECT18], "arcs")
near(ct_row(inspected(rr, 0), CT).get('area'), A0 + segment_area(1.8, 0.2), 1e-6, "the area with a 0.2 sagitta on the 1.8 edge")
e = [x for x in ct_row(inspected(rr, 0), CT).get('edges') or [] if x.get('index') == I][0]
near(e.get('sagitta'), 0.2, 1e-9, "the edge's sagitta")
near(e.get('radius'), 2.125, 1e-9, "the edge's radius ((L²/4 + s²)/(2s))")
g = probed(rr, [[2.45, 3.075, 0.9], [2.45, 3.15, 0.9]], what="arc")
hit(g[0], 'counter', CT, "inside the bulge (apex y 3.125)")
nohit(g[1], "beyond the bulge")
e = [x for x in ct_row(inspected(rr, 1), CT).get('edges') or [] if x.get('index') == I][0]
near(e.get('sagitta'), 0.2, 1e-9, "radius 2.125 sets sagitta 0.2 (the minor arc)")
near(ct_row(inspected(rr, 2), CT).get('area'), A0 - segment_area(1.8, 0.1), 1e-6, "an inward sagitta subtracts the segment")
print("ok: segment areas and the radius form")

# 2. The waterfall.
W = [[3.331, 2.8, 0.3], [3.316, 2.8, 0.3], [3.310, 2.8, 0.3]]
rr = run_ok(BASE + [edit(edge=J, waterfall=True), INSPECT18, probe_cmd(W), edit(edge=J, waterfall=False), probe_cmd(W[:1])], "a waterfall end")
e = [x for x in ct_row(inspected(rr), CT).get('edges') or [] if x.get('index') == J][0]
if e.get('waterfall') is not True:
    fail(f"edge {J} is a waterfall: {e}")
g = probed(rr, W, nth=0, what="waterfall")
hit(g[0], 'counter', CT, "the waterfall panel at z 0.3 (x 3.331, under the seating overhang)")
hit(g[1], 'counter', CT, "3 mm inside the panel's inner face (x 3.316; the panel is the counter's 0.038 thickness)")
nohit(g[2], "x 3.310, inside the edge line past the panel's thickness")
nohit(probed(rr, W[:1], nth=1, what="waterfall off")[0], "the panel's place once the waterfall is off")
refused(BASE + [edit(edge=I, sagitta=0.2), edit(edge=I, waterfall=True)], "a waterfall on an arc edge")
print("ok: a 0.038 panel from the floor; refused on an arc")

# 3. A bullnose.
B = [[2.45, 2.923, 0.912], [2.45, 2.923, 0.895]]
rr = run_ok(BASE + [edit(edge=I, profile="bullnose"), INSPECT18, probe_cmd(B)], "a bullnose front edge")
row = ct_row(inspected(rr), CT)
e = [x for x in row.get('edges') or [] if x.get('index') == I][0]
if e.get('profile') != 'bullnose':
    fail(f"edge {I}'s profile is bullnose: {e}")
near(row.get('area'), A0, 1e-9, "the outline's area is unchanged by a profile")
g = probed(rr, B, what="bullnose")
nohit(g[0], "the top-front corner a square edge fills (2 mm in, 2 mm down: 0.024 from the nose's centre, radius 0.019)")
hit(g[1], 'counter', CT, "the nose at mid-thickness (0.017 from the centre)")
print("ok: a bullnose rounds the edge and keeps the outline")
print("PASS 08")
