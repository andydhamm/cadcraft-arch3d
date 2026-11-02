# The benchmark house: `arch.sample {"house": true}` builds two levels with 300 walls meeting in L, T and X joins, 150
# openings (doors in partitions, windows in the exterior), detected rooms on both levels and a hip roof, deterministically,
# and names a benchmark edit (an interior wall end and a point 0.3 m from it along the wall) that stretches and back.
# needs: p13-sample-house
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

SAMPLE = cmd('arch.sample', {"house": True})
INS = cmd('arch.inspect', {"walls": True, "openings": True, "rooms": True, "levels": True, "roofs": True})
rc, rr, so, se = run([SAMPLE, INS])
rep = [r for r in rr if isinstance(r, dict) and 'bench' in r]
if not rep:
    fail(f"arch.sample {{house: true}} replies with walls, openings, levels, rooms, roofs and bench: {json.dumps(rr)[:500]}")
rep = rep[-1]
ins = inspected(rr)
walls, ops, rooms, levels = ins['walls'], ins.get('openings') or [], ins.get('rooms') or [], ins.get('levels') or []
print(f"house: {len(walls)} walls, {len(ops)} openings, {len(rooms)} rooms, {len(levels)} levels, reply {dict((k, rep.get(k)) for k in ('walls', 'openings', 'rooms', 'roofs'))}")
if not (300 <= len(walls) <= 320) or rep.get('walls') != len(walls):
    fail(f"300 to 320 walls (the reply agrees with arch.inspect): {len(walls)} / {rep.get('walls')}")
if len(levels) != 2 or rep.get('levels') != 2:
    fail(f"two levels: {len(levels)}")
per_level = {}
for w in walls:
    per_level[w.get('level')] = per_level.get(w.get('level'), 0) + 1
if len(per_level) != 2 or min(per_level.values()) < 120:
    fail(f"walls on both levels, at least 120 each: {per_level}")
joins = classify_joins(walls)
print(f"joins from the wall geometry: {joins}")
if min(joins.values()) < 8:
    fail(f"at least 8 L, 8 T and 8 X joins: {joins}")
wall_by = {w['id']: w for w in walls}
doors = [o for o in ops if o.get('kind') == 'door' and wall_by.get(o.get('host'), {}).get('exterior') is False]
windows = [o for o in ops if o.get('kind') == 'window' and wall_by.get(o.get('host'), {}).get('exterior') is True]
if not (150 <= len(ops) <= 170) or len(doors) < 40 or len(windows) < 40:
    fail(f"150 to 170 openings with at least 40 doors in partitions and 40 windows in the exterior: {len(ops)}, "
         f"{len(doors)} doors, {len(windows)} windows")
by_room_level = {}
for r in rooms:
    by_room_level[r.get('level')] = by_room_level.get(r.get('level'), 0) + 1
if len(by_room_level) != 2 or min(by_room_level.values()) < 15:
    fail(f"rooms detected on both levels, at least 15 each: {by_room_level}")
roofs = ins.get('roofs') or []
if len(roofs) != 1 or rep.get('roofs') != 1:
    fail(f"one roof: {len(roofs)}")
planes = roofs[0].get('planes') or []
slopes = [p.get('slope') for p in planes]
if len(planes) < 4 or any(not isinstance(s, (int, float)) or not math.isfinite(s) or s <= 0.1 for s in slopes):
    fail(f"a hip roof: at least 4 sloped planes, none vertical (gable): slopes {slopes}")
if roofs[0].get('level') != max(per_level):
    fail(f"the roof sits on the upper level (wall level {max(per_level)}): {roofs[0].get('level')}")
print(f"ok: {len(walls)} walls on two levels, {len(ops)} openings, {len(rooms)} rooms, a {len(planes)}-plane hip roof")

b = rep['bench']
w = wall_by.get(b.get('wall'))
if not w or w.get('exterior') is not False or b.get('end') not in ('start', 'end'):
    fail(f"the bench edit names an interior wall and one of its ends: {b}, wall {w}")
end_pt = w[b['end']]
if max(abs(num(b['from'][k], 'bench from') - end_pt[k]) for k in range(2)) > 1e-9:
    fail(f"bench 'from' is the wall's {b['end']} point {end_pt}: {b['from']}")
dx, dy = b['to'][0] - b['from'][0], b['to'][1] - b['from'][1]
ax, ay = w['end'][0] - w['start'][0], w['end'][1] - w['start'][1]
if abs(math.hypot(dx, dy) - 0.3) > 1e-9 or abs(dx * ay - dy * ax) > 1e-9 * math.hypot(ax, ay):
    fail(f"bench 'to' is 0.3 m from 'from' along the wall's axis: {b}")
st = lambda to: cmd('arch.wall.stretch', {"wall": b['wall'], "end": b['end'], "to": to})
rc, rr2, so2, se2 = run([SAMPLE, st(b['to']), INS, st(b['from']), INS])
if was_refused(rc, se2):
    fail(f"the bench stretch and its way back succeed: {se2[-400:]}")
w_out = {x['id']: x for x in inspected(rr2, -2)['walls']}[b['wall']]
w_back = {x['id']: x for x in inspected(rr2, -1)['walls']}[b['wall']]
if max(abs(w_out[b['end']][k] - b['to'][k]) for k in range(2)) > 1e-9 or max(abs(w_back[b['end']][k] - b['from'][k]) for k in range(2)) > 1e-9:
    fail(f"the stretch moves the end to 'to' and back to 'from': {w_out[b['end']]}, {w_back[b['end']]}")
print(f"ok: bench edit {b['wall']} {b['end']} {b['from']} -> {b['to']} and back")

rc, rr3, so3, se3 = run([SAMPLE, INS])
if json.dumps(inspected(rr3)['walls'], sort_keys=True) != json.dumps(walls, sort_keys=True) or \
        json.dumps(inspected(rr3).get('openings'), sort_keys=True) != json.dumps(ops, sort_keys=True):
    fail("arch.sample {house: true} is deterministic")
print("ok: deterministic")
print("PASS 07")
