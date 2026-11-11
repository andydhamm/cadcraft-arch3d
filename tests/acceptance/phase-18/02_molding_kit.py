#!/usr/bin/env python3
# The molding kit (the brief's "profiles are library items ... generated in Rust ... plus any closed 2D polyline"):
# the Moldings folder of Cabinet parts lists exactly molding:base (15 x 90 mm), molding:shoe, molding:crown (100 x
# 100 mm), molding:chair-rail and molding:casing, each kind "molding" with its profile's size [w, 1.0, h]; an icon of
# one renders deterministically and is not blank; a closed polyline drawn in the plan becomes a user molding item
# through `library.add {"kind": "molding"}` with the polyline's bounding size, while an open polyline and a bow-tie
# are refused; `library {"for": "room/r0/molding"}` answers the Moldings folder and a missing room is refused.
# A stub fails: no Moldings folder, wrong sizes, a blank icon, no user item, a `for` query that dies on the key.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-molding-kit-1, p18-molding-kit-2
import os, base64, hashlib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

WANT = ['molding:base', 'molding:shoe', 'molding:crown', 'molding:chair-rail', 'molding:casing']


def library_items(rows):
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('items'), list)]
    if not hits:
        fail(f"no library reply: {json.dumps(rows)[-400:]}")
    return hits[-1]['items']


def item_reply(rows, what):
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('item'), dict)]
    if not hits:
        fail(f"{what}: no library.item reply: {json.dumps(rows)[-400:]}")
    return hits[-1]['item']


# 1. The five built-in profiles.
rr = run_ok([cmd('library', {"chips": [{"kind": "folder", "value": "Moldings"}]}),
             cmd('library.item', {"id": "molding:base"}), cmd('library.item', {"id": "molding:crown"})], "the Moldings folder")
items = library_items(rr)
ids = sorted(i.get('id') for i in items)
if ids != sorted(WANT):
    fail(f"the Moldings folder lists exactly {WANT}, got {ids}")
for i in items:
    if i.get('kind') != 'molding' or i.get('category') != 'Cabinet parts':
        fail(f"{i.get('id')} is a molding of Cabinet parts: kind {i.get('kind')!r}, category {i.get('category')!r}")
base = [r for r in rr if isinstance(r, dict) and isinstance(r.get('item'), dict) and r['item'].get('id') == 'molding:base']
crown = [r for r in rr if isinstance(r, dict) and isinstance(r.get('item'), dict) and r['item'].get('id') == 'molding:crown']
if not base or not crown:
    fail(f"library.item answers for molding:base and molding:crown: {json.dumps(rr)[-500:]}")
for rep, want, what in ((base[0]['item'], [0.015, 1.0, 0.09], 'base 15 x 90'), (crown[0]['item'], [0.1, 1.0, 0.1], 'crown 100 x 100')):
    size = rep.get('size')
    if not (isinstance(size, list) and len(size) == 3):
        fail(f"{what}: size [w, 1.0, h], got {size!r}")
    for g, w in zip(size, want):
        near(g, w, 1e-12, f"{what} size")
print("ok: five built-in moldings, base 0.015 x 0.09 and crown 0.1 x 0.1")

# 2. The icon: deterministic, 128 x 128, not blank.
rr = run_ok([cmd('library.icon', {"item": "molding:crown", "cache": False}), cmd('library.icon', {"item": "molding:crown", "cache": False})], "icons")
pngs = [r for r in rr if isinstance(r, dict) and isinstance(r.get('png'), str)]
if len(pngs) != 2:
    fail(f"two library.icon replies with a png: {len(pngs)}")
data = [base64.b64decode(p['png']) for p in pngs]
if hashlib.sha256(data[0]).hexdigest() != hashlib.sha256(data[1]).hexdigest():
    fail("two icon renders of molding:crown are byte-identical")
tmp = tempfile.mkdtemp()
path = os.path.join(tmp, 'crown.png')
open(path, 'wb').write(data[0])
w, h, nch, rows = png(path)
if (w, h) != (128, 128):
    fail(f"the icon is 128 x 128, got {w} x {h}")
fg, sd = foreground(path)
if not (0.02 < fg < 0.9):
    fail(f"the icon's non-background fraction is in (0.02, 0.9): {fg:.3f}")
print("ok: the crown icon is deterministic, 128 x 128 and drawn")

# 3. A drawn closed polyline becomes a molding item.
PL = cmd('pline', {"vertices": [[0, 0], [0.02, 0], [0.02, 0.05], [0, 0.07]], "closed": True})
rr = run_ok([PL], "a closed polyline")
hrep = [r for r in rr if isinstance(r, dict) and isinstance(r.get('handle'), str)]
if not hrep:
    fail(f"pline replies with its handle: {json.dumps(rr)[-300:]}")
H = hrep[-1]['handle']
rr = run_ok([PL, cmd('library.add', {"objects": [H], "name": "Wainscot cap", "kind": "molding"}),
             cmd('library.item', {"id": "user:wainscot-cap"}), cmd('library', {"text": "wainscot"})], "library.add kind molding")
added = [r for r in rr if isinstance(r, dict) and isinstance(r.get('item'), str)]
if not added or added[-1]['item'] != 'user:wainscot-cap':
    fail(f"library.add replies {{item: user:wainscot-cap}}: {json.dumps(rr)[-400:]}")
it = item_reply(rr, "the user molding")
if it.get('kind') != 'molding' or it.get('folder') != 'Moldings':
    fail(f"user:wainscot-cap is a molding in the Moldings folder: kind {it.get('kind')!r}, folder {it.get('folder')!r}")
size = it.get('size')
if not (isinstance(size, list) and len(size) == 3):
    fail(f"the user molding has a size: {size!r}")
near(size[0], 0.02, 1e-9, "the user molding's width")
near(size[2], 0.07, 1e-9, "the user molding's height")
found = [i.get('id') for i in library_items(rr)]
if 'user:wainscot-cap' not in found:
    fail(f"`library {{text: wainscot}}` finds the new item: {found}")
for bad_pl, what in ((cmd('pline', {"vertices": [[0, 0], [0.02, 0], [0.02, 0.05]]}), "an open polyline"),
                     (cmd('pline', {"vertices": [[0, 0], [0.02, 0.05], [0.02, 0], [0, 0.05]], "closed": True}), "a bow-tie")):
    rr = run_ok([bad_pl], what)
    hb = [r for r in rr if isinstance(r, dict) and isinstance(r.get('handle'), str)][-1]['handle']
    rr, se = refused([bad_pl, cmd('library.add', {"objects": [hb], "name": "Bad", "kind": "molding"})], f"{what} as a molding")
    if any(isinstance(r, dict) and isinstance(r.get('item'), str) for r in rr):
        fail(f"{what}: refused and no item added")
print("ok: a closed polyline becomes user:wainscot-cap (0.02 x 0.07); an open one and a bow-tie are refused")

# 4. The `for` targets.
rr = run_ok(MOLD_ROOM + [cmd('library', {"for": "room/r0/molding"})], "library for room/r0/molding")
q = [r for r in rr if isinstance(r, dict) and isinstance(r.get('query'), dict)]
chips = (q[-1]['query'].get('chips') if q else None) or []
if not any(isinstance(c, dict) and c.get('kind') == 'folder' and c.get('value') == 'Moldings' for c in chips):
    fail(f"`library {{for: room/r0/molding}}` queries the Moldings folder: {json.dumps(q[-1] if q else rr[-1:])[:300]}")
refused(MOLD_ROOM + [cmd('library', {"for": "room/r9/molding"})], "a for-target naming a missing room")
print("ok: for room/r0/molding -> the Moldings folder; room/r9 refused")
print("PASS 02")
