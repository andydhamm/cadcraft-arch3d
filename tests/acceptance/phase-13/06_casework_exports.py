# Exports of the polished model: cabinets and fixtures are GLB nodes named by label (cabinet-cb0, cabinet-cb0-front,
# cabinet-cb0-counter, fixture-fx0), the OBJ's groups are still exactly the GLB's mesh nodes, and IFC writes cabinets as
# IfcFurniture, plumbing as IfcSanitaryTerminal and appliances as IfcElectricAppliance, each in its storey, with the
# finished faces inside their walls' bodies.
# needs: p13-doc-finish, p13-arch3d-finish-meshes, p13-cmd-casework, p13-arch3d-casework, p13-glb-textures, p13-io-casework
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp()
BATH = [cmd('fixture', {"kind": "toilet", "at": [1.0, 2.0]}), cmd('fixture', {"kind": "lavatory", "at": [2.0, 2.0]})]
model = ROOM + [cmd('material', {"walls": "exterior", "finish": "brick"})] + KITCHEN + BATH
g, o, i = (os.path.join(tmp, 'k.' + e) for e in ('glb', 'obj', 'ifc'))
for out in (g, o, i):
    export(model, out)
j, binc = glb(g)
names = [n.get('name') for n in j.get('nodes', []) if 'mesh' in n]
for n in ['cabinet-cb0', 'cabinet-cb0-front', 'cabinet-cb0-counter', 'cabinet-cb3', 'cabinet-cb3-front', 'cabinet-cb4',
          'fixture-fx0', 'fixture-fx1', 'fixture-fx2', 'fixture-fx3', 'fixture-fx4', 'fixture-fx5', 'wall-w0-exterior']:
    if n not in names:
        fail(f"GLB node {n}; casework/wall nodes: {sorted(x for x in names if x.startswith(('cabinet', 'fixture', 'wall')))}")
if 'cabinet-cb3-counter' in names:
    fail("the wall cabinet has no counter node")
# Since phase 14 the touching run cb0+cb1+cb2 has one counter, its owner cb0's, 0.6 + 0.9 + 0.6 = 2.1 m long.
for n in ('cabinet-cb1-counter', 'cabinet-cb2-counter'):
    if n in names:
        fail(f"the run cb0+cb1+cb2 has one counter node, cabinet-cb0-counter, but also {n}")
lo, hi = node_box(j, 'cabinet-cb0-counter')
spans = sorted(round(hi[k] - lo[k], 3) for k in range(3))
if abs(spans[0] - 0.038) > 0.002 or abs(spans[1] - 0.625) > 0.002 or abs(spans[2] - 2.1) > 0.002:
    fail(f"cb0's counter runs over cb0, cb1 and cb2: 2.1 long, 0.625 deep (overhang 0.025), 0.038 thick: spans {spans}")
groups = [l[2:].strip() for l in open(o, encoding='utf-8') if l.startswith('o ')]
if sorted(groups) != sorted(names):
    fail(f"the OBJ's groups are the GLB's mesh nodes: only OBJ {sorted(set(groups) - set(names))[:6]}, "
         f"only GLB {sorted(set(names) - set(groups))[:6]}")
print(f"ok: GLB and OBJ name {sum(1 for x in names if x.startswith(('cabinet', 'fixture')))} casework nodes alike")

hdr, inst = step_file(i)
def of(name):
    return of_type(inst, name)
def nm(v):
    return decode_step_string(v or '')
furn = of('IFCFURNITURE')
if len(furn) != 5:
    fail(f"five cabinets are five IfcFurniture: {len(furn)}")
if any(not isinstance(a[-1], str) or a[-1].strip('.') != 'USERDEFINED' for n, a in furn):
    fail(f"cabinets are .USERDEFINED. furniture: {[a[-1] for n, a in furn]}")
otypes = sorted(nm(a[4]) for n, a in furn)
if otypes != ['BASE CABINET', 'BASE CABINET', 'BASE CABINET', 'TALL CABINET', 'WALL CABINET']:
    fail(f"the furniture's ObjectType says which cabinet: {otypes}")
san = sorted(a[-1].strip('.') for n, a in of('IFCSANITARYTERMINAL'))
if san != ['SINK', 'TOILETPAN', 'WASHHANDBASIN']:
    fail(f"sink, toilet and lavatory are IfcSanitaryTerminal SINK, TOILETPAN, WASHHANDBASIN: {san}")
app = sorted(a[-1].strip('.') for n, a in of('IFCELECTRICAPPLIANCE'))
if app != ['DISHWASHER', 'ELECTRICCOOKER', 'REFRIGERATOR']:
    fail(f"range, refrigerator and dishwasher are IfcElectricAppliance: {app}")
names_ifc = [nm(a[2]) for n, a in furn + of('IFCSANITARYTERMINAL') + of('IFCELECTRICAPPLIANCE')]
if sorted(names_ifc) != sorted([f'cabinet-cb{k}' for k in range(5)] + [f'fixture-fx{k}' for k in range(6)]):
    fail(f"IFC names follow the GLB nodes: {names_ifc}")
contained = contained_in(inst)
for n, a in furn + of('IFCSANITARYTERMINAL') + of('IFCELECTRICAPPLIANCE'):
    if n not in contained:
        fail(f"#{n} ({nm(a[2])}) is contained in a storey")
    if not isinstance(a[6], Ref):
        fail(f"#{n} ({nm(a[2])}) has a body")
walls = of('IFCWALL')
w0 = [(n, a) for n, a in walls if nm(a[2]) == 'wall-w0']
if not w0:
    fail("IfcWall wall-w0")
pts, tris = face_set_points(inst, w0[0][1])
def tri_count(name):
    return sum(j['accessors'][pr['indices']]['count'] // 3 for m in node_meshes(j, name) for pr in m['primitives'])
want = tri_count('wall-w0') + tri_count('wall-w0-exterior')
if len(tris) != want:
    fail(f"wall-w0's IFC body is its whole wall, brick exterior face included: {len(tris)} triangles, GLB wall-w0 + "
         f"wall-w0-exterior = {want}")
print(f"ok: IFC: 5 IfcFurniture, 3 IfcSanitaryTerminal, 3 IfcElectricAppliance in their storey")
print("PASS 06")
