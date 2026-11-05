#!/usr/bin/env python3
# The user catalog: library.add turns three objects into a user item that places as a group (the same meshes, moved:
# equal geometry once moved back), kept outside the drawing for the next session; Copy to User Catalog makes an editable
# copy of a stock item (stock items stay read-only); delete goes to the Trash, which restores and empties. Set as Default
# on a cabinet, a door and a furnishing makes the next one of its kind match.
# needs: p14-doc-furnishing, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-io-glb-furnishing, p14-cmd-furnishing-transform, p14-cmd-user-catalog, p14-cmd-defaults, p14-inspect, p14-cmd-casework-parts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
BIG = room_cmds(0, 0, 10, 4)
SET = [cmd('library.place', {"item": "table-dining-150", "at": [2, 2]}),
       cmd('library.place', {"item": "chair-dining-wood", "at": [2, 1.4]}),
       cmd('library.place', {"item": "chair-dining-wood", "at": [2, 2.6], "rotation": 180})]
rc, rr, so, se = run(BIG + SET + [cmd('library.add', {"objects": ["f0", "f1", "f2"], "name": "Dining set", "insertionPoint": [2, 2]})])
add = reply_with(rr, 'item', -1, 'library.add')
if add.get('item') != 'user:dining-set' or add.get('objects') != 3:
    fail(f"library.add: {add}")
print("ok: library.add made user:dining-set from three objects")

# A new session: the user item is still there; placed 5 m along, it is the same three meshes moved 5 m.
rr, (j, binc) = export_glb(BIG + SET + [cmd('library.item', {"id": "user:dining-set"}),
                                        cmd('library.place', {"item": "user:dining-set", "at": [7, 2]}), INSPECT14], tmp, 'group.glb')
rec = reply_with(rr, 'item', 0, 'library.item')['item']
if rec.get('category') != 'User catalog' or rec.get('readOnly') is not False:
    fail(f"the user item in a new session: {rec}")
g = reply_with(rr, 'furnishings', 0, 'library.place (a user item)')
new = g.get('furnishings')
if not isinstance(new, list) or len(new) != 3:
    fail(f"placing the user item made {new!r}")
fs = {f['id']: f for f in furnishings_of(inspected(rr))}
pairs = list(zip(['f0', 'f1', 'f2'], new))
for a, b in pairs:
    if fs[b]['item'] != fs[a]['item'] or abs(fs[b]['at'][0] - fs[a]['at'][0] - 5) > 1e-6 or abs(fs[b]['at'][1] - fs[a]['at'][1]) > 1e-6:
        fail(f"{b} should be {a} moved 5 m: {fs[a]} / {fs[b]}")
    if not same_geometry(node_positions(j, binc, f'furnishing-{a}'), node_positions(j, binc, f'furnishing-{b}'), 5.0):
        fail(f"furnishing-{b} is not furnishing-{a}'s mesh moved 5 m")
handles = [h for h in g.get('handles', [])]
if len(handles) != 3:
    fail(f"the group placement returned handles {handles}")
rr, (j2, binc2) = export_glb(BIG + SET + [cmd('library.place', {"item": "user:dining-set", "at": [7, 2]}),
                                          cmd('move', {"handles": handles, "delta": [-5, 0]})], tmp, 'back.glb')
for a, b in pairs:
    if not same_geometry(node_positions(j2, binc2, f'furnishing-{a}'), node_positions(j2, binc2, f'furnishing-{b}')):
        fail(f"moved back, furnishing-{b} differs from furnishing-{a}")
print("ok: the user item persists, places as a group of the same meshes, equal once moved back")

# Copy to User Catalog, edit, Trash.
rc, rr, so, se = run([cmd('library.copy', {"item": "sofa-futon"}),
                      cmd('library.edit', {"item": "user:sofa-futon", "name": "Guest futon", "keywords": ["guest"], "notes": "for the den"}),
                      cmd('library.item', {"id": "user:sofa-futon"}),
                      cmd('library.delete', {"item": "user:dining-set"}),
                      cmd('library', {"chips": [{"kind": "category", "value": "User catalog"}]}),
                      cmd('library.trash', {"action": "list"}),
                      cmd('library.trash', {"action": "restore", "item": "user:dining-set"}),
                      cmd('library', {"chips": [{"kind": "category", "value": "User catalog"}]}),
                      cmd('library.delete', {"item": "user:dining-set"}),
                      cmd('library.trash', {"action": "empty"}),
                      cmd('library.trash', {"action": "list"}),
                      cmd('library', {"chips": [{"kind": "category", "value": "User catalog"}]})])
rec = reply_with(rr, 'item', -1, 'library.item')['item']
if rec.get('name') != 'Guest futon' or rec.get('notes') != 'for the den' or 'guest' not in (rec.get('keywords') or []):
    fail(f"the edited copy: {rec}")
u1, u2, u3 = (ids_of(lib_reply(rr, k)['items']) for k in range(3))
trash = [r['trash'] for r in rr if isinstance(r, dict) and isinstance(r.get('trash'), list)]
if 'user:dining-set' in u1 or trash[0] != ['user:dining-set'] or 'user:dining-set' not in u2:
    fail(f"delete/restore: listed {u1}, trash {trash[0]}, after restore {u2}")
if trash[1] != [] or 'user:dining-set' in u3 or 'user:sofa-futon' not in u3:
    fail(f"after emptying the Trash: trash {trash[1]}, user catalog {u3}")
rc, rr, so, se = run([cmd('library.delete', {"item": "sofa-futon"})], expect_ok=False)
if not errors(se):
    fail("deleting a stock item was not refused")
print("ok: Copy to User Catalog, edit, Trash (restore, empty); stock items stay")

# Set as Default.
rc, rr, so, se = run(ROOM + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "depth": 0.55, "doorStyle": "shaker",
                                             "pull": "own:pull-knob-32"}),
                             cmd('arch.default', {"id": "cb0"}),
                             cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 2.5}),
                             cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 2.5}),
                             cmd('door', {"wall": "w2", "offset": 2.0, "width": 0.8, "height": 2.0}),
                             cmd('arch.default', {"id": "o0"}),
                             cmd('door', {"wall": "w3", "offset": 2.0}),
                             cmd('library.place', {"item": "armchair", "at": [2, 2]}),
                             cmd('furnishing.edit', {"id": "f0", "size": [0.7, 0.7, 0.95]}),
                             cmd('arch.default', {"id": "f0"}),
                             cmd('library.place', {"item": "armchair", "at": [3, 3]}),
                             cmd('library.place', {"item": "sofa-futon", "at": [1, 3]}),
                             cmd('arch.inspect', {"walls": True, "levels": True, "cabinets": True, "openings": True, "furnishings": True})])
insp = inspected(rr)
cabs = {c['id']: c for c in objects_named(insp, 'cabinets', 'cabinets')}
if (cabs['cb1'].get('depth'), cabs['cb1'].get('doorStyle'), cabs['cb1'].get('pull')) != (0.55, 'shaker', 'own:pull-knob-32'):
    fail(f"the next base cabinet does not match the default: {cabs['cb1']}")
if cabs['cb2'].get('doorStyle') == 'shaker' and cabs['cb2'].get('depth') == 0.55:
    fail("a wall cabinet took the base cabinets' default")
ops = {o['id']: o for o in objects_named(insp, 'openings', 'openings')}
near(ops['o1']['width'], 0.8, 1e-9, "the next door's width"); near(ops['o1']['height'], 2.0, 1e-9, "the next door's height")
fs = {f['id']: f for f in furnishings_of(insp)}
if [round(v, 9) for v in fs['f1']['size']] != [0.7, 0.7, 0.95]:
    fail(f"the next armchair's size: {fs['f1']['size']}")
if [round(v, 9) for v in fs['f2']['size']] == [0.7, 0.7, 0.95]:
    fail("a different item took the armchair's default")
print("ok: Set as Default on a cabinet, a door and a furnishing")
print("PASS 08")
