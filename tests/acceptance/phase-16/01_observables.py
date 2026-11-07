#!/usr/bin/env python3
# Observables first: `build3d {"ceiling": [[x, y], ...], "level"}` answers the finished ceiling over each point on that
# level (null outside every room) and every build3d reply counts the scene's parts; room rows report flatCeiling,
# ceilingStack, roughCeiling, finishedCeiling and ceilingWarning; a ceiling item hangs from the room's own ceiling height
# (it used to take the level's). A stub (no keys, zeros, the level's height everywhere) fails every assertion.
# needs: p16-observables
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
PTS = [[5, 1.5], [3, 5], [8, 5], [20, 20]]


def same_ceilings(got, want, what):
    if not isinstance(got, list) or len(got) != len(want):
        fail(f"{what}: expected {len(want)} answers, got {got!r}")
    for k, (g, w) in enumerate(zip(got, want)):
        if w is None:
            if g is not None:
                fail(f"{what}: point {PTS[k] if k < len(PTS) else k} is in no room, expected null, got {g!r}")
        else:
            near(g, w, 1e-6, f"{what}: ceiling at point #{k}")


# 1. The flat house, by level index and by level name; the parts; the room rows.
rc, rr, so, se = run(HOUSE + [ceiling_cmd(PTS, 0), ceiling_cmd(PTS, "Level 1"), INSPECT16])
same_ceilings(ceilings(rr, 0), [2.4, 2.4, 2.4, None], 'level 0')
same_ceilings(ceilings(rr, 1), [2.4, 2.4, 2.4, None], 'level "Level 1"')
parts = parts_of(rr)
if parts != {"ceiling": 3, "floor": 3, "wall": 6}:
    fail(f"build3d parts of the house: expected {{'ceiling': 3, 'floor': 3, 'wall': 6}}, got {parts}")
insp = inspected(rr)
for name, rid in R.items():
    row = room_row(insp, rid)
    for key in ('flatCeiling', 'ceilingStack', 'roughCeiling', 'finishedCeiling', 'ceilingWarning'):
        if key not in row:
            fail(f"room {name} ({rid}) row has no '{key}': {sorted(row)}")
    if row['flatCeiling'] is not True:
        fail(f"room {name}: flatCeiling defaults to true, got {row['flatCeiling']!r}")
    if row['ceilingStack'] != []:
        fail(f"room {name}: ceilingStack defaults to [], got {row['ceilingStack']!r}")
    near(row['roughCeiling'], 2.4, 1e-6, f"room {name} roughCeiling")
    near(row['finishedCeiling'], 2.4, 1e-6, f"room {name} finishedCeiling")
    if row['ceilingWarning'] is not None:
        fail(f"room {name}: ceilingWarning is null without a roof, got {row['ceilingWarning']!r}")

# 2. The room's own ceiling height: the query and a library ceiling item follow it (2.7, not the level's 2.4).
rc, rr, so, se = run(HOUSE + [cmd('room', {"id": R['NE'], "ceiling": 2.7}),
                              cmd('library.place', {"item": "light-pendant", "at": [8, 5]}),
                              ceiling_cmd([[8, 5], [3, 5]]), INSPECT16])
same_ceilings(ceilings(rr), [2.7, 2.4], "NE at 2.7")
fid = reply_with(rr, 'furnishing', -1, 'library.place').get('furnishing')
insp = inspected(rr)
near(item_top(insp, fid), 2.7, 1e-6, "the pendant's top (it hangs from NE's own 2.7 m ceiling)")
ne = room_row(insp, R['NE'])
near(ne.get('roughCeiling'), 2.7, 1e-6, "NE roughCeiling")
near(ne.get('finishedCeiling'), 2.7, 1e-6, "NE finishedCeiling")

# 3. A second storey: Level 2 at 2.7 (2.4 + 0.3) with the same walls and rooms; its ceiling is 2.7 + 2.4 = 5.1.
rc, rr, so, se = run(HOUSE + [cmd('level', {"above": 0})] + house_walls(1) +
                     [cmd('roomdetect', {"level": 1}), ceiling_cmd([[5, 1.5]], 1), ceiling_cmd([[5, 1.5]], "Level 2"),
                      ceiling_cmd([[5, 1.5]])])
same_ceilings(ceilings(rr, 0), [5.1], 'level 1')
same_ceilings(ceilings(rr, 1), [5.1], 'level "Level 2"')
same_ceilings(ceilings(rr, 2), [2.4], 'the default level 0')

# 4. Bad queries are refused with an error line, without a panic.
BAD = [{"ceiling": [[1, "x"]]}, {"ceiling": "all"}, {"ceiling": [[1, 1]], "level": "Moon"}]
rc, rr, so, se = run(HOUSE + [cmd('build3d', p) for p in BAD] + [INSPECT16], expect_ok=False)
errs = errors(se)
if len(errs) < len(BAD):
    fail(f"each of the {len(BAD)} bad build3d queries is refused with an error line; got {len(errs)}:\n" + '\n'.join(errs))
if any(isinstance(r, dict) and isinstance(r.get('ceiling'), list) for r in rr):
    fail(f"a refused query still answered: {json.dumps(rr)[-400:]}")
print("ok: ceiling queries 2.4/null, by index and name; parts ceiling 3, floor 3, wall 6; NE 2.7 and its pendant; level 2 5.1; "
      "bad queries refused; room rows")
print("PASS 01")
