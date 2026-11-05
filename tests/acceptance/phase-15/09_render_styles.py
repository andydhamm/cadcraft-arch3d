#!/usr/bin/env python3
# Clay (every material a plain grey) and, if built, technical illustration (flat material colours with silhouette and crease
# lines): render {"style": "clay"|"technical"}. Technical is optional (Andy: "possibly"): a refusal that says it is not
# available yet is a SKIP of that half; anything else must work.
# needs: p15-render3d-clay, p15-render3d-technical
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
SCENE = ROOM64 + [cmd('paint.material', {"material": "#c04020", "target": "wall/w1/left", "scope": "component"}),
                  cmd('paint.material', {"material": "#2060c0", "target": "wall/w2/left", "scope": "component"}),
                  cmd('library.place', {"item": "sofa-futon", "at": [3.0, 2.0]})]
CAM = {"kind": "full", "eye": [0.6, 0.5, 1.5], "target": [6, 4, 1.2], "fov": 75}   # inside, facing the painted w1/w2 corner
sh, cl, te = (os.path.join(tmp, n) for n in ('shaded.png', 'clay.png', 'technical.png'))
rc, rr, so, se = run(SCENE + [cmd('render', {"out": sh, "size": [240, 180], "camera": CAM}),
                              cmd('render', {"out": cl, "size": [240, 180], "camera": CAM, "style": "clay"})])

def saturated(path):
    w, h, nch, rows = png(path)
    return sum(1 for r in rows for x in range(w) if max(r[x * nch:x * nch + 3]) - min(r[x * nch:x * nch + 3]) > 24)

s_sh, s_cl = saturated(sh), saturated(cl)
if s_sh < 200:
    fail(f"the shaded render shows the red and blue walls ({s_sh} saturated pixels)")
if s_cl > 0.01 * 240 * 180:
    fail(f"clay is every material a plain grey: {s_cl} saturated pixels (shaded had {s_sh})")
if luma_sd(cl) < 5:
    fail("the clay render shows the scene (it is blank)")
artifact(cl, 'style-clay.png')
print(f"ok: clay renders grey ({s_cl} saturated pixels vs {s_sh} shaded)")
rc, rr, so, se = run(SCENE + [cmd('render', {"out": te, "size": [240, 180], "camera": CAM, "style": "technical"})], expect_ok=False)
err = ' '.join(errors(se))
if rc != 0 and 'not available yet' in err:
    print("SKIP technical illustration: not available yet (optional)")
elif rc != 0:
    fail(f"render style technical: {err[:300]}")
else:
    w, h, nch, rows = png(te)
    dark = sum(1 for r in rows for x in range(w) if max(r[x * nch:x * nch + 3]) < 40)
    if dark < 150 or saturated(te) < 100 or digest(te) == digest(sh):
        fail(f"technical illustration has dark lines ({dark} px) over flat colours ({saturated(te)} saturated px) and is not the shaded image")
    artifact(te, 'style-technical.png')
    print(f"ok: technical illustration: {dark} line pixels over flat colours")
print("PASS 09")
