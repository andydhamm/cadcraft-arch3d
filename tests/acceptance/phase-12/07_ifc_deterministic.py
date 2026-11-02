# Two exports of one model with one SOURCE_DATE_EPOCH are byte-identical; another time stamp changes only the header; an
# added wall adds one IfcWall and every other object keeps its GlobalId (ids come from ArchIds, not from file order).
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp()
for x in 'abcd':
    os.makedirs(os.path.join(tmp, x))
a, b, c, d = (os.path.join(tmp, x, 'house.ifc') for x in 'abcd')      # one file name: FILE_NAME is in the header
export(HOUSE, a); export(HOUSE, b)
if open(a, 'rb').read() != open(b, 'rb').read():
    fail("two exports of one model with one SOURCE_DATE_EPOCH are byte-identical")
export(HOUSE, c, epoch=EPOCH + 86400)
ta, tc = open(a).read(), open(c).read()
if ta == tc or ta[ta.index('DATA;'):] != tc[tc.index('DATA;'):]:
    fail("another time stamp changes the header only")
if "'2026-11-02T18:00:00'" not in tc:
    fail("the header's time stamp follows SOURCE_DATE_EPOCH")
export(HOUSE + [cmd('wall', {"points": [[12, 0], [12, 4]], "thickness": 0.2, "height": 2.7})], d)


def ids(path):
    _, inst = step_file(path)
    return {(e, decode_step_string(x[2]) if isinstance(x[2], str) else None): x[0] for e, x in inst.values()
            if e in ('IFCWALL', 'IFCDOOR', 'IFCWINDOW', 'IFCSPACE', 'IFCSTAIR', 'IFCBUILDINGSTOREY', 'IFCLIGHTFIXTURE', 'IFCSLAB')}


before, after = ids(a), ids(d)
new = [k for k in after if k not in before]
changed = [k for k in before if after.get(k) != before[k]]
print(f"{len(before)} objects; after one more wall: new {new}, changed ids {changed[:5]}")
if len([k for k in new if k[0] == 'IFCWALL']) != 1:
    fail(f"an added wall adds one IfcWall: {new}")
if changed:
    fail(f"every other object keeps its GlobalId: {changed[:5]}")
print("PASS 07")
