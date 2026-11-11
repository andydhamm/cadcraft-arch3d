# A sky that follows the sun (`backdrop {kind: "sky"}`, Preetham clear sky, turbidity 3), in the interactive view, the
# raster Finalize and the path tracer, filling the background and lighting the scene. Brief numbers: at solar noon sky
# pixels have blue > red; at sun elevation 2 degrees the sky toward the sun has red > blue; at -10 degrees the brightest
# sky pixel is under 1 % of the noon sky's. Also: twilight fades between 0 and -6 degrees (-3 lies strictly between),
# the moon is a dim directional light (0.25 lx, its own position) only at night, optional stars, and a moonlit night is
# metered like any other night (exposure 0.18 / (0.1/pi * 0.25) = 22.6 for albedo 0.1), while the legacy
# `render {sun: false}` keeps 0.01.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-r3d-groundwork-1, p19-r3d-groundwork-2, p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-sun-1, p19-sun-2, p19-env-path, p19-env-raster, p19-sky-1, p19-sky-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
WALL = cmd('wall', {"points": [[2, -3], [3, -3]], "thickness": 0.2, "height": 1.0, "exterior": False})   # behind the cameras
ANGLES = [cmd('sunangle', dict({"name": "noon"}, **NOON)), cmd('sunangle', {"name": "low", "azimuth": 270, "elevation": 2}),
          cmd('sunangle', {"name": "m3", "azimuth": 270, "elevation": -3}),
          cmd('sunangle', {"name": "m10", "azimuth": 270, "elevation": -10}),
          cmd('sunangle', {"name": "deep", "azimuth": 0, "elevation": -20})]
SET = [SITE, WALL] + ANGLES + [cmd('backdrop', {"kind": "sky"})]
UP = level_cam([0, 0, 1.6], 90, 70, 30)          # north, pitched up 70 degrees
WEST = level_cam([0, 0, 1.6], 180, 4, 60)        # west (toward the low sun), pitched up 4 degrees
SIZE = (120, 90)
PATH4 = {"final": "path", "spp": 4, "bounces": 0, "seed": 0, "denoise": False}
M3 = (('interactive', {}), ('raster', {"final": "raster"}), ('path', PATH4))


def shots(tag, cam, angle, ev, **k):
    out = []
    for mode, extra in M3:
        o = os.path.join(tmp, f'{tag}-{mode}.png')
        out.append((mode, o, render_cmd(o, cam, SIZE, sun={"angle": angle}, exposure={"ev100": ev}, toneMap="none",
                                        **k, **extra)))
    return out


def brightest(o):
    w, h, p = img(o)
    return max(srgb8_to_linear(c[k]) for c in p for k in range(3))


plan = {"noon": shots('noon', UP, "noon", 15), "low": shots('low', WEST, "low", 15), "m10": shots('m10', UP, "m10", 15),
        "w2": shots('w2', WEST, "low", 8), "w3": shots('w3', WEST, "m3", 8), "w10": shots('w10', WEST, "m10", 8),
        "stars": shots('stars', level_cam([0, 0, 1.6], 90, 89, 60), "deep", -6, sky={"stars": True}),
        "nostars": shots('nostars', level_cam([0, 0, 1.6], 90, 89, 60), "deep", -6, sky={"stars": False})}
rows, rs = rendered(SET + [c for v in plan.values() for (_, _, c) in v], "sky renders")
if len(rs) != sum(len(v) for v in plan.values()):
    fail(f"one reply per sky render: {len(rs)}")
for mode_i, (mode, _) in enumerate(M3):
    o = plan['noon'][mode_i][1]
    w, h, p = img(o)
    bad = [c for c in p if not c[2] > c[0]]
    clear = [c for c in p if all(abs(c[k] - CLEAR[k]) <= 2 for k in range(3))]
    if bad or clear:
        fail(f"(a) {mode}: at solar noon every sky pixel has blue > red and none is the clear colour: "
             f"{len(bad)} not blue, {len(clear)} clear (e.g. {(bad + clear)[:3]})")
    w, h, p = img(plan['low'][mode_i][1])
    hz = lambda deg: round(h / 2 - math.tan(math.radians(deg - 4)) / math.tan(math.radians(30)) * h / 2)
    m = box_mean((w, h, p), w // 3, hz(10), 2 * w // 3, hz(0.5))
    if not m[0] > m[2]:
        fail(f"(b) {mode}: with the sun 2 degrees up, the sky toward it (within 10 degrees of the horizon) is redder "
             f"than blue: mean linear {m}")
    noon_b, m10_b = brightest(plan['noon'][mode_i][1]), brightest(plan['m10'][mode_i][1])
    if not m10_b < 0.01 * noon_b:
        fail(f"(c) {mode}: at -10 degrees the brightest sky pixel is under 1 % of noon's: {m10_b} vs {noon_b}")
    b2, b3, b10 = (brightest(plan[t][mode_i][1]) for t in ('w2', 'w3', 'w10'))
    if not b10 < b3 < b2:
        fail(f"(d) {mode}: twilight fades: the brightest pixel at -3 degrees lies strictly between -10 and 2: "
             f"{b10}, {b3}, {b2}")
    _, _, off = img(plan['nostars'][mode_i][1])
    top = max(sum(c) for c in off)
    _, _, on = img(plan['stars'][mode_i][1])
    n = sum(1 for c in on if sum(c) > top)
    if n < 20:
        fail(f"(f) {mode}: stars on at -20 degrees give >= 20 pixels brighter than every pixel without them: {n}")
    print(f"{mode}: noon blue, low sun red toward it ({m[0]:.4f} > {m[2]:.4f}), -10 at {m10_b:.2g} of noon {noon_b:.3f}, "
          f"twilight {b10:.3g} < {b3:.3g} < {b2:.3g}, {n} star pixels")

# (e) the moon: 0.25 lx from its own direction, only with the sun down.
P = [5, 5, 0]
rows = run_ok([SITE] + ANGLES + [
    cmd('illuminance', {"point": P, "sun": {"angle": "deep"}, "sky": {"moon": {"azimuth": 180, "elevation": 30}}}),
    cmd('illuminance', {"point": P, "sun": {"angle": "deep"}, "sky": {"moon": {"lux": 0, "azimuth": 180, "elevation": 30}}}),
    cmd('illuminance', {"point": P, "sun": {"angle": "noon"}, "sky": {"moon": {"azimuth": 180, "elevation": 30}}})],
    "moon illuminance")
lx = [r for r in rows if isinstance(r, dict) and 'lux' in r and 'direct' in r]
if len(lx) != 3 or any('moon' not in r for r in lx):
    fail(f"three direct illuminance replies with a 'moon' term: {lx}")
within(num(lx[0]['moon'], 'moon'), 0.125, 0.01, "(e) the moon at elevation 30 gives 0.25 sin 30 = 0.125 lx")
if abs(num(lx[1]['moon'], 'moon')) > 1e-9 or abs(num(lx[2]['moon'], 'moon')) > 1e-9:
    fail(f"(e) a 0 lx moon, and the moon while the sun is up, give 0: {lx[1]['moon']}, {lx[2]['moon']}")

# (g) a moonlit night is metered toward 0.18; the legacy night keeps 0.01.
DOWN = down_cam(2.5, -4.0, 0.8, 30)
cmds = []
for mode, extra in MODES:
    cmds.append(render_cmd(os.path.join(tmp, f'g-{mode}.png'), DOWN, (64, 48), ground=True, albedo=0.1, exposure="auto",
                           toneMap="none", sun={"angle": "deep"}, sky={"moon": {"azimuth": 180, "elevation": 90}}, **extra))
cmds.append(render_cmd(os.path.join(tmp, 'g-legacy.png'), DOWN, (64, 48), ground=True, sun=False, exposure="auto"))
rows, rs = rendered([SITE, WALL] + ANGLES + cmds, "moonlit renders")
want = 0.18 / (0.1 / math.pi * 0.25)
for r, (mode, _) in zip(rs, MODES):
    if r.get('exposureMode') != 'auto':
        fail(f"(g) {mode}: exposureMode auto: {r.get('exposureMode')}")
    within(num(r.get('exposure'), 'exposure'), want, 0.03, f"(g) {mode}: the moonlit ground is metered (exposure 22.6)")
    within(lum(centre_mean(os.path.join(tmp, f'g-{mode}.png'))), 0.18, 0.03, f"(g) {mode}: the moonlit ground reads 0.18")
near(num(rs[-1].get('exposure'), 'legacy exposure'), 0.01, 1e-9, "(g) render {sun: false, exposure: auto} keeps 0.01")
print("PASS 03")
