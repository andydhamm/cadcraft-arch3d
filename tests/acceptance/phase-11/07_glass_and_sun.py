# Daylight through glass: the noon sun through a south window lights a patch of floor (shadow rays pass the glass,
# tinted); the floor away from it gets bounced daylight, so its illuminance rises with bounces.
# Where the patch is (fix cycle 1, from geometry): at solar time 12:00 on 2026-06-21 at latitude 51.05 the sun is due
# south at 62.4 degrees (tan 1.913). The window on w0 (the south wall, inner face y = 0, outer face y = -0.2) is 1.6 m
# wide centred on x = 2.0, glass from z 0.9 to 2.1. A floor point (x, 0.0) sees the sun through the opening when the ray
# clears the sill at the inner face (1.913 y >= 0.9) and the head at the outer face (1.913 (y + 0.2) <= 2.1): y from
# 0.47 to 0.90, less the frame. The samples sit on the band's middle line y = 0.68, inside the window's width (the old
# point (2.0, 0.85) was on the band's far edge, in the frame's shadow).
# Bounced light is measured as illuminance: at the daylight exposure (pi / 100 000 lx) the floor away from the patch is
# a few levels of an 8-bit pixel, so a pixel cannot show it. Expected from the room (split flux, albedo ~0.5): tens to
# hundreds of lux from bounces; the bar is +20 lx over bounces 0.
# The patch's illuminance with bounces 0 is the sun through one pane: 100 000 lx x sin 62.4 deg = 88 600 lx on the
# floor, times the pane's tint once (glass albedo (0.55, 0.70, 0.80) x transmission 0.65: luminance 0.44) and Fresnel
# at both faces at 62.4 deg incidence (Schlick, F0 0.04: (1 - 0.083)^2 = 0.84): about 32 700 lx. The bar is 25 000 to
# 88 600 lx. A pane tinted at each of its two faces gives about 17 300 lx (fix cycle 1 measured 17 260).
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path, p11f-glass-shadow-once
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
SUN = {"date": "2026-06-21", "time": "12:00"}
CAM = {"kind": "full", "eye": [3.6, 3.6, 1.6], "target": [1.8, 1.0, 0.3], "fov": 80}
PATCH = [[1.6, 0.68, 0.0], [2.0, 0.68, 0.0], [2.4, 0.68, 0.0]]
SHADE = [1.0, 3.2, 0.0]
setup = room_cmds() + [cmd('window', {"wall": "w0", "offset": 2.1, "width": 1.6, "sill": 0.9, "height": 1.2}),
                       cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 0})]
LUX = lambda **k: cmd('illuminance', dict({"point": SHADE, "normal": [0, 0, 1], "method": "path", "sun": SUN}, **k))
rc, rr, so, se = run(setup + [cmd('render', {"out": os.path.join(tmp, 'day.png'), "final": "path", "size": [120, 90], "camera": CAM,
                                             "sun": SUN, "spp": 64, "samples": PATCH + [SHADE]}),
                              LUX(bounces=0), LUX(bounces=4),
                              cmd('illuminance', {"point": PATCH[1], "normal": [0, 0, 1], "method": "path", "sun": SUN, "bounces": 0})])
r = path_renders(rr)
if len(r) != 1:
    fail(f"one path-traced render reply, got {len(r)}: {json.dumps(rr)[-800:]}")
patch = [rgb_sum(s, f'sun patch {i}') for i, s in enumerate(r[0]['samples'][:3])]
shade = rgb_sum(r[0]['samples'][3], 'shaded floor')
flat, bounced = num(lux_of(rr, -3).get('lux'), 'lux, bounces 0'), num(lux_of(rr, -2).get('lux'), 'lux, bounces 4')
sunlit = num(lux_of(rr, -1).get('lux'), 'patch lux, bounces 0')
print(f"floor (r+g+b): in the sun patch {[round(p) for p in patch]}, away from it {shade:.0f}; "
      f"illuminance away from it: bounces 0 {flat:.1f} lx, bounces 4 {bounced:.1f} lx; in the patch (bounces 0) {sunlit:.0f} lx")
if not (25000 <= sunlit <= 88600):
    fail(f"the sun through one pane lights the patch with 25 000-88 600 lx (expected ~32 700: the tint once, Fresnel at "
         f"62.4 deg; a pane tinted at both faces gives ~17 300): {sunlit:.0f} lx")
if min(patch) < shade + 100:
    fail(f"the sun through the glass lights a patch far brighter (by 100+) than the floor away from it: {patch} vs {shade}")
if bounced < flat + 20:
    fail(f"bounced daylight brightens the floor away from the patch (+20 lx): bounces 4 {bounced} lx, bounces 0 {flat} lx")
artifact(os.path.join(tmp, 'day.png'), 'daylit-room-path.png')
print("PASS 07")
