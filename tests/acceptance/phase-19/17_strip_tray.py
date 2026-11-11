# Strip lights round a tray: `lightfixture {type: "LED strip", tray}` runs a closed strip along the outline of a
# phase-16 tray ceiling (the recess's edge), `inset` (default 0.05) inside it, facing out at the recess's sides, at the
# finished ceiling just outside the outline + 0.02. NW room of the phase-1 house (0.1..5.95 x 3.05..7.9) under the gable,
# tray inset 0.6: outline x 0.7..5.35, y 3.65..7.3; the strip ring x 0.75..5.30, y 3.70..7.25: 2 (4.55 + 3.55) = 16.2 m,
# 8100 lm at 500 lm/m; inset 0.3: 2 (4.05 + 3.05) = 14.2 m. (The room form {room, inset 0.05} would give another ring.)
# needs: p19-strip-model-1, p19-strip-model-2, p19-strip-tray, p16-trays
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
NW = house_rooms()['NW']
SET = HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3})]
rows = run_ok(SET + [cmd('arch.inspect', {"trays": True})], "the tray")
tr = [t for t in (inspected(rows).get('trays') or []) if isinstance(t, dict)]
if len(tr) != 1:
    fail(f"one tray: {tr}")
tid = tr[0]['id']
rows = run_ok(SET + [cmd('lightfixture', {"type": "LED strip", "tray": tid}),
                     cmd('lightfixture', {"type": "LED strip", "tray": tid, "inset": 0.3}),
                     cmd('arch.inspect', {"lights": True}), cmd('build3d', {"ceiling": [[0.65, 5.475]]})], "strips round the tray")
s = [l for l in lights_of(rows) if l.get('kind') == 'strip']
if len(s) != 2:
    fail(f"two strips: {lights_of(rows)}")
a, b = s
for k, v in (("closed", True), ("facing", "out"), ("tray", tid)):
    if a.get(k) != v:
        fail(f"the tray strip's row has {k} {v!r}: {a}")
pts = [(num(p[0], 'x'), num(p[1], 'y')) for p in a.get('path') or []]
if pts and math.dist(pts[0], pts[-1]) < 1e-9:
    pts = pts[:-1]
want = {(0.75, 3.70), (5.30, 3.70), (5.30, 7.25), (0.75, 7.25)}
if len(pts) != 4 or not all(any(math.dist(p, w) < 1e-6 for p in pts) for w in want):
    fail(f"the strip runs 0.05 inside the tray outline: ring x 0.75..5.30, y 3.70..7.25: {pts}")
area2 = sum(pts[i][0] * pts[(i + 1) % 4][1] - pts[(i + 1) % 4][0] * pts[i][1] for i in range(4))
if area2 <= 0:
    fail(f"the ring runs counter-clockwise (so `out` faces the recess's sides): {pts}")
near(num(a.get('length'), 'length'), 16.2, 1e-6, "the strip's length round the tray")
near(num(a.get('lumens'), 'lumens'), 8100.0, 1e-6, "the strip's lumens (500 lm/m)")
near(num(b.get('length'), 'length'), 14.2, 1e-6, "the strip 0.3 inside the outline")
ceil = ceilings(rows)[0]
near(num(a.get('height'), 'height'), num(ceil, 'ceiling') + 0.02, 1e-6, "the strip sits 2 cm above the finished ceiling round the recess")
rc, rr, so, se = run(SET + [cmd('lightfixture', {"type": "LED strip", "tray": "tr9"}),
                            cmd('lightfixture', {"type": "LED strip", "tray": tid, "inset": 2.0}),
                            cmd('arch.inspect', {"lights": True})], expect_ok=False)
if len(errors(se)) < 2 or [l for l in lights_of(rr) if l.get('kind') == 'strip']:
    fail(f"an unknown tray and an inset that leaves no ring are refused with error lines, adding nothing: {errors(se)}")
g = os.path.join(tmp, 'tray.glb')
export(SET + [cmd('lightfixture', {"type": "LED strip", "tray": tid})], g)
j, binc = glb(g)
lf = [n for n in node_names(j) if n.startswith('light-lf')]
if not lf:
    fail(f"the GLB has a light-lf<n> node for the tray strip: {sorted(node_names(j))[:30]}")
print("PASS 17")
