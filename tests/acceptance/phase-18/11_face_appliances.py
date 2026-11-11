#!/usr/bin/env python3
# Face appliances (the brief's "a microwave drawer, wall oven or warming drawer placed on a cabinet face replaces that
# front item at the appliance's height; a tall cabinet can stack two", and the number "a wall oven of height h placed
# on a tall cabinet's upper door makes a face item of height h ± 1e-6"). A 0.762 tall cabinet (face 2.034 high) with
# two door rows: a wall oven dropped from the library at elevation 1.6 replaces the upper row with an appliance row of
# height 0.737 and the lower door takes the rest (2.034 - 0.737 - 0.003); a microwave drawer on the lower row stacks a
# second appliance (0.381); a third is refused; the oven stands 5 mm proud of the door plane for a probe; a base cabinet
# takes one warming drawer and refuses a second; a 0.6 tall cabinet refuses the 0.762 oven.
# A stub fails: no appliance row, a door row of 1.0155, a flush face.
# needs: p18-cabinet-fronts-1, p18-cabinet-fronts-2, p18-face-appliances
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

TALL = cmd('cabinet', {"kind": "tall", "wall": "w0", "offset": 1.0, "width": 0.762})
TWO = cmd('cabinet.face', {"id": "cb0", "rows": [{"kind": "door"}, {"kind": "door"}]})
OVEN = cmd('library.place', {"item": "appliance:wall-oven", "at": [0.9, 0.62], "elevation": 1.6})
MICRO = cmd('cabinet.face', {"id": "cb0", "row": 1, "appliance": "appliance:microwave-drawer"})
WARM = cmd('cabinet.face', {"id": "cb0", "row": 2, "appliance": "appliance:warming-drawer"})
H = 2.134 - 0.1


def rows_of_face(insp, cid, what):
    return face_of(by_id(cabinets_of(insp), cid, 'cabinets'), what)['rows']


rr = run_ok([cmd('library', {"text": "oven"})], "the library knows the oven")
items = [r for r in rr if isinstance(r, dict) and isinstance(r.get('items'), list)]
ids = [i.get('id') for i in items[-1]['items']] if items else []
if 'appliance:wall-oven' not in ids:
    fail(f"appliance:wall-oven is a library item: {ids}")

rr = run_ok(KITCHEN + [TALL, TWO, OVEN, INSPECT18, probe_cmd([[0.9, 0.603, 1.6], [0.9, 0.603, 0.6]]), MICRO, INSPECT18], "an oven, then a microwave drawer")
rows = rows_of_face(inspected(rr, 0), 'cb0', "after the oven")
if len(rows) != 2 or rows[0].get('kind') != 'appliance' or rows[0].get('appliance') != 'appliance:wall-oven':
    fail(f"row 0 is the wall oven: {json.dumps(rows)[:400]}")
near(rows[0].get('height'), 0.737, 1e-6, "the oven row's height (the appliance's height)")
item = (rows[0].get('items') or [{}])[0].get('rect')
if not (isinstance(item, list) and len(item) == 4):
    fail(f"the oven row has one item rect: {rows[0]}")
near(item[3] - item[1], 0.737, 1e-6, "the oven item's v1 - v0")
if rows[1].get('kind') != 'door':
    fail(f"row 1 stays a door: {rows[1]}")
near(rows[1].get('height'), H - 0.737 - 0.003, 1e-9, "the door row under the oven")
g = probed(rr, [[0.9, 0.603, 1.6], [0.9, 0.603, 0.6]], what="appliance")
hit(g[0], 'cabinet', 'cb0', "the oven 5 mm proud of the door plane (y 0.6) at z 1.6")
nohit(g[1], "the door below is flush at y 0.6 (nothing at 0.603)")
rows = rows_of_face(inspected(rr, 1), 'cb0', "after the microwave drawer")
kinds = [(r.get('kind'), r.get('appliance')) for r in rows]
if kinds != [('appliance', 'appliance:wall-oven'), ('appliance', 'appliance:microwave-drawer'), ('door', None)]:
    fail(f"three rows oven / microwave drawer / door: {kinds}")
for r, want in zip(rows, (0.737, 0.381, H - 0.737 - 0.381 - 0.006)):
    near(r.get('height'), want, 1e-9, f"row height ({r.get('kind')})")
print("ok: oven 0.737 on the upper row, microwave 0.381 below, the door takes the rest")

rr, se = refused(KITCHEN + [TALL, TWO, OVEN, MICRO, WARM, INSPECT18], "a third appliance on a tall cabinet")
if len(rows_of_face(inspected(rr), 'cb0', "after the refusal")) != 3:
    fail("the refused third appliance changed nothing")
BASE = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "width": 0.762})
rr = run_ok(KITCHEN + [BASE, cmd('cabinet.face', {"id": "cb0", "row": 0, "appliance": "appliance:warming-drawer"}), INSPECT18], "a warming drawer on a base cabinet")
rows = rows_of_face(inspected(rr), 'cb0', "base with a warming drawer")
if not any(r.get('appliance') == 'appliance:warming-drawer' for r in rows):
    fail(f"the base cabinet took the warming drawer: {json.dumps(rows)[:300]}")
refused(KITCHEN + [BASE, cmd('cabinet.face', {"id": "cb0", "row": 0, "appliance": "appliance:warming-drawer"}),
                   cmd('cabinet.face', {"id": "cb0", "row": 1, "appliance": "appliance:microwave-drawer"})], "a second appliance on a base cabinet")
refused(KITCHEN + [cmd('cabinet', {"kind": "tall", "wall": "w0", "offset": 1.0, "width": 0.6}),
                   cmd('cabinet.face', {"id": "cb0", "row": 0, "appliance": "appliance:wall-oven"})], "a 0.762 oven on a 0.6 cabinet")
print("ok: a third appliance, a second on a base cabinet and an oven on a narrow cabinet are refused")
print("PASS 11")
