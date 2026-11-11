#!/usr/bin/env python3
# Incremental rebuild (the brief's "incremental as before"): a model holding every phase-18 object (a run of three base
# cabinets, a crowned wall cabinet, a tall cabinet with a face, the run's counter detached and moved, a backsplash on it,
# base and crown moldings, a molding line, a glass pony wall, a niche). After each phase-18 edit the incremental scene's
# digest equals a full build's, and `rebuilt` (objects rebuilt) stays within what the edit can reach:
#   moldingline.edit elevation         -> 1 (the line)
#   countertop.edit sagitta            -> <= 2 (the countertop and the backsplash on it; no cabinet)
#   countertop.edit waterfall          -> <= 2
#   backsplash.edit height             -> 1
#   room.moldings (crown offset)       -> 1 (the room)
#   cabinet.edit crownSize             -> 1 (the cabinet; its neighbours read only the crown's presence)
#   cabinet.edit backsplash off        -> <= 2 (the cabinet and the backsplash on its counter)
#   cabinet.edit crown null            -> <= 2 (the wall and tall cabinets of the level)
#   cabinet.face split down            -> 1
#   wall.split height                  -> 1 (the free-standing glass wall)
#   arch.opening.move (the niche)      -> <= 3 (the niche, its wall, the room whose moldings run along it)
#   room type                          -> 1
# A full rebuild per edit gives `rebuilt` >= the model's 12 objects and fails every bound.
# needs: p18-rebuild
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

BASES = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.6, 0.6]})      # cb0..cb2
WALLC = cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "width": 0.6, "crown": "molding:crown"})   # cb3
TALL = cmd('cabinet', {"kind": "tall", "wall": "w2", "offset": 2.0, "width": 0.762})                   # cb4
FACE = cmd('cabinet.face', {"id": "cb4", "rows": [{"kind": "door"}, {"kind": "drawer"}]})
DETACH = cmd('countertop.detach', {"run": "cb0"})                                                      # ct0
insp = inspected(run_ok(KITCHEN + [BASES, WALLC, DETACH, INSPECT18], "the detached run"))
CT = countertops_of(insp, False)[0]['id'] if countertops_of(insp, False) else fail("a custom countertop")
row = ct_row(insp, CT)
I, _ = edge_index(row, lambda mx, my: abs(my - 0.625) < 1e-6, "the front edge")
J, _ = edge_index(row, lambda mx, my: abs(mx - 2.4) < 1e-6, "the end edge")
SETUP = KITCHEN + [BASES, WALLC, TALL, FACE, DETACH, cmd('countertop.edit', {"id": CT, "edge": I, "move": 0.3}),
                   cmd('backsplash', {"run": CT}),
                   cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}, {"kind": "crown"}]}),
                   cmd('moldingline', {"points": [[3, 3], [5, 3]], "elevation": 1.0}),
                   cmd('wall', {"points": [[4, 1], [5.5, 1]], "type": "glass"}),                       # w4
                   cmd('wall.split', {"id": "w4", "height": 1.07, "lower": "interior"}),
                   cmd('niche', {"wall": "w1", "offset": 2.0, "depth": 0.1})]                          # o0
insp = inspected(run_ok(SETUP + [INSPECT18], "the set-up"))
B0 = backsplashes_of(insp)[0]['id'] if backsplashes_of(insp) else fail("a backsplash")
ML = molding_lines_of(insp)[0]['id'] if molding_lines_of(insp) else fail("a molding line")
O = [o for o in openings_rows(insp) if o.get('kind') == 'niche'][0]['id']

EDITS = [(cmd('moldingline.edit', {"id": ML, "elevation": 1.2}), 1, "the molding line"),
         (cmd('countertop.edit', {"id": CT, "edge": I, "sagitta": 0.1}), 2, "the countertop and its backsplash"),
         (cmd('countertop.edit', {"id": CT, "edge": J, "waterfall": True}), 2, "the countertop and its backsplash"),
         (cmd('backsplash.edit', {"id": B0, "height": 0.6}), 1, "the backsplash"),
         (cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}, {"kind": "crown", "offset": 0.05}]}), 1, "the room"),
         (cmd('cabinet.edit', {"id": "cb3", "crownSize": [0.08, 0.08]}), 1, "the wall cabinet"),
         (cmd('cabinet.edit', {"id": "cb1", "backsplash": False}), 2, "cb1 and the backsplash"),
         (cmd('cabinet.edit', {"id": "cb3", "crown": None}), 2, "the wall and tall cabinets of the level"),
         (cmd('cabinet.face', {"id": "cb4", "row": 1, "split": "down", "count": 2}), 1, "the tall cabinet"),
         (cmd('wall.split', {"id": "w4", "height": 1.2}), 1, "the pony wall"),
         (cmd('arch.opening.move', {"opening": O, "offset": 2.5}), 3, "the niche, its wall and the room"),
         (cmd('room', {"id": "r0", "type": "kitchen"}), 1, "the room (its floor finish)")]
steps = SETUP + [cmd('build3d', {"incremental": True})]
for edit, _, _ in EDITS:
    steps += [edit, cmd('build3d', {"incremental": True, "digest": True}), cmd('build3d', {"digest": True})]
rr = run_ok(steps, "the edits, each followed by an incremental and a full build")
b = [r for r in rr if isinstance(r, dict) and 'rebuilt' in r and isinstance(r.get('digest'), str)]
if len(b) != 2 * len(EDITS):
    fail(f"{2 * len(EDITS)} build3d replies with digests, got {len(b)}")
total = b[1].get('objects')
if not isinstance(total, int) or total < 12:
    fail(f"the model holds at least 12 objects: {total!r}")
for k, (edit, bound, what) in enumerate(EDITS):
    inc, full = b[2 * k], b[2 * k + 1]
    if inc.get('incremental') is not True:
        fail(f"after `{edit}`: the build is incremental, got {inc.get('incremental')!r}")
    if inc.get('digest') != full.get('digest'):
        fail(f"after `{edit}`: the incremental scene's digest {inc.get('digest')} differs from the full build's {full.get('digest')}")
    n = inc.get('rebuilt')
    if not isinstance(n, int) or n < 1 or n > bound:
        fail(f"`{edit}` rebuilds 1..{bound} object(s) ({what}), it rebuilt {n!r}")
    print(f"ok: `{edit[:60]}` rebuilt {n} (<= {bound}: {what}); incremental == full")
print("PASS 17")
