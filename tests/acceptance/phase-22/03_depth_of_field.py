# Depth of field (path tracer only): a thin lens, f/2.8, 60 degrees horizontal view, focus 3 m. The lens is a 36 mm wide
# sensor's: f = 0.018 / tan(30) = 31.18 mm, aperture 11.13 mm across. A small emitter on the axis at 3 m images within
# 1.5 px; at 1.5 m and at 10 m its blur disc is A |d - s| / (d s) F px across (6.17 and 4.32 px at 1921 px wide), +-10 %,
# measured from the second moment of the linear image (PFM). Aperture off is the pinhole image exactly; focus {pick}
# resolves to the distance under the pixel; the interactive view ignores the aperture.
# needs: p22-doc-look, p22-arch3d-lens, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-lens-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (1921, 61)
W, H = SIZE
CAM = hcam([0.0, 0.0, 1.5], 90, 60, SIZE)
T30 = math.tan(math.radians(30))
F_PX = (W / 2) / T30
APERTURE = (0.018 / T30) / 2.8
CASES = [(3.0, 0.001), (1.5, 0.001), (10.0, 0.006)]
BASE = pt(spp=512, bounces=0, sun={'on': False})


def emitter(d, diam):
    return calib([obj('sphere', [0.0, d, 1.5], diam, color=[0, 0, 0], emission=1.0, segments=32)])


cmds, outs = [], []
for d, diam in CASES:
    p = os.path.join(tmp, 'dof-%g.pfm' % d)
    cmds.append(render_cmd(os.path.join(tmp, 'dof-%g.png' % d), CAM, SIZE, calibrate=emitter(d, diam), aperture=2.8, focus=3.0, pfm=p, **BASE))
    outs.append(p)
pick = os.path.join(tmp, 'pick.png')
cmds.append(render_cmd(pick, CAM, SIZE, calibrate=emitter(3.0, 0.02), aperture=2.8, focus={'pick': [960.5, 30.5]}, **pt(spp=4, bounces=0, sun={'on': False})))
rows, rs = rendered(cmds, 'the depth-of-field renders', timeout=3000)
if len(rs) != 4:
    fail('four render replies: %d' % len(rs))
for r in rs:
    is_path(r, 'depth of field')
    near(lens_of(r, 'dof').get('aperture'), 2.8, 1e-9, 'look.lens.aperture')
near(lens_of(rs[3], 'focus pick').get('focus'), 2.99, 0.02, 'focus {pick} resolves to the distance of what is under the pixel (the emitter at 3 m)')
for (d, diam), p in zip(CASES, outs):
    w, h, vals = pfm_lumas(p)
    s, cx, cy, xx, yy, xy = moments(w, h, vals)
    if abs(cx - 960.5) > 0.6 or abs(cy - 30.5) > 0.6:
        fail('the emitter at %g m images at (%.2f, %.2f), not on the axis (960.5, 30.5)' % (d, cx, cy))
    rs_px = (diam / 2) / d * F_PX
    if d == 3.0:
        inside = sum(vals[y * w + x] for y in range(h) for x in range(950, 972) if math.hypot(x - 960, y - 30) <= 1.5)
        print('in focus at 3 m: %.1f %% of the light within 1.5 px' % (100 * inside / s))
        if inside < 0.95 * s:
            fail('the emitter at the focus distance images within 1.5 px: only %.1f %% of its light does' % (100 * inside / s))
        continue
    want = APERTURE * abs(d - 3.0) / (d * 3.0) * F_PX
    r2 = 2 * (xx + yy - rs_px * rs_px / 2 - 1.0 / 6)
    got = 2 * math.sqrt(max(r2, 0.0))
    print('emitter at %g m: blur disc %.2f px across (thin lens %.2f)' % (d, got, want))
    within(got, want, 0.10, 'blur disc diameter at %g m' % d)
    if abs(xx - yy) > 0.25 * (xx + yy):
        fail('the blur at %g m is not round (variances %.2f and %.2f)' % (d, xx, yy))

# aperture off = the pinhole image, pixel for pixel; the interactive view has no depth of field.
S2 = (160, 120)
C2 = {'kind': 'full', 'eye': [2.0, 0.4, 1.4], 'target': [2.0, 3.0, 0.8], 'fov': 60}
q = pt(spp=4, bounces=1, sun={'on': False})
files = [os.path.join(tmp, n) for n in ('pin.png', 'off.png', 'open.png', 'i0.png', 'i1.png')]
rows, rs = rendered(ROOM + [COLUMN, cmd('lightfixture', {'room': 'r0', 'rows': 1, 'cols': 1}),
                            render_cmd(files[0], C2, S2, **q), render_cmd(files[1], C2, S2, aperture=None, focus=None, **q),
                            render_cmd(files[2], C2, S2, aperture=1.4, focus=0.6, **q),
                            render_cmd(files[3], C2, S2), render_cmd(files[4], C2, S2, aperture=1.4, focus=0.6)], 'pinhole identity')
if digest(files[0]) != digest(files[1]):
    fail('aperture off gives the pinhole image exactly (same seed, identical pixels)')
if digest(files[0]) == digest(files[2]):
    fail('f/1.4 focused at 0.6 m changes nothing in the room: the thin lens is not traced')
if digest(files[3]) != digest(files[4]):
    fail('the interactive view has no depth of field: its image changed with the aperture')
print('PASS 03')
