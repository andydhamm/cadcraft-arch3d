# Exposure: a pixel's linear value is the scene luminance x 1 / (1.2 * 2^EV100), then the tone map (ACES default, Hable,
# none), the same in the interactive view, the raster Finalize and the path tracer. (a) the brief's numbers: phase 11's
# white furnace (radiance 2.0), tone mapping off: EV100 1 -> 0.8333 (sRGB 235), EV100 2 -> 0.4167 (173), +-2 %. (b) a
# physically scaled raster: open ground of albedo 0.5 (the `albedo` calibration param) under the sun straight down at
# 100 000 lx, sky off, reads L = 0.5/pi * 100 000 = 15 915.5 nits in all three renderers: EV100 14 -> 0.8095, EV100 15
# -> 0.4048, and through ACES and Hable at EV100 14. (c) auto meters the frame toward middle grey 0.18 (albedo 0.05 and
# 0.1 give exposure 1.131e-4 and 5.655e-5). (d) the legacy `sun: false` night keeps 0.01 (EV100 6.3808, phase-12
# check 14). (e) any other night is metered: `sun {on: false}` with sky off, the raster's stand-in night ambient
# (6.2832 lx) gives exposure 0.9; the path tracer sees nothing lit and stops at the night limit, EV100 -6 = 53.333.
# The path renders of (b), (c) and (e) are pinned at bounces 0: the numbers assume the sun alone.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-r3d-groundwork-1, p19-r3d-groundwork-2, p19-exposure-1, p19-exposure-2, p19-exposure-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
L = 0.5 / math.pi * 100000.0


def check_reply(r, mode, what):
    if r.get('exposureMode') != mode:
        fail(f"{what}: exposureMode {mode!r}: {r.get('exposureMode')!r}")
    for k in ('exposure', 'ev100', 'toneMap'):
        if k not in r:
            fail(f"{what}: the reply has no {k!r}: {json.dumps(r)[:400]}")


# (a) the furnace, EV100 1 and 2, tone map none: every pixel.
FCAM = {"kind": "full", "eye": [1.0, 1.0, 1.2], "target": [3.0, 3.0, 1.0], "fov": 100}
outs = [os.path.join(tmp, f'furnace{ev}.png') for ev in (1, 2)]
rows, rs = rendered(ROOM + [render_cmd(o, FCAM, (12, 9), final="path", spp=4096, denoise=False, toneMap="none",
                                       furnace={"albedo": 0.5, "emission": 1.0}, exposure={"ev100": ev})
                            for o, ev in zip(outs, (1, 2))], "furnace renders")
for r, o, ev, want, s8 in zip(rs, outs, (1, 2), (0.8333, 0.4167), (235, 173)):
    check_reply(r, 'manual', f"furnace EV100 {ev}")
    near(num(r['ev100'], 'ev100'), ev, 1e-9, f"furnace EV100 {ev} echoed")
    w, h, p = img(o)
    vals = [srgb8_to_linear(c[k]) for c in p for k in range(3)]
    lo, hi = min(vals), max(vals)
    print(f"furnace EV100 {ev}: linear {lo:.4f}..{hi:.4f} (want {want} +- 2 %, sRGB {s8})")
    if not (want * 0.98 <= lo and hi <= want * 1.02):
        fail(f"furnace EV100 {ev}: every pixel is {want} +- 2 % (sRGB {s8}): {lo:.4f}..{hi:.4f}")

# (b) open ground, albedo 0.5, the sun straight down, sky off: three renderers, EV100 14 and 15, none / ACES / Hable.
CAM = down_cam(3.0, 1.5, 0.8, 30)
SUN = {"azimuth": 0, "elevation": 90}
cases = [(14, "none", L * ev_scale(14)), (15, "none", L * ev_scale(15)), (14, "aces", aces(L * ev_scale(14))),
         (14, "hable", hable(L * ev_scale(14)))]
cmds, keys = [], []
for ev, tm, want in cases:
    for mode, extra in MODES:
        o = os.path.join(tmp, f'b-{ev}-{tm}-{mode}.png')
        cmds.append(render_cmd(o, CAM, (64, 48), ground=True, albedo=0.5, sun=SUN, sky=False, exposure={"ev100": ev},
                               toneMap=tm, **extra))
        keys.append((ev, tm, want, mode, o))
rows, rs = rendered([COLUMN] + cmds, "the open-ground renders")
if len(rs) != len(keys):
    fail(f"{len(keys)} render replies: {len(rs)}")
for r, (ev, tm, want, mode, o) in zip(rs, keys):
    check_reply(r, 'manual', f"{mode} EV100 {ev} {tm}")
    if r.get('toneMap') != tm:
        fail(f"{mode} EV100 {ev}: toneMap {tm!r} echoed: {r.get('toneMap')!r}")
    got = lum(centre_mean(o))
    print(f"ground albedo 0.5 under 100 000 lx, {mode:11s} EV100 {ev} {tm:5s}: {got:.4f} (want {want:.4f} +- 2 %)")
    within(got, want, 0.02, f"{mode} view, EV100 {ev}, tone map {tm}")

# (c) auto exposure meters the sunlit ground toward 0.18.
cmds, keys = [], []
for a in (0.05, 0.1):
    for mode, extra in MODES:
        o = os.path.join(tmp, f'c-{a}-{mode}.png')
        cmds.append(render_cmd(o, CAM, (64, 48), ground=True, albedo=a, sun=SUN, sky=False, exposure="auto",
                               toneMap="none", **extra))
        keys.append((a, mode, o))
rows, rs = rendered([COLUMN] + cmds, "auto-exposed renders")
for r, (a, mode, o) in zip(rs, keys):
    check_reply(r, 'auto', f"auto, albedo {a}, {mode}")
    want_scale = 0.18 / (a / math.pi * 100000.0)
    within(num(r['exposure'], 'exposure'), want_scale, 0.03, f"auto exposure scale, albedo {a}, {mode}")
    got = lum(centre_mean(o))
    print(f"auto, albedo {a}, {mode:11s}: exposure {r['exposure']:.4g}, ground {got:.4f} (want 0.18 +- 3 %)")
    within(got, 0.18, 0.03, f"auto-exposed ground, albedo {a}, {mode}")

# (d) the legacy night keeps 0.01, auto or not; (e) any other night is metered.
cmds, keys = [], []
for mode, extra in (('raster', {"final": "raster"}), ('path', PATH0)):
    for tag, k in (('legacy', {"sun": False}), ('legacy-auto', {"sun": False, "exposure": "auto"}),
                   ('off', {"sun": {"on": False}, "sky": False, "exposure": "auto", "toneMap": "none", "albedo": 0.1})):
        o = os.path.join(tmp, f'n-{tag}-{mode}.png')
        cmds.append(render_cmd(o, CAM, (64, 48), ground=True, **k, **extra))
        keys.append((tag, mode, o))
cmds.append(render_cmd(os.path.join(tmp, 'n-off-interactive.png'), CAM, (64, 48), ground=True, sun={"on": False}, sky=False,
                       exposure="auto", toneMap="none", albedo=0.1))
keys.append(('off', 'interactive', os.path.join(tmp, 'n-off-interactive.png')))
rows, rs = rendered([COLUMN] + cmds, "night renders")
for r, (tag, mode, o) in zip(rs, keys):
    e = num(r.get('exposure'), f'{tag} {mode} exposure')
    if tag.startswith('legacy'):
        near(e, 0.01, 1e-9, f"the legacy `sun: false` night keeps exposure 0.01 ({tag}, {mode})")
        near(num(r.get('ev100'), 'ev100'), 6.3808, 0.001, f"the legacy night's EV100 ({tag}, {mode})")
    elif mode == 'path':
        check_reply(r, 'auto', f"sun off, path")
        within(e, 1.0 / (1.2 * 2 ** -6), 0.001, "an unlit night in the path tracer stops at the night limit EV100 -6")
        near(num(r.get('ev100'), 'ev100'), -6.0, 0.01, "the path tracer's night limit EV100")
    else:
        check_reply(r, 'auto', f"sun off, {mode}")
        within(e, 0.9, 0.03, f"sun off, sky off, {mode}: the stand-in night ambient 6.2832 lx is metered to 0.18")
        within(lum(centre_mean(o)), 0.18, 0.03, f"sun off, {mode}: the ground reads middle grey")
    print(f"night {tag:11s} {mode:11s}: exposure {e:.5g}, EV100 {r.get('ev100')}")
print("PASS 01")
