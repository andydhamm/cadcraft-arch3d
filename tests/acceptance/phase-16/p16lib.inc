# Phase-16 check helpers (ceilings and foundations). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())
# It loads the phase-15 helpers, which load phases 14..8 (run, cmd, cli, inspected, objects_named, by_id, reply_with, near,
# fail, probes, part, glb, node_box, node_names, png, luma_sd, export, step_file, of_type, validate_glb, room_at, errors,
# furnishings_of, rows_of).
#
# What the checks drive (lengths in metres, elevations absolute unless they say "above the level's floor"; ids as the
# replies and arch.inspect give them, never hard-coded):
#   room {..., "flatCeiling"?: bool, "ceilingStack"?: name | [{"name"?, "material": string | null, "thickness"}, ...]}
#        A room's ceiling height (its own `ceiling`, else its level's) is its ROUGH ceiling (the underside of the structure
#        above). flatCeiling (default true) off: under a roof the ceiling follows the underside of the roof planes over the
#        room (a vault). ceilingStack: finish layers top to bottom (material null = an air gap; at most 8 layers, each
#        0.001..1 m, the top and bottom layers have a material, 1 m in all); the templates are "none", "drywall"
#        (0.0127) and "suspended" (drywall 0.0127, air 0.2, ceiling tile 0.016), case-insensitive. The FINISHED ceiling is
#        the rough one minus the stack. Edit path: only keys present change; an absent `ceiling`/`floor` keeps the stored
#        height, null clears it. A create, or an edit that raises a flat ceiling (or turns flatCeiling on), is refused when
#        the flat ceiling would rise above the roof underside (the error names the room); keeping or lowering one never is.
#        The level's ceiling-hung items are re-seated. Replies gain "flatCeiling" and "ceilingStack": [{name, material,
#        thickness}].
#   level {"index" | "name", "ceiling"} and level.edit {"level", "ceiling"}: refused (nothing written) when raising the level's
#        ceiling would put a flat room ceiling that takes the level's height above the roof underside. Roof edits
#        (roofauto, roofplane) are never refused; a flat ceiling a roof edit leaves above the underside follows the
#        underside there and its room row carries a ceilingWarning.
#   build3d {"ceiling"?: [[x, y], ...] (at most 1024, each exactly two finite numbers), "level"?: index | name (default 0),
#            "probe"?, "incremental"?, "digest"?}
#        -> {..., "ceiling": [z | null, ...] (only when `ceiling` was given: the finished ceiling over each point on that
#            level, null outside every room), "parts": {part: count}}. parts counts the scene's meshes by probe part, except
#            "furnishing" and "electrical", which count the furnishings and plates built; parts with none are left out.
#            Part names include ceiling, floor, wall, roof, soffit (interior soffits; the roof's own soffit stays "roof"),
#            foundation (stem walls), footing (footings and slab edges), slab (slab floors). A ceiling stack's upper layers
#            are part "ceiling" too, and `ceilings` stays one per room.
#   tray {"room": "r1", "inset", "depth", "sides"?: "vertical" | degrees} | {"tray": "tr0", "inset", "depth", "sides"?}
#            (nested inside tr0) | {"polygon": closed ring [[x, y], ...] (>= 4 points, last = first), "depth", "inset"?: 0,
#            "sides"?, "level"?: name | index}
#        -> {"tray": "tr0", "handle", "room", "parent": null | "tr0", "outline": [[x, y], ...], "ceiling": z}. Inside the
#           outline (inset in from the room's walls, the parent's outline, or the drawn polygon) the ceiling is `depth`
#           higher; pitched sides ramp up at that angle. Refused, building nothing, above the roof underside, under a level
#           (a room on a higher level overlapping it), in a vaulted room, overlapping a sibling, or with bad numbers.
#   tray.edit {"id", "inset"?, "depth"?, "sides"?, "polygon"?} (re-validated with its nested trays); tray.delete {"id"}
#        -> {"deleted": ["tr0", ...]} (nested trays go too). Trays draw nothing in plan in phase 16 (layer A-CLNG).
#   soffit {"rect": [[x0, y0], [x1, y1]] | "polygon": closed ring | "over": ["cb3", ...] (wall cabinets), "bottom"?: m (above
#            the level's floor) | "depth"?: m (below the finished ceiling), "material"?: key, "level"?: name | index}
#        -> {"soffit": "sf0", "handle", "polygon": ring, "bottom": z, "top": z, "room"}. A rect side within 0.15 m of a wall
#           face snaps onto it; `over` fills from the cabinet tops to the ceiling and takes neither bottom nor depth.
#           Ceiling items over a soffit hang from its underside, and moving a soffit re-seats the items under it. Drawn in
#           plan as a dashed outline on A-CLNG.
#   soffit.edit {"id", "rect"? | "polygon"?, "bottom"? | "depth"?, "material"?}; soffit.delete {"id"}
#   foundation {"type": "stem" | "slab", "footingDepth"?: 1.2, "level"?: name | index}
#        -> {"foundations": ["fd0", ...], "handles", "walls": [founded wall labels], "concrete": [walls retyped]}. One per
#           level with exterior walls that have no level below them (updated in place when it exists). Stem: a 0.2 m
#           concrete wall flush with the outer face from the floor structure's underside down to a 0.6 x 0.25 m footing
#           whose bottom is footingDepth below the level's floor. Slab: the floors become 0.1 m slabs with a thickened
#           edge 0.3 m wide reaching 0.3 m below the slab's top. A basement's exterior walls take "Concrete 0.2" and a
#           footing under the floor structure. Footings draw dashed on S-FNDN in plan and fill in sections.
#   foundation.edit {"id", "type"?, "footingDepth"?}; foundation.delete {"id"}
#   arch.inspect {"rooms": true} rows gain "flatCeiling", "ceilingStack" ([{name, material, thickness}]), "roughCeiling" and
#        "finishedCeiling" (above the level's floor; null without a ceiling) and "ceilingWarning" (string or null).
#   arch.inspect {"trays": true} -> "trays": [{"id", "handle", "level", "room", "parent", "inset", "depth",
#        "sides": "vertical" | degrees, "polygon": drawn ring | null, "outline": ring | null, "ceiling": z, "layer"}]
#   arch.inspect {"soffits": true} -> "soffits": [{"id", "handle", "level", "polygon", "bottom", "top": [min, max],
#        "depth", "over": [cabinet labels], "material", "room", "layer"}]
#   arch.inspect {"foundations": true} -> "foundations": [{"id", "handle", "level", "type", "footingDepth", "basement",
#        "walls": [labels], "layer"}]
#   GLB / OBJ names: soffit-sf<n>, foundation-fd<n>-<k>, footing-fd<n>-<k>, slab-r<n>, ceiling-r<n>-layer, wall-w<n>-rise.
#   IFC: IfcFooting footing-fd<n>, IfcWall foundation-fd<n>, IfcCovering soffit-sf<n> (.CEILING.), IfcSlab slab-r<n>
#        (.BASESLAB.). DXF: closed dashed LWPOLYLINE proxies, soffits on A-CLNG and footings on S-FNDN.
#
# The phase-1 house every check uses (house_walls(): exactly these three wall commands, each 2.4 m high; without
# "height" a wall takes the level's ceiling + structure, 2.7 m, and pokes into the attic): w0 south, w1 east, w2 north,
# w3 west (the ring, 0.2 thick, exterior), w4 the interior wall along y = 3 and w5 along x = 6 (0.1 thick). After
# `roomdetect {}` the rooms are S (interior 0.1..9.9 x 0.1..2.95), NW (0.1..5.95 x 3.05..7.9) and NE (6.05..9.9 x
# 3.05..7.9); level 0 "Level 1" at 0, ceiling 2.4, structure 0.3. GABLE is the phase-4 6/12 roof with gables on w1 and
# w3: its underside is U = 2.4 + 0.5 (y + 0.1) for y <= 4 and 2.4 + 0.5 (8.1 - y) for y >= 4 (eave 2.4 at the outer
# faces, ridge 4.45 at y = 4).
import math, os, json
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-15', 'p15lib.inc')).read())

HOUSE_WALLS = [
    {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True},
    {"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"},
    {"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"},
]


def house_walls(level=None):
    """The house's three wall commands; with `level`, each also gets "level": level."""
    out = []
    for w in HOUSE_WALLS:
        p = dict(w)
        if level is not None:
            p["level"] = level
        out.append(cmd('wall', p))
    return out


HOUSE = house_walls() + ['roomdetect {}']


def gable(rise):
    """The phase-4 gable with another pitch rise (over a run of 12)."""
    return cmd('roofauto', {"pitchRise": rise, "pitchRun": 12,
                            "walls": [{"wall": "w1", "directive": "gable"}, {"wall": "w3", "directive": "gable"}]})


GABLE = gable(6)
INSPECT16 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "roofs": True, "furnishings": True,
                                 "lights": True, "cabinets": True, "trays": True, "soffits": True, "foundations": True,
                                 "views": True})

_HOUSE_ROOMS = {}


def house_rooms():
    """The house's room ids by name, read from arch.inspect once: {'S', 'NW', 'NE'}."""
    if not _HOUSE_ROOMS:
        rc, rr, so, se = run(HOUSE + [INSPECT16])
        insp = inspected(rr)
        _HOUSE_ROOMS.update({'S': room_at(insp, 5, 1.5), 'NW': room_at(insp, 3, 5.5), 'NE': room_at(insp, 8, 5.5)})
        if len(set(_HOUSE_ROOMS.values())) != 3:
            fail(f"the house does not have three rooms: {_HOUSE_ROOMS}")
    return dict(_HOUSE_ROOMS)


def ceiling_cmd(points, level=None):
    """build3d with a finished-ceiling query at plan points [[x, y], ...]."""
    p = {"ceiling": points}
    if level is not None:
        p["level"] = level
    return cmd('build3d', p)


def probe_cmd(points):
    return cmd('build3d', {"probe": points})


def _build3d_replies(rows):
    return [r for r in rows if isinstance(r, dict) and 'meshes' in r and 'rebuildMs' in r]


def ceilings(rows, nth=-1):
    """The 'ceiling' list of the nth build3d reply carrying one."""
    hits = [r for r in _build3d_replies(rows) if isinstance(r.get('ceiling'), list)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no build3d reply with a 'ceiling' list (#{nth}); replies: {json.dumps(rows)[-600:]}")
    return hits[nth]['ceiling']


def parts_of(rows, nth=-1):
    """The 'parts' object of the nth build3d reply."""
    hits = _build3d_replies(rows)
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no build3d reply (#{nth}); replies: {json.dumps(rows)[-600:]}")
    parts = hits[nth].get('parts')
    if not isinstance(parts, dict):
        fail(f"build3d reports no 'parts' object: {json.dumps(hits[nth])[:400]}")
    return parts


def trays_of(insp):
    return objects_named(insp, 'trays', 'trays')


def soffits_of(insp):
    return objects_named(insp, 'soffits', 'soffits')


def foundations_of(insp):
    return objects_named(insp, 'foundations', 'foundations')


def room_row(insp, rid):
    return by_id(objects_named(insp, 'rooms', 'rooms'), rid, 'arch.inspect rooms')


def item_top(insp, fid):
    """A furnishing's top (absolute): its elevation + height + its level's elevation."""
    f = by_id(furnishings_of(insp), fid, 'arch.inspect furnishings')
    size = f.get('size')
    if not (isinstance(size, list) and len(size) == 3):
        fail(f"furnishing {fid} has no [w, d, h] size: {f}")
    lv = [l for l in insp.get('levels') or [] if isinstance(l, dict) and l.get('index') == f.get('level')]
    if len(lv) != 1:
        fail(f"furnishing {fid}: no level row with index {f.get('level')!r} (arch.inspect needs \"levels\": true)")
    return num(f.get('elevation'), f"{fid} elevation") + num(size[2], f"{fid} height") + num(lv[0].get('elevation'), 'level elevation')


# ---- helpers for checks 03-10 and 13 (added with them; the ones above are unchanged) ----------------------------------

def run_ok(cmds, what, **kw):
    """run() where every step must be accepted: a refused or unknown step fails the check with its error lines (a
    refusal in the middle of a run still exits 0, so the stderr lines are what tell)."""
    rc, rr, so, se = run(cmds, expect_ok=False, **kw)
    errs = errors(se)
    if rc != 0 or errs:
        fail(f"{what}: a step is refused or unknown (exit {rc}): {' | '.join(errs[:4]) or se[-400:]}")
    return rr


def made(cmds, key, what):
    """Run `cmds` (the last one creates something) and return `key` of the last reply carrying it, so a label such as
    tr0, sf0, fd0 or cb0 comes from the session's own reply, never from the check. Fails naming the refusal."""
    rc, rr, so, se = run(cmds, expect_ok=False)
    hits = [r for r in rr if isinstance(r, dict) and key in r]
    if errors(se) or not hits:
        fail(f"{what}: refused or not a command yet (no reply with '{key}'): {' | '.join(errors(se)[:3]) or se[-300:]}")
    return hits[-1][key]


def build3d_of(rows, nth=-1):
    """The nth build3d reply."""
    hits = _build3d_replies(rows)
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no build3d reply (#{nth}); replies: {json.dumps(rows)[-600:]}")
    return hits[nth]


def probed(rows, points, nth=-1, what='probes'):
    """The answers of the nth build3d reply that probed (its 'probe' list is not empty), one per point."""
    hits = [r for r in _build3d_replies(rows) if isinstance(r.get('probe'), list) and r['probe']]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"{what}: no build3d reply with probe answers (#{nth}); replies: {json.dumps(rows)[-600:]}")
    got = hits[nth]['probe']
    if len(got) != len(points):
        fail(f"{what}: {len(points)} probes, {len(got)} answers")
    return got


def expect_hits(rows, cases, what, nth=-1):
    """cases: [([x, y, z], want)] with want None (nothing there), a part name, or (part, id). Reads the nth build3d reply
    that probed these points, in order."""
    got = probed(rows, [p for p, _ in cases], nth, what)
    for (p, w), g in zip(cases, got):
        if w is None:
            if g is not None:
                fail(f"{what}: nothing at {p}, the probe hit {g}")
            continue
        wp, wid = (w, None) if isinstance(w, str) else w
        if not isinstance(g, dict) or part(g) != wp or (wid is not None and g.get('id') != wid):
            fail(f"{what}: {p} is in {f'{wid} ' if wid is not None else 'a '}{wp}, the probe answered {g}")


def expect_ceilings(rows, points, want, what, nth=-1, tol=1e-6):
    """The finished ceilings of the nth build3d reply that answered a `ceiling` query, against `want` (z or None)."""
    got = ceilings(rows, nth)
    if not isinstance(got, list) or len(got) != len(want):
        fail(f"{what}: expected {len(want)} ceiling answers, got {got!r}")
    for p, g, w in zip(points, got, want):
        if w is None:
            if g is not None:
                fail(f"{what}: {p} is in no room, expected null, got {g!r}")
        else:
            near(g, w, tol, f"{what}: the ceiling at {p}")


def level_row(insp, ref):
    """The arch.inspect level row named `ref` or with index `ref` (needs "levels": true)."""
    rows = [l for l in insp.get('levels') or [] if isinstance(l, dict)
            and ((isinstance(ref, str) and l.get('name') == ref) or (isinstance(ref, int) and l.get('index') == ref))]
    if len(rows) != 1:
        fail(f"no single level row for {ref!r}: {[(l.get('index'), l.get('name')) for l in insp.get('levels') or [] if isinstance(l, dict)]}")
    return rows[0]


def on_level(v, lrow):
    """Whether a row's "level" (an index, or a name) is the level of row `lrow`."""
    return (isinstance(v, int) and not isinstance(v, bool) and v == lrow.get('index')) or (isinstance(v, str) and v == lrow.get('name'))


def ring_box(ring, what):
    """[x0, y0, x1, y1] of a ring of [x, y] points (fails on anything else)."""
    if not isinstance(ring, list) or len(ring) < 3 or not all(isinstance(q, list) and len(q) >= 2 for q in ring):
        fail(f"{what}: not a ring of [x, y] points: {ring!r}")
    xs = [num(q[0], f"{what} x") for q in ring]
    ys = [num(q[1], f"{what} y") for q in ring]
    return [min(xs), min(ys), max(xs), max(ys)]


def expect_box(ring, want, what, tol=1e-6):
    got = ring_box(ring, what)
    for k, name in enumerate(('x min', 'y min', 'x max', 'y max')):
        near(got[k], want[k], tol, f"{what} {name}")


def interior_point(poly):
    """A point inside a simple polygon of [x, y]: its area centroid when that is inside, else the middle of the widest
    inside span on the horizontal line through the middle of its bounding box."""
    ring_box(poly, "a polygon")
    pts = [(float(q[0]), float(q[1])) for q in poly]

    def inside(x, y):
        c = False
        for k in range(len(pts)):
            (x1, y1), (x2, y2) = pts[k], pts[(k + 1) % len(pts)]
            if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
                c = not c
        return c
    a = cx = cy = 0.0
    for k in range(len(pts)):
        (x1, y1), (x2, y2) = pts[k], pts[(k + 1) % len(pts)]
        cr = x1 * y2 - x2 * y1
        a += cr; cx += (x1 + x2) * cr; cy += (y1 + y2) * cr
    if abs(a) > 1e-12 and inside(cx / (3 * a), cy / (3 * a)):
        return [cx / (3 * a), cy / (3 * a)]
    y = (min(p[1] for p in pts) + max(p[1] for p in pts)) / 2 + 1e-7
    xs = sorted(x1 + (y - y1) * (x2 - x1) / (y2 - y1) for (x1, y1), (x2, y2) in zip(pts, pts[1:] + pts[:1]) if (y1 > y) != (y2 > y))
    spans = [(xs[k], xs[k + 1]) for k in range(0, len(xs) - 1, 2)]
    if not spans:
        fail(f"no interior point in {poly}")
    a0, a1 = max(spans, key=lambda s: s[1] - s[0])
    return [(a0 + a1) / 2, y]


def foreground(path, tol=8):
    """(fraction of pixels differing from the corner colour by more than `tol` in some channel, luma sd) of a PNG: the
    corner colour is the most common of the four corner pixels (the clear colour)."""
    w, h, nch, rows = png(path)
    if w < 2 or h < 2:
        fail(f"{os.path.basename(path)} is {w} x {h}")
    px = lambda x, y: tuple(rows[y][x * nch:x * nch + min(3, nch)])
    cs = [px(0, 0), px(w - 1, 0), px(0, h - 1), px(w - 1, h - 1)]
    clear = max(cs, key=cs.count)
    fg = sum(1 for y in range(0, h, 2) for x in range(0, w, 2)
             if max(abs(a - b) for a, b in zip(px(x, y), clear)) > tol)
    return fg / (((h + 1) // 2) * ((w + 1) // 2)), luma_sd(path)
