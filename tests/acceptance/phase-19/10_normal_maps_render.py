# Normal maps are used by the raster views and the path tracer. A brick wall facing south (w1 of HOUSE1) lit by two
# mirrored suns (azimuth 135 and 225, elevation 25, sky off): a flat-shaded wall reads the same under both (the same
# cosine everywhere), while the brick's normal map tilts the head joints toward one sun and away from the other, so the
# two images differ pixel by pixel though their means agree. The same sun twice gives the same image.
# needs: p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-normal-maps-1, p19-normal-maps-2, p19-normal-maps-render
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
SET = HOUSE1 + [SITE, cmd('material', {"walls": "exterior", "finish": "brick"})]
CAM = {"kind": "full", "eye": [5, -6, 1.2], "target": [5, 0, 1.2], "fov": 40}
SIZE = (200, 150)
BOX = (80, 65, 120, 85)          # the central 40 x 20 px: brick wall only
LOOK = {"sky": False, "exposure": {"ev100": 15}, "toneMap": "none"}
PATHK = {"final": "path", "spp": 16, "bounces": 0, "seed": 0, "denoise": False}
cmds, keys = [], []
for mode, extra in (('raster', {}), ('path', PATHK)):
    for tag, az in (('A', 135), ('B', 225), ('A2', 135)):
        o = os.path.join(tmp, f'{mode}-{tag}.png')
        cmds.append(render_cmd(o, CAM, SIZE, sun={"azimuth": az, "elevation": 25}, **LOOK, **extra))
        keys.append((mode, tag, o))
rendered(SET + cmds, "mirrored-sun renders")
im = {(m, t): img(o) for m, t, o in keys}
for mode, need in (('raster', 1.5), ('path', 1.0)):
    a, b, a2 = im[(mode, 'A')], im[(mode, 'B')], im[(mode, 'A2')]
    x0, y0, x1, y1 = BOX
    pa = [at(a, x, y) for y in range(y0, y1) for x in range(x0, x1)]
    pb = [at(b, x, y) for y in range(y0, y1) for x in range(x0, x1)]
    d = sum(abs(p[k] - q[k]) for p, q in zip(pa, pb) for k in range(3)) / (3 * len(pa))
    ma, mb = (sum(sum(p) for p in s) / (3 * len(s)) for s in (pa, pb))
    print(f"{mode}: mean |A - B| {d:.2f} levels, means {ma:.1f} / {mb:.1f}")
    if not 20 < ma < 235:
        fail(f"{mode}: the wall patch is exposed toward mid-grey (mean {ma:.1f} of 255)")
    if abs(ma - mb) > 0.03 * ma:
        fail(f"{mode}: the mirrored suns light the wall alike on average (within 3 %): {ma:.1f} vs {mb:.1f}")
    if d < need:
        fail(f"{mode}: the brick's normal map makes the mirrored suns differ pixel by pixel (mean |A - B| >= {need}): {d:.2f}")
    if mode == 'raster' and a[2] != a2[2]:
        fail("raster: the same sun twice gives the same image")
print("PASS 10")
