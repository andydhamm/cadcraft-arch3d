#!/usr/bin/env python3
# Hostile input to the library, placing, painting, the user catalog, defaults and backdrops: every one is refused with an
# error, never a panic, never clamped, and changes nothing: afterwards there is no furnishing, no paint, no backdrop, no
# user item, and every cabinet is as made.
# needs: p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-cmd-place, p14-cmd-paint, p14-cmd-user-catalog, p14-cmd-defaults, p14-cmd-backdrop, p14-inspect, p14-cmd-casework-parts, p14-cmd-icons, p14-cmd-furnishing-transform
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

nowhere = tempfile.mkdtemp()
H = [('library', {"text": 7}),
     ('library', {"chips": [{"kind": "colour", "value": "red"}]}),
     ('library', {"chips": "chairs"}),
     ('library', {"chips": [{"kind": "tag"}]}),
     ('library', {"for": "wall/w99/left"}),
     ('library.item', {"id": "no-such-item"}),
     ('library.icon', {"item": "no-such-item"}),
     ('library.icon', {"category": "Spaceships"}),
     ('library.tag', {"item": "no-such-item", "tag": "Favorites"}),
     ('library.tag', {"item": "sofa-futon", "tag": ""}),
     ('library.filter', {"apply": "never saved"}),
     ('library.install', {"from": os.path.join(nowhere, 'missing')}),
     ('library.install', {"from": nowhere}),
     ('library.local', {"path": os.path.join(nowhere, 'missing'), "licence": "CC0-1.0"}),
     ('library.local', {"path": nowhere, "licence": ""}),
     ('library.place', {"item": "no-such-item", "at": [1, 1]}),
     ('library.place', {"item": "armchair"}),
     ('library.place', {"item": "armchair", "at": [1e308, 1]}),
     ('library.place', {"item": "armchair", "at": "here"}),
     ('library.place', {"item": "armchair", "at": [1, 1], "level": "Level 9"}),
     ('library.place', {"item": "armchair", "at": [1, 1], "size": [0, 1, 1]}),
     ('library.place', {"item": "armchair", "at": [1, 1], "size": [1, 1]}),
     ('library.place', {"item": "armchair", "at": [1, 1], "size": [1e9, 1, 1]}),
     ('library.place', {"item": "armchair", "at": [1, 1], "rotation": "north"}),
     ('library.place', {"item": "armchair", "at": [1, 1], "elevation": -2}),
     ('library.place', {"item": "towel-rack", "at": [2, 2]}),                 # no wall face within 0.3 m
     ('library.place', {"item": "light-pendant", "at": [9, 9]}),              # a ceiling item outside every room
     ('library.place', {"item": "own:pull-knob-32", "at": [2, 2]}),           # a pull over no cabinet
     ('library.place', {"item": "frontstyle:door-shaker", "at": [2, 2]}),     # a style over no cabinet
     ('library.place', {"item": "finish:brick", "at": [2, 2]}),               # a material is painted, not placed
     ('furnishing.edit', {"id": "f0", "size": [1, 1, 1]}),
     ('furnishing.delete', {"id": "cb0"}),
     ('furnishing.centre', {"id": "f0", "on": "room"}),
     ('paint.material', {"material": "unobtainium", "target": "wall/w0/left", "scope": "room"}),
     ('paint.material', {"material": "#12345", "target": "wall/w0/left", "scope": "room"}),
     ('paint.material', {"material": "material-oak", "target": "wall/w99/left", "scope": "room"}),
     ('paint.material', {"material": "material-oak", "target": "wall/w0/up", "scope": "room"}),
     ('paint.material', {"material": "material-oak", "target": "wall/w0/left", "scope": "galaxy"}),
     ('paint.material', {"material": "material-oak", "target": "wall/w0/left", "scope": "room", "mode": "smear"}),
     ('paint.material', {"material": "sofa-futon", "target": "wall/w0/left", "scope": "room"}),
     ('paint.material', {"material": "shingles", "target": "room/r0/floor", "scope": "component"}),
     ('paint.object', {"item": "sofa-futon", "target": "cabinet/cb0/pull", "scope": "room"}),
     ('paint.object', {"item": "own:pull-knob-32", "target": "wall/w0/left", "scope": "room"}),
     ('paint.object', {"item": "own:pull-knob-32", "target": "cabinet/cb9/pull", "scope": "room"}),
     ('paint.pick', {"target": "roof/r7"}),
     ('library.add', {"objects": [], "name": "Nothing"}),
     ('library.add', {"objects": ["f99"], "name": "Ghost"}),
     ('library.add', {"objects": ["cb0"]}),
     ('library.copy', {"item": "no-such-item"}),
     ('library.edit', {"item": "sofa-futon", "name": "Mine"}),
     ('library.delete', {"item": "sofa-futon"}),
     ('library.trash', {"action": "restore", "item": "user:never"}),
     ('library.trash', {"action": "burn"}),
     ('arch.default', {"id": "w0"}),
     ('arch.default', {"id": "f42"}),
     ('backdrop', {"kind": "plaid"}),
     ('backdrop', {"kind": "colour", "colour": "blue"}),
     ('backdrop', {"kind": "gradient", "top": "#ffffff"}),
     ('backdrop', {"kind": "image", "item": "sofa-futon"}),
     ('backdrop', {"kind": "image", "item": "own:backdrop-sky", "projection": "fisheye"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 2.0, "doorStyle": "gothic"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 2.0, "pull": "sofa-futon"}),
     ('cabinet.edit', {"id": "cb0", "pull": "no-such-pull"})]
SETUP = ROOM + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0})]
rc, rr, so, se = run(SETUP + [INSPECT14])
BASE = surfaces_of(inspected(rr))
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT14, cmd('backdrop', {}),
                                                          cmd('library', {"chips": [{"kind": "category", "value": "User catalog"}]})],
                     expect_ok=False)
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:70]))
ins = inspected(rr)
if furnishings_of(ins):
    fail(f"refused commands placed furnishings: {ins.get('furnishings')}")
painted = [k for k, v in surfaces_of(ins).items() if BASE.get(k) != v] + [k for k in BASE if k not in surfaces_of(ins)]
if painted:
    fail(f"refused paints changed surfaces: {painted[:4]}")
cab = objects_named(ins, 'cabinets', 'cabinets')
if len(cab) != 1 or cab[0].get('pull') is not None or cab[0].get('doorStyle') != 'slab':
    fail(f"the cabinet changed: {cab}")
if reply_with(rr, 'backdrop', -1, 'backdrop {}').get('backdrop') is not None:
    fail("a refused backdrop was set")
if lib_reply(rr)['items']:
    fail(f"refused user-catalog commands made items: {ids_of(lib_reply(rr)['items'])}")
print(f"ok: {len(H)} hostile commands refused; nothing placed, painted, set or added")
print("PASS 11")
