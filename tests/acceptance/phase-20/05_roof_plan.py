#!/usr/bin/env python3
# Roof plan (the brief's roof-plan bullet and number "phase 4 hip at 6/12: one ridge line 2.0 m long, four hip lines of
# plan length 5.798 and four 6/12 labels"): the Roof Plan view draws the ridges, hips and valleys, the eaves and gable
# (rake) edges from the plane outlines (A-ROOF-OTLN), and per plane its pitch as a label with a down-slope arrow
# (A-ROOF-ANNO). The gable has rakes on its gable walls; a 4/12 roof reads 4/12; an overhang moves the eaves out. The
# Floor Plan view and the plan with no view open draw no roof-plan annotation.
# A stub fails: roof_plan draws nothing, so (a) finds no pitch label.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-roof-plan
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

ROOFPLAN = cmd('arch.inspect', {"plan": "Roof Plan", "roofs": True})
WALLS = house_walls()


def check_arrows(plan, roof, want_dirs, what):
    labels = pv_labels(plan, 'pitch')
    rings = [[pv_xy(q, 'ring') for q in p.get('ring') or []] for p in roof.get('planes') or []]
    shafts = pv_lines(plan, 'slopeArrow')
    if len(shafts) != len(want_dirs) or len(pv_lines(plan, 'slopeArrowHead')) != 2 * len(want_dirs):
        fail(f"{what}: {len(want_dirs)} slope arrows with two heads each: {len(shafts)} shafts, "
             f"{len(pv_lines(plan, 'slopeArrowHead'))} heads")
    dirs = []
    for s in shafts:
        if s.get('layer') != 'A-ROOF-ANNO':
            fail(f"{what}: slope arrows are on A-ROOF-ANNO: {s}")
        near(pv_len(s), 0.6, 1e-6, f"{what}: a slope arrow's shaft")
        (ax, ay), (bx, by) = pv_pt(s['a'], 'shaft a'), pv_pt(s['b'], 'shaft b')
        dirs.append((round((bx - ax) / 0.6, 6) + 0.0, round((by - ay) / 0.6, 6) + 0.0))
        if not any(pv_inside((ax, ay), r) for r in rings):
            fail(f"{what}: a slope arrow starts inside a roof plane: {s}")
    if sorted(dirs) != sorted(want_dirs):
        fail(f"{what}: slope arrows point down-slope {sorted(want_dirs)}, got {sorted(dirs)}")
    used = set()
    for l in labels:
        if l.get('layer') != 'A-ROOF-ANNO':
            fail(f"{what}: pitch labels are on A-ROOF-ANNO: {l}")
        p = pv_pt(l.get('at'), 'pitch label')
        inside = [i for i, r in enumerate(rings) if pv_inside(p, r)]
        if len(inside) != 1 or inside[0] in used:
            fail(f"{what}: each pitch label sits inside a different plane: {l} in planes {inside}")
        used.add(inside[0])


# (a) the hip at 6/12.
rr = run_ok(WALLS + [PV_HIP, ROOFPLAN], "the hip roof plan")
plan, roof = pv_plan(rr), objects_named(inspected(rr), 'roofs', 'roofs')[0]
if plan.get('kind') != 'roof':
    fail(f"the Roof Plan view's kind is roof: {plan.get('kind')}")
ridge, hips = pv_lines(plan, 'ridge'), pv_lines(plan, 'hip')
if len(ridge) != 1 or abs(pv_len(ridge[0]) - 2.0) > 1e-6:
    fail(f"THE NUMBER: one ridge line 2.0 m long: {[pv_len(l) for l in ridge]}")
if len(hips) != 4 or any(abs(pv_len(l) - 5.798) > 1e-3 for l in hips):
    fail(f"THE NUMBER: four hip lines of plan length 5.798: {[pv_len(l) for l in hips]}")
pitch = pv_labels(plan, 'pitch')
if [l.get('text') for l in pitch] != ["6/12"] * 4:
    fail(f"THE NUMBER: four 6/12 labels: {[l.get('text') for l in pitch]}")
check_arrows(plan, roof, [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)], "the hip")
eaves = pv_lines(plan, 'eave')
if any(l.get('layer') != 'A-ROOF-OTLN' for l in eaves) or pv_lines(plan, 'rake'):
    fail(f"the hip's eaves are on A-ROOF-OTLN and it has no rakes: {eaves} / {pv_lines(plan, 'rake')}")
near(sum(pv_len(l) for l in eaves), 36.8, 1e-6, "the hip's eaves (2 x 10.2 + 2 x 8.2)")
for l in eaves:
    (ax, ay), (bx, by) = pv_pt(l['a'], 'eave'), pv_pt(l['b'], 'eave')
    if not ((abs(ax - bx) < 1e-6 and abs(abs(ax - 5) - 5.1) < 1e-6) or (abs(ay - by) < 1e-6 and abs(abs(ay - 4) - 4.1) < 1e-6)):
        fail(f"each eave lies on a footprint side (x = -0.1 / 10.1 or y = -0.1 / 8.1): {l}")
print("ok (a): the 6/12 hip: one 2.0 m ridge, four 5.798 m hips, four 6/12 labels with down-slope arrows, 36.8 m of eaves")

# (b) the gable.
rr = run_ok(WALLS + [GABLE, ROOFPLAN], "the gable roof plan")
plan, roof = pv_plan(rr), objects_named(inspected(rr), 'roofs', 'roofs')[0]
if [l.get('text') for l in pv_labels(plan, 'pitch')] != ["6/12"] * 2:
    fail(f"the gable has two 6/12 labels: {pv_labels(plan, 'pitch')}")
check_arrows(plan, roof, [(0.0, -1.0), (0.0, 1.0)], "the gable")
eaves, rakes = pv_lines(plan, 'eave'), pv_lines(plan, 'rake')
near(sum(pv_len(l) for l in eaves), 20.4, 1e-6, "the gable's eaves (2 x 10.2)")
near(sum(pv_len(l) for l in rakes), 16.4, 1e-6, "the gable's rakes (4 x 4.1)")
if any(abs(abs(pv_pt(l['a'], 'eave')[1] - 4) - 4.1) > 1e-6 or abs(pv_pt(l['b'], 'eave')[1] - pv_pt(l['a'], 'eave')[1]) > 1e-6
       for l in eaves):
    fail(f"the gable's eaves lie on y = -0.1 and y = 8.1: {eaves}")
if any(abs(abs(pv_pt(l['a'], 'rake')[0] - 5) - 5.1) > 1e-6 or abs(pv_pt(l['b'], 'rake')[0] - pv_pt(l['a'], 'rake')[0]) > 1e-6
       for l in rakes) or any(l.get('layer') != 'A-ROOF-OTLN' for l in rakes):
    fail(f"the gable's rakes lie on x = -0.1 and x = 10.1 on A-ROOF-OTLN: {rakes}")
r = pv_lines(plan, 'ridge')
if len(r) != 1 or abs(pv_len(r[0]) - 10.2) > 1e-6:
    fail(f"the gable has one 10.2 m ridge: {[pv_len(l) for l in r]}")
print("ok (b): the gable: two 6/12 labels (arrows south and north), 20.4 m of eaves, 16.4 m of rakes, a 10.2 m ridge")

# (c) another pitch, and an overhang.
rr = run_ok(WALLS + [cmd('roofauto', {"pitchRise": 4, "pitchRun": 12}), ROOFPLAN], "a 4/12 hip")
if [l.get('text') for l in pv_labels(pv_plan(rr), 'pitch')] != ["4/12"] * 4:
    fail(f"a 4/12 hip reads 4/12 four times: {[l.get('text') for l in pv_labels(pv_plan(rr), 'pitch')]}")
rr = run_ok(WALLS + [cmd('roofauto', {"pitchRise": 6, "pitchRun": 12, "overhang": 0.5}), ROOFPLAN], "a hip with overhang")
near(sum(pv_len(l) for l in pv_lines(pv_plan(rr), 'eave')), 40.8, 1e-6, "eaves with a 0.5 m overhang (2 x 11.2 + 2 x 9.2)")
print("ok (c): 4/12 labels; a 0.5 m overhang moves the eaves out to 40.8 m")

# (d) no roof-plan annotation elsewhere.
rr = run_ok(WALLS + [PV_HIP, cmd('arch.inspect', {"plan": "Floor Plan"}), cmd('arch.inspect', {"plan": True})],
            "the hip in Floor Plan and with no view")
for nth, what in ((0, "Floor Plan"), (1, "no view open")):
    p = pv_plan(rr, nth)
    if pv_labels(p, 'pitch') or pv_lines(p, 'slopeArrow') or pv_lines(p, 'eave') or pv_lines(p, 'rake'):
        fail(f"{what}: no pitch labels, slope arrows, eaves or rakes")
    if len(pv_lines(p, 'hip')) != 4 or len(pv_lines(p, 'ridge')) != 1:
        fail(f"{what}: the ridge and hips are still drawn")
print("ok (d): Floor Plan and the plain plan keep only the ridge and hips")
print("PASS 05")
