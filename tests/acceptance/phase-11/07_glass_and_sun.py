# Daylight through glass: the noon sun through a south window lights a patch of floor (shadow rays pass the glass,
# tinted), the floor away from it gets sky and bounced light, more with bounces than without.
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
SUN = {"date": "2026-06-21", "time": "12:00"}
CAM = {"kind": "full", "eye": [3.6, 3.6, 1.6], "target": [1.8, 1.0, 0.3], "fov": 80}
PATCH, SHADE = [2.0, 0.85, 0.0], [1.0, 3.2, 0.0]
P = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [120, 90], "camera": CAM, "sun": SUN,
                                         "spp": 64, "samples": [PATCH, SHADE]}, **k))
setup = room_cmds() + [cmd('window', {"wall": "w0", "offset": 2.1, "width": 1.6, "sill": 0.9, "height": 1.2}),
                       cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 0})]
rc, rr, so, se = run(setup + [P('day.png'), P('day-flat.png', bounces=0)])
r = path_renders(rr)
if len(r) != 2:
    fail(f"two path-traced render replies, got {len(r)}: {json.dumps(rr)[-800:]}")
patch, shade = rgb_sum(r[0]['samples'][0], 'sun patch'), rgb_sum(r[0]['samples'][1], 'shaded floor')
flat_shade = rgb_sum(r[1]['samples'][1], 'shaded floor, bounces 0')
print(f"floor (r+g+b): in the sun patch {patch:.0f}, away from it {shade:.0f} (bounces 0: {flat_shade:.0f})")
if patch < shade + 100:
    fail(f"the sun through the glass lights a patch far brighter (by 100+) than the floor away from it: {patch} vs {shade}")
if shade < flat_shade + 10:
    fail(f"bounced daylight brightens the floor away from the patch (+10): bounces 4 {shade}, bounces 0 {flat_shade}")
artifact(os.path.join(tmp, 'day.png'), 'daylit-room-path.png')
print("PASS 07")
