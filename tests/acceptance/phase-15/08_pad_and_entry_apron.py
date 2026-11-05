#!/usr/bin/env python3
# The building pad follows the levels: the first floor (the lowest level at or above 0, here the Entry landing) is padded at
# its floor - structure, lower levels are cut, a floor level whose footprint reaches past the levels below it (a side
# split) gets its own pad there, and the grade outside a door on a landing is a flat apron one 0.15 m step below the landing
# (the apron wins over pad margins; a cut wins inside its level's footprint).
# needs: p15-doc-levels, p15-cmd-levels, p15-arch-spanning-walls, p15-arch-pad-split
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())



def elevations(rows, nth=-1):
    """terrain.query's elevations (phase 9's helper: p9lib.inc is not in the p15 -> p10 include chain)."""
    e = reply_with(rows, 'elevations', nth, 'terrain.query')['elevations']
    if not isinstance(e, list):
        fail(f"elevations is not a list: {e!r}")
    return e


T = cmd('terrain', {"boundary": [[-20, -20], [30, -20], [30, 28], [-20, 28]],
                    "points": [[-20, -20, -0.5], [30, -20, -0.5], [30, 28, 1.0], [-20, 28, 1.0]], "pad": True, "padMargin": 1.0})
HOUSE = [cmd('level.edit', {"level": 0, "name": "Lower", "elevation": -1.2}),
         cmd('level', {"name": "Main", "elevation": 1.5}),
         cmd('level', {"name": "Entry", "elevation": 0.15, "kind": "landing"}),
         cmd('wall', {"points": [[0, 0], [10, 0]], "thickness": 0.2, "exterior": True, "level": "Lower", "topLevel": "Main"}),
         cmd('wall', {"points": [[10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Lower"}),
         cmd('wall', {"points": [[10, 0], [14, 0], [14, 8], [10, 8]], "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Main"}),
         cmd('wall', {"points": [[10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Main"}),
         cmd('wall', {"points": [[3, 0], [3, 3], [6, 3], [6, 0]], "thickness": 0.1, "height": 2.4, "level": "Entry"}),
         cmd('door', {"wall": "w0", "offset": 4.6, "width": 0.9, "level": "Entry"})]
Q = [[4.6, -0.8],    # the apron outside the front door: 0.15 - 0.15 = 0.0
     [2.5, -0.5],    # the Entry pad's flat margin outside the front wall, beside the apron: 0.15 - 0.3 = -0.15
     [4.5, 1.5],     # inside Lower's footprint (under the landing too): the cut, -1.2 - 0.3 = -1.5
     [12.0, 4.0],    # Main's side-split extension (x 10..14): 1.5 - 0.3 = 1.2
     [25.0, 25.0]]   # far away: the original surface, -0.5 + 1.5 * 45/48
rc, rr, so, se = run(HOUSE + [T, cmd('terrain.query', {"points": Q})])
z = elevations(rr)
want = [0.0, -0.15, -1.5, 1.2, -0.5 + 1.5 * 45 / 48]
for p, g, w in zip(Q, z, want):
    near(g, w, 1e-6, f"the finished grade at {p}")
print(f"ok: apron 0.0 at the landing's door, the Entry pad's margin -0.15, Lower's cut -1.5, side-split pad 1.2, the original surface beyond")
print("PASS 08")
