# Carry-over from phase 11: a path-traced still with the sun on is exposed for what it shows. At the fixed day exposure
# (pi / 100 000: a sunlit white surface at 1.0) a living room lit through its window is nearly black (the floor away from
# the sun patch read 3-6 of 765). `render {"exposure": "auto"}` is the default with the sun on: 0.18 over the log-average
# luminance of the still's surfaces, clamped between the day exposure and the night exposure (0.01); the reply says the
# exposure used. A number is used as given; at night the night exposure stays (auto or not).
# needs: p12-pt-auto-exposure
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp()
SUN = {"date": "2026-06-21", "time": "12:00"}
ROOM = room_cmds() + [cmd('window', {"wall": "w0", "offset": 2.1, "width": 1.6, "sill": 0.9, "height": 1.2}),
                      cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 0})]
CAM = {"kind": "full", "eye": [3.6, 3.6, 1.6], "target": [1.8, 1.0, 0.3], "fov": 80}
FAR = [1.0, 3.2, 0.0]                    # floor away from the sun patch (phase-11 check 07's shaded point)
R = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [120, 90], "camera": CAM,
                                          "spp": 64, "samples": [FAR]}, **k))
rc, rr, so, se = run(ROOM + [R('auto.png', sun=SUN), R('fixed.png', sun=SUN, exposure=3.14159265e-5),
                             R('night.png', sun=False), R('night-auto.png', sun=False, exposure="auto")])
r = path_renders(rr)
if len(r) != 4:
    fail(f"four path-traced replies: {len(r)}")
e_auto, e_fixed = num(r[0].get('exposure'), 'auto exposure'), num(r[1].get('exposure'), 'fixed exposure')
far_auto, far_fixed = rgb_sum(r[0]['samples'][0], 'far floor, auto'), rgb_sum(r[1]['samples'][0], 'far floor, fixed')
print(f"daylit room: auto exposure {e_auto:.3g} -> far floor {far_auto:.0f}; fixed {e_fixed:.3g} -> {far_fixed:.0f} (r+g+b)")
if not (3.1e-5 < e_auto <= 0.01):
    fail(f"auto exposure lies between the day exposure (3.14e-5) and the night exposure (0.01): {e_auto}")
if abs(e_fixed - 3.14159265e-5) > 1e-9:
    fail(f"a number is used as given: {e_fixed}")
if far_auto < 90:
    fail(f"with auto exposure the floor away from the sun patch is visible (r+g+b >= 90): {far_auto}")
if far_fixed > 30:
    fail(f"the fixed day exposure still renders the room dark (r+g+b <= 30), so auto is what lifts it: {far_fixed}")
n1, n2 = num(r[2].get('exposure'), 'night exposure'), num(r[3].get('exposure'), 'night auto exposure')
if abs(n1 - 0.01) > 1e-9 or abs(n2 - 0.01) > 1e-9 or digest(os.path.join(tmp, 'night.png')) != digest(os.path.join(tmp, 'night-auto.png')):
    fail(f"at night the night exposure (0.01) stays, auto or not: {n1}, {n2}")
artifact(os.path.join(tmp, 'auto.png'), 'daylit-room-auto-exposure.png')
print("PASS 14")
