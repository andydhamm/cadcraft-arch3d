# Rung 5, film (path tracer, before the tone curve). (a) every control neutral = the film-off image, pixel for pixel;
# (b) bloom spreads light from pixels above the threshold and keeps the frame's summed linear energy within 1 %;
# (c) vignette v multiplies by 1 - v (d / d_corner)^2: 0.15 leaves the centre and darkens the corner to 0.85;
# (d) white balance 3000 K makes a card lit by a 3000 K fixture neutral; (e) a mid-grey card stays mid-grey +- 2 % under
# the Photoreal film, contrast pivots on 0.18, saturation 0 is grey, lift adds; (f) the raster Finalize ignores the film.
# needs: p22-doc-look, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-film
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
S = (160, 120)
NEUTRAL = {'bloom': 0, 'bloomThreshold': 1.0, 'vignette': 0, 'whiteBalance': None, 'contrast': 1, 'saturation': 1, 'lift': 0}
n = [0]


def shot(cam, size, **kw):
    n[0] += 1
    p, o = os.path.join(tmp, 'f%d.pfm' % n[0]), os.path.join(tmp, 'f%d.png' % n[0])
    return p, o, render_cmd(o, cam, size, pfm=p, pfmStage='film', **kw)


# (a) and (f) in the room.
C2 = {'kind': 'full', 'eye': [2.0, 0.4, 1.4], 'target': [2.0, 3.0, 0.8], 'fov': 60}
q = dict(final='path', spp=4, seed=2, bounces=1, sun={'on': False})
a0, a1, a2 = shot(C2, S, **q), shot(C2, S, film=NEUTRAL, **q), shot(C2, S, film='photoreal', **q)
r0, r1 = shot(C2, S, final='raster'), shot(C2, S, final='raster', film='photoreal')
rows, rs = rendered(ROOM + [COLUMN, cmd('lightfixture', {'room': 'r0', 'rows': 1, 'cols': 1})] + [x[2] for x in (a0, a1, a2, r0, r1)], 'film in the room')
if digest(a0[1]) != digest(a1[1]):
    fail('(a) with every film control at its neutral value the image is identical to the film-off image')
if digest(a0[1]) == digest(a2[1]):
    fail('(a) film: photoreal changes nothing')
fm = look_of(rs[2], 'film photoreal').get('film') or {}
for key, want in (('bloom', 0.1), ('vignette', 0.15), ('contrast', 1.05), ('saturation', 1.0), ('lift', 0.0)):
    near(fm.get(key), want, 1e-9, 'look.film.%s of film: photoreal' % key)
if digest(r0[1]) != digest(r1[1]):
    fail('(f) the raster Finalize keeps its look: the film changed it')
print('ok: (a) neutral film is the film-off image, (f) the raster ignores the film')

# (b) bloom: an emitter 20 display units bright, 0.4 m across at 5 m (6.6 px radius), on black.
CAM = level_cam([0.0, 0.0, 0.0], 90, 0, 40)
EM = calib([obj('sphere', [0.0, 5.0, 0.0], 0.4, color=[0, 0, 0], emission=20.0)])
b = pt(spp=64, bounces=0, sun={'on': False}, exposure=1.0)
b0, b1 = shot(CAM, S, calibrate=EM, **b), shot(CAM, S, calibrate=EM, film={'bloom': 0.3}, **b)
l0, l1 = shot(CAM, S, calibrate=EM, film={'lift': 0.05}, **b), None
rendered([b0[2], b1[2], l0[2]], 'the bloom renders')
w, h, v0 = pfm_lumas(b0[0])
_, _, v1 = pfm_lumas(b1[0])
e0, e1 = sum(v0), sum(v1)
ring = 60 * w + 94
print('bloom: energy %.3f -> %.3f, a pixel 14 px from the centre %.5f -> %.5f, peak %.2f -> %.2f' % (e0, e1, v0[ring], v1[ring], max(v0), max(v1)))
within(max(v0), 20.0, 0.02, 'the emitter reads 20 display units before the film (the check set-up)')
if abs(e1 - e0) > 0.01 * e0:
    fail('(b) bloom keeps the frame summed linear energy within 1 %%: %.3f -> %.3f' % (e0, e1))
if not (v0[ring] < 1e-6 and v1[ring] > 1e-4):
    fail('(b) bloom spreads light beyond the emitter: the pixel 14 px out reads %.6f -> %.6f' % (v0[ring], v1[ring]))
if not max(v1) < max(v0):
    fail('(b) the light bloom spreads comes from the bright pixels: peak %.3f -> %.3f' % (max(v0), max(v1)))
_, _, vl = pfm_lumas(l0[0])
near(vl[5 * w + 5], 0.05, 0.003, '(e) lift 0.05 on a black pixel')

# (c) vignette on a flat frame (the furnace, radiance 2.0, exposure 1).
FC = {'kind': 'full', 'eye': [1.0, 1.0, 1.2], 'target': [3.0, 3.0, 1.0], 'fov': 100}
fq = dict(final='path', spp=256, seed=1, denoise=False, toneMap='none', furnace={'albedo': 0.01, 'emission': 1.98})
g0, g1 = shot(FC, S, **fq), shot(FC, S, film={'vignette': 0.15}, **fq)
rendered(ROOM + [g0[2], g1[2]], 'the vignette renders')
_, _, u0 = pfm_lumas(g0[0])
_, _, u1 = pfm_lumas(g1[0])
ratio = lambda x, y: u1[y * w + x] / u0[y * w + x]
print('vignette 0.15: centre %.3f, mid edge %.3f, corner %.3f' % (ratio(80, 60), ratio(0, 60), ratio(0, 0)))
within(u0[60 * w + 80], 2.0, 0.02, 'the furnace frame reads 2.0 (the check set-up)')
if not (0.99 <= ratio(80, 60) <= 1.001 and 0.89 <= ratio(0, 60) <= 0.92 and 0.84 <= ratio(0, 0) <= 0.87):
    fail('(c) vignette 0.15 is 1 - 0.15 (d / d_corner)^2: centre %.3f, mid edge %.3f, corner %.3f' % (ratio(80, 60), ratio(0, 60), ratio(0, 0)))

# (d) white balance and saturation: a card lit by one 3000 K light.
KC = {'kind': 'full', 'eye': [0.0, -3.0, 0.0], 'target': [0.0, 0.0, 0.0], 'fov': 30}
card = lambda col: calib([obj('box', [0, 0, 0], [2.0, 0.02, 2.0], color=col, roughness=1.0)], [{'at': [0.0, -2.0, 0.5], 'candela': 500, 'kelvin': 3000}])
k = pt(spp=16, bounces=0, sun={'on': False}, exposure=0.005)
w0, w1 = shot(KC, S, calibrate=card([1, 1, 1]), **k), shot(KC, S, calibrate=card([1, 1, 1]), film={'whiteBalance': 3000}, **k)
s1 = shot(KC, S, calibrate=card([0.8, 0.2, 0.2]), film={'saturation': 0}, **k)
rendered([w0[2], w1[2], s1[2]], 'the white-balance renders')
mid = lambda p: read_pfm(p)[2][60 * 160 + 80]
c0, c1, cs = mid(w0[0]), mid(w1[0]), mid(s1[0])
print('3000 K on white: r/b %.3f, with whiteBalance 3000: r/g %.3f r/b %.3f' % (c0[0] / c0[2], c1[0] / c1[1], c1[0] / c1[2]))
if not c0[0] / c0[2] > 1.3:
    fail('(d) a 3000 K fixture on a white card reads warm without white balance: r/b %.3f' % (c0[0] / c0[2]))
if not (0.95 < c1[0] / c1[2] < 1.05 and 0.95 < c1[0] / c1[1] < 1.05):
    fail('(d) whiteBalance 3000 makes the card neutral: r %.4f g %.4f b %.4f' % c1)
if max(cs) - min(cs) > 0.01 * max(cs) or max(cs) <= 0:
    fail('(e) saturation 0 makes a red card grey: %r' % (cs,))

# (e) the mid-grey card: open ground of albedo 0.5 under the sun straight down is 15 915.5 nits (phase-19 check 01).
L = 0.5 / math.pi * 100000.0
GC = down_cam(3.0, 1.5, 0.8, 30)
g = dict(final='path', spp=16, bounces=0, seed=0, denoise=False, ground=True, albedo=0.5, sun={'azimuth': 0, 'elevation': 90}, sky=False, toneMap='none')
m0 = shot(GC, (64, 48), exposure=0.18 / L, **g)
m1 = shot(GC, (64, 48), exposure=0.18 / L, film='photoreal', **g)
m2 = shot(GC, (64, 48), exposure=0.18 / L, film={'contrast': 1.5}, **g)
m3 = shot(GC, (64, 48), exposure=0.36 / L, film={'contrast': 1.5}, **g)
rendered([COLUMN] + [x[2] for x in (m0, m1, m2, m3)], 'the mid-grey renders')
cv = lambda p: pfm_lumas(p)[2][24 * 64 + 32]
print('mid grey: %.4f film off, %.4f Photoreal film, contrast 1.5: %.4f at 0.18 and %.4f at 0.36' % (cv(m0[0]), cv(m1[0]), cv(m2[0]), cv(m3[0])))
within(cv(m0[0]), 0.18, 0.02, 'the card reads mid grey without the film (the check set-up)')
within(cv(m1[0]), 0.18, 0.02, '(e) a mid-grey card stays mid-grey under the default grade')
within(lum(centre_mean(m1[1])), 0.18, 0.03, '(e) the same in the PNG')
within(cv(m2[0]), 0.18, 0.02, '(e) contrast pivots on 0.18')
within(cv(m3[0]), 0.45, 0.03, '(e) contrast 1.5 takes 0.36 to 0.45')
print('PASS 08')
