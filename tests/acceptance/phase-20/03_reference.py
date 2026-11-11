#!/usr/bin/env python3
# Reference display (the brief's "a view can draw another level beneath in a muted colour"): a plan view of level 1
# with a reference to level 0 draws level 0's walls (their poche outlines, on A-WALL, in #b4b4b4 at transparency 50 by
# default, carrying the walls' handles) beneath the shown level; nothing for level-1 walls; none when the reference is
# cleared or is the shown level; the colour is the view's and the reference is saved with it.
# A stub fails: reference_lines draws nothing, so (a) finds no line with the level-0 walls' handles.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-plan-render, p20-plan-views, p20-reference
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()
UP = HOUSE + [cmd('level', {"above": 0}),
              cmd('wall', {"points": [[0, 0], [6, 0], [6, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4,
                           "exterior": True, "level": 1}),
              cmd('planview', {"name": "Upper", "level": 1, "reference": {"level": 0}})]
svg1, svg2, svg3 = (os.path.join(tmp, n) for n in ('ref.svg', 'noref.svg', 'orange.svg'))
dxf = os.path.join(tmp, 'ref.dxf')
rr = run_ok(UP + [INSPECT20, cmd('arch.inspect', {"plan": True}), cmd('export', {"path": svg1}),
                  cmd('planview.edit', {"name": "Upper", "reference": None}), cmd('arch.inspect', {"plan": True}),
                  cmd('export', {"path": svg2}),
                  cmd('planview.edit', {"name": "Upper", "reference": {"level": 0, "color": "#ff8800"}}),
                  cmd('export', {"path": svg3})], "a level-1 view referencing level 0", save=dxf)
walls = objects_named(inspected(rr, 0), 'walls', 'walls')
low = {w['handle'] for w in walls if w.get('id') in ('w0', 'w1', 'w2', 'w3', 'w4', 'w5')}
high = {w['handle'] for w in walls if w.get('id') in ('w6', 'w7', 'w8', 'w9')}
if len(low) != 6 or len(high) != 4:
    fail(f"walls w0..w5 on level 0 and w6..w9 on level 1: {[(w.get('id'), w.get('level')) for w in walls]}")

# (a) the plan object.
plan = pv_plan(rr, 0)
if plan.get('kind') != 'floor' or plan.get('level') != 1 or plan.get('view') != 'Upper':
    fail(f"the plan shows view Upper, kind floor, level 1: {(plan.get('view'), plan.get('kind'), plan.get('level'))}")
ref = pv_lines(plan, 'reference')
by_h = pv_by_handle(ref)
if set(by_h) != low:
    fail(f"THE NUMBER: reference lines carry exactly the level-0 walls' handles {sorted(low)}, got {sorted(map(str, by_h))}")
for h, ls in by_h.items():
    if len(ls) < 4:
        fail(f"wall {h}: its poche outline is at least 4 reference lines, got {len(ls)}")
for l in ref:
    if l.get('layer') != 'A-WALL' or str(l.get('color', '')).lower() != '#b4b4b4' or l.get('transparency') != 50:
        fail(f"a reference line is on A-WALL in #b4b4b4 at transparency 50: {l}")
    for k in ('a', 'b'):
        x, y = pv_pt(l.get(k), 'reference point')
        if not (-0.1 - 1e-6 <= x <= 10.1 + 1e-6 and -0.1 - 1e-6 <= y <= 8.1 + 1e-6):
            fail(f"a reference line leaves level 0's outer faces -0.1..10.1 x -0.1..8.1: {l}")
print(f"ok (a): {len(ref)} reference lines, the outlines of w0..w5 on A-WALL in #b4b4b4 at 50 %, none of level 1")

# (b) the drawing.
if '#b4b4b4' not in pv_svg_colours(svg1):
    fail(f"the plan's SVG draws the reference in #b4b4b4: {sorted(pv_svg_colours(svg1))}")
if '#b4b4b4' in pv_svg_colours(svg2) or pv_lines(pv_plan(rr, 1), 'reference'):
    fail("with the reference cleared there is no reference line and no #b4b4b4 in the SVG")
if '#ff8800' not in pv_svg_colours(svg3):
    fail(f"the view's own reference colour #ff8800 is drawn: {sorted(pv_svg_colours(svg3))}")
artifact(svg1, 'reference.svg')
print("ok (b): the SVG draws the reference in its colour; clearing it removes it")

# (c) the shown level as reference draws nothing; the reference is saved.
rr = run_ok(UP + [cmd('planview', {"name": "Same", "level": 0, "reference": {"level": 0}}),
                  cmd('arch.inspect', {"plan": True})], "a reference to the shown level")
if pv_lines(pv_plan(rr), 'reference'):
    fail("a reference to the shown level draws no reference lines")
rr = run_ok([INSPECT20], "the drawing reopened", file=dxf)
up = pv_view(pv_views(rr)[0], 'Upper')
r = up.get('reference') or {}
if r.get('level') != 0 or str(r.get('color', '')).lower() != '#ff8800':
    fail(f"the reference survives a DXF round trip (level 0, #ff8800): {up}")
print("ok (c): no lines for the shown level; the reference survives a DXF save and reopen")
print("PASS 03")
