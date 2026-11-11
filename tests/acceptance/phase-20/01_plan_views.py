#!/usr/bin/env python3
# Plan views (the brief's plan-views bullet): a saved plan view is a named object in the drawing with a level (or any
# level), a layer state with transparency, an annotation set and a zoom. The seven templates are listed in order; views
# are created (from a template), duplicated, renamed, deleted, opened and closed as tabs; opening one applies its level,
# its wildcard layer rules (with transparency) or captured layers, its annotation layer and dimension style, and its
# zoom; opening creates the layers the plan draws the model's objects on so the rules can hide terrain and plants;
# views survive a DXF round trip.
# A stub fails: planview.list / planview / planview.open do not exist.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-plan-views
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()

# (a) the templates.
rr = run_ok([cmd('planview.list', {}), cmd('arch.inspect', {"planViews": True})], "planview.list on a fresh drawing")
lst = reply_with(rr, 'planViews', 0, 'planview.list')
if [v.get('name') for v in lst['planViews'] if isinstance(v, dict)] != PV_TEMPLATES:
    fail(f"planview.list names the templates {PV_TEMPLATES} in order: {json.dumps(lst)[:400]}")
if lst.get('open') != [] or lst.get('active') is not None:
    fail(f"nothing is open on a fresh drawing: {json.dumps(lst)[:300]}")
views, opened, active = pv_views(rr)
for v in views:
    pv_expect_template(v)
print("ok (a): planview.list lists the seven templates, none open")

# (b) create, rename, open, close, delete; tabs follow.
LV = cmd('level', {"above": 0})
rr = run_ok(HOUSE + [LV,
                     cmd('planview', {"name": "L2 Floor", "from": "Floor Plan", "level": 1}),
                     cmd('arch.inspect', {"plan": True, "planViews": True}),
                     cmd('planview.edit', {"name": "L2 Floor", "rename": "Upper Floor"}),
                     cmd('planview.open', {"name": "Electrical"}),
                     cmd('arch.inspect', {"plan": True, "planViews": True}),
                     cmd('planview.close', {"name": "Electrical"}),
                     cmd('arch.inspect', {"planViews": True}),
                     cmd('planview.delete', {"name": "Upper Floor"}),
                     cmd('arch.inspect', {"planViews": True})], "planview create / edit rename / open / close / delete")
created = reply_with(rr, 'planView', 0, 'planview')
if created.get('planView') != "L2 Floor" or created.get('open') != ["L2 Floor"] or created.get('active') != "L2 Floor":
    fail(f"planview L2 Floor opens and activates it: {json.dumps(created)}")
views, opened, active = pv_views(rr, 0)
l2 = pv_view(views, "L2 Floor")
if pv_rules(l2) != PV_TEMPLATE_SPEC["Floor Plan"][1] or l2.get('level') != 1 or l2.get('kind') != 'floor':
    fail(f"L2 Floor copies Floor Plan's rules and is bound to level 1: {json.dumps(l2)[:400]}")
if pv_plan(rr, 0).get('level') != 1 or pv_plan(rr, 0).get('view') != "L2 Floor":
    fail(f"opening L2 Floor shows level 1 with that view: {json.dumps(pv_plan(rr, 0))[:200]}")
views, opened, active = pv_views(rr, 1)
if opened != ["Upper Floor", "Electrical"] or active != "Electrical" or pv_plan(rr, 1).get('level') != 1:
    fail(f"after the rename and opening Electrical (an any-level view): tabs ['Upper Floor', 'Electrical'], "
         f"Electrical active, level 1 still shown; got {opened}, {active}, level {pv_plan(rr, 1).get('level')}")
views, opened, active = pv_views(rr, 2)
if opened != ["Upper Floor"] or active != "Upper Floor":
    fail(f"closing Electrical activates Upper Floor: {opened}, {active}")
views, opened, active = pv_views(rr, 3)
if [v.get('name') for v in views] != PV_TEMPLATES or opened != [] or active is not None:
    fail(f"deleting Upper Floor leaves the seven templates, no tab, none active: "
         f"{[v.get('name') for v in views]}, {opened}, {active}")
rc, rr, so, se = run(HOUSE + [cmd('planview', {"name": "working"}), cmd('planview.delete', {"name": "Nope"}),
                              cmd('arch.inspect', {"planViews": True})], expect_ok=False)
if len(errors(se)) != 2:
    fail(f"planview 'working' (Working exists, any case) and deleting 'Nope' are refused: {errors(se)}")
if [v.get('name') for v in pv_views(rr)[0]] != PV_TEMPLATES:
    fail("the refusals changed the plan views")
print("ok (b): create from Floor Plan bound to level 1, rename, open, close and delete follow the tabs; refusals")

# (c) the annotation set.
rr = run_ok(HOUSE + [cmd('planview.open', {"name": "Reflected Ceiling"}), cmd('getvar', {"name": "CLAYER"}), PV_LAYERS,
                     cmd('planview', {"name": "Notes", "annotation": {"layer": "A-ANNO-NOTE", "dimStyle": "Standard"}}),
                     cmd('getvar', {"name": "DIMSTYLE"}), cmd('getvar', {"name": "CLAYER"})], "annotation sets")
cl = [r for r in rr if isinstance(r, dict) and r.get('name') in ('CLAYER', 'DIMSTYLE') and 'value' in r]
if [(r['name'], str(r['value'])) for r in cl] != [("CLAYER", "A-CLNG-ANNO"), ("DIMSTYLE", "Standard"),
                                                 ("CLAYER", "A-ANNO-NOTE")]:
    fail(f"Reflected Ceiling sets CLAYER A-CLNG-ANNO; a view with annotation A-ANNO-NOTE / Standard sets them: {cl}")
pv_layer(pv_layer_rows(rr), "A-CLNG-ANNO", "opening Reflected Ceiling creates its annotation layer")
print("ok (c): the annotation layer and dimension style are applied on open")

# (d) zoom.
rr = run_ok(HOUSE + [cmd('planview.edit', {"name": "Floor Plan", "zoom": {"center": [5, 4], "width": 12, "height": 9}}),
                     cmd('zoom.extents', {}), cmd('planview.open', {"name": "Floor Plan"}), cmd('view.get', {})],
            "a view's zoom")
vg = [r for r in rr if isinstance(r, dict) and 'center' in r and 'height' in r][-1]
c = pv_pt(vg['center'], 'view center')
if abs(c[0] - 5) > 1e-9 or abs(c[1] - 4) > 1e-9 or num(vg['height'], 'view height') < 9 - 1e-9:
    fail(f"opening Floor Plan restores its zoom (centre (5, 4), height >= 9): {vg}")
print("ok (d): the view's zoom is restored on open")

# (e) a layer set with transparency, and captured layers.
dxf = os.path.join(tmp, 'views.dxf')
rr = run_ok(HOUSE + [cmd('planview', {"name": "Dim walls", "layers": {"rules": [
                         {"pattern": "*", "on": True, "transparency": 0}, {"pattern": "A-WALL", "transparency": 60}]}}),
                     PV_LAYERS,
                     cmd('layer.set', {"name": "A-DOOR", "on": False}),
                     cmd('planview', {"name": "Snap", "layers": {"capture": True}, "open": False}),
                     cmd('layer.set', {"name": "A-DOOR", "on": True}),
                     cmd('planview.open', {"name": "Snap"}), PV_LAYERS,
                     cmd('planview.open', {"name": "Dim walls"})], "rules and capture", save=dxf)
L = pv_layer_rows(rr, 0)
if pv_layer(L, 'A-WALL').get('transparency') != 60 or pv_layer(L, 'A-DOOR').get('transparency') != 0 \
        or not pv_layer(L, 'A-DOOR').get('on'):
    fail(f"'Dim walls' shows A-WALL at 60 and A-DOOR on at 0: {L.get('A-WALL')}, {L.get('A-DOOR')}")
if pv_layer(pv_layer_rows(rr, 1), 'A-DOOR').get('on') is not False:
    fail(f"'Snap' captured A-DOOR off: opening it turns A-DOOR off again: {pv_layer_rows(rr, 1).get('A-DOOR')}")
print("ok (e): wildcard rules with transparency, and captured layers")

# (f) the Floor Plan view hides roofs, terrain and plants.
rr = run_ok(HOUSE + [PV_HIP, PV_TERRAIN, PV_PLANT, PV_LAYERS,
                     cmd('planview.open', {"name": "Floor Plan"}), PV_LAYERS,
                     cmd('planview.open', {"name": "Reflected Ceiling"}), PV_LAYERS,
                     cmd('planview.open', {"name": "Site"}), PV_LAYERS], "Floor Plan / Reflected Ceiling / Site layers")
L0 = pv_layer_rows(rr, 0)
if 'C-TOPO-MAJR' in L0 or 'L-PLNT' in L0:
    fail(f"terrain and plant create no layer (the view must): {sorted(L0)}")
L1 = pv_layer_rows(rr, 1)
for name, on in (('A-ROOF', False), ('A-WALL', True), ('A-ROOM', True), ('C-TOPO-MAJR', False), ('C-TOPO-MINR', False),
                 ('L-PLNT', False)):
    if pv_layer(L1, name, "Floor Plan open").get('on') is not on:
        fail(f"Floor Plan: {name} {'on' if on else 'off'}: {L1[name]}")
L2 = pv_layer_rows(rr, 2)
for name in ('C-TOPO-MAJR', 'L-PLNT', 'A-ROOF'):
    if pv_layer(L2, name, "Reflected Ceiling open").get('on') is not False:
        fail(f"Reflected Ceiling: {name} off: {L2[name]}")
L3 = pv_layer_rows(rr, 3)
for name in ('C-TOPO-MAJR', 'L-PLNT'):
    if pv_layer(L3, name, "Site open").get('on') is not True:
        fail(f"Site: {name} on: {L3[name]}")
print("ok (f): opening a view creates the terrain, plant and room layers; Floor Plan and Reflected Ceiling hide them, "
      "Site shows them")

# (g) persistence.
rr = run_ok([cmd('planview.list', {}), cmd('arch.inspect', {"planViews": True})], "plan views after a DXF reopen", file=dxf)
views, opened, active = pv_views(rr)
names = [v.get('name') for v in views]
if names[:7] != PV_TEMPLATES or "Dim walls" not in names or "Snap" not in names:
    fail(f"the reopened drawing keeps the templates and 'Dim walls' and 'Snap': {names}")
if pv_rules(pv_view(views, "Dim walls")) != [_pv_r("*", True, 0), _pv_r("A-WALL", None, 60)]:
    fail(f"'Dim walls' keeps its rules: {pv_view(views, 'Dim walls')}")
if not isinstance(pv_view(views, "Snap").get('snapshot'), int) or pv_view(views, "Snap")['snapshot'] < 12:
    fail(f"'Snap' keeps its captured layers (at least the twelve default layers): {pv_view(views, 'Snap')}")
if "Dim walls" not in (opened or []) or active != "Dim walls":
    fail(f"'Dim walls' is still open and active after the reopen: {opened}, {active}")
print("ok (g): views, rules, captured layers and tabs survive a DXF round trip")
print("PASS 01")
