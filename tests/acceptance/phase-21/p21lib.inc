# Phase-21 check helpers (schedules, elevations and sheets). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())
# It loads the newest previous check library that exists: phase 20's, else 19's, 17's or 16's (never phase 18's, planned
# in parallel). Each loads the ones before it, so p16lib's HOUSE, house_walls(), house_rooms(), run, run_ok, made, cmd,
# fail, near, num, errors, inspected, objects_named, by_id, room_at, reply_with, parse_rows, cli_args, artifact and ROOT
# are available. Names defined here start with sh_ / SH_ (and INSPECT21), so no earlier library's name is redefined;
# nothing here needs a phase-17/19/20 helper.
#
# What the checks drive (lengths in metres, paper in millimetres; ids and handles as the replies give them):
#   opening.edit {"id": "o1", "width"?, "height"?, "sill"?, "style"?} -> {"opening", "width", "height", "sill", "style"}
#        re-validated on its wall exactly as a new opening; a refusal changes nothing.
#   schedule {"kind": "door" | "window" | "cabinet" | "room", "columns"?: [column], "name"?, "at"?: [x, y] (model
#            space, the table's top-left), "level"?: name | index | "all"}
#        -> {"schedule": "sc0", "table": handle hex (a CADCraft TABLE in model space on A-ANNO-SCHD), "name", "kind",
#            "columns", "rows": [{"number", "count", "members": [labels], "cells": [strings]}]}
#        Columns: door: number, count, width, height, style, kind, swing, level (default number, count, width, height,
#        style); window: number, count, width, height, sill, style, level (default number, count, width, height, sill,
#        style); cabinet: number, count, kind, width, depth, height, fronts, doors, drawers, level (default number,
#        count, kind, width, depth, height, fronts); room: name, number, level, floor, walls, ceiling, area,
#        ceilingHeight (default name, floor, walls, ceiling, area). Header titles: "No.", "Count", "Width", "Height",
#        "Sill", "Style", "Kind", "Swing", "Level", "Depth", "Fronts", "Doors", "Drawers", "Name", "Floor", "Walls",
#        "Ceiling", "Area", "Ceiling Height". Cells: lengths in metres with 3 decimals ("0.900"), areas with 2
#        ("28.37"), counts as integers, materials by name ("wood"; a colour paint as "#c04020"), several wall materials
#        sorted and joined with ", ". The TABLE has a title row (the name), a header row, then one row per group.
#        Schedule numbers: identical items share one number (doors and garage doors/cased openings: kind, style,
#        width, height; windows: style, width, height, sill; cabinets: kind, width, depth, height, fronts, doors,
#        drawers; lengths compared to 1e-6), numbered D01.., W01.., C01.. per prefix in the order of each group's
#        lowest id; a row per number. Room schedules have one row per room (number = the room's id, count 1).
#        Live: after every successful undoable command each schedule's TABLE is rewritten when its rows changed, in
#        that command's undo step.
#   schedule.edit {"id", "columns"?, "name"?, "level"?, "at"?} -> as schedule; schedule.delete {"id"} -> {"deleted"}
#        (the TABLE goes with it).
#   arch.inspect {"schedules": true} -> "schedules": [{"id", "name", "kind", "columns", "level": index | null,
#        "table": hex | null, "rows": [...]}]; openings and cabinets rows gain "number" ("W01", "D01", "C01" | null).
#   planview {..., "labels": {..., "numbers"?: bool}}: Working, Floor Plan and Kitchen & Bath show schedule numbers.
#        arch.inspect {"plan": ...} labels of kind "scheduleNumber" (text "W01", layer A-DOOR-IDEN | A-GLAZ-IDEN |
#        A-FLOR-CASE-IDEN, the object's handle): an opening's 0.35 m beyond its wall's right face (outside a ring wall
#        drawn counter-clockwise), a cabinet's at its plan centre. The plan with no view open shows none.
#   elevation {"room": id | name, "wall": "w5" | "side": "north" | "south" | "east" | "west", "level"?, "name"?,
#              "insert"?, "scale"?, "dimensions"?} -> the phase-7 reply; an INTERIOR elevation: looking from the room at
#        that wall (the wall's face on the room's boundary), showing only what lies inside the room's outline between
#        its floor and its finished ceiling, with cabinet door swings and automatic dimensions (each cabinet run's
#        cabinets, the run, the overall width and the floor-to-ceiling height).
#   arch.inspect {"views": true} rows gain "kind" ("elevation" | "section" | "interior"), "room", "wall", "patterns":
#        [{"a", "b", "id", "material"}] (A-ELEV-PATT), "indicators": [{"a", "b", "id"}] (A-ELEV-SWNG), "labels":
#        [{"text", "at", "height", "layer", "id"}] (schedule numbers on A-ELEV-IDEN), "notes": [{"id", "kind", "text",
#        "at", "b", "placed", "height", "layer", "attach", "value"}]. View coordinates (u, v): u = p . (ly, -lx) for the
#        look (lx, ly), v = z; a view's geometry is placed at insert + (u, v) - extent[0..2].
#        Lines carry the kind and id of their object: "cabinet" / "cb0" for casework (no longer "wall" / "w0").
#   view.note {"view": "v0", "text", "at": [u, v] | "attach": {"object": label, "corner": "topLeft" | "topRight" |
#              "bottomLeft" | "bottomRight" | "center"}, "offset"?: [du, dv], "height"?, "kind"?: "text" | "marker",
#              "layer"?} -> {"view", "note": "n0", "at", "placed"}. view.dim {"view", "a", "b" ([u, v] or an
#              attach object), "offset"?} -> {"view", "note", "value"}. view.note.delete {"view", "note"}.
#        Notes are stored with the view: free ones keep their view coordinates when it regenerates; attached ones
#        follow the corner of their object's lines.
#   sheetset {"project"?, "date"?} -> {"project", "date"}
#   sheet {"name", "number"?, "title"?, "size"?: paper (default "ARCH D"), "landscape"?: true,
#          "views": [{"view": plan view name | view id or name | schedule id or name, "level"?, "scale": "1:50" | 50,
#                     "at"?: [x, y] mm} | {"camera": saved camera, "width"?: mm, "pixels"?: [w, h], "at"?}]}
#        -> {"sheet", "number", "layout", "paper": [w, h] mm, "views": [{"number", "source": {"kind", "name"},
#            "scale", "at", "size", "viewport": hex | null, "image": name | null}]}. A CADCraft layout with one locked
#           viewport per view at 1000 / scale paper mm per metre (a plan view's viewport draws that view: its level,
#           layers and annotation), the camera's render as a raster image, and the master title block (block
#           ARCH-TITLE-BLOCK with attributes SHEETNO, SHEETTITLE, PROJECT, DATE, SCALE from the macros
#           %sheet_number%, %sheet_title%, %project%, %date%, %scale%).
#   sheet.edit {"name", "rename"?, "number"?, "title"?, "add"?, "remove"?, "order"?}; sheet.delete {"name"};
#   sheet.list {}; sheet.open {"name" | "view"} -> {"sheet", "layout"} (switches to it).
#   arch.inspect {"sheets": true} -> "sheets": [{... the sheet reply ..., "titleBlock": {"sheetNumber", "sheetTitle",
#        "project", "date", "scale"}}], "sheetSet": {"project", "date"}.
#   plot {"sheets": "all" | [names], "path"} -> one PDF, a page per sheet in sheet order (page labels: the layouts).
#   Plan callouts: arch.inspect {"plan": ...} labels of kind "callout" ("<view number>/<sheet number>", layer
#        A-ANNO-SYMB, the view's handle) for every elevation, section and interior elevation placed on a sheet.
import math, os, json, re, tempfile, zlib
_P21_HERE = os.path.dirname(os.path.abspath(__file__))
for _n in ('20', '19', '17', '16'):
    _lib = os.path.join(_P21_HERE, '..', f'phase-{_n}', f'p{_n}lib.inc')
    if os.path.exists(_lib):
        exec(open(_lib).read())
        break
else:
    print("FAIL: no phase-20, 19, 17 or 16 check library next to phase-21")
    raise SystemExit(1)

# Four 0.9 x 1.2 windows (sill 0.9) and one 1.2 x 1.2: o0..o2 on the south wall at x = 2, 5, 8, o3 on the north wall at
# x = 8 and o4 (the wide one) on the north wall at x = 3 (Opening.offset is the centre, from the wall's start).
SH_WINDOWS = [cmd('window', {"wall": "w0", "offset": 2.0, "width": 0.9, "height": 1.2, "sill": 0.9}),
              cmd('window', {"wall": "w0", "offset": 5.0, "width": 0.9, "height": 1.2, "sill": 0.9}),
              cmd('window', {"wall": "w0", "offset": 8.0, "width": 0.9, "height": 1.2, "sill": 0.9}),
              cmd('window', {"wall": "w2", "offset": 2.0, "width": 0.9, "height": 1.2, "sill": 0.9}),
              cmd('window', {"wall": "w2", "offset": 7.0, "width": 1.2, "height": 1.2, "sill": 0.9})]
# A base cabinet against the interior wall w5 on its north-west face (x = 5.95), along y 5.2..5.8.
SH_CAB = cmd('cabinet', {"kind": "base", "wall": "w5", "offset": 2.5})
INSPECT21 = cmd('arch.inspect', {"walls": True, "rooms": True, "openings": True, "levels": True, "cabinets": True,
                                 "views": True, "schedules": True, "sheets": True})
SH_TOL = 1e-3


def sh_tmp():
    return tempfile.mkdtemp(prefix='p21-')


def sh_list(insp, key, what=None):
    v = insp.get(key)
    if not isinstance(v, list) or not all(isinstance(x, dict) for x in v):
        fail(f"arch.inspect has no '{key}' list ({what or key}): {json.dumps(insp)[:300]}")
    return v


def sh_one(items, key, value, what):
    hits = [x for x in items if isinstance(x, dict) and x.get(key) == value]
    if len(hits) != 1:
        fail(f"{what}: no single row with {key} = {value!r}: {[x.get(key) for x in items if isinstance(x, dict)]}")
    return hits[0]


def sh_reply(rows, key, nth=-1, what=None):
    """The nth reply that is an object carrying `key`."""
    hits = [r for r in rows if isinstance(r, dict) and key in r]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no reply with '{key}' ({what or key}); replies: {json.dumps(rows)[-500:]}")
    return hits[nth]


def sh_counts(sched):
    """[(number, count)] of a schedule row list, in table order."""
    out = []
    for r in sched.get('rows') or []:
        if not isinstance(r, dict):
            fail(f"a schedule row is not an object: {r!r}")
        out.append((r.get('number'), r.get('count')))
    return out


def sh_csv(rows, nth=-1):
    """The cells of the nth `tableexport` reply as a list of rows of strings."""
    r = sh_reply(rows, 'csv', nth, 'tableexport')
    text = r.get('csv')
    if not isinstance(text, str):
        fail(f"tableexport gave no csv text: {r}")
    import csv as _csv, io as _io
    return [row for row in _csv.reader(_io.StringIO(text))]


def sh_table_rows(cells, what):
    """(header, data rows) of a schedule TABLE's csv: row 0 is the title, row 1 the header."""
    if len(cells) < 2:
        fail(f"{what}: the table has no title and header rows: {cells}")
    return [c.strip() for c in cells[1]], [[c.strip() for c in row] for row in cells[2:]]


def sh_col(header, title, what):
    low = [h.lower() for h in header]
    if title.lower() not in low:
        fail(f"{what}: no '{title}' column in the header {header}")
    return low.index(title.lower())


def sh_view(insp, vid):
    return sh_one(sh_list(insp, 'views', 'views'), 'id', vid, 'arch.inspect views')


def sh_pt(p, what):
    if not (isinstance(p, list) and len(p) >= 2 and all(isinstance(c, (int, float)) for c in p[:2])):
        fail(f"{what}: not an [x, y] point: {p!r}")
    return (float(p[0]), float(p[1]))


def sh_items(view, key, oid=None, kind=None):
    v = view.get(key)
    if not isinstance(v, list):
        fail(f"view {view.get('id')} has no '{key}' list: {json.dumps(view)[:300]}")
    out = [x for x in v if isinstance(x, dict)]
    if oid is not None:
        out = [x for x in out if x.get('id') == oid]
    if kind is not None:
        out = [x for x in out if x.get('kind') == kind]
    return out


def sh_box(lines, what):
    """(u0, v0, u1, v1) over the end points of `lines`."""
    pts = [sh_pt(l.get(k), f"{what} {k}") for l in lines for k in ('a', 'b')]
    if not pts:
        fail(f"{what}: no lines")
    return (min(p[0] for p in pts), min(p[1] for p in pts), max(p[0] for p in pts), max(p[1] for p in pts))


def sh_covers(lines, a, b, tol=SH_TOL):
    """Whether the union of the lines collinear with segment a-b (within tol) covers it."""
    (ax, ay), (bx, by) = a, b
    L = math.hypot(bx - ax, by - ay)
    if L <= 0:
        return False
    dx, dy = (bx - ax) / L, (by - ay) / L
    spans = []
    for l in lines:
        p, q = sh_pt(l.get('a'), 'line a'), sh_pt(l.get('b'), 'line b')
        if any(abs((x - ax) * dy - (y - ay) * dx) > tol for x, y in (p, q)):
            continue
        t0, t1 = sorted(((p[0] - ax) * dx + (p[1] - ay) * dy, (q[0] - ax) * dx + (q[1] - ay) * dy))
        spans.append((t0, t1))
    t = 0.0
    for t0, t1 in sorted(spans):
        if t0 > t + tol:
            return False
        t = max(t, t1)
    return t >= L - tol


def sh_plan(rows, nth=-1):
    """The 'plan' object of the nth arch.inspect reply carrying one (phase 20's arch.inspect {"plan": ...})."""
    hits = [r['plan'] for r in rows if isinstance(r, dict) and isinstance(r.get('plan'), dict)]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no arch.inspect reply with a 'plan' object (#{nth}): phase 20's plan observable is not there; "
             f"replies: {json.dumps(rows)[-300:]}")
    p = hits[nth]
    if not isinstance(p.get('labels'), list) or not isinstance(p.get('lines'), list):
        fail(f"the plan object has no 'lines' and 'labels' lists: {json.dumps(p)[:300]}")
    return p


def sh_plan_labels(plan, kind):
    return [l for l in plan.get('labels') or [] if isinstance(l, dict) and l.get('kind') == kind]


def sh_svg_strokes(path):
    """[(colour, [(x, y), ...])] of every <polyline>/<path>/<line> with a stroke in an SVG, in viewBox units, and the
    number of viewBox units per millimetre (from the width attribute when it is in mm, else viewBox / paper)."""
    try:
        text = open(path, encoding='utf-8', errors='replace').read()
    except OSError as e:
        fail(f"cannot read {path}: {e}")
    out = []
    for m in re.finditer(r'<(polyline|path|line)\b([^>]*)>', text):
        attrs = m.group(2)
        col = re.search(r'stroke\s*=\s*"(#[0-9a-fA-F]{6})"', attrs)
        if not col:
            continue
        pts = []
        pm = re.search(r'\bpoints\s*=\s*"([^"]*)"', attrs)
        if pm:
            nums = [float(x) for x in re.findall(r'-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?', pm.group(1))]
            pts = list(zip(nums[0::2], nums[1::2]))
        dm = re.search(r'\bd\s*=\s*"([^"]*)"', attrs)
        if dm:
            nums = [float(x) for x in re.findall(r'-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?', dm.group(1))]
            pts = list(zip(nums[0::2], nums[1::2]))
        if m.group(1) == 'line':
            g = {k: float(v) for k, v in re.findall(r'\b(x1|y1|x2|y2)\s*=\s*"(-?[\d.eE+-]+)"', attrs)}
            if len(g) == 4:
                pts = [(g['x1'], g['y1']), (g['x2'], g['y2'])]
        if pts:
            out.append((col.group(1).lower(), pts))
    vb = re.search(r'viewBox\s*=\s*"\s*[-\d.]+\s+[-\d.]+\s+([\d.]+)\s+([\d.]+)', text)
    return out, (float(vb.group(1)), float(vb.group(2))) if vb else None


def sh_svg_extent(path, colour):
    """(width, height) in viewBox units of every stroke of `colour`, or None when there is none."""
    strokes, _ = sh_svg_strokes(path)
    pts = [p for c, ps in strokes if c == colour.lower() for p in ps]
    if not pts:
        return None
    return (max(p[0] for p in pts) - min(p[0] for p in pts), max(p[1] for p in pts) - min(p[1] for p in pts))


def sh_svg_colours(path):
    strokes, _ = sh_svg_strokes(path)
    return {c for c, _ in strokes}


def sh_pdf(path):
    """{"pages": n, "boxes": [(w, h) pt per page], "labels": [page label strings], "ops": [line ops per stream]}."""
    try:
        b = open(path, 'rb').read()
    except OSError as e:
        fail(f"no PDF at {path}: {e}")
    if not b.startswith(b'%PDF'):
        fail(f"{path} is not a PDF")
    pages = re.findall(rb'/Type\s*/Page(?![sL])[^>]*?/MediaBox\s*\[\s*([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s*\]', b)
    pages += re.findall(rb'/MediaBox\s*\[\s*([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s*\][^>]*?/Type\s*/Page(?![sL])', b)
    count = re.search(rb'/Type\s*/Pages\b[^>]*?/Count\s+(\d+)', b) or re.search(rb'/Count\s+(\d+)[^>]*?/Type\s*/Pages\b', b)
    labels = [m.decode('latin-1') for m in re.findall(rb'/PageLabel\s*/P\s*\(([^)]*)\)', b)]
    ops = []
    for m in re.finditer(rb'/Length\s+(\d+)[^>]*>>\s*stream\r?\n', b):
        s = b[m.end():m.end() + int(m.group(1))]
        try:
            s = zlib.decompress(s)
        except zlib.error:
            pass
        ops.append(len(re.findall(rb'\sl\b', s)))
    return {"pages": int(count.group(1)) if count else len(pages),
            "boxes": [(float(x1) - float(x0), float(y1) - float(y0)) for x0, y0, x1, y1 in pages],
            "labels": labels, "ops": ops}


def sh_dxf_pairs(path):
    """[(code, value)] of a DXF file."""
    try:
        lines = open(path, encoding='utf-8', errors='replace').read().splitlines()
    except OSError as e:
        fail(f"cannot read {path}: {e}")
    return [(lines[i].strip(), lines[i + 1].strip()) for i in range(0, len(lines) - 1, 2)]


def sh_dxf_attribs(path):
    """[(tag, value)] of every ATTRIB entity in a DXF file."""
    pairs = sh_dxf_pairs(path)
    out, cur = [], None
    for code, val in pairs:
        if code == '0':
            if cur is not None and cur.get('2') is not None:
                out.append((cur['2'], cur.get('1', '')))
            cur = {} if val == 'ATTRIB' else None
        elif cur is not None and code in ('1', '2') and code not in cur:
            cur[code] = val
    if cur is not None and cur.get('2') is not None:
        out.append((cur['2'], cur.get('1', '')))
    return out


def sh_run_ok(cmds, what, **kw):
    """run() where every step must be accepted (an error line fails the check, naming the refusal)."""
    rc, rr, so, se = run(cmds, expect_ok=False, **kw)
    errs = errors(se)
    if rc != 0 or errs:
        fail(f"{what}: a step is refused or unknown (exit {rc}): {' | '.join(errs[:4]) or se[-400:]}")
    return rr
