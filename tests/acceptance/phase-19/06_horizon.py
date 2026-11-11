# Ground to the horizon: outside the site skirt a flat ground plane at the site boundary's mean edge elevation, in the
# site's ground material, reaching the far plane, so no exterior view shows the edge of the world. Brief number: an
# exterior camera 1.6 m above the site looking level: the pixel row 5 px under the horizon has no sky or clear-colour
# pixels across its full width. Here the 40 x 30 site ends 25 m ahead and row 125 of 240 (fov 60) meets the ground
# about 66 m ahead, so only the horizon plane can fill it. Checked in the interactive view, the raster Finalize and the
# path tracer; the path tracer leaves a miss black without a backdrop (never the clear colour), so a magenta colour
# backdrop is what shows a miss there. (a2) the plane is the site's ground material: green like phase-10 check 08's
# terrain and within 12 per channel of the site's own sunlit terrain on row 140 (about 16 m ahead). Probes report part
# "horizon" outside the boundary, the plane is not in the scene's bounds or meshes, and it sits at the mean edge
# elevation (corners 0, 2, 2, 0 -> (40 + 60 + 40 + 0) / 140 = 1.0).
# needs: p19-horizon-1, p19-horizon-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
BOUND = [[0, 0], [40, 0], [40, 30], [0, 30]]
TERRAIN = cmd('terrain', {"boundary": BOUND, "elevation": 0})
SET = [TERRAIN] + room_cmds(16, 10)
CAM = {"kind": "full", "eye": [20, 5, 1.6], "target": [20, 1000, 1.6], "fov": 60}
M = (('interactive', {}), ('raster', {"final": "raster"}), ('path', {"final": "path", "spp": 4, "seed": 0}))
cmds, outs = [], {}
for mode, extra in M:
    for bg in ('none', 'magenta'):
        o = os.path.join(tmp, f'{mode}-{bg}.png')
        outs[(mode, bg)] = o
        cmds.append(render_cmd(o, CAM, (320, 240), **extra))
    # the magenta backdrop is set between the two renders of each mode below
rows, rs = rendered(SET + [c for i, c in enumerate(cmds) if i % 2 == 0] +
                    [cmd('backdrop', {"kind": "colour", "colour": "#ff00ff"})] + [c for i, c in enumerate(cmds) if i % 2 == 1],
                    "level views over the site")
for mode, _ in M:
    w, h, p = img(outs[(mode, 'none')])
    row = [p[125 * w + x] for x in range(w)]
    clear = [x for x, c in enumerate(row) if all(abs(c[k] - CLEAR[k]) <= 8 for k in range(3))]
    black = [x for x, c in enumerate(row) if all(c[k] <= 8 for k in range(3))]
    if clear or black:
        fail(f"(a) {mode}: row 125 (5 px under the horizon) shows the clear colour at columns {clear[:8]} or nothing "
             f"(black) at {black[:8]}")
    m125 = box_mean((w, h, p), 180, 125, 320, 126, linear=False)
    m140 = box_mean((w, h, p), 180, 140, 320, 141, linear=False)
    if not (m125[1] > m125[0] + 8 and m125[1] > m125[2] + 8):
        fail(f"(a2) {mode}: the horizon plane is the site's green ground material: row 125 mean {m125}")
    if any(abs(m125[k] - m140[k]) > 12 for k in range(3)):
        fail(f"(a2) {mode}: the horizon plane reads like the site's own terrain (within 12 per channel): {m125} vs {m140}")
    w, h, p = img(outs[(mode, 'magenta')])
    tr = p[2 * w + w - 3]
    if not (tr[0] - tr[1] > 100 and tr[2] - tr[1] > 100):
        fail(f"(a) {mode}: the magenta backdrop shows in the sky (top right pixel {tr}), so the test below can see a miss")
    mag = [x for x in range(w) if (lambda c: c[0] - c[1] > 100 and c[2] - c[1] > 100)(p[125 * w + x])]
    if mag:
        fail(f"(a) {mode}: with a magenta backdrop row 125 shows the backdrop (the edge of the world) at columns {mag[:8]}")
    print(f"{mode}: row 125 ground everywhere; mean {[round(v) for v in m125]} vs site terrain {[round(v) for v in m140]}")

# (b) probes; the plane goes with the terrain.
rows = run_ok([TERRAIN, cmd('build3d', {"probe": [[500, 500, -0.5], [500, 500, 0.5], [20, 15, -0.5]]}),
               'terrain.delete {}', cmd('build3d', {"probe": [[500, 500, -0.5]]})], "horizon probes")
b = [r for r in rows if isinstance(r, dict) and 'probe' in r and 'bounds' in r]
if len(b) != 2:
    fail(f"two build3d replies: {len(b)}")
pr = b[0]['probe']
if not (isinstance(pr[0], dict) and pr[0].get('part') == 'horizon' and pr[1] is None and isinstance(pr[2], dict)
        and pr[2].get('part') == 'terrain'):
    fail(f"(b) probes: (500, 500, -0.5) is the horizon, (500, 500, 0.5) nothing, (20, 15, -0.5) the terrain: {pr}")
if b[0].get('horizon') != 1 or b[1].get('horizon') != 0 or b[1]['probe'][0] is not None:
    fail(f"(b) build3d reports horizon 1 with a terrain, 0 after terrain.delete (and the far probe hits nothing): "
         f"{b[0].get('horizon')}, {b[1].get('horizon')}, {b[1]['probe']}")
bx = b[0]['bounds']
if not (bx['min'][0] >= -1 and bx['min'][1] >= -1 and bx['max'][0] <= 41 and bx['max'][1] <= 31):
    fail(f"(d) the horizon plane stays out of the scene bounds (x -1..41, y -1..31): {bx}")
if 'horizon' in (b[0].get('parts') or {}) or b[0].get('meshes') != 1:
    fail(f"(d) the horizon plane is not a scene mesh (meshes 1: the terrain; no 'horizon' part): {b[0].get('meshes')}, "
         f"{b[0].get('parts')}")

# (c) the plane sits at the boundary's mean edge elevation.
rows = run_ok([cmd('terrain', {"boundary": BOUND, "points": [[0, 0, 0], [40, 0, 2], [40, 30, 2], [0, 30, 0]]}),
               cmd('build3d', {"probe": [[500, 500, 0.9], [500, 500, 1.1]]})], "a sloped site")
pr = [r for r in rows if isinstance(r, dict) and 'probe' in r][-1]['probe']
if not (isinstance(pr[0], dict) and pr[0].get('part') == 'horizon' and pr[1] is None):
    fail(f"(c) the plane's top is the mean edge elevation 1.0: (500, 500, 0.9) horizon, (500, 500, 1.1) nothing: {pr}")
print("PASS 06")
