#!/usr/bin/env python3
# Openings in detail, part 1. Grilles: the phase-4 window with 2 x 2 lites has 20 x 20 mm bars in the glass plane that
# divide the glass evenly (a vertical bar at the glass centre, a horizontal one at mid-height): on the plane reported
# by arch.inspect (glassCentre, glassNormal) the centre and 0.15 m beside it are grille, 0.15 m beside and 0.15 m above
# is glass; part "top" grilles only the upper half. A fixed door is a glazed leaf that does not open: its plan symbol
# has no swing arc (every polyline is two points) and its centre probes as glass. Sills and casings switch per side:
# "both" gives two casing and two sill parts, "none" neither. A stub (no bars, one casing) fails.
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-grilles-doors
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

# (a) 2 x 2 lites over the whole sash.
rr = run_ok(HOUSE + [GRILLE22, INSPECT17], "the 2 x 2 window")
w = openings_of(inspected(rr))[0]
if w.get('grille') != {"across": 2, "down": 2, "part": "whole"}:
    fail(f"the window row reports its grille {{across 2, down 2, part whole}}: {w.get('grille')!r}")
c, a, n = glass_frame(w, "the 2 x 2 window")
P = [c, at(c, a, 0.15, 0), at(c, a, 0.15, 0.15), at(c, a, 0, 0.15), at(c, a, -0.15, -0.15)]
rr = run_ok(HOUSE + [GRILLE22, probe_cmd(P)], "probes on the glass plane")
expect_hits(rr, [(P[0], 'grille'), (P[1], 'grille'), (P[2], 'glass'), (P[3], 'grille'), (P[4], 'glass')],
            "the vertical bar through the glass centre, the horizontal bar at mid-height, glass between")
if not parts_of(rr).get('grille'):
    fail(f"parts counts the grille: {parts_of(rr)}")
print(f"ok: 2 x 2 grille on the plane through {c}: bars at the centre lines, glass in the lites")

# (b) The top part only.
TOP = window4(grille={"across": 2, "down": 2, "part": "top"})
P = [at(c, a, 0, 0.05), at(c, a, 0, -0.2), at(c, a, 0.15, 0.27), at(c, a, 0.15, 0.1)]
rr = run_ok(HOUSE + [TOP, probe_cmd(P)], "a top-part grille")
expect_hits(rr, [(P[0], 'grille'), (P[1], 'glass'), (P[2], 'grille'), (P[3], 'glass')],
            "part top: the vertical bar runs over the upper half (z 1.5..2.04), its horizontal bar at 1.77, the lower half is clear")
print("ok: part top: bars in the upper half only (horizontal bar at 1.77)")

# (c) A fixed door: no swing in plan, glass at its centre.
FIXED = cmd('door', {"wall": "w4", "offset": 8.0, "width": 0.6, "height": 2.1, "style": "fixed"})
rr = run_ok(HOUSE + [FIXED, INSPECT17_SYM], "a fixed door")
dr = openings_of(inspected(rr))[0]
if dr.get('style') != 'fixed':
    fail(f"the door's style is fixed: {dr.get('style')!r}")
sym = dr.get('symbol')
if not isinstance(sym, list) or not sym or any(len(poly) != 2 for poly in sym):
    fail(f"a fixed door's plan symbol is lines only (frame and glass, no swing arc): {json.dumps(sym)[:300]}")
c, a, n = glass_frame(dr, "the fixed door")
rr = run_ok(HOUSE + [FIXED, probe_cmd([c, at(c, a, 0.0, -0.5)])], "the fixed door's glass")
expect_hits(rr, [(c, 'glass'), (at(c, a, 0.0, -0.5), 'glass')], "the fixed door is glass in a leaf frame")
print(f"ok: fixed door {dr['id']}: {len(sym)} straight symbol lines, glass at {c}")

# (d) Sills and casings per side.
BOTH = cmd('window', {"wall": "w0", "offset": 3.0, "width": 0.9, "height": 1.2, "sill": 0.9, "casing": 0.05, "casingSides": "both", "sillSides": "both"})
NONE = cmd('window', {"wall": "w0", "offset": 3.0, "width": 0.9, "height": 1.2, "sill": 0.9, "casing": 0.05, "casingSides": "none", "sillSides": "none"})
rr = run_ok(HOUSE + [BOTH, cmd('build3d', {})], "sides both")
parts = parts_of(rr)
if parts.get('casing') != 2 or parts.get('sill') != 2:
    fail(f"casingSides and sillSides both: parts.casing 2 and parts.sill 2, got {parts}")
rr = run_ok(HOUSE + [NONE, cmd('build3d', {})], "sides none")
parts = parts_of(rr)
if parts.get('casing') or parts.get('sill'):
    fail(f"casingSides and sillSides none: no casing and no sill part, got {parts}")
print("ok: sides both: 2 casings, 2 sills; sides none: neither")
print("PASS 09")
