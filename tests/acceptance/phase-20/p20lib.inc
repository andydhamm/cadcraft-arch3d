# Phase-20 check helpers (plan views and dimensions). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())
# It loads the newest previous check library that exists: phase 19's, else 17's, else 16's (never phase 18's, planned
# in parallel). Each loads the ones before it, so p16lib's HOUSE, house_walls(), GABLE, house_rooms(), room_row,
# trays_of, soffits_of, run_ok, made, ring_box, expect_box and the older run, cmd, fail, near, num, errors, inspected,
# objects_named, by_id, reply_with, furnishings_of, png, luma_sd, digest, artifact are available. Names defined here
# start with pv_ / PV_ (and INSPECT20), so no earlier library's name is redefined.
#
# What the checks drive (lengths in metres; ids and handles as the replies give them, never hard-coded):
#   planview {"name" (required, new, case-insensitive), "from"?: view (default "Working"; copies every field),
#             "level"?: level name | index | "any" | null, "kind"?: "floor" | "reflectedCeiling" | "roof",
#             "layers"?: {"rules"?: [{"pattern", "on"?: bool, "transparency"?: 0..90}], "capture"?: bool},
#             "annotation"?: {"layer"?, "textStyle"?, "dimStyle"?}, "labels"?: {"name"?, "area"?, "dimensions"?,
#             "ceiling"?: bool}, "reference"?: {"level", "color"?: "#rrggbb", "transparency"?: 0..90} | null,
#             "zoom"?: "current" | {"center": [x, y], "width", "height"} | null, "open"?: bool (default true)}
#        -> {"planView", "open": [names], "active": name | null}. Wildcard rules: `*` any run, `?` one character,
#           case-insensitive, the last matching rule wins; the snapshot's layers (capture) win over the rules.
#   planview.edit {"name", "rename"?, ...every planview field but from/open} -> {"planView", "active"}
#   planview.delete {"name"} -> {"deleted", "open", "active"}; planview.close {"name"?: default the active one}
#        -> {"closed", "open", "active"}; planview.open {"name"} -> {"planView", "level", "kind", "layers": {"on", "off",
#        "dimmed"}, "clayer", "open", "active"}; planview.list {} -> {"planViews": [{"name", "level", "kind"}], "open",
#        "active"}. Opening a view first creates every layer the plan can draw the model's objects on, and its
#        annotation layer; it sets the shown level (bound views), the layers, CLAYER/TEXTSTYLE/DIMSTYLE and the zoom.
#        The seven templates: Working, Floor Plan, Electrical, Reflected Ceiling, Roof Plan, Kitchen & Bath, Site.
#   view.electrical {} = open the Electrical plan view; the reply keeps phase 15's keys and adds "planView".
#   export {"path", "planView": name} exports that view (not with "view" or "level"); the session is left alone.
#   room.label {"room", "area": "interior" | "standard"} -> {"room", "area"}
#   dimlinear / dimaligned: a model-space p1/p2 on a wall face, a wall centre line, an opening centre or an object edge
#        attaches to it (Arch attachment) and follows it.
#   dimauto {"level"?: name | index (default the shown level, else 0), "on"?: bool, "rows"?: ["openings", "walls",
#            "overall"]} -> {"level", "rows": [{"side", "row", "values"}], "dimensions": n}
#   dimdrive {"dimension": handle hex, "value": m, "object"?: "w5" | "o0" | "cb0" | "fx0" | "f0"} -> {"moved", "by",
#            "measurement"}; without "object" the selected object attached to the dimension moves.
#   arch.inspect {"planViews": true} -> "planViews": [{"name", "level", "kind", "labels", "annotation", "reference",
#        "zoom", "rules": [{"pattern", "on", "transparency"}], "snapshot"}], "openPlanViews", "activePlanView"
#   arch.inspect {"plan": true | "<view name>"} -> "plan": {"view", "kind", "level", "lines": [{"layer", "kind", "a",
#        "b", "length", "handle", "dashed", "color"?, "transparency"?}], "labels": [{"layer", "kind", "text", "at",
#        "height", "rotation", "handle"}]}. Line kinds: ridge, hip, valley, gableEnd, eave, rake, slopeArrow,
#        slopeArrowHead, trayOutline, trayTop, vaultLine, ceilingItem, soffit, reference. Label kinds: roomName,
#        roomArea, roomDimensions, roomCeiling, pitch, trayHeight.
#   arch.inspect {"dimensions": true} -> "dimensions": [{"handle" (hex, as select takes it), "kind", "rotation", "p1",
#        "p2", "at", "measurement", "layer", "style", "text", "attached": [{"point", "object", "feature", "edge"?,
#        "along"}]}]
#   arch.inspect {"autoDims": true} -> "autoDims": [{"level", "side", "row", "edited", "handles", "values"}]
#   arch.inspect {"rooms": true} rows gain "labelArea" and "ceilingText".
#
# The house (p16lib): w0 south, w1 east, w2 north, w3 west (exterior, 0.2 thick, centre lines on the 10 x 8 ring, outer
# faces -0.1..10.1 x -0.1..8.1), w4 along y = 3 and w5 along x = 6 (interior, 0.1 thick); rooms S 0.1..9.9 x 0.1..2.95,
# NW 0.1..5.95 x 3.05..7.9, NE 6.05..9.9 x 3.05..7.9.
import math, os, json, re, tempfile
_P20_HERE = os.path.dirname(os.path.abspath(__file__))
for _n in ('19', '17', '16'):
    _lib = os.path.join(_P20_HERE, '..', f'phase-{_n}', f'p{_n}lib.inc')
    if os.path.exists(_lib):
        exec(open(_lib).read())
        break
else:
    print("FAIL: no phase-19, phase-17 or phase-16 check library next to phase-20")
    raise SystemExit(1)

# Phase 4's window: o0, 0.9 wide, centred at x = 4 on the north wall (Opening.offset is the centre, measured from w2's
# start (10, 8)).
PV_WINDOW = cmd('window', {"wall": "w2", "offset": 6.0, "width": 0.9, "height": 1.2, "sill": 0.9})
# Phase 4's hip roof at 6/12 (overhang 0).
PV_HIP = cmd('roofauto', {"pitchRise": 6, "pitchRun": 12})
# Phase 16's suspended ceiling stack, in the list form (drywall 0.0127, air 0.2, tile 0.016: finished 2.1713 -> "2.17").
PV_SUSPENDED = [{"name": "drywall", "material": "drywall", "thickness": 0.0127},
                {"name": "air", "material": None, "thickness": 0.2},
                {"name": "ceiling tile", "material": "ceiling-tile", "thickness": 0.016}]
# Phase 9's sloped site (one major contour at x = 20) and a plant. Neither command creates a layer.
PV_TERRAIN = cmd('terrain', {"boundary": [[0, 0], [40, 0], [40, 30], [0, 30]],
                             "points": [[0, 0, 0.0], [40, 0, 2.0], [40, 30, 2.0], [0, 30, 0.0]]})
PV_PLANT = cmd('plant', {"species": "Sugar Maple", "at": [8, 22], "age": 10})
PV_TEMPLATES = ["Working", "Floor Plan", "Electrical", "Reflected Ceiling", "Roof Plan", "Kitchen & Bath", "Site"]
INSPECT20 = cmd('arch.inspect', {"walls": True, "rooms": True, "openings": True, "levels": True, "roofs": True,
                                 "cabinets": True, "furnishings": True, "trays": True, "soffits": True,
                                 "planViews": True, "dimensions": True, "autoDims": True})
PV_LAYERS = 'layer {}'


def pv_plan(rows, nth=-1):
    """The 'plan' object of the nth reply carrying one (arch.inspect {"plan": ...})."""
    hits = [r['plan'] for r in rows if isinstance(r, dict) and isinstance(r.get('plan'), dict)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply with a 'plan' object (#{nth}): arch.inspect {{\"plan\": ...}} is not there yet; "
             f"replies: {json.dumps(rows)[-500:]}")
    p = hits[nth]
    if not isinstance(p.get('lines'), list) or not isinstance(p.get('labels'), list):
        fail(f"the plan object has no 'lines' and 'labels' lists: {json.dumps(p)[:400]}")
    return p


def pv_lines(plan, kind=None, layer=None):
    out = [l for l in plan.get('lines') or [] if isinstance(l, dict)]
    if kind is not None:
        out = [l for l in out if l.get('kind') == kind]
    if layer is not None:
        out = [l for l in out if l.get('layer') == layer]
    return out


def pv_labels(plan, kind=None, layer=None):
    out = [l for l in plan.get('labels') or [] if isinstance(l, dict)]
    if kind is not None:
        out = [l for l in out if l.get('kind') == kind]
    if layer is not None:
        out = [l for l in out if l.get('layer') == layer]
    return out


def pv_pt(p, what):
    if not (isinstance(p, list) and len(p) == 2):
        fail(f"{what}: not an [x, y] point: {p!r}")
    return (num(p[0], f"{what} x"), num(p[1], f"{what} y"))


def pv_len(line):
    (ax, ay), (bx, by) = pv_pt(line.get('a'), 'line a'), pv_pt(line.get('b'), 'line b')
    return math.hypot(bx - ax, by - ay)


def pv_bbox(lines):
    """(x0, y0, x1, y1) over the end points of `lines`."""
    pts = [pv_pt(l.get(k), f"line {k}") for l in lines for k in ('a', 'b')]
    if not pts:
        fail("pv_bbox: no lines")
    return (min(p[0] for p in pts), min(p[1] for p in pts), max(p[0] for p in pts), max(p[1] for p in pts))


def pv_by_handle(items):
    out = {}
    for it in items:
        out.setdefault(it.get('handle'), []).append(it)
    return out


def pv_inside(pt, ring):
    """Whether pt is strictly inside the polygon ring ([[x, y], ...] or [(x, y), ...]) by the even-odd rule."""
    x, y = pt
    pts = [(float(q[0]), float(q[1])) for q in ring]
    c = False
    for k in range(len(pts)):
        (x1, y1), (x2, y2) = pts[k], pts[(k + 1) % len(pts)]
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            c = not c
    return c


def pv_box_ring(b):
    x0, y0, x1, y1 = b
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def pv_seg_dist(p, a, b):
    """Distance from point p to segment a-b."""
    (px, py), (ax, ay), (bx, by) = p, a, b
    dx, dy = bx - ax, by - ay
    L2 = dx * dx + dy * dy
    t = 0.0 if L2 <= 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / L2))
    return math.hypot(px - ax - t * dx, py - ay - t * dy)


def pv_closed(lines, what):
    """The lines form one closed loop: every end point is shared by exactly two line ends (+-1e-6)."""
    ends = [pv_pt(l.get(k), what) for l in lines for k in ('a', 'b')]
    for p in ends:
        n = sum(1 for q in ends if abs(p[0] - q[0]) <= 1e-6 and abs(p[1] - q[1]) <= 1e-6)
        if n != 2:
            fail(f"{what}: the lines do not form a closed loop (end {p} is shared by {n} line ends): "
                 f"{json.dumps(lines)[:400]}")


def pv_views(rows, nth=-1):
    """(planViews rows, openPlanViews, activePlanView) of the nth arch.inspect reply with planViews."""
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('planViews'), list) and 'openPlanViews' in r]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply with planViews / openPlanViews (#{nth}); replies: {json.dumps(rows)[-400:]}")
    r = hits[nth]
    return r['planViews'], r.get('openPlanViews'), r.get('activePlanView')


def pv_view(views, name):
    hits = [v for v in views if isinstance(v, dict) and v.get('name') == name]
    if len(hits) != 1:
        fail(f"no single plan view named {name!r}: {[v.get('name') for v in views if isinstance(v, dict)]}")
    return hits[0]


def pv_layer_rows(rows, nth=-1):
    """{name: row} of the nth `layer {}` reply."""
    hits = [r for r in rows if isinstance(r, dict) and isinstance(r.get('layers'), list) and 'states' in r]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no `layer {{}}` reply (#{nth}); replies: {json.dumps(rows)[-300:]}")
    return {l.get('name'): l for l in hits[nth]['layers'] if isinstance(l, dict)}


def pv_layer(layers, name, what=''):
    if name not in layers:
        fail(f"{what}: the drawing has no layer {name}: {sorted(map(str, layers))}")
    return layers[name]


def pv_dims(rows, nth=-1):
    """The 'dimensions' rows of the nth arch.inspect reply with dimensions."""
    hits = [r['dimensions'] for r in rows if isinstance(r, dict) and isinstance(r.get('dimensions'), list)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply with a 'dimensions' list (#{nth}): arch.inspect {{\"dimensions\": true}} is not "
             f"there yet; replies: {json.dumps(rows)[-300:]}")
    return hits[nth]


def pv_dim(dims, handle):
    hits = [d for d in dims if isinstance(d, dict) and d.get('handle') == handle]
    if len(hits) != 1:
        fail(f"no single dimension row with handle {handle}: {[d.get('handle') for d in dims if isinstance(d, dict)]}")
    return hits[0]


def pv_dim_valued(dims, v, tol=1e-6):
    return [d for d in dims if isinstance(d, dict) and isinstance(d.get('measurement'), (int, float))
            and abs(d['measurement'] - v) <= tol]


def pv_attached(dim):
    """{point: (object, feature)} of a dimension row."""
    out = {}
    for a in dim.get('attached') or []:
        if isinstance(a, dict):
            out[a.get('point')] = (a.get('object'), a.get('feature'))
    return out


def pv_auto(rows, nth=-1):
    hits = [r['autoDims'] for r in rows if isinstance(r, dict) and isinstance(r.get('autoDims'), list)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply with an 'autoDims' list (#{nth}): arch.inspect {{\"autoDims\": true}} is not "
             f"there yet; replies: {json.dumps(rows)[-300:]}")
    return hits[nth]


def pv_auto_row(auto, side, row, level=0):
    hits = [r for r in auto if isinstance(r, dict) and r.get('side') == side and r.get('row') == row
            and r.get('level') == level]
    if len(hits) != 1:
        fail(f"no single auto-dimension row level {level} {side} {row}: "
             f"{[(r.get('level'), r.get('side'), r.get('row'), r.get('values')) for r in auto if isinstance(r, dict)]}")
    return hits[0]


def pv_values(row, want, what, tol=1e-6):
    got = row.get('values')
    if not isinstance(got, list) or len(got) != len(want) or any(
            not isinstance(g, (int, float)) or abs(g - w) > tol for g, w in zip(got, want)):
        fail(f"{what}: expected {want}, got {got!r}")


def pv_svg_colours(path):
    """The set of lower-case #rrggbb colours in stroke= and fill= attributes (and style properties) of an SVG."""
    try:
        text = open(path, encoding='utf-8', errors='replace').read()
    except OSError as e:
        fail(f"cannot read {path}: {e}")
    found = re.findall(r'(?:stroke|fill)\s*[=:]\s*["\']?\s*(#[0-9a-fA-F]{6})\b', text)
    return {c.lower() for c in found}


def pv_proxy_hex(label):
    """The Handle::hex spelling `select` and dimdrive take for w<n> / o<n> (arch.inspect's "handle" is another one)."""
    m = re.fullmatch(r'([wo])(\d+)', label)
    if not m:
        fail(f"pv_proxy_hex: unsupported label {label!r}")
    n = int(m.group(2)) + (0x8000 if m.group(1) == 'o' else 0)
    return "%X" % (0xFFFFFFFF00000000 + n)


def pv_export(cmds, view, out, expect_ok=True):
    """Run `cmds` then `export {"path": out, "planView": view}`; returns run()'s tuple."""
    return run(cmds + [cmd('export', {"path": out, "planView": view})], expect_ok=expect_ok)


def pv_wall(insp, label):
    return by_id(objects_named(insp, 'walls', 'walls'), label, 'arch.inspect walls')


def pv_xy(p, what):
    """[x, y] (or [x, y, z]) -> (x, y)."""
    if not (isinstance(p, list) and len(p) >= 2):
        fail(f"{what}: not a point: {p!r}")
    return (num(p[0], f"{what} x"), num(p[1], f"{what} y"))


def _pv_r(pattern, on=None, transparency=None):
    return {"pattern": pattern, "on": on, "transparency": transparency}


def _pv_l(name=False, area=False, dimensions=False, ceiling=False):
    return {"name": name, "area": area, "dimensions": dimensions, "ceiling": ceiling}


# The seven templates exactly as the plan defines them: (kind, rules, room-label flags, annotation layer).
PV_TEMPLATE_SPEC = {
    "Working": ("floor", [_pv_r("*", True, 0)], _pv_l(True, True), "A-ANNO-NOTE"),
    "Floor Plan": ("floor", [_pv_r("*", True, 0), _pv_r("E-*", False), _pv_r("A-ROOF*", False), _pv_r("C-*", False),
                             _pv_r("L-*", False)], _pv_l(True, True, True), "A-ANNO-NOTE"),
    "Electrical": ("floor", [_pv_r("*", None, 70), _pv_r("E-*", True, 0)], _pv_l(True), "E-ANNO-NOTE"),
    "Reflected Ceiling": ("reflectedCeiling",
                          [_pv_r("*", False), _pv_r("0", True, 0), _pv_r("A-WALL*", True, 0), _pv_r("A-GLAZ", True, 0),
                           _pv_r("A-CLNG*", True, 0), _pv_r("E-LITE*", True, 0), _pv_r("A-ROOM*", True, 0),
                           _pv_r("A-ANNO*", True, 0), _pv_r("A-ANNO-DIMS", False), _pv_r("A-FLOR-CASE-UPPR", True, 50)],
                          _pv_l(True, ceiling=True), "A-CLNG-ANNO"),
    "Roof Plan": ("roof", [_pv_r("*", False), _pv_r("0", True, 0), _pv_r("A-ROOF*", True, 0), _pv_r("A-WALL", True, 70),
                           _pv_r("A-ANNO*", True, 0), _pv_r("A-ANNO-DIMS", False)], _pv_l(), "A-ROOF-ANNO"),
    "Kitchen & Bath": ("floor", [_pv_r("*", True, 0), _pv_r("I-FURN*", True, 70), _pv_r("E-*", False),
                                 _pv_r("A-ROOF*", False), _pv_r("C-*", False), _pv_r("L-*", False)],
                       _pv_l(True, dimensions=True), "A-ANNO-NOTE"),
    "Site": ("floor", [_pv_r("*", False), _pv_r("0", True, 0), _pv_r("A-WALL", True, 0), _pv_r("A-ROOF*", True, 0),
                       _pv_r("C-*", True, 0), _pv_r("L-*", True, 0), _pv_r("V-*", True, 0), _pv_r("A-ANNO*", True, 0),
                       _pv_r("A-ANNO-DIMS", False)], _pv_l(), "C-ANNO-NOTE"),
}


def pv_rules(v):
    """A planViews row's rules as [{"pattern", "on", "transparency"}] (missing keys read as null)."""
    rules = v.get('rules')
    if not isinstance(rules, list):
        fail(f"plan view {v.get('name')!r} has no 'rules' list: {json.dumps(v)[:300]}")
    return [{"pattern": r.get('pattern'), "on": r.get('on'), "transparency": r.get('transparency')}
            for r in rules if isinstance(r, dict)]


def pv_expect_template(v, name=None):
    """A planViews row equals the template `name` (default its own name): kind, rules, labels, annotation layer, any
    level, no reference, no zoom, no saved layers."""
    name = name or v.get('name')
    if name not in PV_TEMPLATE_SPEC:
        fail(f"{name!r} is not a template view")
    kind, rules, labels, layer = PV_TEMPLATE_SPEC[name]
    what = f"plan view {v.get('name')!r} (from the template {name!r})"
    if v.get('kind') != kind:
        fail(f"{what}: kind {kind!r}, got {v.get('kind')!r}")
    if pv_rules(v) != rules:
        fail(f"{what}: rules {rules}, got {pv_rules(v)}")
    got = v.get('labels') if isinstance(v.get('labels'), dict) else {}
    if {k: got.get(k) for k in labels} != labels:
        fail(f"{what}: room-label flags {labels}, got {v.get('labels')}")
    ann = v.get('annotation') if isinstance(v.get('annotation'), dict) else {}
    if ann.get('layer') != layer:
        fail(f"{what}: annotation layer {layer!r}, got {v.get('annotation')}")
    if v.get('reference') is not None or v.get('zoom') is not None or v.get('snapshot') != 0:
        fail(f"{what}: no reference, no zoom and no saved layers: {json.dumps(v)[:300]}")
