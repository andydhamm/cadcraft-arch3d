# The spatial structure: IfcProject -> IfcSite -> IfcBuilding -> one IfcBuildingStorey per level (Elevation = the level's,
# placed at it), each IfcSpace aggregated in its level's storey, every element contained in exactly one structure: building
# elements in their level's storey, geographic elements in the site.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
m = model_counts(rr)
levels = sorted(m['raw']['levels'], key=lambda l: l.get('elevation', 0.0))
header, inst = step_file(out)
agg = aggregates(inst)
one = lambda e: [n for n, _ in of_type(inst, e)]
project, site, building = one('IFCPROJECT'), one('IFCSITE'), one('IFCBUILDING')
storeys = one('IFCBUILDINGSTOREY')
if (len(project), len(site), len(building)) != (1, 1, 1):
    fail(f"one project, site and building: {len(project)}, {len(site)}, {len(building)}")
if agg.get(project[0]) != site or agg.get(site[0]) != building:
    fail(f"IfcRelAggregates: project -> site -> building, got {agg}")
if sorted(agg.get(building[0], [])) != sorted(storeys):
    fail(f"the building aggregates every storey: {agg.get(building[0])} vs {storeys}")
elev = [inst[n][1][9] for n in storeys]
want = [l.get('elevation') for l in levels]
print(f"storeys at {elev}; levels at {want}")
if len(elev) != len(want) or any(abs(a - b) > 1e-6 for a, b in zip(sorted(elev), sorted(want))):
    fail(f"one storey per level at its elevation: storeys {elev}, levels {want}")
for n in storeys:
    _, pl = deref(inst, inst[n][1][5])
    _, ax = deref(inst, pl[1])
    _, pt = deref(inst, ax[0])
    if abs(pt[0][2] - inst[n][1][9]) > 1e-6:
        fail(f"storey #{n} is placed at its elevation: z {pt[0][2]} vs {inst[n][1][9]}")
spaces = one('IFCSPACE')
in_storeys = {p for s in storeys for p in agg.get(s, []) if p in spaces}
if in_storeys != set(spaces):
    fail(f"every IfcSpace is aggregated in a storey: {len(in_storeys)} of {len(spaces)}")
cont = contained_in(inst)
ELEMENTS = ('IFCWALL', 'IFCDOOR', 'IFCWINDOW', 'IFCSLAB', 'IFCCOVERING', 'IFCROOF', 'IFCSTAIR', 'IFCRAILING', 'IFCLIGHTFIXTURE')
parts = {p for w in one('IFCROOF') for p in agg.get(w, [])}
for e in ELEMENTS:
    for n in one(e):
        if n in parts:
            continue                     # a roof's planes and coverings are its parts, not contained themselves
        if cont.get(n) not in storeys:
            fail(f"#{n} {e} {ifc_names(inst, e)[one(e).index(n)]!r} is contained in a storey, got {cont.get(n)}")
geo = one('IFCGEOGRAPHICELEMENT')
if not geo or any(cont.get(n) != site[0] for n in geo):
    fail(f"the terrain and plants are IfcGeographicElements contained in the site: {[(n, cont.get(n)) for n in geo]}")
upper = max(storeys, key=lambda n: inst[n][1][9]) if len(storeys) > 1 else None      # the storey at the highest elevation
walls = one('IFCWALL')
upper_walls = [n for n in walls if cont.get(n) == upper]
if len(upper_walls) != 4:
    fail(f"the four upper walls are in the upper storey: {len(upper_walls)}")
print(f"ok: project -> site -> building -> {len(storeys)} storeys; {len(spaces)} spaces; {len(cont)} contained elements")
print("PASS 04")
