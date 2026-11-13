#!/usr/bin/env python3
# Brief: a text placed on a linked view keeps its view coordinates after a wall moves and the view regenerates.
# On the north-west room's interior elevation of w5: a free note at (u, v) = (-6.5, 1.5) keeps exactly those
# coordinates when w5 moves to x = 5.5 (two stretches and a re-detect: the view regenerates, its room is found again)
# and is placed by the new extent (insert + at - extent min); a note attached to the cabinet's top-left corner follows
# the cabinet when it moves 1 m along the wall; a dimension attached to the cabinet's bottom corners reads 0.6 and
# follows it too; the notes are stored with the view (a DXF round trip keeps them).
# A stub fails: notes lost on regeneration, attached notes that stay put, notes not saved.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-view-plumbing, p21-interior-elevation, p21-view-notes
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
dxf = os.path.join(tmp, 'notes.dxf')
R = house_rooms()
NW = R['NW']
SET = HOUSE + [SH_CAB, cmd('elevation', {"room": NW, "wall": "w5"}),
               cmd('view.note', {"view": "v0", "text": "PAINT GRADE", "at": [-6.5, 1.5]}),
               cmd('view.note', {"view": "v0", "text": "BASE", "attach": {"object": "cb0", "corner": "topLeft"},
                                 "offset": [0, 0.1]}),
               cmd('view.dim', {"view": "v0", "a": {"object": "cb0", "corner": "bottomLeft"},
                                "b": {"object": "cb0", "corner": "bottomRight"}})]


def notes(insp, what):
    v = sh_view(insp, 'v0')
    ns = {n.get('text') if n.get('kind') != 'dimension' else 'DIM': n for n in sh_items(v, 'notes')}
    if set(ns) != {'PAINT GRADE', 'BASE', 'DIM'}:
        fail(f"{what}: the view keeps its three notes: {[(n.get('id'), n.get('kind'), n.get('text')) for n in sh_items(v, 'notes')]}")
    return v, ns


def placed_ok(v, n, what):
    at, pl = sh_pt(n.get('at'), 'at'), sh_pt(n.get('placed'), 'placed')
    ins, ext = sh_pt(v.get('insert'), 'insert'), v.get('extent')
    want = (ins[0] + at[0] - ext[0], ins[1] + at[1] - ext[1])
    if abs(pl[0] - want[0]) > 1e-6 or abs(pl[1] - want[1]) > 1e-6:
        fail(f"{what}: placed {pl} is insert + at - extent min = {want}")


rr = sh_run_ok(SET + [INSPECT21], "the notes")
insp = inspected(rr)
va, na = notes(insp, "as placed")
top = num(by_id(objects_named(insp, 'cabinets', 'cabinets'), 'cb0', 'cabinets').get('counterTop'), 'counterTop')
free = sh_pt(na['PAINT GRADE'].get('at'), 'free note')
if free != (-6.5, 1.5):
    fail(f"the free note is at (-6.5, 1.5): {free}")
base = sh_pt(na['BASE'].get('at'), 'attached note')
if abs(base[0] - (-5.8)) > SH_TOL or abs(base[1] - (top + 0.1)) > SH_TOL:
    fail(f"the note attached to cb0's top-left is at (-5.8, {top} + 0.1): {base}")
if abs(num(na['DIM'].get('value'), 'dimension value') - 0.6) > SH_TOL:
    fail(f"the dimension across cb0's bottom corners reads 0.6: {na['DIM']}")
placed_ok(va, na['PAINT GRADE'], "before")

# The wall moves: w5 to x = 5.5, rooms re-detected; the view regenerates and finds its room again.
MOVE = [cmd('arch.wall.stretch', {"wall": "w5", "end": "start", "to": [5.5, 3]}),
        cmd('arch.wall.stretch', {"wall": "w5", "end": "end", "to": [5.5, 8]}), 'roomdetect {}']
rr = sh_run_ok(SET + MOVE + [INSPECT21], "the wall move")
insp = inspected(rr)
vb, nb = notes(insp, "after the wall moved")
if vb.get('revision') == va.get('revision'):
    fail(f"the view regenerated after the wall moved (revision {vb.get('revision')})")
if vb.get('room') != room_at(insp, 3, 5.5) or vb.get('error'):
    fail(f"the view follows its re-detected room {room_at(insp, 3, 5.5)}: room {vb.get('room')}, error {vb.get('error')}")
if sh_pt(nb['PAINT GRADE'].get('at'), 'free note') != (-6.5, 1.5):
    fail(f"the free note keeps its view coordinates (-6.5, 1.5): {nb['PAINT GRADE'].get('at')}")
placed_ok(vb, nb['PAINT GRADE'], "after the wall moved")
for l in sh_items(vb, 'lines'):
    for k in ('a', 'b'):
        if not 3.05 - SH_TOL <= -sh_pt(l.get(k), 'p')[0] <= 7.9 + SH_TOL:
            fail(f"after the move the view still shows only the room: {l}")

# The cabinet moves 1 m along w5 (y 4.2..4.8): the attached note and the dimension follow, the free note stays.
# cabinet.edit places along a wall as `cabinet` does: an `offset` is given with its `wall`.
rr = sh_run_ok(SET + [cmd('cabinet.edit', {"id": "cb0", "wall": "w5", "offset": 1.5}), INSPECT21], "the cabinet move")
vc, nc = notes(inspected(rr), "after the cabinet moved")
base = sh_pt(nc['BASE'].get('at'), 'attached note')
if abs(base[0] - (-4.8)) > SH_TOL or abs(base[1] - (top + 0.1)) > SH_TOL:
    fail(f"the attached note follows cb0's top-left to (-4.8, {top + 0.1}): {base}")
a = sh_pt(nc['DIM'].get('at'), 'dimension a')
if abs(num(nc['DIM'].get('value'), 'dimension') - 0.6) > SH_TOL or abs(a[0] + 4.8) > SH_TOL:
    fail(f"the dimension follows cb0 (from u = -4.8, reading 0.6): {nc['DIM']}")
if sh_pt(nc['PAINT GRADE'].get('at'), 'free note') != (-6.5, 1.5):
    fail(f"the free note stays at (-6.5, 1.5): {nc['PAINT GRADE'].get('at')}")

# Stored with the view: a DXF round trip keeps them.
sh_run_ok(SET, "saving the notes", save=dxf)
rc, rr, so, se = run([INSPECT21], file=dxf)
vd, nd = notes(inspected(rr), "after a DXF round trip")
if sh_pt(nd['PAINT GRADE'].get('at'), 'free note') != (-6.5, 1.5):
    fail(f"the free note survives the round trip at (-6.5, 1.5): {nd['PAINT GRADE']}")
print("ok: the free note keeps (-6.5, 1.5) through the wall move and regeneration; the attached note and dimension follow "
      "the cabinet; the notes survive a DXF round trip")
print("PASS 07")
