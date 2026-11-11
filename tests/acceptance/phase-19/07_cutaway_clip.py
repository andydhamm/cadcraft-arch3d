# Seeing in, the model: with cutaway on, `pick3d` at the pixel of a window in a camera-facing wall returns an object
# behind it or null, never that wall or window; with a clip box ending at z = 2.0, a pick from above at a room's centre
# returns its floor; a near clip skips what lies nearer than it; a saved camera's name works in pick3d. HOUSE1 (the
# 8 x 5 two-room house) with a window in w0 (the south wall of room A) centred at x = 3.0 (Opening.offset is the centre:
# x 2.4..3.6, z 0.9..2.1).
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-seeing-in
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

SET = HOUSE1 + [cmd('window', {"wall": "w0", "offset": 3.0, "width": 1.2, "sill": 0.9, "height": 1.2})]
SIZE, PIX = [200, 150], [100, 75]
SOUTH = {"kind": "full", "eye": [3.0, -8, 1.5], "target": [3.0, 0, 1.5], "fov": 40}
INSIDE = {"kind": "full", "eye": [3.0, 2.5, 1.5], "target": [3.0, 0, 1.5], "fov": 40}
ABOVE = {"kind": "full", "eye": [2.025, 2.5, 20], "target": [2.025, 2.501, 0], "fov": 30}
CLOSE = {"kind": "full", "eye": [3.0, -0.6, 1.5], "target": [3.0, 0, 1.5], "fov": 40}
CLIP = {"min": [-100, -100, -100], "max": [100, 100, 2.0]}


def pick(camera, **k):
    return cmd('pick3d', dict({"camera": camera, "size": SIZE, "pixel": PIX}, **k))


rows = run_ok(SET + [cmd('arch.inspect', {"rooms": True, "openings": True}),
                     pick(SOUTH), pick(SOUTH, cutaway=True), pick(INSIDE, cutaway=True),
                     pick(ABOVE), pick(ABOVE, clip=CLIP), pick(CLOSE), pick(CLOSE, nearClip=1.0),
                     cmd('camera', {"save": "south", "kind": "full", "eye": [3.0, -8], "eyeHeight": 1.5,
                                    "target": [3.0, 0, 1.5], "fov": 40}),
                     pick("south", cutaway=True)], "picks")
insp = inspected(rows)
room_a = room_at(insp, 2.0, 2.5)
picks = [r for r in rows if isinstance(r, dict) and 'hit' in r]
if len(picks) != 8:
    fail(f"eight pick3d replies: {len(picks)}")
if not all(isinstance(p.get('filter'), dict) for p in picks):
    fail(f"every pick3d reply reports its view filter: {[p.get('filter') for p in picks]}")
h = [p['hit'] for p in picks]
hid = lambda x: x.get('id') if isinstance(x, dict) else None
if hid(h[0]) != 'o0' or h[0].get('part') != 'glass':
    fail(f"(a) without the cutaway the window's centre pixel hits the window's glass (o0): {h[0]}")
for i, what in ((1, "(a) with the cutaway"), (7, "(d) through the saved camera `south` with the cutaway")):
    if hid(h[i]) in ('o0', 'w0'):
        fail(f"{what} the pick passes w0 and its window (an object behind or null): {h[i]}")
if hid(h[2]) != 'o0':
    fail(f"(a) from inside the house the cutaway hides nothing: the pick still hits the window: {h[2]}")
if not isinstance(h[3], dict) or h[3].get('part') not in ('ceiling', 'roof'):
    fail(f"(b) from above, unclipped, the pick at room A's centre hits its ceiling: {h[3]}")
if not isinstance(h[4], dict) or h[4].get('part') != 'floor' or hid(h[4]) != room_a:
    fail(f"(b) with a clip box ending at z = 2.0 the pick from above returns room A's floor ({room_a}): {h[4]}")
if hid(h[5]) != 'o0' or not 0.4 <= num(h[5].get('distance'), 'distance') <= 0.7:
    fail(f"(c) 0.6 m in front of the window the pick hits its glass 0.4..0.7 m away: {h[5]}")
if isinstance(h[6], dict) and (hid(h[6]) in ('o0', 'w0') or num(h[6].get('distance'), 'distance') < 1.0):
    fail(f"(c) with nearClip 1.0 nothing nearer than 1 m is picked (not w0 or o0): {h[6]}")
print(f"ok: cutaway {hid(h[1])}, inside {hid(h[2])}, clipped floor {hid(h[4])}, near clip {hid(h[6])}, named camera {hid(h[7])}")
print("PASS 07")
