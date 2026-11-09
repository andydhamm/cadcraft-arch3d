#!/usr/bin/env python3
# Interior soffits (the brief's soffit line): in the north-east room (no stack, ceiling 2.4) a soffit over x 7.0..9.0,
# y 7.3..7.9 with its bottom at 2.1 is a box from 2.1 up to the ceiling: probes hit it (part soffit) at (8.0,7.6,2.2)
# and nothing at (8.0,7.2,2.2). Ceiling items hang from its underside: a library item placed under it after it exists
# has its top at 2.1, one placed before it goes from 2.4 to 2.1 and back to 2.4 when the soffit moves away, and a surface
# ceiling light under it sits at 2.1 - 0.05 = 2.05 (2.35 after the move); moving the soffit back over them re-seats both
# again (the brief: "moving a soffit over placed lights re-seats them"). A rect side within 0.15 m of a wall face snaps
# onto it; a depth is measured below the finished ceiling; `over` fills from the tops of wall cabinets (2.134) to the
# ceiling. Interior soffits are not roof soffits (a roof's own soffit stays part roof), and the soffit draws in plan.
# A stub fails: no box (both probes empty), items and lights stay at 2.4 / 2.35, no snap, an unchanged plan.
# needs: p16-soffits
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
NE = R['NE']
tmp = tempfile.mkdtemp()
SF = cmd('soffit', {"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1})
S0 = made(HOUSE + [SF], 'soffit', "soffit {rect x 7..9, y 7.3..7.9, bottom 2.1} in NE")
PEND = cmd('library.place', {"item": "light-pendant", "at": [8.0, 7.6]})
LF = cmd('lightfixture', {"type": "Surface ceiling fixture", "at": [8.0, 7.6]})
MOVE = cmd('soffit.edit', {"id": S0, "rect": [[6.5, 3.5], [7.3, 4.3]]})       # 0.45 from every wall face: no snap
BACK = cmd('soffit.edit', {"id": S0, "rect": [[7.0, 7.3], [9.0, 7.9]]})       # over the items again


def light_z(insp, lid):
    return by_id(rows_of(insp, 'lights'), lid, 'arch.inspect lights').get('z')


# 1. The box, the items and the light: before, under, after the soffit moves away, and once it moves back over them.
rr = run_ok(HOUSE + [PEND, LF, INSPECT16, SF, probe_cmd([[8.0, 7.6, 2.2], [8.0, 7.2, 2.2]]), INSPECT16, PEND, INSPECT16,
                     MOVE, INSPECT16, BACK, INSPECT16], "the soffit with a pendant and a surface light under it")
f0 = reply_with(rr, 'furnishing', 0, 'library.place (before the soffit)').get('furnishing')
f1 = reply_with(rr, 'furnishing', 1, 'library.place (under the soffit)').get('furnishing')
lid = (placed(rr, 0) or [None])[0]
rep = reply_with(rr, 'soffit', 0, 'soffit')
if rep.get('soffit') != S0 or rep.get('room') != NE:
    fail(f"the soffit's reply: soffit {S0}, room {NE}: {json.dumps(rep)[:300]}")
expect_box(rep.get('polygon'), [7.0, 7.3, 9.0, 7.9], "the soffit's polygon (no side is near a wall but the north one, already on it)")
near(rep.get('bottom'), 2.1, 1e-6, "the soffit's bottom")
near(rep.get('top'), 2.4, 1e-6, "the soffit's top (NE's finished ceiling)")
expect_hits(rr, [([8.0, 7.6, 2.2], ('soffit', S0)), ([8.0, 7.2, 2.2], None)], "the soffit box 2.1..2.4 over y 7.3..7.9")
i0, i1, i2, i3, i4 = (inspected(rr, k) for k in range(5))
near(item_top(i0, f0), 2.4, 1e-6, "the pendant's top before the soffit")
near(light_z(i0, lid), 2.35, 1e-6, "the surface light before the soffit (2.4 - 0.05)")
near(item_top(i1, f0), 2.1, 1e-6, "the pendant placed before the soffit: its top once the soffit is over it")
near(light_z(i1, lid), 2.05, 1e-6, "the surface light under the soffit (2.1 - 0.05)")
near(item_top(i2, f1), 2.1, 1e-6, "a pendant placed under the soffit: its top")
near(item_top(i3, f0), 2.4, 1e-6, "the first pendant's top after the soffit moves away")
near(light_z(i3, lid), 2.35, 1e-6, "the surface light after the soffit moves away")
near(item_top(i4, f0), 2.1, 1e-6, "the first pendant's top once the soffit moves back over it")
near(light_z(i4, lid), 2.05, 1e-6, "the surface light once the soffit moves back over it")
row = by_id(soffits_of(i1), S0, 'arch.inspect soffits')
if row.get('layer') != 'A-CLNG' or row.get('room') != NE:
    fail(f"the soffit's row: layer A-CLNG, room {NE}: {json.dumps(row)[:300]}")
expect_box(by_id(soffits_of(i3), S0, 'arch.inspect soffits').get('polygon'), [6.5, 3.5, 7.3, 4.3], "the moved soffit")
print(f"ok: {S0} is a box 2.1..2.4; items hang from it (2.4 -> 2.1 -> 2.4 -> 2.1 as it moves away and back), the light 2.35 -> 2.05 -> 2.35 -> 2.05")

# 2. Snap, depth.
rr = run_ok(HOUSE + [cmd('soffit', {"rect": [[9.0, 7.0], [9.96, 7.95]], "bottom": 2.1}), INSPECT16], "a soffit drawn into NE's corner")
expect_box(soffits_of(inspected(rr))[0].get('polygon') if soffits_of(inspected(rr)) else None, [9.0, 7.0, 9.9, 7.9],
           "a rect drawn to 9.96 x 7.95 snaps onto the wall faces x 9.9, y 7.9 (x 9.0 and y 7.0 stay)")
rr = run_ok(HOUSE + [cmd('soffit', {"rect": [[6.5, 3.5], [7.3, 4.3]], "depth": 0.3}), INSPECT16], "a soffit given a depth")
near(reply_with(rr, 'soffit', -1, 'soffit {depth}').get('bottom'), 2.1, 1e-6, "a soffit 0.3 below the 2.4 ceiling: its bottom")
print("ok: a rect within 0.15 of the wall faces snaps onto them; depth 0.3 under 2.4 gives a 2.1 bottom")

# 3. Over wall cabinets: two 0.6 x 0.305 x 0.762 wall cabinets on w2 at offsets 1.0 and 1.6 (x 8.7..9.3, 8.1..8.7,
# elevation 1.372, top 2.134).
CABS = [cmd('cabinet', {"kind": "wall", "wall": "w2", "offset": 1.0}), cmd('cabinet', {"kind": "wall", "wall": "w2", "offset": 1.6})]
cabs = objects_named(inspected(run_ok(HOUSE + CABS + [INSPECT16], "two wall cabinets on w2")), 'cabinets', 'cabinets')
if len(cabs) != 2:
    fail(f"two wall cabinets: {json.dumps(cabs)[:300]}")
for c in cabs:
    near(num(c.get('elevation'), 'cabinet elevation') + num(c.get('height'), 'cabinet height'), 2.134, 1e-6, f"{c.get('id')}'s top")
ids = [c.get('id') for c in cabs]
rr = run_ok(HOUSE + CABS + [cmd('soffit', {"over": ids}), probe_cmd([[8.9, 7.75, 2.2], [8.9, 7.75, 2.1]]), INSPECT16],
            f"soffit {{over: {ids}}}")
rep = reply_with(rr, 'soffit', -1, 'soffit {over}')
expect_box(rep.get('polygon'), [8.1, 7.595, 9.3, 7.9], "the soffit over the two cabinets")
near(rep.get('bottom'), 2.134, 1e-6, "the soffit over the cabinets: its bottom (the cabinet tops)")
expect_hits(rr, [([8.9, 7.75, 2.2], ('soffit', rep.get('soffit'))), ([8.9, 7.75, 2.1], 'cabinet')],
            "the soffit above the cabinet tops, the cabinet below")
over = by_id(soffits_of(inspected(rr)), rep.get('soffit'), 'arch.inspect soffits').get('over')
if not isinstance(over, list) or sorted(map(str, over)) != sorted(map(str, ids)):
    fail(f"the soffit's row lists the cabinets it is over: {over!r}, expected {ids}")
print(f"ok: over {ids}: x 8.1..9.3, y 7.595..7.9, bottom 2.134")

# 4. A roof with its own soffit: the probe and the part counts keep the two apart.
ROOF = cmd('roofauto', {"pitchRise": 6, "pitchRun": 12, "soffit": True,
                        "walls": [{"wall": "w1", "directive": "gable"}, {"wall": "w3", "directive": "gable"}]})
rr = run_ok(HOUSE + [ROOF, cmd('build3d', {}), SF, cmd('build3d', {})], "the gable with a roof soffit, then an interior soffit")
if parts_of(rr, 0).get('soffit') is not None or parts_of(rr, 1).get('soffit') != 1:
    fail(f"parts.soffit counts interior soffits only: {parts_of(rr, 0).get('soffit')} with the roof's soffit alone, "
         f"{parts_of(rr, 1).get('soffit')} with one interior soffit (expected none, then 1)")
print("ok: the roof's own soffit is not part soffit; one interior soffit is")

# 5. The plan draws the soffit.
a, b = os.path.join(tmp, 'plain.png'), os.path.join(tmp, 'soffit.png')
run_ok(HOUSE + [cmd('export', {"path": a})], "the plan without the soffit")
run_ok(HOUSE + [SF, cmd('export', {"path": b})], "the plan with the soffit")
if not (os.path.exists(a) and os.path.exists(b)):
    fail("the plan exports were written")
if digest(a) == digest(b) or luma_sd(b) <= 1:
    fail(f"the plan with the soffit differs from the plan without it and is not blank (luma sd {luma_sd(b):.2f})")
artifact(b, 'soffit-plan.png')
print("ok: the plan with the soffit differs from the plan without it")
print("PASS 06")
