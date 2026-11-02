# The site and lights export too: IfcSite RefLatitude / RefLongitude (compound angles) and RefElevation from the site
# location; the terrain and each plant as IfcGeographicElements in the site; one IfcLightFixture per fixture (named
# light-lf<n>) in its storey, with a CADCraft_Light property set (Lumens, ColourTemperature).
# needs: p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
m = model_counts(rr)
header, inst = step_file(out)
(sn, site), = of_type(inst, 'IFCSITE')
lat, lon, elev = site[9], site[10], site[11]
if not isinstance(lat, list) or not isinstance(lon, list):
    fail(f"IfcSite carries RefLatitude and RefLongitude: {lat!r}, {lon!r}")
la, lo = compound_degrees(lat), compound_degrees(lon)
print(f"IfcSite: latitude {la:.7f}, longitude {lo:.7f}, elevation {elev}")
near(la, 51.05, 1e-6, 'RefLatitude (degrees)'); near(lo, -114.07, 1e-6, 'RefLongitude (degrees)')
if not (all(x <= 0 for x in lon) and all(x >= 0 for x in lat)):
    fail(f"every part of a compound angle carries the angle's sign: {lat}, {lon}")
near(num(elev, 'RefElevation'), 1045.0, 1e-9, 'RefElevation (m)')
cont = contained_in(inst)
geo = of_type(inst, 'IFCGEOGRAPHICELEMENT')
kinds = sorted((decode_step_string(a[2] or ''), a[-1], a[4]) for _, a in geo)
print(f"geographic elements: {kinds}")
if sum(1 for _, a in geo if a[-1] == 'TERRAIN') != 1:
    fail(f"one terrain (.TERRAIN.): {kinds}")
if sum(1 for _, a in geo if a[-1] == 'USERDEFINED' and a[4] == 'PLANT') != 2:
    fail(f"each of the two plants is an IfcGeographicElement .USERDEFINED. with ObjectType 'PLANT': {kinds}")
if any(cont.get(n) != sn for n, _ in geo):
    fail("the geographic elements are contained in the site")
lights = of_type(inst, 'IFCLIGHTFIXTURE')
storeys = {n for n, _ in of_type(inst, 'IFCBUILDINGSTOREY')}
names = sorted(decode_step_string(a[2]) for _, a in lights)
if len(lights) != m['lights'] or any(not re.fullmatch(r'light-lf\d+', x) for x in names):
    fail(f"one IfcLightFixture per fixture, named light-lf<n>: {names} for {m['lights']} fixtures")
if any(cont.get(n) not in storeys for n, _ in lights):
    fail("each light fixture is contained in its storey")
psets = {}
for _, rel in of_type(inst, 'IFCRELDEFINESBYPROPERTIES'):
    e, d = deref(inst, rel[5])
    if e == 'IFCPROPERTYSET' and d[2] == 'CADCraft_Light':
        vals = {deref(inst, r)[1][0]: deref(inst, r)[1][2] for r in d[4]}
        for o in rel[4]:
            psets[o.n] = vals
for n, a in lights:
    v = psets.get(n)
    if not v or 'Lumens' not in v or 'ColourTemperature' not in v:
        fail(f"{decode_step_string(a[2])} has a CADCraft_Light property set with Lumens and ColourTemperature: {v}")
    lum = v['Lumens'].value if isinstance(v['Lumens'], Typed) else v['Lumens']
    if abs(lum - 800) > 1e-6:
        fail(f"a pot light gives 800 lm: {v}")
print(f"ok: site at 51.05, -114.07, 1045 m; {len(geo)} geographic elements; {len(lights)} light fixtures")
print("PASS 09")
