#!/usr/bin/env python3
# Railings built as built (an empty drawing, level 0 at 0): path (0,0) (0,3) (4,3) (4,0) at the default spacing 1.8 gives
# 90 mm posts at the four path points and at (0,1.5), (1.3333,3), (2.6667,3), (4,1.5) (equal bays at most 1.8 apart),
# a top rail 0.864..0.914, a bottom rail 0.05 above the walking surface (0.05..0.10) and a panel between: probes
# (1.3333,3,0.5) post, (2,3,0.89) and (2,3,0.07) and (2,3,0.5) railing, (2,3,0.95) and (2,3,0.03) nothing; parts.post 8.
# postSpacing 1.0 on a 4 m path gives 5 posts; inset 0.2 moves everything 0.2 left of travel; balusters leave clear
# gaps under 101.6 mm. A stub (the phase-5 one-piece railing) fails: no post part, a solid at (2,3,0.03).
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-railing-parts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

PATH = [[0, 0], [0, 3], [4, 3], [4, 0]]
POSTS = [[0, 0], [0, 1.5], [0, 3], [4 / 3, 3], [8 / 3, 3], [4, 3], [4, 1.5], [4, 0]]
P = [[4 / 3, 3, 0.5], [2, 3, 0.89], [2, 3, 0.07], [2, 3, 0.5], [2, 3, 0.95], [2, 3, 0.03]]

# (a) The brief's railing.
rr = run_ok([cmd('railing', {"path": PATH}), INSPECT17, probe_cmd(P)], "the brief's railing")
r = railings_of(inspected(rr))
if len(r) != 1:
    fail(f"one railing: {[x.get('id') for x in r]}")
same_points(r[0].get('posts'), POSTS, 1e-3, "the railing's posts (ends, corners and equal bays at most 1.8 apart)")
expect_hits(rr, [(P[0], 'post'), (P[1], 'railing'), (P[2], 'railing'), (P[3], 'railing'), (P[4], None), (P[5], None)],
            "a post at (1.3333,3), the top rail 0.864..0.914, the bottom rail 0.05..0.10, the panel between, air above and below")
parts = parts_of(rr)
if parts.get('post') != 8:
    fail(f"parts.post is 8, got {parts.get('post')} in {parts}")
if not parts.get('railing'):
    fail(f"the rails and the panel are part railing: {parts}")
print("ok: 8 posts at the path points and (0,1.5), (1.333,3), (2.667,3), (4,1.5); rails at 0.05..0.10 and 0.864..0.914; a panel")

# (b) Spacing 1.0.
rr = run_ok([cmd('railing', {"path": [[0, 0], [4, 0]], "postSpacing": 1.0}), INSPECT17, cmd('build3d', {})], "spacing 1.0")
if parts_of(rr).get('post') != 5:
    fail(f"a 4 m railing at spacing 1.0 has 5 posts: parts {parts_of(rr)}")
same_points(railings_of(inspected(rr))[0].get('posts'), [[0, 0], [1, 0], [2, 0], [3, 0], [4, 0]], 1e-6, "posts at spacing 1.0")
print("ok: spacing 1.0 on 4 m: 5 posts")

# (c) Inset 0.2 (left of travel along +x is +y).
rr = run_ok([cmd('railing', {"path": [[0, 0], [4, 0]], "inset": 0.2}), INSPECT17, probe_cmd([[4 / 3, 0.2, 0.5], [4 / 3, 0.0, 0.5]])], "inset 0.2")
same_points(railings_of(inspected(rr))[0].get('posts'), [[0, 0.2], [4 / 3, 0.2], [8 / 3, 0.2], [4, 0.2]], 1e-3, "posts moved 0.2 left of travel")
expect_hits(rr, [([4 / 3, 0.2, 0.5], 'post'), ([4 / 3, 0.0, 0.5], None)], "the built railing follows the inset path")
print("ok: inset 0.2: posts and parts at y 0.2, nothing on the path itself")

# (d) Balusters: the first bay (post faces 0.045 .. 1.2883) sampled every 5 mm at z 0.5 shows gaps, none over 101.6 mm.
xs = [round(0.05 + 0.005 * k, 4) for k in range(int((1.28 - 0.05) / 0.005) + 1)]
rr = run_ok([cmd('railing', {"path": [[0, 0], [4, 0]], "style": "balusters"}), cmd('build3d', {"probe": [[x, 0.0, 0.5] for x in xs]})], "balusters")
got = probed(rr, [[x, 0.0, 0.5] for x in xs], -1, "balusters")
runs, cur = [], None
for x, g in zip(xs, got):
    kind = None if g is None else part(g)
    if cur and cur[0] == kind:
        cur[2] = x
    else:
        cur = [kind, x, x]
        runs.append(cur)
gaps = [r for r in runs if r[0] is None]
bars = [r for r in runs if r[0] == 'railing']
if len(gaps) < 3 or len(bars) < 3:
    fail(f"balusters alternate with gaps along the first bay: {[(k, a, b) for k, a, b in runs][:12]}")
longest = max(b - a + 0.005 for _, a, b in gaps)
if longest > 0.1016 + 0.006:
    fail(f"every clear gap between balusters is under 101.6 mm (+ the 5 mm sampling): the longest is {longest:.4f}")
print(f"ok: balusters: {len(bars)} bars and {len(gaps)} gaps in the first bay, the longest gap {longest:.3f} m")
print("PASS 06")
