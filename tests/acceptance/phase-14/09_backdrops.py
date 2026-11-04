#!/usr/bin/env python3
# Backdrops behind the 3D view, the raster Finalize and the path tracer: a solid colour, a two-colour gradient (top to
# bottom of the view), or an image, flat (stretched over the view) or 360° equirectangular (by view direction: the top row
# is the zenith, the middle row the horizon). Shown where no surface is hit, after tone mapping; saved with the drawing.
# needs: p14-doc-furnishing, p14-cmd-backdrop, p14-r3d-backdrop, p14-r3d-pt-backdrop, p14-arch3d-furnishing, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
W, H, FOV = 320, 240, 40.0
LEVEL = {"kind": "full", "eye": [2.0, -10.0, 1.2], "target": [2.0, 2.0, 1.2], "fov": FOV}     # horizontal: walls below the middle
UP = {"kind": "full", "eye": [2.0, -10.0, 1.2], "target": [2.0, -9.9, 30.0], "fov": FOV}       # straight up: only sky


def still(cmds, cam, name, final=None):
    out = os.path.join(tmp, name)
    p = {"out": out, "size": [W, H], "style": "shaded", "ground": False, "camera": cam}
    if final:
        p["final"] = final
        if final == 'path':
            p["spp"] = 4
    rc, rr, so, se = run(cmds + [cmd('render', p)])
    if not os.path.exists(out):
        fail(f"render wrote no {name}: {se[-400:]}")
    w, h, nch, rows = png(out)
    return lambda x, y: tuple(rows[y][x * nch:x * nch + 3]), out


def close(got, want, tol, what):
    if any(abs(a - b) > tol for a, b in zip(got, want)):
        fail(f"{what}: pixel {got}, expected {tuple(round(v) for v in want)} ± {tol}")


COL = cmd('backdrop', {"kind": "colour", "colour": "#204060"})
for final in (None, 'raster', 'path'):
    px, out = still(ROOM + [COL], LEVEL, f'colour-{final}.png', final)
    close(px(0, 0), (0x20, 0x40, 0x60), 2, f"colour backdrop, top-left ({final or 'view'})")
    close(px(W - 1, 0), (0x20, 0x40, 0x60), 2, f"colour backdrop, top-right ({final or 'view'})")
artifact(out, 'backdrop-colour-path.png')
print("ok: a colour backdrop in the 3D view, the raster Finalize and the path tracer")

top, bottom = (0x30, 0x50, 0xa0), (0xe0, 0xd0, 0xb0)
px, out = still(ROOM + [cmd('backdrop', {"kind": "gradient", "top": "#3050a0", "bottom": "#e0d0b0"})], UP, 'gradient.png')
for y in (0, H // 4, H // 2, H - 1):
    t = y / (H - 1)
    close(px(5, y), [a + (b - a) * t for a, b in zip(top, bottom)], 4, f"gradient row {y}")
print("ok: a gradient runs from the top colour to the bottom colour of the view")

sky_top, sky_hor = (74, 128, 196), (214, 228, 240)
IMG = 'backdrop {"kind": "image", "item": "own:backdrop-sky", "projection": "%s"}'
px, out = still(ROOM + [IMG % 'flat'], LEVEL, 'flat.png')
close(px(W // 2, 0), sky_top, 6, "flat image: the view's top row is the image's top row")
px, out = still(ROOM + [IMG % 'equirect'], LEVEL, 'equirect-level.png')
t = 0.5 - (FOV / 2) / 180.0
close(px(W // 2, 0), [a + (b - a) * t for a, b in zip(sky_top, sky_hor)], 8,
      f"equirect, looking level: the top-centre pixel is {FOV / 2}° above the horizon")
artifact(out, 'backdrop-equirect.png')
px, out = still(ROOM + [IMG % 'equirect'], UP, 'equirect-up.png')
close(px(W // 2, H // 2), sky_top, 8, "equirect, looking up: the zenith is the image's top row")
print("ok: an image backdrop, flat and equirectangular")

dxf = os.path.join(tmp, 'b.dxf')
run(ROOM + [COL], save=dxf)
rc, rr, so, se = run([cmd('backdrop', {})], file=dxf)
b = reply_with(rr, 'backdrop', -1, 'backdrop {}')['backdrop']
if not isinstance(b, dict) or b.get('kind') != 'colour' or b.get('colour') != '#204060':
    fail(f"the backdrop after a DXF round trip: {b}")
rc, rr, so, se = run(ROOM + [COL, cmd('backdrop', {"kind": "none"}), cmd('backdrop', {})])
if reply_with(rr, 'backdrop', -1)['backdrop'] is not None:
    fail("backdrop none does not clear it")
print("ok: the backdrop is saved with the drawing; none clears it")
print("PASS 09")
