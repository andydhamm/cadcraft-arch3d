#!/usr/bin/env python3
# Corner cabinets (the brief's "a base or wall cabinet pushed into an inside corner where two runs meet becomes a
# corner cabinet (blind, diagonal door or lazy susan, its plan symbol showing the cut-out), and the counter runs round
# the corner in one piece", and the number "two base runs meeting at an inside corner give one corner cabinet and one
# counter polygon"). In the kitchen (inner faces x 0..6, y 0..4): a run of 0.9 + 0.6 + 0.6 along the south wall from
# the south-west corner and a run of two 0.6 down the west wall butting against it make cb0 a blind corner cabinet
# (leg 0.6) and ONE automatic counter of area 2.1 x 0.625 + 0.625 x 1.2 - 0.625 x 0.025 = 2.046875, probed on both
# legs with one id; `corner: none` splits them again. A run that starts 0.9 up the west wall makes the cabinet pushed
# into the corner a lazy susan (leg 0.9) with an L footprint, one counter of 2.234375, a hollow notch, and a diagonal
# footprint on demand; the plan symbol carries the footprint.
# A stub fails: no corner row, two counters, a rectangular footprint, a probe in the notch.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-corners
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

SOUTH = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 0.55, "widths": [0.9, 0.6, 0.6]})     # cb0 x 0..0.9, cb1, cb2 to 2.1
WEST = cmd('cabinet', {"kind": "base", "wall": "w3", "offset": 2.6, "widths": [0.6, 0.6]})             # cb3 y 1.2..1.8, cb4 y 0.6..1.2


def corners(insp):
    return [c for c in cabinets_of(insp) if isinstance(c.get('corner'), dict)]


# 1. Blind.
rr = run_ok(KITCHEN + [SOUTH, WEST, INSPECT18, probe_cmd([[1.0, 0.3, 0.9], [0.3, 1.5, 0.9]])], "two runs meeting in the south-west corner")
insp = inspected(rr)
cs = corners(insp)
if len(cs) != 1 or cs[0].get('id') != 'cb0':
    fail(f"exactly one corner cabinet, cb0: {[(c.get('id'), c.get('corner')) for c in cabinets_of(insp)]}")
co = cs[0]['corner']
if co.get('style') != 'blind' or co.get('end') != 'start' or co.get('partner') != 'cb4':
    fail(f"cb0 is a blind corner at its start end with partner cb4: {co}")
near(co.get('leg'), 0.6, 1e-9, "the blind corner's leg (the other run butts against the front)")
auto = countertops_of(insp, True)
if len(auto) != 1 or sorted(auto[0].get('cabinets') or []) != ['cb0', 'cb1', 'cb2', 'cb3', 'cb4']:
    fail(f"one automatic counter over all five cabinets: {[(r.get('id'), r.get('cabinets')) for r in auto]}")
near(auto[0].get('area'), 2.046875, 1e-9, "the merged counter's area")
g = probed(rr, [[1.0, 0.3, 0.9], [0.3, 1.5, 0.9]], what="merged counter")
hit(g[0], 'counter', auto[0]['id'], "the south leg of the counter")
hit(g[1], 'counter', auto[0]['id'], "the west leg of the counter (the same object)")
rr = run_ok(KITCHEN + [SOUTH, WEST, cmd('cabinet.edit', {"id": "cb0", "corner": "none"}), INSPECT18], "corner: none")
if corners(inspected(rr)) or len(countertops_of(inspected(rr), True)) != 2:
    fail(f"`corner: none` makes two plain runs with two counters: {[(r.get('id'), r.get('cabinets')) for r in countertops_of(inspected(rr))]}")
print("ok: a blind corner cabinet and one counter of 2.046875; corner: none splits them")

# 2. Lazy susan, then diagonal.
S2 = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.3, "widths": [0.6, 0.6]})     # x 0.9..2.1
W2 = cmd('cabinet', {"kind": "base", "wall": "w3", "offset": 2.3, "widths": [0.6, 0.6]})     # y 1.5..2.1, 0.9..1.5
PUSH = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 0.55, "width": 0.9})          # x 0..0.9, into the corner
rr = run_ok(KITCHEN + [S2, W2, PUSH, INSPECT18, probe_cmd([[0.3, 0.75, 0.5], [0.75, 0.75, 0.5]])], "a cabinet pushed into the corner")
insp = inspected(rr)
cs = corners(insp)
if len(cs) != 1:
    fail(f"one corner cabinet: {[(c.get('id'), c.get('corner')) for c in cabinets_of(insp)]}")
CID, co = cs[0]['id'], cs[0]['corner']
if co.get('style') != 'lazySusan':
    fail(f"the pushed-in cabinet is a lazy susan (the other run starts 0.9 out): {co}")
near(co.get('leg'), 0.9, 1e-9, "the lazy susan's leg")
same_vertex_set(co.get('footprint'), [(0, 0), (0.9, 0), (0.9, 0.6), (0.6, 0.6), (0.6, 0.9), (0, 0.9)], 1e-9, "the lazy susan's L footprint")
auto = countertops_of(insp, True)
if len(auto) != 1:
    fail(f"one counter round the corner: {[(r.get('id'), r.get('cabinets')) for r in auto]}")
near(auto[0].get('area'), 2.234375, 1e-9, "the lazy-susan counter's area")
g = probed(rr, [[0.3, 0.75, 0.5], [0.75, 0.75, 0.5]], what="lazy susan")
hit(g[0], 'cabinet', CID, "inside the L (y 0.6..0.9 at x 0.3)")
nohit(g[1], "the notch (x > 0.6, y > 0.6)")
rr = run_ok(KITCHEN + [S2, W2, PUSH, cmd('cabinet.edit', {"id": CID, "corner": "diagonal"}),
                       cmd('arch.inspect', {"walls": True, "levels": True, "cabinets": True, "symbols": True, "countertops": True})], "diagonal")
insp = inspected(rr)
co = by_id(cabinets_of(insp), CID, 'cabinets').get('corner') or {}
DIAG = [(0, 0), (0.9, 0), (0.9, 0.6), (0.6, 0.9), (0, 0.9)]
same_vertex_set(co.get('footprint'), DIAG, 1e-9, "the diagonal footprint")
sym = by_id(cabinets_of(insp), CID, 'cabinets').get('symbol')
if not isinstance(sym, list):
    fail("the cabinet row carries its plan symbol with symbols: true")
ok = False
for ring in sym:
    pts = [tuple(p[:2]) for p in ring if isinstance(p, list)]
    if pts and pts[0] == pts[-1]:
        pts = pts[:-1]
    if len(pts) == 5 and all(any(abs(p[0] - q[0]) < 1e-9 and abs(p[1] - q[1]) < 1e-9 for q in DIAG) for p in pts):
        ok = True
if not ok:
    fail(f"the plan symbol includes the diagonal footprint ring: {json.dumps(sym)[:400]}")
print("ok: lazy susan (leg 0.9, L footprint, counter 2.234375, hollow notch); diagonal footprint in the symbol")
print("PASS 12")
