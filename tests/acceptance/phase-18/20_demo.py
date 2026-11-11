#!/usr/bin/env python3
# The phase-18 demo. (1) docs/arch3d/showcase/phase-18-demo.json (replayed by `cadcraft-cli run --commands`): a small
# house whose kitchen has an island counter detached with a seating overhang (its area exceeds the detached area by 0.3
# times the moved edge's length) and a waterfall end, a backsplash, a corner cabinet with one counter round the corner,
# at least two fillers, crown on a wall cabinet, an open-shelf cabinet and a wall oven on a tall cabinet's face; two
# baths with glass shower enclosures and shower doors, a glass pony wall and a niche; base and crown moldings in every
# finished room, on solid faces only (nothing 0.01 off a door-less shower glass wall at the base or crown heights, nothing
# over the pony wall at the crown height); typed room floors; two saved cameras render as raster Finalizes and one as a
# path-traced Finalize, none blank. (2) The showcase house (docs/arch3d/showcase/bilevel.json) carries the same kitchen
# and bath features and moldings in its kitchen and baths.
# A stub fails: a missing or partly refused commands file, no countertop, backsplash, corner, filler, crown, shelves,
# appliance, shower door, pony wall or niche, moldings on glass, blank renders.
# needs: p18-room-finishes-1, p18-room-finishes-2, p18-room-finishes-3, p18-room-moldings, p18-molding-lines-1, p18-molding-lines-2, p18-cabinet-moldings, p18-countertop-shapes, p18-backsplash, p18-face-appliances, p18-corners, p18-fillers-shelves, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3, p18-shower-niche-1, p18-shower-niche-2, p18-shower-niche-3, p18-shower-niche-4, p18-demo
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

DEMO = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'phase-18-demo.json')
SHOWCASE = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'bilevel.json')
FINISHED = {'living', 'bedroom', 'kitchen', 'bath', 'closet', 'utility', 'dining', 'family', 'entry', 'laundry'}
tmp = tempfile.mkdtemp()


def check_house(insp, what, every_room):
    """The phase-18 features in an inspected model; returns the custom countertop with a waterfall."""
    cabs = cabinets_of(insp)
    cts = [r for r in countertops_of(insp, False) if any(e.get('waterfall') is True for e in r.get('edges') or [])]
    if not cts:
        fail(f"{what}: a custom countertop with a waterfall edge: {[(r.get('id'), r.get('auto')) for r in countertops_of(insp)]}")
    if not [b for b in backsplashes_of(insp) if b.get('segments')]:
        fail(f"{what}: a backsplash with segments")
    corner = [c for c in cabs if isinstance(c.get('corner'), dict)]
    if len(corner) != 1:
        fail(f"{what}: exactly one corner cabinet: {[(c.get('id'), c.get('corner')) for c in corner]}")
    if not [r for r in countertops_of(insp, True) if corner[0]['id'] in (r.get('cabinets') or []) and corner[0]['corner'].get('partner') in (r.get('cabinets') or [])]:
        fail(f"{what}: one automatic counter holds the corner cabinet and its partner")
    if len([c for c in cabs if c.get('filler') is True]) < 2:
        fail(f"{what}: at least two fillers: {[c.get('id') for c in cabs if c.get('filler')]}")
    if not [c for c in cabs if isinstance(c.get('crown'), dict) and c.get('kind') == 'wall']:
        fail(f"{what}: a wall cabinet with a crown")
    if not [c for c in cabs if isinstance(c.get('shelves'), list) and len(c['shelves']) >= 1]:
        fail(f"{what}: an open cabinet with shelves")
    if not [c for c in cabs if c.get('kind') == 'tall' and isinstance(c.get('face'), dict)
            and any(r.get('kind') == 'appliance' for r in c['face'].get('rows') or [])]:
        fail(f"{what}: a tall cabinet with an appliance face row")
    ops = openings_rows(insp)
    walls = {w.get('id'): w for w in walls_rows(insp)}
    showers = [o for o in ops if o.get('style') == 'shower' and walls.get(o.get('host'), {}).get('glass') is True]
    if len(showers) < 2:
        fail(f"{what}: at least two shower doors in glass walls: {[(o.get('id'), o.get('style'), o.get('host')) for o in ops]}")
    if not [w for w in walls.values() if isinstance(w.get('split'), dict) and str(w['split'].get('upper', '')).lower() == 'glass']:
        fail(f"{what}: a split wall with a glass upper (a pony wall)")
    if not [o for o in ops if o.get('kind') == 'niche']:
        fail(f"{what}: a niche")
    rooms = objects_named(insp, 'rooms', 'rooms')
    finished = [r for r in rooms if str(r.get('type', '')).lower() in FINISHED]
    if len(finished) < 4:
        fail(f"{what}: at least four finished rooms: {[(r.get('id'), r.get('type')) for r in rooms]}")
    for r in finished:
        if not every_room and str(r.get('type', '')).lower() not in ('kitchen', 'bath'):
            continue
        for kind in ('base', 'crown'):
            m = molding_rows(insp, r.get('id'), kind)
            if not m or not any(isinstance(x.get('length'), (int, float)) and x['length'] > 0 for x in m):
                fail(f"{what}: room {r.get('id')} ({r.get('type')}) has a {kind} molding with a positive length: {m}")
    surf = surfaces_of(insp)
    for r in finished:
        mat = surf.get(f"room/{r.get('id')}/floor", {}).get('material', '')
        if not str(mat).startswith('finish:'):
            fail(f"{what}: room {r.get('id')}'s floor shows a finish (its type's or a painted one): {mat!r}")
    return cts[0]


# 1. The demo house.
steps = steps_of(DEMO)
CAMS = [st['params']['save'] for st in steps if st['cmd'] == 'camera' and isinstance(st['params'].get('save'), str)]
if len(CAMS) < 2:
    fail(f"the demo saves two cameras (camera {{save: name, ...}}): {CAMS}")
insp = inspected(replay_file18(DEMO, [INSPECT18], "inspect"))
ct = check_house(insp, "the demo", True)
moved = [e for e in ct.get('edges') or [] if e.get('waterfall') is not True and e.get('sagitta', 0) == 0]
detached = [st for st in steps if st['cmd'] == 'countertop.detach']
moves = [st for st in steps if st['cmd'] == 'countertop.edit' and isinstance(st['params'].get('move'), (int, float))]
if not detached or not moves:
    fail("the demo detaches a counter and moves one of its edges (the seating overhang)")
d = moves[0]['params']['move']
e = [x for x in ct.get('edges') or [] if x.get('index') == moves[0]['params'].get('edge')]
if len(e) != 1:
    fail(f"the moved edge {moves[0]['params'].get('edge')} is an edge of {ct.get('id')}: {[x.get('index') for x in ct.get('edges') or []]}")
L = num(e[0].get('length'), "the moved edge's length")
poly = poly_area(ct.get('polygon') or [])
if d < 0.3 - 1e-9:
    fail(f"the seating overhang is at least 0.3 outward: {d}")
# The inspected area is the polygon's plus its arc segments; the polygon already holds the d x L overhang over the
# detached outline (a rectangular island), which must itself be a real counter.
near(num(ct.get('area'), 'area'), poly + sum(segment_area(num(x.get('length'), 'L'), num(x.get('sagitta', 0) or 0, 's')) for x in ct['edges']), 1e-6,
     "the countertop's area is its polygon's plus its arc segments")
if poly - d * L <= 0.5:
    fail(f"the island counter before its {d} x {L:.3f} overhang is a real counter: {poly - d * L:.3f} m2")
# Moldings on solid faces only: probe beside door-less glass walls and over the pony wall.
walls = walls_rows(insp)
hosts = {o.get('host') for o in openings_rows(insp)}
probes, labels = [], []
for w in walls:
    if w.get('glass') is not True or w.get('id') in hosts or isinstance(w.get('split'), dict):
        continue
    (x0, y0), (x1, y1) = w['start'][:2], w['end'][:2]
    L2 = math.hypot(x1 - x0, y1 - y0)
    if L2 < 0.3:
        continue
    nx, ny = -(y1 - y0) / L2, (x1 - x0) / L2
    mx, my = (x0 + x1) / 2, (y0 + y1) / 2
    off = num(w.get('thickness'), 'thickness') / 2 + 0.01
    for s in (1, -1):
        for z in (0.07, 2.35):
            probes.append([mx + s * nx * off, my + s * ny * off, z])
            labels.append(f"0.01 off glass wall {w['id']} at z {z}")
for w in walls:
    sp = w.get('split')
    if not (isinstance(sp, dict) and str(sp.get('upper', '')).lower() == 'glass'):
        continue
    (x0, y0), (x1, y1) = w['start'][:2], w['end'][:2]
    L2 = math.hypot(x1 - x0, y1 - y0)
    nx, ny = -(y1 - y0) / L2, (x1 - x0) / L2
    mx, my = (x0 + x1) / 2, (y0 + y1) / 2
    off = num(w.get('thickness'), 'thickness') / 2 + 0.01
    for s in (1, -1):
        probes.append([mx + s * nx * off, my + s * ny * off, 2.35])
        labels.append(f"0.01 off pony wall {w['id']} at z 2.35 (no crown over its glass top)")
if len(probes) < 4:
    fail(f"the demo has door-less shower glass walls and a pony wall to probe beside: {len(probes)} probes")
shots = [os.path.join(tmp, f'demo-{k}.png') for k in range(3)]
rr = replay_file18(DEMO, [probe_cmd(probes),
                          cmd('render', {"out": shots[0], "size": [640, 480], "camera": CAMS[0], "final": "raster"}),
                          cmd('render', {"out": shots[1], "size": [640, 480], "camera": CAMS[1], "final": "raster"}),
                          cmd('render', {"out": shots[2], "size": [320, 240], "camera": CAMS[0], "final": "path", "spp": 16})],
                   "probes and renders")
for g, what in zip(probed(rr, probes, what="glass faces"), labels):
    if g is not None and part(g) == 'molding':
        fail(f"no molding {what}: the probe hit {g}")
for path, name in zip(shots, ('raster ' + CAMS[0], 'raster ' + CAMS[1], 'path ' + CAMS[0])):
    if not os.path.exists(path):
        fail(f"{name}: no image written")
    w, h, nch, rows = png(path)
    want = (320, 240) if 'path' in name else (640, 480)
    if (w, h) != want:
        fail(f"{name}: {w} x {h}, asked {want[0]} x {want[1]}")
    frac, sd = foreground(path)
    if not (0.2 <= frac <= 1.0) or sd < 10:
        fail(f"{name} renders non-blank: {frac * 100:.1f} % of the pixels differ from the clear colour (20..100 %), luma sd {sd:.2f} (>= 10)")
    artifact(path, os.path.basename(path))
print(f"ok: the demo house: island {ct.get('id')} with a {d} overhang and a waterfall, backsplash, corner, fillers, crown, shelves, oven, showers, pony wall, "
      f"niche; moldings on solid faces only; cameras {CAMS[:2]} render")

# 2. The showcase house.
insp = inspected(replay_file18(SHOWCASE, [INSPECT18], "the showcase"))
check_house(insp, "the showcase", False)
print("ok: the showcase kitchen and baths carry the phase-18 features; moldings in the kitchen and the baths")
print("PASS 20")
