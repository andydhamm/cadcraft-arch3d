# Lens shift: shift 0.25 moves the horizon 25 % of the frame height (down the image, so a level camera sees more of what
# is above) and keeps vertical edges vertical (+-0.5 px), in the interactive view, the raster Finalize and the path
# tracer, alone and with Panini. A horizontal bar at eye height and two poles, from a level camera at 1.5 m.
# needs: p22-doc-look, p22-arch3d-lens, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-lens-path, p22-r3d-lens-raster
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (600, 400)
W, H = SIZE
CAM = hcam([0.0, 0.0, 1.5], 90, 70, SIZE)
c = math.radians(25.0)
WHITE = dict(color=[1, 1, 1], roughness=0.9)
OBJS = [obj('box', [0.0, 10.0, 1.5], [40.0, 0.05, 0.05], **WHITE),
        obj('box', [-8 * math.sin(c), 8 * math.cos(c), 1.5], [0.08, 0.08, 12.0], **WHITE),
        obj('box', [8 * math.sin(c), 8 * math.cos(c), 1.5], [0.08, 0.08, 12.0], **WHITE)]
CASES = [('s0', {}, 200.0), ('s25', {'shift': 0.25}, 300.0), ('pan25', {'projection': 'panini', 'shift': 0.25}, 300.0)]
cmds, keys = [], []
for mode, extra in MODES:
    for tag, lens, row in CASES:
        o = os.path.join(tmp, mode + '-' + tag + '.png')
        k = dict(calibrate=calib(OBJS), sun={'azimuth': 180, 'elevation': 10}, sky=False, toneMap='none', samples=[[0.0, 10.0, 1.5]])
        k.update(lens)
        k.update(extra)
        cmds.append(render_cmd(o, CAM, SIZE, **k))
        keys.append((mode, tag, row, o))
rows, rs = rendered(cmds, 'the shifted renders', timeout=3000)
if len(rs) != len(keys):
    fail('%d render replies, not %d' % (len(rs), len(keys)))
poles = {}
for r, (mode, tag, want_row, o) in zip(rs, keys):
    what = mode + ' ' + tag
    near(lens_of(r, what).get('shift'), 0.0 if tag == 's0' else 0.25, 1e-9, what + ': look.lens.shift')
    w, h, vals = png_lumas(o)
    m = mask(vals, vals[5 * w + w // 2 + 60])
    ys = [y + 0.5 for y in range(h) for x in range(w // 2 - 20, w // 2 + 20) if m[y * w + x] > 0.5]
    if len(ys) < 40:
        fail(what + ': the horizon bar is not drawn at the middle columns')
    bar = sum(ys) / len(ys)
    tol = 1.0 if tag == 's0' else 1.5
    if abs(bar - want_row) > tol:
        fail(what + ': the horizon is at row %.2f, not %.1f +- %.1f (25 %% of the %d px height)' % (bar, want_row, tol, h))
    s = (r.get('samples') or [{}])[0].get('pixel')
    if not (isinstance(s, list) and abs(float(s[1]) - bar) <= 1.5):
        fail(what + ': the reply places the bar point at %r, the image shows it at row %.2f' % (s, bar))
    for side, x0, x1 in (('left', 0, w // 2 - 30), ('right', w // 2 + 30, w)):
        cols = []
        for y in range(10, h - 10):
            if abs(y + 0.5 - bar) < 8:
                continue
            xs = [x + 0.5 for x in range(x0, x1) if m[y * w + x] > 0.5]
            if len(xs) < 2:
                fail(what + ': the %s pole is missing from row %d' % (side, y))
            cols.append(sum(xs) / len(xs))
        spread = max(cols) - min(cols)
        if spread > 1.0:
            fail(what + ': the %s vertical edge wanders %.2f px' % (side, spread))
        poles[(mode, tag, side)] = sum(cols) / len(cols)
    print('%-20s horizon row %.2f, poles at x %.1f and %.1f' % (what, bar, poles[(mode, tag, 'left')], poles[(mode, tag, 'right')]))
for mode, _ in MODES:
    for side in ('left', 'right'):
        if abs(poles[(mode, 's25', side)] - poles[(mode, 's0', side)]) > 0.75:
            fail(mode + ': the shift moved the %s pole sideways (%.2f -> %.2f)' % (side, poles[(mode, 's0', side)], poles[(mode, 's25', side)]))
print('PASS 02')
