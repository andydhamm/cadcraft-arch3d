#!/usr/bin/env python3
# Molding lines (the brief's "a profile swept along a drawn 2D path at an elevation ... with mitred corners"): in an
# empty drawing `moldingline` along (1,1) (3,1) (3,2) at elevation 1.0 with the default chair-rail profile (19 x 64 mm,
# on the left of travel) is 3.0 long; probes find it 0.01 left of the path at z 1.03, not on the right, in the mitred
# corner, not beyond the corner and not above 1.064; `flip` mirrors it, `inset` 0.1 shifts the path left with mitres
# (length 2.8); a closed square with the base profile is 4.0 long and probes; delete removes it and undo restores.
# A stub fails: no moldingLine reply, length 0, no probe hits.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-molding-kit-1, p18-molding-kit-2, p18-molding-lines-1, p18-molding-lines-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

L = cmd('moldingline', {"points": [[1, 1], [3, 1], [3, 2]], "elevation": 1.0})
P1 = [[2.0, 1.01, 1.03], [2.0, 0.99, 1.03], [2.99, 1.01, 1.03], [3.01, 1.5, 1.03], [2.0, 1.01, 1.08]]
ML = made([L], 'moldingLine', "moldingline along an L")
rep = reply_with(run_ok([L], "the L"), 'moldingLine', -1, 'moldingline')
near(rep.get('length'), 3.0, 1e-9, "the L's length")

rr = run_ok([L, INSPECT18, probe_cmd(P1),
             cmd('moldingline.edit', {"id": ML, "flip": True}), probe_cmd(P1[:2]),
             cmd('moldingline.edit', {"id": ML, "flip": False, "inset": 0.1}), INSPECT18, probe_cmd([[2.0, 1.11, 1.03], [2.0, 1.05, 1.03]])],
            "an L molding line, flipped, inset")
row = by_id(molding_lines_of(inspected(rr, 0)), ML, 'arch.inspect moldingLines')
if row.get('profile') != 'molding:chair-rail':
    fail(f"the default profile is molding:chair-rail: {row.get('profile')!r}")
size = row.get('size')
if not (isinstance(size, list) and len(size) == 2):
    fail(f"the row carries the profile size: {size!r}")
near(size[0], 0.019, 1e-9, "chair rail width")
near(size[1], 0.064, 1e-9, "chair rail height")
near(row.get('length'), 3.0, 1e-9, "the inspected length")
g = probed(rr, P1, nth=0, what="the L")
hit(g[0], 'molding', ML, "0.01 left of the first leg at z 1.03")
nohit(g[1], "0.01 right of the first leg (the profile is on the left)")
hit(g[2], 'molding', ML, "inside the mitred corner")
nohit(g[3], "right of the second leg beyond the corner")
nohit(g[4], "above the profile's 0.064 top")
g = probed(rr, P1[:2], nth=1, what="flipped")
hit(g[1], 'molding', ML, "flipped: 0.01 right of the path")
nohit(g[0], "flipped: 0.01 left of the path")
near(by_id(molding_lines_of(inspected(rr, 1)), ML, 'moldingLines').get('length'), 2.8, 1e-9, "the L inset 0.1 to the left (1.9 + 0.9)")
g = probed(rr, [[2.0, 1.11, 1.03], [2.0, 1.05, 1.03]], nth=2, what="inset")
hit(g[0], 'molding', ML, "the inset line at y 1.1..1.119")
nohit(g[1], "the old line's place after the inset")
print(f"ok: {ML} is 3.0 long, mitred, flips and insets (2.8)")

# A closed square with the base profile; delete and undo.
SQ = cmd('moldingline', {"points": [[5, 5], [6, 5], [6, 6], [5, 6]], "closed": True, "elevation": 0.5, "profile": "molding:base"})
rr = run_ok([L, SQ, INSPECT18, probe_cmd([[5.5, 5.005, 0.55]])], "a closed square")
sq = [r for r in molding_lines_of(inspected(rr)) if r.get('id') != ML]
if len(sq) != 1:
    fail(f"a second molding line: {[r.get('id') for r in molding_lines_of(inspected(rr))]}")
ML1 = sq[0]['id']
near(sq[0].get('length'), 4.0, 1e-9, "the closed square's length")
hit(probed(rr, [[5.5, 5.005, 0.55]], what="square")[0], 'molding', ML1, "the base inside the square's south side")
rr = run_ok([L, SQ, cmd('moldingline.delete', {"id": ML1}), INSPECT18, probe_cmd([[5.5, 5.005, 0.55]]), 'undo {}', INSPECT18, probe_cmd([[5.5, 5.005, 0.55]])],
            "delete and undo")
if any(r.get('id') == ML1 for r in molding_lines_of(inspected(rr, 0))):
    fail("moldingline.delete removes the line")
nohit(probed(rr, [[5.5, 5.005, 0.55]], nth=0, what="deleted")[0], "the deleted square")
if not any(r.get('id') == ML1 for r in molding_lines_of(inspected(rr, 1))):
    fail("undo restores the deleted line")
hit(probed(rr, [[5.5, 5.005, 0.55]], nth=1, what="undone")[0], 'molding', ML1, "the square after undo")
print("ok: a closed square of 4.0; delete and undo")
print("PASS 05")
