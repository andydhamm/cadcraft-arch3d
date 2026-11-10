#!/usr/bin/env python3
# Hostile input to every phase-17 command is refused with an error line, never a panic, and nothing changes: no stair,
# railing, room or opening row, and the built scene's digest is the same. (Non-finite numbers cannot be sent: JSON has
# no NaN, and the CLI refuses a non-JSON --cmd before running anything; 1e308 is finite and must be refused as a
# board direction.)
# needs: p17-observables, p17-commands, p17-window-tops
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

R = house_rooms()
S = R['S']
SETUP = HOUSE2 + [STAIR5, NORTH_DECK, cmd('railing', {"path": [[12, 0], [12, 3], [16, 3]]}), GRILLE22]
D = deck_room(inspected(run_ok(SETUP + [INSPECT17], "the set-up"))).get('id')
FREE = [[20, 0], [24, 0], [24, 3], [20, 3], [20, 0]]
H = [('stair', {"start": [2, 5], "direction": 0, "openRisers": "yes"}),
     ('stair', {"start": [2, 5], "direction": 0, "stringers": "three"}),
     ('stair', {"start": [2, 5], "direction": 0, "underside": 0.1}),
     ('stair', {"start": [2, 5], "direction": 0, "underside": 99}),
     ('stair', {"start": [2, 5], "direction": 0, "railLeft": "maybe"}),
     ('stair', {"start": [2, 5], "direction": 0, "planCut": -1}),
     ('stair.edit', {"id": "s9", "openRisers": True}),
     ('stair.edit', {"id": "s0"}),                                   # nothing to change
     ('stair.edit', {"id": "s0", "stringers": 2}),
     ('stair.edit', {"id": "s0", "underside": "deep"}),
     ('deck', {"points": FREE, "boards": "north"}),
     ('deck', {"points": FREE, "rails": ["auto"]}),                  # 4 edges need 4 entries
     ('deck', {"points": FREE, "rails": "sometimes"}),
     ('deck.edit', {"id": "r99", "boards": 0}),
     ('deck.edit', {"id": S, "boards": 0}),                          # not a deck
     ('deck.edit', {"id": D, "height": -1}),
     ('deck.edit', {"id": D, "boards": 1e308}),                      # finite, but |boards| > 360
     ('railing', {"path": [[0, 10], [4, 10]], "postSpacing": 0}),
     ('railing', {"path": [[0, 10], [4, 10]], "postSpacing": 99}),
     ('railing', {"path": [[0, 10], [4, 10]], "inset": 5}),
     ('window', {"wall": "w2", "offset": 2.5, "grille": {"across": 0, "down": 2}}),
     ('window', {"wall": "w2", "offset": 2.5, "grille": {"across": 99, "down": 2}}),
     ('window', {"wall": "w2", "offset": 2.5, "grille": {"across": 2, "down": 2, "part": "bottom"}}),
     ('window', {"wall": "w2", "offset": 2.5, "grille": "2x2"}),
     ('window', {"wall": "w2", "offset": 2.5, "top": "oval"}),
     ('window', {"wall": "w2", "offset": 2.5, "width": 0.9, "height": 1.2, "top": "segment", "topRise": 2.0}),
     ('window', {"wall": "w2", "offset": 2.5, "width": 0.9, "height": 0.3, "sill": 0.9, "top": "round"}),
     ('window', {"wall": "w0", "offset": 5, "top": "roof"}),         # no roof
     ('window', {"wall": "w2", "offset": 2.5, "roofOffset": -0.1}),
     ('door', {"wall": "w4", "offset": 2, "style": "revolving"}),
     ('door', {"wall": "w4", "offset": 2, "top": "round"}),
     ('window', {"wall": "w2", "offset": 2.5, "sillSides": "up"}),
     ('window', {"wall": "w2", "offset": 2.5, "casingSides": 3})]
DIGEST = cmd('build3d', {"digest": True})


def state(rows):
    insp = inspected(rows)
    digests = [r['digest'] for r in rows if isinstance(r, dict) and isinstance(r.get('digest'), str)]
    if not digests:
        fail(f"no build3d digest; replies: {json.dumps(rows)[-400:]}")
    return {'stairs': stairs_of(insp), 'railings': railings_of(insp), 'rooms': objects_named(insp, 'rooms', 'rooms'),
            'openings': openings_of(insp), 'digest': digests[-1]}


rc, rr, so, se = run(SETUP + [INSPECT17, DIGEST])
if errors(se):
    fail("the set-up is refused:\n" + '\n'.join(errors(se)))
before = state(rr)
if len(before['stairs']) != 1 or len(before['openings']) != 1:
    fail(f"the set-up places one stair and one window: {len(before['stairs'])}, {len(before['openings'])}")
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT17, DIGEST], expect_ok=False)
if 'panicked' in se:
    fail(f"a hostile command panicked: {se[-400:]}")
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:60]))
after = state(rr)
for key in ('stairs', 'railings', 'rooms', 'openings', 'digest'):
    if json.dumps(before[key], sort_keys=True) != json.dumps(after[key], sort_keys=True):
        fail(f"refused commands changed '{key}': {json.dumps(before[key])[:300]} -> {json.dumps(after[key])[:300]}")
print(f"ok: {len(H)} hostile commands refused; stairs, railings, rooms, openings and the scene unchanged")
print("PASS 12")
