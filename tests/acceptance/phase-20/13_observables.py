#!/usr/bin/env python3
# Observables first: every drawing carries the seven template plan views (arch.inspect {"planViews": true}: names in
# order, kinds, rules, room-label flags and annotation layers exactly as the plan defines them, any level, nothing
# open), and arch.inspect {"plan": ...} reports the plan's annotation: with no view open it is today's plan exactly (one
# name label per room at the vertex average of its polygon, 0.24 high on A-ROOM, and each roof's ridges and hips on
# A-ROOF, nothing else), made at the shown level (level.show moves it); {"plan": "<view>"} answers for a view without
# opening it, an unknown view or a bad value is refused. Room rows gain labelArea and ceilingText (the finished ceiling
# above the floor: 2.40, and 2.17 under phase 16's suspended stack). The templates survive a DXF round trip.
# A stub fails: no planViews, no plan object, no ceilingText.
# needs: p20-observables-1, p20-observables-2, p20-observables-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()
R = house_rooms()

# 1. The templates, on a fresh drawing and after a DXF round trip.
dxf = os.path.join(tmp, 'house.dxf')
rr = run_ok(HOUSE + [cmd('arch.inspect', {"planViews": True})], "arch.inspect planViews", save=dxf)
for label, rows in (("a fresh drawing", rr),
                    ("the drawing saved to DXF and reopened", run_ok([cmd('arch.inspect', {"planViews": True})],
                                                                      "planViews after a DXF reopen", file=dxf))):
    views, opened, active = pv_views(rows)
    names = [v.get('name') for v in views if isinstance(v, dict)]
    if names != PV_TEMPLATES:
        fail(f"{label}: the template plan views {PV_TEMPLATES} in that order, got {names}")
    for v in views:
        pv_expect_template(v)
        if v.get('level') is not None:
            fail(f"{label}: template {v.get('name')!r} is bound to no level, got level {v.get('level')!r}")
    if opened != [] or active is not None:
        fail(f"{label}: no plan view is open or active, got open {opened!r}, active {active!r}")
print("ok: the seven templates with their kinds, rules, label flags and annotation layers; none open; kept by DXF")

# 2. With no view open the annotation is today's plan: room names and the roof's ridges and hips.
rr = run_ok(HOUSE + [PV_HIP, cmd('arch.inspect', {"plan": True, "rooms": True, "roofs": True})], "arch.inspect plan")
plan, insp = pv_plan(rr), inspected(rr)
rooms = objects_named(insp, 'rooms', 'rooms')
roofs = objects_named(insp, 'roofs', 'roofs')
if plan.get('view') is not None or plan.get('kind') != 'floor' or plan.get('level') is not None:
    fail(f"no view open: view null, kind floor, level null (all levels shown), got "
         f"{ {k: plan.get(k) for k in ('view', 'kind', 'level')} }")
labels = pv_labels(plan)
if len(labels) != 3 or any(l.get('kind') != 'roomName' or l.get('layer') != 'A-ROOM' for l in labels):
    fail(f"no view open: exactly one roomName label on A-ROOM per room (3), got {json.dumps(labels)[:500]}")
by_h = pv_by_handle(labels)
for r in rooms:
    got = by_h.get(r.get('handle'))
    if not got or len(got) != 1:
        fail(f"room {r.get('id')}: one name label carrying the room's handle {r.get('handle')}, got {got}")
    l = got[0]
    poly = [pv_xy(q, 'room polygon') for q in r.get('polygon') or []]
    cx, cy = sum(p[0] for p in poly) / len(poly), sum(p[1] for p in poly) / len(poly)
    ax, ay = pv_pt(l.get('at'), 'label at')
    if abs(ax - cx) > 1e-9 or abs(ay - cy) > 1e-9:
        fail(f"room {r.get('id')}: its name sits at the vertex average ({cx}, {cy}) as today, got ({ax}, {ay})")
    if l.get('text') != (r.get('name') or r.get('id')):
        fail(f"room {r.get('id')}: the label reads its name, or r<n> when unnamed, got {l.get('text')!r}")
    near(l.get('height'), 0.24, 1e-12, f"room {r.get('id')} label height")
    near(l.get('rotation'), 0.0, 1e-12, f"room {r.get('id')} label rotation")
if len(roofs) != 1:
    fail(f"one roof: {json.dumps(roofs)[:300]}")
edges = [e for e in roofs[0].get('edges') or [] if e.get('kind') != 'eave']
lines = pv_lines(plan)
if len(lines) != len(edges) or len(edges) != 5:
    fail(f"no view open: exactly the roof's 5 non-eave edges (1 ridge, 4 hips) as lines, got {len(lines)} lines: "
         f"{json.dumps(lines)[:500]}")
for e in edges:
    a, b = pv_xy(e.get('a'), 'edge a'), pv_xy(e.get('b'), 'edge b')
    hit = [l for l in lines if l.get('kind') == e.get('kind') and l.get('layer') == 'A-ROOF'
           and l.get('handle') == roofs[0].get('handle')
           and {tuple(round(v, 6) for v in pv_pt(l.get(k), 'line')) for k in ('a', 'b')}
           == {tuple(round(v, 6) for v in p) for p in (a, b)}]
    if len(hit) != 1:
        fail(f"the roof's {e.get('kind')} {a}-{b} is one line on A-ROOF with the roof's handle: {json.dumps(lines)[:500]}")
    near(hit[0].get('length'), math.dist(a, b), 1e-9, f"the {e.get('kind')} line's length")
    if hit[0].get('dashed') is not False:
        fail(f"roof lines are solid: {hit[0]}")
print("ok: no view open, the plan annotation is today's room names (vertex average, 0.24, A-ROOM) and ridge and hips")

# 3. A named view answers without opening it; unknown views and bad values are refused.
rc, rr, so, se = run(HOUSE + [PV_HIP, cmd('arch.inspect', {"plan": "Roof Plan"}),
                              cmd('arch.inspect', {"plan": "Reflected Ceiling"}),
                              cmd('arch.inspect', {"plan": "Nope"}), cmd('arch.inspect', {"plan": 3}),
                              cmd('arch.inspect', {"planViews": True})], expect_ok=False)
roof_plan, rcp = pv_plan(rr, 0), pv_plan(rr, 1)
if (roof_plan.get('view'), roof_plan.get('kind')) != ("Roof Plan", "roof") or \
        (rcp.get('view'), rcp.get('kind')) != ("Reflected Ceiling", "reflectedCeiling"):
    fail(f"{{\"plan\": view}} names the view and its kind: {(roof_plan.get('view'), roof_plan.get('kind'))}, "
         f"{(rcp.get('view'), rcp.get('kind'))}")
if len(pv_lines(roof_plan, 'hip', 'A-ROOF')) != 4 or len(pv_lines(roof_plan, 'ridge', 'A-ROOF')) != 1:
    fail(f"every view lists the roof's ridge and hips: {json.dumps(pv_lines(roof_plan))[:400]}")
errs = errors(se)
if len(errs) != 2 or not any('Nope' in e for e in errs):
    fail(f"{{\"plan\": \"Nope\"}} and {{\"plan\": 3}} are refused with an error each (naming Nope): {errs}")
views, opened, active = pv_views(rr)
if opened != [] or active is not None:
    fail(f"inspecting a view opens nothing: open {opened!r}, active {active!r}")
print("ok: {\"plan\": view} reports Roof Plan (roof) and Reflected Ceiling (reflectedCeiling) without opening them; "
      "Nope and 3 are refused")

# 4. The plan is annotated at the shown level.
UP = [cmd('level', {"above": 0}),
      cmd('wall', {"points": [[0, 0], [6, 0], [6, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4,
                   "exterior": True, "level": 1}),
      cmd('roomdetect', {"level": 1})]
rr = run_ok(HOUSE + UP + [cmd('level.show', {"level": 0}), cmd('arch.inspect', {"plan": True, "rooms": True}),
                          cmd('level.show', {"level": 1}), cmd('arch.inspect', {"plan": True})], "level.show 0 and 1")
rooms = objects_named(inspected(rr, -2), 'rooms', 'rooms')
by_level = {lv: {r.get('handle') for r in rooms if r.get('levelIndex') == lv} for lv in (0, 1)}
for nth, lv in ((0, 0), (1, 1)):
    p = pv_plan(rr, nth)
    got = {l.get('handle') for l in pv_labels(p, 'roomName')}
    if p.get('level') != lv or got != by_level[lv] or not got:
        fail(f"level.show {lv}: the plan is annotated at level {lv} with that level's room names only "
             f"({sorted(by_level[lv])}), got level {p.get('level')!r} and {sorted(map(str, got))}")
print("ok: level.show 0 / 1 move the annotation to that level's rooms")

# 5. Room rows: labelArea and ceilingText.
rr = run_ok(HOUSE + [cmd('arch.inspect', {"rooms": True}), cmd('room', {"id": R['NE'], "ceilingStack": PV_SUSPENDED}),
                     cmd('arch.inspect', {"rooms": True})], "rooms rows with labelArea and ceilingText")
for nth, want in ((0, {"S": "2.40", "NW": "2.40", "NE": "2.40"}), (1, {"S": "2.40", "NW": "2.40", "NE": "2.17"})):
    insp = inspected(rr, nth)
    for k, rid in R.items():
        row = room_row(insp, rid)
        if row.get('labelArea') != 'interior' or row.get('ceilingText') != want[k]:
            fail(f"room {k} ({rid}){' after the suspended stack' if nth else ''}: labelArea 'interior' and "
                 f"ceilingText {want[k]!r}, got {row.get('labelArea')!r} and {row.get('ceilingText')!r}")
print("ok: rooms report labelArea interior and ceilingText 2.40 (2.17 under the suspended stack)")
print("PASS 13")
