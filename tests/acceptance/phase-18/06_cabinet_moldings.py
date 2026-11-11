#!/usr/bin/env python3
# Cabinet moldings (the brief's "crown and light rail on wall cabinets, following the box front (or the face where
# there's no door), with a size override"): two 0.6 wall cabinets on the kitchen's south wall (x 0.6..1.2 and 1.2..1.8,
# depth 0.305, bottom 1.372, top 2.134). A crown on cb0 alone runs back-left, along the door face and back-right:
# 0.305 + 0.6 + 0.305 = 1.21; with cb1 crowned too the shared return goes and each is 0.905. Probes 3 mm in front of
# the door face at z 2.18 hit part `cabinet` of cb0, nothing 0.145 out, nothing above 2.234; a 0.05 x 0.05 size override
# shrinks it; a light rail (casing profile, 64 mm) hangs under the bottom (z 1.308..1.372); an open-shelf cabinet's crown
# follows the box front (y 0.286 instead of 0.305); null clears; a base cabinet refuses a crown.
# A stub fails: no crown row, length 0, no probe hits.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-molding-kit-1, p18-molding-kit-2, p18-cabinet-moldings
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

CABS = cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.6]})
C0 = cmd('cabinet.edit', {"id": "cb0", "crown": "molding:crown"})
C1 = cmd('cabinet.edit', {"id": "cb1", "crown": "molding:crown"})
P = [[0.9, 0.308, 2.18], [0.9, 0.45, 2.18], [0.9, 0.308, 2.25]]


def crown_of(insp, cid):
    row = by_id(cabinets_of(insp), cid, 'arch.inspect cabinets')
    c = row.get('crown')
    if not isinstance(c, dict):
        fail(f"cabinet {cid} has a crown row: {json.dumps(row)[:300]}")
    return c


rr = run_ok(KITCHEN + [CABS, C0, INSPECT18, probe_cmd(P), C1, INSPECT18], "two wall cabinets, one then two crowns")
i0, i1 = inspected(rr, 0), inspected(rr, 1)
cab = by_id(cabinets_of(i0), 'cb0', 'cabinets')
near(num(cab.get('elevation'), 'elevation') + num(cab.get('height'), 'height'), 2.134, 1e-6, "cb0's top")
near(crown_of(i0, 'cb0').get('length'), 1.21, 1e-9, "cb0's crown alone (0.305 + 0.6 + 0.305)")
if by_id(cabinets_of(i0), 'cb1', 'cabinets').get('crown') is not None:
    fail("cb1 has no crown yet")
near(crown_of(i0, 'cb0').get('z'), 2.134, 1e-9, "the crown's bottom sits on the cabinet top")
g = probed(rr, P, what="crown")
hit(g[0], 'cabinet', 'cb0', "the crown 3 mm in front of the door face at z 2.18")
nohit(g[1], "0.145 out from the door face (the crown reaches 0.1)")
nohit(g[2], "above the crown's top 2.234")
near(crown_of(i1, 'cb0').get('length'), 0.905, 1e-9, "cb0's crown with cb1 crowned (the shared return goes)")
near(crown_of(i1, 'cb1').get('length'), 0.905, 1e-9, "cb1's crown")
print("ok: crown 1.21 alone, 0.905 each in a pair; probes on the door face")

SIZE = cmd('cabinet.edit', {"id": "cb0", "crownSize": [0.05, 0.05]})
RAIL = cmd('cabinet.edit', {"id": "cb0", "lightRail": "molding:casing"})
OPEN = cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 3.0, "width": 0.6, "fronts": "open", "crown": "molding:crown"})
rr = run_ok(KITCHEN + [CABS, C0, SIZE, probe_cmd([[0.9, 0.308, 2.2], [0.9, 0.3065, 2.14]]),
                       RAIL, INSPECT18, probe_cmd([[0.9, 0.308, 1.34]]),
                       OPEN, INSPECT18, probe_cmd([[2.9, 0.289, 2.18]])], "size override, light rail, open cabinet")
g = probed(rr, [[0.9, 0.308, 2.2], [0.9, 0.3065, 2.14]], nth=0, what="size override")
nohit(g[0], "above a 0.05 x 0.05 crown (2.134..2.184)")
hit(g[1], 'cabinet', 'cb0', "1.5 mm in front of the face, 6 mm up: inside the scaled crown")
rail = by_id(cabinets_of(inspected(rr, 0)), 'cb0', 'cabinets').get('lightRail')
if not isinstance(rail, dict):
    fail("cb0 has a lightRail row")
near(rail.get('z'), 1.372 - 0.064, 1e-9, "the light rail's bottom (64 mm under the cabinet bottom)")
hit(probed(rr, [[0.9, 0.308, 1.34]], nth=1, what="light rail")[0], 'cabinet', 'cb0', "the light rail under the cabinet")
open_rows = [c for c in cabinets_of(inspected(rr, 1)) if c.get('id') not in ('cb0', 'cb1')]
if len(open_rows) != 1:
    fail(f"the open cabinet: {[c.get('id') for c in cabinets_of(inspected(rr, 1))]}")
oc = open_rows[0]
path = (oc.get('crown') or {}).get('path')
if not isinstance(path, list) or not any(abs(num(p[1], 'path y') - 0.286) < 1e-9 for p in path):
    fail(f"the open cabinet's crown follows the box front y = 0.286 (depth - 0.019): {path}")
hit(probed(rr, [[2.9, 0.289, 2.18]], nth=2, what="open crown")[0], 'cabinet', oc['id'], "the open cabinet's crown 3 mm in front of the box front")
print("ok: size override, light rail at 1.308, an open cabinet's crown on the box front")

rr = run_ok(KITCHEN + [CABS, C0, cmd('cabinet.edit', {"id": "cb0", "crown": None}), INSPECT18], "crown cleared")
if by_id(cabinets_of(inspected(rr)), 'cb0', 'cabinets').get('crown') is not None:
    fail("`crown: null` clears the crown")
refused(KITCHEN + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.6, "crown": "molding:crown"})], "a crown on a base cabinet")
print("ok: null clears; a base cabinet refuses a crown")
print("PASS 06")
