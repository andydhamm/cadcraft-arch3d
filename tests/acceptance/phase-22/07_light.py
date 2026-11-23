# Rung 4, light. (a) The sun is a disc: with disc 0.53 the penumbra of a 3 m pole's shadow at its tip is
# 3 m / sin(elev) tan(0.53 deg) wide, +-15 %. Pole 0.3 x 0.3 x 3 m at the origin, sun due south at elevation 30: the
# shadow's east edge is x = 0.15 and at y = 3 / tan(30) = 5.196 its occluder is the top edge, 6 m away along the ray, so
# the full width is 0.0555 m; a uniform disc crossing a straight edge goes from 10 % to 90 % over 0.687 of that. Measured
# with path-traced illuminance probes on the ground. With disc 0 the edge is sharp. (b) Clamp: a small bright emitter
# seen in a glossy floor is clamped per sample and the reply's clampLoss is the fraction of the frame's light removed
# (the same seed traces the same paths, so it equals 1 - clamped / unclamped).
# needs: p22-doc-look, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-sun-disc, p22-r3d-clamp
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
POLE = cmd('wall', {'points': [[-0.1, -0.1], [0.1, -0.1], [0.1, 0.1], [-0.1, 0.1], [-0.1, -0.1]], 'thickness': 0.1, 'height': 3.0, 'exterior': True})
Y = 3.0 / math.tan(math.radians(30))
XS = [0.15 - 0.06 + 0.003 * i for i in range(41)]


def profile(disc):
    cmds = [POLE] + [cmd('illuminance', {'point': [x, Y, 0.0], 'normal': [0, 0, 1], 'method': 'path', 'spp': 2048, 'bounces': 0, 'seed': 3,
                                         'sun': {'azimuth': 180, 'elevation': 30, 'disc': disc}, 'sky': False}) for x in XS]
    rows = run_ok(cmds, 'illuminance across the shadow edge (disc %g)' % disc, timeout=3000)
    lux = [num(r['lux'], 'lux') for r in rows if isinstance(r, dict) and 'lux' in r and 'method' in r]
    if len(lux) != len(XS):
        fail('%d illuminance replies, not %d' % (len(lux), len(XS)))
    return [v / 50000.0 for v in lux]


def crossing(f, level):
    for i in range(len(f) - 1):
        if f[i] <= level < f[i + 1]:
            return XS[i] + (level - f[i]) / (f[i + 1] - f[i]) * 0.003
    fail('the lit fraction never crosses %.0f %% across the shadow edge: %r' % (100 * level, [round(v, 3) for v in f]))


f = profile(0.53)
if f[0] > 0.02 or f[-1] < 0.98:
    fail('the probes run from the umbra (0) to full sun (1 = 50 000 lx): %.3f .. %.3f' % (f[0], f[-1]))
width = (crossing(f, 0.9) - crossing(f, 0.1)) / 0.687
want = 6.0 * math.tan(math.radians(0.53))
print('penumbra at the tip of the 3 m pole: %.4f m (3 / sin(30) tan(0.53) = %.4f)' % (width, want))
within(width, want, 0.15, 'penumbra width at the pole tip')
f0 = profile(0.0)
partial = [v for v in f0 if 0.02 < v < 0.98]
if len(partial) > 1:
    fail('with disc 0 the shadow edge is sharp: %d of 41 probes 3 mm apart are partly lit' % len(partial))
print('ok: (a) sun disc penumbra')

# (b) clamp.
CAM = {'kind': 'full', 'eye': [0.0, -3.0, 1.0], 'target': [0.0, -1.5, 0.0], 'fov': 40}
SCN = calib([obj('box', [0.0, 0.0, -0.05], [6.0, 6.0, 0.1], color=[0.9, 0.9, 0.9], metallic=1.0, roughness=0.15),
             obj('sphere', [0.0, 1.5, 3.0], 0.1, color=[0, 0, 0], emission=1.0e6, segments=24)])
q = pt(spp=64, bounces=2, sun={'on': False}, exposure={'ev100': 10})
po, pc = os.path.join(tmp, 'open.pfm'), os.path.join(tmp, 'clamped.pfm')
rows, rs = rendered([render_cmd(os.path.join(tmp, 'open.png'), CAM, (160, 120), calibrate=SCN, pfm=po, clamp=None, **q),
                     render_cmd(os.path.join(tmp, 'clamped.png'), CAM, (160, 120), calibrate=SCN, pfm=pc, clamp=4, **q)], 'the clamp renders')
for r in rs:
    is_path(r, 'clamp')
if photo_of(rs[0], 'open').get('clamp') is not None or abs(num(rs[0].get('clampLoss'), 'clampLoss')) > 1e-9:
    fail('without a clamp the reply reports clamp null and clampLoss 0: %r, %r' % (rs[0]['photo'].get('clamp'), rs[0].get('clampLoss')))
near(photo_of(rs[1], 'clamped').get('clamp'), 4.0, 1e-9, 'photo.clamp')
loss = num(rs[1].get('clampLoss'), 'clampLoss')
s_open, s_cl = sum(pfm_lumas(po)[2]), sum(pfm_lumas(pc)[2])
if s_open <= 0:
    fail('the glossy floor shows no reflection of the emitter')
measured = 1.0 - s_cl / s_open
print('clamp 4 at EV100 10: clampLoss %.4f, images say %.4f' % (loss, measured))
if not 0.05 < loss < 1.0:
    fail('the emitter seen in the glossy floor is clamped: clampLoss %.4f' % loss)
if abs(loss - measured) > 0.02:
    fail('clampLoss %.4f is the light removed from the frame: the images say %.4f' % (loss, measured))
print('PASS 07')
