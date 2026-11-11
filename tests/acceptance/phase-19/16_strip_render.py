# Strip lights rendered: in the raster views a strip counts as one light against the budget (sampled as points 0.15 m
# apart, never a shadow map); in the path tracer it is an emissive strip. The 2 m, 500 lm/m strip of check 05 in the
# dark 4 x 4 room: path-traced illuminance under its centre is the brief's 49.72 lx +- 2 %; it lights the floor under it
# in both renderers, and its lens reads near white from below.
# needs: p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-strip-model-1, p19-strip-model-2, p19-strip-render
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
WANT = 500 / math.pi * (1 / (2.4 ** 2 + 1) + math.atan(1 / 2.4) / 2.4)
STRIP = cmd('lightfixture', {"type": "LED strip", "path": [[1, 2], [3, 2]], "height": 2.4, "lumensPerMetre": 500, "facing": "down"})
OFF = cmd('lightfixture.edit', {"id": "lf0", "on": False})
rows = run_ok(ROOM + [STRIP, cmd('illuminance', {"point": [2, 2, 0], "method": "path", "bounces": 0, "spp": 1024})],
              "path illuminance of a strip")
r = lux_of(rows)
print(f"path illuminance under the strip: {r['lux']:.3f} lx (want {WANT:.2f})")
within(num(r['lux'], 'lux'), WANT, 0.02, "(a) path-traced illuminance under the strip's centre")

CAM = down_cam(2, 2.9, 2.2, 70)
LOOK = {"sun": False, "exposure": {"ev100": 4}, "toneMap": "none", "samples": [[2, 2, 0], [2, 3.8, 0]]}
PT = {"final": "path", "spp": 16, "bounces": 1, "seed": 0}
UPCAM = {"kind": "full", "eye": [2, 2, 1.0], "target": [2, 2.001, 3.0], "fov": 30}
o = lambda n: os.path.join(tmp, n)
rows, rs = rendered(ROOM + [STRIP, render_cmd(o('on.png'), CAM, (120, 90), **LOOK),
                            render_cmd(o('on-path.png'), CAM, (120, 90), **LOOK, **PT),
                            render_cmd(o('up.png'), UPCAM, (120, 90), sun=False, exposure={"ev100": 4}, toneMap="none", **PT),
                            OFF, render_cmd(o('off.png'), CAM, (120, 90), **LOOK),
                            render_cmd(o('off-path.png'), CAM, (120, 90), **LOOK, **PT)], "strip renders")
on, onp, up, off, offp = rs
for a, b, what in ((on, off, "raster"), (onp, offp, "path")):
    s_on, s_far, s_off = rgb_sum(a['samples'][0], what), rgb_sum(a['samples'][1], what), rgb_sum(b['samples'][0], what)
    print(f"{what}: floor under the strip {s_on:.0f} on, {s_off:.0f} off; 1.8 m away {s_far:.0f} (r+g+b)")
    if s_on - s_off < 60 or s_on <= s_far:
        fail(f"(b) {what}: the strip lights the floor under it (>= 60 brighter than with it off, and brighter than "
             f"1.8 m away): {s_on}, off {s_off}, away {s_far}")
if on.get('drawnLights') != 1 or on.get('shadowedLights') != 0:
    fail(f"(b) the raster draws the strip as one light, with no shadow map: drawnLights {on.get('drawnLights')}, "
         f"shadowedLights {on.get('shadowedLights')}")
w, h, p = img(o('up.png'))
white = sum(1 for c in p if sum(c) >= 700)
if white < 10:
    fail(f"(c) from below, the strip's lens reads near white in the path tracer (>= 10 pixels with r+g+b >= 700): {white}")
rows, rs = rendered(ROOM + [cmd('lightfixture', {"type": "LED strip", "path": [[0.5, 0.5], [3.5, 0.5], [3.5, 1.5]],
                                                 "height": 2.4, "lumensPerMetre": 500}),
                            render_cmd(o('l.png'), CAM, (60, 45), sun=False)], "an L-shaped strip")
if rs[-1].get('drawnLights') != 1:
    fail(f"(b) an L-shaped 4 m strip is still one light against the budget: drawnLights {rs[-1].get('drawnLights')}")
print("PASS 16")
