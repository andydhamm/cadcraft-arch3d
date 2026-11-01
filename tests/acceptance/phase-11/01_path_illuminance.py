# Path-traced illuminance in the phase-10 room: with bounces 0 it is the direct light (165 lx under a pot light, none
# behind the column); with bounces on, the shadowed point gets bounced light; a seed fixes the estimate.
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

ill = lambda p, **k: cmd('illuminance', dict({"point": p, "method": "path"}, **k))
rc, rr, so, se = run(room_cmds() + [POTS, COLUMN,
                                    ill([1, 1, 0], bounces=0, spp=256),
                                    ill([1.8, 1.8, 0], bounces=0, spp=256),
                                    ill([1.8, 1.8, 0], bounces=4, spp=1024),
                                    ill([1.8, 1.8, 0], bounces=4, spp=1024),
                                    ill([1.8, 1.8, 0], bounces=4, spp=1024, seed=7),
                                    cmd('illuminance', {"point": [1, 1, 0]})])
r = [x for x in rr if isinstance(x, dict) and x.get('method') == 'path']
if len(r) != 5:
    fail(f"five path-traced illuminance replies, got {len(r)}: {json.dumps(rr)[-800:]}")
for x, (b, n) in zip(r, [(0, 256), (0, 256), (4, 1024), (4, 1024), (4, 1024)]):
    if num(x.get('bounces'), 'bounces') != b or num(x.get('spp'), 'spp') != n or not isinstance(x.get('rgb'), list):
        fail(f"a path reply echoes its bounces and spp and gives rgb: {x}")
near(r[0]['lux'], 165.0, 165.0 * 0.03, 'path-traced lux under lf0 with bounces 0 (950.4 cd / 2.4² m²)')
if num(r[1]['lux'], 'lux') > 0.5:
    fail(f"with bounces 0 the column's shadow at (1.8,1.8,0) gets no light (<= 0.5 lx): {r[1]['lux']}")
if num(r[2]['lux'], 'lux') <= 1.0:
    fail(f"with bounces on, bounced light reaches (1.8,1.8,0) behind the column (> 1 lx): {r[2]['lux']}")
if r[2]['lux'] != r[3]['lux']:
    fail(f"the same seed gives the same estimate: {r[2]['lux']} vs {r[3]['lux']}")
if r[4]['lux'] == r[2]['lux']:
    fail(f"another seed gives another estimate: seed 7 {r[4]['lux']} = seed 0 {r[2]['lux']}")
d = [x for x in rr if isinstance(x, dict) and 'lux' in x and x.get('method') in (None, 'direct')]
if not d:
    fail(f"the phase-10 direct illuminance still answers: {json.dumps(rr)[-400:]}")
near(d[-1]['lux'], 165.0, 1.65, 'direct (phase-10) lux under lf0')
print(f"ok: path lux {r[0]['lux']:.1f} under lf0, {r[1]['lux']:.2f} in the shadow without bounces, {r[2]['lux']:.1f} with bounces")
print("PASS 01")
