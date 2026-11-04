# Phase-14 check helpers (the library). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())
# It loads the phase-13 helpers (run, cmd, cli, glb, node_box, png, num, near, fail, probes, part, inspected, validate_glb…).
#
# What the checks drive (lengths in metres, angles in degrees; ids as the replies give them: furnishings f<n>):
#   library {"text"?, "chips"?: [{"kind": "category"|"folder"|"symbolType"|"placement"|"source"|"style"|"tag", "value"}],
#            "for"?: target, "back"?: true, "forward"?: true}
#        -> {"categories": [{"name", "items", "icon"}], "items": [record], "total", "query": {"text", "chips"},
#            "back": bool, "forward": bool, "folders"?: [{"name", "items"}]}
#   record = {"id", "name", "category", "folder", "kind", "symbolType", "placement", "size", "keywords", "tags", "source",
#             "licence", "author", "catalog", "readOnly", "elevation", "cutout", "notes"}
#   library.item {"id"} -> {"item": record};  library.icon {"item"? | "category"?, "cache"?: bool} -> {"png" (base64), "hash", "cached"}
#   library.tag {"item", "tag", "on"?}; library.filter {"save" | "apply" | "delete": name} or {} -> {"filters"}
#   library.install {"from": dir} -> {"catalog", "items", "skipped": [{"file", "reason"}]}
#   library.local {"path": dir, "licence", "category"?} -> {"catalog": "local", "items", "skipped": [...]}
#   library.place {"item", "at": [x, y], "rotation"?, "level"?, "elevation"?, "size"?: [w, d, h]}
#        -> {"furnishing": "f0", "handle", "host", "at", "rotation", "elevation", "size"} | {"swapped": {"cabinet", "part"}}
#           | {"via": "<command>", ...that command's reply} | {"furnishings": [...], "cabinets": [...], "fixtures": [...]} (a user item)
#   furnishing.edit {"id", "at"?, "rotation"?, "elevation"?, "size"?, "mirrored"?}; furnishing.delete {"id"}
#   paint.material {"material", "target", "scope", "mode"?} -> {"painted": [surface keys]}
#   paint.object {"item", "target", "scope"} -> {"replaced": [keys]};  paint.pick {"target"} -> {"material", "tint"}
#   library.add {"objects": [ids], "name", "insertionPoint"?} -> {"item": "user:<slug>", "objects": n}
#   library.copy {"item"} -> {"item"}; library.edit {"item", "name"?, "keywords"?, "notes"?}; library.delete {"item"}
#   library.trash {"action": "list" | "restore" | "empty", "item"?} -> {"trash": [ids]} | {"restored"} | {"emptied"}
#   arch.default {"id"} -> {"kind", "default"};  backdrop {"kind": "colour"|"gradient"|"image"|"none", ...}
#   arch.inspect {"furnishings": true} -> furnishings[{id, item, name, category, symbolType, placement, level, at, rotation,
#            elevation, size, mirrored, host, layer, cutout}]; {"surfaces": true} -> surfaces[{surface, room, level, material,
#            tint}]; {"cabinets": true} rows gain doorStyle, drawerStyle, pull, falseFront, cutout.
#   Surface keys: wall/w3/left, wall/w3/right, room/r0/floor, room/r0/ceiling, roof/r0, cabinet/cb0/{carcass,front,counter,pull},
#   fixture/fx0, furnishing/f2/<part>. Material keys in replies: finish:<name>, item:<id>, colour:#rrggbb, default:<what>,
#   sym:<symbol key>:<part>. The GLB: nodes furnishing-f<n>, wall-w<n>-left / -right (painted faces), cabinet-cb<n>-pull.
import os, tempfile, base64, hashlib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-13', 'p13lib.inc')).read())

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
SAMPLE = json.load(open(os.path.join(ROOT, 'assets', 'library', 'sample', 'catalog.json'), encoding='utf-8'))['items']
SAMPLE_IDS = {i['id'] for i in SAMPLE}
CATEGORIES = ['Furniture', 'Appliances and plumbing fixtures', 'Cabinet parts', 'Lighting and electrical', 'Materials',
              'Doors and windows', 'Plants', 'Backdrops', 'User catalog']

# The user catalog, installed catalogs, tags, saved filters and the icon cache live in this folder for the whole check
# (never the real user's): every `run` inherits it.
os.environ['CADCRAFT_LIBRARY_USER_DIR'] = tempfile.mkdtemp(prefix='cc-lib-')

# The phase-1 house with two rooms, its walls broken where the partition meets them (each wall side faces one room):
# ring w0..w5 counter-clockwise, 0.2 m thick, 2.4 m high; partition w6 at x = 4 from (4,0) to (4,5), 0.1 m thick.
# Room A is x 0.1..3.95, room B x 4.05..7.9, y 0.1..4.9 (inner faces). The ring's left sides face the rooms.
HOUSE1 = [cmd('wall', {"points": [[0, 0], [4, 0], [8, 0], [8, 5], [4, 5], [0, 5], [0, 0]], "thickness": 0.2, "height": 2.4,
                       "exterior": True}),
          cmd('wall', {"points": [[4, 0], [4, 5]], "thickness": 0.1, "height": 2.4, "exterior": False}),
          'roomdetect {}']
INSPECT14 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "cabinets": True, "fixtures": True,
                                 "furnishings": True, "surfaces": True})


def lib_reply(rows, nth=-1):
    """The nth `library` reply (it always carries categories and items)."""
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('items'), list) and isinstance(r.get('categories'), list)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no library reply; replies: {json.dumps(rows)[-600:]}")
    return hits[nth]


def ids_of(items):
    return sorted(i.get('id') for i in items if isinstance(i, dict))


def furnishings_of(insp):
    return objects_named(insp, 'furnishings', 'furnishings')


def surfaces_of(insp):
    v = objects_named(insp, 'surfaces', 'surfaces')
    return {s.get('surface'): s for s in v}


def room_at(insp, x, y):
    """The id of the room whose polygon holds (x, y)."""
    for r in objects_named(insp, 'rooms', 'rooms'):
        poly = r.get('polygon') or []
        inside = False
        for k in range(len(poly)):
            (x1, y1), (x2, y2) = poly[k][:2], poly[(k + 1) % len(poly)][:2]
            if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
                inside = not inside
        if inside:
            return r.get('id')
    fail(f"no room holds ({x}, {y}): {json.dumps(insp.get('rooms'))[:400]}")


def drawing_box(j, name):
    """A GLB node's box in drawing axes. The writer keeps the positions in drawing axes (x, y, z up) and turns the
    scene Y-up with the root node's rotation alone (io glb.rs ROOT_ROTATION), so the accessors already are drawing axes."""
    return node_box(j, name)


def node_positions(j, binc, name):
    """Every vertex of a node's primitives in drawing axes (the accessors' own axes, see drawing_box)."""
    out = []
    for m in node_meshes(j, name):
        for prim in m['primitives']:
            out += [(p[0], p[1], p[2]) for p in _acc(j, binc, prim['attributes']['POSITION'])]
    if not out:
        fail(f"GLB node '{name}' has no vertices; nodes: {[n.get('name') for n in j.get('nodes', [])][:50]}")
    return out


def geometry_key(points, dx=0.0, dy=0.0, q=1e-4):
    """A node's vertices moved by (dx, dy), rounded to q and sorted: equal for equal meshes in any vertex order."""
    return sorted((round((x + dx) / q), round((y + dy) / q), round(z / q)) for x, y, z in points)


def node_names(j):
    return [n.get('name') for n in j.get('nodes', [])]


def export_glb(cmds, tmp, name='x.glb'):
    out = os.path.join(tmp, name)
    rc, rr, so, se = run(cmds + [cmd('export', {"path": out})])
    if not os.path.exists(out):
        fail(f"export wrote no {name}: {se[-500:]}")
    return rr, glb(out)


def icon_png(rows, nth=-1):
    r = reply_with(rows, 'png', nth, 'library.icon')
    try:
        data = base64.b64decode(r['png'])
    except Exception as e:
        fail(f"library.icon png is not base64: {e}")
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        fail("library.icon did not return a PNG")
    return data, r


def errors(se):
    return [l for l in se.splitlines() if l.startswith('error: ')]
