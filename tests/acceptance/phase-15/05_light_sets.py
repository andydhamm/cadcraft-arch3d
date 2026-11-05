#!/usr/bin/env python3
# Light sets: named sets of lights, each on or off with a brightness; chosen per camera (camera {"save", "lightSet"}) and per
# render (render {"lightSet"}). A night render with a set that turns half the lights off and dims one is darker than with
# all on; a camera's set is what a render through that camera uses.
# needs: p15-doc-electrical, p15-arch3d-light-brightness, p15-cmd-light-sets, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
BASE = ROOM64 + [cmd('lightfixture', {"room": "r0", "rows": 2, "cols": 2}),   # lf0..lf3
                 cmd('lightset', {"name": "Evening", "lights": {"lf0": {"on": True, "brightness": 1.0}, "lf1": {"on": False},
                                                                "lf2": {"on": True, "brightness": 0.5}, "lf3": {"on": False}}}),
                 cmd('lightset', {"name": "All", "lights": {k: {"on": True, "brightness": 1.0} for k in ("lf0", "lf1", "lf2", "lf3")}}),
                 cmd('camera', {"save": "inside", "kind": "full", "eye": [0.6, 0.6], "eyeHeight": 1.6, "target": [5.0, 3.5, 1.0],
                                "fov": 70, "lightSet": "Evening"})]
ev, al, cam = (os.path.join(tmp, n) for n in ('evening.png', 'all.png', 'camera.png'))
R = lambda out, **k: cmd('render', dict({"out": out, "size": [200, 150], "sun": False, "camera": "inside"}, **k))
rc, rr, so, se = run(BASE + [R(ev, lightSet="Evening"), R(al, lightSet="All"), R(cam), cmd('lightset', {"list": True}), INSPECT15])
sets = reply_with(rr, 'lightSets', -1, 'lightset list')['lightSets']
if sorted(s['name'] for s in sets) != ['All', 'Evening']:
    fail(f"lightset list: {sets}")
le, la = mean_luma(ev), mean_luma(al)
if not la > le * 1.3:
    fail(f"half the lights off and one dimmed is darker: evening {le:.1f}, all on {la:.1f}")
if digest(cam) != digest(ev):
    fail("a render through camera 'inside' uses its light set (Evening): the images differ")
lights = {l['id']: l for l in lights_of(rr)}
if [lights[k]['on'] for k in ('lf0', 'lf1', 'lf2', 'lf3')] != [True, True, True, True]:
    fail(f"a light set changes nothing in the model; it applies to the render only: {lights}")
for p, n in ((ev, 'lightset-evening.png'), (al, 'lightset-all.png')):
    artifact(p, n)
print(f"ok: Evening {le:.1f} vs All {la:.1f}; the camera's set applies; the model's lights are unchanged")
for bad, why in ((cmd('lightset', {"name": "X", "lights": {"lf9": {"on": True}}}), "a missing light"),
                 (cmd('lightset', {"name": "X", "lights": {"lf0": {"on": True, "brightness": 2}}}), "a brightness above 1"),
                 (cmd('lightset', {"name": "", "lights": {}}), "an empty name"),
                 (R(os.path.join(tmp, 'x.png'), lightSet="Nope"), "an unknown light set"),
                 (cmd('camera', {"save": "c2", "lightSet": "Nope"}), "a camera with an unknown set")):
    rc, rr, so, se = run(BASE + [bad], expect_ok=False)
    if not errors(se):
        fail(f"{why} is refused")
print("PASS 05")
