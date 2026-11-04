# Hardware ray queries are optional: asked for with rt, they are used only where the adapter offers them, and give the
# compute path's image within noise. SKIPs (passes) where the adapter has no ray queries (the reply says rt false and
# carries no rtError). The gates' software adapter (llvmpipe) offers them, so there the comparison runs. Until 2026-11-03
# the ray-query kernel never compiled and the session kept its first (compute) device, so this check skipped everywhere;
# an adapter that offers ray queries but whose kernel fails to build now fails it (rtError).
# needs: p11-r3d-rt, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
SAMPLES = [[1, 1, 0], [2, 2, 0], [2.0, 4.0, 1.2]]
P = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [96, 72], "camera": NIGHT_CAM,
                                         "sun": False, "spp": 256, "samples": SAMPLES}, **k))
rc, rr, so, se = run(room_cmds() + [POTS, COLUMN, P('compute.png'), P('rt.png', rt=True)])
r = path_renders(rr)
if len(r) != 2:
    fail(f"two path-traced replies, got {len(r)}: {json.dumps(rr)[-500:]}")
if r[0].get('rt') is not False:
    fail(f"without rt the compute path runs (rt false): {r[0].get('rt')}")
if r[1].get('rtError'):
    fail(f"the adapter offers ray queries but the ray-query kernel did not build: {str(r[1]['rtError'])[:800]}")
if r[1].get('rt') is not True:
    print(f"SKIP: the adapter offers no ray queries ({r[1].get('device')}); the compute path rendered it")
    if not os.path.exists(os.path.join(tmp, 'rt.png')):
        fail("asked for rt without ray queries, the still is still written (compute path)")
    print("PASS 10")
    sys.exit(0)
_, _, a = pixels(os.path.join(tmp, 'compute.png'))
_, _, b = pixels(os.path.join(tmp, 'rt.png'))
d = mean_abs(a, b)
print(f"mean |compute - ray query| = {d:.2f} levels")
if d > 2.0:
    fail(f"the ray-query image matches the compute path within noise (<= 2 levels mean): {d:.2f}")
for k in range(3):
    if abs(rgb_sum(r[0]['samples'][k], 'compute') - rgb_sum(r[1]['samples'][k], 'rt')) > 18:
        fail(f"sample {SAMPLES[k]}: compute {r[0]['samples'][k]['rgb']} vs rt {r[1]['samples'][k]['rgb']}")
print("PASS 10")
