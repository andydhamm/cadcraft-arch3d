#!/usr/bin/env python3
# Rebuild and exports. The set-up: the phase-1 house with Level 2, a 0.1 m wall (3,0)-(3,3), a 0.9 m stair from (1,1)
# over it (with its two side railings and well guard), the north deck (with its railing) and the 2 x 2 grille window.
# Each edit rebuilds only what depends on it and the incremental scene equals a full build (same digest):
#   stair.edit openRisers      -> 1 (the stair)
#   stair.edit underside 0.4   -> 2 (the stair and the wall under it, whose cut follows the underside)
#   deck.edit boards 90        -> 1 (the deck)
#   deck.edit height 1.0       -> 2 (the deck and its railing, whose surface follows the deck)
#   arch.opening.move o0       -> 2 (the window and its host wall)
# Exports: the IFC has 14 IfcMember (.MEMBER.: ledger, 9 joists, 3 rims, beam), 3 IfcColumn (.COLUMN.), 3 IfcFooting
# (.PAD_FOOTING.: the piers), one IfcRailing per railing (.HANDRAIL. for the stair sides, .GUARDRAIL. for the deck's) and
# one IfcStair with a body; the GLB has one node stair-s0 holding every stair part as a primitive (34: 15 treads, 16
# risers, 2 stringers, the landing; x 0.975..5.71), one node per railing, deck-r<D> (the boards) and one node per deck
# structure kind (joists 9 primitives, rims 3, ledger 1, beams 1, posts 3, piers 3), window-grille-o0; it validates; the
# OBJ's groups equal the GLB's mesh nodes. A stub fails: a full rebuild per edit gives other counts, no members / nodes.
# needs: p17-stair-flight, p17-wall-under-flight, p17-railing-auto, p17-deck-frame, p17-grilles-doors, p17-exports-rebuild
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

tmp = tempfile.mkdtemp()
WALL = cmd('wall', {"points": [[3, 0], [3, 3]], "thickness": 0.1})
STAIR = cmd('stair', {"start": [1, 1], "direction": 0, "width": 0.9, "fromLevel": 0, "toLevel": 1})
SETUP = HOUSE2 + [WALL, STAIR, NORTH_DECK, GRILLE22]
rr = run_ok(SETUP + [INSPECT17], "the set-up")
insp = inspected(rr)
D = deck_room(insp)['id']
RAILS = railings_of(insp)
if len(RAILS) != 4:
    fail(f"the set-up has four railings (well guard, two stair sides, the deck's): {[(r.get('id'), r.get('host')) for r in RAILS]}")

# 1. Incremental rebuild counts.
EDITS = [(cmd('stair.edit', {"id": "s0", "openRisers": True}), 1, "s0"),
         (cmd('stair.edit', {"id": "s0", "underside": 0.4}), 2, "s0 and the wall under it"),
         (cmd('deck.edit', {"id": D, "boards": 90}), 1, D),
         (cmd('deck.edit', {"id": D, "height": 1.0}), 2, f"{D} and its railing"),
         (cmd('arch.opening.move', {"opening": "o0", "offset": 5.0}), 2, "o0 and w2")]
steps = SETUP + [cmd('build3d', {"incremental": True})]
for edit, _, _ in EDITS:
    steps += [edit, cmd('build3d', {"incremental": True, "digest": True}), cmd('build3d', {"digest": True})]
rr = run_ok(steps, "the five edits, each followed by an incremental and a full build")
b = [r for r in rr if isinstance(r, dict) and 'rebuilt' in r and isinstance(r.get('digest'), str)]
if len(b) != 2 * len(EDITS):
    fail(f"{2 * len(EDITS)} build3d replies with digests, got {len(b)}")
for k, (edit, want, what) in enumerate(EDITS):
    inc, full = b[2 * k], b[2 * k + 1]
    if inc.get('incremental') is not True:
        fail(f"after `{edit}`: the build is incremental, got {inc.get('incremental')!r}")
    if inc.get('digest') != full.get('digest'):
        fail(f"after `{edit}`: the incremental scene's digest {inc.get('digest')} differs from the full build's {full.get('digest')}")
    if inc.get('rebuilt') != want:
        fail(f"`{edit}` rebuilds {want} object(s) ({what}), it rebuilt {inc.get('rebuilt')!r}")
    print(f"ok: `{edit}` rebuilt {want} ({what}); incremental == full")

# 2. Exports.
g, i, o = (os.path.join(tmp, n) for n in ('p17.glb', 'p17.ifc', 'p17.obj'))
run_ok(SETUP + [cmd('export', {"path": g}), cmd('export', {"path": i}), cmd('export', {"path": o})], "the exports")
for p in (g, i, o):
    if not os.path.exists(p):
        fail(f"{os.path.basename(p)} was written")
hdr, inst = step_file(i)


def typed(entity, enum):
    """The instances of `entity` whose PredefinedType (the last attribute, an Enum word) is `enum`."""
    return [a for _, a in of_type(inst, entity) if a and isinstance(a[-1], str) and str(a[-1]) == enum]


members, columns, footings = typed('IFCMEMBER', 'MEMBER'), typed('IFCCOLUMN', 'COLUMN'), typed('IFCFOOTING', 'PAD_FOOTING')
if len(members) != 14 or len(columns) != 3 or len(footings) != 3:
    fail(f"IFC: 14 IfcMember .MEMBER., 3 IfcColumn .COLUMN., 3 IfcFooting .PAD_FOOTING.; got {len(members)}, {len(columns)}, {len(footings)} "
         f"(all IfcMember {len(of_type(inst, 'IFCMEMBER'))}, IfcColumn {len(of_type(inst, 'IFCCOLUMN'))}, IfcFooting {len(of_type(inst, 'IFCFOOTING'))})")
for a in members[:1] + columns[:1] + footings[:1]:
    face_set_points(inst, a)
rails = of_type(inst, 'IFCRAILING')
if len(rails) != len(RAILS):
    fail(f"IFC: one IfcRailing per railing ({len(RAILS)}), got {len(rails)}")
kinds = {}
for _, a in rails:
    name = decode_step_string(a[2]) if isinstance(a[2], str) else ''
    kinds[name] = str(a[-1])
by_label = {f"railing-{r['id']}": r for r in RAILS}
for name, k in kinds.items():
    r = by_label.get(name)
    if r is None:
        fail(f"IFC: IfcRailing {name!r} is one of the model's railings {sorted(by_label)}")
    want = 'GUARDRAIL' if r.get('host') == 'deck' else 'HANDRAIL'
    if k != want:
        fail(f"IFC: {name} ({r.get('host')}) is .{want}., got .{k}.")
stairs = of_type(inst, 'IFCSTAIR')
if len(stairs) != 1:
    fail(f"IFC: one IfcStair, got {len(stairs)}")
face_set_points(inst, stairs[0][1])
print(f"ok: IFC: 14 members, 3 columns, 3 pad footings, {len(rails)} railings (deck guardrail, stair handrails), one stair with a body")

j, binc = glb(g)
names = [n.get('name') for n in j.get('nodes', []) if 'mesh' in n]
stair_nodes = [n for n in j.get('nodes', []) if n.get('name') == 'stair-s0' and 'mesh' in n]
if len(stair_nodes) != 1:
    fail(f"GLB: exactly one node stair-s0 (every stair part a primitive of it), got {len(stair_nodes)}")
prims = len(j['meshes'][stair_nodes[0]['mesh']]['primitives'])
if prims != 34:
    fail(f"GLB: stair-s0 holds 34 primitives (15 treads, 16 risers, 2 stringers, the landing), got {prims}")
lo, hi = node_box(j, 'stair-s0')
near(lo[0], 0.975, 1e-3, "stair-s0's box min x (the first nosing, 25 mm before start)")
near(hi[0], 5.71, 1e-3, "stair-s0's box max x (the landing: start + run + 0.9)")
for r in RAILS:
    node = f"railing-{r['id']}"
    if names.count(node) != 1:
        fail(f"GLB: one node {node}, got {names.count(node)} among {sorted(n for n in names if n and n.startswith('railing'))}")
for suffix, want in (('', None), ('-joists', 9), ('-rims', 3), ('-ledger', 1), ('-beams', 1), ('-posts', 3), ('-piers', 3)):
    name = f"deck-{D}{suffix}"
    nodes = [n for n in j['nodes'] if n.get('name') == name and 'mesh' in n]
    if len(nodes) != 1:
        fail(f"GLB: one node {name}, got {len(nodes)} among {sorted(n for n in names if n and n.startswith('deck'))}")
    if want is not None and len(j['meshes'][nodes[0]['mesh']]['primitives']) != want:
        fail(f"GLB: {name} holds {want} primitives, got {len(j['meshes'][nodes[0]['mesh']]['primitives'])}")
if 'window-grille-o0' not in names:
    fail(f"GLB: a node window-grille-o0: {sorted(n for n in names if n and 'o0' in n)}")
validate_glb(g)
groups = [l[2:].strip() for l in open(o, encoding='utf-8', errors='replace').read().splitlines() if l.startswith('o ')]
if sorted(groups) != sorted(names):
    fail(f"the OBJ's `o` groups equal the GLB's mesh nodes: only in OBJ {sorted(set(groups) - set(names))[:10]}, "
         f"only in GLB {sorted(set(names) - set(groups))[:10]}; counts {len(groups)} vs {len(names)}")
print(f"ok: GLB: stair-s0 (34 primitives, x 0.975..5.71), {len(RAILS)} railing nodes, deck-{D} and its six structure nodes, window-grille-o0; OBJ groups match")
print("PASS 11")
