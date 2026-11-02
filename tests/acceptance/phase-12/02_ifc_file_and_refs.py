# The IFC file: ISO 10303-21 framing and header, FILE_SCHEMA(('IFC4')), every #n referenced is defined, every IfcRoot
# entity has a unique 22-character GlobalId, one project, metres, and every product has a tessellated body.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
export(HOUSE, out)
header, inst = step_file(out)
if "FILE_SCHEMA(('IFC4'));" not in header:
    fail(f"the header says FILE_SCHEMA(('IFC4')): {header[:400]}")
if "FILE_NAME('house.ifc','2026-11-01T18:00:00'" not in header:
    fail(f"FILE_NAME carries the file's name and the SOURCE_DATE_EPOCH time: {header[:400]}")
missing = sorted({(n, r) for n, (e, a) in inst.items() for r in refs_of(a) if r not in inst})
if missing:
    fail(f"references to undefined instances (#from, #to): {missing[:10]}")
ALPHA = set('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$')
ROOTS = ('IFCPROJECT', 'IFCSITE', 'IFCBUILDING', 'IFCBUILDINGSTOREY', 'IFCWALL', 'IFCDOOR', 'IFCWINDOW', 'IFCOPENINGELEMENT',
         'IFCSLAB', 'IFCCOVERING', 'IFCROOF', 'IFCSTAIR', 'IFCRAILING', 'IFCSPACE', 'IFCGEOGRAPHICELEMENT', 'IFCLIGHTFIXTURE',
         'IFCRELAGGREGATES', 'IFCRELCONTAINEDINSPATIALSTRUCTURE', 'IFCRELVOIDSELEMENT', 'IFCRELFILLSELEMENT', 'IFCPROPERTYSET',
         'IFCRELDEFINESBYPROPERTIES', 'IFCELEMENTQUANTITY')
ids = [(e, a[0]) for e, a in inst.values() if e in ROOTS]
bad = [x for x in ids if not (isinstance(x[1], str) and len(x[1]) == 22 and set(x[1]) <= ALPHA and x[1][0] in '0123')]
if bad:
    fail(f"GlobalIds are 22 characters of IFC's base-64 alphabet: {bad[:5]}")
dup = [g for g, c in __import__('collections').Counter(g for _, g in ids).items() if c > 1]
if dup:
    fail(f"GlobalIds repeat: {dup[:5]}")
if len(of_type(inst, 'IFCPROJECT')) != 1:
    fail("one IfcProject")
units = [a for _, a in of_type(inst, 'IFCSIUNIT')]
if not any(a[1] == 'LENGTHUNIT' and a[3] == 'METRE' and a[2] is None for a in units):
    fail(f"lengths are in metres (IfcSIUnit .LENGTHUNIT. without a prefix): {units}")
PRODUCTS = ('IFCWALL', 'IFCDOOR', 'IFCWINDOW', 'IFCSLAB', 'IFCCOVERING', 'IFCSTAIR', 'IFCRAILING', 'IFCGEOGRAPHICELEMENT', 'IFCLIGHTFIXTURE')
nprod = 0
for name in PRODUCTS:
    for n, a in of_type(inst, name):
        pts, tris = face_set_points(inst, a)
        nprod += 1
        if any(not (1 <= k <= len(pts)) for t in tris for k in t):
            fail(f"#{n} {name}: a triangle indexes past its points")
print(f"house.ifc: {len(inst)} instances, {len(ids)} rooted entities with unique GlobalIds, {nprod} products with bodies")
if nprod < 20:
    fail(f"the demo house has walls, openings, floors, ceilings, a roof, a stair, railings, plants, terrain and lights: only {nprod} products with a body")
print("PASS 02")
