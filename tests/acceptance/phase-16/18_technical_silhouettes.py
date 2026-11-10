#!/usr/bin/env python3
# Technical illustration draws silhouettes for the camera's view direction (the phase-15 checkpoint, decision 2). The
# sample set's desk lamp (lamp-desk) has a smooth round stem: no crease runs along its sides, so only a silhouette
# outlines it. Rendered `technical` on a white backdrop from the front (eye (0,-0.7,0.2)) and from the side
# (eye (0.7,0,0.2)), both looking level at (0,0,0.2), the stem's left and right edges carry the dark line colour on most
# of its rows, and the middle of the stem stays its flat fill (a smooth surface draws no lines inside its outline).
# A stub fails: no silhouettes, or silhouettes for a fixed top-down direction (today's placeholder), leave the stem's
# sides unlined; drawing every smooth edge lines the inside of the stem too; a blank image has no stem.
# needs: p16-rw-technical-silhouettes
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = 400
VIEWS = [("front", [0, -0.7, 0.2]), ("side", [0.7, 0, 0.2])]
cmds = [cmd('library.place', {"item": "lamp-desk", "at": [0, 0]}), cmd('backdrop', {"kind": "colour", "colour": "#ffffff"})]
for name, eye in VIEWS:
    cmds += [cmd('camera', {"save": name, "kind": "full", "eye": eye, "target": [0, 0, 0.2], "fov": 40}),
             cmd('render', {"out": os.path.join(tmp, f'{name}.png'), "size": [SIZE, SIZE], "style": "technical", "camera": name})]
os.environ.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
os.environ.setdefault('CADCRAFT_LIBRARY_USER_DIR', tempfile.mkdtemp())
run_ok(cmds, "the desk lamp rendered technical from the front and from the side")


def lum(p):
    return 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]


for name, _ in VIEWS:
    path = os.path.join(tmp, f'{name}.png')
    if not os.path.exists(path):
        fail(f"{name}: no image written")
    artifact(path, f'technical-lamp-{name}.png')
    w, h, nch, rows = png(path)
    px = lambda x, y: tuple(rows[y][x * nch:x * nch + 3])
    brass = lambda p: p[0] > 140 and p[1] > 130 and p[0] - p[2] > 60      # the lamp's flat brass fill
    fills, stem_rows = [], []
    for y in range(int(h * 0.45), int(h * 0.75)):
        xs = [x for x in range(w // 2 - 60, w // 2 + 60) if brass(px(x, y))]
        if not xs:
            continue
        # the run of fill through the image's middle column (the stem), with its ends
        mid = min(xs, key=lambda x: abs(x - w // 2))
        xl = xr = mid
        while xl - 1 >= 0 and brass(px(xl - 1, y)):
            xl -= 1
        while xr + 1 < w and brass(px(xr + 1, y)):
            xr += 1
        if 8 <= xr - xl <= 60:
            stem_rows.append((y, xl, xr))
            fills += [lum(px(x, y)) for x in range(xl, xr + 1)]
    if len(stem_rows) < 40:
        fail(f"{name}: the lamp's stem is not drawn ({len(stem_rows)} rows of a 8..60 px brass run through the middle)")
    fill = sorted(fills)[len(fills) // 2]
    dark = lambda p: lum(p) < min(110.0, fill - 50.0)
    lined = inside = 0
    for y, xl, xr in stem_rows:
        left = any(dark(px(x, y)) for x in range(max(0, xl - 3), xl + 3))
        right = any(dark(px(x, y)) for x in range(xr - 2, min(w, xr + 4)))
        lined += left and right
        third = (xr - xl) // 3
        inside += any(dark(px(x, y)) for x in range(xl + third, xr - third + 1))
    n = len(stem_rows)
    if lined < 0.7 * n:
        fail(f"{name}: the stem's sides are drawn as silhouette lines on at least 70 % of its rows: {lined} of {n} rows "
             f"(fill luma {fill:.0f})")
    if inside > 0.3 * n:
        fail(f"{name}: the middle of the smooth stem draws no lines (only its outline): {inside} of {n} rows have a dark "
             f"pixel in the stem's middle third")
    print(f"ok: {name}: the stem is outlined on {lined} of {n} rows and clean inside on {n - inside}")
print("PASS 18")
