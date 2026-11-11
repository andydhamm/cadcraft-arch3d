#!/usr/bin/env python3
# The crown wraps soffits and trays and a soffit can opt out (the brief's "crown runs round the finished ceiling and
# wraps soffits and trays (a soffit can opt out)"). In the molding room with a crown only: a 2.0 x 0.6 soffit against the
# north face (its bottom at 2.1) makes the crown leave the wall and run round the soffit's three free sides: 14.0 - 2.0
# + 2.0 + 2 x 0.6 = 15.2, and a probe 0.01 in front of the soffit at z 2.35 hits the crown; with the soffit opted out
# the crown stops at the soffit: 12.0, no probe. Under a hip roof, a tray inset 0.6 and 0.2 deep adds a closed crown run
# round the tray's visible side: phase 16 builds a vertical tray's side as a 20 mm band just inside the outline, so the
# crown's back sits on that band's inner face, the outline (0.65..3.45 x 0.65..2.45) inset by 0.02: 14.0 + 2 x (2.76 +
# 1.76) = 23.04, and a probe at (2.0, 0.70, 2.55) hits it (0.03 out from y = 0.67 at profile height 0.05, outside the
# band strip and below the raised 20 mm ceiling slab 2.58..2.6, which answer first in scene order).
# A stub fails: a crown of 14.0 with a soffit or a tray, no probe at the soffit's front or the tray's edge.
# needs: p18-room-moldings, p18-crown-wrap, p16-soffits, p16-trays, p16-vault-ceiling
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

CROWN = cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "crown"}]})
SOFFIT = cmd('soffit', {"rect": [[1.05, 2.45], [3.05, 3.05]], "bottom": 2.1})
P_SOFFIT = probe_cmd([[2.0, 2.44, 2.35]])

# 1. A soffit on the north face: wrapped, then opted out.
S0 = made(MOLD_ROOM + [SOFFIT], 'soffit', "a soffit against the north face")
rr = run_ok(MOLD_ROOM + [CROWN, SOFFIT, INSPECT18, P_SOFFIT,
                         cmd('soffit.edit', {"id": S0, "moldings": False}), INSPECT18, P_SOFFIT], "the crown with a soffit")
near(molding_of(inspected(rr, 0), 'r0', 'crown').get('length'), 15.2, 1e-6, "the crown wrapping the soffit (14 - 2 + 2 + 1.2)")
hit(probed(rr, [[2.0, 2.44, 2.35]], nth=0, what="soffit front")[0], 'molding', 'r0', "the crown along the soffit's front at z 2.35")
near(molding_of(inspected(rr, 1), 'r0', 'crown').get('length'), 12.0, 1e-6, "the crown with the soffit opted out (14 - 2)")
nohit(probed(rr, [[2.0, 2.44, 2.35]], nth=1, what="opted out")[0], "the soffit's front once it opts out")
print("ok: crown 15.2 round the soffit, 12.0 once it opts out")

# 2. A tray under a hip roof.
TRAY = cmd('tray', {"room": "r0", "inset": 0.6, "depth": 0.2})
P_TRAY = probe_cmd([[2.0, 0.70, 2.55], [2.0, 1.5, 2.55]])
rr = run_ok(MOLD_ROOM + [CROWN, cmd('roofauto', {"pitchRise": 6, "pitchRun": 12}), TRAY, INSPECT18, P_TRAY], "the crown with a tray")
insp = inspected(rr)
tr = objects_named(insp, 'trays', 'trays') if isinstance(insp.get('trays'), list) else []
crown = molding_of(insp, 'r0', 'crown')
near(crown.get('length'), 23.04, 1e-6, "the crown with a tray (14.0 + 2 x (2.76 + 1.76))")
runs = crown.get('runs') or []
if len(runs) != 2 or not all(r.get('closed') is True for r in runs):
    fail(f"the crown is two closed runs (the walls and the tray): {json.dumps(runs)[:300]}")
got = probed(rr, [[2.0, 0.70, 2.55], [2.0, 1.5, 2.55]], what="tray crown")
hit(got[0], 'molding', 'r0', "the tray's crown at (2.0, 0.70, 2.55)")
nohit(got[1], "the tray's middle at z 2.55 (under its raised ceiling, no crown)")
print("ok: crown 23.04 with the tray, hit at its edge")
print("PASS 04")
