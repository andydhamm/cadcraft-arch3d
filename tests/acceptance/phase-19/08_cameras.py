# Cameras keep their look: a saved camera stores its technique, sun angle, backdrop, exposure, light set, cutaway and
# clip box. Brief numbers: a saved camera re-rendered after changing the global exposure, sun and light set gives the
# same image as before (fixed seed), and `camera.reset` after an orbit gives the saved image again. Also: the path
# technique renders path-traced without `final`, clay/technical techniques render as those styles, a camera keeps its
# own picture backdrop, and the floor overview camera kind (the current level and the ones below, no ceilings, lower
# levels optional). HOUSE1 (the 8 x 5 two-room house) with two fixtures per room.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-sun-1, p19-sun-2, p19-seeing-in, p19-seeing-in-render-1, p19-seeing-in-render-2, p19-cameras-1, p19-cameras-2, p19-cameras-3, p19-cameras-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (160, 120)
SET = HOUSE1 + [SITE, cmd('window', {"wall": "w0", "offset": 2.0, "width": 1.2, "sill": 0.9, "height": 1.2}),
                cmd('window', {"wall": "w1", "offset": 2.0, "width": 1.2, "sill": 0.9, "height": 1.2}),
                cmd('lightfixture', {"room": "r0", "rows": 1, "cols": 2}), cmd('lightfixture', {"room": "r1", "rows": 1, "cols": 2}),
                cmd('lightset', {"name": "Evening", "lights": {f"lf{i}": {"on": True} for i in range(4)}}),
                cmd('lightset', {"name": "Day", "lights": {f"lf{i}": {"on": False} for i in range(4)}}),
                cmd('sunangle', dict({"name": "noon"}, **NOON)), cmd('sunangle', {"name": "evening", "azimuth": 300, "elevation": 8})]
POSE = {"kind": "full", "eye": [-6, -6], "eyeHeight": 1.6, "target": [4, 2.5, 1.2], "fov": 50}
INLINE = {"kind": "full", "eye": [-6, -6, 1.6], "target": [4, 2.5, 1.2], "fov": 50}
GLOBAL = [cmd('look', {"exposure": {"ev100": 12}, "sun": {"angle": "noon"}, "lightSet": "Day"}),
          cmd('backdrop', {"kind": "colour", "colour": "#204060"})]
PT = {"spp": 4, "seed": 7, "bounces": 1}
n = [0]


def shot(camera, **k):
    n[0] += 1
    o = os.path.join(tmp, f's{n[0]}.png')
    return o, render_cmd(o, camera, SIZE, **k)


def cams(rows):
    rs = [r for r in rows if isinstance(r, dict) and isinstance(r.get('cameras'), list) and 'activeCamera' in r]
    if not rs:
        fail(f"arch.inspect {{cameras: true}} lists the cameras and the active one: {json.dumps(rows)[-400:]}")
    return {c.get('name'): c for c in rs[-1]['cameras'] if isinstance(c, dict)}


# (a)-(c), (e): looks kept through global changes, orbit and reset, techniques (the inline clay and technical renders
# run before the global changes, so they share the cameras' captured look but for the explicit style).
a1, ra1 = shot("dusk")
p1, rp1 = shot("pathcam", **PT)
c1, rc1 = shot("claycam")
t1, rt1 = shot("techcam")
a2, ra2 = shot("dusk")
p2, rp2 = shot("pathcam", **PT)
inl, rinl = shot(INLINE)
orb, rorb = shot("dusk")
rst, rrst = shot("dusk")
cl, rcl = shot(INLINE, style="clay", exposure={"ev100": 6}, sun={"angle": "evening"}, lightSet="Evening")
tl, rtl = shot(INLINE, style="technical", exposure={"ev100": 6}, sun={"angle": "evening"}, lightSet="Evening")
LOOK = {"exposure": {"ev100": 6}, "sun": {"angle": "evening"}, "lightSet": "Evening"}
rows, rs = rendered(SET + [
    cmd('camera', dict(POSE, save="dusk", technique="standard", **LOOK)),
    cmd('camera', dict(POSE, save="pathcam", technique="path", **LOOK)),
    cmd('camera', dict(POSE, save="claycam", technique="clay", **LOOK)),
    cmd('camera', dict(POSE, save="techcam", technique="technical", **LOOK)),
    ra1, rp1, rc1, rt1, rcl, rtl] + GLOBAL + [ra2, rp2, rinl,
    cmd('camera.orbit', {"name": "dusk", "azimuth": 30}), cmd('arch.inspect', {"cameras": True}), rorb,
    cmd('camera.reset', {"name": "dusk"}), cmd('arch.inspect', {"cameras": True}), rrst], "camera looks")
if len(rs) != 11:
    fail(f"eleven render replies: {len(rs)}")
A = digest(a1)
if digest(a2) != A:
    fail("(a) the saved camera `dusk` renders the same image after the global exposure, sun, light set and backdrop change")
if digest(inl) == A:
    fail("(a) the same pose inline (the global look) renders differently from the camera with its own look")
if rs[1].get('finalMode') != 'path' or rs[7].get('finalMode') != 'path' or digest(p1) != digest(p2):
    fail(f"(b) the path technique renders path-traced without `final` and keeps its image (seed 7): "
         f"{rs[1].get('finalMode')}, {rs[7].get('finalMode')}, same {digest(p1) == digest(p2)}")
if digest(orb) == A:
    fail("(c) camera.orbit {azimuth 30} changes what `dusk` shows")
if digest(rst) != A:
    fail("(c) camera.reset returns `dusk` to its saved state: the saved image again")
mods = [cams(rows[:i + 1]) for i, r in enumerate(rows) if isinstance(r, dict) and isinstance(r.get('cameras'), list)]
if len(mods) != 2 or mods[0].get('dusk', {}).get('modified') is not True or mods[1].get('dusk', {}).get('modified') is not False:
    fail(f"(c) arch.inspect cameras: dusk modified true after the orbit, false after the reset: "
         f"{[m.get('dusk', {}).get('modified') for m in mods]}")
if digest(c1) != digest(cl) or digest(t1) != digest(tl):
    fail("(e) the clay and technical techniques render as `style: clay` and `style: technical` with the camera's look")
print("ok: (a) a camera keeps its look, (b) path technique, (c) orbit and reset, (e) clay and technical")

# (f) a camera keeps its own picture backdrop.
SKY = {"kind": "full", "eye": [-6, -6], "eyeHeight": 1.6, "target": [4, 2.5, 3.0], "fov": 60}
s1, rs1 = shot("skycam")
s2, rs2 = shot("skycam", final="path", **PT)
s3, rs3 = shot("skycam")
s4, rs4 = shot("skycam", final="path", **PT)
s5, rs5 = shot({"kind": "full", "eye": [-6, -6, 1.6], "target": [4, 2.5, 3.0], "fov": 60})
rendered(SET + [cmd('backdrop', {"kind": "image", "item": "own:backdrop-sky", "projection": "equirect"}),
                cmd('camera', dict(SKY, save="skycam")), rs1, rs2,
                cmd('backdrop', {"kind": "colour", "colour": "#204060"}), rs3, rs4, rs5], "a camera with a picture backdrop")
if digest(s1) != digest(s3) or digest(s2) != digest(s4):
    fail("(f) `skycam` keeps its picture backdrop after the global backdrop becomes a colour (raster and path)")
if digest(s5) == digest(s1):
    fail("(f) the same pose inline shows the global colour backdrop, not the camera's picture")
print("ok: (f) a camera keeps its picture backdrop")

# (d) floor overview: level 0 with the cutaway, level 1 with and without the levels below.
UP = [cmd('level', {"above": 0, "name": "Upper"}),
      cmd('wall', {"points": [[0, 0], [4, 0], [4, 5], [0, 5], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True,
                   "level": "Upper"}),
      cmd('roomdetect', {"level": "Upper"}), cmd('roofauto', {"pitchRise": 6, "pitchRun": 12, "overhang": 0.5, "level": "Upper"})]
OV = {"azimuth": 0, "elevation": 89, "target": [4, 2.5, 0], "distance": 30}
P = [6.5, 1.5, 0]
rows = run_ok(HOUSE1 + UP + [cmd('camera', dict(OV, save="ov0", kind="overview", level=0, cutaway=True)),
                             cmd('camera', dict(OV, save="ov1", kind="overview", level=1, lowerLevels=True)),
                             cmd('camera', dict(OV, save="ov1x", kind="overview", level=1, lowerLevels=False)),
                             render_cmd(os.path.join(tmp, 'ov0.png'), "ov0", SIZE, samples=[P]),
                             cmd('arch.inspect', {"rooms": True})], "overview cameras")
r = nth_render(rows, -1, "overview render")
px = r['samples'][0].get('pixel') if r.get('samples') else None
if not (isinstance(px, list) and len(px) == 2):
    fail(f"(d) the overview render places P = (6.5, 1.5, 0) on a pixel: {r.get('samples')}")
room_b = room_at(inspected(rows), 6.5, 1.5)
rows = run_ok(HOUSE1 + UP + [cmd('camera', dict(OV, save="ov0", kind="overview", level=0, cutaway=True)),
                             cmd('camera', dict(OV, save="ov1", kind="overview", level=1, lowerLevels=True)),
                             cmd('camera', dict(OV, save="ov1x", kind="overview", level=1, lowerLevels=False))] +
              [cmd('pick3d', {"camera": c, "size": list(SIZE), "pixel": px}) for c in ("ov0", "ov1", "ov1x")], "overview picks")
h = [p['hit'] for p in rows if isinstance(p, dict) and 'hit' in p]
if len(h) != 3:
    fail(f"three picks: {h}")
for i, name in ((0, "ov0 (level 0, cutaway)"), (1, "ov1 (level 1 and below)")):
    if not isinstance(h[i], dict) or h[i].get('part') != 'floor' or h[i].get('id') != room_b:
        fail(f"(d) {name}: the pick at P's pixel is room B's floor ({room_b}), no roof or ceiling: {h[i]}")
if h[2] is not None:
    fail(f"(d) ov1 without the lower levels shows nothing of level 0 at P: {h[2]}")
print("PASS 08")
