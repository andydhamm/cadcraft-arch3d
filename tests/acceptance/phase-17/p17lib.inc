# Phase-17 check helpers (stairs, decks, railings and openings in detail). A fixture, not a check (the gate runs only
# *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())
# It loads the phase-16 helpers, which load phases 15..8 (run, run_ok, made, cmd, cli_args, parse_rows, inspected,
# objects_named, by_id, reply_with, near, num, fail, errors, probes, part, expect_hits, parts_of, build3d_of, room_row,
# level_row, house_rooms, HOUSE, GABLE, glb, node_box, node_meshes, node_names, material_of_node, texture_image_name,
# _acc, validate_glb, step_file, of_type, ifc_names, decode_step_string, face_set_points, dxf_text, entities, values,
# png, luma_sd, foreground, artifact, digest, ROOT).
#
# What the checks drive (lengths in metres, angles in degrees; ids as the replies and arch.inspect give them, never
# hard-coded):
#   stair {..., openRisers?: bool, stringers?: "two" | "centre" | "none", underside?: 0.25..1.0 (the flight's underside
#          below the line through the nosings, measured vertically, default 0.3), railLeft?/railRight?: "auto" | "on" |
#          "off", planCut?: 0.3..3.0 (the plan cut height above the from-level floor, default 1.2)}
#        -> the phase-5 reply plus "openRisers", "stringers", "underside", "railLeft", "railRight", "planCut" and
#           "railings": [labels of the stair's automatic side railings].
#   stair.edit {"id": "s0", openRisers?, stringers?, underside?, railLeft?, railRight?, planCut?} (at least one key; one
#          undo step; the stair never moves) -> {"stair", the six values, "railings"}.
#   deck {..., boards?: degrees (|boards| <= 360, stored in [0, 180); null = automatic, parallel to the ledger),
#          rails?: "auto" | "on" | "off" | [one per edge of the stored ring, edge i from vertex i to i+1]}
#        -> the phase-5 reply plus "boards", "rails", "railings".
#   deck.edit {"id": "r3", boards?: deg | null, rails?, height?} (a Deck room only) -> {"deck", "deckHeight", "boards",
#          "rails", "railings"}.
#   railing {..., postSpacing?: 0.3..3.0 (default 1.8), inset?: |inset| <= 0.5 (horizontal, positive = left of travel)}
#        -> the phase-5 reply plus "postSpacing", "inset", "posts": [[x, y], ...].
#   window / door / garagedoor {..., grille?: {"across": 1..12, "down": 1..12, "part"?: "whole" | "top"} | null,
#          top?: "square" | "round" | "segment" | "roof" (windows only), topRise?: m (segment), roofOffset?: 0..1
#          (default 0.15), sillSides?/casingSides?: "left" | "right" | "both" | "none" (default right)}; door style
#          "fixed" (a glazed leaf that does not open). Each reply entry gains "top", "grille", "sillSides", "casingSides".
#   arch.inspect {"stairs": true} rows gain openRisers, stringers, underside, railLeft, railRight, planCut.
#   arch.inspect {"railings": true} rows gain postSpacing, inset, host ("well" | "stairLeft" | "stairRight" | "deck" |
#          null for a drawn railing), auto, posts [[x, y], ...]; "follows" is the stair label for the well guard and the
#          stair sides, the deck room's label for a deck railing, null for a drawn railing.
#   arch.inspect {"rooms": true}: a deck room's row gains boards (degrees or null) and rails ([names], [] = all auto).
#   arch.inspect {"openings": true} rows gain top, topRise, roofOffset, grille ({across, down, part} | null), sillSides,
#          casingSides, glassCentre [x, y, z] | null, glassNormal [nx, ny] | null (the glass plane).
#   arch.inspect {"decks": true} -> "decks": [{"id", "level", "top", "boardDirection", "ledger": {"edge", "wall", "line"} |
#          null, "joists": [[[x,y],[x,y]], ...], "rims", "beams", "posts": [[x, y], ...], "postGrades": [z, ...]}].
#   build3d parts: tread, riser, stringer, skirt, stair (the landing), post (railing and deck posts), railing (rails,
#          panels, balusters), joist, rim, ledger, beam, footing (deck piers and phase-16 footings), deck (the boards),
#          grille, sill, casing.
#   Materials / GLB nodes: stair-tread/-riser/-stringer/-skirt/-landing -> one node stair-s<n>; railing-post/-newel/
#          -rail/-panel/-baluster -> one node railing-ra<n>; deck (boards) deck-r<n>; deck-ledger/-joist/-rim/-beam/-post/
#          -footing -> deck-r<n>-ledger/-joists/-rims/-beams/-posts/-piers (one node per kind); window-grille-o<n>.
#   IFC: IfcMember (.MEMBER.) per deck ledger, joist, rim and beam, IfcColumn (.COLUMN.) per deck post, IfcFooting
#          (.PAD_FOOTING.) per pier, IfcRailing .HANDRAIL. for stair sides and well guards, .GUARDRAIL. for deck and
#          drawn railings.
#   DXF: the stair plan symbol as LWPOLYLINE / TEXT proxies on the stair's layer with xdata 1001 CADCRAFT, 1000
#          "CADCRAFT-STAIR", 1000 <role> (nosing | break | side | arrow | well | label); a dashed line carries group 6
#          "DASHED". `export {"path", "level": index | name}` writes the plan of that level.
#
# Fixtures: HOUSE is p16lib's phase-1 house (w0 south (0,0)->(10,0), w1 east, w2 north (10,8)->(0,8), w3 west
# (0,8)->(0,0), 0.2 thick, 2.4 high; w4 along y = 3 and w5 along x = 6, 0.1 thick; rooms S, NW, NE after roomdetect;
# level 0 at 0, ceiling 2.4, structure 0.3). HOUSE2 adds Level 2 (index 1) at 2.7. STAIR5 is the phase-5 stair: start
# (2,1) heading +x, 1.0 wide, 16 risers of 0.16875 m, 15 treads of 0.254 m, run 3.81, rise 2.7; its flight spans
# x 1.975 (the first nosing, 25 mm before start) .. 5.81 and y 0.5 .. 1.5 (centred on start). The line through the
# nosings is nosing_v(u) = RH + (u + NOSING) * RH / TD with u measured along the flight from start.
import math, os, json
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-16', 'p16lib.inc')).read())

RH, TD, NOSING, RISE, RUN = 0.16875, 0.254, 0.025, 2.7, 3.81
LEVEL2 = cmd('level', {"above": 0})
HOUSE2 = HOUSE + [LEVEL2]
STAIR_MARKER = 'CADCRAFT-STAIR'


def nosing_v(u):
    """Height of the line through the tread nosings at distance u along the phase-5 flight."""
    return RH + (u + NOSING) * RH / TD


def stair_cmd(**extra):
    """The phase-5 stair (start (2,1), +x, 1.0 wide, level 0 -> 1) with extra parameters."""
    p = {"start": [2, 1], "direction": 0, "width": 1.0, "fromLevel": 0, "toLevel": 1}
    p.update(extra)
    return cmd('stair', p)


STAIR5 = stair_cmd()
NO_RAILS = {"railLeft": "off", "railRight": "off"}
DECK_RING = [[2, 8.1], [6, 8.1], [6, 11.1], [2, 11.1], [2, 8.1]]


def deck_cmd(points, **extra):
    p = {"points": points, "height": 0.9}
    p.update(extra)
    return cmd('deck', p)


NORTH_DECK = deck_cmd(DECK_RING)     # against the north wall's outer face y = 8.1, 0.9 m above grade


def free_deck(x0, **extra):
    """A free-standing 4 x 3 deck at x0..x0+4, y 0..3, 0.9 m above grade (touches no wall)."""
    return deck_cmd([[x0, 0], [x0 + 4, 0], [x0 + 4, 3], [x0, 3], [x0, 0]], **extra)


def window4(**extra):
    """The phase-4 window: 0.9 x 1.2, sill 0.9, centred at x = 4 on the north wall w2 (offset 6 from (10,8))."""
    p = {"wall": "w2", "offset": 6.0, "width": 0.9, "height": 1.2, "sill": 0.9}
    p.update(extra)
    return cmd('window', p)


WINDOW4 = window4()
GRILLE22 = window4(grille={"across": 2, "down": 2})
INSPECT17 = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "openings": True, "stairs": True,
                                 "railings": True, "roofs": True, "furnishings": True, "decks": True})
INSPECT17_SYM = cmd('arch.inspect', {"walls": True, "rooms": True, "levels": True, "openings": True, "stairs": True,
                                     "railings": True, "symbols": True, "decks": True})


def stairs_of(insp):
    return objects_named(insp, 'stairs', 'arch.inspect stairs')


def railings_of(insp):
    return objects_named(insp, 'railings', 'arch.inspect railings')


def openings_of(insp):
    return objects_named(insp, 'openings', 'arch.inspect openings')


def decks_of(insp):
    return objects_named(insp, 'decks', 'arch.inspect {"decks": true}')


def deck_rooms(insp):
    return [r for r in objects_named(insp, 'rooms', 'rooms') if r.get('type') == 'deck']


def deck_room(insp):
    rows = deck_rooms(insp)
    if len(rows) != 1:
        fail(f"one deck room expected, got {[(r.get('id'), r.get('type')) for r in rows]}")
    return rows[0]


def railings_by_host(insp, host):
    """The railing rows whose host is `host` ("well", "stairLeft", "stairRight", "deck"); host None = drawn railings."""
    return [r for r in railings_of(insp) if r.get('host') == host]


def one_railing(insp, host, follows=None, what=None):
    rows = [r for r in railings_by_host(insp, host) if follows is None or r.get('follows') == follows]
    if len(rows) != 1:
        fail(f"{what or f'the {host} railing'}: one row with host {host!r}{f' following {follows}' if follows else ''} expected; "
             f"railings: {[(r.get('id'), r.get('host'), r.get('follows')) for r in railings_of(insp)]}")
    return rows[0]


def label_of(rows, key, what):
    """The 'id' of the last reply carrying `key` (a stair reply has 'risers', a deck reply 'deckHeight', a railing
    reply 'balusterGap', an opening reply 'openings')."""
    r = reply_with(rows, key, -1, what)
    if key == 'openings':
        ops = r['openings']
        if not (isinstance(ops, list) and ops and isinstance(ops[0].get('id'), str)):
            fail(f"{what}: the opening reply names its opening: {json.dumps(r)[:300]}")
        return ops[0]['id']
    if not isinstance(r.get('id'), str):
        fail(f"{what}: the reply has no 'id': {json.dumps(r)[:300]}")
    return r['id']


def same_points(got, want, tol, what):
    """`got` is a list of [x, y] equal to `want` in order, within tol."""
    if not isinstance(got, list) or len(got) != len(want):
        fail(f"{what}: {len(want)} points expected, got {got!r}")
    for k, (g, w) in enumerate(zip(got, want)):
        if not (isinstance(g, list) and len(g) >= 2):
            fail(f"{what}: point {k} is not [x, y]: {g!r}")
        if abs(num(g[0], f'{what}[{k}].x') - w[0]) > tol or abs(num(g[1], f'{what}[{k}].y') - w[1]) > tol:
            fail(f"{what}: point {k} is {g[:2]}, expected {w} (tol {tol})")


def expect_walls(rows, cases, what, nth=-1):
    """cases: [([x, y, z], True | False)]: True = the probe hits part wall, False = anything but a wall (or nothing)."""
    got = probed(rows, [p for p, _ in cases], nth, what)
    for (p, want_wall), g in zip(cases, got):
        is_wall = isinstance(g, dict) and part(g) == 'wall'
        if want_wall and not is_wall:
            fail(f"{what}: {p} is inside the wall, the probe answered {g}")
        if not want_wall and is_wall:
            fail(f"{what}: {p} is not wall (an opening cut it), the probe answered {g}")


def glass_frame(row, what):
    """(centre [x, y, z], along [ax, ay], normal [nx, ny]) of an opening row's glass plane."""
    c, n = row.get('glassCentre'), row.get('glassNormal')
    if not (isinstance(c, list) and len(c) == 3 and isinstance(n, list) and len(n) == 2):
        fail(f"{what}: the opening row reports glassCentre [x, y, z] and glassNormal [nx, ny]: {c!r}, {n!r}")
    c = [num(v, f'{what} glassCentre') for v in c]
    n = [num(v, f'{what} glassNormal') for v in n]
    if abs(math.hypot(n[0], n[1]) - 1.0) > 1e-6:
        fail(f"{what}: glassNormal is a unit vector: {n}")
    return c, [-n[1], n[0]], n


def at(c, along, da, dz):
    """A point on the glass plane: the centre moved `da` along the wall and `dz` up."""
    return [c[0] + along[0] * da, c[1] + along[1] * da, c[2] + dz]


def stair_proxies(path):
    """Every DXF entity whose CADCRAFT xdata carries the CADCRAFT-STAIR marker:
    {"kind", "layer", "role", "dashed", "points": [[x, y], ...], "text"}."""
    out = []
    for kind, layer, pairs in entities(dxf_text(path)):
        strings = [v.strip() for c, v in pairs if c == '1000']
        if STAIR_MARKER not in strings:
            continue
        i = strings.index(STAIR_MARKER)
        role = strings[i + 1] if i + 1 < len(strings) else ''
        xs = [float(v) for c, v in pairs if c == '10']
        ys = [float(v) for c, v in pairs if c == '20']
        dashed = any(v.strip().upper() == 'DASHED' for c, v in pairs if c == '6')
        text = next((v.strip() for c, v in pairs if c == '1'), '')
        out.append({"kind": kind, "layer": layer, "role": role, "dashed": dashed, "points": [[x, y] for x, y in zip(xs, ys)],
                    "text": text})
    return out


def nosings(prox, dashed):
    return [p for p in prox if p['role'] == 'nosing' and p['dashed'] is dashed]


def labels(prox):
    return [p['text'] for p in prox if p['role'] == 'label']


def line_x(p, what):
    """The x of a nosing line drawn across a +x flight (both ends share it)."""
    xs = [q[0] for q in p['points']]
    if len(xs) < 2 or max(xs) - min(xs) > 1e-6:
        fail(f"{what}: a nosing line across a +x flight has one x: {p['points']}")
    return xs[0]


def replay_file(path, extra, what, timeout=1800):
    """`cadcraft-cli run --metric --commands path`, then the `extra` --cmd steps; a refusal or a panic fails."""
    args = cli_args('run') + ['--metric', '--commands', path]
    for c in extra:
        args += ['--cmd', c]
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    p = subprocess.run(args, capture_output=True, text=True, timeout=timeout, env=env)
    if 'panicked' in p.stderr or 'panicked' in p.stdout:
        fail(f"panic replaying {os.path.relpath(path, ROOT)} ({what}): {(p.stderr + p.stdout)[-800:]}")
    if p.returncode != 0 or errors(p.stderr):
        fail(f"{os.path.relpath(path, ROOT)} replays without an error ({what}; exit {p.returncode}): "
             f"{errors(p.stderr)[:3] or p.stderr[-400:]}")
    return parse_rows(p.stdout)


def top_face_u_gradient(j, binc, name, z_top, what):
    """For the GLB node `name`: the plan gradient (du/dx, du/dy) of TEXCOORD_0's u over its upward-facing triangles at
    z_top (every vertex normal z > 0.999), the same on every such triangle within 1e-3."""
    grads = []
    for m in node_meshes(j, name):
        for prim in m['primitives']:
            a = prim.get('attributes', {})
            if 'TEXCOORD_0' not in a or 'NORMAL' not in a:
                fail(f"{what}: node {name} has NORMAL and TEXCOORD_0: {sorted(a)}")
            pos, nrm, uv = _acc(j, binc, a['POSITION']), _acc(j, binc, a['NORMAL']), _acc(j, binc, a['TEXCOORD_0'])
            idx = _acc(j, binc, prim['indices']) if 'indices' in prim else [(k,) for k in range(len(pos))]
            idx = [t[0] for t in idx]
            for k in range(0, len(idx) - 2, 3):
                tri = idx[k:k + 3]
                if not all(nrm[i][2] > 0.999 and abs(pos[i][2] - z_top) < 1e-3 for i in tri):
                    continue
                (x0, y0), (x1, y1), (x2, y2) = [(pos[i][0], pos[i][1]) for i in tri]
                u0, u1, u2 = [uv[i][0] for i in tri]
                det = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)
                if abs(det) < 1e-9:
                    continue
                gx = ((u1 - u0) * (y2 - y0) - (u2 - u0) * (y1 - y0)) / det
                gy = ((u2 - u0) * (x1 - x0) - (u1 - u0) * (x2 - x0)) / det
                grads.append((gx, gy))
    if not grads:
        fail(f"{what}: node {name} has no upward-facing triangles at z {z_top}")
    g0 = grads[0]
    for g in grads[1:]:
        if abs(g[0] - g0[0]) > 1e-3 or abs(g[1] - g0[1]) > 1e-3:
            fail(f"{what}: the top face's u gradient differs between triangles: {g0} vs {g}")
    return g0
