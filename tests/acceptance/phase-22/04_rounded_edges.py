# Rung 2, rounded-edge shading (path tracer, no geometry added). Brief numbers: a 1 m white cube under one directional
# light (the sun at compass azimuth 120, elevation 10: its +X face reads cos 0.853, its -Y face 0.492). From 0.5 m the
# pixel row across the vertical edge has one step with radius 0; with 5 mm it has a gradient at least 3 px wide and a
# highlight pixel brighter than both faces (the blended normal faces the light at 0.951); the cube's total reflected
# energy, from a camera that sees all of it, changes by under 1 %. A material's own edgeRadius 0 switches it off.
# needs: p22-doc-look, p22-arch3d-material-ext, p22-arch3d-calibrate, p22-r3d-types, p22-r3d-kernel-hooks, p22-r3d-material-map, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-edges
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
k = 0.5 * math.sqrt(0.5)
NEAR = {'kind': 'full', 'eye': [0.5 + k, -0.5 - k, 0.0], 'target': [0.5, -0.5, 0.0], 'fov': 40}
FAR = {'kind': 'full', 'eye': [3.0, -3.0, 1.2], 'target': [0.0, 0.0, 0.0], 'fov': 30}
CUBE = calib([obj('box', [0.0, 0.0, 0.0], 1.0, color=[1, 1, 1], roughness=1.0)])
CUBE0 = calib([obj('box', [0.0, 0.0, 0.0], 1.0, color=[1, 1, 1], roughness=1.0, edgeRadius=0.0)])
BASE = pt(spp=64, bounces=0, sun={'azimuth': 120, 'elevation': 10})
n = [0]


def shot(cam, size, cal, **kw):
    n[0] += 1
    p = os.path.join(tmp, 'e%d.pfm' % n[0])
    o = os.path.join(tmp, 'e%d.png' % n[0])
    d = dict(BASE)
    d.update(kw)
    return p, o, render_cmd(o, cam, size, calibrate=cal, pfm=p, **d)


N0 = shot(NEAR, (600, 400), CUBE, edges=False)
N5 = shot(NEAR, (600, 400), CUBE, edges={'radius': 0.005})
F0 = shot(FAR, (300, 300), CUBE, edges=False)
F5 = shot(FAR, (300, 300), CUBE, edges={'radius': 0.005})
M1 = shot(NEAR, (600, 400), CUBE, edges=True)
M0 = shot(NEAR, (600, 400), CUBE0, edges=True)
rows, rs = rendered([c[2] for c in (N0, N5, F0, F5, M1, M0)], 'the cube renders', timeout=3000)
if len(rs) != 6:
    fail('six render replies: %d' % len(rs))
for r in rs:
    is_path(r, 'rounded edges')
if photo_of(rs[0], 'edges off').get('edges') is not False:
    fail('photo.edges echoes false: %r' % rs[0]['photo'].get('edges'))
e5 = photo_of(rs[1], 'edges 5 mm').get('edges')
if not (isinstance(e5, dict) and abs(float(e5.get('radius', 0)) - 0.005) < 1e-9):
    fail('photo.edges echoes {radius: 0.005}: %r' % e5)


def row_of(p):
    w, h, vals = pfm_lumas(p)
    return [vals[200 * w + x] for x in range(w)]


r0, r5 = row_of(N0[0]), row_of(N5[0])
med = lambda v: sorted(v)[len(v) // 2]
A, B = med(r0[100:200]), med(r0[400:500])
if min(A, B) <= 0:
    fail('a face of the cube is black: plateaus %.1f and %.1f nits' % (A, B))
within(max(A, B) / min(A, B), 0.853 / 0.4924, 0.03, 'the two faces read cos 0.853 and 0.492 of the sun')
off = lambda row: [x for x in range(250, 350) if abs(row[x] - A) > 0.02 * A and abs(row[x] - B) > 0.02 * B]
step0, grad5 = off(r0), off(r5)
peak = max(r5[250:350])
print('faces %.0f and %.0f nits; radius 0: %d transition px; 5 mm: %d px, highlight %.0f nits' % (A, B, len(step0), len(grad5), peak))
if len(step0) > 2:
    fail('with radius 0 the row across the edge has one step: %d pixels are neither face' % len(step0))
if len(grad5) < 3:
    fail('with radius 5 mm at 0.5 m the row has a gradient at least 3 px wide: %d px' % len(grad5))
if not peak > 1.02 * max(A, B):
    fail('with radius 5 mm a highlight pixel is brighter than both faces: %.0f vs %.0f' % (peak, max(A, B)))
if max(r0[250:350]) > 1.02 * max(A, B):
    fail('with radius 0 no pixel is brighter than the brighter face')
s0, s5 = sum(pfm_lumas(F0[0])[2]), sum(pfm_lumas(F5[0])[2])
print('total reflected energy: %.6g -> %.6g (%.3f %%)' % (s0, s5, 100 * abs(s5 - s0) / s0))
if s0 <= 0 or abs(s5 - s0) > 0.01 * s0:
    fail('the cube total reflected energy changes by under 1 %%: %.6g -> %.6g' % (s0, s5))
if digest(N0[1]) == digest(M1[1]):
    fail('edges: true (the per-material radius, default 2 mm) changes nothing on the cube')
if digest(N0[1]) != digest(M0[1]):
    fail('a material with edgeRadius 0 keeps its hard edges under edges: true')
print('PASS 04')
