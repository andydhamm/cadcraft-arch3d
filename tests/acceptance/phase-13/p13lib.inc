# Phase-13 check helpers (polish: materials, cabinets and fixtures, the incremental rebuild, docs). A fixture, not a check
# (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())
# It loads the phase-12 helpers (run, cmd, cli, glb, node_box, png, num, near, fail, artifact, step_file, export, …).
#
# What the checks drive (lengths in metres, angles in degrees):
#   material {"walls": ["w0", …] | "exterior" | "roofs": ["r0"] | "all" | "rooms": ["r0"] | "all",
#             "finish": "siding" | "shingles" | "brick" | "wood" | "none"} -> {"target", "finish", "ids"}
#   arch.inspect {"walls": true, "roofs": true, "rooms": true} -> walls[].finish, roofs[].covering, rooms[].floorFinish
#   cabinet {"kind": "base" | "wall" | "tall", "wall"?, "offset"?, "at"?, "rotation"?, "level"?, "width"?, "depth"?,
#            "height"?, "elevation"?, "fronts"?, "doors"?, "drawers"?, "counter"?, "widths"?} -> {"cabinets", "handles"}
#   cabinet.edit {"id", …} -> {"cabinet"};  cabinet.delete {"id"} -> {"deleted"}
#   fixture {"kind": "sink" | "lavatory" | "toilet" | "bathtub" | "shower" | "range" | "refrigerator" | "dishwasher",
#            "wall"?, "offset"?, "at"?, "rotation"?, "level"?, "width"?, "depth"?, "height"?, "elevation"?}
#            -> {"fixtures", "handles"};  fixture.edit / fixture.delete / fixture.types
#   arch.inspect {"cabinets": true, "fixtures": true} -> cabinets[{id, kind, level, at, rotation, width, depth, height,
#            elevation, fronts, doors, drawers, counter, counterTop, layer}], fixtures[{id, kind, …, layer}]
#   arch.sample {"house": true} -> {walls, openings, levels, rooms, roofs, revision, bench: {wall, end, from, to}}
#   build3d {"incremental"?: bool, "digest"?: bool, "probe"?} -> {…, rebuildMs, incremental, rebuilt, objects, digest?}
#   The GLB: images (PNG in bufferViews), samplers, textures; finished materials "wall-<f>", "roof-<f>", "floor-<f>" with
#   pbrMetallicRoughness.baseColorTexture; nodes "wall-w<n>-exterior", "cabinet-cb<n>", "cabinet-cb<n>-front",
#   "cabinet-cb<n>-counter", "fixture-fx<n>".
#   The IFC: IfcFurniture (cabinets), IfcSanitaryTerminal (sink, lavatory, toilet, bathtub, shower), IfcElectricAppliance
#   (range, refrigerator, dishwasher).
import os, statistics
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-12', 'p12lib.inc')).read())

# room_cmds(): one 0.2 m exterior wall ring drawn counter-clockwise, (-0.1,-0.1) -> (4.1,-0.1) -> (4.1,4.1) -> (-0.1,4.1):
# w0 runs +x along y = -0.1 (inner face y = 0), w1 +y along x = 4.1, w2 -x, w3 -y; their exterior faces point outwards.
ROOM = room_cmds()

INSPECT_ALL = cmd('arch.inspect', {"walls": True, "roofs": True, "rooms": True, "cabinets": True, "fixtures": True,
                                   "openings": True, "levels": True})


def inspected(rows, nth=-1):
    """The nth `arch.inspect` reply (it always carries walls and levels)."""
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('walls'), list) and 'levels' in r]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply; replies: {json.dumps(rows)[-600:]}")
    return hits[nth]


def objects_named(insp, key, what):
    v = insp.get(key)
    if not isinstance(v, list) or not all(isinstance(x, dict) for x in v):
        fail(f"arch.inspect has no '{key}' list of objects ({what}): {json.dumps(insp)[:400]}")
    return v


def by_id(objs, i, what):
    for o in objs:
        if o.get('id') == i:
            return o
    fail(f"{what}: no '{i}' among {[o.get('id') for o in objs]}")


def replies_with_list(rows, key):
    """Command replies like {"cabinets": ["cb0"], "handles": [...]} (labels, not inspect objects)."""
    return [r for r in rows if isinstance(r, dict) and isinstance(r.get(key), list) and 'handles' in r]


def refused(rows_before, se, what):
    if not any(l.startswith('error: ') for l in se.splitlines()):
        fail(f"{what} was not refused (no 'error:' line); stderr: {se[-400:]}")


# ---- images ----------------------------------------------------------------------------------------------------------

def luma_grid(path, frac=0.3):
    """Luminance (0-255) of the central `frac` x `frac` crop of a PNG, as a list of rows."""
    w, h, nch, rows = png(path)
    x0, x1 = int(w * (0.5 - frac / 2)), int(w * (0.5 + frac / 2))
    y0, y1 = int(h * (0.5 - frac / 2)), int(h * (0.5 + frac / 2))
    out = []
    for y in range(y0, y1):
        r = rows[y]
        line = []
        for x in range(x0, x1):
            if nch >= 3:
                p = r[x * nch:x * nch + 3]
                line.append(0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
            else:
                line.append(float(r[x * nch]))
        out.append(line)
    return out


def grid_stats(g):
    """(standard deviation, mean |difference| between vertical neighbours, mean |difference| between horizontal ones)."""
    flat = [v for row in g for v in row]
    sd = statistics.pstdev(flat) if len(flat) > 1 else 0.0
    dv = [abs(g[y + 1][x] - g[y][x]) for y in range(len(g) - 1) for x in range(len(g[0]))]
    dh = [abs(row[x + 1] - row[x]) for row in g for x in range(len(row) - 1)]
    return sd, (sum(dv) / len(dv) if dv else 0.0), (sum(dh) / len(dh) if dh else 0.0)


def png_bytes_grid(data, tmp, name):
    """Decode PNG bytes (a texture from the GLB): (width, height, luminance rows of the whole image)."""
    p = os.path.join(tmp, name)
    with open(p, 'wb') as f:
        f.write(data)
    w, h, nch, rows = png(p)
    g = [[0.2126 * r[x * nch] + 0.7152 * r[x * nch + 1] + 0.0722 * r[x * nch + 2] for x in range(w)] for r in rows]
    return w, h, g


def glb_images(j, binc):
    """{image name: PNG bytes} from the GLB's images and bufferViews."""
    out = {}
    for i, im in enumerate(j.get('images', [])):
        if 'bufferView' not in im:
            fail(f"image {i} is not embedded in a bufferView: {im}")
        v = j['bufferViews'][im['bufferView']]
        data = binc[v.get('byteOffset', 0):v.get('byteOffset', 0) + v['byteLength']]
        if data[:8] != b'\x89PNG\r\n\x1a\n':
            fail(f"image {i} ({im.get('name')}) is not a PNG")
        if im.get('mimeType') != 'image/png':
            fail(f"image {i} has mimeType {im.get('mimeType')!r}, not image/png")
        out[im.get('name') or str(i)] = data
    return out


def material_of_node(j, name):
    ms = node_meshes(j, name)
    if not ms:
        fail(f"GLB has no node '{name}'; nodes: {sorted(n.get('name') for n in j.get('nodes', []) if 'mesh' in n)[:60]}")
    prim = ms[0]['primitives'][0]
    if 'material' not in prim:
        fail(f"node '{name}' has no material")
    return j['materials'][prim['material']], prim


def texture_image_name(j, mat):
    t = (mat.get('pbrMetallicRoughness') or {}).get('baseColorTexture')
    if not isinstance(t, dict) or not isinstance(t.get('index'), int):
        return None
    tex = j['textures'][t['index']]
    if 'sampler' not in tex or 'source' not in tex:
        fail(f"texture {t['index']} lacks a sampler or a source: {tex}")
    return j['images'][tex['source']].get('name')


def render_face(cmds, eye, target, out, size=(480, 360), fov=40):
    """A shaded raster still (software adapter) of the scene, camera at `eye` looking at `target`."""
    rc, rr, so, se = run(cmds + [cmd('render', {"out": out, "size": list(size), "style": "shaded",
                                                "camera": {"kind": "full", "eye": eye, "target": target, "fov": fov}})])
    if not os.path.exists(out):
        fail(f"render wrote no {os.path.basename(out)}: {se[-500:]}")
    return rr


# ---- joins (from wall geometry, independent of the solver) -------------------------------------------------------------

def classify_joins(walls, tol=1e-4):
    """Count L, T and X joins on each level from the walls' centre lines: endpoints within `tol` form a cluster; a cluster
    of 2 non-collinear ends is an L, 3 ends a T, 4 an X; an end on another wall's interior is a T; two walls crossing in
    their interiors is an X."""
    def key(p):
        return (round(p[0] / tol), round(p[1] / tol))
    counts = {'L': 0, 'T': 0, 'X': 0}
    by_level = {}
    for w in walls:
        by_level.setdefault(w.get('level'), []).append(w)
    for lvl, ws in by_level.items():
        clusters = {}
        for w in ws:
            for p, q in ((w['start'], w['end']), (w['end'], w['start'])):
                clusters.setdefault(key(p), []).append((p, q))
        for k, ends in clusters.items():
            if len(ends) == 2:
                (p, a), (_, b) = ends
                ux, uy = a[0] - p[0], a[1] - p[1]; vx, vy = b[0] - p[0], b[1] - p[1]
                lu, lv = math.hypot(ux, uy), math.hypot(vx, vy)
                if lu > 0 and lv > 0 and abs((ux * vy - uy * vx) / (lu * lv)) > 0.5:
                    counts['L'] += 1
            elif len(ends) == 3:
                counts['T'] += 1
            elif len(ends) >= 4:
                counts['X'] += 1
        for w in ws:                       # an end on another wall's side
            for p in (w['start'], w['end']):
                if len(clusters.get(key(p), [])) > 1:
                    continue
                for o in ws:
                    if o is w:
                        continue
                    ax, ay = o['start']; bx, by = o['end']; dx, dy = bx - ax, by - ay; ll = dx * dx + dy * dy
                    if ll == 0:
                        continue
                    t = ((p[0] - ax) * dx + (p[1] - ay) * dy) / ll
                    if 0.01 < t < 0.99 and math.hypot(ax + t * dx - p[0], ay + t * dy - p[1]) <= o.get('thickness', 0.2) / 2 + 1e-6:
                        counts['T'] += 1
                        break
        for i, a in enumerate(ws):         # interiors crossing
            for b in ws[i + 1:]:
                (x1, y1), (x2, y2) = a['start'], a['end']; (x3, y3), (x4, y4) = b['start'], b['end']
                d = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3)
                if abs(d) < 1e-12:
                    continue
                t = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / d
                u = ((x3 - x1) * (y2 - y1) - (y3 - y1) * (x2 - x1)) / d
                if 0.01 < t < 0.99 and 0.01 < u < 0.99:
                    counts['X'] += 1
    return counts


# ---- the kitchen -------------------------------------------------------------------------------------------------------

# In ROOM: a run of three base cabinets on w0 starting 1.0 m along it (centres x = 0.9, 1.65, 2.4; backs on y = 0), a wall
# cabinet above the first, a tall cabinet on w1, a sink on the middle base cabinet, a range, a refrigerator and a
# dishwasher placed by point.
KITCHEN = [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "widths": [0.6, 0.9, 0.6]}),
           cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0}),
           cmd('cabinet', {"kind": "tall", "wall": "w1", "offset": 1.0}),
           cmd('fixture', {"kind": "sink", "at": [1.65, 0.0]}),
           cmd('fixture', {"kind": "range", "wall": "w2", "offset": 1.5}),
           cmd('fixture', {"kind": "refrigerator", "wall": "w2", "offset": 3.0}),
           cmd('fixture', {"kind": "dishwasher", "at": [3.3, 0.0]})]


def validate_glb(path):
    """The Khronos glTF validator (npm, under CHECK_TOOLS) on `path`: fails on any error; prints SKIP when not installed."""
    import shutil
    node = shutil.which('node')
    pkg = os.path.join(CHECK_TOOLS, 'node_modules', 'gltf-validator')
    if not node or not os.path.isdir(pkg):
        print(f"SKIP gltf-validator: not installed (node {node}, {pkg})")
        return
    js = ("const v=require(process.argv[1]);const fs=require('fs');"
          "v.validateBytes(new Uint8Array(fs.readFileSync(process.argv[2]))).then(r=>{console.log(JSON.stringify(r));})"
          ".catch(e=>{console.error(String(e));process.exit(2);});")
    p = subprocess.run([node, '-e', js, pkg, path], capture_output=True, text=True, timeout=300)
    if p.returncode != 0:
        fail(f"the glTF validator did not run: {p.stderr[-400:]}")
    iss = json.loads(p.stdout).get('issues', {})
    errs = [x for x in iss.get('messages', []) if x.get('severity') == 0]
    for x in errs[:8]:
        print(f"  {x.get('code')}: {x.get('message')} at {x.get('pointer')}")
    if iss.get('numErrors', 1) != 0:
        fail(f"the glTF validator reports {iss.get('numErrors')} error(s) in {os.path.basename(path)}: "
             f"{[x.get('code') for x in errs][:10]}")
    print(f"ok: gltf-validator: no errors in {os.path.basename(path)} ({iss.get('numWarnings')} warnings)")
