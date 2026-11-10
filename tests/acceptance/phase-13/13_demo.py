# The phase-13 demo: the phase-12 house (two levels, stair, roof, site, lights) finished in siding with a brick front,
# a shingled roof and a wood floor, a kitchen of base and wall cabinet runs with a sink, a range, a refrigerator and a
# dishwasher, and an upstairs bath. It renders outside and in the kitchen (textured), exports a GLB the glTF validator
# accepts with its four textures, an IFC that IfcOpenShell reads with the casework (both SKIP when the tool is missing),
# and survives a DXF round trip.
# needs: p13-doc-finish, p13-texture-gen, p13-arch3d-finish-meshes, p13-glb-textures, p13-render3d-textures, p13-doc-casework, p13-arch-casework-symbols, p13-arch3d-casework, p13-cmd-casework, p13-dxf-casework, p13-io-casework
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp()
# HOUSE: w0..w3 the ground ring (counter-clockwise, 10 x 8), w4 the partition at y = 3, w5.. the upper ring; r1 the
# kitchen (y 3-8). The kitchen runs along w2 (from (10,8) towards (0,8); inner face y = 7.9).
DEMO = HOUSE + [cmd('material', {"walls": "exterior", "finish": "siding"}),
                cmd('material', {"walls": ["w0"], "finish": "brick"}),
                cmd('material', {"roofs": "all", "finish": "shingles"}),
                cmd('material', {"rooms": ["r1"], "finish": "wood"}),
                cmd('cabinet', {"kind": "base", "wall": "w2", "offset": 1.0, "widths": [0.6, 0.9, 0.6]}),
                cmd('cabinet', {"kind": "wall", "wall": "w2", "offset": 1.0, "widths": [0.6, 0.9, 0.6]}),
                cmd('fixture', {"kind": "sink", "at": [8.25, 7.9], "rotation": 180}),
                cmd('fixture', {"kind": "range", "wall": "w2", "offset": 3.2}),
                cmd('fixture', {"kind": "dishwasher", "wall": "w2", "offset": 4.0}),
                cmd('fixture', {"kind": "refrigerator", "wall": "w1", "offset": 6.5}),
                cmd('fixture', {"kind": "toilet", "at": [1.0, 0.1], "level": "Level 2"}),
                cmd('fixture', {"kind": "lavatory", "at": [2.0, 0.1], "level": "Level 2"}),
                cmd('fixture', {"kind": "bathtub", "at": [3.6, 0.1], "level": "Level 2"})]
saved = os.path.join(tmp, 'demo.dxf')
outside, kitchen = os.path.join(tmp, 'demo-exterior.png'), os.path.join(tmp, 'demo-kitchen.png')
rc, rr, so, se = run(DEMO + [INSPECT_ALL,
                             cmd('render', {"out": outside, "size": [800, 600], "style": "shaded",
                                            "camera": {"kind": "full", "eye": [-7, -11, 6], "target": [5, 4, 2.5], "fov": 55}}),
                             cmd('render', {"out": kitchen, "size": [800, 600], "style": "shaded",
                                            "camera": {"kind": "full", "eye": [5.0, 4.0, 1.6], "target": [8.5, 7.9, 0.9], "fov": 75}})],
                     save=saved)
if was_refused(rc, se):
    fail(f"every demo step runs: {[l for l in se.splitlines() if l.startswith('error')][:5]}")
ins = inspected(rr)
cabs, fx = objects_named(ins, 'cabinets', 'cabinets'), objects_named(ins, 'fixtures', 'fixtures')
if len(cabs) != 6 or len(fx) != 7:
    fail(f"six cabinets and seven fixtures: {len(cabs)}, {len(fx)}")
if [f.get('level') for f in fx][-3:] != [1, 1, 1] and [f.get('level') for f in fx][-3:] != ['Level 2'] * 3:
    fail(f"the bath fixtures are on Level 2: {[f.get('level') for f in fx]}")
for name, path in (('exterior', outside), ('kitchen', kitchen)):
    if not os.path.exists(path):
        fail(f"the {name} render was written")
    w, h, nch, rows = png(path)
    flat = [0.2126 * r[x * nch] + 0.7152 * r[x * nch + 1] + 0.0722 * r[x * nch + 2] for r in rows[::4] for x in range(0, w, 4)]
    if statistics.pstdev(flat) < 10:
        fail(f"the {name} render shows the scene (luminance sd {statistics.pstdev(flat):.1f})")
    artifact(path, f'demo-{name}.png')
print("ok: the finished house renders outside and in the kitchen")

g, i = os.path.join(tmp, 'demo.glb'), os.path.join(tmp, 'demo.ifc')
export(DEMO, g)
export(DEMO, i)
j, binc = glb(g)
# Phase 19 adds normalTexture images named <base>-normal; the four base-colour textures are what this demo shows.
base_imgs = sorted(k for k in glb_images(j, binc) if not k.endswith('-normal'))
if base_imgs != ['brick', 'shingles', 'siding', 'wood']:
    fail(f"the GLB carries the four textures: {base_imgs}")
names = [n.get('name') for n in j.get('nodes', []) if 'mesh' in n]
if sum(1 for n in names if re.fullmatch(r'cabinet-cb\d+', n)) != 6 or sum(1 for n in names if re.fullmatch(r'fixture-fx\d+', n)) != 7:
    fail(f"the GLB has six cabinet and seven fixture nodes: {sorted(n for n in names if n.startswith(('cabinet', 'fixture')))}")
validate_glb(g)
hdr, inst = step_file(i)
counts = {e: len(of_type(inst, e)) for e in ('IFCFURNITURE', 'IFCSANITARYTERMINAL', 'IFCELECTRICAPPLIANCE')}
if counts != {'IFCFURNITURE': 6, 'IFCSANITARYTERMINAL': 4, 'IFCELECTRICAPPLIANCE': 3}:
    fail(f"IFC: 6 furniture, 4 sanitary terminals, 3 electric appliances: {counts}")
py = os.path.join(CHECK_TOOLS, 'venv', 'bin', 'python')
if os.path.exists(py) and subprocess.run([py, '-c', 'import ifcopenshell, ifcopenshell.validate, ifcopenshell.geom'],
                                         capture_output=True).returncode == 0:
    code = r'''
import sys, json, ifcopenshell, ifcopenshell.validate, ifcopenshell.geom
f = ifcopenshell.open(sys.argv[1])
log = ifcopenshell.validate.json_logger()
ifcopenshell.validate.validate(f, log, express_rules=True)
s = ifcopenshell.geom.settings()
built, failed = 0, []
for e in f.by_type('IfcFurniture') + f.by_type('IfcSanitaryTerminal') + f.by_type('IfcElectricAppliance'):
    try:
        built += len(ifcopenshell.geom.create_shape(s, e).geometry.verts) > 0
    except Exception as x:
        failed.append((e.Name, str(x)[:120]))
print(json.dumps({"schema": f.schema, "nissues": len(log.statements), "issues": [str(x)[:300] for x in log.statements][:8],
                  "built": built, "failed": failed[:5]}))
'''
    p = subprocess.run([py, '-c', code, i], capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        fail(f"IfcOpenShell could not read demo.ifc: {p.stderr[-600:]}")
    r = json.loads(p.stdout.strip().splitlines()[-1])
    if r['schema'] != 'IFC4' or r['nissues'] or r['failed'] or r['built'] != 13:
        fail(f"IfcOpenShell reads demo.ifc as IFC4 without issues and builds all 13 casework shapes: {r}")
    print(f"ok: IfcOpenShell: IFC4, no issues, 13 casework shapes")
else:
    print(f"SKIP IfcOpenShell: not installed ({py})")

rc, rr2, so2, se2 = run([INSPECT_ALL], file=saved)
ins2 = inspected(rr2)
if len(objects_named(ins2, 'cabinets', '')) != 6 or len(objects_named(ins2, 'fixtures', '')) != 7 or \
        by_id(ins2['walls'], 'w0', 'walls').get('finish') != 'brick':
    fail("the demo DXF reopens with its cabinets, fixtures and finishes")
print("ok: DXF round trip keeps casework and finishes")
print("PASS 13")
