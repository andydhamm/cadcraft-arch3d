# The phase-19 demo (docs/arch3d/showcase/phase-19-demo.json, replayed by `cadcraft-cli run --commands`): a house at
# dusk (twilight sky, windows and strip lights lit, ground to the horizon), at noon under the CC-0 HDRI, and a floor
# overview with the cutaway, each from its own saved camera with its own look. Every step is accepted; the drawing saved
# as DXF renders each camera; each image has content; the dusk camera's sun angle lies in twilight (0..-6 degrees), its
# view shows ground 5 px under the horizon, and its light set visibly lights the house (against the demo's all-off
# light set "Day"); noon-hdri path-traces under the
# HDRI at the noon angle; the overview hides walls with the cutaway and a pick at its centre finds a floor; each render's
# look echo is that camera's own. Artifacts: dusk.png, noon-hdri.png, overview.png.
# needs: p19-demo, p19-sky-1, p19-sky-2, p19-hdri-1, p19-hdri-2, p19-hdri-3, p19-hdri-4, p19-horizon-1, p19-horizon-2, p19-strip-render, p19-strip-tray, p19-normal-maps-render, p19-cameras-1, p19-cameras-2, p19-cameras-3, p19-cameras-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

DEMO = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'phase-19-demo.json')
if not os.path.exists(DEMO):
    fail(f"{os.path.relpath(DEMO, ROOT)} is missing")
try:
    steps = json.load(open(DEMO, encoding='utf-8'))
except ValueError as e:
    fail(f"phase-19-demo.json is not JSON: {e}")
if not isinstance(steps, list) or not steps or not all(
        isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict) for st in steps):
    fail("phase-19-demo.json is a non-empty JSON array of {\"cmd\", \"params\"} steps")
tmp = tempfile.mkdtemp()
dxf = os.path.join(tmp, 'demo.dxf')
args = cli_args('run') + ['--metric', '--commands', DEMO, '--cmd', cmd('arch.inspect', {"cameras": True, "sunAngles": True,
                                                                                       "lights": True}), '--save', dxf]
env = dict(os.environ)
env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
p = subprocess.run(args, capture_output=True, text=True, timeout=1800, env=env)
if 'panicked' in p.stderr or p.returncode != 0 or errors(p.stderr):
    fail(f"the demo replays without an error (exit {p.returncode}): {errors(p.stderr)[:3] or p.stderr[-400:]}")
insp = [r for r in parse_rows(p.stdout) if isinstance(r, dict) and isinstance(r.get('cameras'), list)]
if not insp:
    fail("arch.inspect lists the demo's cameras")
cams = {c.get('name'): c for c in insp[-1]['cameras'] if isinstance(c, dict)}
angles = {a.get('name'): a for a in (insp[-1].get('sunAngles') or []) if isinstance(a, dict)}
for name, kind in (('dusk', 'full'), ('noon-hdri', None), ('overview', 'overview')):
    c = cams.get(name)
    if not c or not isinstance(c.get('look'), dict) or (kind and c.get('kind') != kind):
        fail(f"the demo saves camera {name!r}{' of kind ' + kind if kind else ''} with its own look: {c}")
dl, nl, ol = cams['dusk']['look'], cams['noon-hdri']['look'], cams['overview']['look']
da = angles.get((dl.get('sun') or {}).get('angle'))
if not da or not (-6.0 <= num(da.get('sunElevation'), 'dusk sun elevation') < 0.0) or (dl.get('backdrop') or {}).get('kind') != 'sky':
    fail(f"dusk: a sky backdrop under a sun angle in twilight (0 to -6 degrees): {dl.get('backdrop')}, {da}")
if (nl.get('backdrop') or {}).get('kind') != 'hdri' or (nl['backdrop'].get('image') != "polyhaven:kloofendal-puresky") \
        or nl.get('technique') != 'path':
    fail(f"noon-hdri: path-traced under the sample HDRI: {nl}")
if ol.get('cutaway') is not True:
    fail(f"overview: the cutaway is on: {ol}")

# Renders from the saved DXF.
e = cams['dusk']
eye = e.get('eye') or []
eye3 = [num(eye[0], 'eye x'), num(eye[1], 'eye y'), num(eye[2] if len(eye) > 2 else e.get('eyeHeight'), 'eye z')]
tg = [num(v, 'target') for v in (e.get('target') or [])]
if len(tg) != 3:
    fail(f"dusk: a full camera with a target: {e}")
d = [tg[0] - eye3[0], tg[1] - eye3[1]]
n = math.hypot(*d) or 1.0
FAR = [eye3[0] + 2000 * d[0] / n, eye3[1] + 2000 * d[1] / n, eye3[2]]
o = {k: os.path.join(tmp, f'{k}.png') for k in ('dusk', 'dark', 'noon-hdri', 'overview')}
rows = run_ok([render_cmd(o['dusk'], 'dusk', (320, 240), samples=[FAR]),
               render_cmd(o['dark'], 'dusk', (320, 240), lightSet="Day"),
               render_cmd(o['noon-hdri'], 'noon-hdri', (160, 120), spp=8),
               render_cmd(o['overview'], 'overview', (320, 240)),
               cmd('pick3d', {"camera": "overview", "size": [320, 240], "pixel": [160, 120]})], "renders of the demo", file=dxf)
rs = render_replies(rows)
if len(rs) != 4:
    fail(f"four render replies: {len(rs)}")
for k in ('dusk', 'noon-hdri', 'overview'):
    w, h, px = img(o[k])
    colours = len(set(px))
    sds = []
    for ch in range(3):
        v = [c[ch] for c in px]
        m = sum(v) / len(v)
        sds.append(math.sqrt(sum((x - m) ** 2 for x in v) / len(v)))
    if colours < 40 or min(sds) < 8:
        fail(f"{k}: the image has content (>= 40 colours, per-channel sd >= 8): {colours} colours, sd {sds}")
    artifact(o[k], f'{k}.png')
for r, name in zip((rs[0], rs[2], rs[3]), ('dusk', 'noon-hdri', 'overview')):
    lk, own = look_of(r, name), cams[name]['look']
    for key in ('exposure', 'lightSet'):
        if json.dumps(lk.get(key), sort_keys=True) != json.dumps(own.get(key), sort_keys=True):
            fail(f"{name}: the render's look echo has the camera's own {key}: {lk.get(key)} vs {own.get(key)}")
    if (lk.get('sun') or {}).get('angle') != (own.get('sun') or {}).get('angle'):
        fail(f"{name}: the render uses the camera's own sun angle: {lk.get('sun')} vs {own.get('sun')}")
hz = rs[0]['samples'][0].get('pixel') if rs[0].get('samples') else None
if not (isinstance(hz, list) and 0 <= hz[1] + 5 < 240):
    fail(f"dusk: the view shows the horizon (a far point at eye height lands in the frame): {rs[0].get('samples')}")
w, h, px = img(o['dusk'])
row = [px[(hz[1] + 5) * w + x] for x in range(w)]
clear = [x for x, c in enumerate(row) if all(abs(c[k] - CLEAR[k]) <= 8 for k in range(3))]
if clear:
    fail(f"dusk: ground to the horizon: row {hz[1] + 5} (5 px under the horizon) shows the clear colour at {clear[:8]}")
lit, dark = img(o['dusk'])[2], img(o['dark'])[2]
changed = sum(1 for a, b in zip(lit, dark) if sum(a) - sum(b) >= 30)
if changed < 0.005 * len(lit):
    fail(f"dusk: the camera's light set lights windows and strips (>= 0.5 % of pixels brighter by 30 than under the "
         f"demo's all-off light set \"Day\"): {changed} pixels")
if rs[2].get('finalMode') != 'path':
    fail(f"noon-hdri: its technique path-traces without `final`: finalMode {rs[2].get('finalMode')}")
f = rs[3].get('filter') or {}
if not (isinstance(f.get('hidden'), int) and f['hidden'] > 0):
    fail(f"overview: the cutaway hides camera-facing walls: filter {rs[3].get('filter')}")
hit = [r['hit'] for r in rows if isinstance(r, dict) and 'hit' in r]
if not hit or not isinstance(hit[-1], dict) or hit[-1].get('part') != 'floor':
    fail(f"overview: a pick at the centre through the overview camera finds a floor (no ceiling, no roof): {hit}")
print("PASS 11")
