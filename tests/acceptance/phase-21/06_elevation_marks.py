#!/usr/bin/env python3
# Elevations carry the schedule numbers and opening indicators on their own layers: in the south elevation (u = x) the
# door (0.9 x 2.1 at x = 3) shows "D01" and the window (0.9 x 1.2, sill 0.9, at x = 7) "W01", each inside its opening
# on A-ELEV-IDEN; the hinged door's swing triangle (A-ELEV-SWNG) has its apex at mid-height on one jamb and its two
# other ends at the far jamb's top and bottom; the labels and indicators are not part of the linework or the extent;
# the drawn model's SVG has the A-ELEV-SWNG and A-ELEV-IDEN colours.
# A stub fails: no label or swing, a label outside its opening, a triangle of the wrong shape.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-view-plumbing, p21-elevation-marks, p21-view-render, p21-schedule-model
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
svg = os.path.join(tmp, 'model.svg')
SET = [house_walls()[0],
       cmd('door', {"wall": "w0", "offset": 3.0, "width": 0.9, "height": 2.1, "style": "hinged"}),   # o0
       cmd('window', {"wall": "w0", "offset": 7.0, "width": 0.9, "height": 1.2, "sill": 0.9}),          # o1
       cmd('layer.new', {"name": "A-ELEV-SWNG", "color": "#b03a2e"}),
       cmd('layer.new', {"name": "A-ELEV-IDEN", "color": "#1f618d"})]
rr = sh_run_ok(SET + [cmd('elevation', {"side": "south", "dimensions": False}), INSPECT21,
                      cmd('export', {"path": svg})], "the south elevation")
v = sh_view(inspected(rr), 'v0')
BOX = {'o0': (2.55, 0.0, 3.45, 2.1), 'o1': (6.55, 0.9, 7.45, 2.1)}
for oid, text in (('o0', 'D01'), ('o1', 'W01')):
    labs = sh_items(v, 'labels', oid)
    if len(labs) != 1 or labs[0].get('text') != text or labs[0].get('layer') != 'A-ELEV-IDEN':
        fail(f"{oid}: one label {text} on A-ELEV-IDEN: {labs or sh_items(v, 'labels')}")
    u, z = sh_pt(labs[0].get('at'), f"{oid} label")
    b = BOX[oid]
    if not (b[0] < u < b[2] and b[1] < z < b[3]):
        fail(f"{oid}'s label sits inside its opening {b}: {(u, z)}")
sw = sh_items(v, 'indicators', 'o0')
if len(sw) != 2:
    fail(f"the hinged door's swing is two lines: {sw}")
ends = [sh_pt(l.get(k), 'swing') for l in sw for k in ('a', 'b')]
apex = [p for p in ends if sum(1 for q in ends if abs(p[0] - q[0]) <= SH_TOL and abs(p[1] - q[1]) <= SH_TOL) == 2]
if not apex:
    fail(f"the two swing lines share an apex: {sw}")
ax, az = apex[0]
x0, z0, x1, z1 = BOX['o0']
side = min((x0, x1), key=lambda x: abs(x - ax))
far = x1 if side == x0 else x0
if abs(ax - side) > 0.08 or abs(az - (z0 + z1) / 2) > 0.08:
    fail(f"the apex sits at mid-height (z {(z0 + z1) / 2}) on a jamb (x {x0} or {x1}): {apex[0]}")
others = sorted([p for p in ends if abs(p[0] - ax) > SH_TOL or abs(p[1] - az) > SH_TOL], key=lambda p: p[1])
if len(others) != 2 or any(abs(p[0] - far) > 0.08 for p in others) or abs(others[0][1] - z0) > 0.08 or abs(others[1][1] - z1) > 0.08:
    fail(f"the swing's other ends sit at the far jamb's bottom and top (x {far}, z {z0} and {z1}): {others}")
for l in sh_items(v, 'lines'):
    if l.get('id') == 'o0' and any(sh_covers([l], ends[i], ends[i + 1]) for i in (0, 2)):
        fail("the swing lines are reported among the view's linework too")
ext = v.get('extent') or []
if len(ext) != 4 or abs(ext[0] + 0.1) > SH_TOL or abs(ext[2] - 10.1) > SH_TOL:
    fail(f"labels and indicators do not grow the extent (u -0.1..10.1): {ext}")
cols = sh_svg_colours(svg)
for col, what in (('#b03a2e', 'A-ELEV-SWNG'), ('#1f618d', 'A-ELEV-IDEN')):
    if col not in cols:
        fail(f"the drawing's SVG draws nothing in {what}'s colour {col}")
print(f"ok: D01 and W01 inside their openings, the door's swing triangle, extent unchanged, both layers drawn")
print("PASS 06")
