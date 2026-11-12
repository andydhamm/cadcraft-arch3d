#!/usr/bin/env python3
# Driving dimensions (the brief's number "with the interior wall (6,3)-(6,8) selected, setting the 6.1 dimension to
# 5.6 moves it to x = 5.5: north-west room interior 5.35 x 4.85 = 25.9475 m2, north-east 4.35 x 4.85 = 21.0975 m2, and
# the row reads 5.6 + 4.6"): dimdrive moves the selected (or named) object a dimension locates, the far end stays; a
# window moves along its wall; refusals move nothing, and a positive control shows the attachment test is what refuses.
# A stub fails: dimdrive does not exist, w5 stays at x = 6.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-dims-model, p20-auto-dims-1, p20-auto-dims-2, p20-dims-drive
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

AUTO = cmd('dimauto', {"level": 0})


def wall_x(insp, label):
    w = pv_wall(insp, label)
    return pv_xy(w.get('start'), f"{label} start"), pv_xy(w.get('end'), f"{label} end")


# (a) THE NUMBER.
auto = pv_auto(run_ok(HOUSE + [AUTO, cmd('arch.inspect', {"autoDims": True})], "the auto rows"))
row = pv_auto_row(auto, 'north', 'walls')
pv_values(row, [6.1, 4.1], "the north walls row before driving")
h = row['handles'][0]
rr = run_ok(HOUSE + [AUTO, INSPECT20, cmd('select', {"handles": [pv_proxy_hex("w5")]}),
                     cmd('dimdrive', {"dimension": h, "value": 5.6}), INSPECT20, 'roomdetect {}', INSPECT20],
            "dimdrive the 6.1 dimension to 5.6 with w5 selected")
r = reply_with(rr, 'by', -1, 'dimdrive')
if r.get('moved') != 'w5':
    fail(f"dimdrive moves w5: {r}")
before, after = inspected(rr, 0), inspected(rr, 1)
(s, e) = wall_x(after, 'w5')
if any(abs(v - w) > 1e-6 for v, w in zip(s + e, (5.5, 3.0, 5.5, 8.0))):
    fail(f"THE NUMBER: w5 moves to x = 5.5 (ends (5.5, 3) and (5.5, 8)): {s}, {e}")
for lab in ('w2', 'w4', 'w3'):
    if wall_x(after, lab) != wall_x(before, lab):
        fail(f"{lab} does not move (the far end stays): {wall_x(before, lab)} -> {wall_x(after, lab)}")
rooms = objects_named(inspected(rr, 2), 'rooms', 'rooms')
for pt, area, what in (((3.0, 5.5), 25.9475, "north-west (5.35 x 4.85)"), ((8.0, 5.5), 21.0975, "north-east (4.35 x 4.85)")):
    hit = [q for q in rooms if pv_inside(pt, [pv_xy(v, 'room') for v in q['polygon']])]
    if len(hit) != 1:
        fail(f"one room holds {pt}: {hit}")
    near(hit[0].get('interiorArea'), area, 1e-6, f"THE NUMBER: the {what} room's interior area")
pv_values(pv_auto_row(pv_auto(rr, 1), 'north', 'walls'), [5.6, 4.6], "THE NUMBER: the north walls row after driving")
print("ok (a): w5 to x = 5.5; NW 25.9475 m2, NE 21.0975 m2; the row reads 5.6 + 4.6")

# (b) the object form: a window moves along its wall, the far end stays.
rr = run_ok(HOUSE + [PV_WINDOW, cmd('dimlinear', {"p1": [-0.1, 8.0], "p2": [4.0, 8.0], "at": [2.0, 9.5]}), INSPECT20],
            "the window's dimension")
dh = [q.get('handle') for q in rr if isinstance(q, dict) and set(q) == {'handle'}][-1]
rr = run_ok(HOUSE + [PV_WINDOW, cmd('dimlinear', {"p1": [-0.1, 8.0], "p2": [4.0, 8.0], "at": [2.0, 9.5]}),
                     cmd('dimdrive', {"dimension": dh, "value": 3.0, "object": "o0"}), INSPECT20], "driving o0")
insp = inspected(rr)
o0 = by_id(objects_named(insp, 'openings', 'openings'), 'o0', 'openings')
near(o0.get('offset'), 7.1, 1e-6, "o0's offset from w2's start (10, 8): centre x = 2.9")
if wall_x(insp, 'w3') != ((0.0, 8.0), (0.0, 0.0)):
    fail(f"w3 (the far end) does not move: {wall_x(insp, 'w3')}")
near(pv_dim(pv_dims(rr), dh).get('measurement'), 3.0, 1e-6, "the driven dimension")
print("ok (b): driving the window's 4.1 to 3.0 moves o0 to x = 2.9; w3 stays")

# (c) refusals move nothing; then a positive control.
SET = HOUSE + [PV_WINDOW, AUTO]
hn = pv_auto_row(pv_auto(run_ok(SET + [cmd('arch.inspect', {"autoDims": True})], "rows")), 'north', 'walls')['handles'][0]
BAD = [cmd('dimdrive', {"dimension": hn, "value": -1, "object": "w5"}),
       cmd('dimdrive', {"dimension": hn, "value": "5", "object": "w5"}),
       cmd('dimdrive', {"dimension": hn, "value": 5, "object": "w1"}),
       cmd('select', {"clear": True}), cmd('dimdrive', {"dimension": hn, "value": 5}),
       cmd('dimdrive', {"dimension": "ZZ", "value": 1})]
rc, rr, so, se = run(SET + [INSPECT20] + BAD + [INSPECT20], expect_ok=False)
errs = errors(se)
if len(errs) != 5:
    fail(f"value -1, value \"5\", object w1 (not attached to the dimension), no selection and an unknown handle are each "
         f"refused with an error line: {errs}")
for key in ('walls', 'openings'):
    if objects_named(inspected(rr, 0), key, key) != objects_named(inspected(rr, -1), key, key):
        fail(f"refused drives move nothing ({key} changed)")
# A LINE is not a dimension; a vertical dimension across o0's wall cannot drive o0 along it.
EXTRA = [cmd('line', {"points": [[20, 20], [21, 20]]}),
         cmd('dimlinear', {"p1": [4.0, 8.0], "p2": [4.0, 5.0], "at": [5.0, 6.5], "rotation": 90})]
rr = run_ok(SET + EXTRA, "a line and a vertical dimension from o0")
line_h = reply_with(rr, 'handles', -1, 'line')['handles'][0]
vh = [q.get('handle') for q in rr if isinstance(q, dict) and set(q) == {'handle'}][-1]
rc, rr, so, se = run(SET + EXTRA + [cmd('select', {"clear": True}), cmd('dimdrive', {"dimension": line_h, "value": 1}),
                                    cmd('dimdrive', {"dimension": vh, "value": 2.0, "object": "o0"}), INSPECT20],
                     expect_ok=False)
if len(errors(se)) != 2:
    fail(f"a LINE's handle as the dimension, and a dimension across o0's wall, are refused: {errors(se)}")
o0 = by_id(objects_named(inspected(rr), 'openings', 'openings'), 'o0', 'openings')
near(o0.get('offset'), 6.0, 1e-9, "o0 after the refused drives")
rr = run_ok(SET + [cmd('dimdrive', {"dimension": hn, "value": 5, "object": "w5"}), INSPECT20], "the positive control")
(s, e) = wall_x(inspected(rr), 'w5')
if abs(s[0] - 4.9) > 1e-6 or abs(e[0] - 4.9) > 1e-6:
    fail(f"the same dimension drives w5 (attached) to x = 4.9: {s}, {e}")
print("ok (c): refusals move nothing; driving w5 by the same dimension works (x = 4.9)")
print("PASS 09")
