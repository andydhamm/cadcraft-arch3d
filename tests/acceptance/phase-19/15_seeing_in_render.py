# Seeing in, rendered: the cutaway, the clip box and the near clip are honoured by the interactive view, the raster
# Finalize and the path tracer, shadows included (clipped geometry casts none). HOUSE1 with check 07's window. Every
# render pins the sun from the south at 60 degrees, sky off, EV100 15 and tone mapping off, and a magenta colour backdrop
# shows what sees nothing (the path tracer leaves misses black without a backdrop). At bounces 0 the path tracer gives
# glass no direct light and room A's north wall lies in the ceiling's shadow, so every pixel test sets a sunlit surface
# against a shadowed one or the backdrop, never two dark ones.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-seeing-in, p19-seeing-in-render-1, p19-seeing-in-render-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
SET = HOUSE1 + [cmd('window', {"wall": "w0", "offset": 3.0, "width": 1.2, "sill": 0.9, "height": 1.2}),
                cmd('backdrop', {"kind": "colour", "colour": "#ff00ff"})]
PIN = {"sun": {"azimuth": 180, "elevation": 60}, "sky": False, "exposure": {"ev100": 15}, "toneMap": "none"}
M = (('interactive', {}), ('raster', {"final": "raster"}),
     ('path', {"final": "path", "spp": 4, "bounces": 0, "seed": 0, "denoise": False}))
SOUTH = {"kind": "full", "eye": [3.0, -8, 1.5], "target": [3.0, 0, 1.5], "fov": 40}
ABOVE = {"kind": "full", "eye": [2.025, 2.5, 20], "target": [2.025, 2.501, 0], "fov": 30}
WALLCAM = {"kind": "full", "eye": [1.0, 0.7, 1.5], "target": [1.0, -5, 1.5], "fov": 40}
CLOSE = {"kind": "full", "eye": [3.0, -0.6, 1.5], "target": [3.0, 0, 1.5], "fov": 40}
CLIP = {"min": [-100, -100, -100], "max": [100, 100, 2.0]}
SIZE = (200, 150)
shots = [('a0', SOUTH, {}, [[1.0, 4.89, 1.2]]), ('a1', SOUTH, {"cutaway": True}, [[1.0, 4.89, 1.2]]),
         ('b0', ABOVE, {}, [[3.0, 1.5, 0.0]]), ('b1', ABOVE, {"clip": CLIP}, [[3.0, 1.5, 0.0], [1.0, 1.5, 0], [1.0, 0.6, 0]]),
         ('c0', WALLCAM, {}, [[1.0, 0.11, 1.5]]), ('c1', WALLCAM, {"nearClip": 1.0}, [[1.0, 0.11, 1.5]]),
         ('c2', CLOSE, {"nearClip": 1.0}, [[3.0, 4.89, 1.5]])]
cmds, keys = [], []
for mode, extra in M:
    for tag, cam, k, samples in shots:
        o = os.path.join(tmp, f'{tag}-{mode}.png')
        cmds.append(render_cmd(o, cam, SIZE, samples=samples, **PIN, **k, **extra))
        keys.append((mode, tag, o))
rows, rs = rendered(SET + cmds, "seeing-in renders")
if len(rs) != len(keys):
    fail(f"{len(keys)} render replies: {len(rs)}")
R = {(m, t): (r, o) for r, (m, t, o) in zip(rs, keys)}


def sample(mode, tag, i=0):
    s = R[(mode, tag)][0]['samples'][i]
    if not isinstance(s, dict) or not isinstance(s.get('rgb'), list):
        fail(f"{mode} {tag}: sample {i} has a pixel in the frame: {s}")
    return s


for mode, _ in M:
    if not isinstance(R[(mode, 'a1')][0].get('filter'), dict) or R[(mode, 'a1')][0]['filter'].get('hidden', 0) < 1:
        fail(f"(a) {mode}: the reply reports the filter with the hidden walls: {R[(mode, 'a1')][0].get('filter')}")
    s0, s1 = sample(mode, 'a0'), sample(mode, 'a1')
    if s0.get('visible') is not False or s1.get('visible') is not True:
        fail(f"(a) {mode}: room A's north wall (1.0, 4.89, 1.2) is hidden by w0 without the cutaway, seen with it: "
             f"{s0.get('visible')}, {s1.get('visible')}")
    if abs(sum(s0['rgb']) - sum(s1['rgb'])) < 30:
        fail(f"(a) {mode}: that pixel changes from w0's sunlit outer face to the shaded wall inside: {s0['rgb']} vs {s1['rgb']}")
    b0, b1 = sample(mode, 'b0'), sample(mode, 'b1')
    if b0.get('visible') is not False or b1.get('visible') is not True or abs(sum(b0['rgb']) - sum(b1['rgb'])) < 30:
        fail(f"(b) {mode}: with the clip box ending at z 2.0 the floor at (3.0, 1.5) shows instead of the ceiling: "
             f"{b0}, {b1}")
    lit, shade = sample(mode, 'b1', 1), sample(mode, 'b1', 2)
    if sum(lit['rgb']) - sum(shade['rgb']) < 60:
        fail(f"(d) {mode}: clipped geometry casts no shadow: the floor at (1.0, 1.5) is sunlit (the cut south wall shades "
             f"only to y 1.255) and brighter than (1.0, 0.6) by >= 60: {lit['rgb']} vs {shade['rgb']}")
    im0, im1 = img(R[(mode, 'c0')][1]), img(R[(mode, 'c1')][1])
    c0, c1 = at(im0, 100, 75), at(im1, 100, 75)
    if abs(c0[0] - 255) + abs(c0[1]) + abs(c0[2] - 255) < 30 or any(abs(a - b) > 2 for a, b in zip(c1, (255, 0, 255))):
        fail(f"(c) {mode}: nearClip 1.0 removes w0 0.6..0.8 m ahead, leaving the backdrop: centre {c0} then {c1}")
    v0, v1, v2 = sample(mode, 'c0').get('visible'), sample(mode, 'c1').get('visible'), sample(mode, 'c2').get('visible')
    if v0 is not True or v1 is not False or v2 is not True:
        fail(f"(c) {mode}: (1.0, 0.11, 1.5) is seen without the near clip and not with it; through the window camera's "
             f"near clip the north wall inside is seen: {v0}, {v1}, {v2}")
    print(f"{mode}: cutaway, clip box (and its shadows) and near clip honoured")
print("PASS 15")
