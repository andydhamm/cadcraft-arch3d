#!/usr/bin/env python3
# Tray ceilings (the brief's tray line): in the north-west room (interior 0.1..5.95 x 3.05..7.9) under the 6/12 gable,
# `tray {"room": NW, "inset": 0.6, "depth": 0.3}` recesses the ceiling 0.3 m into the attic inside the outline
# 0.7..5.35 x 3.65..7.3: 2.7 at the room centre (3.025,5.475), 2.4 at (0.4,5.0) outside it. A tray nested 0.6 inside it
# with depth 0.15 gives 2.85 at the centre. A first tray with depth 0.5 is refused and builds nothing (its top, 2.9,
# would cross the underside at y = 7.3, 2.8). Trays stay editable (depth, inset; the nested tray follows) and a delete
# takes the nested tray with it. Pitched sides ramp at their angle (45 degrees: 2.5 at 0.1 m inside the outline), a drawn
# outline recesses its own polygon, and a room with a level above it (not an attic) refuses a tray. In 3D the recessed
# ceiling solid moves up (2.68..2.7 at the centre, nothing at 2.39) and the recess has a vertical side band.
# A stub fails: no lift answers 2.4 everywhere, nothing is refused, and the ceiling solid stays at 2.38..2.4.
# needs: p16-trays
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
NW = R['NW']
C, OUT, IN1 = [3.025, 5.475], [0.4, 5.0], [1.0, 5.475]     # the centre; outside the outline; inside tr0, outside the nest
TRAY0 = cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3})
T0 = made(HOUSE + [GABLE, TRAY0], 'tray', f"tray {{room NW ({NW}), inset 0.6, depth 0.3}} under the gable")
NEST = cmd('tray', {"tray": T0, "inset": 0.6, "depth": 0.15})
BAND = [[3.025, 5.475, 2.69], [3.025, 5.475, 2.39], [0.71, 5.475, 2.5]]

# 1. tr0 and the tray nested in it.
rr = run_ok(HOUSE + [GABLE, TRAY0, ceiling_cmd([C, OUT, IN1]), probe_cmd(BAND), NEST, ceiling_cmd([C, OUT, IN1]), INSPECT16],
            "tr0 and a tray nested in it")
r0, r1 = reply_with(rr, 'tray', 0, 'tray'), reply_with(rr, 'tray', 1, 'tray (nested)')
T1 = r1.get('tray')
if r0.get('tray') != T0 or r0.get('room') != NW or r0.get('parent') is not None:
    fail(f"tr0's reply: tray {T0}, room {NW}, parent null: {json.dumps(r0)[:300]}")
expect_box(r0.get('outline'), [0.7, 3.65, 5.35, 7.3], "tr0's outline (NW inset 0.6)")
if not isinstance(T1, str) or T1 == T0 or r1.get('room') != NW or r1.get('parent') != T0:
    fail(f"the nested tray's reply: a new id, room {NW}, parent {T0}: {json.dumps(r1)[:300]}")
expect_box(r1.get('outline'), [1.3, 4.25, 4.75, 6.7], "the nested tray's outline (tr0's inset 0.6)")
expect_ceilings(rr, [C, OUT, IN1], [2.7, 2.4, 2.7], "tr0 (depth 0.3)", nth=0)
expect_ceilings(rr, [C, OUT, IN1], [2.85, 2.4, 2.7], "tr0 and the nested tray (depth 0.15)", nth=1)
expect_hits(rr, [(BAND[0], ('ceiling', NW)), (BAND[1], None), (BAND[2], ('ceiling', NW))],
            "tr0 in 3D: the recessed solid 2.68..2.7, nothing at 2.39, the side band inside the outline at x 0.71")
rows = trays_of(inspected(rr))
if sorted(map(str, (t.get('id') for t in rows))) != sorted([T0, T1]) or by_id(rows, T1, 'trays').get('parent') != T0:
    fail(f"arch.inspect lists {T0} and {T1} (nested in {T0}): {json.dumps(rows)[:400]}")
print(f"ok: {T0} outline 0.7..5.35 x 3.65..7.3: 2.7 at the centre, 2.4 outside; nested {T1}: 2.85; the 3D solid moved up "
      "and the recess has its side band")

# 2. A first tray with depth 0.5 is refused (its 2.9 m top crosses the 2.8 m underside at y = 7.3) and builds nothing.
rc, rr, so, se = run(HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.5}), ceiling_cmd([C]), INSPECT16],
                     expect_ok=False)
errs = errors(se)
if len(errs) != 1 or 'roof' not in errs[0].lower():
    fail(f"a tray whose top would rise above the roof underside is refused with an error about the roof: {errs or '(none)'}")
if trays_of(inspected(rr)):
    fail(f"the refused tray built nothing: {json.dumps(trays_of(inspected(rr)))[:300]}")
expect_ceilings(rr, [C], [2.4], "NW after the refused tray")
print(f"ok: depth 0.5 is refused ({errs[0][:100]}); no tray, the centre stays 2.4")

# 3. Editable: depth, then inset (the nested tray follows), then a delete that takes the nested tray too.
rr = run_ok(HOUSE + [GABLE, TRAY0, NEST,
                     cmd('tray.edit', {"id": T0, "depth": 0.25}), ceiling_cmd([C, IN1]),
                     cmd('tray.edit', {"id": T0, "inset": 0.8}), ceiling_cmd([[0.8, 5.0], C]),
                     cmd('tray.delete', {"id": T0}), ceiling_cmd([C, IN1]), INSPECT16], "tray.edit and tray.delete")
expect_ceilings(rr, [C, IN1], [2.8, 2.65], "after tray.edit depth 0.25 (the nested tray follows)", nth=0)
expect_ceilings(rr, [[0.8, 5.0], C], [2.4, 2.8], "after tray.edit inset 0.8 (outline 0.9..5.15)", nth=1)
expect_ceilings(rr, [C, IN1], [2.4, 2.4], "after tray.delete", nth=2)
d = reply_with(rr, 'deleted', -1, 'tray.delete').get('deleted')
if not isinstance(d, list) or sorted(map(str, d)) != sorted([T0, T1]):
    fail(f"tray.delete {T0} deletes it and its nested tray {T1}: {d!r}")
if trays_of(inspected(rr)):
    fail(f"no tray is left after the delete: {json.dumps(trays_of(inspected(rr)))[:300]}")
print("ok: depth 0.25 -> 2.8 / 2.65; inset 0.8 -> 2.4 at (0.8,5.0); the delete removes both trays")

# 4. Pitched sides (45 degrees) and a drawn outline.
rr = run_ok(HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3, "sides": 45}),
                     ceiling_cmd([[0.8, 5.475], C])], "a pitched tray")
expect_ceilings(rr, [[0.8, 5.475], C], [2.5, 2.7], "45-degree sides: 0.1 m inside the outline rises 0.1; the centre 0.3")
rr = run_ok(HOUSE + [GABLE, cmd('tray', {"polygon": [[2, 4], [4, 4], [4, 6], [2, 6], [2, 4]], "depth": 0.2}),
                     ceiling_cmd([[3, 5], [1, 5]])], "a drawn tray outline")
r = reply_with(rr, 'tray', -1, 'tray {polygon}')
if r.get('room') != NW or r.get('parent') is not None:
    fail(f"the drawn tray lies in NW ({NW}) with no parent: {json.dumps(r)[:300]}")
expect_ceilings(rr, [[3, 5], [1, 5]], [2.6, 2.4], "the drawn outline 2..4 x 4..6, depth 0.2")
print("ok: pitched 45: 2.5 / 2.7; a drawn outline: 2.6 inside, 2.4 outside")

# 5. A room with a level above it has no attic: the tray is refused, naming the level above.
rc, rr, so, se = run(HOUSE + [GABLE, cmd('level', {"above": 0})] + house_walls(1) +
                     [cmd('roomdetect', {"level": 1}), TRAY0, INSPECT16], expect_ok=False)
errs = errors(se)
upper = level_row(inspected(rr), 1).get('name') or 'Level 2'
if len(errs) != 1 or not ('level above' in errs[0].lower() or str(upper).lower() in errs[0].lower()):
    fail(f"a tray under a level (not an attic) is refused naming the level above: {errs or '(none)'}")
if trays_of(inspected(rr)):
    fail(f"the refused tray built nothing: {json.dumps(trays_of(inspected(rr)))[:300]}")
print(f"ok: a tray under {upper} is refused ({errs[0][:100]})")
print("PASS 05")
