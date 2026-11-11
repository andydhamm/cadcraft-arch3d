#!/usr/bin/env python3
# Room-type finishes (the brief's first bullet): a room's floor, wall faces and ceiling take its type's template
# materials unpainted (garage: concrete floor; bath, laundry, kitchen: tile; bedroom: carpet; living: wood; every wall
# face toward a room drywall, every ceiling plaster), it takes them when its type changes, and a painter edit wins over
# the template. In the phase-1 house (rooms S, NW, NE) retyped garage / bath / bedroom: the surfaces rows show
# finish:concrete / finish:tile / finish:carpet floors, finish:plaster ceilings and finish:drywall room-facing wall
# sides, and no surface with a room keeps a `default:` material (phase 14's rule, now by construction); the rooms rows
# carry typeFinishes. Retyping changes the floor; paint.material on a floor survives a retype; an explicit `material`
# floor finish is cleared by a type change. The GLB textures the garage floor with the concrete image and the bath floor
# with tile; the five new finishes are library items and `material` accepts tile on rooms but refuses carpet on roofs.
# A stub fails: default:floor / default:wall materials, no typeFinishes, untextured floors, four finish items.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-room-finishes-1, p18-room-finishes-2, p18-room-finishes-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']
tmp = tempfile.mkdtemp()
TYPES = [cmd('room', {"id": S, "type": "garage"}), cmd('room', {"id": NW, "type": "bath"}), cmd('room', {"id": NE, "type": "bedroom"})]


def floor_of(insp, rid):
    return surfaces_of(insp).get(f'room/{rid}/floor', {}).get('material')


# 1. The template, read through the surfaces and the rooms rows.
rr = run_ok(HOUSE + TYPES + [INSPECT18], "three retyped rooms")
insp = inspected(rr)
surf = surfaces_of(insp)
for rid, want in ((S, 'finish:concrete'), (NW, 'finish:tile'), (NE, 'finish:carpet')):
    got = floor_of(insp, rid)
    if got != want:
        fail(f"room {rid}'s unpainted floor is {want}, the surfaces row says {got!r}")
    ceil = surf.get(f'room/{rid}/ceiling', {}).get('material')
    if ceil != 'finish:plaster':
        fail(f"room {rid}'s unpainted ceiling is finish:plaster, got {ceil!r}")
wall_sides = [s for k, s in surf.items() if k.startswith('wall/') and s.get('room') is not None]
if len(wall_sides) < 6:
    fail(f"the house has wall sides facing rooms: {len(wall_sides)} surfaces carry a room")
for s in wall_sides:
    if s.get('material') != 'finish:drywall':
        fail(f"a wall side facing room {s.get('room')} is finish:drywall unpainted: {s}")
defaults = [k for k, s in surf.items() if s.get('room') is not None and str(s.get('material', '')).startswith('default:')]
if defaults:
    fail(f"no surface with a room keeps a default: material: {defaults[:6]}")
for rid, want in ((S, 'concrete'), (NW, 'tile'), (NE, 'carpet')):
    tf = room_row(insp, rid).get('typeFinishes')
    if not isinstance(tf, dict) or tf.get('floor') != f'finish:{want}' or tf.get('wall') != 'finish:drywall' or tf.get('ceiling') != 'finish:plaster':
        fail(f"room {rid}'s typeFinishes: floor finish:{want}, wall finish:drywall, ceiling finish:plaster; got {tf!r}")
print("ok: garage concrete, bath tile, bedroom carpet floors; drywall walls and plaster ceilings; no default: material on a room")

# 2. Retyping follows the type; a painter edit wins; a type change clears an explicit finish.
rr = run_ok(HOUSE + TYPES + [cmd('room', {"id": NE, "type": "kitchen"}), cmd('room', {"id": NW, "type": "laundry"}),
                             cmd('room', {"id": S, "type": "living"}), INSPECT18], "retyped again")
insp = inspected(rr)
for rid, want in ((NE, 'finish:tile'), (NW, 'finish:tile'), (S, 'finish:wood')):
    if floor_of(insp, rid) != want:
        fail(f"after retyping, room {rid}'s floor is {want}, got {floor_of(insp, rid)!r}")
rr = run_ok(HOUSE + TYPES + [cmd('paint.material', {"material": "wood", "target": f"room/{NE}/floor", "scope": "component"}),
                             cmd('room', {"id": NE, "type": "bath"}), INSPECT18,
                             cmd('material', {"rooms": [NW], "finish": "carpet"}), INSPECT18,
                             cmd('room', {"id": NW, "type": "kitchen"}), INSPECT18], "paint and material against the template")
i0, i1, i2 = inspected(rr, 0), inspected(rr, 1), inspected(rr, 2)
if floor_of(i0, NE) != 'finish:wood':
    fail(f"a painted floor keeps its paint through a type change: {floor_of(i0, NE)!r}")
if floor_of(i1, NW) != 'finish:carpet':
    fail(f"`material {{rooms, finish: carpet}}` sets the floor finish: {floor_of(i1, NW)!r}")
if floor_of(i2, NW) != 'finish:tile':
    fail(f"a type change clears the explicit floor finish (kitchen -> tile): {floor_of(i2, NW)!r}")
print("ok: retype follows the template, a paint wins, a type change clears an explicit finish")

# 3. The GLB textures the floors; the walls stay untextured.
rr, j = export_glb(HOUSE + TYPES, tmp, 'finishes.glb')
jj, binc = j
names = node_names(jj)
for rid, img in ((S, 'concrete'), (NW, 'tile'), (NE, 'carpet')):
    node = f'floor-{rid}'
    if node not in names:
        fail(f"the GLB has a node {node}: {names[:40]}")
    mat, prim = material_of_node(jj, node)
    got = texture_image_name(jj, mat)
    if got != img:
        fail(f"{node} is textured with the {img} image, got {got!r}")
for node in [n for n in names if n and n.startswith('wall-w') and '-' not in n[6:]]:
    if texture_image_name(jj, material_of_node(jj, node)[0]):
        fail(f"{node} carries no texture (walls keep their default material)")
print("ok: floor-r<n> nodes carry concrete / tile / carpet images, wall nodes none")

# 4. The finishes as library items, and `material` with the new names.
rr = run_ok([cmd('library', {"chips": [{"kind": "folder", "value": "Finishes"}]})], "the Finishes folder")
items = [r for r in rr if isinstance(r, dict) and isinstance(r.get('items'), list)]
if not items:
    fail(f"no library reply: {json.dumps(rr)[-400:]}")
finishes = sorted(i.get('name', '').lower() for i in items[-1]['items'] if i.get('kind') == 'finish')
if finishes != ['brick', 'carpet', 'concrete', 'drywall', 'plaster', 'shingles', 'siding', 'tile', 'wood']:
    fail(f"the nine finishes as library items: {finishes}")
run_ok(HOUSE + [cmd('material', {"rooms": [S], "finish": "tile"})], "material tile on a room")
refused(HOUSE + [GABLE, cmd('material', {"roofs": "all", "finish": "carpet"})], "carpet on a roof")
print("ok: nine finish items; tile on a room accepted, carpet on a roof refused")
print("PASS 01")
