# HDRI: a Radiance .hdr (RGBE, run-length encoded or flat) read in Rust, used as a 360-degree backdrop that lights the
# scene (`backdrop {kind: "hdri", file | item, intensity?, rotation?, horizon?}`). Brief numbers: a uniform environment of
# radiance 1.0 (an RGBE file this check writes) over an upward-facing white diffuse plane of albedo 0.5, EV100 -1, tone
# mapping off: plane pixels 0.5/pi * pi * 1/(1.2 * 2^-1) = 0.8333, +-2 % path-traced and +-5 % raster. Also intensity,
# rotation (a red/blue map seen along -Y: u = 0.25 red at rotation 0, u = 0.75 blue at 180; never along +-X, the seam),
# the horizon offset of an HDRI and of a 360-degree picture, RLE and flat files alike, and the CC-0 sample HDRI as a
# Backdrops item (placed from the library as an hdri, listed, its icon, saved in the DXF).
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-r3d-groundwork-1, p19-r3d-groundwork-2, p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-env-path, p19-env-raster, p19-hdri-1, p19-hdri-2, p19-hdri-3, p19-hdri-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
ITEM = "polyhaven:kloofendal-puresky"
UNI = write_rgbe(os.path.join(tmp, 'uniform.hdr'), 32, 16, lambda x, y: (1.0, 1.0, 1.0))
UNI_RLE = write_rgbe(os.path.join(tmp, 'uniform-rle.hdr'), 32, 16, lambda x, y: (1.0, 1.0, 1.0), rle=True)
RB = write_rgbe(os.path.join(tmp, 'redblue.hdr'), 32, 16, lambda x, y: (1.0, 0.0, 0.0) if x < 16 else (0.0, 0.0, 1.0))
HALF = write_rgbe(os.path.join(tmp, 'half.hdr'), 64, 32, lambda x, y: (1.0, 1.0, 1.0) if y < 16 else (0.01, 0.01, 0.01))
TERRAIN = cmd('terrain', {"boundary": [[0, 0], [40, 0], [40, 30], [0, 30]], "elevation": 0})
DOWN = down_cam(20, 15, 3.0, 30)
LIT = {"albedo": 0.5, "sun": {"on": False}, "toneMap": "none"}
PATH = {"final": "path", "spp": 256, "denoise": False, "seed": 0}
RASTERS = (('interactive', {}), ('raster', {"final": "raster"}))


def hdri(path, **k):
    return cmd('backdrop', dict({"kind": "hdri", "file": path}, **k))


# (a) uniform 1.0, EV100 -1 and (b) intensity 2, EV100 0: the plane reads 0.8333; (e) the RLE file renders the same.
cases = [('a', UNI, {}, -1), ('b', UNI, {"intensity": 2}, 0), ('e', UNI_RLE, {}, -1)]
cmds, keys = [TERRAIN], []
for tag, f, k, ev in cases:
    cmds.append(hdri(f, **k))
    for mode, extra in RASTERS + (('path', PATH),):
        o = os.path.join(tmp, f'{tag}-{mode}.png')
        size = (32, 24) if mode == 'path' else (64, 48)
        cmds.append(render_cmd(o, DOWN, size, exposure={"ev100": ev}, **LIT, **extra))
        keys.append((tag, mode, o))
rows, rs = rendered(cmds, "renders under a uniform HDRI")
if len(rs) != len(keys):
    fail(f"{len(keys)} render replies: {len(rs)}")
got = {}
for r, (tag, mode, o) in zip(rs, keys):
    got[(tag, mode)] = v = lum(centre_mean(o))
    tol = 0.02 if mode == 'path' else 0.05
    print(f"({tag}) {mode:11s}: plane {v:.4f} (want 0.8333 +- {tol * 100:g} %)")
    within(v, 0.8333, tol, f"({tag}) {mode}: a white plane of albedo 0.5 under the uniform HDRI")
    if mode == 'path' and not isinstance(r.get('environment'), dict):
        fail(f"({tag}) the path reply names the environment it used: {r.get('environment')}")
for mode in ('interactive', 'raster', 'path'):
    within(got[('e', mode)], got[('a', mode)], 0.005, f"(e) {mode}: the RLE file renders as the flat one")

# (c) rotation: looking along -Y (u = 0.25) the map is red at rotation 0 and blue at rotation 180.
SOUTH = level_cam([20, 15, 1.6], -90, 30, 40)
cmds, keys = [TERRAIN], []
for rot in (0, 180):
    cmds.append(hdri(RB, rotation=rot))
    for mode, extra in RASTERS + (('path', {"final": "path", "spp": 4, "bounces": 0, "seed": 0, "denoise": False}),):
        o = os.path.join(tmp, f'c-{rot}-{mode}.png')
        cmds.append(render_cmd(o, SOUTH, (60, 45), sun={"on": False}, exposure={"ev100": 0}, toneMap="none", **extra))
        keys.append((rot, mode, o))
rows, rs = rendered(cmds, "renders under the red/blue HDRI")
for r, (rot, mode, o) in zip(rs, keys):
    w, h, p = img(o)
    m = box_mean((w, h, p), w // 3, 0, 2 * w // 3, h)
    red = m[0] > 5 * m[2] and m[0] > 0.3
    blue = m[2] > 5 * m[0] and m[2] > 0.3
    print(f"(c) rotation {rot:3d} {mode:11s}: central third mean linear {[round(x, 3) for x in m]}")
    if (rot == 0 and not red) or (rot == 180 and not blue):
        fail(f"(c) {mode}: looking along -Y the map's u = 0.25 half (red) shows at rotation 0 and the blue half at "
             f"rotation 180: rotation {rot} gives {m}")

# (d) horizon offset: the row 5 degrees above the true horizon is bright at horizon 0 and dark at horizon 10 (the map
# is bright above its middle row); a 360-degree picture's horizon moves the same way.
NORTH = level_cam([20, 15, 1.6], 90, 0, 40)
H = 90
ROW5 = round(H / 2 - math.tan(math.radians(5)) / math.tan(math.radians(20)) * H / 2)
cmds, keys = [TERRAIN], []
for hz in (0, 10):
    cmds.append(hdri(HALF, horizon=hz))
    for mode, extra in RASTERS:
        o = os.path.join(tmp, f'd-{hz}-{mode}.png')
        cmds.append(render_cmd(o, NORTH, (120, H), sun={"on": False}, exposure={"ev100": 0}, toneMap="none", **extra))
        keys.append(('hdri', hz, mode, o))
for hz in (0, 10):
    cmds.append(cmd('backdrop', {"kind": "image", "item": "own:backdrop-sky", "projection": "equirect", "horizon": hz}))
    o = os.path.join(tmp, f'd-pic-{hz}.png')
    cmds.append(render_cmd(o, NORTH, (120, H)))
    keys.append(('picture', hz, 'interactive', o))
rows, rs = rendered(cmds, "horizon offsets")
row = {}
for (kind, hz, mode, o) in keys:
    w, h, p = img(o)
    row[(kind, hz, mode)] = box_mean((w, h, p), w // 3, ROW5, 2 * w // 3, ROW5 + 1, linear=(kind == 'hdri'))
for mode, _ in RASTERS:
    b0, b10 = lum(row[('hdri', 0, mode)]), lum(row[('hdri', 10, mode)])
    print(f"(d) {mode}: the row 5 degrees up reads {b0:.4f} at horizon 0 and {b10:.4f} at horizon 10")
    if not (b0 > 0.5 and b10 < 0.05):
        fail(f"(d) {mode}: horizon 10 raises the map's horizon above the row 5 degrees up (bright {b0}, then dark {b10})")
p0, p10 = row[('picture', 0, 'interactive')], row[('picture', 10, 'interactive')]
if not p10[0] - p0[0] >= 4:
    fail(f"(d) a 360-degree picture's horizon 10 shows the picture's lower (redder) sky 5 degrees up: red {p0[0]:.1f} -> {p10[0]:.1f}")

# (f) the CC-0 sample HDRI: a Backdrops item, placed from the library as an hdri, listed, its icon, saved in the DXF.
dxf = os.path.join(tmp, 'hdri.dxf')
o1, o2 = os.path.join(tmp, 'f-raster.png'), os.path.join(tmp, 'f-path.png')
rows = run_ok([TERRAIN, cmd('backdrop', {"kind": "hdri", "item": ITEM}),
               render_cmd(o1, SOUTH, (80, 60)), render_cmd(o2, SOUTH, (40, 30), final="path", spp=4, bounces=0),
               cmd('backdrop', {"kind": "colour", "colour": "#204060"}), cmd('library.place', {"item": ITEM, "at": [0, 0]}),
               'backdrop {}', 'library {}', cmd('library.icon', {"item": ITEM})], "the sample HDRI", save=dxf)
for o in (o1, o2):
    if luma_sd(o) < 2:
        fail(f"(f) the sample HDRI renders a non-uniform background: luma sd {luma_sd(o):.2f} in {os.path.basename(o)}")
bd = backdrops(rows)
if len(bd) < 4 or not all(isinstance(b, dict) and b.get('kind') == 'hdri' and b.get('image') == ITEM for b in bd[-2:]):
    fail(f"(f) library.place of the HDR backdrop item replies with an hdri backdrop of image {ITEM}, and `backdrop {{}}` "
         f"reports it: {bd[-2:]}")
items = {i.get('id'): i for i in lib_reply(rows)['items'] if isinstance(i, dict)}
it = items.get(ITEM)
if not it or it.get('category') != 'Backdrops' or it.get('licence') != 'CC0-1.0':
    fail(f"(f) the library lists {ITEM} under Backdrops with licence CC0-1.0: {it}")
data, _ = icon_png(rows)
ip = os.path.join(tmp, 'icon.png')
open(ip, 'wb').write(data)
if luma_sd(ip) < 2:
    fail("(f) the HDRI's library icon is a picture of the sky, not a flat colour")
rows = run_ok(['backdrop {}'], "reopening the DXF", file=dxf)
bd = backdrops(rows)
if not bd or not isinstance(bd[-1], dict) or bd[-1].get('kind') != 'hdri' or bd[-1].get('image') != ITEM:
    fail(f"(f) the DXF reopens with its HDRI backdrop: {bd[-1:]}")
print("PASS 04")
