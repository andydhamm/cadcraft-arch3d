#!/usr/bin/env python3
# Exports (the brief's "IFC IfcCovering (MOLDING, SKIRTINGBOARD) for moldings, glass walls as IfcWall with a glass
# material, countertops and backsplashes as part of their cabinets' IfcFurniture", plus glTF nodes and OBJ groups for
# every phase-18 mesh and the DXF round trip). The kitchen with a run of three base cabinets, a crowned wall cabinet, the
# run's counter detached (its front edge moved 0.3, a waterfall on its end), a backsplash on it, base and crown moldings,
# a molding line, a glass wall and a niche in the east wall: the GLB has nodes molding-r0-base-0, molding-r0-crown-1,
# molding-ml0, countertop-ct0, countertop-ct0-waterfall, backsplash-bs0, cabinet-cb3-crown, wall-w4-glass and
# wall-w1-niche-o0 and validates; the OBJ has the same groups; the IFC has one IfcCovering .SKIRTINGBOARD. and two
# .MOLDING., one IfcMaterial 'Glass' associated with wall-w4, four IfcFurniture and no product for the countertop or
# the backsplash (their triangles join cabinet-cb0's body, painted or not), and the niche as one IfcOpeningElement
# .RECESS. with no IfcDoor; a save and reopen keeps every phase-18 row.
# A stub fails: missing nodes or groups, no coverings, a door for the niche, lost rows after the round trip.
# needs: p18-room-moldings, p18-molding-lines-1, p18-molding-lines-2, p18-cabinet-moldings, p18-countertop-shapes, p18-backsplash, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3, p18-shower-niche-1, p18-shower-niche-2, p18-shower-niche-3, p18-shower-niche-4, p18-exports
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

tmp = tempfile.mkdtemp()
BASES = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.6, 0.6]})        # cb0..cb2, x 0.6..2.4
WALLC = cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "width": 0.6, "crown": "molding:crown"})   # cb3
DETACH = cmd('countertop.detach', {"run": "cb0"})
MOLD = cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}, {"kind": "crown"}]})
LINE = cmd('moldingline', {"points": [[3, 3], [5, 3]], "elevation": 1.0})
GLASS = cmd('wall', {"points": [[4, 1], [5.5, 1]], "type": "glass"})                                     # w4
NICHE = cmd('niche', {"wall": "w1", "offset": 2.0, "depth": 0.1})                                       # o0
insp = inspected(run_ok(KITCHEN + [BASES, WALLC, DETACH, INSPECT18], "the detached run"))
CT = countertops_of(insp, False)[0]['id'] if countertops_of(insp, False) else fail("a custom countertop")
row = ct_row(insp, CT)
I, _ = edge_index(row, lambda mx, my: abs(my - 0.625) < 1e-6, "the run's front edge")
J, _ = edge_index(row, lambda mx, my: abs(mx - 2.4) < 1e-6, "the run's end edge")
SHAPE = [cmd('countertop.edit', {"id": CT, "edge": I, "move": 0.3}), cmd('countertop.edit', {"id": CT, "edge": J, "waterfall": True})]
BS = cmd('backsplash', {"run": CT})
PLAIN = KITCHEN + [BASES, WALLC]
SETUP = PLAIN + [DETACH] + SHAPE + [BS, MOLD, LINE, GLASS, NICHE]
rr = run_ok(SETUP + [INSPECT18], "the set-up")
insp = inspected(rr)
B0 = backsplashes_of(insp)[0]['id'] if backsplashes_of(insp) else fail("a backsplash")
ML = molding_lines_of(insp)[0]['id'] if molding_lines_of(insp) else fail("a molding line")
niches = [o for o in openings_rows(insp) if o.get('kind') == 'niche']
if len(niches) != 1:
    fail("one niche")
O = niches[0]['id']
NAMES = ['molding-r0-base-0', 'molding-r0-crown-1', f'molding-{ML}', f'countertop-{CT}', f'countertop-{CT}-waterfall', f'backsplash-{B0}',
         'cabinet-cb3-crown', 'wall-w4-glass', f'wall-w1-niche-{O}']

g, i, o, d = (os.path.join(tmp, n) for n in ('p18.glb', 'p18.ifc', 'p18.obj', 'p18.dxf'))
run_ok(SETUP + [cmd('export', {"path": g}), cmd('export', {"path": i}), cmd('export', {"path": o})], "the exports", save=d)
for p in (g, i, o, d):
    if not os.path.exists(p):
        fail(f"{os.path.basename(p)} was written")

# 1. GLB nodes.
j, binc = glb(g)
names = set(node_names(j))
missing = [n for n in NAMES if n not in names]
if missing:
    fail(f"GLB nodes missing: {missing} (present: {sorted(n for n in names if n and n.split('-')[0] in ('molding', 'countertop', 'backsplash', 'cabinet', 'wall'))[:40]})")
for n in NAMES:
    if not node_meshes(j, n):
        fail(f"GLB node {n} carries a mesh")
validate_glb(g)
print(f"ok: GLB nodes {', '.join(NAMES)}")

# 2. OBJ groups.
lines = {l.strip() for l in open(o, encoding='utf-8', errors='replace').read().splitlines()}
missing = [n for n in NAMES if f'o {n}' not in lines and f'g {n}' not in lines]
if missing:
    fail(f"OBJ groups missing: {missing}")
print("ok: OBJ groups with the same names")


# 3. IFC.
def ifc_of(path):
    return step_file(path)[1]


def named(inst, entity, name):
    return [a for _, a in of_type(inst, entity) if len(a) >= 3 and isinstance(a[2], str) and decode_step_string(a[2]) == name]


def tri_count(inst, entity, name):
    found = named(inst, entity, name)
    if len(found) != 1:
        fail(f"IFC: one {entity} named {name}: {sorted(str(n) for n in ifc_names(inst, entity) if n)[:20]}")
    pts, tris = face_set_points(inst, found[0])
    return len(tris)


inst = ifc_of(i)
cov = [a for _, a in of_type(inst, 'IFCCOVERING')]
kinds = sorted(str(a[8]) for a in cov if len(a) > 8)
if kinds.count('SKIRTINGBOARD') != 1 or kinds.count('MOLDING') != 2:
    fail(f"IFC: one IfcCovering .SKIRTINGBOARD. (the base) and two .MOLDING. (the crown, the molding line): {kinds}")
for a in cov:
    face_set_points(inst, a)
mats = [a for _, a in of_type(inst, 'IFCMATERIAL') if a and isinstance(a[0], str) and decode_step_string(a[0]) == 'Glass']
if len(mats) != 1:
    fail(f"IFC: one IfcMaterial 'Glass': {[a[0] for _, a in of_type(inst, 'IFCMATERIAL')]}")
walls = {n: a for n, (e, a) in inst.items() if e == 'IFCWALL'}
w4 = [n for n, a in walls.items() if isinstance(a[2], str) and decode_step_string(a[2]) == 'wall-w4']
if len(w4) != 1:
    fail(f"IFC: an IfcWall named wall-w4 (the glass wall): {sorted(str(n) for n in ifc_names(inst, 'IFCWALL') if n)}")
assoc = [a for _, a in of_type(inst, 'IFCRELASSOCIATESMATERIAL') if any(r.n == w4[0] for r in refs_of(a[4]))]
if not assoc or not any(deref(inst, r)[0] == 'IFCMATERIAL' for a in assoc for r in refs_of(a[5])):
    fail("IFC: an IfcRelAssociatesMaterial relates wall-w4 to the Glass material")
furniture = [str(n) for n in ifc_names(inst, 'IFCFURNITURE')]
if len(furniture) != 4:
    fail(f"IFC: four IfcFurniture (the cabinets): {furniture}")
for forbidden in (f'countertop-{CT}', f'backsplash-{B0}'):
    for ent in ('IFCFURNITURE', 'IFCCOVERING', 'IFCBUILDINGELEMENTPROXY', 'IFCSLAB'):
        if forbidden in [str(n) for n in ifc_names(inst, ent)]:
            fail(f"IFC: {forbidden} is part of its cabinet's IfcFurniture, not its own {ent}")
op = [a for _, a in of_type(inst, 'IFCOPENINGELEMENT')]
if len(op) != 1 or decode_step_string(op[0][2]) != f'niche-{O}' or str(op[0][8]) != 'RECESS':
    fail(f"IFC: exactly one IfcOpeningElement, niche-{O} .RECESS.: {[(decode_step_string(a[2]) if isinstance(a[2], str) else None, a[8] if len(a) > 8 else None) for a in op]}")
face_set_points(inst, op[0])
if named(inst, 'IFCDOOR', f'door-{O}') or named(inst, 'IFCWINDOW', f'window-{O}'):
    fail(f"IFC: the niche {O} is not written as a door or a window")
voids = [a for _, a in of_type(inst, 'IFCRELVOIDSELEMENT')]
if not voids:
    fail("IFC: the niche's IfcRelVoidsElement")
fills = [a for _, a in of_type(inst, 'IFCRELFILLSELEMENT')]
if fills:
    fail(f"IFC: no IfcRelFillsElement for the niche (nothing fills a recess): {len(fills)}")
n_with = tri_count(inst, 'IFCFURNITURE', 'cabinet-cb0')
i2 = os.path.join(tmp, 'plain.ifc')
run_ok(PLAIN + [cmd('export', {"path": i2})], "the plain run's IFC")
n_plain = tri_count(ifc_of(i2), 'IFCFURNITURE', 'cabinet-cb0')
if n_with <= n_plain:
    fail(f"IFC: cabinet-cb0's body grows by the countertop and the backsplash ({n_plain} triangles plain, {n_with} with them)")
i3 = os.path.join(tmp, 'painted.ifc')
run_ok(SETUP + [cmd('paint.material', {"material": "#c04020", "target": f"countertop/{CT}", "scope": "component"}),
                cmd('paint.material', {"material": "#2040c0", "target": f"backsplash/{B0}", "scope": "component"}),
                cmd('export', {"path": i3})], "the painted export")
inst3 = ifc_of(i3)
if len(ifc_names(inst3, 'IFCFURNITURE')) != 4 or f'countertop-{CT}' in [str(n) for n in ifc_names(inst3, 'IFCFURNITURE')]:
    fail(f"IFC (painted): still four IfcFurniture and no countertop product: {ifc_names(inst3, 'IFCFURNITURE')}")
if tri_count(inst3, 'IFCFURNITURE', 'cabinet-cb0') != n_with:
    fail("IFC (painted): the painted countertop and backsplash stay in cabinet-cb0's body")
print(f"ok: IFC coverings 1 skirting + 2 molding, Glass material on wall-w4, 4 furniture holding {CT} and {B0}, niche-{O} a recess")

# 4. The DXF round trip.
KEYS = ('countertops', 'backsplashes', 'moldings', 'moldingLines', 'walls', 'openings')
before = {k: insp.get(k) for k in KEYS}
after = inspected(run_ok([INSPECT18], "reopen the saved DXF", file=d))
for k in KEYS:
    if json.dumps(before[k], sort_keys=True) != json.dumps(after.get(k), sort_keys=True):
        fail(f"the DXF round trip keeps the {k} rows: before {json.dumps(before[k])[:300]} after {json.dumps(after.get(k))[:300]}")
print("ok: the DXF round trip keeps countertops, backsplashes, moldings, molding lines, walls and openings")
print("PASS 16")
