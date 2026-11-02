# An IfcSpace per room, named for it, with its areas: a CADCraft_RoomAreas property set (InteriorArea, StandardArea as
# IfcAreaMeasure) and Qto_SpaceBaseQuantities (NetFloorArea = interior, GrossFloorArea = standard), equal to the model's
# (arch.inspect) within 1e-6 m²; its body spans the room from its floor up to its ceiling height.
# needs: p12-ifc-spaces
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
rooms = model_counts(rr)['raw']['rooms']
header, inst = step_file(out)
spaces = of_type(inst, 'IFCSPACE')
if len(spaces) != len(rooms):
    fail(f"one IfcSpace per room: {len(spaces)} vs {len(rooms)}")
props = {}                                    # space #n -> {property: value}
for _, rel in of_type(inst, 'IFCRELDEFINESBYPROPERTIES'):
    e, d = deref(inst, rel[5])
    values = {}
    if e == 'IFCPROPERTYSET' and d[2] == 'CADCraft_RoomAreas':
        for r in d[4]:
            _, pv = deref(inst, r)
            values[pv[0]] = pv[2].value if isinstance(pv[2], Typed) else pv[2]
    elif e == 'IFCELEMENTQUANTITY' and d[2] == 'Qto_SpaceBaseQuantities':
        for r in d[5]:
            qe, q = deref(inst, r)
            if qe == 'IFCQUANTITYAREA':
                values[q[0]] = q[3]
    for o in rel[4]:
        props.setdefault(o.n, {}).update(values)
want = sorted((round(r['interiorArea'], 6), round(r['standardArea'], 6)) for r in rooms)
got = []
for n, a in spaces:
    p = props.get(n, {})
    for k in ('InteriorArea', 'StandardArea', 'NetFloorArea', 'GrossFloorArea'):
        if k not in p:
            fail(f"IfcSpace #{n} {decode_step_string(a[2] or '')!r} has no {k}: {p}")
    if abs(p['NetFloorArea'] - p['InteriorArea']) > 1e-9 or abs(p['GrossFloorArea'] - p['StandardArea']) > 1e-9:
        fail(f"NetFloorArea is the interior area and GrossFloorArea the standard area: {p}")
    got.append((round(p['InteriorArea'], 6), round(p['StandardArea'], 6)))
    pts, _ = face_set_points(inst, a)
    zs = [q[2] for q in pts]
    if not (abs(min(zs)) < 0.05 and 2.2 < max(zs) < 3.2):
        fail(f"IfcSpace #{n}'s body runs from its floor to its ceiling height: z {min(zs):.3f}..{max(zs):.3f} in its storey")
print(f"rooms (interior, standard) {want}; IfcSpaces {sorted(got)}")
if sorted(got) != want:
    fail("each IfcSpace carries its room's areas")
print("PASS 08")
