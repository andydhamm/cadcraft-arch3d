#!/usr/bin/env python3
# Reflected ceiling plan (the brief's RCP bullet and number "phase 16's tray room shows two tray outlines and the label
# 2.70"): under the 6/12 gable, the north-west room's tray (inset 0.6, depth 0.3) and the tray nested in it (inset 0.6,
# depth 0.15) show as two closed outlines on A-CLNG, labelled 2.70 and 2.85; the room's own finished ceiling height
# (2.40) is labelled outside the trays, clear of their labels. A pitched tray adds its top ring; a vaulted room gets the
# ridge as a vault line and its pitch (6/12) as its ceiling text; a suspended stack reads 2.17; soffits and ceiling items
# are outlined. None of it is in the Floor Plan view.
# A stub fails: ceiling_plan draws nothing, so (a) finds no tray outline and no 2.70.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-room-labels, p20-rcp
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

R = house_rooms()
NW = R['NW']
RCP = cmd('arch.inspect', {"plan": "Reflected Ceiling", "rooms": True, "trays": True})
FLOOR = cmd('arch.inspect', {"plan": "Floor Plan"})
T0 = made(HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3})], 'tray', "tray in NW")

# (a) the tray room.
rr = run_ok(HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3}),
                     cmd('tray', {"tray": T0, "inset": 0.6, "depth": 0.15}), RCP, FLOOR], "the nested trays")
plan, insp = pv_plan(rr, 0), inspected(rr, 0)
trays = trays_of(insp)
t0 = by_id(trays, T0, 'trays')
outl = pv_lines(plan, 'trayOutline')
by_h = pv_by_handle(outl)
if len(by_h) != 2 or any(len(v) != 4 for v in by_h.values()):
    fail(f"THE NUMBER: two tray outlines of four lines each, got {[(h, len(v)) for h, v in by_h.items()]}")
boxes = {}
for h, ls in by_h.items():
    pv_closed(ls, f"tray outline {h}")
    if any(l.get('layer') != 'A-CLNG' for l in ls):
        fail(f"tray outlines are on A-CLNG: {ls}")
    boxes[h] = pv_bbox(ls)
want = {t0['handle']: (0.7, 3.65, 5.35, 7.3)}
other = [h for h in by_h if h != t0['handle']]
if t0['handle'] not in by_h or len(other) != 1:
    fail(f"one outline carries {T0}'s handle {t0['handle']}: {sorted(map(str, by_h))}")
want[other[0]] = (1.3, 4.25, 4.75, 6.7)
for h, w in want.items():
    if any(abs(g - e) > 1e-6 for g, e in zip(boxes[h], w)):
        fail(f"tray outline {h}: box {w}, got {boxes[h]}")
if pv_lines(plan, 'trayTop'):
    fail("vertical-sided trays have no trayTop lines")
th = pv_labels(plan, 'trayHeight')
if sorted(l.get('text') for l in th) != ["2.70", "2.85"]:
    fail(f"THE NUMBER: the tray labels read 2.70 and 2.85: {[l.get('text') for l in th]}")
l270 = [l for l in th if l.get('text') == "2.70"][0]
p270 = pv_pt(l270.get('at'), '2.70 label')
if l270.get('handle') != t0['handle'] or not pv_inside(p270, pv_box_ring(want[t0['handle']])) \
        or pv_inside(p270, pv_box_ring(want[other[0]])):
    fail(f"the 2.70 label carries {T0}'s handle and sits inside its outline, outside the nested one: {l270}")
nw = room_row(insp, NW)
mine = [l for l in pv_labels(plan) if l.get('handle') == nw.get('handle')]
ceil = [l for l in mine if l.get('kind') == 'roomCeiling']
name = [l for l in mine if l.get('kind') == 'roomName']
if len(ceil) != 1 or ceil[0].get('text') != "2.40" or len(name) != 1:
    fail(f"NW has one roomName and one roomCeiling label reading 2.40 in the Reflected Ceiling view: {mine}")
nw_ring = [pv_xy(q, 'NW') for q in nw.get('polygon') or []]
for l in ceil + name:
    p = pv_pt(l.get('at'), 'NW label')
    if not pv_inside(p, nw_ring) or any(pv_inside(p, pv_box_ring(b)) for b in want.values()):
        fail(f"NW's {l.get('kind')} label sits inside the room and outside both tray outlines: {l}")
pc = pv_pt(ceil[0].get('at'), 'NW ceiling label')
for l in th:
    if math.dist(pc, pv_pt(l.get('at'), 'tray label')) < 0.3:
        fail(f"NW's ceiling label is at least 0.3 m from the tray label {l.get('text')}: {ceil[0]} / {l}")
for k, rid in R.items():
    if room_row(insp, rid).get('ceilingText') != "2.40":
        fail(f"room {k}'s ceilingText is 2.40 (trays are labelled separately): {room_row(insp, rid).get('ceilingText')!r}")
fl = pv_plan(rr, 1)
if pv_lines(fl, 'trayOutline') or pv_labels(fl, 'trayHeight'):
    fail("the Floor Plan view has no tray outlines or tray labels")
print("ok (a): two closed tray outlines on A-CLNG (0.7..5.35 x 3.65..7.3 and 1.3..4.75 x 4.25..6.7), labels 2.70 "
      "and 2.85, NW's 2.40 outside the trays; none of it in Floor Plan")

# (b) a pitched tray.
rr = run_ok(HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3, "sides": 45}), RCP], "a pitched tray")
plan = pv_plan(rr)
outl, top = pv_lines(plan, 'trayOutline'), pv_lines(plan, 'trayTop')
for ls, box, what in ((outl, (0.7, 3.65, 5.35, 7.3), 'outline'), (top, (1.0, 3.95, 5.05, 7.0), 'top ring')):
    if len(ls) != 4:
        fail(f"the 45-degree tray's {what} is four lines: {ls}")
    pv_closed(ls, f"tray {what}")
    if any(abs(g - e) > 1e-6 for g, e in zip(pv_bbox(ls), box)):
        fail(f"the 45-degree tray's {what} spans {box} (inset 0.3 / tan 45 for the top), got {pv_bbox(ls)}")
if [l.get('text') for l in pv_labels(plan, 'trayHeight')] != ["2.70"]:
    fail(f"the pitched tray is labelled 2.70: {pv_labels(plan, 'trayHeight')}")
print("ok (b): a pitched tray draws its outline and its top ring 1.0..5.05 x 3.95..7.0, labelled 2.70")

# (c) a vault.
rr = run_ok(HOUSE + [GABLE, cmd('room', {"id": NW, "flatCeiling": False}), RCP], "a vaulted NW room")
plan, insp = pv_plan(rr), inspected(rr)
vl = pv_lines(plan, 'vaultLine')
nwh = room_row(insp, NW).get('handle')
if len(vl) != 1 or vl[0].get('layer') != 'A-CLNG' or vl[0].get('handle') != nwh:
    fail(f"exactly one vault line on A-CLNG with NW's handle: {vl}")
ends = sorted(pv_pt(vl[0].get(k), 'vault line') for k in ('a', 'b'))
if any(abs(g - e) > 1e-6 for p, q in zip(ends, [(0.1, 4.0), (5.95, 4.0)]) for g, e in zip(p, q)):
    fail(f"the vault line is the ridge clipped to NW: (0.1, 4) to (5.95, 4), got {ends}")
near(pv_len(vl[0]), 5.85, 1e-6, "the vault line's length")
want = {"S": "2.40", "NW": "6/12", "NE": "2.40"}
for k, rid in R.items():
    if room_row(insp, rid).get('ceilingText') != want[k]:
        fail(f"room {k}'s ceilingText is {want[k]} (a vault reads its pitch): {room_row(insp, rid).get('ceilingText')!r}")
print("ok (c): the vaulted room has its ridge as a 5.85 m vault line and reads 6/12")

# (d) a suspended stack.
rr = run_ok(HOUSE + [cmd('room', {"id": R['NE'], "ceilingStack": PV_SUSPENDED}), RCP], "a suspended ceiling in NE")
plan, insp = pv_plan(rr), inspected(rr)
ne = room_row(insp, R['NE'])
lab = [l.get('text') for l in pv_labels(plan, 'roomCeiling') if l.get('handle') == ne.get('handle')]
if ne.get('ceilingText') != "2.17" or lab != ["2.17"]:
    fail(f"NE under the suspended stack reads 2.17 (row and label): {ne.get('ceilingText')!r}, {lab}")
print("ok (d): the suspended stack's finished ceiling reads 2.17")

# (e) a soffit and a ceiling item.
rr = run_ok(HOUSE + [cmd('soffit', {"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1}),
                     cmd('library.place', {"item": "light-ceiling", "at": [3, 5]}),
                     cmd('arch.inspect', {"plan": "Reflected Ceiling", "soffits": True, "furnishings": True}), FLOOR],
            "a soffit and a ceiling light")
plan, insp = pv_plan(rr, 0), inspected(rr, 0)
sf = soffits_of(insp)
if len(sf) != 1:
    fail(f"one soffit: {sf}")
sl = [l for l in pv_lines(plan, 'soffit') if l.get('handle') == sf[0].get('handle')]
if not sl or any(l.get('layer') != 'A-CLNG' for l in sl):
    fail(f"soffit lines on A-CLNG with the soffit's handle: {pv_lines(plan, 'soffit')}")
near(sum(pv_len(l) for l in sl), 5.2, 1e-6, "the soffit outline's length (2 x (2.0 + 0.6))")
pv_closed(sl, "the soffit outline")
light = [f for f in furnishings_of(insp) if f.get('item') == 'light-ceiling']
if len(light) != 1:
    fail(f"one ceiling light: {light}")
cl = [l for l in pv_lines(plan, 'ceilingItem') if l.get('handle') == light[0].get('handle')]
if len(cl) < 3:
    fail(f"the ceiling light is outlined (ceilingItem lines with its handle): {pv_lines(plan, 'ceilingItem')}")
pv_closed(cl, "the ceiling light's outline")
if not pv_inside((3.0, 5.0), [pv_pt(l.get('a'), 'item') for l in cl]) and \
        not (pv_bbox(cl)[0] < 3 < pv_bbox(cl)[2] and pv_bbox(cl)[1] < 5 < pv_bbox(cl)[3]):
    fail(f"the ceiling light's outline surrounds (3, 5): {pv_bbox(cl)}")
fl = pv_plan(rr, 1)
if pv_lines(fl, 'soffit') or pv_lines(fl, 'ceilingItem'):
    fail("the Floor Plan view has no soffit or ceilingItem lines")
print("ok (e): the soffit outline (5.2 m) and the ceiling light's outline on A-CLNG; not in Floor Plan")
print("PASS 04")
