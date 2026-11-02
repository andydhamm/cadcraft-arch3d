# An open IFC toolkit reads the file: IfcOpenShell (outside the codebase) opens it as IFC4, finds no schema or EXPRESS rule
# issue, counts the same walls, openings and spaces, and builds a shape for every wall and slab. SKIPs where IfcOpenShell
# is missing; it looks in $XDG_CACHE_HOME/cadcraft-checks/venv (python -m venv there; pip install ifcopenshell pytest).
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

py = os.path.join(CHECK_TOOLS, 'venv', 'bin', 'python')
if not os.path.exists(py) or subprocess.run([py, '-c', 'import ifcopenshell, ifcopenshell.validate, ifcopenshell.geom'],
                                            capture_output=True).returncode != 0:
    print(f"SKIP: IfcOpenShell is not installed ({py})"); print("PASS 11"); sys.exit(0)
tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
m = model_counts(rr)
code = r'''
import sys, json, ifcopenshell, ifcopenshell.validate, ifcopenshell.geom
f = ifcopenshell.open(sys.argv[1])
log = ifcopenshell.validate.json_logger()
ifcopenshell.validate.validate(f, log, express_rules=True)
s = ifcopenshell.geom.settings()
shapes, failed = 0, []
for e in f.by_type('IfcWall') + f.by_type('IfcSlab'):
    try:
        sh = ifcopenshell.geom.create_shape(s, e)
        shapes += len(sh.geometry.verts) > 0
    except Exception as x:
        failed.append((e.Name, str(x)[:120]))
print(json.dumps({"schema": f.schema, "issues": [str(x)[:300] for x in log.statements][:10], "nissues": len(log.statements),
                  "walls": len(f.by_type('IfcWall')), "openings": len(f.by_type('IfcOpeningElement')),
                  "spaces": len(f.by_type('IfcSpace')), "shapes": shapes, "failed": failed[:5],
                  "slabs": len(f.by_type('IfcSlab'))}))
'''
p = subprocess.run([py, '-c', code, out], capture_output=True, text=True, timeout=600)
if p.returncode != 0:
    fail(f"IfcOpenShell could not read house.ifc: {p.stderr[-600:]}")
r = json.loads(p.stdout.strip().splitlines()[-1])
print(f"IfcOpenShell: {r['schema']}, {r['nissues']} issues, {r['walls']} walls, {r['openings']} openings, {r['spaces']} spaces, "
      f"{r['shapes']} of {r['walls'] + r['slabs']} wall and slab shapes built")
if r['schema'] != 'IFC4':
    fail(f"IfcOpenShell reads the file as IFC4, not {r['schema']}")
if r['nissues']:
    fail(f"IfcOpenShell's validation reports {r['nissues']} issue(s): {r['issues']}")
if (r['walls'], r['openings'], r['spaces']) != (m['walls'], m['openings'], m['rooms']):
    fail(f"IfcOpenShell counts the model's walls, openings and rooms: {(r['walls'], r['openings'], r['spaces'])} vs {(m['walls'], m['openings'], m['rooms'])}")
if r['failed'] or r['shapes'] != r['walls'] + r['slabs'] or r['slabs'] == 0:
    fail(f"IfcOpenShell builds a shape for every wall and slab: {r['shapes']} of {r['walls'] + r['slabs']}; failures {r['failed']}")
print("PASS 11")
