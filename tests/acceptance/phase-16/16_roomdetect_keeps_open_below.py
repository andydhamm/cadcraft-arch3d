#!/usr/bin/env python3
# roomdetect keeps Open Below rooms (the phase-15 checkpoint, decision 1): a stair well's room and an Open Below room
# drawn with `room` survive re-detection on their level, keep their ids, and are cut again as holes out of the detected
# room around them. The phase-5 stair (start (2,1), +y, 1.0 wide) between Level 1 (0) and Level 2 (2.7) of a 10 x 8 m
# ring opens the well x 1.5..2.5, y 1.508..4.81 (3.302 m²) on Level 2, whose room is then 76.44 - 3.302 = 73.138 m².
# After `roomdetect {"level": 1}` and again after `roomdetect {}`: the stair's wellRoom is still an Open Below room on
# Level 2 over the well, the only one there, and Level 2's room still has the well as its one hole (73.138 m²). An Open
# Below room drawn at x 6..8, y 5..7 on Level 2 is a second hole (69.138 m²) and survives `roomdetect {"level": 1}` too.
# A stub fails: today re-detection drops the well's room (the stair points at nothing) and Level 2's room comes back
# whole (76.44 m², no hole).
# needs: p16-rw-open-below-rooms
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

RING = {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True}
INSPECT = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "stairs": True})
BASE = [cmd('wall', RING), cmd('level', {"above": 0}), cmd('wall', dict(RING, level=1)), cmd('roomdetect', {}),
        cmd('stair', {"start": [2, 1], "direction": 90, "width": 1.0, "fromLevel": 0, "toLevel": 1})]
WELL = [1.5, 1.508, 2.5, 4.81]
DRAWN = [[6, 5], [8, 5], [8, 7], [6, 7], [6, 5]]


def holds(poly, x, y):
    inside = False
    for k in range(len(poly)):
        (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            inside = not inside
    return inside


def level2(insp, what, holes, area, drawn_id=None):
    l2 = level_row(insp, 1)
    rooms = [r for r in objects_named(insp, 'rooms', 'rooms') if on_level(r.get('level'), l2)]
    st = objects_named(insp, 'stairs', 'stairs')
    if len(st) != 1:
        fail(f"{what}: one stair: {json.dumps(st)[:300]}")
    wr = [r for r in rooms if r.get('id') == st[0].get('wellRoom')]
    if len(wr) != 1 or wr[0].get('type') != 'openBelow':
        fail(f"{what}: the stair's wellRoom {st[0].get('wellRoom')!r} is an Open Below room on {l2.get('name')}: "
             f"{[(r.get('id'), r.get('type')) for r in rooms]}")
    expect_box(wr[0].get('polygon'), WELL, f"{what}: the well's room")
    opens = [r for r in rooms if r.get('type') == 'openBelow']
    if len(opens) != (2 if drawn_id else 1) or (drawn_id and drawn_id not in [r.get('id') for r in opens]):
        fail(f"{what}: the Open Below rooms on {l2.get('name')} are the well's{' and ' + drawn_id if drawn_id else ''}: "
             f"{[(r.get('id'), r.get('type')) for r in rooms]}")
    host = [r for r in rooms if r.get('type') != 'openBelow' and holds(r.get('polygon') or [], 5, 1)]
    if len(host) != 1:
        fail(f"{what}: one room on {l2.get('name')} around the well: {[(r.get('id'), r.get('type')) for r in rooms]}")
    hr = host[0].get('holeRings') or []
    if len(hr) != holes:
        fail(f"{what}: {host[0].get('id')} has {holes} hole(s) (the Open Below rooms): holeRings {hr!r}")
    if not any(abs(ring_box(h, 'a hole')[1] - 1.508) < 1e-6 for h in hr):
        fail(f"{what}: one of {host[0].get('id')}'s holes is the well: {hr!r}")
    near(host[0].get('interiorArea'), area, 1e-6, f"{what}: {host[0].get('id')}'s interior area")


rr = run_ok(BASE + [INSPECT, cmd('roomdetect', {"level": 1}), INSPECT, cmd('roomdetect', {}), INSPECT],
            "the stair, then roomdetect on Level 2 and on every level")
level2(inspected(rr, 0), "after the stair", 1, 73.138)
level2(inspected(rr, 1), "after roomdetect {level: 1}", 1, 73.138)
level2(inspected(rr, 2), "after roomdetect {}", 1, 73.138)
print("ok: the well's Open Below room survives roomdetect (on its level and on all) and stays a hole of Level 2's room (73.138 m²)")

rr = run_ok(BASE + [cmd('room', {"points": DRAWN, "type": "openBelow", "level": 1}), INSPECT], "an Open Below room drawn on Level 2")
insp = inspected(rr)
drawn = [r for r in objects_named(insp, 'rooms', 'rooms') if r.get('type') == 'openBelow' and holds(r.get('polygon') or [], 7, 6)]
if len(drawn) != 1:
    fail(f"the drawn Open Below room at x 6..8, y 5..7: {[(r.get('id'), r.get('type'), r.get('level')) for r in objects_named(insp, 'rooms', 'rooms')]}")
D = drawn[0]['id']
level2(insp, "after drawing an Open Below room", 2, 73.138 - 4.0, D)
rr = run_ok(BASE + [cmd('room', {"points": DRAWN, "type": "openBelow", "level": 1}), cmd('roomdetect', {"level": 1}), INSPECT],
            "the drawn Open Below room, then roomdetect {level: 1}")
level2(inspected(rr), "after roomdetect with a drawn Open Below room", 2, 73.138 - 4.0, D)
print(f"ok: a drawn Open Below room ({D}) is a second hole (69.138 m²) and survives roomdetect")
print("PASS 16")
