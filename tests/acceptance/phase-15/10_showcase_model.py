#!/usr/bin/env python3
# The showcase house (docs/arch3d/showcase/bilevel.json, replayed by `cadcraft-cli run --commands`): a split-entry bi-level
# with the front door on a landing between the lower and main levels, the lower level a basement and the main level above
# grade, at least 371.6 m² (4,000 sq ft) above grade; every room furnished, the kitchen and full baths equipped, every
# bedroom a bed, no surface on its default material, lights, switches and connections in every room, outlets meeting the
# spacing rule, a roof, and a sloping graded site with a driveway, paths, trees, shrubs and beds.
# Revised 2026-11-09 (phase-15 checkpoint, decision 1): the house is where the checks say it is.
#  - every room lies inside the outer faces of the exterior walls that reach its level, and no two rooms of a level
#    overlap (Open Below rooms excepted among themselves); no wall is drawn twice (collinear, overlapping by more than
#    0.5 m in plan and in height);
#  - at least three bedrooms; the kitchen holds at least three base cabinets with counters, two wall cabinets and an
#    island (a base cabinet on no wall), and every cabinet stands in a kitchen, bath, laundry or utility room; the
#    kitchen's counters get GFCI outlets (at least two: the counter rule runs);
#  - each of the four sides of the house has a window, and every bedroom, the living room and the kitchen has one;
#  - the half flights meet the landing: the up flight starts within 0.3 m of it, the down flight ends within 0.3 m of it;
#  - a roof exists over the main level (arch.inspect is asked for roofs): one roof there with at least two planes.
# needs: p15-showcase-house, p15-cli-commands-file, p15-inspect, p15-cmd-outlets-auto, p15-arch-grade, p15-cmd-levels, p16-rw-showcase-script
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

INSPECT10 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "cabinets": True, "fixtures": True,
                                 "furnishings": True, "surfaces": True, "electrical": True, "lights": True,
                                 "lightSets": True, "stairs": True, "site": True, "openings": True, "roofs": True})
out = subprocess.run(cli_args('run') + ['--metric', '--commands', SHOWCASE, '--cmd', INSPECT10], capture_output=True, text=True,
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
stairs = rows_of(ins, 'stairs')
up = [s for s in stairs if s.get('fromLevel') == land[0]['index'] and s.get('toLevel') == main['index']]
down = [s for s in stairs if s.get('toLevel') == land[0]['index'] and s.get('fromLevel') == lower['index']]
if not up or not down:
    fail(f"half flights from the landing up to the main level and down to the lower level: {stairs}")
print(f"ok: a split-entry bi-level: landing {land[0]['elevation']} between {lower['elevation']} (basement) and {main['elevation']} (above grade); {area:.1f} m² above grade")


def holds(poly, x, y):
    inside = False
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            inside = not inside
    return inside


def in_room(r, x, y):
    return holds(r.get('polygon') or [], x, y) and not any(holds(h, x, y) for h in r.get('holeRings') or [])


def inner_point(r):
    """A point inside room r (its polygon less its holes): the centroid, else a scan of its bounding box."""
    poly = r.get('polygon') or []
    a = cx = cy = 0.0
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        c = x1 * y2 - x2 * y1
        a += c; cx += (x1 + x2) * c; cy += (y1 + y2) * c
    if abs(a) > 1e-12 and in_room(r, cx / (3 * a), cy / (3 * a)):
        return cx / (3 * a), cy / (3 * a)
    xs, ys = [q[0] for q in poly], [q[1] for q in poly]
    for i in range(1, 40):
        for j in range(1, 40):
            x, y = min(xs) + (max(xs) - min(xs)) * i / 40, min(ys) + (max(ys) - min(ys)) * j / 40
            if in_room(r, x, y):
                return x, y
    fail(f"room {r.get('id')} has no inside point: {poly}")


def dist_to(poly, x, y):
    if holds(poly, x, y):
        return 0.0
    best = math.inf
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        dx, dy = x2 - x1, y2 - y1
        t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.hypot(x - x1 - t * dx, y - y1 - t * dy))
    return best


rooms = rows_of(ins, 'rooms')
walls = rows_of(ins, 'walls')
wall_by = {w['id']: w for w in walls}
level_by_name = {l['name']: l for l in lv}
level_by_index = {l['index']: l for l in lv}
lvl = lambda r: level_by_name.get(r.get('level')) or level_by_index.get(r.get('level'))
reach = lambda w, l: num(w.get('bottom'), 'wall bottom') <= l['elevation'] + 1e-6 and l['elevation'] < num(w.get('top'), 'wall top') - 0.01


def box_of(l):
    ext = [w for w in walls if w.get('exterior') and reach(w, l)]
    if not ext:
        fail(f"exterior walls reach level {l['name']}")
    xs = [c[0] + s * w['thickness'] / 2 for w in ext for c in (w['start'], w['end']) for s in (-1, 1)]
    ys = [c[1] + s * w['thickness'] / 2 for w in ext for c in (w['start'], w['end']) for s in (-1, 1)]
    return min(xs), min(ys), max(xs), max(ys)


# Rooms inside the walls, no overlaps.
for r in rooms:
    l = lvl(r)
    x0, y0, x1, y1 = box_of(l)
    out_pts = [q for q in r.get('polygon') or [] if not (x0 - 1e-6 <= q[0] <= x1 + 1e-6 and y0 - 1e-6 <= q[1] <= y1 + 1e-6)]
    if out_pts:
        fail(f"room {r['id']} ({r.get('name')}, {l['name']}) lies inside the exterior walls of its level "
             f"({x0:.2f}..{x1:.2f} x {y0:.2f}..{y1:.2f}): {out_pts[:3]}")
for r in rooms:
    x, y = inner_point(r)
    others = [o for o in rooms if o is not r and o.get('level') == r.get('level') and in_room(o, x, y)
              and not (o.get('type') == 'openBelow' and r.get('type') == 'openBelow')]
    if others:
        fail(f"room {r['id']} ({r.get('name')}) overlaps {[(o['id'], o.get('name')) for o in others]} on {r.get('level')} at ({x:.2f}, {y:.2f})")
for i, a in enumerate(walls):
    for b in walls[i + 1:]:
        if a['thickness'] <= 0 or b['thickness'] <= 0 or min(a['top'], b['top']) - max(a['bottom'], b['bottom']) <= 0.01:
            continue
        (ax0, ay0), (ax1, ay1) = a['start'], a['end']
        la = math.hypot(ax1 - ax0, ay1 - ay0)
        if la < 1e-9:
            continue
        ux, uy = (ax1 - ax0) / la, (ay1 - ay0) / la
        off = [abs((p[0] - ax0) * uy - (p[1] - ay0) * ux) for p in (b['start'], b['end'])]
        if max(off) > 0.05:
            continue
        t = sorted(((p[0] - ax0) * ux + (p[1] - ay0) * uy) for p in (b['start'], b['end']))
        if min(la, t[1]) - max(0.0, t[0]) > 0.5:
            fail(f"walls {a['id']} and {b['id']} are one wall drawn twice (collinear, overlapping in plan and height)")
print(f"ok: {len(rooms)} rooms inside the exterior walls of their levels, none overlapping; no wall drawn twice")

furn = furnishings_of(ins)
fx = rows_of(ins, 'fixtures') if 'fixtures' in ins else []
def contents(rid):
    return [f for f in furn if f.get('room') == rid] + [x for x in fx if x.get('room') == rid]
need = {'living': 1, 'kitchen': 1, 'bath': 2, 'bedroom': 3, 'dining': 1, 'family': 1}
types = {}
for r in rooms:
    types.setdefault(r.get('type'), []).append(r)
for t, n in need.items():
    if len(types.get(t, [])) < n:
        fail(f"at least {n} {t} room(s): {sorted((k, len(v)) for k, v in types.items())}")
for r in rooms:
    if r.get('type') in ('closet', 'openBelow'):
        continue
    if not contents(r['id']):
        fail(f"room {r['id']} ({r.get('type')}, {r.get('name')}) has no furnishing")
def has(rid, *words):
    st = ' '.join(str(f.get('symbolType', '')) + ' ' + str(f.get('item', '')) + ' ' + str(f.get('kind', '')) for f in contents(rid)).lower()
    return all(any(w in st for w in alt.split('|')) for alt in words)
k = types['kitchen'][0]['id']
if not has(k, 'range|cooktop', 'refrigerator', 'sink', 'dishwasher'):
    fail(f"the kitchen has a range, refrigerator, sink and dishwasher: {[f.get('item') for f in contents(k)]}")
full = [r for r in types['bath'] if has(r['id'], 'toilet', 'sink|lavatory', 'tub|bathtub|shower')]
if len(full) < 2:
    fail(f"two full baths (toilet, sink, tub or shower): {[(r['id'], [f.get('item') for f in contents(r['id'])]) for r in types['bath']]}")
for r in types['bedroom']:
    if not has(r['id'], 'bed'):
        fail(f"bedroom {r['id']} has a bed")
defaults = [s for s in surfaces_of(ins).values() if str(s.get('material', '')).startswith('default:')]
if defaults:
    fail(f"no surface keeps its default material: {len(defaults)} do, e.g. {defaults[:5]}")
print(f"ok: {len(rooms)} rooms, every one furnished; the kitchen and {len(full)} full baths equipped; no default materials")

# The kitchen's casework.
def cabinet_rooms(c):
    l = level_by_index.get(c.get('level')) or level_by_name.get(c.get('level'))
    th = math.radians(num(c.get('rotation', 0.0), 'cabinet rotation'))
    x = c['at'][0] - math.sin(th) * c['depth'] / 2
    y = c['at'][1] + math.cos(th) * c['depth'] / 2
    return [r for r in rooms if lvl(r) is l and in_room(r, x, y)]
cabs = rows_of(ins, 'cabinets')
kitchens = {r['id'] for r in types['kitchen']}
in_kitchen = [c for c in cabs if any(r['id'] in kitchens for r in cabinet_rooms(c))]
base = [c for c in in_kitchen if c.get('kind') == 'base' and c.get('counter')]
upper = [c for c in in_kitchen if c.get('kind') == 'wall']
island = [c for c in base if c.get('wall') is None]
if len(base) < 3 or len(upper) < 2 or not island:
    fail(f"the kitchen holds at least three base cabinets with counters ({len(base)}), two wall cabinets ({len(upper)}) and an "
         f"island ({len(island)}): {[(c['id'], c.get('kind'), c.get('at'), [r['id'] for r in cabinet_rooms(c)]) for c in cabs]}")
for c in cabs:
    rs = cabinet_rooms(c)
    if not rs or any(r.get('type') not in ('kitchen', 'bath', 'laundry', 'utility') for r in rs):
        fail(f"cabinet {c['id']} ({c.get('kind')}) stands in a kitchen, bath, laundry or utility room: {[(r['id'], r.get('type')) for r in rs]}")
gfci = [e for e in rows_of(ins, 'electrical') if e.get('room') in kitchens and str(e.get('kind', '')).lower().startswith('gfci')]
if len(gfci) < 2:
    fail(f"the kitchen's counters get their GFCI outlets (the counter rule runs): {len(gfci)} GFCI outlet(s) in the kitchen")
print(f"ok: the kitchen holds {len(base)} base cabinets with counters, {len(upper)} wall cabinets and an island; every cabinet "
      f"in a kitchen, bath, laundry or utility room; the counter outlet rule runs")

# Windows on every side and in the rooms that need light.
wins = [o for o in rows_of(ins, 'openings') if o.get('kind') == 'window']
sides = set()
lit_rooms = set()
x0, y0, x1, y1 = box_of(main)
for o in wins:
    w = wall_by.get(o.get('host'))
    if not w:
        continue
    (ax, ay), (bx, by) = w['start'], w['end']
    ln = math.hypot(bx - ax, by - ay)
    if ln < 1e-9:
        continue
    ux, uy = (bx - ax) / ln, (by - ay) / ln
    cx, cy = ax + ux * o['offset'], ay + uy * o['offset']
    if w.get('exterior'):
        for name, d in (('south', abs(cy - y0)), ('north', abs(cy - y1)), ('west', abs(cx - x0)), ('east', abs(cx - x1))):
            if d <= w['thickness'] / 2 + 0.05:
                sides.add(name)
    z = num(w.get('bottom'), 'wall bottom') + num(o.get('sill'), 'sill') + num(o.get('height'), 'height') / 2
    for s in (1, -1):
        px, py = cx - uy * s * (w['thickness'] / 2 + 0.15), cy + ux * s * (w['thickness'] / 2 + 0.15)
        for r in rooms:
            l = lvl(r)
            if l['elevation'] <= z <= l['elevation'] + 2.7 and in_room(r, px, py):
                lit_rooms.add(r['id'])
if sides != {'south', 'north', 'east', 'west'}:
    fail(f"each side of the house has a window: {sorted(sides)}")
dark = [(r['id'], r.get('type'), r.get('name')) for r in rooms if r.get('type') in ('bedroom', 'living', 'kitchen') and r['id'] not in lit_rooms]
if dark:
    fail(f"every bedroom, the living room and the kitchen has a window: {dark}")
print(f"ok: windows on all four sides ({len(wins)}) and in every bedroom, living room and kitchen")

# The half flights meet the landing; the roof exists.
landing_rooms = [r for r in rooms if lvl(r) is land[0] and r.get('type') != 'openBelow']
def flight_ends(s):
    a = math.radians(num(s.get('direction'), 'direction'))
    return s['start'], [s['start'][0] + s['run'] * math.cos(a), s['start'][1] + s['run'] * math.sin(a)]
du = min([dist_to(r['polygon'], *flight_ends(up[0])[0]) for r in landing_rooms] or [math.inf])
dd = min([dist_to(r['polygon'], *flight_ends(down[0])[1]) for r in landing_rooms] or [math.inf])
if du > 0.3 or dd > 0.3:
    fail(f"the half flights meet the landing: the up flight starts {du:.2f} m and the down flight ends {dd:.2f} m from it (0.3 at most)")
roofs = [r for r in rows_of(ins, 'roofs') if r.get('level') == main['index'] and len(r.get('planes') or []) >= 2]
if not roofs:
    fail(f"a roof with at least two planes over {main['name']}: {[(r.get('id'), r.get('level'), len(r.get('planes') or [])) for r in rows_of(ins, 'roofs')]}")
print(f"ok: the flights meet the landing ({du:.2f} / {dd:.2f} m); roof {roofs[0].get('id')} over {main['name']} with {len(roofs[0]['planes'])} planes")

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
print(f"ok: lights, switches and connections in every room; {len(el)} devices; outlet spacing met; driveway, paths, beds, {len(plants)} plants")
print("PASS 10")
