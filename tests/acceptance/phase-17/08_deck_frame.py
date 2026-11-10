#!/usr/bin/env python3
# Decks you can stand under: the north deck (2,8.1) (6,8.1) (6,11.1) (2,11.1), 0.9 m above grade against the north wall,
# gets a ledger on w2, 9 joists 38 x 184 at x 2.4, 2.8 .. 5.6 (10 equal bays of 0.4 <= 0.406) plus rim joists on its
# three free edges, a 76 x 235 drop beam on y = 10.8 (0.3 in from the outer edge), 0.14 m posts at x 2.3, 4.0, 5.7
# (end posts 0.3 in, bays <= 2.4) on 0.3 m round piers, and 25 mm boards on top: probes post (4.0,10.8,0.2), nothing
# (3.15,10.8,0.2), beam (3.15,10.8,0.6), joist (2.8,9.5,0.8), nothing (3.0,9.5,0.8), boards (3.0,9.5,0.89). The board
# direction (default parallel to the ledger) turns the Wood board texture in the GLB and never re-frames the joists.
# A free-standing deck spans its short side with a beam 0.3 in from each end; grade is the terrain under each post.
# A stub (the 0.05 slab) fails (a).
# needs: p17-observables, p17-commands, p17-scene-plumbing, p17-deck-frame
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

tmp = tempfile.mkdtemp()
P = [[4.0, 10.8, 0.2], [3.15, 10.8, 0.2], [3.15, 10.8, 0.6], [2.8, 9.5, 0.8], [3.0, 9.5, 0.8], [3.0, 9.5, 0.89]]


def lines_x(lines, what):
    """The x of vertical member lines [[x, y0], [x, y1]], sorted; fails on a slanted one."""
    xs = []
    for l in lines:
        if not (isinstance(l, list) and len(l) == 2 and abs(num(l[0][0], what) - num(l[1][0], what)) < 1e-6):
            fail(f"{what}: a member line [[x, y0], [x, y1]] expected: {l!r}")
        xs.append(num(l[0][0], what))
    return sorted(xs)


# (a) The north deck.
rr = run_ok(HOUSE + [NORTH_DECK, INSPECT17, probe_cmd(P)], "the north deck")
insp = inspected(rr)
D = deck_room(insp)['id']
rows = decks_of(insp)
if len(rows) != 1 or rows[0].get('id') != D:
    fail(f"arch.inspect decks has the one deck {D}: {[r.get('id') for r in rows]}")
d = rows[0]
for key in ('level', 'top', 'boardDirection', 'ledger', 'joists', 'rims', 'beams', 'posts', 'postGrades'):
    if key not in d:
        fail(f"the decks row has no '{key}': {sorted(d)}")
near(d['top'], 0.9, 1e-9, "the deck's top")
if not isinstance(d['ledger'], dict) or d['ledger'].get('wall') != 'w2':
    fail(f"the ledger sits on the north wall w2: {d['ledger']!r}")
xs = lines_x(d['joists'], 'joists')
if len(xs) != 9:
    fail(f"9 joists in 10 equal bays of 0.4: {len(xs)} at x {xs}")
for k, x in enumerate(xs):
    near(x, 2.4 + 0.4 * k, 1e-6, f"joist {k} x")
for l in d['joists']:
    ys = sorted((num(l[0][1], 'joist y'), num(l[1][1], 'joist y')))
    near(ys[0], 8.138, 1e-6, "a joist's start (the ledger's inner face)")
    near(ys[1], 11.062, 1e-6, "a joist's end (the rim's inner face)")
if len(d['rims']) != 3:
    fail(f"3 rim joists on the free edges: {len(d['rims'])}")
if len(d['beams']) != 1:
    fail(f"one beam: {d['beams']!r}")
b = d['beams'][0]
near(num(b[0][1], 'beam y'), 10.8, 1e-6, "the beam's y (0.3 in from the outer edge)")
near(num(b[1][1], 'beam y'), 10.8, 1e-6, "the beam's y")
near(min(num(b[0][0], 'beam x'), num(b[1][0], 'beam x')), 2.0, 1e-6, "the beam's start x")
near(max(num(b[0][0], 'beam x'), num(b[1][0], 'beam x')), 6.0, 1e-6, "the beam's end x")
same_points(d['posts'], [[2.3, 10.8], [4.0, 10.8], [5.7, 10.8]], 1e-6, "the posts (0.3 in from the sides, bays <= 2.4)")
if not isinstance(d['postGrades'], list) or len(d['postGrades']) != 3 or any(abs(num(g, 'grade')) > 1e-9 for g in d['postGrades']):
    fail(f"without a terrain every post stands at grade 0: {d['postGrades']!r}")
near(d['boardDirection'], 0.0, 1e-9, "the default board direction (parallel to the ledger)")
expect_hits(rr, [(P[0], 'post'), (P[1], None), (P[2], 'beam'), (P[3], 'joist'), (P[4], None), (P[5], 'deck')],
            "a post from grade to the beam, the beam 0.456..0.691, a joist 0.691..0.875 at x 2.8, the boards 0.875..0.9")
parts = parts_of(rr)
for k, v in (('joist', 9), ('rim', 3), ('ledger', 1), ('beam', 1), ('footing', 3)):
    if parts.get(k) != v:
        fail(f"parts.{k} is {v}, got {parts.get(k)} in {parts}")
print(f"ok: deck {D}: ledger on w2, 9 joists at x 2.4..5.6 from y 8.138 to 11.062, 3 rims, a beam on y 10.8, posts at 2.3, 4.0, 5.7 on piers")

# (b) The board direction turns the texture (and nothing else).
ga, gb = os.path.join(tmp, 'a.glb'), os.path.join(tmp, 'b.glb')
run_ok(HOUSE + [NORTH_DECK, cmd('export', {"path": ga})], "the deck's GLB")
rr = run_ok(HOUSE + [NORTH_DECK, cmd('deck.edit', {"id": D, "boards": 90}), INSPECT17, cmd('export', {"path": gb})], "boards 90")
d2 = decks_of(inspected(rr))[0]
near(d2.get('boardDirection'), 90.0, 1e-9, "boardDirection after deck.edit boards 90")
if json.dumps(d2.get('joists')) != json.dumps(d['joists']) or json.dumps(d2.get('posts')) != json.dumps(d['posts']):
    fail("the board direction never re-frames the joists or moves the posts")
for path, want, what in ((ga, (1 / 1.2, 0.0), "boards along the ledger (+x)"), (gb, (0.0, 1 / 1.2), "boards turned 90 degrees")):
    j, binc = glb(path)
    mat, _ = material_of_node(j, f'deck-{D}')
    if texture_image_name(j, mat) != 'wood':
        fail(f"{what}: the deck boards' material has the Wood board texture (image 'wood'), got {texture_image_name(j, mat)!r}")
    g = top_face_u_gradient(j, binc, f'deck-{D}', 0.9, what)
    if abs(g[0] - want[0]) > 1e-3 or abs(g[1] - want[1]) > 1e-3:
        fail(f"{what}: the top face's u gradient is {want} (one repeat per 1.2 m along the boards), got {g}")
print("ok: the boards carry the wood texture; its u runs along +x by default and along +y with boards 90; the frame is unchanged")

# (c) A free-standing deck spans its short side.
rr = run_ok([free_deck(12), INSPECT17, probe_cmd([[13.0, 0.3, 0.6], [14.0, 2.7, 0.2], [14.0, 1.5, 0.89]])], "a free-standing deck")
f = decks_of(inspected(rr))[0]
if f.get('ledger') is not None:
    fail(f"a deck touching no wall has no ledger: {f.get('ledger')!r}")
xs = lines_x(f['joists'], 'free-standing joists')
if len(xs) != 9 or abs(xs[0] - 12.4) > 1e-6 or abs(xs[-1] - 15.6) > 1e-6:
    fail(f"9 joists across the 4 m side at x 12.4..15.6: {xs}")
if len(f['rims']) != 4:
    fail(f"rims on all four free edges: {len(f['rims'])}")
bys = sorted(num(b[0][1], 'beam y') for b in f['beams'])
if len(bys) != 2 or abs(bys[0] - 0.3) > 1e-6 or abs(bys[1] - 2.7) > 1e-6:
    fail(f"two beams 0.3 in from each end of the joists (y 0.3 and 2.7): {bys}")
same_points(sorted(f['posts']), sorted([[12.3, 0.3], [14.0, 0.3], [15.7, 0.3], [12.3, 2.7], [14.0, 2.7], [15.7, 2.7]]), 1e-6, "six posts")
expect_hits(rr, [([13.0, 0.3, 0.6], 'beam'), ([14.0, 2.7, 0.2], 'post'), ([14.0, 1.5, 0.89], 'deck')], "the free-standing frame")
print("ok: free-standing: no ledger, joists across the short side, beams at y 0.3 and 2.7, six posts")

# (d) Grade from the terrain: a flat site at -0.3.
TERR = cmd('terrain', {"boundary": [[-20, -20], [40, -20], [40, 40], [-20, 40]], "elevation": -0.3})
rr = run_ok([TERR, free_deck(12), INSPECT17, cmd('terrain.query', {"points": [[14.0, 2.7]]}),
             probe_cmd([[14.0, 2.7, -0.2], [14.0, 2.7, -0.5]])], "a deck on a terrain at -0.3")
f = decks_of(inspected(rr))[0]
for g in f.get('postGrades') or []:
    near(g, -0.3, 1e-6, "a post's grade on the -0.3 terrain")
if len(f.get('postGrades') or []) != 6:
    fail(f"six post grades: {f.get('postGrades')!r}")
near(reply_with(rr, 'elevations', -1, 'terrain.query')['elevations'][0], -0.3, 1e-6, "terrain.query at a post")
expect_hits(rr, [([14.0, 2.7, -0.2], 'post'), ([14.0, 2.7, -0.5], 'footing')], "the post reaches the terrain; its pier is below it")
print("ok: on a -0.3 terrain the posts reach -0.3 and stand on piers")
print("PASS 08")
