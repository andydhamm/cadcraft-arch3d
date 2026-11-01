# Demo: the night kitchen and a daylit living room, each Finalized raster and path-traced side by side. The path-traced
# Finalize is the default now; bounced light reaches the kitchen ceiling the raster still leaves to the ambient term.
# needs: p11-final-default-path, p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-r3d-denoise, p11-r3d-final-path, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
HOUSE = [cmd('wall', {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True}),
         cmd('wall', {"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "exterior": False}),
         cmd('wall', {"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "exterior": False}),
         'roomdetect {}', cmd('window', {"wall": "w0", "offset": 3.0, "width": 2.0, "sill": 0.9, "height": 1.2}),
         cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 0})]
KITCHEN_CAM = {"kind": "full", "eye": [6.4, 3.4, 2.0], "target": [8.5, 6.0, 0.6], "fov": 80}
LIVING_CAM = {"kind": "full", "eye": [9.5, 2.7, 1.6], "target": [3.0, 0.8, 0.6], "fov": 80}
CEIL, FLOOR_K = [8.0, 5.5, 2.38], [8.0, 5.5, 0.0]
F = lambda name, cam, sun, mode, samples: cmd('render', {"out": os.path.join(tmp, name), "size": [240, 180], "camera": cam, "sun": sun,
                                                         "final": mode, "spp": 64, "samples": samples})
SUN = {"date": "2026-06-21", "time": "12:00"}
rc, rr, so, se = run(HOUSE + [cmd('lightfixture', {"room": "r1", "rows": 2, "cols": 2}),
                              F('kitchen-raster.png', KITCHEN_CAM, False, 'raster', [CEIL, FLOOR_K]),
                              F('kitchen-path.png', KITCHEN_CAM, False, True, [CEIL, FLOOR_K]),
                              F('living-raster.png', LIVING_CAM, SUN, 'raster', [[3.0, 0.9, 0.0]]),
                              F('living-path.png', LIVING_CAM, SUN, 'path', [[3.0, 0.9, 0.0]])])
r = [x for x in rr if isinstance(x, dict) and 'finalMode' in x]
if [x.get('finalMode') for x in r] != ['raster', 'path', 'raster', 'path']:
    fail(f"raster, then the default Finalize (path-traced now), then raster and path: {[x.get('finalMode') for x in r]}")
for name in ['kitchen-raster.png', 'kitchen-path.png', 'living-raster.png', 'living-path.png']:
    w, h, nch, rows = png(os.path.join(tmp, name))
    if (w, h) != (240, 180):
        fail(f"{name} is 240 x 180, got {w} x {h}")
if digest(os.path.join(tmp, 'kitchen-raster.png')) == digest(os.path.join(tmp, 'kitchen-path.png')):
    fail("the path-traced kitchen differs from the raster one")
ceil_raster, ceil_path = rgb_sum(r[0]['samples'][0], 'raster ceiling'), rgb_sum(r[1]['samples'][0], 'path ceiling')
print(f"kitchen ceiling at night (r+g+b): raster {ceil_raster:.0f}, path-traced {ceil_path:.0f}")
if ceil_path < ceil_raster + 15:
    fail(f"bounced light lights the kitchen ceiling more than the raster ambient (+15): {ceil_path} vs {ceil_raster}")
for name in ['kitchen-raster.png', 'kitchen-path.png', 'living-raster.png', 'living-path.png']:
    artifact(os.path.join(tmp, name), name)
print("PASS 13")
