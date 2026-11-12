#!/usr/bin/env python3
# The reworked showcase house (docs/arch3d/showcase/bilevel.json, replayed by `cadcraft-cli run --commands`) is a real
# split entry (the phase-15 checkpoint, decision 1): the front door's landing sits between the lower and main levels with
# a half flight up to Main starting at the landing's edge and a half flight down to Lower ending at it; the main floor
# opens over the landing as an Open Below foyer, so the landing has full headroom (its ceiling at least 2.032 above it);
# nothing (no slab, ceiling or wall) crosses either flight within 2 m above its treads; the up flight arrives on a Main
# room with a floor. The above-grade area (Main's rooms, Open Below, Garage, Porch and Deck left out) is at least
# 371.6 m². The rendered views are committed: docs/arch3d/showcase/renders/manifest.json lists every view kind of
# phase 15's check 11 (rendered from bilevel.json: a manifest of an earlier script is noted, not failed, since 11-11: the
# operator re-renders when a phase closes), each image and the contact sheet tracked in
# git, non-blank (5..99 % of the pixels differ from the clear colour, luma sd > 2), 15 MB at most in all.
# A stub fails: today's script's lower flight ends 12 m from the landing, its upper flight runs into Main's floor slab,
# the landing has 1.05 m of headroom under Main's floor, and no render is committed.
# needs: p16-rw-stair-wells, p16-rw-overhead-arch, p16-rw-overhead-3d, p16-rw-open-below-rooms, p16-rw-above-grade-living, p16-rw-showcase-script, p16-rw-showcase-renders
import os, hashlib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

INSPECT = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "stairs": True})
RENDERS = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'renders')
NEED = {'plan': 3, 'electrical': 2, 'elevation': 4, 'section': 2, 'camera': 4, 'night': 1, 'path': 1, 'clay': 1}


def replay(extra, what):
    args = cli_args('run') + ['--metric', '--commands', SHOWCASE]
    for c in extra:
        args += ['--cmd', c]
    env = dict(os.environ, CADCRAFT_LIBRARY_USER_DIR=tempfile.mkdtemp())
    p = subprocess.run(args, capture_output=True, text=True, timeout=1800, env=env)
    if 'panicked' in p.stderr or p.returncode != 0 or errors(p.stderr):
        fail(f"bilevel.json replays without an error ({what}; exit {p.returncode}): {errors(p.stderr)[:3] or p.stderr[-400:]}")
    return parse_rows(p.stdout)


def holds(poly, x, y):
    inside = False
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            inside = not inside
    return inside


def in_room(r, x, y):
    return holds(r.get('polygon') or [], x, y) and not any(holds(h, x, y) for h in r.get('holeRings') or [])


def dist_to(poly, x, y):
    """0 inside the polygon, else the distance to its boundary."""
    if holds(poly, x, y):
        return 0.0
    best = math.inf
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        dx, dy = x2 - x1, y2 - y1
        t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.hypot(x - x1 - t * dx, y - y1 - t * dy))
    return best


rr = replay([INSPECT], "inspect")
insp = inspected(rr)
lv = objects_named(insp, 'levels', 'levels')
land = [l for l in lv if l.get('kind') == 'landing']
floors = sorted((l for l in lv if l.get('kind') == 'floor'), key=lambda l: l['elevation'])
if len(land) != 1 or len(floors) < 2 or not floors[0]['elevation'] < land[0]['elevation'] < floors[1]['elevation']:
    fail(f"one landing between the lower and main levels: {[(l.get('name'), l.get('kind'), l.get('elevation')) for l in lv]}")
LAND, LOWER, MAIN = land[0], floors[0], floors[1]
by_index = {l['index']: l for l in lv}
rooms = objects_named(insp, 'rooms', 'rooms')
landing_rooms = [r for r in rooms if on_level(r.get('level'), LAND) and r.get('type') != 'openBelow']
if not landing_rooms:
    fail(f"a room on the landing {LAND.get('name')}: {[(r.get('id'), r.get('level')) for r in rooms]}")
main_floored = [r for r in rooms if on_level(r.get('level'), MAIN) and r.get('type') != 'openBelow']
stairs = objects_named(insp, 'stairs', 'stairs')
up = [s for s in stairs if s.get('fromLevel') == LAND['index'] and s.get('toLevel') == MAIN['index']]
down = [s for s in stairs if s.get('fromLevel') == LOWER['index'] and s.get('toLevel') == LAND['index']]
if len(up) != 1 or len(down) != 1:
    fail(f"one half flight from the landing up to {MAIN.get('name')} and one from {LOWER.get('name')} up to the landing: "
         f"{[(s.get('id'), s.get('fromLevel'), s.get('toLevel')) for s in stairs]}")


def ends(s):
    a = math.radians(num(s.get('direction'), 'stair direction'))
    x0, y0 = s['start']
    run = num(s.get('run'), 'stair run')
    return (x0, y0), (x0 + run * math.cos(a), y0 + run * math.sin(a))


(ux0, uy0), (ux1, uy1) = ends(up[0])
(dx0, dy0), (dx1, dy1) = ends(down[0])
du = min(dist_to(r['polygon'], ux0, uy0) for r in landing_rooms)
dd = min(dist_to(r['polygon'], dx1, dy1) for r in landing_rooms)
if du > 0.3 or dd > 0.3:
    fail(f"the half flights meet the landing: the up flight starts {du:.2f} m and the down flight ends {dd:.2f} m from it (0.3 at most)")
dm = min([dist_to(r['polygon'], ux1, uy1) for r in main_floored] or [math.inf])
if dm > 0.3:
    fail(f"the up flight arrives on a floored {MAIN.get('name')} room: its top end ({ux1:.2f}, {uy1:.2f}) is {dm:.2f} m from one")
print(f"ok: the up flight starts on the landing ({du:.2f} m) and arrives on {MAIN.get('name')}; the down flight ends on the landing ({dd:.2f} m)")

# The foyer and the landing's headroom.
lx, ly = interior_point(landing_rooms[0]['polygon'])
foyer = [r for r in rooms if on_level(r.get('level'), MAIN) and r.get('type') == 'openBelow' and in_room(r, lx, ly)]
if not foyer:
    fail(f"an Open Below foyer on {MAIN.get('name')} over the landing at ({lx:.2f}, {ly:.2f}): "
         f"{[(r.get('id'), r.get('type')) for r in rooms if on_level(r.get('level'), MAIN) and in_room(r, lx, ly)]}")

# Probes along both flights: nothing within 2 m over their treads; the landing's ceiling.
pts = []
for s in (up[0], down[0]):
    a = math.radians(s['direction'])
    base = by_index[s['fromLevel']]['elevation']
    rh, td, n = num(s.get('riserHeight'), 'riserHeight'), num(s.get('treadDepth'), 'treadDepth'), int(s.get('treads') or 0)
    for k in sorted({1, max(1, n // 2), max(1, n)}):
        d = (k - 0.5) * td
        top = base + k * rh
        for dz in (1.0, 2.0):
            pts.append([s['start'][0] + d * math.cos(a), s['start'][1] + d * math.sin(a), top + dz])
rr = replay([ceiling_cmd([[lx, ly]], LAND['name']), probe_cmd(pts)], "the landing's ceiling and probes over the flights")
z = ceilings(rr)[0]
if not isinstance(z, (int, float)) or z - LAND['elevation'] < 2.032:
    fail(f"the landing has full headroom: its ceiling at ({lx:.2f}, {ly:.2f}) is {z!r}, {LAND['elevation']} + 2.032 at least")
hits = probed(rr, pts, what='over the flights')
bad = [(p, h) for p, h in zip(pts, hits) if h is not None]
if bad:
    fail(f"nothing crosses the flights within 2 m over their treads: {[(tuple(round(c, 2) for c in p), h.get('part'), h.get('id')) for p, h in bad][:6]}")
print(f"ok: the foyer {foyer[0].get('id')} opens {MAIN.get('name')} over the landing (ceiling {z:.2f}); nothing crosses either flight")

# The above-grade area.
area = num(insp.get('aboveGradeArea'), 'aboveGradeArea')
living = sum(num(r.get('interiorArea'), 'interiorArea') for r in rooms if on_level(r.get('level'), MAIN)
             and r.get('type') not in ('openBelow', 'garage', 'porch', 'deck'))
if area < 371.6 or abs(area - living) > 1e-6:
    fail(f"the above-grade area is {MAIN.get('name')}'s living rooms ({living:.3f} m²) and at least 371.6 m²: {area:.3f}")
print(f"ok: {area:.1f} m² above grade ({MAIN.get('name')}'s rooms less Open Below, garage, porch and deck)")

# The committed renders.
man_path = os.path.join(RENDERS, 'manifest.json')
if not os.path.exists(man_path):
    fail("docs/arch3d/showcase/renders/manifest.json is committed (tools/showcase/render_all.py --out docs/arch3d/showcase/renders)")
man = json.load(open(man_path, encoding='utf-8'))
sha = hashlib.sha256(open(SHOWCASE, 'rb').read()).hexdigest()
if man.get('scriptSha256') != sha:
    # Operator 2026-11-11: not a failure any more. Every segment that edits bilevel.json had to re-render and commit all
    # 19 images (binary files that conflict between segments): p17f-showcase-phase17 and p18-room-finishes-1 were refused
    # on this line alone, and each refusal cost merge cycles. The operator re-renders when a phase closes
    # (tools/showcase/render_all.py --out docs/arch3d/showcase/renders); between closes the images may trail the script.
    print(f"note: the committed renders are of an earlier bilevel.json (manifest scriptSha256 {str(man.get('scriptSha256'))[:12]}, "
          f"bilevel.json {sha[:12]}); the operator re-renders at the phase's close")
tracked = set(subprocess.run(['git', 'ls-files', 'docs/arch3d/showcase/renders'], capture_output=True, text=True, cwd=ROOT).stdout.split())
kinds = {}
total = 0
for im in man.get('images', []):
    rel = 'docs/arch3d/showcase/renders/' + str(im.get('file'))
    f = os.path.join(ROOT, rel)
    if rel not in tracked or not os.path.exists(f):
        fail(f"{rel} ({im.get('kind')}) is committed")
    frac, sd = foreground(f)
    # Line drawings (plans, electrical plans, elevations, sections: thin lines on white) as phase-15 check 11 takes them
    # (0.5 % at least); shaded and rendered views 5 %. A plan of the showcase covers ~3 % of its frame.
    lo = 0.005 if im.get('kind') in ('plan', 'electrical', 'elevation', 'section') else 0.05
    if not (lo <= frac <= 0.99) or sd <= 2:
        fail(f"{rel} renders non-blank: {frac * 100:.1f} % foreground ({lo * 100:g}..99 %), luma sd {sd:.2f} (> 2)")
    kinds.setdefault(im.get('kind'), []).append(im)
    total += os.path.getsize(f)
for k, n in NEED.items():
    if len(kinds.get(k, [])) < n:
        fail(f"the committed renders hold at least {n} '{k}' image(s): {sorted((a, len(b)) for a, b in kinds.items())}")
sheet = 'docs/arch3d/showcase/renders/' + str(man.get('contactSheet', 'contact-sheet.png'))
if sheet not in tracked or luma_sd(os.path.join(ROOT, sheet)) <= 2:
    fail(f"{sheet} is committed and not blank")
total += os.path.getsize(os.path.join(ROOT, sheet))
if total > 15 * 1024 * 1024:
    fail(f"the committed renders take {total / 1048576:.1f} MB (15 MB at most)")
print(f"ok: {len(man.get('images', []))} committed renders of today's script and the contact sheet ({total / 1048576:.1f} MB)")
print("PASS 20")
