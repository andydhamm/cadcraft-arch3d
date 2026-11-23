# Rung 3, materials (path tracer, materials: full). (a) White furnace per lobe: a white sphere in a uniform radiance-1.0
# environment reflects 1.0 +- 2 %: no energy gain, a loss of up to 5 % is allowed and printed. Lobes: clear coat, sheen,
# anisotropy, rough transmission, thin translucency. (b) each lobe is really there: a coat adds a highlight, sheen lifts
# the rim, anisotropy 0.8 under a point light gives a highlight at least 3 x longer than wide, transmission lets an
# emitter behind the sphere show through. (c) materials: basic ignores the fields. (d) surface variation changes a flat
# card's colour from place to place and keeps its mean within 2 %.
# needs: p22-doc-look, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-r3d-material-map, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-bsdf-ext, p22-r3d-bsdf-transmission, p22-r3d-variation
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
CAM = {'kind': 'full', 'eye': [0.0, -3.0, 0.0], 'target': [0.0, 0.0, 0.0], 'fov': 30}
WHITE = [1, 1, 1]
LOBES = [('base', dict(roughness=0.5), 2),
         ('clear coat', dict(roughness=0.6, clearcoat=1.0, clearcoatRoughness=0.2), 2),
         ('sheen', dict(roughness=0.9, sheen=1.0, sheenRoughness=0.5), 2),
         ('anisotropy', dict(metallic=1.0, roughness=0.3, anisotropy=0.8), 2),
         ('rough transmission', dict(roughness=0.3, transmission=1.0, ior=1.5), 16),
         ('thin translucency', dict(roughness=0.8, transmission=0.6, thin=True), 16)]
n = [0]


def shot(size, objects, lights=None, **kw):
    n[0] += 1
    p, o = os.path.join(tmp, 'm%d.pfm' % n[0]), os.path.join(tmp, 'm%d.png' % n[0])
    d = pt(sun={'on': False}, materials='full', exposure=0.001)
    d.update(kw)
    return p, o, render_cmd(o, CAM, size, calibrate=calib(objects, lights), pfm=p, **d)


def disc(p, r0, r1):
    w, h, vals = pfm_lumas(p)
    c = w / 2
    v = [vals[y * w + x] for y in range(h) for x in range(w) if r0 <= math.hypot(x + 0.5 - c, y + 0.5 - c) < r1]
    return sum(v) / len(v), max(v)


# (a) the furnace: 160 px, the sphere's radius is 50.5 px; the inner 36 px are judged.
S = (160, 160)
fur = [shot(S, [obj('sphere', [0, 0, 0], 1.0, color=WHITE, **m)], spp=128, bounces=b) for _, m, b in LOBES]
rows, rs = rendered([uniform_env(tmp)] + [f[2] for f in fur], 'the furnace renders', timeout=3000)
for r in rs:
    is_path(r, 'furnace')
    if photo_of(r, 'furnace').get('materials') != 'full':
        fail('photo.materials echoes full: %r' % r['photo'].get('materials'))
for (name, _, _), f in zip(LOBES, fur):
    w, h, vals = pfm_lumas(f[0])
    within(vals[2 * w + 2], 1.0, 0.02, 'the environment behind the sphere is radiance 1.0 (the check set-up)')
    mean, _ = disc(f[0], 0, 36)
    print('furnace, %-18s reflects %.4f (loss %.1f %%)' % (name, mean, 100 * max(0.0, 1 - mean)))
    if mean > 1.02:
        fail('white furnace, %s: the sphere reflects %.4f, an energy gain' % (name, mean))
    if mean < 0.95:
        fail('white furnace, %s: the sphere reflects %.4f, a loss of more than 5 %%' % (name, mean))

# (b) the lobes are there. A point light at the camera; 240 px (sphere radius 75.7 px).
B = (240, 240)
LIGHT = [{'at': [0.0, -3.0, 0.3], 'candela': 1000}]
sph = lambda **m: [obj('sphere', [0, 0, 0], 1.0, color=WHITE, **m)]
PANEL = obj('box', [0.0, 2.0, 0.0], [3.0, 0.05, 3.0], color=[0, 0, 0], emission=1000.0)
c_base = shot(B, sph(roughness=0.6), LIGHT, spp=16, bounces=0)
c_coat = shot(B, sph(roughness=0.6, clearcoat=1.0, clearcoatRoughness=0.2), LIGHT, spp=16, bounces=0)
s_base = shot(B, sph(roughness=0.9), LIGHT, spp=16, bounces=0)
s_sheen = shot(B, sph(roughness=0.9, sheen=1.0, sheenRoughness=0.5), LIGHT, spp=16, bounces=0)
a_iso = shot(B, sph(metallic=1.0, roughness=0.3), LIGHT, spp=16, bounces=0)
a_ani = shot(B, sph(metallic=1.0, roughness=0.3, anisotropy=0.8), LIGHT, spp=16, bounces=0)
a_basic = shot(B, sph(metallic=1.0, roughness=0.3, anisotropy=0.8), LIGHT, spp=16, bounces=0, materials='basic')
t_opq = shot(B, sph(roughness=0.3) + [PANEL], None, spp=64, bounces=8)
t_tr = shot(B, sph(roughness=0.3, transmission=1.0, ior=1.5) + [PANEL], None, spp=64, bounces=8)
t_thin = shot(B, sph(roughness=0.8, transmission=0.6, thin=True) + [PANEL], None, spp=64, bounces=8)
allb = (c_base, c_coat, s_base, s_sheen, a_iso, a_ani, a_basic, t_opq, t_tr, t_thin)
rendered([x[2] for x in allb], 'the lobe renders', timeout=3000)
pk0, pk1 = disc(c_base[0], 0, 70)[1], disc(c_coat[0], 0, 70)[1]
print('clear coat: peak %.1f -> %.1f nits' % (pk0, pk1))
if not pk1 >= 3 * pk0 > 0:
    fail('a clear coat (roughness 0.2) over a rough base adds a highlight at least 3 x the base peak: %.2f vs %.2f' % (pk1, pk0))
rim = lambda p: disc(p, 60, 72)[0] / disc(p, 0, 22)[0]
print('sheen: rim / centre %.3f -> %.3f' % (rim(s_base[0]), rim(s_sheen[0])))
if not rim(s_sheen[0]) >= 1.15 * rim(s_base[0]):
    fail('sheen lifts the rim: rim / centre %.3f with sheen, %.3f without' % (rim(s_sheen[0]), rim(s_base[0])))


def highlight(p):
    w, h, vals = pfm_lumas(p)
    pk = max(vals)
    s, cx, cy, xx, yy, xy = moments(w, h, [1.0 if v > 0.5 * pk else 0.0 for v in vals])
    return eig_aspect(xx, yy, xy), s


h_iso, h_ani = highlight(a_iso[0]), highlight(a_ani[0])
print('anisotropy: highlight aspect %.2f (%d px) isotropic, %.2f (%d px) at 0.8' % (h_iso[0], h_iso[1], h_ani[0], h_ani[1]))
if h_iso[0] > 1.5:
    fail('the isotropic highlight under a light at the camera is round: aspect %.2f' % h_iso[0])
if h_ani[0] < 3.0:
    fail('anisotropy 0.8 gives a highlight at least 3 x longer than wide: %.2f' % h_ani[0])
if digest(a_basic[1]) != digest(a_iso[1]):
    fail('(c) materials: basic ignores the anisotropy field: the image differs from the plain metal sphere')
m_o, m_t, m_n = disc(t_opq[0], 0, 55)[0], disc(t_tr[0], 0, 55)[0], disc(t_thin[0], 0, 55)[0]
print('emitter behind the sphere: opaque %.3f, rough transmission %.1f, thin %.1f nits' % (m_o, m_t, m_n))
if not (m_o < 0.5 and m_t > 5.0 and m_n > 5.0):
    fail('an emitter behind the sphere shows through rough glass and a thin translucent sheet, not through the opaque one: %.3f, %.2f, %.2f' % (m_o, m_t, m_n))

# (d) surface variation on a flat grey card facing the sun and the camera.
card = lambda **m: [obj('box', [0, 0, 0], [2.0, 0.02, 2.0], color=[0.5, 0.5, 0.5], roughness=0.8, **m)]
V = dict(spp=16, bounces=0, sun={'azimuth': 180, 'elevation': 20})
v0 = shot(S, card(variation={'scale': 0.2, 'amount': 0.5}), None, variation=False, **V)
v1 = shot(S, card(variation={'scale': 0.2, 'amount': 0.5}), None, variation=True, **V)
rendered([v0[2], v1[2]], 'the variation renders')


def stats(p):
    w, h, vals = pfm_lumas(p)
    v = [vals[y * w + x] for y in range(50, 110) for x in range(50, 110)]
    m = sum(v) / len(v)
    return m, math.sqrt(sum((x - m) ** 2 for x in v) / len(v)) / m


(mean0, sd0), (mean1, sd1) = stats(v0[0]), stats(v1[0])
print('variation: sd / mean %.4f off, %.4f on; mean %.1f -> %.1f' % (sd0, sd1, mean0, mean1))
if sd0 > 0.01:
    fail('with variation off the card is flat: sd / mean %.4f' % sd0)
if sd1 < 0.02:
    fail('with variation on the card varies from place to place: sd / mean %.4f' % sd1)
within(mean1, mean0, 0.02, 'variation keeps the card mean colour')
print('PASS 05')
