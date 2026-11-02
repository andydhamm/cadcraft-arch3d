# One IfcOpeningElement per opening, voiding its wall (IfcRelVoidsElement) and filled (IfcRelFillsElement) by an IfcDoor
# or IfcWindow whose OverallHeight and OverallWidth are the opening's; each opening element has a body (its void) that
# sits inside the wall's thickness, and the door or window is in the wall's storey.
# needs: p12-ifc-openings
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'house.ifc')
rc, rr, so, se = export(HOUSE + [INSPECT], out)
m = model_counts(rr)
header, inst = step_file(out)
voids = [a for _, a in of_type(inst, 'IFCRELVOIDSELEMENT')]
fills = [a for _, a in of_type(inst, 'IFCRELFILLSELEMENT')]
ops = [n for n, _ in of_type(inst, 'IFCOPENINGELEMENT')]
print(f"{m['openings']} openings: {len(ops)} opening elements, {len(voids)} void and {len(fills)} fill relations")
if len(ops) != m['openings'] or len(voids) != m['openings'] or len(fills) != m['openings']:
    fail(f"one opening element, one void and one fill per opening: {m['openings']} openings, {len(ops)}, {len(voids)}, {len(fills)}")
voided = {a[5].n: a[4].n for a in voids}
filled = {a[4].n: a[5].n for a in fills}
if set(voided) != set(ops) or set(filled) != set(ops):
    fail("each opening element is voided into one wall and filled once")
cont = contained_in(inst)
by_size = sorted((round(o['width'], 3), round(o['height'], 3), o['kind']) for o in m['raw']['openings'])
got = []
for op in ops:
    wall = voided[op]
    if inst[wall][0] != 'IFCWALL':
        fail(f"opening element #{op} voids #{wall}, a {inst[wall][0]}, not an IfcWall")
    fill = filled[op]
    e, a = inst[fill]
    if e not in ('IFCDOOR', 'IFCWINDOW'):
        fail(f"opening element #{op} is filled by a {e}")
    got.append((round(a[9], 3), round(a[8], 3), 'door' if e == 'IFCDOOR' else 'window'))   # OverallWidth, OverallHeight
    if cont.get(fill) != cont.get(wall):
        fail(f"{e} #{fill} is in its wall's storey: {cont.get(fill)} vs {cont.get(wall)}")
    pts, _ = face_set_points(inst, inst[op][1])
    wpts, _ = face_set_points(inst, inst[wall][1])
    lo = [min(p[k] for p in wpts) - 0.01 for k in range(3)]; hi = [max(p[k] for p in wpts) + 0.01 for k in range(3)]
    if any(not (lo[k] <= p[k] <= hi[k]) for p in pts for k in range(3)):
        fail(f"opening element #{op}'s void lies within its wall's box {lo}..{hi}")
if sorted(got) != by_size:
    fail(f"doors and windows carry the openings' width and height: model {by_size}, IFC {sorted(got)}")
print("PASS 05")
