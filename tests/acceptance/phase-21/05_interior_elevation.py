#!/usr/bin/env python3
# Brief numbers: in the phase-1 house, the north-west room's elevation of its east wall (the interior wall x = 6, w5)
# has every segment within y 3.05..7.9 (+-1e-3), and a 0.6 m base cabinet against that wall shows as its front
# rectangle. Also: only that room's contents (a cabinet against the east wall of the north-east room behind w5 is not
# drawn, nor are the walls that do not bound the room), nothing below the floor or above the finished ceiling, the
# cabinet's door swing, and automatic dimensions for the cabinet run (0.6) and the overall height (2.4); "side": "east"
# picks the same wall.
# The look (1, 0) makes u = -y (u = p . (ly, -lx)).
# The cabinet has two doors, so the gap between them is a front line well inside the rectangle (a single door's edges
# sit 3 mm inside the box sides, which the first version of this check, with its 10 mm margin, could never see).
# A stub fails: an ordinary elevation (it draws the north-east room's cabinet and the outer walls), a missing
# cabinet outline, a bare box with no fronts, no swing or no dimensions.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-view-plumbing, p21-interior-elevation, p21-interior-dims, p21-elevation-marks, p21f-interior-cabinet-front
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

R = house_rooms()
NW = R['NW']
# cb0: a two-door base cabinet against w5 on its north-west face (x = 5.95), along y 5.2..5.8.
CAB2 = cmd('cabinet', {"kind": "base", "wall": "w5", "offset": 2.5, "fronts": "doors", "doors": 2})
SET = HOUSE + [CAB2, cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 5.5})]   # cb1 in the north-east room
rr = sh_run_ok(SET + [cmd('elevation', {"room": NW, "wall": "w5"}), cmd('elevation', {"room": NW, "side": "east"}),
                      INSPECT21], "the interior elevations")
insp = inspected(rr)
v = sh_view(insp, 'v0')
if v.get('kind') != 'interior' or v.get('room') != NW or v.get('wall') != 'w5':
    fail(f"v0 is an interior elevation of {NW} on w5: kind {v.get('kind')!r}, room {v.get('room')!r}, wall {v.get('wall')!r}")
lx, ly = sh_pt(v.get('look'), 'look')
if abs(lx - 1) > 1e-9 or abs(ly) > 1e-9:
    fail(f"looking from the north-west room at w5 is looking east (1, 0): {v.get('look')}")
v1 = sh_view(insp, 'v1')
if v1.get('kind') != 'interior' or v1.get('wall') != 'w5':
    fail(f"side 'east' of {NW} is w5: {v1.get('kind')}, {v1.get('wall')}")
lines = sh_items(v, 'lines')
if len(lines) < 8:
    fail(f"the interior elevation has hardly any lines: {len(lines)}")
cab_rows = {c.get('id'): c for c in objects_named(insp, 'cabinets', 'cabinets')}
if 'cb0' not in cab_rows:
    fail(f"arch.inspect lists no cabinet cb0: {sorted(cab_rows)}")
top = num(cab_rows['cb0'].get('counterTop'), 'cb0 counterTop')
for l in lines:
    for k in ('a', 'b'):
        u, z = sh_pt(l.get(k), f"line {k}")
        y = -u
        if not (3.05 - SH_TOL <= y <= 7.9 + SH_TOL):
            fail(f"a segment of {l.get('kind')} {l.get('id')} reaches y = {y:.4f}, outside 3.05..7.9: {l}")
        if not (-SH_TOL <= z <= 2.4 + SH_TOL):
            fail(f"a segment of {l.get('kind')} {l.get('id')} reaches z = {z:.4f}, outside the floor..ceiling 0..2.4: {l}")
for other in ('cb1', 'w0', 'w1', 'w3'):
    if sh_items(v, 'lines', other):
        fail(f"{other} is not in front of w5 inside the north-west room, yet it is drawn: {sh_items(v, 'lines', other)[:2]}")
face = sh_items(v, 'lines', 'w5')
if not face or not any(sh_covers(face, (-7.9, z), (-3.05, z)) for z in (0.0, 2.4)):
    fail(f"w5's face is drawn across the room (y 3.05..7.9 at the floor or the ceiling): {face[:6]}")
cab = sh_items(v, 'lines', 'cb0', 'cabinet')
if not cab:
    fail(f"the base cabinet cb0 is not drawn: {sorted(set((l.get('kind'), l.get('id')) for l in lines))}")
u0, z0, u1, z1 = sh_box(cab, 'cb0')
if abs(-u1 - 5.2) > SH_TOL or abs(-u0 - 5.8) > SH_TOL or abs(z0) > SH_TOL or abs(z1 - top) > SH_TOL:
    fail(f"cb0's front spans y 5.2..5.8 and z 0..{top}: got y {-u1:.4f}..{-u0:.4f}, z {z0:.4f}..{z1:.4f}")
for u in (u0, u1):
    if not sh_covers(cab, (u, 0.0), (u, top)):
        fail(f"cb0's front rectangle has its side at y = {-u:.3f} from the floor to the counter top {top}")
if not sh_covers(cab, (u0, top), (u1, top)):
    fail(f"cb0's front rectangle has its top edge at z = {top}")
# The gap between the two doors is at the middle of the front (y = 5.5, u = -5.5): a vertical front line there.
mid = (u0 + u1) / 2
inner = []
for l in cab:
    (ua, za), (ub, zb) = sh_pt(l.get('a'), 'p'), sh_pt(l.get('b'), 'p')
    if abs(ua - mid) <= 0.01 and abs(ub - mid) <= 0.01 and abs(za - zb) >= 0.3:
        inner.append(l)
if not inner:
    fail(f"cb0 shows its two door fronts inside the rectangle (a vertical line at the door gap, y = {-mid:.3f}, at "
         f"least 0.3 long): {[(l.get('a'), l.get('b')) for l in cab][:12]}")
swing = sh_items(v, 'indicators', 'cb0')
if len(swing) < 2:
    fail(f"cb0's door swing (at least two indicator lines): {swing}")
for l in swing:
    for k in ('a', 'b'):
        u, z = sh_pt(l.get(k), 'swing')
        if not (u0 - SH_TOL <= u <= u1 + SH_TOL and -SH_TOL <= z <= top + SH_TOL):
            fail(f"cb0's swing line leaves the cabinet front: {l}")
dims = [d for d in v.get('dims') or [] if isinstance(d, dict)]
if not any(d.get('kind') == 'width' and abs(num(d.get('value'), 'dim') - 0.6) <= SH_TOL for d in dims):
    fail(f"a 0.6 width dimension for the cabinet run: {[(d.get('kind'), d.get('value')) for d in dims]}")
if not any(d.get('kind') == 'height' and abs(num(d.get('value'), 'dim') - 2.4) <= SH_TOL for d in dims):
    fail(f"the overall floor-to-ceiling height dimension 2.4: {[(d.get('kind'), d.get('value')) for d in dims]}")
print(f"ok: interior elevation of {NW} on w5 (east): {len(lines)} lines all within y 3.05..7.9 and z 0..2.4; cb0's "
      f"front 0.6 x {top} with its door gap; {len(swing)} swing lines; dims 0.6 and 2.4")
print("PASS 05")
