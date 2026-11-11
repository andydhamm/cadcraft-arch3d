#!/usr/bin/env python3
# Observables first: every phase-17 data field has its default and is reported by arch.inspect. The phase-5 stair row
# gains openRisers false, stringers "two", underside 0.3, railLeft/railRight "auto", planCut 1.2; a drawn railing row
# gains postSpacing 1.8 (the new default), inset 0, host null, auto false and its posts (ends plus equal bays at most
# 1.8 m apart: a 5 m path gives 4 posts, bays of 1.6667); the well guard has host "well" and follows the stair; a deck
# room row gains boards null and rails []; the phase-4 window row gains top "square", topRise 0, roofOffset 0.15,
# grille null, sillSides/casingSides "right" and its glass plane: centre (4.0, 8.05, 1.5), normal (0, -1) (the wall's
# left, w2 running (10,8) -> (0,8)); a hinged door has no glass plane. A stub (missing keys, no posts, the old 1.2 m
# spacing) fails.
# needs: p17-observables
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

RAIL = cmd('railing', {"path": [[12, 0], [17, 0]]})
DOOR = cmd('door', {"wall": "w4", "offset": 2.0, "width": 0.9, "height": 2.1})
rr = run_ok(HOUSE2 + [STAIR5, WINDOW4, DOOR, RAIL, NORTH_DECK, INSPECT17], "the set-up (stair, window, door, railing, deck)")
insp = inspected(rr)

# 1. The stair row.
S = stairs_of(insp)
if len(S) != 1:
    fail(f"one stair: {[s.get('id') for s in S]}")
s = S[0]
for key, want in (('openRisers', False), ('stringers', 'two'), ('railLeft', 'auto'), ('railRight', 'auto')):
    if key not in s:
        fail(f"the stair row has no '{key}': {sorted(s)}")
    if s[key] != want:
        fail(f"stair {s.get('id')}: {key} defaults to {want!r}, got {s[key]!r}")
near(s.get('underside'), 0.3, 1e-9, "the stair's underside (default 0.3)")
near(s.get('planCut'), 1.2, 1e-9, "the stair's planCut (default 1.2)")
print(f"ok: stair {s['id']}: openRisers false, stringers two, underside 0.3, rails auto, planCut 1.2")

# 2. Railings: the drawn one (host null) and the well guard (host "well", follows the stair).
R = railings_of(insp)
for r in R:
    for key in ('postSpacing', 'inset', 'host', 'auto', 'posts', 'follows'):
        if key not in r:
            fail(f"railing row {r.get('id')} has no '{key}': {sorted(r)}")
drawn = [r for r in R if r.get('host') is None]
if len(drawn) != 1 or drawn[0].get('follows') is not None or drawn[0].get('auto') is not False:
    fail(f"one drawn railing (host null, follows null, auto false): {[(r.get('id'), r.get('host'), r.get('follows'), r.get('auto')) for r in R]}")
d = drawn[0]
near(d['postSpacing'], 1.8, 1e-9, "the drawn railing's postSpacing (default 1.8)")
near(d['inset'], 0.0, 1e-9, "the drawn railing's inset (default 0)")
# 5 m at 1.8 at most: 3 equal bays of 5/3, so posts at 12, 13.6667, 15.3333 and 17.
same_points(d['posts'], [[12, 0], [12 + 5 / 3, 0], [12 + 10 / 3, 0], [17, 0]], 1e-3, "the drawn railing's posts (5 m at 1.8: 3 bays)")
guard = one_railing(insp, 'well', s['id'], "the well guard")
if guard.get('auto') is not False or guard.get('level') != 1:
    fail(f"the well guard is a phase-5 railing on Level 2 (auto false, level 1): {json.dumps(guard)[:300]}")
print(f"ok: drawn railing {d['id']} spacing 1.8, posts at 12, 13.667, 15.333, 17; well guard {guard['id']} follows {s['id']}")

# 3. Rooms: the deck row has boards and rails, no other room does.
dk = deck_room(insp)
if 'boards' not in dk or dk['boards'] is not None or dk.get('rails') != []:
    fail(f"the deck room {dk.get('id')} has boards null and rails []: {json.dumps(dk)[:300]}")
for r in objects_named(insp, 'rooms', 'rooms'):
    if r.get('type') != 'deck' and ('boards' in r or 'rails' in r):
        fail(f"room {r.get('id')} ({r.get('type')}) carries deck keys: {sorted(r)}")
print(f"ok: deck {dk['id']}: boards null, rails []")

# 4. Openings: the window's defaults and glass plane; the door has none.
O = openings_of(insp)
win = [o for o in O if o.get('kind') == 'window']
door = [o for o in O if o.get('kind') == 'door']
if len(win) != 1 or len(door) != 1:
    fail(f"one window and one door: {[(o.get('id'), o.get('kind')) for o in O]}")
w, dr = win[0], door[0]
for key in ('top', 'topRise', 'roofOffset', 'grille', 'sillSides', 'casingSides', 'glassCentre', 'glassNormal'):
    if key not in w:
        fail(f"the window row has no '{key}': {sorted(w)}")
if w['top'] != 'square' or w['grille'] is not None or w['sillSides'] != 'right' or w['casingSides'] != 'right':
    fail(f"window {w['id']}: top square, grille null, sides right: {w['top']!r}, {w['grille']!r}, {w['sillSides']!r}, {w['casingSides']!r}")
near(w['topRise'], 0.0, 1e-9, "the window's topRise (0 = unset)")
near(w['roofOffset'], 0.15, 1e-9, "the window's roofOffset (default 0.15)")
c, along, n = glass_frame(w, f"window {w['id']}")
for k, want in enumerate((4.0, 8.05, 1.5)):
    near(c[k], want, 1e-6, f"the window's glassCentre {'xyz'[k]}")
near(n[0], 0.0, 1e-6, "glassNormal x")
near(n[1], -1.0, 1e-6, "glassNormal y (the wall's left: w2 runs (10,8) -> (0,8))")
if dr.get('glassCentre') is not None or dr.get('glassNormal') is not None or dr.get('top') != 'square':
    fail(f"the hinged door has no glass plane and a square top: {json.dumps(dr)[:300]}")
print(f"ok: window {w['id']}: square, no grille, right sides, glass centre (4.0, 8.05, 1.5) normal (0, -1); door {dr['id']} has no glass")
print("PASS 01")
