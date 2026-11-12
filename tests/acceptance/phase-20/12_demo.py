#!/usr/bin/env python3
# The phase-20 demo. (A) docs/arch3d/showcase/phase-20-demo.json, a small two-level house replayed with
# `cadcraft-cli run --commands`: plan views for every level (floor plan, electrical, reflected ceiling), the Roof Plan
# and Site, dimensioned automatically; every view exports non-blank and draws what it should (its layer colours: the
# furniture #13579b and dimensions #108040 in the floor plan, power #c02060 in electrical, ceiling #2468ac and no
# furniture in the reflected ceiling with the Bedroom's suspended 2.17 and the Level 2 tray's height, roof annotation
# #a05010 in the roof plan, the major contour #5a3c1e in the site plan only, the reference level #b4b4b4 under Level 2).
# (B) the showcase house (docs/arch3d/showcase/bilevel.json) followed by docs/arch3d/showcase/phase-20-showcase.json:
# floor plan, electrical and reflected ceiling views for each of its floor levels, auto exterior dimensions on each, the
# roof plan and site plan, all exported non-blank.
# A stub fails: missing files, refused steps, no plan views or auto rows, blank or identical images.
# needs: p20-plan-render, p20-plan-views, p20-view-export, p20-reference, p20-rcp, p20-roof-plan, p20-room-labels, p20-auto-dims-1, p20-auto-dims-2, p20-demo
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

SHOW = os.path.join(ROOT, 'docs', 'arch3d', 'showcase')
DEMO, BILEVEL, SHOW20 = (os.path.join(SHOW, n) for n in ('phase-20-demo.json', 'bilevel.json', 'phase-20-showcase.json'))
tmp = tempfile.mkdtemp()


def load(path):
    if not os.path.exists(path):
        fail(f"{os.path.relpath(path, ROOT)} is missing")
    try:
        steps = json.load(open(path, encoding='utf-8'))
    except ValueError as e:
        fail(f"{os.path.relpath(path, ROOT)} is not JSON: {e}")
    if not isinstance(steps, list) or not steps or not all(
            isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict)
            for st in steps):
        fail(f"{os.path.relpath(path, ROOT)} is a non-empty JSON array of {{\"cmd\", \"params\"}} steps")
    return steps


def replay(files, extra, what):
    """Every step of `files` (in order), then `extra`; a refusal or a panic fails."""
    args = cli_args('run') + ['--metric']
    for f in files:
        args += ['--commands', f]
    for c in extra:
        args += ['--cmd', c]
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    p = subprocess.run(args, capture_output=True, text=True, timeout=1800, env=env)
    if 'panicked' in p.stderr or 'panicked' in p.stdout:
        fail(f"panic replaying {what}: {(p.stderr + p.stdout)[-800:]}")
    if p.returncode != 0 or errors(p.stderr):
        fail(f"{what}: a step is refused (exit {p.returncode}): {' | '.join(errors(p.stderr)[:4]) or p.stderr[-400:]}")
    return parse_rows(p.stdout)


def exports(views):
    """export steps for each view to PNG and SVG; returns (steps, {view: (png, svg)})."""
    out, paths = [], {}
    for i, v in enumerate(views):
        png_, svg_ = os.path.join(tmp, f"v{i}.png"), os.path.join(tmp, f"v{i}.svg")
        out += [cmd('export', {"path": png_, "planView": v}), cmd('export', {"path": svg_, "planView": v})]
        paths[v] = (png_, svg_)
    return out, paths


# ---- (A) the small two-level house -------------------------------------------------------------------------------
load(DEMO)
V = ["Level 1 Floor Plan", "Level 2 Floor Plan", "Level 1 Electrical", "Level 2 Electrical",
     "Level 1 Reflected Ceiling", "Level 2 Reflected Ceiling", "Roof Plan", "Site"]
EXP, P = exports(V)
rr = replay([DEMO], [INSPECT20, PV_LAYERS] + EXP +
            [cmd('arch.inspect', {"plan": "Level 1 Reflected Ceiling", "rooms": True}),
             cmd('arch.inspect', {"plan": "Level 2 Reflected Ceiling", "rooms": True, "trays": True})], "the demo")
insp = inspected(rr, 0)
rooms = objects_named(insp, 'rooms', 'rooms')
if not any(r.get('levelIndex') == 0 for r in rooms) or not any(r.get('levelIndex') == 1 for r in rooms):
    fail(f"both levels have rooms: {[(r.get('id'), r.get('levelIndex')) for r in rooms]}")
views, opened, active = pv_views(rr, 0)
names = [v.get('name') for v in views]
for v in V:
    if v not in names:
        fail(f"the demo has the plan view {v!r}: {names}")
for v in V[:6]:
    lv = 0 if v.startswith("Level 1") else 1
    if pv_view(views, v).get('level') != lv:
        fail(f"{v} is bound to level {lv}: {pv_view(views, v)}")
if (pv_view(views, "Level 2 Floor Plan").get('reference') or {}).get('level') != 0:
    fail(f"Level 2 Floor Plan references level 0: {pv_view(views, 'Level 2 Floor Plan')}")
if active != "Level 1 Floor Plan":
    fail(f"the demo ends with Level 1 Floor Plan open: active {active!r}")
auto = pv_auto(rr, 0)
if not any(r.get('level') == 0 for r in auto) or not any(r.get('level') == 1 for r in auto):
    fail(f"auto rows on levels 0 and 1: {[(r.get('level'), r.get('side'), r.get('row')) for r in auto]}")
pv_values(pv_auto_row(auto, 'north', 'walls', 0), [6.1, 4.1], "Level 1's north walls row")
L = pv_layer_rows(rr, 0)
for name, col in (('A-ANNO-DIMS', '#108040'), ('E-POWR', '#c02060')):
    if str(pv_layer(L, name).get('colorRgb', '')).lower() != col:
        fail(f"the demo colours {name} {col} (after the last outlet and switch for E-POWR): {L[name]}")
for name in ('L-PLNT', 'C-TOPO-MINR'):
    if pv_layer(L, name, "after the replay (Level 1 Floor Plan open)").get('on') is not False:
        fail(f"{name} (made by no demo command) exists and is off with Level 1 Floor Plan open: {L[name]}")
col = {v: pv_svg_colours(P[v][1]) for v in V}
for v in V:
    if luma_sd(P[v][0]) < 1:
        fail(f"the {v} PNG is blank")
    artifact(P[v][0], f"phase-20-{v.lower().replace(' ', '-').replace('&', 'and')}.png")
want = {"Level 1 Floor Plan": (['#13579b', '#108040'], ['#5a3c1e']), "Level 1 Electrical": (['#c02060'], []),
        "Level 1 Reflected Ceiling": (['#2468ac'], ['#13579b', '#5a3c1e']), "Level 2 Reflected Ceiling": (['#2468ac'], []),
        "Roof Plan": (['#a05010'], []), "Level 2 Floor Plan": (['#b4b4b4'], []), "Site": (['#5a3c1e'], [])}
for v, (has, hasnt) in want.items():
    for c in has:
        if c not in col[v]:
            fail(f"the {v} SVG draws {c}: {sorted(col[v])}")
    for c in hasnt:
        if c in col[v]:
            fail(f"the {v} SVG does not draw {c}: {sorted(col[v])}")
if open(P["Level 1 Floor Plan"][1], 'rb').read() == open(P["Level 2 Floor Plan"][1], 'rb').read():
    fail("Level 1 and Level 2 Floor Plan export different drawings")
p1, p2 = pv_plan(rr, 0), pv_plan(rr, 1)
i1, i2 = inspected(rr, 1), inspected(rr, 2)
bed = [r for r in objects_named(i1, 'rooms', 'rooms') if r.get('name') == 'Bedroom']
if len(bed) != 1 or [l.get('text') for l in pv_labels(p1, 'roomCeiling') if l.get('handle') == bed[0].get('handle')] != ["2.17"]:
    fail(f"Level 1 Reflected Ceiling labels the Bedroom's suspended ceiling 2.17: {pv_labels(p1, 'roomCeiling')}")
trays = trays_of(i2)
if len(trays) != 1:
    fail(f"the demo has one tray: {trays}")
t = trays[0]
west = room_row(i2, t.get('room'))
outl = [l for l in pv_lines(p2, 'trayOutline') if l.get('handle') == t.get('handle')]
th = pv_labels(p2, 'trayHeight')
if len(outl) < 4 or len(th) != 1 or th[0].get('text') != f"{float(west.get('ceilingText')) + 0.3:.2f}":
    fail(f"Level 2 Reflected Ceiling outlines the tray and labels it {west.get('ceilingText')} + 0.3: {th}")
rc_lab = [l for l in pv_labels(p2, 'roomCeiling') if l.get('handle') == west.get('handle')]
ring = [pv_pt(l.get('a'), 'tray') for l in outl]
if len(rc_lab) != 1 or pv_inside(pv_pt(rc_lab[0].get('at'), 'west ceiling'), pv_box_ring(pv_bbox(outl))):
    fail(f"the tray room's ceiling label sits outside the tray outline: {rc_lab}")
print("ok (A): the demo replays; eight views export non-blank with their colours; Bedroom 2.17; the tray labelled "
      f"{th[0].get('text')}; auto rows on both levels (north walls 6.1 + 4.1)")

# ---- (B) the showcase house ----------------------------------------------------------------------------------------
load(BILEVEL)
load(SHOW20)
rr = replay([BILEVEL, SHOW20], [INSPECT20], "the showcase with phase-20-showcase.json")
insp = inspected(rr)
levels = [l for l in objects_named(insp, 'levels', 'levels') if l.get('kind') == 'floor']
views, opened, active = pv_views(rr)
walls = objects_named(insp, 'walls', 'walls')
auto = pv_auto(rr)
SV = ["Roof Plan", "Site"]
for lv in levels:
    for suffix in ("Floor Plan", "Electrical", "Reflected Ceiling"):
        v = pv_view(views, f"{lv['name']} {suffix}")
        if v.get('level') != lv['index']:
            fail(f"{v.get('name')} is bound to level {lv['index']}: {v}")
        SV.append(v['name'])
    ext = [w for w in walls if w.get('exterior') and w.get('level') == lv['index']]
    if not ext:
        continue
    xs = [c for w in ext for p in ('start', 'end') for c in (pv_xy(w[p], 'wall')[0] - w['thickness'] / 2,
                                                             pv_xy(w[p], 'wall')[0] + w['thickness'] / 2)]
    ys = [c for w in ext for p in ('start', 'end') for c in (pv_xy(w[p], 'wall')[1] - w['thickness'] / 2,
                                                             pv_xy(w[p], 'wall')[1] + w['thickness'] / 2)]
    for side, span in (('south', max(xs) - min(xs)), ('north', max(xs) - min(xs)), ('east', max(ys) - min(ys)),
                       ('west', max(ys) - min(ys))):
        pv_values(pv_auto_row(auto, side, 'overall', lv['index']), [span], f"the showcase's {lv['name']} {side} overall row")
if len(SV) < 8:
    fail(f"views for at least two floor levels: {SV}")
EXP, P = exports(SV)
rr = replay([BILEVEL, SHOW20], EXP + [cmd('arch.inspect', {"plan": "Roof Plan"})], "the showcase's views exported")
for v in SV:
    if luma_sd(P[v][0]) < 1:
        fail(f"the showcase's {v} PNG is blank")
    artifact(P[v][0], f"showcase-{v.lower().replace(' ', '-')}.png")
for lv in levels:
    a, b = P[f"{lv['name']} Floor Plan"][0], P[f"{lv['name']} Reflected Ceiling"][0]
    if digest(a) == digest(b):
        fail(f"the showcase's {lv['name']} floor plan and reflected ceiling plan differ")
if not pv_labels(pv_plan(rr), 'pitch'):
    fail("the showcase's Roof Plan labels its roof planes' pitches")
print(f"ok (B): the showcase's {len(SV)} plan views export non-blank; auto exterior dimensions on {len(levels)} levels")
print("PASS 12")
