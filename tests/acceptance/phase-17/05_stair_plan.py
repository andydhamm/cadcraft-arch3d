#!/usr/bin/env python3
# The stair plan symbol replaces the outline and riser lines. On the lower level the phase-5 stair (16 risers of
# 0.16875) draws its nosing lines solid up to the 1.2 m plan cut (7 treads: 7 x 0.16875 = 1.18125), a diagonal break
# line, the 8 nosing lines beyond dashed, and a walk arrow labelled UP; on the upper level the part seen through the
# well is solid and labelled DN. All of it is centred on `start` (each nosing line spans y 0.5..1.5, the first at
# x 1.975) and reaches the DXF export as CADCRAFT-STAIR proxies on the stair's layer (dashed ones with linetype DASHED).
# planCut 1.5 moves the break (8 solid, 7 dashed). The two level plans render differently and not blank.
# A stub (the old outline and riser lines, or no proxies) fails.
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-stair-plan
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

tmp = tempfile.mkdtemp()
l1, l2, p1, p2, l1b = (os.path.join(tmp, n) for n in ('l1.dxf', 'l2.dxf', 'l1.png', 'l2.png', 'l1-cut15.dxf'))
rr = run_ok(HOUSE2 + [STAIR5, cmd('export', {"path": l1, "level": 0}), cmd('export', {"path": l2, "level": 1}),
                      cmd('export', {"path": p1, "level": 0}), cmd('export', {"path": p2, "level": 1})], "the two level plans")
for p in (l1, l2, p1, p2):
    if not os.path.exists(p):
        fail(f"{os.path.basename(p)} was written")


def check_lower(path, solid_n, what):
    prox = stair_proxies(path)
    if not prox:
        fail(f"{what}: the DXF has no CADCRAFT-STAIR proxy entity")
    layers = {p['layer'] for p in prox}
    if layers != {'A-STAIR'}:
        fail(f"{what}: every stair proxy is on the stair's layer A-STAIR: {layers}")
    solid, dashed = nosings(prox, False), nosings(prox, True)
    if len(solid) != solid_n or len(dashed) != 15 - solid_n:
        fail(f"{what}: {solid_n} solid and {15 - solid_n} dashed nosing lines, got {len(solid)} solid, {len(dashed)} dashed "
             f"(roles: {sorted(set(p['role'] for p in prox))})")
    xs = sorted(line_x(p, what) for p in solid)
    for k, x in enumerate(xs):
        near(x, 1.975 + 0.254 * k, 1e-6, f"{what}: solid nosing line {k + 1} x")
    for p in solid + dashed:
        ys = sorted(q[1] for q in p['points'])
        near(ys[0], 0.5, 1e-6, f"{what}: a nosing line's low end (centred on start)")
        near(ys[-1], 1.5, 1e-6, f"{what}: a nosing line's high end")
    xd = sorted(line_x(p, what) for p in dashed)
    for k, x in enumerate(xd):
        near(x, 1.975 + 0.254 * (solid_n + k), 1e-6, f"{what}: dashed nosing line {solid_n + k + 1} x")
    brk = [p for p in prox if p['role'] == 'break']
    if len(brk) != 1:
        fail(f"{what}: one break line, got {len(brk)}")
    bx = [q[0] for q in brk[0]['points']]
    lo, hi = 1.975 + 0.254 * (solid_n - 1), 1.975 + 0.254 * solid_n
    if not bx or min(bx) <= lo + 1e-9 or max(bx) >= hi - 1e-9:
        fail(f"{what}: the break line lies between the last solid ({lo:.3f}) and the first dashed ({hi:.3f}) nosing line: x {bx}")
    lab = labels(prox)
    if 'UP' not in lab or 'DN' in lab:
        fail(f"{what}: the lower plan is labelled UP (and not DN): {lab}")
    if not any(p['role'] == 'arrow' for p in prox):
        fail(f"{what}: the lower plan has a walk arrow: roles {sorted(set(p['role'] for p in prox))}")


check_lower(l1, 7, "the level-1 plan")
print("ok: level 1: 7 solid nosing lines at x 1.975 + 0.254 k, 8 dashed, one break line between them, UP, an arrow; all on A-STAIR, y 0.5..1.5")

prox = stair_proxies(l2)
if not prox:
    fail("the level-2 plan has CADCRAFT-STAIR proxies (the part seen through the well)")
if nosings(prox, True):
    fail(f"the level-2 plan has no dashed nosing line: {len(nosings(prox, True))}")
if not nosings(prox, False):
    fail("the level-2 plan has solid nosing lines (the treads seen through the well)")
lab = labels(prox)
if 'DN' not in lab or 'UP' in lab:
    fail(f"the level-2 plan is labelled DN (and not UP): {lab}")
print(f"ok: level 2: {len(nosings(prox, False))} solid nosing lines through the well, DN, nothing dashed")

if digest(p1) == digest(p2) or luma_sd(p1) <= 1 or luma_sd(p2) <= 1:
    fail(f"the two level plans differ and are not blank (luma sd {luma_sd(p1):.2f}, {luma_sd(p2):.2f})")
artifact(p1, 'stair-plan-level-1.png')
artifact(p2, 'stair-plan-level-2.png')
print("ok: the two plan images differ and are not blank")

run_ok(HOUSE2 + [STAIR5, cmd('stair.edit', {"id": "s0", "planCut": 1.5}), cmd('export', {"path": l1b, "level": 0})], "planCut 1.5")
check_lower(l1b, 8, "the level-1 plan with planCut 1.5")
print("ok: planCut 1.5: 8 solid, 7 dashed")
print("PASS 05")
