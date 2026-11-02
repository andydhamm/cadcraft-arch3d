# Entity counts equal the model's: IfcWall per wall, IfcDoor/IfcWindow and IfcOpeningElement per opening, IfcSpace per
# room, IfcBuildingStorey per level, IfcStair per stair, IfcRailing per railing, IfcLightFixture per fixture, one IfcRoof
# with an IfcSlab .ROOF. per roof plane, an IfcSlab .FLOOR. per floor and an IfcCovering .CEILING. per ceiling.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc'); glbp = os.path.join(tmp, 'house.glb')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
m = model_counts(rr)
export(HOUSE, glbp)
j, _ = glb(glbp)
names = [n.get('name') or '' for n in j.get('nodes', []) if 'mesh' in n]
want_glb = {'roof planes': sum(1 for n in names if re.fullmatch(r'roof-r\d+p\d+', n)),
            'floors': sum(1 for n in names if n.startswith('floor-r')), 'ceilings': sum(1 for n in names if n.startswith('ceiling-r')),
            'roofs': len({re.match(r'roof-r(\d+)', n).group(1) for n in names if re.match(r'roof-r\d+', n)})}
header, inst = step_file(out)
cnt = lambda e: len(of_type(inst, e))
slabs = [a for _, a in of_type(inst, 'IFCSLAB')]
cover = [a for _, a in of_type(inst, 'IFCCOVERING')]
got = {'walls': cnt('IFCWALL'), 'doors': cnt('IFCDOOR'), 'windows': cnt('IFCWINDOW'), 'opening elements': cnt('IFCOPENINGELEMENT'),
       'spaces': cnt('IFCSPACE'), 'storeys': cnt('IFCBUILDINGSTOREY'), 'stairs': cnt('IFCSTAIR'), 'railings': cnt('IFCRAILING'),
       'lights': cnt('IFCLIGHTFIXTURE'), 'roofs': cnt('IFCROOF'), 'roof planes': sum(1 for a in slabs if a[-1] == 'ROOF'),
       'floors': sum(1 for a in slabs if a[-1] == 'FLOOR'), 'ceilings': sum(1 for a in cover if a[-1] == 'CEILING')}
want = {'walls': m['walls'], 'doors': m['doors'], 'windows': m['windows'], 'opening elements': m['openings'], 'spaces': m['rooms'],
        'storeys': m['levels'], 'stairs': m['stairs'], 'railings': m['railings'], 'lights': m['lights'],
        'roofs': want_glb['roofs'], 'roof planes': want_glb['roof planes'], 'floors': want_glb['floors'], 'ceilings': want_glb['ceilings']}
print("model / IFC:", ", ".join(f"{k} {want[k]}/{got[k]}" for k in want))
if m['walls'] < 9 or m['openings'] != 3 or m['levels'] != 2 or m['stairs'] != 1 or m['lights'] != 4 or want['roof planes'] < 2:
    fail(f"the demo house did not build as the check expects: {m['raw'] and {k: v for k, v in m.items() if k != 'raw'}}, {want_glb}")
diff = {k: (want[k], got[k]) for k in want if want[k] != got[k]}
if diff:
    fail(f"IFC entity counts differ from the model (model, IFC): {diff}")
print("PASS 03")
