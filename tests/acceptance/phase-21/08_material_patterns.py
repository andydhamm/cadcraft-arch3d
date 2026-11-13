#!/usr/bin/env python3
# Brief numbers: a wall in phase 13's brick shows horizontal pattern lines across its face in the front elevation, and
# the same wall in a plain paint shows none. The south wall w0 (exterior face y = -0.1, u = x -0.1..10.1, z 0..2.4)
# with a 0.9 x 1.2 window at x = 5 (sill 0.9): brick courses every 0.075 m (the brick texture's 0.6 m tile holds 8),
# clipped to the face (none inside the window, some on both sides of it), on A-ELEV-PATT; painted plain #c04020 it has
# none, nor has the unfinished north wall in the north elevation. The drawing's SVG draws the pattern layer's colour
# only for the brick.
# A stub fails: no pattern, lines outside the face or across the window, a pattern under a plain paint.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-view-plumbing, p21-patterns, p21-view-render
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
SET = [house_walls()[0], cmd('window', {"wall": "w0", "offset": 5.0, "width": 0.9, "height": 1.2, "sill": 0.9}),
       cmd('layer.new', {"name": "A-ELEV-PATT", "color": "#7b2d26"}),
       cmd('material', {"walls": ["w0"], "finish": "brick"})]
ELEV = [cmd('elevation', {"side": "south", "dimensions": False}), cmd('elevation', {"side": "north", "dimensions": False})]
PAINT = cmd('paint.material', {"material": "#c04020", "target": "wall/w0/right", "scope": "component"})
brick_svg, plain_svg = os.path.join(tmp, 'brick.svg'), os.path.join(tmp, 'plain.svg')

rr = sh_run_ok(SET + ELEV + [INSPECT21, cmd('export', {"path": brick_svg})], "the brick wall")
insp = inspected(rr)
south, north = sh_view(insp, 'v0'), sh_view(insp, 'v1')
pat = sh_items(south, 'patterns', 'w0')
horiz = [l for l in pat if abs(sh_pt(l.get('a'), 'a')[1] - sh_pt(l.get('b'), 'b')[1]) <= 1e-9]
if len(horiz) < 20:
    fail(f"the brick face shows horizontal course lines (at least 20 over 2.4 m): {len(horiz)} of {len(pat)} pattern lines")
if not all('brick' in str(l.get('material')) for l in pat):
    fail(f"the pattern lines name the brick material: {sorted(set(str(l.get('material')) for l in pat))}")
for l in pat:
    for k in ('a', 'b'):
        u, z = sh_pt(l.get(k), 'pattern')
        if not (-0.1 - SH_TOL <= u <= 10.1 + SH_TOL and -SH_TOL <= z <= 2.4 + SH_TOL):
            fail(f"a pattern line leaves the wall's face (u -0.1..10.1, z 0..2.4): {l}")
    (ua, za), (ub, zb) = sh_pt(l.get('a'), 'a'), sh_pt(l.get('b'), 'b')
    if abs(za - zb) <= 1e-9 and 0.91 < za < 2.09 and min(ua, ub) < 5.44 and max(ua, ub) > 4.56:
        fail(f"a course line crosses the window (u 4.55..5.45, z 0.9..2.1): {l}")
levels = sorted({round(sh_pt(l.get('a'), 'a')[1], 6) for l in horiz})
gaps = sorted(b - a for a, b in zip(levels, levels[1:]))
if not gaps or abs(gaps[len(gaps) // 2] - 0.075) > SH_TOL:
    fail(f"brick courses every 0.075 m (the 0.6 m tile's 8 courses): course heights {levels[:12]}...")
mid = [l for l in horiz if 0.95 < sh_pt(l.get('a'), 'a')[1] < 2.05]
left = any(max(sh_pt(l.get('a'), 'a')[0], sh_pt(l.get('b'), 'b')[0]) < 4.6 for l in mid)
right = any(min(sh_pt(l.get('a'), 'a')[0], sh_pt(l.get('b'), 'b')[0]) > 5.4 for l in mid)
if not (left and right):
    fail("beside the window the courses run on both sides of it (clipped, not dropped)")
if sh_items(north, 'patterns'):
    fail(f"the unfinished north wall shows no pattern: {sh_items(north, 'patterns')[:3]}")
if '#7b2d26' not in sh_svg_colours(brick_svg):
    fail("the drawing's SVG draws no pattern line in A-ELEV-PATT's colour #7b2d26")

rr = sh_run_ok(SET + [PAINT] + ELEV + [INSPECT21, cmd('export', {"path": plain_svg})], "the painted wall")
south = sh_view(inspected(rr), 'v0')
if sh_items(south, 'patterns', 'w0'):
    fail(f"painted plain #c04020 the wall shows no pattern: {len(sh_items(south, 'patterns', 'w0'))} lines")
if '#7b2d26' in sh_svg_colours(plain_svg):
    fail("the painted wall's SVG still draws pattern lines (#7b2d26)")
print(f"ok: {len(horiz)} brick course lines 0.075 m apart, clipped round the window; none under a plain paint or on the "
      f"unfinished wall")
print("PASS 08")
