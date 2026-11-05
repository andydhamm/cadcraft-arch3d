#!/usr/bin/env python3
# A copied library item places like its source. Phase 14 left Copy to User Catalog's copies unplaceable (`library.place`
# on one said "user item `user:sofa-futon` has no objects"; operator 11-05). A copy of the stock futon places as the
# futon (its mesh, category, layer and the item id of its source, which IFC and the licence notes key on) under the
# copy's name; an edited copy's size is the placed size; a copied sink still drops into a base cabinet's counter and
# keeps its layer; a copy of a copy places as the same source with the copied name and size, even once the copy in
# between is deleted; a copy of a library.add group places as the group; a bad copy is an error, never a panic.
# needs: p15-cmd-library-copy-place
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

tmp = tempfile.mkdtemp()
BIG = room_cmds(0, 0, 10, 4)


def session(cmds, what, glb_name=None):
    """One cadcraft-cli session that must run without an error (the CLI reports a failed command as `error: ...` on
    stderr and goes on); with `glb_name` it also exports a GLB. Returns the replies (and the GLB)."""
    out = os.path.join(tmp, glb_name) if glb_name else None
    rc, rr, so, se = run(cmds + ([cmd('export', {"path": out})] if out else []))
    if errors(se):
        fail(f"{what}: {errors(se)[0][:400]}")
    return (rr, glb(out)) if out else rr


def placed(rr, what):
    """The furnishing a `library.place` of one symbol made (its reply's `furnishing` label), as arch.inspect lists it."""
    label = reply_with(rr, 'furnishing', -1, what).get('furnishing')
    fs = {f['id']: f for f in furnishings_of(inspected(rr))}
    if label not in fs:
        fail(f"{what}: placed {label!r}, inspect lists {sorted(fs)}")
    return fs[label], fs


def same_size(a, b):
    return isinstance(a, list) and isinstance(b, list) and len(a) == len(b) == 3 and all(abs(x - y) < 1e-9 for x, y in zip(a, b))


# ---- a copy of the futon, renamed, placed 4 m along from the stock futon in a new session.
rr = session([cmd('library.copy', {"item": "sofa-futon"}),
                cmd('library.edit', {"item": "user:sofa-futon", "name": "Guest futon"})], 'copy and rename the futon')
if reply_with(rr, 'item', 0, 'library.copy').get('item') != 'user:sofa-futon':
    fail(f"library.copy of sofa-futon: {reply_with(rr, 'item', 0)}")
rr, (j, binc) = session(BIG + [cmd('library.place', {"item": "sofa-futon", "at": [2, 2]}),
                               cmd('library.place', {"item": "user:sofa-futon", "at": [6, 2]}), INSPECT14],
                         'place the futon and its copy', 'copy.glb')
copy, fs = placed(rr, 'library.place (the copy)')
stock = fs.get('f0')
if not stock or stock['id'] == copy['id']:
    fail(f"the stock futon and its copy: {sorted(fs)}")
for k in ('item', 'category', 'symbolType', 'placement', 'layer'):
    if copy.get(k) != stock.get(k):
        fail(f"the placed copy's {k} is {copy.get(k)!r}, the futon's {stock.get(k)!r}")
if copy.get('name') != 'Guest futon':
    fail(f"the placed copy is named {copy.get('name')!r}, the copy is 'Guest futon'")
if not same_size(copy.get('size'), stock.get('size')):
    fail(f"the placed copy's size {copy.get('size')} differs from the futon's {stock.get('size')}")
if not same_geometry(node_positions(j, binc, f"furnishing-{stock['id']}"), node_positions(j, binc, f"furnishing-{copy['id']}"), 4.0):
    fail(f"furnishing-{copy['id']} is not the futon's mesh moved 4 m")
print("ok: a copy of the futon places as the futon (mesh, item, category, layer) under the copy's name")

# ---- the copy's edited size is the placed size.
rr = session([cmd('library.edit', {"item": "user:sofa-futon", "size": [1.6, 0.8, 0.8]})] + BIG +
             [cmd('library.place', {"item": "user:sofa-futon", "at": [6, 2]}), INSPECT14], 'resize and place the copy')
copy, _ = placed(rr, 'library.place (the resized copy)')
if not same_size(copy.get('size'), [1.6, 0.8, 0.8]):
    fail(f"the copy was resized to [1.6, 0.8, 0.8] and placed at {copy.get('size')}")
print("ok: the copy's edited size is the placed size")

# ---- a copied sink still drops into a base cabinet's counter, like the stock sink (phase-14 check 05's set-up).
rr = session([cmd('library.copy', {"item": "own:sink-kitchen"})], 'copy the sink')
sink_copy = reply_with(rr, 'item', 0, 'library.copy (the sink)').get('item')
if not isinstance(sink_copy, str) or not sink_copy.startswith('user:'):
    fail(f"library.copy of own:sink-kitchen: {reply_with(rr, 'item', 0)}")
CAB = [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.9, "fronts": "drawerOverDoor"})]
results = {}
for item in ('own:sink-kitchen', sink_copy):
    rr = session(ROOM + CAB + [cmd('library.place', {"item": item, "at": [0.95, 0.35]}), INSPECT14], f'place {item} in the cabinet')
    f, _ = placed(rr, f'library.place ({item})')
    cab = {c['id']: c for c in objects_named(inspected(rr), 'cabinets', 'cabinets')}.get('cb0') or {}
    results[item] = (f, cab)
(ref, ref_cab), (got, got_cab) = results['own:sink-kitchen'], results[sink_copy]
if (got.get('host') or {}).get('kind') != 'cabinet' or got['host'].get('cabinet') != 'cb0':
    fail(f"the copied sink's host is {got.get('host')}, the stock sink's {ref.get('host')}")
for k in ('item', 'layer', 'category', 'symbolType'):
    if got.get(k) != ref.get(k):
        fail(f"the copied sink's {k} is {got.get(k)!r}, the stock sink's {ref.get(k)!r}")
if any(abs(a - b) > 1e-6 for a, b in zip(got['at'], ref['at'])) or abs(got['elevation'] - ref['elevation']) > 1e-6:
    fail(f"the copied sink sits at {got['at']} / {got['elevation']}, the stock sink at {ref['at']} / {ref['elevation']}")
if got_cab.get('falseFront') is not True or got_cab.get('cutout') != ref_cab.get('cutout'):
    fail(f"the cabinet under the copied sink: falseFront {got_cab.get('falseFront')}, cutout {got_cab.get('cutout')} "
         f"(under the stock sink: {ref_cab.get('cutout')})")
print("ok: a copied sink drops into the counter like the stock sink and keeps its layer")

# ---- a copy of the copy places as the futon with the copied name and size, after the first copy is deleted.
rr = session([cmd('library.copy', {"item": "user:sofa-futon"})], 'copy the copy')
second = reply_with(rr, 'item', 0, 'library.copy (a copy of a copy)').get('item')
if not isinstance(second, str) or second == 'user:sofa-futon' or not second.startswith('user:'):
    fail(f"library.copy of user:sofa-futon: {reply_with(rr, 'item', 0)}")
rr = session([cmd('library.delete', {"item": "user:sofa-futon"})] + BIG +
             [cmd('library.place', {"item": second, "at": [6, 2]}), INSPECT14], f'delete user:sofa-futon, place {second}')
copy2, _ = placed(rr, 'library.place (a copy of a copy)')
if copy2.get('item') != 'sofa-futon' or copy2.get('name') != 'Guest futon' or not same_size(copy2.get('size'), [1.6, 0.8, 0.8]):
    fail(f"the copy of the copy placed as {copy2}")
print(f"ok: {second}, a copy of a copy, places as the futon with its name and size after user:sofa-futon is deleted")

# ---- a copy of a library.add group places as the group, after the group itself is deleted.
SET = [cmd('library.place', {"item": "table-dining-150", "at": [2, 2]}),
       cmd('library.place', {"item": "chair-dining-wood", "at": [2, 1.4]}),
       cmd('library.place', {"item": "chair-dining-wood", "at": [2, 2.6], "rotation": 180})]
rr = session(BIG + SET + [cmd('library.add', {"objects": ["f0", "f1", "f2"], "name": "Dining set", "insertionPoint": [2, 2]})],
             'library.add the dining set')
if reply_with(rr, 'item', -1, 'library.add').get('item') != 'user:dining-set':
    fail(f"library.add: {reply_with(rr, 'item', -1)}")
rr = session([cmd('library.copy', {"item": "user:dining-set"})], 'copy the group')
gcopy = reply_with(rr, 'item', 0, 'library.copy (a group)').get('item')
rr = session([cmd('library.delete', {"item": "user:dining-set"})] + BIG +
             [cmd('library.place', {"item": gcopy, "at": [7, 2]}), INSPECT14], f'delete the group, place {gcopy}')
g = reply_with(rr, 'furnishings', 0, 'library.place (a copy of a group)')
new = g.get('furnishings')
fs = {f['id']: f for f in furnishings_of(inspected(rr))}
if not isinstance(new, list) or len(new) != 3 or any(n not in fs for n in new):
    fail(f"placing the copy of the group made {new!r}")
if sorted(fs[n]['item'] for n in new) != ['chair-dining-wood', 'chair-dining-wood', 'table-dining-150']:
    fail(f"the copy of the group placed {[fs[n]['item'] for n in new]}")
table = next(fs[n] for n in new if fs[n]['item'] == 'table-dining-150')
if abs(table['at'][0] - 7) > 1e-6 or abs(table['at'][1] - 2) > 1e-6:
    fail(f"the group's table should sit on the insertion point (7, 2): {table['at']}")
print(f"ok: {gcopy}, a copy of the Dining set group, places as the group after the group is deleted")

# ---- refusals: errors, never panics (run() fails on any panic).
for bad in ([cmd('library.copy', {"item": "no-such-item"})],
            [cmd('library.place', {"item": "user:no-such-copy", "at": [1, 1]})],
            [cmd('library.copy', {"item": ""})]):
    rc, rr, so, se = run(BIG + bad, expect_ok=False)
    if not errors(se):
        fail(f"{bad[0]} was not refused")
print("ok: a missing item or copy is refused with an error")
