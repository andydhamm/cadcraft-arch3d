# The night room path-traced: the pot lights' cones on the floor, bounced light between them (more with bounces than
# without), a fixed seed gives the same PNG, and the raster Finalize stays available as final "raster".
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-r3d-denoise, p11-r3d-final-path, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

SAMPLES = [[1, 1, 0], [3, 1, 0], [2, 2, 0], [2, 1, 0]]
tmp = tempfile.mkdtemp()
P = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [160, 120], "camera": NIGHT_CAM,
                                         "sun": False, "spp": 64, "samples": SAMPLES}, **k))
rc, rr, so, se = run(room_cmds() + [POTS, P('a.png'), P('b.png'), P('c.png', seed=1), P('flat.png', bounces=0),
                                    cmd('render', {"out": os.path.join(tmp, 'raster.png'), "final": "raster", "size": [160, 120],
                                                   "camera": NIGHT_CAM, "sun": False, "samples": SAMPLES})])
r = path_renders(rr)
if len(r) != 4:
    fail(f"four path-traced render replies, got {len(r)}: {json.dumps(rr)[-800:]}")
if [num(x.get('pathTraces'), 'pathTraces') for x in r] != [1, 2, 3, 4]:
    fail(f"each path-traced still counts one path trace: {[x.get('pathTraces') for x in r]}")
a = r[0]
if a.get('denoised') is not True or num(a.get('spp'), 'spp') != 64 or num(a.get('bounces'), 'bounces') != 4 or num(a.get('seed'), 'seed') != 0:
    fail(f"the defaults: denoised, bounces 4, seed 0, and spp 64 as asked: {a}")
w, h, nch, rows = png(os.path.join(tmp, 'a.png'))
if (w, h) != (160, 120):
    fail(f"a.png is 160 x 120, got {w} x {h}")
lit = [rgb_sum(a['samples'][k], f"path sample {SAMPLES[k]}") for k in range(4)]
print(f"path-traced night floor (r+g+b): under lf0 {lit[0]:.0f}, under lf2 {lit[1]:.0f}, between cones {lit[3]:.0f}, outside every cone {lit[2]:.0f}")
if min(lit[0], lit[1], lit[3]) < lit[2] + 90:
    fail(f"the floor inside the cones is far brighter (by 90+) than outside every cone: {lit}")
flat = rgb_sum(r[3]['samples'][2], 'bounces 0 sample (2,2,0)')
if lit[2] < flat + 10:
    fail(f"bounced light brightens the floor outside every cone (bounces 4 {lit[2]:.0f} vs bounces 0 {flat:.0f}, want +10)")
if digest(os.path.join(tmp, 'a.png')) != digest(os.path.join(tmp, 'b.png')):
    fail("the same seed renders the same PNG, byte for byte")
if digest(os.path.join(tmp, 'a.png')) == digest(os.path.join(tmp, 'c.png')):
    fail("seed 1 renders a different image from seed 0")
ras = [x for x in rr if isinstance(x, dict) and x.get('finalMode') == 'raster']
if len(ras) != 1 or not os.path.exists(os.path.join(tmp, 'raster.png')):
    fail(f"final \"raster\" still renders the phase-10 still: {json.dumps(rr)[-400:]}")
artifact(os.path.join(tmp, 'a.png'), 'night-path.png'); artifact(os.path.join(tmp, 'raster.png'), 'night-raster.png')
print("PASS 03")
