#!/usr/bin/env python3
# Ceiling build-up: a room's ceiling height is its ROUGH ceiling; a finish stack (layers top to bottom, material null = an
# air gap) hangs below it, and the FINISHED ceiling is the rough one minus the stack. The templates "none", "drywall"
# (12.7 mm) and "suspended" (drywall 0.0127, air 0.2, ceiling tile 0.016) are named case-insensitively. In 3D each material
# layer is a solid of its own (the bottom one is the ceiling people see), an air gap holds nothing, and an empty stack keeps
# today's 20 mm ceiling solid. A stub (no layers in 3D, the 20 mm solid everywhere, the rough height as the finished one)
# fails: the tile at 2.18 and the empty gap at 2.3 need real layers, and 2.1713 needs the stack's thickness.
# needs: p16-observables, p16-scene-plumbing, p16-ceiling-stack
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']
ROUGH = 2.4
SUSP = ROUGH - (0.0127 + 0.2 + 0.016)     # 2.1713
DRYW = ROUGH - 0.0127                    # 2.3873
SUSPENDED = [{"name": "drywall", "material": "drywall", "thickness": 0.0127},
             {"name": "air", "material": None, "thickness": 0.2},
             {"name": "ceiling tile", "material": "ceiling-tile", "thickness": 0.016}]
EXPLICIT = [{"material": "drywall", "thickness": 0.0127}, {"material": None, "thickness": 0.2},
            {"material": "ceiling-tile", "thickness": 0.016}]


def probe_list(rows):
    """The probe answers of the last build3d reply that probed."""
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('probe'), list) and r.get('probe')]
    if not hits:
        fail(f"no build3d reply with probe answers: {json.dumps(rows)[-600:]}")
    return hits[-1]['probe']


def build3d_reply(rows):
    hits = [r for r in rows if isinstance(r, dict) and 'meshes' in r and 'rebuildMs' in r]
    if not hits:
        fail(f"no build3d reply: {json.dumps(rows)[-600:]}")
    return hits[-1]


def expect_probes(rows, points, want, what):
    """want[k]: a room id (the point is inside that room's ceiling) or None (nothing there)."""
    got = probe_list(rows)
    if len(got) != len(points):
        fail(f"{what}: {len(points)} probes, {len(got)} answers")
    for p, g, w in zip(points, got, want):
        if w is None:
            if g is not None:
                fail(f"{what}: nothing at {p}, the probe hit {g}")
        elif not (isinstance(g, dict) and g.get('id') == w and part(g) == 'ceiling'):
            fail(f"{what}: {p} is in {w}'s ceiling, the probe answered {g}")


def same_stack(got, want, what):
    """A stack as a reply or row shows it, against `want` (names compared only where `want` gives one)."""
    if not isinstance(got, list) or len(got) != len(want):
        fail(f"{what}: expected {len(want)} layers, got {got!r}")
    for k, (g, w) in enumerate(zip(got, want)):
        if not isinstance(g, dict):
            fail(f"{what}: layer {k} is not an object: {g!r}")
        if g.get('material') != w['material']:
            fail(f"{what}: layer {k} material {w['material']!r}, got {g.get('material')!r}")
        near(g.get('thickness'), w['thickness'], 1e-12, f"{what}: layer {k} thickness")
        if 'name' in w and g.get('name') != w['name']:
            fail(f"{what}: layer {k} name {w['name']!r}, got {g.get('name')!r}")


# 1. The suspended ceiling in NE, drywall in NW, S keeps the empty stack.
PTS = [[8, 5], [3, 5], [5, 1.5]]
PROBES = [[8, 5, 2.18], [8, 5, 2.3], [8, 5, 2.39], [8, 5, 2.165], [8, 5, 2.41],
          [3, 5, 2.39], [3, 5, 2.38],
          [5, 1.5, 2.39], [5, 1.5, 2.37], [5, 1.5, 2.41]]
WANT = [NE, None, NE, None, None,
        NW, None,
        S, None, None]
rc, rr, so, se = run(HOUSE + [cmd('room', {"id": NE, "ceilingStack": "Suspended"}),
                              cmd('room', {"id": NW, "ceilingStack": "drywall"}),
                              cmd('build3d', {"ceiling": PTS, "probe": PROBES}), INSPECT16])
replies = [r for r in rr if isinstance(r, dict) and isinstance(r.get('ceilingStack'), list) and 'interiorArea' in r]
if len(replies) != 2:
    fail(f"the two room edits reply with their ceilingStack: {json.dumps(rr)[:600]}")
same_stack(replies[0]['ceilingStack'], SUSPENDED, "the room reply for NE's Suspended stack")
same_stack(replies[1]['ceilingStack'], SUSPENDED[:1], "the room reply for NW's drywall stack")
z = ceilings(rr)
if not (isinstance(z, list) and len(z) == 3):
    fail(f"three ceiling answers expected, got {z!r}")
near(z[0], SUSP, 1e-6, "the finished ceiling at (8,5) under the suspended stack")
near(z[1], DRYW, 1e-6, "the finished ceiling at (3,5) under drywall")
near(z[2], ROUGH, 1e-6, "the finished ceiling at (5,1.5) under the empty stack")
expect_probes(rr, PROBES, WANT, "suspended NE, drywall NW, empty S")
b = build3d_reply(rr)
if (b.get('floors'), b.get('ceilings')) != (3, 3):
    fail(f"build3d floors and ceilings stay 3 and 3 (one per room): {b.get('floors')}, {b.get('ceilings')}")
parts = parts_of(rr)
if parts.get('ceiling') != 4:
    fail(f"parts.ceiling is 4 (S 1, NW 1, NE 2: tile and drywall), got {parts.get('ceiling')} in {parts}")
insp = inspected(rr)
for rid, stack, fin in ((NE, SUSPENDED, SUSP), (NW, SUSPENDED[:1], DRYW), (S, [], ROUGH)):
    row = room_row(insp, rid)
    same_stack(row.get('ceilingStack'), stack, f"room {rid}'s inspect ceilingStack")
    near(row.get('roughCeiling'), ROUGH, 1e-6, f"room {rid} roughCeiling")
    near(row.get('finishedCeiling'), fin, 1e-6, f"room {rid} finishedCeiling")
print(f"ok: Suspended NE {SUSP:.4f} (tile at 2.18, gap at 2.3, drywall at 2.39), drywall NW {DRYW:.4f}, S 20 mm at 2.4; "
      "floors 3, ceilings 3, parts.ceiling 4; inspect rows")

# 2. The explicit layers give the same; template names in any case.
PROBES = [[8, 5, 2.18], [8, 5, 2.3], [8, 5, 2.39], [5, 1.5, 2.18], [5, 1.5, 2.3], [3, 5, 2.39], [3, 5, 2.38]]
rc, rr, so, se = run(HOUSE + [cmd('room', {"id": NE, "ceilingStack": EXPLICIT}),
                              cmd('room', {"id": S, "ceilingStack": "SUSPENDED"}),
                              cmd('room', {"id": NW, "ceilingStack": " DryWall "}),
                              cmd('build3d', {"ceiling": PTS, "probe": PROBES}), INSPECT16])
z = ceilings(rr)
near(z[0], SUSP, 1e-6, "the finished ceiling at (8,5) under the explicit layers")
near(z[1], DRYW, 1e-6, "the finished ceiling at (3,5) under \" DryWall \"")
near(z[2], SUSP, 1e-6, "the finished ceiling at (5,1.5) under \"SUSPENDED\"")
expect_probes(rr, PROBES, [NE, None, NE, S, None, NW, None], "explicit NE, SUSPENDED S, DryWall NW")
if parts_of(rr).get('ceiling') != 5 or build3d_reply(rr).get('ceilings') != 3:
    fail(f"parts.ceiling 5 (S 2, NW 1, NE 2) and ceilings 3: {parts_of(rr)}, {build3d_reply(rr).get('ceilings')}")
insp = inspected(rr)
same_stack(room_row(insp, NE).get('ceilingStack'), EXPLICIT, "NE's explicit stack")
same_stack(room_row(insp, S).get('ceilingStack'), SUSPENDED, "S's SUSPENDED stack")
print("ok: the explicit layers give 2.1713 too; SUSPENDED and \" DryWall \" are the templates")

# 3. The ceiling finish layer the library offers: `library {"for": "room/<NE>/ceiling"}` lists Materials, and one of
# their ids is a layer material.
rc, rr, so, se = run(HOUSE + [cmd('library', {"for": f"room/{NE}/ceiling"})])
items = lib_reply(rr)['items']
if not items or any(i.get('category') != 'Materials' for i in items):
    fail(f"library for NE's ceiling lists Materials items: {[(i.get('id'), i.get('category')) for i in items]}")
mats = ids_of(items)
mat = 'material-oak' if 'material-oak' in mats else mats[0]
LIB = [{"material": "drywall", "thickness": 0.0127}, {"material": None, "thickness": 0.2},
       {"material": mat, "thickness": 0.016}]
rc, rr, so, se = run(HOUSE + [cmd('room', {"id": NE, "ceilingStack": LIB}),
                              cmd('build3d', {"ceiling": [[8, 5]], "probe": [[8, 5, 2.18], [8, 5, 2.3]]}), INSPECT16])
same_stack(room_row(inspected(rr), NE).get('ceilingStack'), LIB, f"NE's stack with the library material {mat}")
near(ceilings(rr)[0], SUSP, 1e-6, f"the finished ceiling at (8,5) with a {mat} tile")
expect_probes(rr, [[8, 5, 2.18], [8, 5, 2.3]], [NE, None], f"a {mat} tile")
print(f"ok: library for room/{NE}/ceiling lists {len(items)} Materials; {mat} is a layer material")

# 4. A library ceiling item follows the stack: a pendant placed at (8,5) hangs from 2.4, then from 2.1713 under the
# suspended stack, and from 2.4 again when the stack is cleared.
rc, rr, so, se = run(HOUSE + [cmd('library.place', {"item": "light-pendant", "at": [8, 5]}), INSPECT16,
                              cmd('room', {"id": NE, "ceilingStack": "Suspended"}), INSPECT16,
                              cmd('room', {"id": NE, "ceilingStack": "none"}), INSPECT16])
fid = reply_with(rr, 'furnishing', -1, 'library.place').get('furnishing')
near(item_top(inspected(rr, 0), fid), ROUGH, 1e-6, "the pendant's top before the stack")
near(item_top(inspected(rr, 1), fid), SUSP, 1e-6, "the pendant's top under the suspended stack")
near(item_top(inspected(rr, 2), fid), ROUGH, 1e-6, "the pendant's top once the stack is cleared")
print(f"ok: the pendant's top 2.4 -> {SUSP:.4f} -> 2.4")

# 5. An unknown template is refused with the names it knows, and nothing changes.
rc, rr, so, se = run(HOUSE + [cmd('room', {"id": NE, "ceilingStack": "plaster"}), ceiling_cmd([[8, 5]]), INSPECT16],
                     expect_ok=False)
errs = errors(se)
if not errs or not all(n in errs[0] for n in ('none', 'drywall', 'suspended')):
    fail(f"an unknown stack is refused naming the templates: {errs}")
near(ceilings(rr)[0], ROUGH, 1e-6, "NE's ceiling after the refused stack")
if room_row(inspected(rr), NE).get('ceilingStack') != []:
    fail(f"the refused stack wrote nothing: {room_row(inspected(rr), NE).get('ceilingStack')}")
print("ok: an unknown stack is refused, naming none, drywall and suspended")
print("PASS 02")
