# Geometry: each wall's IfcTriangulatedFaceSet, placed in its storey, covers the same box as the GLB node of that wall
# (within 1 mm), and a roof plane's and the stair's bodies match their GLB nodes too: the IFC is the 3D view's geometry.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc'); glbp = os.path.join(tmp, 'house.glb')
export(HOUSE, out); export(HOUSE, glbp)
j, _ = glb(glbp)
header, inst = step_file(out)
storey_z = {n: a[9] for n, a in of_type(inst, 'IFCBUILDINGSTOREY')}
cont = contained_in(inst)
agg = aggregates(inst)
parent = {p: w for w, ps in agg.items() for p in ps}


def world_box(n):
    a = inst[n][1]
    pts, _ = face_set_points(inst, a)
    s = cont.get(n, cont.get(parent.get(n)))
    z0 = storey_z.get(s, 0.0)
    return [min(p[k] for p in pts) for k in range(3)], [max(p[k] for p in pts) for k in range(3)], z0


def glb_box(name):
    return node_box(j, name)                        # the GLB's positions are drawing axes (its root node turns them Y-up)


checked = 0
for n, a in of_type(inst, 'IFCWALL'):
    name = decode_step_string(a[2])
    (lo, hi, z0), (glo, ghi) = world_box(n), glb_box(name)
    lo[2] += z0; hi[2] += z0
    if any(abs(lo[k] - glo[k]) > 1e-3 or abs(hi[k] - ghi[k]) > 1e-3 for k in range(3)):
        fail(f"{name}: IFC box {lo}..{hi} (storey z {z0}) differs from the GLB node's {glo}..{ghi}")
    checked += 1
for e, prefix in (('IFCSTAIR', 'stair-s'), ('IFCSLAB', 'roof-r')):
    for n, a in of_type(inst, e):
        name = decode_step_string(a[2])
        if not name.startswith(prefix):
            continue
        (lo, hi, z0), (glo, ghi) = world_box(n), glb_box(name)
        lo[2] += z0; hi[2] += z0
        if any(abs(lo[k] - glo[k]) > 1e-3 or abs(hi[k] - ghi[k]) > 1e-3 for k in range(3)):
            fail(f"{e} {name}: IFC box {lo}..{hi} differs from the GLB node's {glo}..{ghi}")
        checked += 1
print(f"ok: {checked} bodies match their GLB nodes within 1 mm")
if checked < 12:
    fail(f"walls, roof planes and the stair are named like their GLB nodes (wall-w1, roof-r0p0, stair-s0): only {checked} compared")
print("PASS 06")
