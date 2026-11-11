#!/usr/bin/env python3
# Hostile input to every phase-18 command is refused with an error line, never a panic, and nothing changes: no wall,
# room, opening, cabinet, countertop, backsplash, molding, molding-line or surface row, and the built scene's digest is
# the same. (Non-finite numbers cannot be sent: JSON has no NaN, and the CLI refuses a non-JSON --cmd before running
# anything; strings, nulls, arrays and 1e308 stand in.)
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-room-moldings, p18-molding-lines-1, p18-molding-lines-2, p18-countertop-shapes, p18-backsplash, p18-cabinet-fronts-1, p18-cabinet-fronts-2, p18-face-appliances, p18-corners, p18-fillers-shelves, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3, p18-shower-niche-1, p18-shower-niche-2, p18-shower-niche-3, p18-shower-niche-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

SETUP = KITCHEN + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.6, 0.6]}),       # cb0..cb2
                   cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "width": 0.6}),                     # cb3
                   cmd('countertop.detach', {"run": "cb0"}),                                                        # ct0
                   cmd('cabinet', {"kind": "tall", "wall": "w2", "offset": 2.0, "width": 0.762}),                   # cb4
                   cmd('cabinet.face', {"id": "cb4", "rows": [{"kind": "door"}, {"kind": "door"}]}),
                   cmd('cabinet.face', {"id": "cb4", "row": 0, "appliance": "appliance:wall-oven"}),
                   cmd('cabinet.face', {"id": "cb4", "row": 1, "appliance": "appliance:microwave-drawer"}),
                   cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}]}),
                   cmd('moldingline', {"points": [[3, 3], [5, 3]], "elevation": 1.0}),
                   cmd('backsplash', {"run": "ct0"})]
KEYS = ('walls', 'rooms', 'openings', 'cabinets', 'countertops', 'backsplashes', 'moldings', 'moldingLines', 'surfaces')
H = [('room.moldings', {"room": "r9", "moldings": []}),
     ('room.moldings', {"room": "r0", "moldings": [{"kind": "skirting"}]}),
     ('room.moldings', {"room": "r0", "moldings": [{"kind": "base", "offset": -1}]}),
     ('room.moldings', {"room": "r0", "moldings": [{"kind": "base", "size": [0, 0.09]}]}),
     ('room.moldings', {"room": "r0", "moldings": [{"kind": "base", "profile": "own:pull-bar-128"}]}),
     ('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}] * 9}),
     ('room.moldings', {"room": "r0", "moldings": "none"}),
     ('moldingline', {"points": [[0, 0]], "elevation": 1}),
     ('moldingline', {"points": [[0, 0], [1, 0]], "elevation": "1"}),
     ('moldingline', {"points": [[0, 0], [1e308, 0]], "elevation": 1}),
     ('moldingline', {"points": [[0, 0], [1, 0]], "elevation": 1, "inset": 5}),
     ('moldingline', {"points": [[0, 0], [1, 0], [0, 0.001]], "elevation": 1}),            # a 180-degree turn
     ('moldingline.edit', {"id": "ml9", "elevation": 1}),
     ('moldingline.edit', {"id": "ml0", "points": [[0, 0]]}),
     ('moldingline.delete', {"id": "w0"}),
     ('countertop.detach', {"run": "cb99"}),
     ('countertop.detach', {"run": "w0"}),
     ('countertop.detach', {"run": "cb3"}),                                                    # a wall cabinet
     ('countertop.detach', {"run": "cb1"}),                                                    # already claimed
     ('countertop', {"polygon": [[0, 0], [1, 0]], "elevation": 0.9, "thickness": 0.03}),
     ('countertop', {"polygon": [[0, 0], [1, 1], [1, 0], [0, 1]], "elevation": 0.9, "thickness": 0.03}),   # a bow-tie
     ('countertop', {"polygon": [[0, 0], [1, 0], [1, 1], [0, 1]], "elevation": 0.9, "thickness": 0}),
     ('countertop', {"polygon": [[0, 0], [1, 0], [1, 1], [0, 1]], "elevation": 99, "thickness": 0.03}),
     ('countertop.edit', {"id": "ct0", "edge": 99, "move": 0.1}),
     ('countertop.edit', {"id": "ct0", "edge": 0, "move": -50}),
     ('countertop.edit', {"id": "ct0", "edge": 0, "sagitta": 1e9}),
     ('countertop.edit', {"id": "ct0", "edge": 0, "radius": 0.01}),
     ('countertop.edit', {"id": "ct0", "edge": 0, "profile": "ogee"}),
     ('countertop.edit', {"id": "ct0", "edge": 0, "move": 0.1, "sagitta": 0.1}),
     ('countertop.edit', {"id": "ct0", "thickness": 5}),
     ('countertop.edit', {"id": "ct9", "top": 1.0}),
     ('countertop.delete', {"id": "cb0"}),
     ('backsplash', {}),
     ('backsplash', {"run": "cb0", "wall": "w0"}),
     ('backsplash', {"run": "cb0", "height": -0.1}),
     ('backsplash', {"run": "cb3"}),
     ('backsplash', {"wall": "w9"}),
     ('backsplash.edit', {"id": "bs9", "height": 0.5}),
     ('backsplash.edit', {"id": "bs0", "top": "sky"}),
     ('backsplash.delete', {"id": "ct0"}),
     ('cabinet.face', {"id": "cb0", "rows": []}),
     ('cabinet.face', {"id": "cb0", "row": 7, "split": "down", "count": 3}),
     ('cabinet.face', {"id": "cb0", "row": 0, "split": "down", "count": 99}),
     ('cabinet.face', {"id": "cb0", "row": 0, "split": "sideways", "count": 2}),
     ('cabinet.face', {"id": "cb0", "rows": [{"kind": "door", "height": 5}]}),
     ('cabinet.face', {"id": "cb0", "rows": [{"kind": "hatch"}]}),
     ('cabinet.face', {"id": "cb4", "row": 2, "appliance": "appliance:warming-drawer"}),      # a third appliance
     ('cabinet.face', {"id": "cb0", "row": 0, "appliance": "sofa-chaise"}),
     ('cabinet.face', {"id": "cb9", "reset": True}),
     ('cabinet.fill', {"run": "w0"}),
     ('cabinet.fill', {"run": "cb99"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "construction": "plywood"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "mount": "flush"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "corner": "round"}),
     ('cabinet', {"kind": "wall", "wall": "w0", "offset": 3.0, "fronts": "open", "shelves": 99}),
     ('cabinet', {"kind": "wall", "wall": "w0", "offset": 3.0, "shelves": "many"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "crown": "molding:crown"}),
     ('cabinet', {"kind": "wall", "wall": "w0", "offset": 3.0, "crown": "sofa-chaise"}),
     ('cabinet', {"kind": "wall", "wall": "w0", "offset": 3.0, "crownSize": [0.05, 0.05]}),  # a size with no crown
     ('cabinet.edit', {"id": "cb3", "backsplash": "yes"}),
     ('wall', {"points": [[0, 0], [1, 0]], "type": "Glasss"}),
     ('wall.split', {"id": "w0", "height": 9}),
     ('wall.split', {"id": "w0", "height": 1.0, "upper": "marble"}),
     ('wall.split', {"id": "w0", "height": 1.0, "upperThickness": 0.5}),
     ('wall.split', {"id": "w9", "height": 1.0}),
     ('niche', {"wall": "w0", "offset": 1, "width": 0.6, "height": 0.9, "sill": 1, "depth": 0.195}),   # 0.2 < 0.195 + 0.012
     ('niche', {"wall": "w0", "offset": 1, "depth": -0.1}),
     ('niche', {"wall": "w0", "offset": 1, "depth": 0.1, "sill": 0}),
     ('niche', {"wall": "w0", "offset": 1, "depth": 0.1, "face": "top"}),
     ('niche', {"wall": "w9", "offset": 1}),
     ('door', {"wall": "w0", "offset": 1, "style": "shower"}),                                 # not a glass wall
     ('library.add', {"objects": ["w0"], "name": "x", "kind": "molding"}),
     ('library.add', {"objects": ["zz"], "name": "x", "kind": "molding"})]


def state(rows):
    insp = inspected(rows)
    digests = [r['digest'] for r in rows if isinstance(r, dict) and isinstance(r.get('digest'), str)]
    if not digests:
        fail(f"no build3d digest; replies: {json.dumps(rows)[-400:]}")
    out = {k: insp.get(k) for k in KEYS}
    for k in KEYS:
        if not isinstance(out[k], list):
            fail(f"arch.inspect has no '{k}' list: {sorted(insp)}")
    out['digest'] = digests[-1]
    return out


rc, rr, so, se = run(SETUP + [INSPECT18, DIGEST])
if errors(se):
    fail("the set-up is refused:\n" + '\n'.join(errors(se)))
before = state(rr)
if len(before['countertops']) != 1 or len(before['backsplashes']) != 1 or len(before['moldings']) != 1 or len(before['moldingLines']) != 1:
    fail(f"the set-up places one countertop, backsplash, molding and molding line: "
         f"{[(k, len(before[k])) for k in ('countertops', 'backsplashes', 'moldings', 'moldingLines')]}")
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT18, DIGEST], expect_ok=False)
if 'panicked' in se:
    fail(f"a hostile command panicked: {se[-400:]}")
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:80]))
after = state(rr)
for k in KEYS + ('digest',):
    if json.dumps(before[k], sort_keys=True) != json.dumps(after[k], sort_keys=True):
        fail(f"refused commands changed '{k}': {json.dumps(before[k])[:300]} -> {json.dumps(after[k])[:300]}")
print(f"ok: {len(H)} hostile commands refused; rows and the scene unchanged")
print("PASS 18")
