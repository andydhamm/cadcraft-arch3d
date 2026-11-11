# The render look lives in the drawing: `look {}` reports the brief's defaults; `look {...}` sets the global look (one
# undo step, saved in the DXF, a refused call changes nothing); a render without look keys echoes the drawing's look, a
# render's own look keys apply to that render only; the legacy `sun: false` is night (sun and sky off); a compass sun
# direction goes through the site's true north; a sun below the horizon is no longer refused; `backdrop {kind: "sky"}`.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
DEFAULT = {"technique": "standard", "exposure": "auto", "toneMap": "aces",
           "sun": {"on": True, "intensity": 100000.0, "shadows": True, "angle": None},
           "sky": {"on": True, "turbidity": 3.0, "stars": False, "moon": {"lux": 0.25, "azimuth": 150.0, "elevation": 35.0}},
           "lightSet": None, "cutaway": False, "clip": None, "nearClip": None}


def looks(rows):
    return [r['look'] for r in rows if isinstance(r, dict) and isinstance(r.get('look'), dict) and 'samples' not in r]


def same(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        return set(a) == set(b) and all(same(a[k], b[k]) for k in a)
    if isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool) and not isinstance(b, bool):
        return abs(a - b) <= 1e-9 * max(1.0, abs(b))
    return a == b


# (1) defaults, exactly.
rows = run_ok(ROOM + ['look {}'], "look {}")
lk = looks(rows)
if not lk:
    fail(f"`look {{}}` replies {{\"look\": {{...}}}}: {json.dumps(rows)[-500:]}")
got = {k: v for k, v in lk[-1].items() if k != 'backdrop'}
if not same(got, DEFAULT):
    fail(f"the default look is the brief's (ACES, auto, 100 000 lx sun with shadows, sky on, 0.25 lx moon):\n got  {got}\n want {DEFAULT}")
print("ok: look {} reports the defaults")

# (2) set, inspect, save, reopen; a refused look changes nothing; undo restores.
SET = cmd('look', {"exposure": {"ev100": 3}, "toneMap": "hable", "cutaway": True})
dxf = os.path.join(tmp, 'look.dxf')
rows = run_ok(ROOM + [SET, cmd('arch.inspect', {"look": True})], "setting a look", save=dxf)
ins = [r['look'] for r in rows if isinstance(r, dict) and isinstance(r.get('look'), dict)]
want = dict(DEFAULT, exposure={"ev100": 3.0}, toneMap="hable", cutaway=True)
if not ins or not same({k: v for k, v in ins[-1].items() if k != 'backdrop'}, want):
    fail(f"arch.inspect {{look: true}} shows the set look: {ins[-1:] or json.dumps(rows)[-400:]}")
rc, rr, so, se = run(['look {}', cmd('look', {"toneMap": "filmic", "exposure": {"ev100": 4}}), 'look {}'], file=dxf, expect_ok=False)
lk = looks(rr)
if len(lk) < 2 or not same({k: v for k, v in lk[0].items() if k != 'backdrop'}, want):
    fail(f"the look survives --save and reopening the DXF: {lk[:1] or json.dumps(rr)[-400:]}")
if not errors(se) or not same(lk[-1], lk[0]):
    fail(f"a refused look (toneMap \"filmic\") prints an error line and changes nothing, not even the valid exposure: {errors(se)}, {lk[-1]}")
rows = run_ok(ROOM + [SET, 'undo {}', 'look {}'], "undo of a look")
if not same({k: v for k, v in looks(rows)[-1].items() if k != 'backdrop'}, DEFAULT):
    fail(f"one undo restores the default look: {looks(rows)[-1]}")
print("ok: look set, inspected, saved and reopened; a refused look changes nothing; undo restores")

# (3) renders echo the effective look; per-render keys apply to that render only.
CAM = {"kind": "full", "eye": [3.5, 3.5, 1.6], "target": [0.5, 0.5, 0.8], "fov": 70}
R = lambda name, **k: render_cmd(os.path.join(tmp, name), CAM, (64, 48), **k)
rows, rs = rendered(ROOM + [SITE, SET, R('a.png'), R('b.png', toneMap="none"), R('c.png'), R('d.png', sun=False),
                            R('e.png', sun={"azimuth": 90, "elevation": 30}),
                            R('f.png', sun={"date": "2026-06-21", "time": "02:00"}),
                            cmd('backdrop', {"kind": "sky"}), 'backdrop {}', R('g.png')], "renders with a look")
if len(rs) != 7:
    fail(f"seven render replies: {len(rs)}")
a, b, c, d, e, f, g = (look_of(r, f"render {i}") for i, r in enumerate(rs))
if a.get('toneMap') != 'hable' or not same(a.get('exposure'), {"ev100": 3.0}):
    fail(f"a render without look keys echoes the drawing's look (hable, EV100 3): {a}")
if b.get('toneMap') != 'none' or c.get('toneMap') != 'hable':
    fail(f"render {{toneMap: none}} applies to that render only: {b.get('toneMap')}, then {c.get('toneMap')}")
if d.get('sun', {}).get('on') is not False or d.get('sky', {}).get('on') is not False:
    fail(f"render {{sun: false}} is the legacy night: sun.on and sky.on false: {d}")
sd = rs[4].get('sunDir')
want_dir = sun_dir(90, 30)
if not (isinstance(sd, list) and len(sd) == 3 and all(abs(num(x, 'sunDir') - y) <= 1e-6 for x, y in zip(sd, want_dir))):
    fail(f"sun {{azimuth 90, elevation 30}} with trueNorth 0 is sunDir [cos 30, 0, sin 30]: {sd}")
sf = rs[5].get('sunDir')
if not (isinstance(sf, list) and len(sf) == 3 and num(sf[2], 'sunDir z') < 0):
    fail(f"a sun below the horizon (02:00) is accepted and reported with sunDir z < 0: {sf}")
bd = [b for b in backdrops(rows) if isinstance(b, dict) and b.get('kind') == 'sky']
if len(bd) < 2:
    fail(f"`backdrop {{kind: sky}}` is accepted and `backdrop {{}}` reports kind sky: {json.dumps(rows)[-600:]}")
if not isinstance(g.get('backdrop'), dict) or g['backdrop'].get('kind') != 'sky':
    fail(f"a render's look echoes the sky backdrop: {g.get('backdrop')}")
rows, rs = rendered(ROOM + [cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 30}),
                            R('h.png', sun={"azimuth": 90, "elevation": 30})], "trueNorth 30")
sh = rs[-1].get('sunDir')
want_dir = sun_dir(90, 30, 30)
if not (isinstance(sh, list) and len(sh) == 3 and all(abs(num(x, 'sunDir') - y) <= 1e-6 for x, y in zip(sh, want_dir))):
    fail(f"trueNorth 30 turns the compass direction 30 degrees about +Z: {sh}, want {want_dir}")
print("ok: renders echo the look, per-render keys stay per render, legacy night, compass sun, night sun, sky backdrop")
print("PASS 14")
