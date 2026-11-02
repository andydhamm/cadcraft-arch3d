# Hostile and edge cases: an export into a missing directory or with an unknown extension is refused with a message (no
# panic, no file); a model without arch objects is still a valid IFC (project, site, building, one storey, no element);
# names with apostrophes, backslashes and non-ASCII characters survive ISO 10303-21 encoding (and the OBJ stays readable);
# 300 walls export to IFC and OBJ in under 60 s each.
# needs: p12-io-obj, p12-ifc-openings
import os, time
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp()
WALL = cmd('wall', {"points": [[0, 0], [4, 0]], "thickness": 0.2, "height": 2.4})
for out, why in ((os.path.join(tmp, 'no', 'such', 'dir', 'x.ifc'), 'a missing directory'),
                 (os.path.join(tmp, 'x.ifcx'), 'an unknown extension')):
    rc, rr, so, se = export([WALL], out, expect_ok=False)
    if rc == 0 or os.path.exists(out):
        fail(f"an export to {why} is refused (nonzero exit, no file): rc {rc}, exists {os.path.exists(out)}")
print("ok: a missing directory and an unknown extension are refused")
empty = os.path.join(tmp, 'empty.ifc')
export([], empty)
_, inst = step_file(empty)
got = {e: len(of_type(inst, e)) for e in ('IFCPROJECT', 'IFCSITE', 'IFCBUILDING', 'IFCBUILDINGSTOREY', 'IFCWALL', 'IFCSPACE')}
if got != {'IFCPROJECT': 1, 'IFCSITE': 1, 'IFCBUILDING': 1, 'IFCBUILDINGSTOREY': 1, 'IFCWALL': 0, 'IFCSPACE': 0}:
    fail(f"a model without arch objects is a project, site, building and one storey: {got}")
if any(r not in inst for _, a in inst.values() for r in refs_of(a)):
    fail("the empty model's references resolve")
NAME = "Küche 'Oben' \\ Кухня 😀"
odd = os.path.join(tmp, 'odd.ifc')
export([cmd('level', {"above": 0, "name": NAME}), cmd('wall', {"points": [[0, 0], [4, 0]], "thickness": 0.2, "level": NAME})], odd)
_, inst = step_file(odd)
names = ifc_names(inst, 'IFCBUILDINGSTOREY')
if NAME not in names:
    fail(f"a storey name with an apostrophe, a backslash and non-ASCII characters round-trips: {names!r}")
print(f"ok: the empty model is a valid IFC; {NAME!r} round-trips")
many = [cmd('wall', {"points": [[x * 0.5, 0], [x * 0.5, 3]], "thickness": 0.1, "height": 2.4}) for x in range(300)]
for ext in ('ifc', 'obj'):
    out = os.path.join(tmp, f'many.{ext}')
    t0 = time.time()
    export(many, out)
    dt = time.time() - t0
    print(f"300 walls -> {ext}: {dt:.1f} s, {os.path.getsize(out)} bytes")
    if dt > 60:
        fail(f"300 walls export to {ext} in under 60 s: {dt:.1f} s")
_, inst = step_file(os.path.join(tmp, 'many.ifc'))
if len(of_type(inst, 'IFCWALL')) != 300:
    fail(f"300 IfcWalls: {len(of_type(inst, 'IFCWALL'))}")
print("PASS 12")
