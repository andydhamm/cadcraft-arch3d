#!/usr/bin/env python3
# Dimensions that know the building (the brief's number "moving the phase-4 window 1 m along its wall changes its
# locating dimension by 1.0 after arch.inspect, with no other command"): a linear dimension drawn from the west wall's
# outer face to the window's centre attaches to both (w3 rightFace, o0 centre) and follows the window; dimensions also
# attach to an interior wall's face (and follow it when it is stretched), to a wall's centre line, and to a cabinet's
# edge (and follow the cabinet); the attachments survive a DXF round trip; a dimension between free points stays put.
# A stub fails: no attachment, no "dimensions" rows, and the measurement stays 4.1.
# needs: p20-dims-model
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()
DIMS = cmd('arch.inspect', {"dimensions": True, "cabinets": True, "openings": True})


def made_dim(rows, nth):
    hs = [r.get('handle') for r in rows if isinstance(r, dict) and set(r) == {'handle'}]
    if not (-len(hs) <= nth < len(hs)):
        fail(f"dimlinear replies with its handle (#{nth}): {json.dumps(rows)[-300:]}")
    return hs[nth]


def expect_dim(rows, h, value, attached, what, nth=-1):
    d = pv_dim(pv_dims(rows, nth), h)
    near(d.get('measurement'), value, 1e-6, f"{what}: the measurement")
    got = pv_attached(d)
    for point, (obj, feat) in attached.items():
        if point not in got or got[point][0] != obj or (feat is not None and got[point][1] != feat):
            fail(f"{what}: {point} is attached to {obj} {feat or ''}, got {d.get('attached')}")
    return d


# (a) THE NUMBER: the window's locating dimension follows it.
A = HOUSE + [PV_WINDOW, cmd('dimlinear', {"p1": [-0.1, 8.0], "p2": [4.0, 8.0], "at": [2.0, 9.5]})]
dxf = os.path.join(tmp, 'dims.dxf')
rr = run_ok(A + [DIMS, cmd('arch.opening.move', {"opening": "o0", "offset": 5.0}), DIMS], "the window's dimension", save=dxf)
h = made_dim(rr, -1)
expect_dim(rr, h, 4.1, {"p13": ("w3", "rightFace"), "p14": ("o0", "centre")}, "before the move", nth=0)
d = expect_dim(rr, h, 5.1, {"p13": ("w3", "rightFace"), "p14": ("o0", "centre")}, "THE NUMBER: after moving o0 1 m")
p2 = pv_xy(d.get('p2'), 'p2')
if abs(p2[0] - 5.0) > 1e-6 or abs(p2[1] - 8.0) > 1e-6:
    fail(f"the dimension's second point follows the window's centre to (5, 8): {d.get('p2')}")
print("ok (a): 4.1 from w3's outer face to o0's centre; moving o0 by 1 m reads 5.1 with no other command")

# (b) a wall face: stretch w5 to x = 5.5.
rr = run_ok(HOUSE + [cmd('dimlinear', {"p1": [-0.1, 5.5], "p2": [5.95, 5.5], "at": [3, 6.5]}), DIMS,
                     cmd('arch.wall.stretch', {"wall": "w5", "end": "start", "to": [5.5, 3]}),
                     cmd('arch.wall.stretch', {"wall": "w5", "end": "end", "to": [5.5, 8]}), DIMS], "a wall face")
h = made_dim(rr, -1)
expect_dim(rr, h, 6.05, {"p13": ("w3", "rightFace"), "p14": ("w5", "leftFace")}, "to w5's face", nth=0)
expect_dim(rr, h, 5.55, {"p14": ("w5", "leftFace")}, "after w5 moved to x = 5.5")
print("ok (b): 6.05 to w5's left face follows the wall to 5.55")

# (c) a wall centre line.
rr = run_ok(HOUSE + [cmd('dimlinear', {"p1": [5.0, -0.1], "p2": [5.0, 3.0], "at": [11.0, 1.5], "rotation": 90}), DIMS,
                     cmd('arch.wall.stretch', {"wall": "w4", "end": "start", "to": [0, 3.5]}),
                     cmd('arch.wall.stretch', {"wall": "w4", "end": "end", "to": [10, 3.5]}), DIMS], "a centre line")
h = made_dim(rr, -1)
expect_dim(rr, h, 3.1, {"p13": ("w0", "rightFace"), "p14": ("w4", "centre")}, "to w4's centre line", nth=0)
expect_dim(rr, h, 3.6, {"p14": ("w4", "centre")}, "after w4 moved to y = 3.5")
print("ok (c): 3.1 to w4's centre line follows it to 3.6")

# (d) an object edge: a base cabinet against the north wall.
rr = run_ok(HOUSE + [cmd('cabinet', {"kind": "base", "wall": "w2", "offset": 8.0}), DIMS], "a base cabinet")
cabs = objects_named(inspected(rr), 'cabinets', 'cabinets')
if len(cabs) != 1:
    fail(f"one cabinet: {cabs}")
cb = cabs[0]
cx, cy = pv_xy(cb.get('at'), 'cabinet at')
w, dp = num(cb.get('width'), 'cabinet width'), num(cb.get('depth'), 'cabinet depth')
xw = cx - w / 2           # the cabinet stands square to the north wall (rotation 180): its least x
if abs(cy - 7.9) > 1e-6 or abs(float(cb.get('rotation', 0)) % 180) > 1e-6:
    fail(f"the cabinet's back edge is on w2's inner face y = 7.9, square to the wall: {cb}")
rr = run_ok(HOUSE + [cmd('cabinet', {"kind": "base", "wall": "w2", "offset": 8.0}),
                     cmd('dimlinear', {"p1": [0.1, 7.6], "p2": [xw, 7.6], "at": [xw / 2, 6.5], "rotation": 0}), DIMS,
                     cmd('cabinet.edit', {"id": cb['id'], "wall": "w2", "offset": 7.5}), DIMS], "a cabinet edge")
h = made_dim(rr, -1)
d0 = expect_dim(rr, h, xw - 0.1, {"p13": ("w3", "leftFace"), "p14": (cb['id'], "edge")}, "to the cabinet's edge", nth=0)
moved = by_id(objects_named(inspected(rr), 'cabinets', 'cabinets'), cb['id'], 'cabinets')
dx = pv_xy(moved.get('at'), 'cabinet at')[0] - cx
if abs(dx - 0.5) > 1e-6:
    fail(f"cabinet.edit wall w2 offset 7.5 moves the cabinet +0.5 in x (w2 runs toward -x): moved by {dx}")
expect_dim(rr, h, xw - 0.1 + 0.5, {"p14": (cb['id'], "edge")}, "after the cabinet moved 0.5")
print(f"ok (d): {xw - 0.1:.3f} to the cabinet's edge grows by 0.5 when the cabinet moves")

# (e) persistence.
rr = run_ok([DIMS, cmd('arch.opening.move', {"opening": "o0", "offset": 3.0}), DIMS], "the reopened drawing", file=dxf)
ds = pv_dims(rr, 0)
if len(ds) != 1:
    fail(f"the reopened drawing has the one dimension: {ds}")
h = ds[0].get('handle')
expect_dim(rr, h, 5.1, {"p13": ("w3", "rightFace"), "p14": ("o0", "centre")}, "after the DXF reopen", nth=0)
expect_dim(rr, h, 7.1, {"p14": ("o0", "centre")}, "after moving o0 to offset 3 (x = 7)")
print("ok (e): the attachments survive a DXF round trip and still follow")

# (f) free points stay free.
rr = run_ok(HOUSE + [PV_WINDOW, cmd('dimlinear', {"p1": [20, 20], "p2": [23, 20], "at": [21.5, 21]}), DIMS,
                     cmd('arch.opening.move', {"opening": "o0", "offset": 5.0}),
                     cmd('arch.wall.stretch', {"wall": "w5", "end": "start", "to": [5.5, 3]}), DIMS], "free points")
h = made_dim(rr, -1)
for nth in (0, 1):
    d = pv_dim(pv_dims(rr, nth), h)
    if d.get('attached'):
        fail(f"a dimension between free points attaches to nothing: {d}")
    near(d.get('measurement'), 3.0, 1e-9, "the free dimension")
print("ok (f): a dimension between free points stays unattached and unchanged")
print("PASS 07")
