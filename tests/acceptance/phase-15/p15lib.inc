# Phase-15 check helpers (electrical, light sets, split levels, grade classes, the showcase house). A fixture, not a check
# (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())
# It loads the phase-14 helpers, which load phases 13..8 (run, cmd, cli, inspected, reply_with, near, fail, probes, part, glb,
# node_box, png, export, step_file, of_type, validate_glb, room_cmds, lights_of, artifact, errors, surfaces_of, furnishings_of).
#
# What the checks drive (lengths in metres, angles in degrees; ids as the replies give them):
#   outlet {"kind": "duplex"|"gfci"|"240v", "wall": "w0", "offset": m | "point": [x, y], "side": "left"|"right",
#           "height"?: m, "counter"?: true, "level"?: name|index}
#   switch {"kind": "single-pole"|"three-way"|"dimmer", "wall", "offset" | "point", "side", "height"?, "level"?}
#        -> {"device": "el0", "handle", "kind", "layer": "E-POWR"|"E-LITE", "at": [x, y], "z", "wall", "side", "offset",
#            "room": "r0"|null}
#        `at` is the plate's centre on the wall face (offset along the wall from its start); `z` is absolute: the level's
#        elevation + height (outlets 0.3, switches 1.2), or with "counter": the counter top in front + 0.15.
#   electrical.edit {"id", "offset"?, "height"?, "side"?, "kind"?}; electrical.delete {"id"}
#   connection {"from": "el1" (a switch), "to": ["lf0", ...], "bulge"?: -1..1} -> {"connection": "cn0", "handle", "from",
#            "to", "bulge"};  connection.edit {"id", "to"?, "bulge"?}; connection.delete {"id"}
#   circuit.brightness {"switch": "el1", "brightness": 0..1} -> {"lights": [...], "brightness"}
#   outlets.auto {"room": "r0", "dryRun"?: bool} -> {"placed": ["el3", ...], "maxGap": m, "kitchen": bool}
#   lightset {"name", "lights": {"lf0": {"on": bool, "brightness"?: 0..1}, ...}} -> {"lightSet": name, "lights": n};
#   lightset {"delete": name}; lightset {"list": true} -> {"lightSets": [{"name", "lights": {...}}]}
#   camera {"save": name, ..., "lightSet"?: name};  render {..., "lightSet"?: name, "style"?: ...|"clay"|"technical"}
#   view.electrical {} -> {"layerState": "Electrical", "shown": [layers], "dimmed": [layers]}
#   level.show {"level": name|index|"all"} -> {"shown": index|null}
#   level {..., "elevation"?: m, "kind"?: "floor"|"landing"};  level.edit {"level", "elevation"?, "ceiling"?, "structure"?,
#            "name"?, "kind"?}
#   wall {..., "level"?: name|index, "topLevel"?: name|index}  (from the level's floor to topLevel's ceiling)
#   terrain.modify {"path": {"kind": "road"|"driveway"|"sidewalk"|"bed", ...}}
#   arch.inspect {"levels": true} rows: {"index", "name", "elevation", "kind", "gradeClass": "aboveGrade"|"basement",
#            "aboveGradeArea"}; the reply's "aboveGradeArea" sums the floor levels classed above grade (landings excluded).
#   arch.inspect {"walls": true} rows gain "level", "topLevel", "bottom", "top" (absolute z);
#   arch.inspect {"cabinets": true} rows gain "counterTop" (absolute z, base cabinets) and "alongWall": [s0, s1];
#   arch.inspect {"electrical": true} -> "electrical": [device rows], "connections": [...],
#            "circuits": [{"switches", "lights", "brightness"}], "outletGaps": [{"room", "maxGap",
#            "counter": null | {"maxSpacing", "startDistance", "endDistance"}}]
#   arch.inspect {"lights": true} rows gain "brightness"; {"lightSets": true} -> "lightSets";
#   furnishing, fixture and light rows gain "room" (the room holding their centre on their level, or null);
#   {"stairs": true} -> "stairs": [{"id", "fromLevel", "toLevel", "rise"}] (level indices);
#   {"site": true} -> "site": {"paths": [{"id", "kind": "road"|"driveway"|"sidewalk"|"bed"}], "plants": [{"id", "species", "form": "tree"|"shrub"|...}]}
#   cadcraft-cli run --commands FILE.json  (a JSON array of {"cmd": id, "params": {...}}, run like --cmd steps)
import math, os, json, tempfile, subprocess
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-14', 'p14lib.inc')).read())

SHOWCASE = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'bilevel.json')
INSPECT15 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "cabinets": True, "fixtures": True,
                                 "furnishings": True, "surfaces": True, "electrical": True, "lights": True,
                                 "lightSets": True, "stairs": True, "site": True})
# The test room: inner faces 0..6 x 0..4 (walls w0 bottom, w1 right, w2 top, w3 left, 0.2 thick, 2.4 high), room r0.
ROOM64 = room_cmds(0, 0, 6, 4)


def rows_of(insp, key):
    v = insp.get(key)
    if not isinstance(v, list):
        fail(f"arch.inspect has no '{key}' list: {sorted(insp)}")
    return v


def device(rows, nth=-1):
    """The reply of an `outlet` / `switch` call."""
    return reply_with(rows, 'device', nth, 'outlet/switch')


def ring_runs(ring, gaps):
    """Usable runs of a closed inner-face ring (counter-clockwise points) with `gaps` = [(edge index, s0, s1)] removed:
    a list of runs, each a list of (point, edge, s) samples every 0.05 m, in order along the ring."""
    pts = []
    for i in range(len(ring)):
        a, b = ring[i], ring[(i + 1) % len(ring)]
        L = math.dist(a, b)
        n = max(1, int(L / 0.05))
        for k in range(n):
            s = L * k / n
            if any(e == i and s0 - 1e-9 <= s <= s1 + 1e-9 for e, s0, s1 in gaps):
                pts.append(None)
                continue
            pts.append(((a[0] + (b[0] - a[0]) * s / L, a[1] + (b[1] - a[1]) * s / L), i, s))
    # Split at the gaps, joining the wrap-around.
    runs, cur = [], []
    for p in pts:
        if p is None:
            if cur:
                runs.append(cur)
            cur = []
        else:
            cur.append(p)
    if cur:
        if runs and pts[0] is not None:
            runs[0] = cur + runs[0]
        else:
            runs.append(cur)
    return runs


def run_gap(run, outlets):
    """The largest distance, measured along the run, from a point of the run to the nearest outlet on the run (an outlet
    belongs to the run if it is within 0.01 m of one of its samples). inf when the run has no outlet."""
    acc, d = [0.0], 0.0
    for k in range(1, len(run)):
        d += math.dist(run[k - 1][0], run[k][0])
        acc.append(d)
    pos = []
    for o in outlets:
        near_k = min(range(len(run)), key=lambda k: math.dist(run[k][0], o))
        if math.dist(run[near_k][0], o) <= 0.03:
            pos.append(acc[near_k])
    if not pos:
        return float('inf')
    return max(min(abs(a - p) for p in pos) for a in acc)


def mean_luma(path):
    w, h, nch, rows = png(path)
    vals = [0.2126 * r[x * nch] + 0.7152 * r[x * nch + 1] + 0.0722 * r[x * nch + 2] for r in rows for x in range(w)]
    return sum(vals) / max(1, len(vals))


def luma_sd(path):
    w, h, nch, rows = png(path)
    vals = [0.2126 * r[x * nch] + 0.7152 * r[x * nch + 1] + 0.0722 * r[x * nch + 2] for r in rows[::2] for x in range(0, w, 2)]
    m = sum(vals) / max(1, len(vals))
    return math.sqrt(sum((v - m) ** 2 for v in vals) / max(1, len(vals)))


def showcase_cmds():
    """The showcase script as --cmd strings (bilevel.json: a JSON array of {"cmd", "params"})."""
    if not os.path.exists(SHOWCASE):
        fail(f"{os.path.relpath(SHOWCASE, ROOT)} is missing")
    steps = json.load(open(SHOWCASE, encoding='utf-8'))
    if not isinstance(steps, list) or not steps:
        fail("bilevel.json is not a non-empty JSON array")
    out = []
    for i, st in enumerate(steps):
        if not (isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict)):
            fail(f"bilevel.json step {i} is not {{\"cmd\", \"params\"}}: {json.dumps(st)[:200]}")
        out.append(cmd(st['cmd'], st.get('params', {})))
    return out
