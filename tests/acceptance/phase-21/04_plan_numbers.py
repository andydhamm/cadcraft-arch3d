#!/usr/bin/env python3
# Schedule numbers show as labels in plan on their own layers (A-GLAZ-IDEN, A-DOOR-IDEN, A-FLOR-CASE-IDEN): through
# phase 20's plan pipeline, in the views that show numbers (Floor Plan), outside the wall for an opening and on the
# cabinet for casework; never in the plan with no view open (older plans stay as they were) nor in the Reflected
# Ceiling view; a view can switch them off; a resize renumbers its label live; the Floor Plan's SVG draws them in their
# layer's colour and the Reflected Ceiling's does not.
# A stub fails: no labels, wrong text or layer, a label inside the house, labels in the plain plan.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-plan-numbers, p21-schedule-model, p20-observables-1, p20-observables-2, p20-observables-3, p20-plan-views, p20-view-export
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
SET = HOUSE + SH_WINDOWS + [cmd('door', {"wall": "w3", "offset": 1.5, "width": 0.9, "height": 2.1}), SH_CAB,
                            cmd('layer.new', {"name": "A-GLAZ-IDEN", "color": "#6a2c91"})]
PLANS = [cmd('arch.inspect', {"plan": "Floor Plan"}), cmd('arch.inspect', {"plan": True}),
         cmd('arch.inspect', {"plan": "Reflected Ceiling"}),
         cmd('planview', {"name": "No Numbers", "from": "Floor Plan", "labels": {"numbers": False}, "open": False}),
         cmd('arch.inspect', {"plan": "No Numbers"})]
rr = sh_run_ok(SET + [INSPECT21] + PLANS, "plan schedule labels")
# The plan replies are arch.inspect replies too (they carry walls and levels): the object lists are in the first one.
insp = inspected(rr, 0)
handles = {o.get('id'): o.get('handle') for o in objects_named(insp, 'openings', 'openings')}
handles.update({c.get('id'): c.get('handle') for c in objects_named(insp, 'cabinets', 'cabinets')})
floor = sh_plan_labels(sh_plan(rr, 0), 'scheduleNumber')
want = {'o0': ('W01', 'A-GLAZ-IDEN'), 'o1': ('W01', 'A-GLAZ-IDEN'), 'o2': ('W01', 'A-GLAZ-IDEN'),
        'o3': ('W01', 'A-GLAZ-IDEN'), 'o4': ('W02', 'A-GLAZ-IDEN'), 'o5': ('D01', 'A-DOOR-IDEN'),
        'cb0': ('C01', 'A-FLOR-CASE-IDEN')}
for oid, (text, layer) in want.items():
    hits = [l for l in floor if l.get('handle') == handles.get(oid)]
    if len(hits) != 1 or hits[0].get('text') != text or hits[0].get('layer') != layer:
        fail(f"Floor Plan: one {text} label on {layer} for {oid} (handle {handles.get(oid)}): {hits or floor[:8]}")
if len(floor) != len(want):
    fail(f"Floor Plan: {len(want)} schedule-number labels, got {len(floor)}: {[l.get('text') for l in floor]}")
for oid in ('o0', 'o1', 'o2'):
    x, y = sh_pt([l for l in floor if l.get('handle') == handles[oid]][0].get('at'), f"{oid} label")
    if not y < -0.1:
        fail(f"{oid}'s label sits outside the south wall (y < -0.1): y = {y}")
for oid in ('o3', 'o4'):
    x, y = sh_pt([l for l in floor if l.get('handle') == handles[oid]][0].get('at'), f"{oid} label")
    if not y > 8.1:
        fail(f"{oid}'s label sits outside the north wall (y > 8.1): y = {y}")
x, y = sh_pt([l for l in floor if l.get('handle') == handles['cb0']][0].get('at'), "cb0 label")
if not (5.35 - 1e-6 <= x <= 5.95 + 1e-6 and 5.2 - 1e-6 <= y <= 5.8 + 1e-6):
    fail(f"cb0's label sits on the cabinet (x 5.35..5.95, y 5.2..5.8): {(x, y)}")
for nth, what in ((1, 'the plan with no view open'), (2, 'Reflected Ceiling'), (3, 'a view with numbers off')):
    extra = sh_plan_labels(sh_plan(rr, nth), 'scheduleNumber')
    if extra:
        fail(f"{what} shows no schedule numbers: {[l.get('text') for l in extra]}")

# Live: resizing o1 renumbers its label.
rr = sh_run_ok(SET + [cmd('opening.edit', {"id": "o1", "width": 1.2}), cmd('arch.inspect', {"plan": "Floor Plan"})],
               "a resize")
lab = [l for l in sh_plan_labels(sh_plan(rr), 'scheduleNumber') if l.get('handle') == handles['o1']]
if len(lab) != 1 or lab[0].get('text') != 'W02':
    fail(f"after resizing o1 to 1.2 x 1.2 its label reads W02: {lab}")

# Drawn: the Floor Plan's SVG has the A-GLAZ-IDEN colour, the Reflected Ceiling's has not.
fp, rcp = os.path.join(tmp, 'fp.svg'), os.path.join(tmp, 'rcp.svg')
sh_run_ok(SET + [cmd('export', {"path": fp, "planView": "Floor Plan"}),
                 cmd('export', {"path": rcp, "planView": "Reflected Ceiling"})], "plan exports")
if '#6a2c91' not in sh_svg_colours(fp):
    fail("the Floor Plan SVG draws no schedule label in A-GLAZ-IDEN's colour #6a2c91")
if '#6a2c91' in sh_svg_colours(rcp):
    fail("the Reflected Ceiling SVG draws schedule labels (#6a2c91)")
print(f"ok: {len(floor)} schedule-number labels in the Floor Plan on their layers, none in the plain plan, the RCP or a "
      f"view with numbers off; o1 relabels W02 live; SVG colours follow the views")
print("PASS 04")
