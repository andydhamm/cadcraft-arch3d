#!/usr/bin/env python3
# Automatic railings: a new stair gets a Railing on each side that is not against a wall (host stairLeft / stairRight,
# auto true, following the stair, from the first riser line to the top riser, posts 0.045 inside the flight's edge, a
# newel at each end, rails sloping with the nosing line) and a new deck gets one on its free edges (host deck, following
# the deck room, posts on the rim joists, ends pulled 0.045 back from the house wall). Per side automatic (default), on
# or off through stair.edit railLeft/railRight and deck.edit rails; drawn railings survive every edit; a drawing saved
# with its rails off reopens with only its well guard (nothing is added on load or build).
# Numbers (phase-5 stair, rh 0.16875, td 0.254): side posts at x 2.0, 3.27, 4.54, 5.81 (3 bays of 1.27), left at
# y 1.455; at x 4.0 the nosing line is 1.5141 so the top rail spans 2.3781..2.4281; the post at x 3.27 stands on tread
# 6 (1.0125) and reaches 1.9431; the bottom newel stands on tread 1 (0.16875) and reaches nosing_v(0) + 0.914 + 0.1 =
# 1.1994. The north deck's railing: path (6,8.145) (6,11.1) (2,11.1) (2,8.145) inset 0.045 inside, posts at the
# corners, (5.955,9.6), (4.6517,11.055), (3.3483,11.055), (2.045,9.6); its posts reach down to the rim bottom 0.691.
# A stub (no automatic railings) fails (a).
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-railing-parts, p17-railing-auto
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

tmp = tempfile.mkdtemp()

# (a) The phase-5 stair in the open: two side railings and the well guard.
P = [[4.0, 1.455, 2.40], [4.0, 1.455, 2.46], [3.27, 1.455, 1.5], [2.0, 1.455, 1.1], [2.0, 1.455, 1.25]]
rr = run_ok(HOUSE2 + [STAIR5, INSPECT17, probe_cmd(P)], "the phase-5 stair with its automatic railings")
insp = inspected(rr)
sr = reply_with(rr, 'risers', -1, 'stair')
left = one_railing(insp, 'stairLeft', 's0', "the left side railing")
right = one_railing(insp, 'stairRight', 's0', "the right side railing")
one_railing(insp, 'well', 's0', "the well guard")
XS = [2.0, 3.27, 4.54, 5.81]
same_points(left.get('path'), [[2, 1.5], [5.81, 1.5]], 1e-6, "the left railing's path (the flight's left edge, first riser to top riser)")
same_points(left.get('posts'), [[x, 1.455] for x in XS], 1e-3, "the left railing's posts (inset 0.045 into the flight)")
same_points(right.get('posts'), [[x, 0.545] for x in XS], 1e-3, "the right railing's posts")
for r, name in ((left, 'left'), (right, 'right')):
    if r.get('auto') is not True or r.get('level') != 0:
        fail(f"the {name} railing is automatic (auto true) on level 0: {json.dumps(r)[:300]}")
    near(r.get('height'), 0.914, 1e-9, f"the {name} railing's height")
if sorted(sr.get('railings') or []) != sorted([left['id'], right['id']]):
    fail(f"the stair reply lists its two automatic railings {left['id']}, {right['id']}: {sr.get('railings')!r}")
expect_hits(rr, [(P[0], 'railing'), (P[1], None), (P[2], 'post'), (P[3], 'post'), (P[4], None)],
            "the sloped top rail (2.3781..2.4281 at x 4.0), the post at x 3.27 (1.0125..1.9431), the bottom newel (0.16875..1.1994)")
print(f"ok: {left['id']} (stairLeft) and {right['id']} (stairRight) follow s0: posts at x 2.0, 3.27, 4.54, 5.81; the rails slope with the flight")

# (b) Against a wall: one side; stair.edit turns sides on and off; a drawn railing survives.
DRAWN = cmd('railing', {"path": [[12, 0], [14, 0]]})
rr = run_ok(HOUSE2 + [stair_cmd(start=[2, 0.6]), DRAWN, INSPECT17, cmd('stair.edit', {"id": "s0", "railRight": "on"}), INSPECT17,
                      cmd('stair.edit', {"id": "s0", "railLeft": "off"}), INSPECT17], "a stair against the south wall")
for k, want in enumerate((['stairLeft'], ['stairLeft', 'stairRight'], ['stairRight'])):
    insp = inspected(rr, k)
    hosts = sorted(r.get('host') for r in railings_of(insp) if r.get('follows') == 's0' and r.get('host') != 'well')
    if hosts != sorted(want):
        fail(f"step {k}: the stair's side railings are {want}, got {hosts}")
    if len(railings_by_host(insp, None)) != 1 or len(railings_by_host(insp, 'well')) != 1:
        fail(f"step {k}: the drawn railing and the well guard survive: {[(r.get('id'), r.get('host')) for r in railings_of(insp)]}")
print("ok: beside w0 only stairLeft; railRight on adds stairRight; railLeft off removes the left; the drawn railing stays")

# (c) The north deck: one railing on its three free edges.
rr = run_ok(HOUSE + [NORTH_DECK, INSPECT17, probe_cmd([[5.955, 11.055, 0.75], [5.955, 11.055, 1.70], [4.0, 11.055, 1.5]])], "the north deck")
insp = inspected(rr)
D = deck_room(insp)['id']
dk = one_railing(insp, 'deck', D, "the deck railing")
same_points(dk.get('path'), [[6, 8.145], [6, 11.1], [2, 11.1], [2, 8.145]], 1e-6, "the deck railing's path (free edges, ends 0.045 off the wall)")
same_points(dk.get('posts'), [[5.955, 8.145], [5.955, 9.6], [5.955, 11.055], [4.651667, 11.055], [3.348333, 11.055], [2.045, 11.055],
                              [2.045, 9.6], [2.045, 8.145]], 1e-3, "the deck railing's posts")
if dk.get('auto') is not True or dk.get('level') != 0:
    fail(f"the deck railing is automatic on the deck's level: {json.dumps(dk)[:300]}")
expect_hits(rr, [([5.955, 11.055, 0.75], 'post'), ([5.955, 11.055, 1.70], 'post'), ([4.0, 11.055, 1.5], 'railing')],
            "a corner post from the rim bottom 0.691 to 1.814; the panel above the deck")
rr = run_ok(HOUSE + [NORTH_DECK, cmd('deck.edit', {"id": D, "rails": ["auto", "auto", "off", "auto"]}), INSPECT17,
                     cmd('deck.edit', {"id": D, "rails": "off"}), INSPECT17], "deck.edit rails")
two = railings_by_host(inspected(rr, 0), 'deck')
paths = sorted(json.dumps(r.get('path')) for r in two)
if len(two) != 2 or paths != sorted([json.dumps([[6, 8.145], [6, 11.1]]), json.dumps([[2, 11.1], [2, 8.145]])]):
    fail(f"edge 2 off splits the deck railing in two: {[r.get('path') for r in two]}")
if railings_by_host(inspected(rr, 1), 'deck'):
    fail(f"rails off removes every deck railing: {[(r.get('id'), r.get('host')) for r in railings_of(inspected(rr, 1))]}")
print(f"ok: deck {D}: one railing on the three free edges with 8 posts reaching the rims; edge 2 off splits it; rails off removes it")

# (d) A free-standing deck: one closed railing.
rr = run_ok([free_deck(12), INSPECT17], "a free-standing deck")
fd = railings_by_host(inspected(rr), 'deck')
if len(fd) != 1 or len(fd[0].get('path') or []) != 5 or json.dumps(fd[0]['path'][0]) != json.dumps(fd[0]['path'][-1]):
    fail(f"a deck touching no wall gets one closed railing (5 path points, first = last): {[r.get('path') for r in fd]}")
if len(fd[0].get('posts') or []) != 10:
    fail(f"the closed railing has 10 posts (4 corners + 2 + 1 + 2 + 1): {len(fd[0].get('posts') or [])}")
print("ok: a free-standing deck: one closed railing, 10 posts")

# (e) Saved drawings keep what they had: nothing is added on load or build.
saved = os.path.join(tmp, 'rails-off.dxf')
run_ok(HOUSE2 + [stair_cmd(**NO_RAILS)], "a stair with its side rails off", save=saved)
rc, rr, so, se = run([cmd('build3d', {}), INSPECT17], file=saved)
insp = inspected(rr)
R = railings_of(insp)
if len(R) != 1 or R[0].get('host') != 'well':
    fail(f"the reopened drawing has only its well guard: {[(r.get('id'), r.get('host'), r.get('auto')) for r in R]}")
s = by_id(stairs_of(insp), 's0', 'stairs')
if s.get('railLeft') != 'off' or s.get('railRight') != 'off':
    fail(f"the rail settings round-trip: {s.get('railLeft')!r}, {s.get('railRight')!r}")
print("ok: a saved drawing reopens with the railings it had")
print("PASS 07")
