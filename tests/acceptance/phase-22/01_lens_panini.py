# Rung 1, the wide-angle projection, in the interactive view, the raster Finalize and the path tracer. Brief numbers: at
# 100 degrees horizontal field of view a sphere in the frame's left quarter (centred 47.5 degrees off the axis: 1 m
# across, 20 m away) projects to an ellipse of aspect cos(a) / sqrt(cos^2(a) - sin^2(47.5)) = 1.48 +- 3 % rectilinear and
# under 1.15 with Panini compression 1.0; a vertical edge stays within 0.5 px of a vertical line at any compression;
# pick3d at a warped pixel returns the object drawn there. Compression 0 is the rectilinear picture; default 0.7.
# needs: p22-doc-look, p22-arch3d-lens, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-lens-path, p22-r3d-lens-raster, p22-engine-pick-lens
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (1200, 300)
W, H = SIZE
CAM = hcam([0.0, 0.0, 1.5], 90, 100, SIZE)
a, b = math.radians(47.5), math.radians(35.0)
SPH = obj('sphere', [-20 * math.sin(a), 20 * math.cos(a), 1.5], 1.0, color=[1, 1, 1], roughness=0.9)
POLE = obj('box', [6 * math.sin(b), 6 * math.cos(b), 1.5], [0.06, 0.06, 8.0], color=[1, 1, 1], roughness=0.9)
PROJ = [('rect', {}), ('p0', {'projection': 'panini', 'compression': 0.0}), ('p03', {'projection': 'panini', 'compression': 0.3}),
        ('p07', {'projection': 'panini'}), ('p10', {'projection': 'panini', 'compression': 1.0})]
cmds, keys = [], []
for mode, extra in MODES:
    for tag, proj in PROJ:
        o = os.path.join(tmp, mode + '-' + tag + '.png')
        k = dict(calibrate=calib([SPH, POLE]), sun={'azimuth': 132.5, 'elevation': 5}, sky=False, toneMap='none')
        k.update(proj)
        k.update(extra)
        cmds.append(render_cmd(o, CAM, SIZE, **k))
        keys.append((mode, tag, o))
rows, rs = rendered(cmds, 'the lens renders', timeout=3000)
if len(rs) != len(keys):
    fail('%d render replies, not %d' % (len(rs), len(keys)))
asp = {}
for r, (mode, tag, o) in zip(rs, keys):
    what = mode + ' ' + tag
    l = lens_of(r, what)
    want = 'rectilinear' if tag == 'rect' else 'panini'
    if l.get('projection') != want:
        fail(what + ': look.lens.projection echoes %r, not %r' % (l.get('projection'), want))
    if tag == 'p07':
        near(l.get('compression'), 0.7, 1e-9, what + ': the default compression')
    w, h, vals = png_lumas(o)
    if (w, h) != SIZE:
        fail(what + ': the image is %dx%d' % (w, h))
    m = mask(vals, vals[5 * w + w // 2])
    s, cx, cy, xx, yy, xy = moments(w, h, m, 0, w // 2)
    if yy <= 0 or s < 300:
        fail(what + ': the sphere covers %d pixels' % s)
    asp[(mode, tag)] = math.sqrt(xx / yy)
    if not cx < w / 4:
        fail(what + ': the sphere is centred at x = %.1f, not in the left quarter of the frame' % cx)
    cols = []
    for y in range(15, h - 15):
        xs = [x + 0.5 for x in range(w // 2, w) if m[y * w + x] > 0.5]
        if len(xs) < 2:
            fail(what + ': the vertical pole is missing from row %d' % y)
        cols.append(sum(xs) / len(xs))
    spread = max(cols) - min(cols)
    print('%-22s sphere aspect %.3f at x %.1f, pole at x %.1f spread %.2f px' % (what, asp[(mode, tag)], cx, cols[0], spread))
    if spread > 1.0:
        fail(what + ': the vertical edge wanders %.2f px (more than +-0.5 px from a vertical line)' % spread)
for mode, _ in MODES:
    A = {t: asp[(mode, t)] for t, _ in PROJ}
    within(A['rect'], 1.48, 0.03, mode + ': rectilinear sphere aspect')
    within(A['p0'], A['rect'], 0.02, mode + ': Panini at compression 0 is the rectilinear picture')
    if not 0.85 < A['p10'] < 1.15:
        fail(mode + ': the sphere aspect with compression 1.0 is %.3f, not under 1.15' % A['p10'])
    if not A['p07'] < A['rect'] - 0.1:
        fail(mode + ': compression 0.7 leaves the aspect at %.3f (rectilinear %.3f)' % (A['p07'], A['rect']))
    if not (A['rect'] + 0.02 >= A['p03'] >= A['p07'] - 0.02 and A['p07'] + 0.02 >= A['p10']):
        fail(mode + ': the aspect does not fall with the compression: %r' % A)
print('ok: sphere aspect and verticals in the interactive view, the raster Finalize and the path tracer')

# pick3d through the same function. The phase-1 house from the south with a window on the south wall at x = 1.
HS = (1200, 400)
HCAM = hcam([5.0, -6.0, 1.5], 90, 100, HS)
P = [1.0, -0.1, 1.5]
WIN = cmd('window', {'wall': 'w0', 'offset': 1.0, 'width': 0.9, 'sill': 0.9, 'height': 1.2})
rows, rs = rendered(HOUSE + [WIN, render_cmd(os.path.join(tmp, 'h0.png'), HCAM, HS, samples=[P]),
                             render_cmd(os.path.join(tmp, 'h1.png'), HCAM, HS, samples=[P], projection='panini', compression=1.0)],
                    'the house renders')
px = []
for r in rs:
    s = r.get('samples') or [{}]
    if not (isinstance(s[0].get('pixel'), list) and len(s[0]['pixel']) == 2):
        fail('the render places the sample point on a pixel: %r' % r.get('samples'))
    px.append([float(s[0]['pixel'][0]), float(s[0]['pixel'][1])])
if abs(px[1][0] - px[0][0]) < 20:
    fail('the sample pixel under Panini (%r) is where the rectilinear one is (%r)' % (px[1], px[0]))
rows = run_ok(HOUSE + [WIN,
                       cmd('pick3d', {'camera': HCAM, 'size': list(HS), 'pixel': px[1], 'projection': 'panini', 'compression': 1.0}),
                       cmd('pick3d', {'camera': HCAM, 'size': list(HS), 'pixel': px[1]}),
                       cmd('pick3d', {'camera': HCAM, 'size': list(HS), 'pixel': px[0]})], 'the picks')
hits = [r['hit'] for r in rows if isinstance(r, dict) and 'hit' in r]
if len(hits) != 3:
    fail('three pick replies: %r' % hits)
isop = lambda h: isinstance(h, dict) and str(h.get('id', '')).startswith('o')
if not isop(hits[0]):
    fail('pick3d at the warped pixel with the Panini lens returns the window drawn there: %r' % hits[0])
if isop(hits[1]):
    fail('the same pixel without the lens is not the window (the pick ignores the projection): %r' % hits[1])
if not isop(hits[2]):
    fail('pick3d at the rectilinear pixel returns the window: %r' % hits[2])
print('PASS 01')
