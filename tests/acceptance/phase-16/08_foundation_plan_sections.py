#!/usr/bin/env python3
# FOUNDATION builds under every exterior wall with no level below it. The phase-1 house: one stem foundation on level 0
# (footingDepth 1.2 by default, not a basement) under exactly its four exterior walls (never the interior ones): four
# stem walls and four footings, dashed in the level's plan and filled in a section across the house; a second FOUNDATION
# on the level updates it in place (footingDepth 1.5 moves the footing to -1.5..-1.25). A basement drawn under the whole
# house takes the foundation instead (its walls become "Concrete 0.2", keeping their thickness; footings only, no stem),
# and a studio on grade higher up (elevation 1.35, no level below it) gets its own: stem 0.4..1.05, footing 0.15..0.4.
# A stub fails: no foundation command or rows, no parts, an unchanged plan, no foundation rings in the section.
# needs: p16-foundation-stem
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

tmp = tempfile.mkdtemp()
STEM = cmd('foundation', {"type": "stem"})


def foundation_reply(rows, nth=-1):
    hits = replies_with_list(rows, 'foundations')
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no foundation reply (#{nth}): {json.dumps(rows)[-400:]}")
    return hits[nth]


# 1. The house: one stem foundation under w0..w3.
rr = run_ok(HOUSE + [STEM, cmd('build3d', {}), INSPECT16], "foundation {type stem} under the house")
rep = foundation_reply(rr)
fds = rep.get('foundations')
if not (isinstance(fds, list) and len(fds) == 1):
    fail(f"one foundation for level 0: {json.dumps(rep)[:300]}")
FD = fds[0]
insp = inspected(rr)
rows = foundations_of(insp)
if [f.get('id') for f in rows] != [FD]:
    fail(f"arch.inspect lists {FD} alone: {json.dumps(rows)[:300]}")
row = rows[0]
if not on_level(row.get('level'), level_row(insp, 0)) or row.get('type') != 'stem' or row.get('basement') is not False:
    fail(f"{FD}'s row: level 0, type stem, basement false: {json.dumps(row)[:300]}")
near(row.get('footingDepth'), 1.2, 1e-9, f"{FD}'s footingDepth (the default)")
if not isinstance(row.get('walls'), list) or sorted(map(str, row['walls'])) != ['w0', 'w1', 'w2', 'w3']:
    fail(f"{FD} founds exactly the four exterior walls w0..w3 (not w4, w5): {row.get('walls')!r}")
if row.get('layer') != 'S-FNDN':
    fail(f"{FD} is on layer S-FNDN: {row.get('layer')!r}")
parts = parts_of(rr)
if parts.get('foundation') != 4 or parts.get('footing') != 4:
    fail(f"four stem walls and four footings: parts {parts}")
print(f"ok: {FD} on level 0, stem, footingDepth 1.2, walls w0..w3; parts foundation 4, footing 4")

# 2. The plan: the footings draw (dashed) on the level they support.
a, b = os.path.join(tmp, 'plain.png'), os.path.join(tmp, 'footings.png')
run_ok(HOUSE + [cmd('export', {"path": a})], "the plan without a foundation")
run_ok(HOUSE + [STEM, cmd('export', {"path": b})], "the plan with the foundation")
if not (os.path.exists(a) and os.path.exists(b)):
    fail("the plan exports were written")
if digest(a) == digest(b) or luma_sd(b) <= 1:
    fail(f"the plan with the footings differs from the plan without them and is not blank (luma sd {luma_sd(b):.2f})")
artifact(b, 'foundation-plan.png')
print("ok: the plan with the footings differs from the plan without them")

# 3. A section across the house cuts the foundations.
rr = run_ok(HOUSE + [STEM, cmd('section', {"from": [5, -2], "to": [5, 10]}), INSPECT16], "a section across the house")
views = objects_named(inspected(rr), 'views', 'views')
kinds = sorted({str(r.get('kind')) for v in views for r in (v.get('rings') or []) if isinstance(r, dict)})
if 'foundation' not in kinds:
    fail(f"the section's rings include the cut foundations (kind foundation): ring kinds {kinds}")
print(f"ok: the section's rings include kind foundation ({kinds})")

# 4. FOUNDATION again on the level updates it in place.
rr = run_ok(HOUSE + [STEM, cmd('foundation', {"type": "stem", "footingDepth": 1.5}),
                     probe_cmd([[5, 0.25, -1.4], [5, 0.25, -1.1]]), INSPECT16], "foundation {footingDepth 1.5} on the same level")
if foundation_reply(rr).get('foundations') != [FD]:
    fail(f"the second FOUNDATION updates {FD} in place: {json.dumps(foundation_reply(rr))[:300]}")
rows = foundations_of(inspected(rr))
if [f.get('id') for f in rows] != [FD]:
    fail(f"still one foundation, {FD}: {json.dumps(rows)[:300]}")
near(rows[0].get('footingDepth'), 1.5, 1e-9, f"{FD}'s footingDepth after the update")
expect_hits(rr, [([5, 0.25, -1.4], ('footing', FD)), ([5, 0.25, -1.1], None)], "the footing at -1.5..-1.25")
print(f"ok: {FD} updated in place: the footing at (5,0.25,-1.4), nothing at -1.1")

# 5. A basement under the whole house takes the foundation: Concrete 0.2 walls on footings, no stem walls.
BASEMENT = [cmd('level', {"name": "Basement", "elevation": -2.7}),
            cmd('wall', dict(HOUSE_WALLS[0], level="Basement")),
            cmd('roomdetect', {"level": "Basement"})]
rr = run_ok(HOUSE + BASEMENT + [STEM, cmd('build3d', {}), INSPECT16], "a basement, then a stem foundation")
insp = inspected(rr)
base = level_row(insp, 'Basement')
rows = foundations_of(insp)
if len(rows) != 1 or not on_level(rows[0].get('level'), base) or rows[0].get('basement') is not True:
    fail(f"one foundation, on the Basement level, basement true: {json.dumps(rows)[:300]}")
bwalls = [w for w in objects_named(insp, 'walls', 'walls') if w.get('level') == base.get('index')]
if len(bwalls) != 4:
    fail(f"four Basement walls: {[(w.get('id'), w.get('level')) for w in objects_named(insp, 'walls', 'walls')]}")
for w in bwalls:
    if w.get('kind') != 'Concrete 0.2':
        fail(f"basement wall {w.get('id')} takes the Concrete 0.2 type: {w.get('kind')!r}")
    near(w.get('thickness'), 0.2, 1e-9, f"basement wall {w.get('id')} keeps its drawn thickness")
conc = foundation_reply(rr).get('concrete')
if not isinstance(conc, list) or sorted(map(str, conc)) != sorted(str(w.get('id')) for w in bwalls):
    fail(f"the reply lists the retyped walls {sorted(w.get('id') for w in bwalls)}: {conc!r}")
parts = parts_of(rr)
if parts.get('footing') != 4 or 'foundation' in parts:
    fail(f"a basement gets four footings and no stem walls: parts {parts}")
print("ok: the basement takes the foundation: its four walls are Concrete 0.2, four footings, no stem")

# 6. A studio on grade higher up gets its own foundation.
STUDIO = [cmd('level', {"name": "Studio", "elevation": 1.35}),
          cmd('wall', {"points": [[20, 0], [24, 0], [24, 4], [20, 4], [20, 0]], "thickness": 0.2, "height": 2.4,
                       "exterior": True, "level": "Studio"})]
rr = run_ok(HOUSE + STUDIO + [STEM, probe_cmd([[22, 0.0, 0.7], [22, 0.25, 0.3]]), INSPECT16], "a studio on grade at 1.35")
insp = inspected(rr)
rows = foundations_of(insp)
studio = [f for f in rows if on_level(f.get('level'), level_row(insp, 'Studio'))]
ground = [f for f in rows if on_level(f.get('level'), level_row(insp, 0))]
if len(rows) != 2 or len(studio) != 1 or len(ground) != 1:
    fail(f"two foundations, one on level 0 and one on Studio: {json.dumps(rows)[:400]}")
sid = studio[0].get('id')
expect_hits(rr, [([22, 0.0, 0.7], ('foundation', sid)), ([22, 0.25, 0.3], ('footing', sid))],
            "the studio's stem wall (0.4..1.05) and footing (0.15..0.4)")
print(f"ok: the studio on grade gets {sid}: stem at (22,0,0.7), footing at (22,0.25,0.3)")
print("PASS 08")
