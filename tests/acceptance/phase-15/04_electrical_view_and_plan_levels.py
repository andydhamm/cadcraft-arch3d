#!/usr/bin/env python3
# The Electrical saved plan view (a layer state that keeps the electrical layers and dims the rest), and a plan per level
# (level.show: the plan draws one level, a wall spanning levels on each of them).
# needs: p15-cmd-electrical, p15-cmd-electrical-view, p15-render-plan-level, p15-render-electrical, p15-doc-levels, p15-cmd-levels
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
EL = [cmd('outlet', {"kind": "duplex", "wall": "w0", "offset": 1.0, "side": "left"}),
      cmd('switch', {"kind": "single-pole", "wall": "w3", "offset": 1.0, "side": "left"})]
rc, rr, so, se = run(ROOM64 + EL + [cmd('view.electrical', {}), cmd('layerstate.list', {})])
v = reply_with(rr, 'layerState', -1, 'view.electrical')
if v['layerState'] != 'Electrical' or not {'E-POWR', 'E-LITE'} <= set(v.get('shown', [])) or 'A-WALL' not in v.get('dimmed', []):
    fail(f"view.electrical shows E-POWR and E-LITE and dims A-WALL: {v}")
if 'E-POWR' in v.get('dimmed', []):
    fail(f"an electrical layer is not dimmed: {v}")
states = json.dumps([r for r in rr if isinstance(r, dict)][-1])
if 'Electrical' not in states:
    fail(f"the layer state 'Electrical' is saved: {states[:300]}")
a, b = os.path.join(tmp, 'plain.png'), os.path.join(tmp, 'electrical.png')
rc, rr, so, se = run(ROOM64 + EL + [cmd('export', {"path": a})])
rc, rr, so, se = run(ROOM64 + EL + [cmd('view.electrical', {}), cmd('export', {"path": b})])
if digest(a) == digest(b) or luma_sd(b) < 1:
    fail("the Electrical view's plan differs from the plain plan and is not blank")
artifact(b, 'electrical-view.png')
print("ok: view.electrical saves and restores the 'Electrical' layer state, electrical layers shown, the rest dimmed")

TWO = [cmd('wall', {"points": [[0, 0], [6, 0], [6, 4], [0, 4], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True}),
       cmd('level', {"name": "Upper", "above": 0}),
       cmd('wall', {"points": [[1, 1], [5, 1]], "thickness": 0.1, "height": 2.4, "level": "Upper"}),
       cmd('wall', {"points": [[3, 0], [3, 4]], "thickness": 0.1, "level": 0, "topLevel": "Upper"})]
p0, p1, pa = (os.path.join(tmp, n) for n in ('l0.png', 'l1.png', 'all.png'))
for lv, out in ((0, p0), ("Upper", p1), ("all", pa)):
    rc, rr, so, se = run(TWO + [cmd('level.show', {"level": lv}), cmd('export', {"path": out})])
    r = reply_with(rr, 'shown', -1, 'level.show')
    if (lv == "all") != (r['shown'] is None):
        fail(f"level.show {lv}: {r}")
if len({digest(p0), digest(p1), digest(pa)}) != 3:
    fail("the plans of level 0, level Upper and all levels are three different images")
for p in (p0, p1):
    if luma_sd(p) < 1:
        fail(f"{os.path.basename(p)} is blank")
artifact(p0, 'plan-level-0.png')
artifact(p1, 'plan-level-upper.png')
rc, rr, so, se = run(TWO + [cmd('level.show', {"level": "Attic"})], expect_ok=False)
if not errors(se):
    fail("level.show of a missing level is refused")
print("ok: level.show draws one level's plan (or all); a missing level is refused")
print("PASS 04")
