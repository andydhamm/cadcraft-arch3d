# The sun: intensity in lux on a surface facing it (default 100 000), cast shadows on or off, the sun off, and named
# sun angles stored in the drawing. Brief numbers: at phase 6's solar noon (elevation 62.39) with the sky off, the
# illuminance on open level ground (direct light only) is 100 000 sin 62.39 = 88 612 lx +- 1 %; with cast shadows off
# the phase-10 column no longer darkens the floor behind it in sunlight.
# SKY OFF IN EVERY SUN MEASUREMENT: every illuminance (direct and path) and every render here passes `sky: false` (merged
# from NOSKY), and the renders pin exposure {ev100: 15} and toneMap none. Once p19-sky makes the default sky a Preetham
# environment, path illuminance counts it even at bounces 0 and the column hides part of it, so the brief's "sky off"
# must be explicit on every call.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-r3d-groundwork-1, p19-r3d-groundwork-2, p19-sun-1, p19-sun-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
E_NOON = 100000.0 * math.sin(math.radians(62.39))
P = [5, 5, 0]
PATH = {"method": "path", "bounces": 0, "spp": 256}


def ill(point, sun, **k):
    return cmd('illuminance', dict({"point": point, "sun": sun}, **NOSKY, **k))


def lux_replies(rows):
    return [r for r in rows if isinstance(r, dict) and 'lux' in r and ('method' in r or 'direct' in r)]


# (a) open ground at noon, intensity, a compass sun.
rows = run_ok([SITE, COLUMN, ill(P, NOON), ill(P, dict(NOON, intensity=50000)), ill(P, NOON, **PATH),
               ill(P, dict(NOON, intensity=50000), **PATH), ill(P, {"azimuth": 180, "elevation": 30}),
               ill(P, dict(NOON, on=False))], "sun illuminance")
lx = lux_replies(rows)
if len(lx) != 6:
    fail(f"six illuminance replies: {len(lx)}")
if 'sun' not in lx[0]:
    fail(f"the direct illuminance reply has a 'sun' term: {lx[0]}")
for r, want, what in zip(lx, (E_NOON, E_NOON / 2, E_NOON, E_NOON / 2, 50000.0, 0.0),
                         ("direct, noon", "direct, noon at 50 000 lx", "path, noon", "path, noon at 50 000 lx",
                          "direct, azimuth 180 elevation 30", "direct, sun off")):
    got = num(r.get('lux'), what)
    print(f"{what}: {got:.1f} lx (want {want:.1f})")
    if want == 0.0:
        if abs(got) > 1e-6 or abs(num(r.get('sun'), 'sun')) > 1e-6:
            fail(f"{what}: `sun {{on: false}}` gives no sun: lux {got}, sun {r.get('sun')}")
    else:
        within(got, want, 0.01, what)

# (b) the column's noon shadow lies 1.0 / tan 62.39 = 0.523 m north of it (y 1.6..2.12): direct and path illuminance,
# then raster and path renders, with cast shadows on and off.
S, O = [1.5, 1.8, 0], [3.0, 1.8, 0]
rows = run_ok([SITE, COLUMN, ill(S, NOON), ill(S, dict(NOON, shadows=False)), ill(S, NOON, **PATH),
               ill(S, dict(NOON, shadows=False), **PATH)], "illuminance in the column's shadow")
lx = lux_replies(rows)
for r, shadows, what in zip(lx, (True, False, True, False), ("direct", "direct", "path", "path")):
    got = num(r.get('lux'), what)
    print(f"{what}, behind the column, shadows {shadows}: {got:.1f} lx")
    if shadows and got >= 0.01 * E_NOON:
        fail(f"{what}: the column shades (1.5, 1.8, 0) at noon (< 1 % of {E_NOON:.0f}): {got}")
    if not shadows:
        within(got, E_NOON, 0.01, f"{what} with cast shadows off: the column no longer shades the point")
CAM = down_cam(2.25, 1.8, 3.0, 60)
cmds, keys = [], []
for mode, extra in (('interactive', {}), ('raster', {"final": "raster"}), ('path', PATH0)):
    for shadows in (True, False):
        o = os.path.join(tmp, f'{mode}-{shadows}.png')
        cmds.append(render_cmd(o, CAM, (120, 90), ground=True, sun=dict(NOON, shadows=shadows), exposure={"ev100": 15},
                               toneMap="none", samples=[S, O], **NOSKY, **extra))
        keys.append((mode, shadows))
rows, rs = rendered([SITE, COLUMN] + cmds, "renders of the column's shadow")
for r, (mode, shadows) in zip(rs, keys):
    s, o = rgb_sum(r['samples'][0], f'{mode} shadow sample'), rgb_sum(r['samples'][1], f'{mode} open sample')
    print(f"{mode}, shadows {shadows}: behind the column {s:.0f}, open ground {o:.0f} (r+g+b)")
    if shadows and o - s < 45:
        fail(f"{mode}: with cast shadows the column darkens the ground behind it by >= 45 (r+g+b): {s} vs {o}")
    if not shadows and abs(o - s) > 6:
        fail(f"{mode}: with cast shadows off the ground behind the column reads like open ground (within 6): {s} vs {o}")

# (d) named sun angles: list, measure, use in the look, refuse deleting one in use.
rows = run_ok([SITE, COLUMN, cmd('sunangle', dict({"name": "noon"}, **NOON)),
               cmd('sunangle', {"name": "low", "azimuth": 180, "elevation": 30}), cmd('sunangle', {"list": True}),
               ill(P, {"angle": "low"}), cmd('look', {"sun": {"angle": "noon"}}),
               render_cmd(os.path.join(tmp, 'angle.png'), CAM, (32, 24))], "sun angles")
lists = [r['sunAngles'] for r in rows if isinstance(r, dict) and isinstance(r.get('sunAngles'), list)]
if not lists:
    fail(f"`sunangle {{list: true}}` replies sunAngles: {json.dumps(rows)[-500:]}")
by = {a.get('name'): a for a in lists[-1] if isinstance(a, dict)}
if set(by) != {"noon", "low"}:
    fail(f"two sun angles, noon and low: {sorted(by)}")
near(num(by['noon'].get('sunElevation'), 'noon elevation'), 62.39, 0.1, "the noon angle's sun elevation")
near(num(by['low'].get('sunElevation'), 'low elevation'), 30.0, 1e-6, "the low angle's sun elevation")
within(num(lux_replies(rows)[-1].get('lux'), 'lux'), 50000.0, 0.01, "illuminance under the named angle `low`")
r = nth_render(rows, -1, "render under the look's sun angle")
nd, sd = by['noon'].get('dir'), r.get('sunDir')
if not (isinstance(nd, list) and isinstance(sd, list) and len(sd) == 3 and all(abs(num(a, 'dir') - num(b, 'dir')) <= 1e-9 for a, b in zip(sd, nd))):
    fail(f"a render without `sun` uses the look's sun angle `noon`: sunDir {sd}, angle dir {nd}")
rc, rr, so, se = run([SITE, cmd('sunangle', dict({"name": "noon"}, **NOON)), cmd('look', {"sun": {"angle": "noon"}}),
                      cmd('sunangle', {"delete": "noon"}), cmd('sunangle', {"list": True}),
                      cmd('look', {"sun": {"angle": None}}), cmd('sunangle', {"delete": "noon"}),
                      cmd('sunangle', {"list": True})], expect_ok=False)
lists = [r['sunAngles'] for r in rr if isinstance(r, dict) and isinstance(r.get('sunAngles'), list)]
if not errors(se) or len(lists) < 2 or [a.get('name') for a in lists[-2]] != ['noon'] or lists[-1] != []:
    fail(f"deleting a sun angle the look uses is refused; after look {{sun: {{angle: null}}}} it is deleted: "
         f"{errors(se)}, {lists}")
print("PASS 02")
