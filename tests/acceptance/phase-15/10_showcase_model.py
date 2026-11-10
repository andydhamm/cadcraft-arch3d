#!/usr/bin/env python3
# The showcase house (docs/arch3d/showcase/bilevel.json, replayed by `cadcraft-cli run --commands`): a split-entry bi-level
# with the front door on a landing between the lower and main levels, the lower level a basement and the main level above
# grade, at least 371.6 m² (4,000 sq ft) above grade; every room furnished, the kitchen and full baths equipped, every
# bedroom a bed, no surface on its default material, lights, switches and connections in every room, outlets meeting the
# spacing rule, a roof, and a sloping graded site with a driveway, paths, trees, shrubs and beds.
# needs: p15-showcase-house, p15-cli-commands-file, p15-inspect, p15-cmd-outlets-auto, p15-arch-grade, p15-cmd-levels
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

steps = showcase_cmds()
out = subprocess.run(cli_args('run') + ['--metric', '--commands', SHOWCASE, '--cmd', INSPECT15], capture_output=True, text=True,
                     timeout=1800, env=dict(os.environ, CADCRAFT_LIBRARY_USER_DIR=tempfile.mkdtemp()))
if out.returncode != 0 or errors(out.stderr):
    fail(f"bilevel.json replays without an error (exit {out.returncode}): {errors(out.stderr)[:3]} {out.stderr[-400:]}")
rows = parse_rows(out.stdout)
ins = inspected(rows)
lv = rows_of(ins, 'levels')
land = [l for l in lv if l.get('kind') == 'landing']
floors = sorted((l for l in lv if l.get('kind') == 'floor'), key=lambda l: l['elevation'])
if len(land) != 1 or len(floors) < 2:
    fail(f"one entry landing and at least two floor levels: {[(l['name'], l.get('kind'), l['elevation']) for l in lv]}")
lower, main = floors[0], floors[1]
if not lower['elevation'] < land[0]['elevation'] < main['elevation']:
    fail(f"the landing sits between the lower and main levels: {lower['elevation']}, {land[0]['elevation']}, {main['elevation']}")
if lower.get('gradeClass') != 'basement' or main.get('gradeClass') != 'aboveGrade':
    fail(f"the lower level is a basement and the main level above grade: {lower.get('gradeClass')}, {main.get('gradeClass')}")
area = num(ins.get('aboveGradeArea'), 'aboveGradeArea')
if area < 371.6:
    fail(f"at least 371.6 m² (4,000 sq ft) above grade: {area:.1f} m²")
walls = {w['id']: w for w in rows_of(ins, 'walls')}
stairs = rows_of(ins, 'stairs')
up = [s for s in stairs if s.get('fromLevel') == land[0]['index'] and s.get('toLevel') == main['index']]
down = [s for s in stairs if s.get('toLevel') == land[0]['index'] and s.get('fromLevel') == lower['index']]
if not up or not down:
    fail(f"half flights from the landing up to the main level and down to the lower level: {stairs}")
print(f"ok: a split-entry bi-level: landing {land[0]['elevation']} between {lower['elevation']} (basement) and {main['elevation']} (above grade); {area:.1f} m² above grade")

rooms = rows_of(ins, 'rooms')
furn = furnishings_of(ins)
fx = rows_of(ins, 'fixtures') if 'fixtures' in ins else []
def in_room(rid):
    return [f for f in furn if f.get('room') == rid] + [x for x in fx if x.get('room') == rid]
need = {'living': 1, 'kitchen': 1, 'bath': 2, 'bedroom': 2, 'dining': 1, 'family': 1}
types = {}
for r in rooms:
    types.setdefault(r.get('type'), []).append(r)
for t, n in need.items():
    if len(types.get(t, [])) < n:
        fail(f"at least {n} {t} room(s): {sorted((k, len(v)) for k, v in types.items())}")
for r in rooms:
    if r.get('type') in ('closet', 'openBelow'):
        continue
    if not in_room(r['id']):
        fail(f"room {r['id']} ({r.get('type')}, {r.get('name')}) has no furnishing")
def has(rid, *words):
    st = ' '.join(str(f.get('symbolType', '')) + ' ' + str(f.get('item', '')) + ' ' + str(f.get('kind', '')) for f in in_room(rid)).lower()
    return all(any(w in st for w in alt.split('|')) for alt in words)
k = types['kitchen'][0]['id']
if not has(k, 'range|cooktop', 'refrigerator', 'sink', 'dishwasher'):
    fail(f"the kitchen has a range, refrigerator, sink and dishwasher: {[f.get('item') for f in in_room(k)]}")
full = [r for r in types['bath'] if has(r['id'], 'toilet', 'sink|lavatory', 'tub|bathtub|shower')]
if len(full) < 2:
    fail(f"two full baths (toilet, sink, tub or shower): {[(r['id'], [f.get('item') for f in in_room(r['id'])]) for r in types['bath']]}")
for r in types['bedroom']:
    if not has(r['id'], 'bed'):
        fail(f"bedroom {r['id']} has a bed")
defaults = [s for s in surfaces_of(ins).values() if str(s.get('material', '')).startswith('default:')]
if defaults:
    fail(f"no surface keeps its default material: {len(defaults)} do, e.g. {defaults[:5]}")
print(f"ok: {len(rooms)} rooms, every one furnished; the kitchen and {len(full)} full baths equipped; no default materials")

el = rows_of(ins, 'electrical')
circ = rows_of(ins, 'circuits')
lit = {l: c for c in circ for l in c['lights']}
lights = lights_of(rows)
for r in rooms:
    if r.get('type') in ('closet', 'openBelow'):
        continue
    rl = [l for l in lights if l.get('room') == r['id']]
    sw = [e for e in el if e.get('room') == r['id'] and e['layer'] == 'E-LITE']
    if not rl or not sw or not any(l['id'] in lit for l in rl):
        fail(f"room {r['id']} ({r.get('type')}) has a light, a switch and a connection: {len(rl)} lights, {len(sw)} switches")
for g in rows_of(ins, 'outletGaps'):
    if g['maxGap'] > 1.8 + 1e-6 or (g.get('counter') and g['counter']['maxSpacing'] > 1.2 + 1e-6):
        fail(f"room {g['room']}'s outlets meet the spacing rule: {g}")
site = ins.get('site') or {}
kinds = {p.get('kind') for p in site.get('paths', [])}
for word in ('driveway', 'sidewalk', 'bed'):
    if word not in kinds:
        fail(f"the site has a {word}: path kinds {sorted(k for k in kinds if k)}")
plants = site.get('plants', [])
forms = {p.get('form') for p in plants}
if len(plants) < 6 or not {'tree', 'shrub'} <= forms:
    fail(f"trees and shrubs on the site: {len(plants)} plants, forms {sorted(f for f in forms if f)}")
if not [w for w in rows_of(ins, 'walls')] or 'roofs' in ins and not ins['roofs']:
    fail("the house has walls and a roof")
print(f"ok: lights, switches and connections in every room; {len(el)} devices; outlet spacing met; driveway, paths, beds, {len(plants)} plants")
print("PASS 10")
