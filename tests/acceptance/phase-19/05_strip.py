# Strip lights: LightFixture kind strip along a 2D path at a height, under a cabinet run, or as a ring along a room's
# walls, with lumens per metre and a colour temperature, emitting downward (Lambertian) or outward. Brief number: a 2 m
# strip of 500 lm/m facing down at 2.4 m in a dark room gives `illuminance` 49.72 lx +- 2 % on the floor under its
# centre: E = (500/pi) (a/(h^2+a^2) + atan(a/h)/h) with a = 1, h = 2.4 (a point light at the midpoint would give 55.3,
# an emitter that ignores its facing would light the floor under an outward strip). Also its inspect row, the plan
# proxy in the DXF and its node in the GLB. Round a tray: check 17.
# needs: p19-strip-model-1, p19-strip-model-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
WANT = 500 / math.pi * (1 / (2.4 ** 2 + 1) + math.atan(1 / 2.4) / 2.4)
STRIP = {"type": "LED strip", "path": [[1, 2], [3, 2]], "height": 2.4, "lumensPerMetre": 500, "facing": "down"}
INSPECT = cmd('arch.inspect', {"lights": True, "cabinets": True})


def strips(rows):
    ls = lights_of(rows)
    return [l for l in ls if l.get('kind') == 'strip']


def length_of(path, closed):
    pts = path + ([path[0]] if closed and path and path[0] != path[-1] else [])
    return sum(math.dist(a, b) for a, b in zip(pts, pts[1:]))


# (a) the brief's number, half the lumens, facing out.
rows = run_ok(ROOM + [cmd('lightfixture', STRIP), cmd('illuminance', {"point": [2, 2, 0]}), INSPECT,
                      cmd('lightfixture.edit', {"id": "lf0", "lumensPerMetre": 250}), cmd('illuminance', {"point": [2, 2, 0]}),
                      cmd('lightfixture.edit', {"id": "lf0", "lumensPerMetre": 500, "facing": "out"}),
                      cmd('illuminance', {"point": [2, 2, 0]})], "a strip light")
lx = [num(r['lux'], 'lux') for r in rows if isinstance(r, dict) and 'lux' in r and 'direct' in r]
if len(lx) != 3:
    fail(f"three illuminance replies: {lx}")
print(f"strip 2 m x 500 lm/m at 2.4 m: {lx[0]:.3f} lx (want {WANT:.2f}); 250 lm/m: {lx[1]:.3f}; facing out: {lx[2]:.3f}")
within(lx[0], WANT, 0.02, "illuminance under the centre of a 2 m, 500 lm/m strip facing down at 2.4 m")
within(lx[1], WANT / 2, 0.02, "the same strip at 250 lm/m")
if lx[2] >= 1.0:
    fail(f"a strip facing out lights the floor under it with less than 1 lx: {lx[2]}")
s = strips(rows)
if len(s) != 1:
    fail(f"arch.inspect lists one light of kind strip: {lights_of(rows)}")
s = s[0]
for k, v in (("closed", False), ("facing", "down")):
    if s.get(k) != v:
        fail(f"the strip's inspect row has {k} {v!r}: {s}")
near(num(s.get('length'), 'length'), 2.0, 1e-9, "the strip's length")
near(num(s.get('lumens'), 'lumens'), 1000.0, 1e-6, "the strip's lumens (500 lm/m x 2 m)")
near(num(s.get('lumensPerMetre'), 'lumensPerMetre'), 500.0, 1e-9, "the strip's lumens per metre")
near(num(s.get('height'), 'height'), 2.4, 1e-9, "the strip's height")
if [[round(num(c, 'path'), 9) for c in p] for p in s.get('path') or []] != [[1, 2], [3, 2]]:
    fail(f"the strip's path is [[1, 2], [3, 2]]: {s.get('path')}")

# (b) a ring along the room's walls, and under a run of two wall cabinets.
rows = run_ok(ROOM + [cmd('lightfixture', {"type": "LED strip", "room": "r0", "inset": 0.5}),
                      cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.9]}), INSPECT],
              "a room ring and a cabinet run")
insp = inspected(rows)
cabs = [c for c in (insp.get('cabinets') or []) if isinstance(c, dict) and c.get('kind') == 'wall']
if len(cabs) != 2:
    fail(f"two wall cabinets: {insp.get('cabinets')}")
rows2 = run_ok(ROOM + [cmd('lightfixture', {"type": "LED strip", "room": "r0", "inset": 0.5}),
                       cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.9]}),
                       cmd('lightfixture', {"type": "LED strip", "under": [c['id'] for c in cabs]}), INSPECT],
               "a strip under the cabinet run")
s = strips(rows2)
if len(s) != 2:
    fail(f"two strips: {lights_of(rows2)}")
ring, under = s
if ring.get('closed') is not True or ring.get('facing') != 'out':
    fail(f"the room ring is closed and faces out: {ring}")
near(num(ring.get('length'), 'ring length'), 12.0, 1e-6, "the ring 0.5 m inside the 4 x 4 room")
near(length_of(ring.get('path') or [], True), 12.0, 1e-6, "the ring's path")
widths = sum(num(c.get('width'), 'width') for c in cabs)
near(num(under.get('length'), 'run length'), widths, 1e-6, "the strip under the cabinets runs their whole width")
elev = num(cabs[0].get('elevation'), 'cabinet elevation')
near(num(under.get('height'), 'run height'), elev - 0.005, 1e-6, "the strip sits 5 mm under the wall cabinets")
if under.get('facing') != 'down':
    fail(f"the under-cabinet strip faces down: {under}")
print(f"ok: ring {ring['length']} m (out), under-cabinet strip {under['length']} m at {under['height']}")

# (c) exports: the plan proxy in the DXF (LWPOLYLINE on E-LITE through (1, 2) and (3, 2)) and the GLB node.
d, g = os.path.join(tmp, 'strip.dxf'), os.path.join(tmp, 'strip.glb')
export(ROOM + [cmd('lightfixture', STRIP)], d)
found = False
for kind, layer, pairs in entities(dxf_text(d)):
    if kind != 'LWPOLYLINE' or layer != 'E-LITE':
        continue
    xs = [float(v) for c, v in pairs if c == '10']
    ys = [float(v) for c, v in pairs if c == '20']
    pts = list(zip(xs, ys))
    if any(math.dist(p, (1, 2)) < 1e-6 for p in pts) and any(math.dist(p, (3, 2)) < 1e-6 for p in pts):
        found = True
if not found:
    fail("the DXF plan has the strip's path (1, 2)-(3, 2) as an LWPOLYLINE on E-LITE")
export(ROOM + [cmd('lightfixture', STRIP)], g)
j, binc = glb(g)
if 'light-lf0' not in node_names(j):
    fail(f"the GLB has node light-lf0 for the strip: {sorted(node_names(j))[:20]}")
print("PASS 05")
