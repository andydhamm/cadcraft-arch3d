#!/usr/bin/env python3
# The phase-16 demo (docs/arch3d/showcase/phase-16-demo.json, replayed by `cadcraft-cli run --commands`): a small house
# with a vaulted living room (its ceiling above 3 m inside), a tray in the dining room (2.7 at its centre), soffits over
# the kitchen wall cabinets (at least three, bottom at their 2.134 m tops), a suspended utility ceiling (three layers,
# finished at 2.1713), footings under every exterior wall on grade and a concrete basement (two foundations: the
# Basement's with its exterior walls "Concrete 0.2", and Level 1's; at least seven footings and three stem walls); its two
# saved cameras and the level-1 plan render non-blank.
# A stub fails: a missing or partly refused commands file, a flat ceiling, no tray, soffit, stack or foundation, a blank
# image.
# needs: p16-vault-walls, p16-trays, p16-soffits, p16-foundation-stem, p16-foundation-slab, p16-demo
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

DEMO = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'phase-16-demo.json')
if not os.path.exists(DEMO):
    fail(f"{os.path.relpath(DEMO, ROOT)} is missing")
try:
    steps = json.load(open(DEMO, encoding='utf-8'))
except ValueError as e:
    fail(f"{os.path.relpath(DEMO, ROOT)} is not JSON: {e}")
if not isinstance(steps, list) or not steps or not all(
        isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict) for st in steps):
    fail("phase-16-demo.json is a non-empty JSON array of {\"cmd\", \"params\"} steps")
CAMS = [st['params']['save'] for st in steps
        if st['cmd'] == 'camera' and isinstance(st.get('params'), dict) and isinstance(st['params'].get('save'), str)]
if len(CAMS) < 2:
    fail(f"the demo saves two cameras (camera {{save: name, ...}}): {CAMS}")
tmp = tempfile.mkdtemp()


def replay(extra, what):
    """The demo's steps, then `extra`: every step runs (a refusal or a panic fails)."""
    args = cli_args('run') + ['--metric', '--commands', DEMO]
    for c in extra:
        args += ['--cmd', c]
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    p = subprocess.run(args, capture_output=True, text=True, timeout=1800, env=env)
    if 'panicked' in p.stderr or 'panicked' in p.stdout:
        fail(f"panic replaying the demo ({what}): {(p.stderr + p.stdout)[-800:]}")
    if p.returncode != 0 or errors(p.stderr):
        fail(f"the demo replays without an error ({what}; exit {p.returncode}): {errors(p.stderr)[:3] or p.stderr[-400:]}")
    return parse_rows(p.stdout)


# 1. The model.
insp = inspected(replay([INSPECT16], "inspect"))
rooms = objects_named(insp, 'rooms', 'rooms')
vaults = [r for r in rooms if r.get('flatCeiling') is False]
if not vaults:
    fail(f"a room with flatCeiling false: {[(r.get('id'), r.get('name'), r.get('flatCeiling')) for r in rooms]}")
stacks = [r for r in rooms if isinstance(r.get('ceilingStack'), list) and len(r['ceilingStack']) == 3
          and isinstance(r.get('finishedCeiling'), (int, float)) and abs(r['finishedCeiling'] - 2.1713) <= 1e-6]
if not stacks:
    fail(f"a room with a three-layer stack finished at 2.1713: "
         f"{[(r.get('id'), r.get('name'), len(r.get('ceilingStack') or []), r.get('finishedCeiling')) for r in rooms]}")
trays = trays_of(insp)
if not trays:
    fail("the demo has a tray")
soffits = [s for s in soffits_of(insp) if isinstance(s.get('over'), list) and len(s['over']) >= 3]
if not soffits or not any(isinstance(s.get('bottom'), (int, float)) and abs(s['bottom'] - 2.134) <= 1e-6 for s in soffits):
    fail(f"a soffit over at least three wall cabinets with its bottom at 2.134: "
         f"{[(s.get('id'), s.get('over'), s.get('bottom')) for s in soffits_of(insp)]}")
fds = foundations_of(insp)
base, ground = level_row(insp, 'Basement'), level_row(insp, 0)
fb = [f for f in fds if on_level(f.get('level'), base)]
fg = [f for f in fds if on_level(f.get('level'), ground)]
if len(fds) != 2 or len(fb) != 1 or len(fg) != 1 or fb[0].get('basement') is not True:
    fail(f"two foundations, the Basement's (basement true) and {ground.get('name')}'s: {json.dumps(fds)[:400]}")
bw = [w for w in objects_named(insp, 'walls', 'walls') if w.get('level') == base.get('index') and w.get('exterior')]
if not bw or any(w.get('kind') != 'Concrete 0.2' for w in bw):
    fail(f"every Basement exterior wall is Concrete 0.2: {[(w.get('id'), w.get('kind')) for w in bw]}")
print(f"ok: {len(vaults)} vaulted room(s), {len(trays)} tray(s), a soffit over {len(soffits[0]['over'])} cabinets at 2.134, "
      f"a three-layer stack at 2.1713, foundations on the Basement ({len(bw)} Concrete 0.2 walls) and {ground.get('name')}")

# 2. Ceilings, parts, the two cameras and the level-1 plan.
q = [ceiling_cmd([interior_point(r.get('polygon') or [])], r.get('levelIndex', 0)) for r in vaults]
centres = []
for t in trays:
    x0, y0, x1, y1 = ring_box(t.get('outline'), f"tray {t.get('id')}'s outline")
    centres.append([(x0 + x1) / 2, (y0 + y1) / 2])
    q.append(ceiling_cmd([centres[-1]], t.get('level', 0)))
shots = [os.path.join(tmp, f'demo-camera-{k}.png') for k in range(2)]
plan = os.path.join(tmp, 'demo-plan-level-1.png')
rr = replay(q + [cmd('build3d', {})] +
            [cmd('render', {"out": s, "size": [800, 600], "camera": c}) for s, c in zip(shots, CAMS)] +
            [cmd('export', {"path": plan, "level": 0})], "ceilings, parts, renders and the plan")
zs = []
for k in range(len(q)):
    z = ceilings(rr, k)
    if not (isinstance(z, list) and len(z) == 1):
        fail(f"ceiling query #{k} answers one point: {z!r}")
    zs.append(z[0])
vz = zs[:len(vaults)]
if not any(isinstance(z, (int, float)) and z > 3.0 for z in vz):
    fail(f"a vaulted room's ceiling is above 3.0 inside it: {[(r.get('id'), z) for r, z in zip(vaults, vz)]}")
tz = zs[len(vaults):]
if not any(isinstance(z, (int, float)) and abs(z - 2.7) <= 1e-6 for z in tz):
    fail(f"a tray's centre ceiling is 2.7: {[(t.get('id'), c, z) for t, c, z in zip(trays, centres, tz)]}")
parts = parts_of(rr)
if (parts.get('footing') or 0) < 7 or (parts.get('foundation') or 0) < 3:
    fail(f"footings under the four basement walls and the three on-grade walls, stem walls under the three: parts {parts}")
for path, name in list(zip(shots, CAMS)) + [(plan, 'the level-1 plan')]:
    if not os.path.exists(path):
        fail(f"{name}: no image written")
    frac, sd = foreground(path)
    if not (0.05 <= frac <= 0.99) or sd <= 2:
        fail(f"{name} renders non-blank: {frac * 100:.1f} % of the pixels differ from the clear colour (5..99 %), luma sd {sd:.2f} (> 2)")
    artifact(path, os.path.basename(path))
print(f"ok: vault {max(z for z in vz if isinstance(z, (int, float))):.3f} inside, a tray centre 2.7, parts footing "
      f"{parts.get('footing')} / foundation {parts.get('foundation')}; cameras {CAMS[:2]} and the plan render non-blank")
print("PASS 13")
