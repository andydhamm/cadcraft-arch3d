#!/usr/bin/env python3
# Cabinet construction and face layouts (the brief's framed / frameless, overlay / inset, `cabinet.face`, door pairs
# above 0.686 m, and the numbers: "a front split into 3 equal drawers gives three heights equal to 1e-9 that, with the
# reveals, sum to the face height; a 0.762 m wall cabinet gets two doors and a 0.610 m one gets one"). A 0.6 base
# cabinet's face (0.776 high over the toe, 3 mm reveals) split down into 3 drawers; a 0.762 wall cabinet gets two doors
# and a 0.610 one door, forced single / double on demand; a split across halves a row; a framed inset base cabinet has a
# smaller face (0.776 - 2 x 0.041), a 0.044 reveal and a stile a probe finds; bad face edits are refused.
# A stub fails: no face row, unequal drawers, one door on 0.762, no frame.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-cabinet-fronts-1, p18-cabinet-fronts-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

B0 = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.6})
F1 = cmd('cabinet.face', {"id": "cb0", "rows": [{"kind": "drawer"}]})
F3 = cmd('cabinet.face', {"id": "cb0", "row": 0, "split": "down", "count": 3})


def cab(insp, cid):
    return by_id(cabinets_of(insp), cid, 'arch.inspect cabinets')


def doors_in(insp, cid):
    return [p for p in (cab(insp, cid).get('frontParts') or []) if p.get('kind') == 'door']


# 1. Three equal drawers.
rr = run_ok(KITCHEN + [B0, F1, F3, INSPECT18], "a base cabinet split into three drawers")
face = face_of(cab(inspected(rr), 'cb0'), "three drawers")
rows = face['rows']
if len(rows) != 3 or any(r.get('kind') != 'drawer' for r in rows):
    fail(f"three drawer rows: {json.dumps(rows)[:300]}")
hs = [num(r.get('height'), 'row height') for r in rows]
if max(hs) - min(hs) > 1e-9:
    fail(f"the three drawer heights are equal within 1e-9: {hs}")
for r in rows:
    items = r.get('items') or []
    if len(items) != 1:
        fail(f"one item per drawer row: {r}")
    rect = items[0].get('rect')
    near(rect[3] - rect[1], r['height'], 1e-9, "an item's v1 - v0 equals its row height")
H, reveal = num(face.get('height'), 'face height'), num(face.get('reveal'), 'reveal')
near(H, 0.776, 1e-12, "the face height of a 0.876 base cabinet over its toe")
near(reveal, 0.003, 1e-12, "the reveal of a frameless overlay face")
near(sum(hs) + 2 * reveal, H, 1e-9, "3 h + 2 reveals = the face height")
print(f"ok: three drawers of {hs[0]:.6f}, reveal 0.003, face 0.776")

# 2. Door pairs by width; forced single and double; a split across.
W762 = cmd('cabinet', {"kind": "wall", "wall": "w1", "offset": 1.0, "width": 0.762})
W610 = cmd('cabinet', {"kind": "wall", "wall": "w1", "offset": 2.5, "width": 0.610})
rr = run_ok(KITCHEN + [B0, F1, F3, W762, W610, INSPECT18], "two wall cabinets")
insp = inspected(rr)
wide = [c for c in cabinets_of(insp) if abs(num(c.get('width'), 'w') - 0.762) < 1e-9]
narrow = [c for c in cabinets_of(insp) if abs(num(c.get('width'), 'w') - 0.610) < 1e-9]
if len(wide) != 1 or len(narrow) != 1:
    fail("one 0.762 and one 0.610 wall cabinet")
WID, NID = wide[0]['id'], narrow[0]['id']
if len(doors_in(insp, WID)) != 2:
    fail(f"a 0.762 wall cabinet gets two doors: {doors_in(insp, WID)}")
if len(doors_in(insp, NID)) != 1:
    fail(f"a 0.610 wall cabinet gets one door: {doors_in(insp, NID)}")
rr = run_ok(KITCHEN + [B0, F1, F3, W762, W610, cmd('cabinet.face', {"id": NID, "doors": "double"}), cmd('cabinet.face', {"id": WID, "doors": "single"}),
                       cmd('cabinet.face', {"id": "cb0", "row": 1, "split": "across", "count": 2}), INSPECT18], "forced doors, split across")
insp = inspected(rr)
if len(doors_in(insp, NID)) != 2 or len(doors_in(insp, WID)) != 1:
    fail(f"doors forced double on {NID} ({len(doors_in(insp, NID))}) and single on {WID} ({len(doors_in(insp, WID))})")
items = face_of(cab(insp, 'cb0'), "split across")['rows'][1].get('items') or []
if len(items) != 2 or abs((items[0]['rect'][2] - items[0]['rect'][0]) - (items[1]['rect'][2] - items[1]['rect'][0])) > 1e-9:
    fail(f"row 1 split across into two equal items: {items}")
print("ok: 0.762 -> 2 doors, 0.610 -> 1; forced single/double; split across")

# 3. A framed inset cabinet.
FI = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "width": 0.6, "construction": "framed", "mount": "inset"})
rr = run_ok(KITCHEN + [FI, INSPECT18], "a framed inset base cabinet")
insp = inspected(rr)
c = cabinets_of(insp)[0]
FID = c['id']
if c.get('construction') != 'framed' or c.get('mount') != 'inset':
    fail(f"construction framed, mount inset echoed: {c.get('construction')!r} {c.get('mount')!r}")
rr = run_ok(KITCHEN + [FI, cmd('cabinet.face', {"id": FID, "equalize": True}), INSPECT18, probe_cmd([[2.619, 0.595, 0.5]])], "its face")
face = face_of(cab(inspected(rr), FID), "framed inset")
near(face.get('height'), 0.776 - 2 * (0.038 + 0.003), 1e-9, "the inset face inside the 0.038 frame and 0.003 gaps")
near(face.get('reveal'), 0.038 + 2 * 0.003, 1e-9, "the mid rail between rows")
for r in face['rows']:
    for it in r.get('items') or []:
        if it['rect'][0] < -0.3 + 0.041 - 1e-9:
            fail(f"every front item lies inside the frame (u0 >= -0.259): {it}")
hit(probed(rr, [[2.619, 0.595, 0.5]], what="stile")[0], 'cabinet', FID, "the left stile (x 2.6..2.638, y 0.581..0.6)")
print("ok: a framed inset face of 0.694 with 0.044 reveals and a stile")

# 4. Refusals.
for p, what in (({"id": "cb0", "rows": []}, "no rows"), ({"id": "cb0", "row": 9, "kind": "door"}, "a row out of range"),
                ({"id": "cb0", "rows": [{"kind": "drawer", "height": 2}]}, "a row taller than the face")):
    refused(KITCHEN + [B0, cmd('cabinet.face', p)], what)
print("ok: refusals")
print("PASS 10")
