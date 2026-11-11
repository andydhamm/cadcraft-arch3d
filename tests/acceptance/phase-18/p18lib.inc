# Phase-18 check helpers (interior finish, kitchen and bath). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())
# It loads the newest earlier helper library (phase 17, else 16), which loads phases 16..8 (run, run_ok, made, cmd,
# cli_args, parse_rows, inspected, objects_named, by_id, reply_with, near, num, fail, errors, probes, part, expect_hits,
# probed, parts_of, build3d_of, probe_cmd, room_row, level_row, house_rooms, HOUSE, GABLE, room_cmds, glb, node_box,
# node_meshes, node_names, material_of_node, texture_image_name, _acc, validate_glb, export, export_glb, step_file,
# of_type, ifc_names, face_set_points, dxf_text, entities, png, luma_sd, foreground, artifact, digest, surfaces_of,
# furnishings_of, room_at, rows_of, ROOT).
#
# What the checks drive (lengths in metres, angles in degrees; ids as the replies and arch.inspect give them, never
# hard-coded; every command below is one undo step and is refused, changing nothing, on a bad value):
#   room {"id", "type"}: a type change clears the room's explicit floor finish; a room's floor, wall faces and ceiling
#        take its type's template finishes unpainted (floor: garage/porch concrete; kitchen/bath/laundry/utility/entry
#        tile; bedroom/closet/family/openBelow carpet; living/dining/other/deck wood; walls drywall; ceilings plaster).
#        arch.inspect rooms rows gain "typeFinishes": {"floor", "wall", "ceiling"} (finish:<name> keys).
#   library {"chips": [{"kind": "folder", "value": "Moldings"}]}: molding:base, molding:shoe, molding:crown,
#        molding:chair-rail, molding:casing (category "Cabinet parts", kind "molding", size [w, 1.0, h]).
#   library.add {"objects": [handle of a closed LWPOLYLINE], "name", "kind": "molding"} -> {"item": "user:<slug>"}.
#   library {"for": "room/r0/molding" | "cabinet/cb0/crown" | "cabinet/cb0/lightRail" | "moldingLine/ml0"}.
#   room.moldings {"room": "r0" | ["r0", ...] | "all", "moldings": [{"kind": "base"|"crown"|"chair", "profile"?: item,
#        "size"?: [w, h], "offset"?: m}, ...]} -> {"rooms": [...], "moldings": n}; an empty list clears.
#   moldingline {"points": [[x, y], ...], "closed"?: false, "elevation": m (the profile's bottom above the level floor),
#        "profile"?: item (default molding:chair-rail), "size"?, "inset"?: 0 (left of travel), "flip"?: false (profile to
#        the right), "level"?} -> {"moldingLine": "ml0", "handle", "length"}; moldingline.edit {"id", ...};
#        moldingline.delete {"id"} -> {"deleted"}.
#   cabinet / cabinet.edit {..., "fronts": "open", "construction": "frameless"|"framed", "mount": "overlay"|"inset",
#        "corner": "auto"|"none"|"blind"|"diagonal"|"lazySusan", "shelves": n|"auto", "crown": item|null,
#        "crownSize": [w, h]|null, "lightRail": item|null, "lightRailSize": [w, h]|null, "backsplash": bool}.
#   cabinet.face {"id", "rows": [{"kind": "door"|"drawer"|"false"|"opening", "height"?, "across"?}]} |
#        {"id", "row": i, "split": "down"|"across", "count": 2..6} | {"id", "row": i, "kind"?|"height"?|"across"?} |
#        {"id", "equalize": true} | {"id", "doors": "auto"|"single"|"double"} | {"id", "reset": true} |
#        {"id", "row": i, "appliance": item|null} -> {"cabinet", "face"}.
#   cabinet.fill {"run": "cb0"} -> {"fillers": [labels], "gaps": [{"end": "start"|"end", "gap": m|null, "filled"}]}.
#   countertop.detach {"run": "cb0"} -> {"countertop": "ct0", "handle", "cabinets", "area"};
#   countertop {"polygon": ring, "elevation": top above the level floor, "thickness", "level"?} -> the same;
#   countertop.edit {"id", "edge": i, "move": d | "sagitta": s | "radius": R (+ "inward"?) | "profile":
#        "square"|"eased"|"bevel"|"bullnose" | "waterfall": bool} | {"id", "top"?, "thickness"?} | {"id", "vertex": i,
#        "to": [x, y]} -> {"countertop", "area"}; countertop.delete {"id"} -> {"deleted"}.
#   backsplash {"run": "cb0"|"ct0"} | {"wall": "w0", "level"?} + {"height"?: 0.45, "top"?: "ceiling", "material"?}
#        -> {"backsplash": "bs0", "handle", "segments": n}; backsplash.edit {"id", "height"?, "top"?: "ceiling"|"auto",
#        "material"?: key|null}; backsplash.delete {"id"}.
#   wall {..., "type": "glass"} (0.01 thick, 2.0 high unless given); wall.split {"id", "height": m|null, "lower"?: type,
#        "upper"?: type (default "Glass"), "lowerThickness"?, "upperThickness"?} -> {"wall", "split": {"height", "upper",
#        "upperThickness"}|null, "thickness"}.
#   door {..., "style": "shower"} (glass walls only; 0.7 x 1.9 frameless, swings both ways).
#   niche {"wall", "offset"|"point", "width"?: 0.6, "height"?: 0.9, "sill"?: 1.0, "depth"?: 0.1, "face"?: "left"|"right",
#        "material"?, "level"?} -> as door: {"openings": [{"id", ...}], "revision"}; refused unless the wall is at least
#        0.012 thicker than depth.
#   arch.inspect keys: {"moldings": true} -> "moldings": [{"room", "index", "kind", "profile", "size", "offset", "z",
#        "length", "runs": [{"points", "closed"}], "layer"}]; openings rows gain "trim": {"left", "right"} and
#        "trimWidth": {"left", "right"} (null on a face without trim), "depth", "face", "material";
#        {"moldingLines": true} -> "moldingLines": [{"id", "handle", "level", "points", "closed", "elevation", "inset",
#        "flip", "profile", "size", "length", "layer"}];
#        {"countertops": true} -> "countertops": [{"id": "cb<owner>"|"ct<n>", "auto", "handle", "level", "cabinets",
#        "polygon", "edges": [{"index", "from", "to", "length", "sagitta", "radius", "profile", "waterfall"}], "top"
#        (absolute), "thickness", "area" (outline, arcs included, holes not subtracted), "holes", "layer"}];
#        {"backsplashes": true} -> "backsplashes": [{"id", "handle", "level", "counter", "wall", "height", "toCeiling",
#        "thickness", "segments": [{"from", "to", "wall", "bottom", "top"}], "material", "layer"}];
#        cabinets rows gain "construction", "mount", "face" (null | {"height", "reveal", "rows": [{"kind", "height",
#        "fixed", "across", "appliance", "items": [{"kind", "rect"}]}]}), "corner" (null | {"style", "end", "leg",
#        "partner", "footprint"}), "filler", "shelves" (null | [{"bottom", "top"}]), "backsplash", "crown" and
#        "lightRail" (null | {"profile", "size", "length", "z", "path"}); walls rows gain "glass" and "split".
#   build3d parts: molding (room moldings, molding lines), counter (automatic counters, countertops, waterfalls),
#        backsplash, glass (glass walls, shower leaves), cabinet (crown, light rail, frame, shelves, appliances,
#        fillers), wall (a niche's back block and skin).
#   GLB / OBJ names: molding-r<n>-<kind>-<i>, molding-ml<n>, countertop-ct<n>, countertop-ct<n>-waterfall,
#        backsplash-bs<n>, wall-w<n>-glass, wall-w<n>-niche-o<m>, cabinet-cb<n>-crown|lightrail|frame|shelves|appliance.
#   IFC: IfcCovering .SKIRTINGBOARD. (base) / .MOLDING. (crown, chair, molding lines); IfcMaterial 'Glass' associated
#        with glass and glass-topped walls; countertops and backsplashes inside their owner cabinet's IfcFurniture;
#        a niche as an IfcOpeningElement .RECESS. with no IfcDoor.
#
# Fixtures: HOUSE is p16lib's phase-1 house (w0 south (0,0)->(10,0), w1 east, w2 north (10,8)->(0,8), w3 west; 0.2
# thick, 2.4 high; w4 along y = 3 and w5 along x = 6, 0.1 thick; rooms S, NW, NE after roomdetect). KITCHEN is
# p10lib's room_cmds(0, 0, 6, 4): one 0.2 m exterior ring whose inner faces are x 0..6, y 0..4 (w0 south along
# y = -0.1 from x -0.1, w1 east, w2 north from (6.1, 4.1) to (-0.1, 4.1), w3 west from (-0.1, 4.1) down), 2.4 high,
# room r0; a cabinet `wall: w0, offset: o` has its back on y = 0 and its centre at x = o - 0.1. MOLD_ROOM is the
# brief's molding room: walls on the centre lines (0,0) (4.1,0) (4.1,3.1) (0,3.1), 0.1 thick, 2.4 high (interior
# 0.05..4.05 x 0.05..3.05, 4.0 x 3.0, perimeter 14.0), one 0.9 m door on w0 at offset 2.05, room r0.
import math, os, json, tempfile
for _n in (17, 16):
    _p = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', f'phase-{_n}', f'p{_n}lib.inc')
    if os.path.exists(_p):
        exec(open(_p).read())
        break
else:
    raise SystemExit("FAIL: no earlier helper library (phase 17 or 16) is present")

INSPECT18 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "openings": True, "cabinets": True,
                                 "fixtures": True, "furnishings": True, "surfaces": True, "countertops": True,
                                 "backsplashes": True, "moldings": True, "moldingLines": True})
KITCHEN = room_cmds(0, 0, 6, 4)
MOLD_ROOM = [cmd('wall', {"points": [[0, 0], [4.1, 0], [4.1, 3.1], [0, 3.1], [0, 0]], "thickness": 0.1, "height": 2.4,
                          "exterior": True}),
             cmd('door', {"wall": "w0", "offset": 2.05, "width": 0.9}),
             'roomdetect {}']
MOLDINGS_ALL = cmd('room.moldings', {"room": "r0", "moldings": [{"kind": "base"}, {"kind": "crown"}, {"kind": "chair"}]})
DIGEST = cmd('build3d', {"digest": True})


def segment_area(chord, s):
    """The brief's circular segment: chord L, sagitta s (signed): R = (L²/4 + s²)/(2|s|)."""
    if s == 0 or chord <= 0:
        return 0.0
    a = abs(s)
    r = (chord * chord / 4.0 + a * a) / (2.0 * a)
    area = r * r * math.acos(max(-1.0, min(1.0, (r - a) / r))) - (r - a) * math.sqrt(max(0.0, 2.0 * r * a - a * a))
    return area if s > 0 else -area


def poly_area(pts):
    a = 0.0
    for k in range(len(pts)):
        (x1, y1), (x2, y2) = pts[k][:2], pts[(k + 1) % len(pts)][:2]
        a += x1 * y2 - x2 * y1
    return abs(a) / 2.0


def same_vertex_set(got, want, tol, what):
    """The two rings hold the same points (any order, any start), each within tol."""
    if not isinstance(got, list) or len(got) != len(want):
        fail(f"{what}: {len(want)} vertices expected, got {got!r}")
    left = [tuple(map(float, q[:2])) for q in got]
    for w in want:
        k = next((i for i, g in enumerate(left) if abs(g[0] - w[0]) <= tol and abs(g[1] - w[1]) <= tol), None)
        if k is None:
            fail(f"{what}: no vertex near {w} among {got}")
        left.pop(k)


def countertops_of(insp, auto=None):
    rows = objects_named(insp, 'countertops', 'countertops')
    if auto is None:
        return rows
    return [r for r in rows if r.get('auto') is auto]


def ct_row(insp, cid):
    return by_id(countertops_of(insp), cid, 'arch.inspect countertops')


def edge_index(row, pred, what):
    """The index of the one edge of a countertops row whose midpoint satisfies pred(mx, my)."""
    hits = []
    for e in row.get('edges') or []:
        a, b = e.get('from'), e.get('to')
        if isinstance(a, list) and isinstance(b, list):
            if pred((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0):
                hits.append(e)
    if len(hits) != 1:
        fail(f"{what}: one edge expected, {len(hits)} match: {json.dumps(row.get('edges'))[:500]}")
    return hits[0]['index'], hits[0]


def molding_rows(insp, room=None, kind=None):
    rows = objects_named(insp, 'moldings', 'moldings')
    return [m for m in rows if (room is None or m.get('room') == room) and (kind is None or m.get('kind') == kind)]


def molding_of(insp, room, kind):
    rows = molding_rows(insp, room, kind)
    if len(rows) != 1:
        fail(f"room {room} has one {kind} molding row, found {len(rows)}: {json.dumps(rows)[:400]}")
    return rows[0]


def molding_lines_of(insp):
    return objects_named(insp, 'moldingLines', 'molding lines')


def backsplashes_of(insp):
    return objects_named(insp, 'backsplashes', 'backsplashes')


def cabinets_of(insp):
    return objects_named(insp, 'cabinets', 'cabinets')


def openings_rows(insp):
    return objects_named(insp, 'openings', 'openings')


def walls_rows(insp):
    return objects_named(insp, 'walls', 'walls')


def face_of(cab_row, what):
    f = cab_row.get('face')
    if not isinstance(f, dict) or not isinstance(f.get('rows'), list):
        fail(f"{what}: cabinet {cab_row.get('id')} has no face layout: {json.dumps(cab_row.get('face'))[:300]}")
    return f


def hit(h, want_part, want_id=None, what='probe'):
    """h is a probe answer: a dict with part `want_part` (and id `want_id`); fails naming the answer."""
    if not isinstance(h, dict) or part(h) != want_part or (want_id is not None and h.get('id') != want_id):
        fail(f"{what}: expected {want_id or 'a'} {want_part}, the probe answered {h}")
    return h


def nohit(h, what='probe'):
    if h is not None:
        fail(f"{what}: expected nothing there, the probe hit {h}")


def refused(cmds, what):
    """Run `cmds`; the last one must be refused with an error line (no panic)."""
    rc, rr, so, se = run(cmds, expect_ok=False)
    if not errors(se):
        fail(f"{what}: expected a refusal, got none: {json.dumps(rr)[-400:]}")
    return rr, se


def covered(j, binc, name, x, y):
    """Whether a GLB node's triangles, seen from above, cover (x, y) (phase-14 check 05's test)."""
    for m in node_meshes(j, name):
        for prim in m['primitives']:
            pos = [(p[0], p[1]) for p in _acc(j, binc, prim['attributes']['POSITION'])]
            idx = [t[0] for t in _acc(j, binc, prim['indices'])] if 'indices' in prim else list(range(len(pos)))
            for t in range(0, len(idx) - 2, 3):
                (ax, ay), (bx, by), (cx, cy) = pos[idx[t]], pos[idx[t + 1]], pos[idx[t + 2]]
                area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
                if abs(area) < 1e-12:
                    continue
                d1 = (x - bx) * (ay - by) - (ax - bx) * (y - by)
                d2 = (x - cx) * (by - cy) - (bx - cx) * (y - cy)
                d3 = (x - ax) * (cy - ay) - (cx - ax) * (y - ay)
                neg = d1 < 0 or d2 < 0 or d3 < 0
                pos_ = d1 > 0 or d2 > 0 or d3 > 0
                if not (neg and pos_):
                    return True
    return False


def steps_of(path):
    """A commands file (a JSON array of {"cmd", "params"}) read and checked."""
    if not os.path.exists(path):
        fail(f"{os.path.relpath(path, ROOT)} is missing")
    try:
        steps = json.load(open(path, encoding='utf-8'))
    except ValueError as e:
        fail(f"{os.path.relpath(path, ROOT)} is not JSON: {e}")
    if not isinstance(steps, list) or not steps or not all(
            isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict) for st in steps):
        fail(f"{os.path.relpath(path, ROOT)} is a non-empty JSON array of {{\"cmd\", \"params\"}} steps")
    return steps


def replay_file18(path, extra, what, timeout=1800):
    """`cadcraft-cli run --metric --commands path`, then the `extra` --cmd steps; a refusal or a panic fails."""
    args = cli_args('run') + ['--metric', '--commands', path]
    for c in extra:
        args += ['--cmd', c]
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    import subprocess
    p = subprocess.run(args, capture_output=True, text=True, timeout=timeout, env=env)
    if 'panicked' in p.stderr or 'panicked' in p.stdout:
        fail(f"panic replaying {os.path.relpath(path, ROOT)} ({what}): {(p.stderr + p.stdout)[-800:]}")
    if p.returncode != 0 or errors(p.stderr):
        fail(f"{os.path.relpath(path, ROOT)} replays without an error ({what}; exit {p.returncode}): "
             f"{errors(p.stderr)[:3] or p.stderr[-400:]}")
    return parse_rows(p.stdout)
