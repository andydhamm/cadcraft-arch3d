#!/usr/bin/env python3
# Callout markers in plan name the sheet and view they open: every elevation, section and interior elevation placed on
# a sheet gets a marker in plan (phase 20's pipeline, layer A-ANNO-SYMB) labelled "<view number>/<sheet number>":
# the south elevation 1/A-201 south of the house, the section 2/A-201 at its cut line, the interior elevation 1/A-301
# inside its room; a view on no sheet gets none; sheet.open {"view"} switches to the sheet holding that view.
# A stub fails: no markers, wrong text, markers for unplaced views, sheet.open that opens nothing.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-sheet-command, p21-callouts, p21-interior-elevation, p20-observables-1, p20-observables-2, p20-observables-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

R = house_rooms()
NW = R['NW']
SET = HOUSE + [cmd('elevation', {"side": "south"}),                                   # v0
               cmd('section', {"from": [-1, 4], "to": [11, 4], "look": [0, 1]}),      # v1
               cmd('elevation', {"room": NW, "wall": "w5"}),                         # v2
               cmd('elevation', {"side": "north"}),                                   # v3, on no sheet
               cmd('sheet', {"name": "Elevations", "number": "A-201",
                             "views": [{"view": "v0", "scale": "1:100", "at": [250, 400]},
                                       {"view": "v1", "scale": "1:100", "at": [600, 400]}]}),
               cmd('sheet', {"name": "Interiors", "number": "A-301", "views": [{"view": "v2", "scale": "1:50", "at": [300, 300]}]})]
rr = sh_run_ok(SET + [INSPECT21, cmd('arch.inspect', {"plan": True}), cmd('sheet.open', {"view": "v1"}),
                      'layout.list {}'], "the callouts")
# The plan reply is an arch.inspect reply too (it carries walls and levels): the views are in the first one.
insp = inspected(rr, 0)
vh = {v.get('id'): v.get('handle') for v in sh_list(insp, 'views')}
plan = sh_plan(rr)
calls = sh_plan_labels(plan, 'callout')
by = {}
for c in calls:
    by.setdefault(c.get('handle'), []).append(c)
for vid, text in (('v0', '1/A-201'), ('v1', '2/A-201'), ('v2', '1/A-301')):
    got = by.get(vh.get(vid)) or []
    if not got or any(c.get('text') != text or c.get('layer') != 'A-ANNO-SYMB' for c in got):
        fail(f"{vid}'s callout reads {text} on A-ANNO-SYMB: {got or [(c.get('text'), c.get('handle')) for c in calls]}")
if by.get(vh.get('v3')):
    fail(f"v3 is on no sheet and has no callout: {by.get(vh.get('v3'))}")
x, y = sh_pt(by[vh['v0']][0].get('at'), 'v0 callout')
if not y < -0.1:
    fail(f"the south elevation's callout sits south of the house (y < -0.1): {(x, y)}")
for c in by[vh['v1']]:
    x, y = sh_pt(c.get('at'), 'v1 callout')
    if abs(y - 4) > 1.0 or not (x < 0 or x > 10):
        fail(f"the section's callouts sit at its cut line's ends (y ~ 4, outside the house): {(x, y)}")
x, y = sh_pt(by[vh['v2']][0].get('at'), 'v2 callout')
if not (0.1 < x < 5.95 and 3.05 < y < 7.9):
    fail(f"the interior elevation's callout sits in the north-west room: {(x, y)}")
if not [l for l in plan.get('lines') or [] if isinstance(l, dict) and l.get('kind') == 'callout' and l.get('layer') == 'A-ANNO-SYMB']:
    fail("the callout markers draw their symbol lines (kind callout) on A-ANNO-SYMB")
op = sh_reply(rr, 'layout', what='sheet.open')
if op.get('sheet') != 'Elevations' or op.get('layout') != 'Elevations':
    fail(f"sheet.open {{view: v1}} opens the Elevations sheet: {op}")
lays = [r for r in rr if isinstance(r, list)]
if not lays or not any(isinstance(l, dict) and l.get('name') == 'Elevations' and l.get('current') for l in lays[-1]):
    fail(f"after sheet.open the Elevations layout is current: {json.dumps(lays[-1] if lays else rr[-1])[:300]}")
print(f"ok: callouts 1/A-201, 2/A-201 (x{len(by[vh['v1']])}), 1/A-301 placed by kind; none for an unplaced view; "
      f"sheet.open goes to Elevations")
print("PASS 10")
