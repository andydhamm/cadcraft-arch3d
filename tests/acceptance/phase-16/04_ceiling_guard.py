#!/usr/bin/env python3
# Ceilings that follow the roof (the guard half): flatCeiling defaults on and a room under a roof carries no warning; a
# vaulted room is re-pitched with its roof (6/12 -> 8/12 -> 6/12 at (5,2): 3.45 -> 2.4 + (8/12) 2.1 = 3.8 -> 3.45, and
# its 20 mm ceiling solid sits under the underside). A room or level edit that would put a flat ceiling above the roof
# underside is refused with an error naming the room and writes nothing (room NE at 3.2 m, room S at 2.6 m, level 0's
# ceiling at 2.6 m through `level.edit` and through the `level` command's own edit path: the underside is 2.5 m at the
# rooms' eave sides). A roof edit is never refused: after S is raised to 2.45 m, a 2/12 roof drops the underside below
# S's flat ceiling near the south wall, S's row carries a ceilingWarning and its ceiling follows the underside there
# (2.4 + 0.25/6 = 2.441667 at (5,0.15)), while it stays 2.45 at (5,2) and NW stays 2.4.
# A stub fails: no refusal, no warning, and 2.4 or the flat height everywhere.
# needs: p16-vault-ceiling
import os, re
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']


def names(err, rid):
    return re.search(rf'(?<![A-Za-z0-9_]){re.escape(rid)}(?![A-Za-z0-9_])', err) is not None


# 1. The defaults under the gable: flat, no warning.
insp = inspected(run_ok(HOUSE + [GABLE, INSPECT16], "the house under the gable"))
for name, rid in R.items():
    row = room_row(insp, rid)
    if row.get('flatCeiling') is not True:
        fail(f"room {name} ({rid}): flatCeiling defaults to true, got {row.get('flatCeiling')!r}")
    if 'ceilingWarning' not in row or row['ceilingWarning'] is not None:
        fail(f"room {name} ({rid}): ceilingWarning is null under the 6/12 gable, got {row.get('ceilingWarning', '(missing)')!r}")
print("ok: every room is flat by default and carries no warning under the gable")

# 2. A vault is re-pitched with its roof: 6/12 -> 8/12 -> 6/12.
P = [[5, 2]]
rr = run_ok(HOUSE + [cmd('room', {"id": S, "flatCeiling": False}), GABLE, ceiling_cmd(P), gable(8), ceiling_cmd(P), GABLE,
                     cmd('build3d', {"ceiling": P, "probe": [[5, 2, 3.44], [5, 2, 3.40]]})], "S vaulted, re-pitched")
expect_ceilings(rr, P, [3.45], "S vaulted under the 6/12 gable", nth=0)
expect_ceilings(rr, P, [2.4 + (8 / 12) * 2.1], "S re-pitched by an 8/12 roofauto (2.4 + (8/12) 2.1 = 3.8)", nth=1)
expect_ceilings(rr, P, [3.45], "S back under the 6/12 gable", nth=2)
expect_hits(rr, [([5, 2, 3.44], ('ceiling', S)), ([5, 2, 3.40], None)], "the vault's 20 mm solid under the underside at (5,2)")
print("ok: S's vault follows the roof: 3.45 -> 3.8 -> 3.45 at (5,2); its solid is 3.43..3.45 there")


# 3. Edits that would raise a flat ceiling above the underside are refused and write nothing.
def refused_step(step, what):
    rc, rr, so, se = run(HOUSE + [GABLE, step, INSPECT16], expect_ok=False)
    errs = errors(se)
    if len(errs) != 1:
        fail(f"{what} is refused with one error line (the set-up runs): {errs or '(no error line)'}")
    return errs[0], inspected(rr)


err, insp = refused_step(cmd('room', {"id": NE, "ceiling": 3.2}), f"room NE ({NE}) at 3.2 m (the underside is 2.5 at y = 7.9)")
if not names(err, NE):
    fail(f"the refusal names the room {NE}: {err}")
near(room_row(insp, NE).get('roughCeiling'), 2.4, 1e-6, "NE's roughCeiling after the refused edit")
err, insp = refused_step(cmd('room', {"id": S, "ceiling": 2.6}), f"room S ({S}) at 2.6 m (the underside is 2.5 at y = 0.1)")
if not names(err, S):
    fail(f"the refusal names the room {S}: {err}")
near(room_row(insp, S).get('roughCeiling'), 2.4, 1e-6, "S's roughCeiling after the refused edit")
for step, what in ((cmd('level.edit', {"level": 0, "ceiling": 2.6}), "level.edit {level 0, ceiling 2.6}"),
                   (cmd('level', {"index": 0, "ceiling": 2.6}), "level {index 0, ceiling 2.6} (the level command's edit path)")):
    err, insp = refused_step(step, what)
    if not any(names(err, rid) for rid in (S, NW, NE)):
        fail(f"{what}: the refusal names the room whose flat ceiling would cross the roof ({S}, {NW} or {NE}): {err}")
    near(level_row(insp, 0).get('ceiling'), 2.4, 1e-6, f"level 0's ceiling after the refused {what}")
    for name, rid in R.items():
        near(room_row(insp, rid).get('roughCeiling'), 2.4, 1e-6, f"room {name}'s roughCeiling after the refused {what}")
print("ok: room NE 3.2, room S 2.6, level.edit and level {index} ceiling 2.6 are refused naming the room; nothing changes")

# 4. Roof edits are never refused: a 2/12 roof under S's raised flat ceiling warns and the ceiling follows the underside.
PTS = [[5, 0.15], [5, 2], [3, 5]]
rr = run_ok(HOUSE + [GABLE, cmd('room', {"id": S, "ceiling": 2.45}), gable(2), INSPECT16,
                     cmd('build3d', {"ceiling": PTS, "probe": [[5, 0.15, 2.43], [5, 0.15, 2.425]]})],
            "S raised to 2.45 m, then a 2/12 roof (never refused)")
insp = inspected(rr)
w = room_row(insp, S).get('ceilingWarning')
if not (isinstance(w, str) and w.strip()):
    fail(f"S's row carries a ceilingWarning once the 2/12 roof drops below its flat 2.45 m ceiling, got {w!r}")
for name in ('NW', 'NE'):
    if room_row(insp, R[name]).get('ceilingWarning') is not None:
        fail(f"room {name}: no warning (the 2/12 underside is 2.4333 at y = 7.9, above 2.4): "
             f"{room_row(insp, R[name]).get('ceilingWarning')!r}")
expect_ceilings(rr, PTS, [2.4 + 0.25 / 6, 2.45, 2.4], "S under the 2/12 roof: the underside at y = 0.15, flat 2.45 at y = 2; NW 2.4")
# The solid follows the underside there: 2.421667..2.441667 at y = 0.15 (2.425 is inside it, not inside the unclipped
# 2.43..2.45 slab, so a ceiling clipped only in the point query fails).
expect_hits(rr, [([5, 0.15, 2.43], ('ceiling', S)), ([5, 0.15, 2.425], ('ceiling', S))],
            "S's ceiling solid under the 2/12 underside at (5,0.15)")
print(f"ok: the 2/12 roof is accepted; S warns ({w[:80]!r}) and its ceiling follows the underside: 2.441667 at (5,0.15)")
print("PASS 04")
