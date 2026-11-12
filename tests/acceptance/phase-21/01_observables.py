#!/usr/bin/env python3
# The phase-21 observables exist before any feature: arch.inspect reports "schedules", "sheets" and "sheetSet"; opening
# and cabinet rows carry a "number" key; view rows carry kind, room, wall, patterns, indicators, labels and notes; and
# the linework of an elevation names casework as what it is (kind "cabinet", id "cb0"), not as wall w0 as it did.
# A stub fails: a missing key, a non-list, or a cabinet line still tagged as a wall.
# needs: p21-observables-1, p21-observables-2, p21-observables-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

# An elevation looking east, scoped to the strip in front of the cabinet on w5 (north-west face, y 5.2..5.8).
rr = sh_run_ok(HOUSE + [SH_WINDOWS[0], SH_CAB,
                        cmd('elevation', {"look": [1, 0], "scope": [[5.0, 5.0], [5.95, 6.0]]}), INSPECT21],
               "the observables set-up")
insp = inspected(rr)
if sh_list(insp, 'schedules') != []:
    fail(f"a drawing without schedules reports schedules: {insp.get('schedules')}")
if sh_list(insp, 'sheets') != []:
    fail(f"a drawing without sheets reports sheets: {insp.get('sheets')}")
ss = insp.get('sheetSet')
if not isinstance(ss, dict) or not isinstance(ss.get('project'), str) or not isinstance(ss.get('date'), str):
    fail(f"arch.inspect has no sheetSet {{project, date}} of strings: {ss!r}")
for key, rows in (('openings', objects_named(insp, 'openings', 'openings')), ('cabinets', objects_named(insp, 'cabinets', 'cabinets'))):
    if not rows:
        fail(f"the set-up has {key}")
    for r in rows:
        if 'number' not in r or not (r['number'] is None or isinstance(r['number'], str)):
            fail(f"{key} row {r.get('id')} has no 'number' (a string or null): {json.dumps(r)[:200]}")
views = sh_list(insp, 'views')
if len(views) != 1:
    fail(f"one view: {[v.get('id') for v in views]}")
v = views[0]
if v.get('kind') != 'elevation' or 'room' not in v or 'wall' not in v:
    fail(f"the view row reports kind 'elevation' and the keys room and wall: {json.dumps(v)[:300]}")
for key in ('patterns', 'indicators', 'labels', 'notes'):
    sh_items(v, key)
cab = sh_items(v, 'lines', 'cb0', 'cabinet')
if not cab:
    fail(f"no elevation line names the cabinet (kind cabinet, id cb0): "
         f"{sorted(set((l.get('kind'), l.get('id')) for l in sh_items(v, 'lines')))}")
u0, v0, u1, v1 = sh_box(cab, 'cb0 lines')
if abs((u1 - u0) - 0.6) > SH_TOL:
    fail(f"the cabinet's lines span {u1 - u0:.4f} m across, want its 0.6 m width")
stray = [l for l in sh_items(v, 'lines', 'w0')]
if stray:
    fail(f"the scoped elevation still tags {len(stray)} lines as wall w0 (the south wall is out of scope): {stray[:3]}")
print(f"ok: schedules, sheets and sheetSet reported; numbers on {len(objects_named(insp, 'openings', 'o'))} opening and "
      f"{len(objects_named(insp, 'cabinets', 'c'))} cabinet rows; the view's new lists; {len(cab)} cabinet lines tagged cb0")
print("PASS 01")
