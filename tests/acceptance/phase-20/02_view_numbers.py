#!/usr/bin/env python3
# The plan-views numbers: the Reflected Ceiling view's SVG export has no entity on I-FURN (I-FURN gets a colour of its
# own, #13579b, and the SVG must not contain it), with positive controls so an empty export fails (its walls #2a7f3c and
# the ceiling light on E-LITE #c8a000 are there; the Floor Plan export has the furniture and no E-LITE); and the
# Electrical view matches phase 15: view.electrical opens the Electrical plan view, keeps phase 15's reply, saves the
# layers from before the view as the layer state 'Electrical' (restoring it undims every layer), and draws exactly what
# planview.open Electrical draws. export {"planView"} leaves the session alone; unknown or combined requests are refused.
# A stub fails: export knows no planView and view.electrical names no plan view.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-plan-views, p20-view-export
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()

# (a) Reflected Ceiling: nothing on I-FURN.
SET = HOUSE + [cmd('layer.set', {"name": "A-WALL", "color": "#2a7f3c"}),
               cmd('layer.new', {"name": "I-FURN", "color": "#13579b"}),
               cmd('layer.new', {"name": "E-LITE", "color": "#c8a000"}),
               cmd('library.place', {"item": "armchair", "at": [3, 5]}),
               cmd('library.place', {"item": "light-ceiling", "at": [7, 5]})]
fp, rcp = os.path.join(tmp, 'floor.svg'), os.path.join(tmp, 'rcp.svg')
rc, rr, so, se = run(SET + [cmd('export', {"path": fp, "planView": "Floor Plan"}),
                            cmd('export', {"path": rcp, "planView": "Reflected Ceiling"}), INSPECT20], expect_ok=False)
if errors(se) or not os.path.exists(fp) or not os.path.exists(rcp):
    fail(f"export {{\"planView\"}} of Floor Plan and Reflected Ceiling: {errors(se) or 'no file written'}")
cf, cr = pv_svg_colours(fp), pv_svg_colours(rcp)
if '#13579b' not in cf or '#2a7f3c' not in cf or '#c8a000' in cf:
    fail(f"Floor Plan SVG: the armchair (#13579b) and the walls (#2a7f3c), no E-LITE (#c8a000): {sorted(cf)}")
if '#2a7f3c' not in cr or '#c8a000' not in cr:
    fail(f"Reflected Ceiling SVG: its walls (#2a7f3c) and the ceiling light on E-LITE (#c8a000): {sorted(cr)}")
if '#13579b' in cr:
    fail(f"THE NUMBER: the Reflected Ceiling SVG has an entity on I-FURN (#13579b): {sorted(cr)}")
views, opened, active = pv_views(rr)
if opened != [] or active is not None:
    fail(f"export {{\"planView\"}} leaves the session's tabs alone: open {opened}, active {active}")
artifact(rcp, 'reflected-ceiling.svg')
print("ok (a): the Reflected Ceiling SVG has walls and the ceiling light and nothing on I-FURN; Floor Plan has the "
      "furniture and no E-LITE; the session is untouched")

# (b) Electrical matches phase 15.
EL = [cmd('outlet', {"kind": "duplex", "wall": "w0", "offset": 1.0, "side": "left"}),
      cmd('switch', {"kind": "single-pole", "wall": "w3", "offset": 1.0, "side": "left"})]
a, b, c = (os.path.join(tmp, n) for n in ('open.png', 'view.png', 'plain.png'))
sa, sb = os.path.join(tmp, 'open.svg'), os.path.join(tmp, 'view.svg')
run_ok(ROOM64 + EL + [cmd('planview.open', {"name": "Electrical"}), cmd('export', {"path": a})], "planview.open Electrical")
rr = run_ok(ROOM64 + EL + [cmd('view.electrical', {}), cmd('layerstate.list', {}), PV_LAYERS, cmd('export', {"path": b}),
                           cmd('export', {"path": sb}), cmd('arch.inspect', {"planViews": True})], "view.electrical")
run_ok(ROOM64 + EL + [cmd('export', {"path": c})], "the plain plan")
run_ok(ROOM64 + EL + [cmd('export', {"path": sa, "planView": "Electrical"})], "export planView Electrical")
v = reply_with(rr, 'layerState', -1, 'view.electrical')
if v.get('layerState') != 'Electrical' or not {'E-POWR', 'E-LITE'} <= set(v.get('shown', [])) \
        or 'A-WALL' not in v.get('dimmed', []) or 'E-POWR' in v.get('dimmed', []) or v.get('planView') != 'Electrical':
    fail(f"view.electrical keeps phase 15's reply (E-POWR, E-LITE shown, A-WALL dimmed) and names the plan view: {v}")
if 'Electrical' not in json.dumps(reply_with(rr, 'states', 0, 'layerstate.list')):
    fail("view.electrical saves the layer state 'Electrical'")
L = pv_layer_rows(rr)
if not (pv_layer(L, 'E-POWR').get('on') and pv_layer(L, 'E-POWR').get('transparency') == 0
        and pv_layer(L, 'A-WALL').get('transparency') == 70):
    fail(f"after view.electrical: E-POWR on at 0, A-WALL at 70: {L.get('E-POWR')}, {L.get('A-WALL')}")
views, opened, active = pv_views(rr)
if active != 'Electrical' or 'Electrical' not in (opened or []):
    fail(f"view.electrical opens the Electrical plan view: open {opened}, active {active}")
if digest(a) != digest(b) or digest(b) == digest(c) or luma_sd(b) < 1:
    fail("THE NUMBER: planview.open Electrical and view.electrical draw the same plan, which differs from the plain "
         "plan and is not blank")
if open(sa, 'rb').read() != open(sb, 'rb').read():
    fail("export {\"planView\": \"Electrical\"} writes the same SVG as the plan after view.electrical")
rr0 = run_ok(ROOM64 + EL + [PV_LAYERS, cmd('view.electrical', {}), cmd('layerstate.restore', {"name": "Electrical"}),
                            PV_LAYERS], "view.electrical then layerstate.restore Electrical")
before, after = pv_layer_rows(rr0, 0), pv_layer_rows(rr0, 1)
if pv_layer(after, 'A-WALL').get('transparency') != 0:
    fail(f"restoring 'Electrical' undims A-WALL (the saved state is the layers from before the view): {after['A-WALL']}")
for name, row in before.items():
    if pv_layer(after, name).get('on') != row.get('on') or after[name].get('transparency') != row.get('transparency'):
        fail(f"restoring 'Electrical' gives {name} back its state from before view.electrical: {row} -> {after[name]}")
artifact(b, 'electrical-view.png')
print("ok (b): view.electrical = the Electrical plan view (same PNG and SVG as planview.open / export planView), "
      "phase 15's reply, and the saved state restores the layers from before")

# (c) refusals write nothing.
bad1, bad2 = os.path.join(tmp, 'nope.svg'), os.path.join(tmp, 'both.svg')
rc, rr, so, se = run(HOUSE + [cmd('export', {"path": bad1, "planView": "Nope"}),
                              cmd('export', {"path": bad2, "planView": "Site", "level": 0}), 'arch.inspect {}'],
                     expect_ok=False)
if len(errors(se)) != 2 or os.path.exists(bad1) or os.path.exists(bad2):
    fail(f"export planView Nope and planView + level are refused and write no file: {errors(se)}")
print("ok (c): export of an unknown plan view, or of a plan view with a level, is refused and writes nothing")
print("PASS 02")
