#!/usr/bin/env python3
# Room moldings, the brief's numbers: the molding room (centre lines (0,0) (4.1,0) (4.1,3.1) (0,3.1), 0.1 thick,
# interior 4.0 x 3.0, one 0.9 m door) with base, crown and chair rail: base length 14.0 minus the door's trim width on
# the room's face (the door's frame 0.06 and no casing on that face, 0.96, as arch.inspect reports it), crown 14.0,
# chair rail as base (± 1e-6, along the wall faces); a probe 0.01 off the south face (y 0.06) at z 0.05 hits part
# `molding` of room r0, nothing in the door gap, the crown at z 2.35 and the chair rail at z 0.93. A window on the east
# wall cuts the chair rail by its trim width but not the base or the crown; a door casing on the right face raises
# trimWidth.right to 1.06 and leaves the room-side 0.96; an empty list clears the moldings and undo restores them.
# A stub fails: no moldings rows, trim width 0, lengths of 14.0 for the base, no molding probes.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-molding-kit-1, p18-molding-kit-2, p18-room-moldings
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

PROBES = probe_cmd([[1.0, 0.06, 0.05], [2.05, 0.06, 0.05], [1.0, 0.06, 2.35], [1.0, 0.06, 0.93]])


def trim_width(insp, oid, side):
    row = by_id(openings_rows(insp), oid, 'arch.inspect openings')
    tw = row.get('trimWidth')
    if not isinstance(tw, dict) or side not in tw:
        fail(f"opening {oid} reports trimWidth.{side}: {json.dumps(row)[:300]}")
    return tw[side]


def door_id(insp):
    doors = [o for o in openings_rows(insp) if o.get('kind') == 'door']
    if len(doors) != 1:
        fail(f"one door: {[(o.get('id'), o.get('kind')) for o in openings_rows(insp)]}")
    return doors[0]['id']


# 1. The numbers.
rr = run_ok(MOLD_ROOM + [MOLDINGS_ALL, INSPECT18, PROBES], "the molding room with base, crown and chair")
insp = inspected(rr)
d = door_id(insp)
tw = num(trim_width(insp, d, 'left'), "the door's trimWidth.left")
if tw <= 0.9 + 1e-6:
    fail(f"the door's trim on the room's face is wider than the door (frame and casings): {tw}")
near(tw, 0.96, 1e-9, "the door's trim width on the room's face (0.9 + frame 0.06, no casing)")
base, crown, chair = molding_of(insp, 'r0', 'base'), molding_of(insp, 'r0', 'crown'), molding_of(insp, 'r0', 'chair')
near(base.get('length'), 14.0 - tw, 1e-6, "the base length (14.0 minus the door's trim)")
near(crown.get('length'), 14.0, 1e-6, "the crown length")
near(chair.get('length'), 14.0 - tw, 1e-6, "the chair rail length (as the base)")
cruns, bruns = crown.get('runs') or [], base.get('runs') or []
if len(cruns) != 1 or cruns[0].get('closed') is not True:
    fail(f"the crown is one closed run: {json.dumps(cruns)[:300]}")
if len(bruns) != 1 or bruns[0].get('closed') is not False:
    fail(f"the base is one open run broken at the door: {json.dumps(bruns)[:300]}")
pts = bruns[0].get('points') or []
ends = sorted([pts[0][0], pts[-1][0]]) if len(pts) >= 2 else []
if not (ends and abs(ends[0] - (2.05 - tw / 2)) < 1e-6 and abs(ends[1] - (2.05 + tw / 2)) < 1e-6):
    fail(f"the base run starts and ends at the door's trim edges x = {2.05 - tw / 2:.3f} and {2.05 + tw / 2:.3f}: {pts[:2]} ... {pts[-2:]}")
for m, zw, what in ((base, [0.0, 0.09], 'base'), (chair, [0.9, 0.964], 'chair'), (crown, [2.3, 2.4], 'crown')):
    z = m.get('z')
    if not (isinstance(z, list) and len(z) == 2):
        fail(f"the {what} row carries z [bottom, top]: {z!r}")
    near(z[0], zw[0], 1e-6, f"the {what}'s bottom")
    near(z[1], zw[1], 1e-6, f"the {what}'s top")
got = probed(rr, [[1.0, 0.06, 0.05], [2.05, 0.06, 0.05], [1.0, 0.06, 2.35], [1.0, 0.06, 0.93]], what="moldings")
hit(got[0], 'molding', 'r0', "0.01 off the south face at z 0.05 (the base)")
nohit(got[1], "the door gap at z 0.05")
hit(got[2], 'molding', 'r0', "the crown at z 2.35")
hit(got[3], 'molding', 'r0', "the chair rail at z 0.93")
print(f"ok: trim {tw}; base {14.0 - tw:.4f}, crown 14.0, chair {14.0 - tw:.4f}; probes hit the base, crown and chair")

# 2. A window cuts the chair rail only.
WIN = cmd('window', {"wall": "w1", "offset": 1.55, "width": 1.2, "height": 1.2, "sill": 0.9})
rr = run_ok(MOLD_ROOM + [WIN, MOLDINGS_ALL, INSPECT18], "a window on the east wall")
insp = inspected(rr)
win = [o for o in openings_rows(insp) if o.get('kind') == 'window']
if len(win) != 1:
    fail("one window")
tww = num(trim_width(insp, win[0]['id'], 'left'), "the window's trimWidth.left")
near(tww, 1.26, 1e-9, "the window's trim on the room's face (1.2 + 0.06)")
near(molding_of(insp, 'r0', 'base').get('length'), 14.0 - tw, 1e-6, "the base with a window (unchanged)")
near(molding_of(insp, 'r0', 'crown').get('length'), 14.0, 1e-6, "the crown with a window (unchanged)")
near(molding_of(insp, 'r0', 'chair').get('length'), 14.0 - tw - tww, 1e-6, "the chair rail cut by the window's trim")
print("ok: a window (sill 0.9) cuts the chair rail by 1.26 and leaves the base and the crown")

# 3. A casing on the right face only.
CASED = [MOLD_ROOM[0], cmd('door', {"wall": "w0", "offset": 2.05, "width": 0.9, "casing": 0.05}), 'roomdetect {}']
rr = run_ok(CASED + [MOLDINGS_ALL, INSPECT18], "a door with a 0.05 casing")
insp = inspected(rr)
d = door_id(insp)
near(trim_width(insp, d, 'left'), 0.96, 1e-9, "the room-side trim stays 0.96 (the casing is on the right face)")
near(trim_width(insp, d, 'right'), 1.06, 1e-9, "the right-face trim with a 0.05 casing")
near(molding_of(insp, 'r0', 'base').get('length'), 14.0 - 0.96, 1e-6, "the base uses the room-side trim")
print("ok: trimWidth left 0.96 / right 1.06 with a casing; the base uses the left")

# 4. Clearing and undo.
rr = run_ok(MOLD_ROOM + [MOLDINGS_ALL, cmd('room.moldings', {"room": "r0", "moldings": []}), INSPECT18, PROBES, 'undo {}', INSPECT18, PROBES],
            "clear and undo")
if molding_rows(inspected(rr, 0), 'r0'):
    fail("an empty list clears the room's moldings")
for g in probed(rr, [[1.0, 0.06, 0.05], [2.05, 0.06, 0.05], [1.0, 0.06, 2.35], [1.0, 0.06, 0.93]], nth=0, what="after clearing"):
    nohit(g, "after clearing")
if len(molding_rows(inspected(rr, 1), 'r0')) != 3:
    fail("undo restores the three moldings")
hit(probed(rr, [[1.0, 0.06, 0.05], [2.05, 0.06, 0.05], [1.0, 0.06, 2.35], [1.0, 0.06, 0.93]], nth=1, what="after undo")[0], 'molding', 'r0', "the base after undo")
print("ok: cleared, then restored by undo")
print("PASS 03")
