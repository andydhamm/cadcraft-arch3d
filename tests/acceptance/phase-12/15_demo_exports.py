# Demo: the two-storey house on its site with its lights, saved as DXF and converted to IFC, OBJ and GLB by the CLI, and
# exported from a session by the export command (MCP's path): every format holds the house, and the IFC converted from the
# DXF has the same objects and GlobalIds as the one exported from the session.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights, p12-io-obj, p12-cmd-export
import os, base64
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); dxf = os.path.join(tmp, 'house.dxf'); direct = os.path.join(tmp, 'direct', 'house.ifc')
os.makedirs(os.path.dirname(direct))
run(HOUSE, save=dxf)
export(HOUSE, direct)
env = dict(os.environ, SOURCE_DATE_EPOCH=str(EPOCH))
for ext in ('ifc', 'obj', 'glb'):
    p = subprocess.run(cli_args('convert') + [dxf, os.path.join(tmp, f'house.{ext}'), '--no-detect'], capture_output=True, text=True,
                       timeout=900, env=env)
    if p.returncode != 0 or 'panicked' in p.stderr:
        fail(f"cadcraft-cli convert house.dxf house.{ext}: {p.stderr[-500:]}")
if not os.path.exists(os.path.join(tmp, 'house.mtl')):
    fail("convert to house.obj writes house.mtl beside it")


def objects(path):
    _, inst = step_file(path)
    keep = ('IFCWALL', 'IFCDOOR', 'IFCWINDOW', 'IFCSPACE', 'IFCSTAIR', 'IFCRAILING', 'IFCROOF', 'IFCSLAB', 'IFCCOVERING',
            'IFCLIGHTFIXTURE', 'IFCGEOGRAPHICELEMENT', 'IFCBUILDINGSTOREY')
    return sorted((e, a[0]) for e, a in inst.values() if e in keep)


a, b = objects(direct), objects(os.path.join(tmp, 'house.ifc'))
print(f"IFC from the session: {len(a)} objects; from the DXF: {len(b)}")
if a != b:
    fail(f"the IFC converted from the DXF has the session's objects and GlobalIds: only in the session's "
         f"{sorted(set(a) - set(b))[:6]}, only in the DXF's {sorted(set(b) - set(a))[:6]}")
if len(a) < 30:
    fail(f"the house exports walls, openings, floors, ceilings, the roof, the stair, railings, spaces, lights and the site: {len(a)}")
j, _ = glb(os.path.join(tmp, 'house.glb'))
groups = [l[2:].strip() for l in open(os.path.join(tmp, 'house.obj'), encoding='utf-8') if l.startswith('o ')]
if sorted(groups) != sorted(n.get('name') for n in j.get('nodes', []) if 'mesh' in n):
    fail("the OBJ's groups are the GLB's mesh nodes")
cmds = HOUSE + [cmd('export', {"path": os.path.join(tmp, 'e.ifc')}), cmd('export', {"path": os.path.join(tmp, 'e.obj')}),
                cmd('export', {"format": "glb"})]
rc, rr, so, se = run(cmds)
outs = [r for r in rr if isinstance(r, dict) and r.get('format') in ('ifc', 'obj', 'glb')]
if [o.get('format') for o in outs] != ['ifc', 'obj', 'glb']:
    fail(f"the export command writes ifc and obj to a path and returns glb as data: {[(o.get('format'), list(o)) for o in outs]}")
if not os.path.exists(os.path.join(tmp, 'e.ifc')) or not os.path.exists(os.path.join(tmp, 'e.mtl')):
    fail("export {path: e.ifc} writes e.ifc; export {path: e.obj} writes e.obj and e.mtl")
data = base64.b64decode(outs[2].get('data') or '')
if data[:4] != b'glTF':
    fail("export {format: glb} returns the GLB as base64 data")
print(f"ok: DXF -> IFC, OBJ, GLB; the export command writes IFC and OBJ (+MTL) and returns a {len(data)}-byte GLB")
print("PASS 15")
