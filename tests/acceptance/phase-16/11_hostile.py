#!/usr/bin/env python3
# Hostile input to every phase-16 command is refused with an error line, and nothing changes: no room's ceiling flag,
# stack or rough ceiling, no tray, soffit, foundation or level, and the built scene's digest is the same.
# (Non-finite numbers cannot be sent: JSON has no NaN, and the CLI refuses a non-JSON --cmd before running anything.)
# needs: p16-vault-ceiling, p16-trays, p16-soffits, p16-foundation-stem
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']
SETUP = HOUSE + [GABLE,
                 cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3}),
                 cmd('soffit', {"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1}),
                 cmd('foundation', {"type": "stem"})]
RECT = [[7.0, 7.3], [9.0, 7.9]]
H = [('room', {"id": S, "flatCeiling": "no"}),
     ('room', {"id": S, "ceilingStack": "plaster"}),
     ('room', {"id": S, "ceilingStack": [{"material": "drywall", "thickness": -0.01}]}),
     ('room', {"id": S, "ceilingStack": [{"material": None, "thickness": 0.1}]}),
     ('room', {"id": S, "ceilingStack": [{"material": "drywall", "thickness": 1.5}]}),
     ('room', {"id": S, "ceilingStack": [{"material": "drywall", "thickness": 0.01}] * 9}),
     ('room', {"id": S, "ceilingStack": {"material": "drywall"}}),
     ('room', {"id": NE, "ceiling": 3.2}),                      # above the roof underside (2.5 at y = 7.9)
     ('level.edit', {"level": 0, "ceiling": 2.6}),               # S would cross the roof at y = 0.1 (2.5)
     ('level', {"index": 0, "ceiling": 2.6}),                    # the `level` command's own edit path
     ('build3d', {"ceiling": [[1, "x"]]}),
     ('build3d', {"ceiling": "all"}),
     ('build3d', {"ceiling": [[1, 1]], "level": "Moon"}),
     ('tray', {"room": "r99", "inset": 0.6, "depth": 0.3}),
     ('tray', {"room": NW, "inset": -1, "depth": 0.3}),
     ('tray', {"room": NW, "inset": 0.6, "depth": 0}),
     ('tray', {"room": NW, "inset": 0.6, "depth": 0.5}),
     ('tray', {"room": NW, "inset": 3.5, "depth": 0.1}),         # no outline left
     ('tray', {"room": NW, "inset": 0.6, "depth": 0.1, "sides": "curved"}),
     ('tray', {"polygon": [[1, 4], [2, 4], [1, 5], [2, 5], [1, 4]], "depth": 0.1}),   # self-intersecting
     ('tray', {"tray": "tr9", "inset": 0.2, "depth": 0.1}),
     ('tray.edit', {"id": "tr0", "depth": 0.5}),
     ('tray.edit', {"id": "tr9", "depth": 0.1}),
     ('tray.delete', {"id": "r0"}),
     ('soffit', {"rect": RECT}),                                 # neither bottom nor depth
     ('soffit', {"rect": RECT, "bottom": 3.0}),                  # above the ceiling
     ('soffit', {"rect": RECT, "bottom": 2.1, "depth": 0.3}),
     ('soffit', {"over": ["cb7"]}),
     ('soffit', {"rect": [[20, 20], [21, 21]], "bottom": 2.1}),  # in no room
     ('soffit.edit', {"id": "sf9", "bottom": 2.0}),
     ('soffit.edit', {"id": "sf0", "bottom": "low"}),
     ('soffit.delete', {"id": "w0"}),
     ('foundation', {"type": "pier"}),
     ('foundation', {"type": "stem", "footingDepth": 0.2}),
     ('foundation', {"type": "stem", "footingDepth": "deep"}),
     ('foundation', {"type": "stem", "level": "Moon"}),
     ('foundation.edit', {"id": "fd9", "type": "slab"}),
     ('foundation.edit', {"id": "fd0", "footingDepth": 99}),
     ('foundation.delete', {"id": "sf0"})]
DIGEST = cmd('build3d', {"digest": True})


def state(rows):
    insp = inspected(rows)
    rooms = []
    for r in objects_named(insp, 'rooms', 'rooms'):
        for key in ('flatCeiling', 'ceilingStack', 'roughCeiling'):
            if key not in r:
                fail(f"room row {r.get('id')} has no '{key}'")
        rooms.append([r.get('id'), r['flatCeiling'], r['ceilingStack'], r['roughCeiling']])
    digests = [r['digest'] for r in rows if isinstance(r, dict) and isinstance(r.get('digest'), str)]
    if not digests:
        fail(f"no build3d digest; replies: {json.dumps(rows)[-400:]}")
    return {'rooms': rooms, 'trays': trays_of(insp), 'soffits': soffits_of(insp), 'foundations': foundations_of(insp),
            'levels': insp.get('levels'), 'digest': digests[-1]}


rc, rr, so, se = run(SETUP + [INSPECT16, DIGEST])
if errors(se):
    fail("the set-up is refused:\n" + '\n'.join(errors(se)))
before = state(rr)
for key in ('trays', 'soffits', 'foundations'):
    if len(before[key]) != 1:
        fail(f"the set-up places one of each: {key} = {json.dumps(before[key])[:300]}")
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT16, DIGEST], expect_ok=False)
if 'panicked' in se:
    fail(f"a hostile command panicked: {se[-400:]}")
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:60]))
after = state(rr)
for key in ('rooms', 'trays', 'soffits', 'foundations', 'levels', 'digest'):
    if json.dumps(before[key], sort_keys=True) != json.dumps(after[key], sort_keys=True):
        fail(f"refused commands changed '{key}': {json.dumps(before[key])[:300]} -> {json.dumps(after[key])[:300]}")
print(f"ok: {len(H)} hostile commands refused; rooms, trays, soffits, foundations, levels and the scene unchanged")
print("PASS 11")
