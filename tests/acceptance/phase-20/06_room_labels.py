#!/usr/bin/env python3
# Room labels (the brief's room-labels bullet): name, area (interior or standard, chosen per room with room.label),
# dimensions and finished ceiling height, each shown or hidden per plan view: Floor Plan shows name, area and
# dimensions (NW 28.37 m2 interior, 5.85 x 4.85), Electrical names only, Reflected Ceiling name and ceiling height (2.40,
# 2.17 under the suspended stack), Roof Plan and Site none, and with no view open exactly today's name at the vertex
# average. A room switched to its standard area (NW 31.11) keeps it through roomdetect.
# A stub fails: (a) finds no roomArea or roomDimensions label.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-room-labels
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

R = house_rooms()
DIMS = {"S": "9.80 x 2.85", "NW": "5.85 x 4.85", "NE": "4.85 x 3.85"}
AREA = {"S": 27.93, "NW": 28.3725, "NE": 18.6725}


def room_labels(plan, row):
    return [l for l in pv_labels(plan) if l.get('handle') == row.get('handle')]


def expect_kinds(plan, insp, kinds, what):
    for k, rid in R.items():
        row = room_row(insp, rid)
        got = [l.get('kind') for l in room_labels(plan, row)]
        if sorted(got) != sorted(kinds):
            fail(f"{what}: room {k} has the labels {kinds}, got {got}")


# (a) Floor Plan: name, area, dimensions.
rr = run_ok(HOUSE + [cmd('arch.inspect', {"plan": "Floor Plan", "rooms": True})], "Floor Plan room labels")
plan, insp = pv_plan(rr), inspected(rr)
expect_kinds(plan, insp, ['roomName', 'roomArea', 'roomDimensions'], "Floor Plan")
for k, rid in R.items():
    row = room_row(insp, rid)
    near(row.get('interiorArea'), AREA[k], 1e-6, f"room {k}'s interior area")
    ls = {l.get('kind'): l for l in room_labels(plan, row)}
    if ls['roomArea'].get('text') != f"{row['interiorArea']:.2f} m2":
        fail(f"room {k}'s area label reads its interior area {row['interiorArea']:.2f} m2: {ls['roomArea']}")
    if ls['roomDimensions'].get('text') != DIMS[k]:
        fail(f"room {k}'s dimensions label reads {DIMS[k]!r}: {ls['roomDimensions'].get('text')!r}")
    ring = [pv_xy(q, 'room') for q in row.get('polygon') or []]
    pts = [pv_pt(ls[n].get('at'), n) for n in ('roomName', 'roomArea', 'roomDimensions')]
    if not all(pv_inside(p, ring) for p in pts):
        fail(f"room {k}'s labels lie inside it: {pts}")
    if max(p[0] for p in pts) - min(p[0] for p in pts) > 1e-9 or not (pts[0][1] > pts[1][1] > pts[2][1]):
        fail(f"room {k}'s labels share one x and run name above area above dimensions: {pts}")
    if any(l.get('layer') != 'A-ROOM' for l in ls.values()):
        fail(f"room labels are on A-ROOM: {list(ls.values())}")
print("ok (a): Floor Plan labels each room with name, interior area (NW 28.37 m2) and dimensions (NW 5.85 x 4.85)")

# (b) the standard area, chosen per room, kept by roomdetect.
rr = run_ok(HOUSE + [cmd('room.label', {"room": R['NW'], "area": "standard"}), INSPECT20,
                     cmd('arch.inspect', {"plan": "Floor Plan", "rooms": True}), 'roomdetect {}',
                     cmd('arch.inspect', {"plan": "Floor Plan", "rooms": True})], "room.label standard")
r0 = reply_with(rr, 'area', 0, 'room.label')
if r0.get('room') != R['NW'] or r0.get('area') != 'standard':
    fail(f"room.label replies with the room and its area kind: {r0}")
insp = inspected(rr, 0)
for k, rid in R.items():
    if room_row(insp, rid).get('labelArea') != ('standard' if k == 'NW' else 'interior'):
        fail(f"labelArea is standard for NW only: room {k} {room_row(insp, rid).get('labelArea')!r}")
for nth in (1, 2):
    insp, plan = inspected(rr, nth), pv_plan(rr, nth - 1)
    nw = [r for r in objects_named(insp, 'rooms', 'rooms') if pv_inside((3.0, 5.5), [pv_xy(q, 'room') for q in r['polygon']])]
    if len(nw) != 1:
        fail(f"one room holds (3, 5.5): {nw}")
    near(nw[0].get('standardArea'), 31.11, 1e-6, "NW's standard area")
    got = [l.get('text') for l in room_labels(plan, nw[0]) if l.get('kind') == 'roomArea']
    if got != [f"{nw[0]['standardArea']:.2f} m2"]:
        fail(f"NW's area label reads its standard area{' after roomdetect' if nth == 2 else ''}: {got}")
s = room_row(inspected(rr, 1), R['S'])
if [l.get('text') for l in room_labels(pv_plan(rr, 0), s) if l.get('kind') == 'roomArea'] != [f"{s['interiorArea']:.2f} m2"]:
    fail("S still reads its interior area")
print("ok (b): room.label NW standard -> 31.11 m2 (S keeps interior), kept through roomdetect")

# (c) per view.
rr = run_ok(HOUSE + [cmd('arch.inspect', {"plan": v, "rooms": True}) for v in
                     ("Electrical", "Reflected Ceiling", "Roof Plan", "Site", True)]
            + [cmd('room', {"id": R['NE'], "ceilingStack": PV_SUSPENDED}),
               cmd('arch.inspect', {"plan": "Reflected Ceiling", "rooms": True})], "room labels per view")
insp = inspected(rr, 0)
expect_kinds(pv_plan(rr, 0), insp, ['roomName'], "Electrical")
expect_kinds(pv_plan(rr, 1), insp, ['roomName', 'roomCeiling'], "Reflected Ceiling")
for k, rid in R.items():
    c = [l.get('text') for l in room_labels(pv_plan(rr, 1), room_row(insp, rid)) if l.get('kind') == 'roomCeiling']
    if c != ["2.40"]:
        fail(f"Reflected Ceiling: room {k}'s ceiling label reads 2.40: {c}")
for nth, v in ((2, "Roof Plan"), (3, "Site")):
    if pv_labels(pv_plan(rr, nth), 'roomName') or pv_labels(pv_plan(rr, nth), 'roomArea'):
        fail(f"{v} has no room labels")
plain = pv_plan(rr, 4)
expect_kinds(plain, insp, ['roomName'], "no view open")
for k, rid in R.items():
    row = room_row(insp, rid)
    poly = [pv_xy(q, 'room') for q in row['polygon']]
    at = pv_pt(room_labels(plain, row)[0].get('at'), 'name')
    if math.dist(at, (sum(p[0] for p in poly) / len(poly), sum(p[1] for p in poly) / len(poly))) > 1e-9:
        fail(f"no view open: room {k}'s name sits at its vertex average as today: {at}")
ne = room_row(inspected(rr, 5), R['NE'])
c = [l.get('text') for l in room_labels(pv_plan(rr, 5), ne) if l.get('kind') == 'roomCeiling']
if c != ["2.17"]:
    fail(f"Reflected Ceiling: NE under the suspended stack reads 2.17: {c}")
print("ok (c): Electrical names only; Reflected Ceiling names and 2.40 (NE 2.17 suspended); Roof Plan and Site none; "
      "no view: today's names")

# (d) a renamed room.
rr = run_ok(HOUSE + [cmd('room', {"id": R['NW'], "name": "Living"}), cmd('arch.inspect', {"plan": "Floor Plan", "rooms": True})],
            "a renamed room")
nw = room_row(inspected(rr), R['NW'])
if [l.get('text') for l in room_labels(pv_plan(rr), nw) if l.get('kind') == 'roomName'] != ["Living"]:
    fail("NW renamed Living is labelled Living")
print("ok (d): a renamed room's label follows")
print("PASS 06")
