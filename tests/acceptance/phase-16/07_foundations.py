#!/usr/bin/env python3
# Foundations, the brief's numbers (level 1 at 0, structure 0.3). Stem: under the south wall (outer face y = -0.1) the
# 0.2 m stem wall is flush with the outer face, y -0.1..0.1, from the footing's top (-1.2 + 0.25 = -0.95) up to the floor
# structure's underside (-0.3), on a 0.6 x 0.25 footing centred under it (y -0.3..0.3, z -1.2..-0.95): probes at x = 5
# hit the foundation wall at (5,0.0,-0.5), the footing at (5,0.25,-1.0), nothing at (5,0.25,-0.5) or (5,0.35,-1.0).
# With a basement at -2.7 (structure 0.3) whose walls lie on the same centre lines, its footing sits directly under the
# basement's floor structure (z -3.25..-3.0, centred under the wall): hit at (5,0.25,-3.1). Slab: the floors become
# 0.1 m slabs (hit at (5,1.0,-0.05), part slab) with a thickened edge 0.3 m wide inward from the outer face reaching 0.3 m
# below the slab's top (y -0.1..0.2, z -0.3..-0.1: hit at (5,0.15,-0.2), part footing; nothing at (5,0.25,-0.2)); three
# slab floors, four edges, no stem walls.
# A stub fails: with nothing built every probe is empty (and a 0.3 m floor answers part floor, not slab, at -0.05 and
# -0.2).
# needs: p16-foundation-stem, p16-foundation-slab
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S = R['S']
STEM = cmd('foundation', {"type": "stem"})
FD = made(HOUSE + [STEM], 'foundations', "foundation {type stem} under the house")
if not (isinstance(FD, list) and len(FD) == 1 and isinstance(FD[0], str)):
    fail(f"one foundation for the one founded level: {FD!r}")
FD = FD[0]

# 1. Stem.
rr = run_ok(HOUSE + [STEM, probe_cmd([[5, 0.0, -0.5], [5, 0.25, -1.0], [5, 0.25, -0.5], [5, 0.35, -1.0]])], "a stem foundation")
expect_hits(rr, [([5, 0.0, -0.5], ('foundation', FD)), ([5, 0.25, -1.0], ('footing', FD)), ([5, 0.25, -0.5], None),
                 ([5, 0.35, -1.0], None)], "the stem wall (y -0.1..0.1, z -0.95..-0.3) and its footing (y -0.3..0.3, z -1.2..-0.95)")
print(f"ok: {FD}: the stem wall at (5,0,-0.5), the footing at (5,0.25,-1.0), nothing beside the stem or past the footing")

# 2. A basement under the whole house: its footing is under the basement's floor structure.
BASEMENT = [cmd('level', {"name": "Basement", "elevation": -2.7}),
            cmd('wall', dict(HOUSE_WALLS[0], level="Basement")),
            cmd('roomdetect', {"level": "Basement"})]
rr = run_ok(HOUSE + BASEMENT + [STEM, probe_cmd([[5, 0.25, -3.1]]), INSPECT16], "a basement, then a stem foundation")
insp = inspected(rr)
b = level_row(insp, 'Basement')
near(b.get('elevation'), -2.7, 1e-9, "the Basement's elevation")
near(b.get('structure'), 0.3, 1e-9, "the Basement's floor structure")
fds = foundations_of(insp)
if len(fds) != 1:
    fail(f"one foundation (the basement's; level 1's walls all have a level below): {json.dumps(fds)[:300]}")
expect_hits(rr, [([5, 0.25, -3.1], ('footing', fds[0].get('id')))], "the basement footing (z -3.25..-3.0, y -0.3..0.3)")
print("ok: the basement's footing at (5,0.25,-3.1)")

# 3. Slab.
SLAB_PTS = [[5, 1.0, -0.05], [5, 0.15, -0.2], [5, 0.25, -0.2]]
rr = run_ok(HOUSE + [cmd('foundation', {"type": "slab"}), probe_cmd(SLAB_PTS), INSPECT16], "a slab foundation")
fd = [f.get('id') for f in foundations_of(inspected(rr))]
if len(fd) != 1:
    fail(f"one slab foundation: {fd}")
expect_hits(rr, [(SLAB_PTS[0], ('slab', S)), (SLAB_PTS[1], ('footing', fd[0])), (SLAB_PTS[2], None)],
            "the 0.1 m slab and its thickened edge (y -0.1..0.2, z -0.3..-0.1)")
parts = parts_of(rr)
if parts.get('slab') != 3 or parts.get('footing') != 4 or 'foundation' in parts:
    fail(f"a slab foundation: parts.slab 3, parts.footing 4, no foundation part; got {parts}")
print("ok: the slab at (5,1.0,-0.05), its thickened edge at (5,0.15,-0.2), nothing at (5,0.25,-0.2); slab 3, footing 4")
print("PASS 07")
