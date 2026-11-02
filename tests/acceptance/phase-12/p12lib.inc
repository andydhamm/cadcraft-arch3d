# Phase-12 check helpers (exports and interoperability). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())
# It loads the phase-11 helpers (run, cmd, cli, glb, node_box, png, num, near, fail, artifact, …) and adds a STEP
# (ISO 10303-21) parser for the IFC files, the demo house, and the exports.
#
# What the checks drive (lengths in metres):
#   cadcraft-cli run … --export OUT.ifc | OUT.obj | OUT.glb   (by extension; OBJ also writes OUT.mtl beside it)
#   cadcraft-cli convert IN.dxf OUT.ifc | OUT.obj | OUT.glb
#   SOURCE_DATE_EPOCH=<seconds> fixes the IFC header's time stamp (two exports of one model are byte-identical).
# The IFC file: FILE_SCHEMA(('IFC4')); IfcProject -> IfcSite -> IfcBuilding -> one IfcBuildingStorey per level (Elevation =
# the level's); IfcWall (name wall-w<n>), IfcDoor/IfcWindow + IfcOpeningElement + IfcRelVoidsElement + IfcRelFillsElement per
# opening, IfcSlab .FLOOR. per floor, IfcCovering .CEILING., IfcRoof aggregating IfcSlab .ROOF. planes, IfcStair, IfcRailing,
# IfcSpace per room (CADCraft_RoomAreas: InteriorArea, StandardArea), IfcGeographicElement (terrain, plants, paths) in the
# site, IfcLightFixture per fixture; every body an IfcTriangulatedFaceSet. GlobalIds from keys like wall/3 (io::ifc::guid).
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-11', 'p11lib.inc')).read())

EPOCH = 1793556000            # 2026-11-01T18:00:00
CHECK_TOOLS = os.path.join(os.environ.get('XDG_CACHE_HOME') or os.path.expanduser('~/.cache'), 'cadcraft-checks')

# The demo house: two levels (a 10 x 8 m ground floor with a partition, a door and two windows; an upper floor), a stair
# between them, an automatic roof on the upper walls, a railing, four pot lights in the kitchen, a site with its
# location, terrain and two plants.
HOUSE = [cmd('site', {"latitude": 51.05, "longitude": -114.07, "elevation": 1045.0, "trueNorth": 0}),
         cmd('wall', {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.7, "exterior": True}),
         cmd('wall', {"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.7, "exterior": False}),
         cmd('door', {"wall": "w0", "offset": 2.0, "width": 0.9, "height": 2.1, "style": "hinged"}),
         cmd('window', {"wall": "w0", "offset": 6.5, "width": 1.6, "sill": 0.9, "height": 1.2}),
         cmd('window', {"wall": "w2", "offset": 5.0, "width": 1.2, "sill": 0.9, "height": 1.2}),
         'roomdetect {}',
         cmd('level', {"above": 0}),
         cmd('wall', {"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "level": "Level 2", "exterior": True}),
         cmd('roomdetect', {"level": 1}),
         cmd('stair', {"start": [6.5, 4.0], "direction": 0, "width": 1.0, "fromLevel": 0, "toLevel": 1}),
         cmd('roofauto', {"pitch": [6, 12], "overhang": 0.5, "level": "Level 2"}),
         cmd('railing', {"path": [[1, 7.5], [4, 7.5]]}),
         cmd('lightfixture', {"room": "r1", "rows": 2, "cols": 2}),
         cmd('terrain', {"boundary": [[-10, -10], [20, -10], [20, 18], [-10, 18]],
                         "points": [[-10, -10, 0.0], [20, -10, 0.0], [20, 18, 0.0], [-10, 18, 0.0]]}),
         cmd('plant', {"species": "Sugar Maple", "at": [15, 13], "age": 20}),
         cmd('plant', {"species": "Common Lilac", "at": [-4, 2], "age": 4})]

INSPECT = cmd('arch.inspect', {"walls": True, "openings": True, "rooms": True, "levels": True, "stairs": True,
                               "railings": True, "lights": True})


def export(cmds, out, epoch=EPOCH, expect_ok=True):
    """Run `cmds` and export to `out` (by extension) with SOURCE_DATE_EPOCH. Returns (rc, replies, stdout, stderr)."""
    args = cli_args('run') + ['--metric']
    for c in cmds:
        args += ['--cmd', c]
    args += ['--export', out]
    env = dict(os.environ, SOURCE_DATE_EPOCH=str(epoch))
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    p = subprocess.run(args, capture_output=True, text=True, timeout=900, env=env)
    if 'panicked' in p.stderr or 'panicked' in p.stdout:
        fail(f"panic: {(p.stderr + p.stdout)[-800:]}")
    if expect_ok and p.returncode != 0:
        fail(f"cadcraft-cli run --export {os.path.basename(out)} exited {p.returncode}: {(p.stderr or p.stdout)[-800:]}")
    return p.returncode, parse_rows(p.stdout), p.stdout, p.stderr


def model_counts(rows):
    """The model's walls, openings (doors, windows), rooms, levels, stairs, railings and lights from arch.inspect."""
    ins = [r for r in rows if isinstance(r, dict) and 'walls' in r and 'levels' in r]
    if not ins:
        fail(f"no arch.inspect reply with walls and levels: {json.dumps(rows)[-600:]}")
    r = ins[-1]
    ln = lambda k: len(r.get(k) or [])
    ops = r.get('openings') or []
    return {'walls': ln('walls'), 'openings': len(ops), 'doors': sum(1 for o in ops if o.get('kind') == 'door'),
            'windows': sum(1 for o in ops if o.get('kind') == 'window'), 'rooms': ln('rooms'), 'levels': ln('levels'),
            'stairs': ln('stairs'), 'railings': ln('railings'), 'lights': ln('lights'), 'raw': r}


# ---- ISO 10303-21 -----------------------------------------------------------------------------------------------------

class Ref:
    __slots__ = ('n',)

    def __init__(self, n):
        self.n = n

    def __repr__(self):
        return f'#{self.n}'

    def __eq__(self, o):
        return isinstance(o, Ref) and o.n == self.n

    def __hash__(self):
        return hash(('#', self.n))


class Enum(str):
    pass


class Typed:
    __slots__ = ('name', 'value')

    def __init__(self, name, value):
        self.name, self.value = name, value

    def __repr__(self):
        return f'{self.name}({self.value!r})'


DERIVED = object()


def decode_step_string(s):
    """The content of a STEP string as text: \\X2\\…\\X0\\ (UTF-16), \\X4\\…\\X0\\ (UTF-32), \\X\\hh, \\\\ (apostrophes are undoubled
    by the parser)."""
    out, i = [], 0
    while i < len(s):
        if s.startswith('\\X2\\', i):
            j = s.index('\\X0\\', i)
            hx = s[i + 4:j]
            out.append(''.join(chr(int(hx[k:k + 4], 16)) for k in range(0, len(hx), 4)))
            i = j + 4
        elif s.startswith('\\X4\\', i):
            j = s.index('\\X0\\', i)
            hx = s[i + 4:j]
            out.append(''.join(chr(int(hx[k:k + 8], 16)) for k in range(0, len(hx), 8)))
            i = j + 4
        elif s.startswith('\\X\\', i):
            out.append(chr(int(s[i + 3:i + 5], 16)))
            i += 5
        elif s.startswith('\\\\', i):
            out.append('\\')
            i += 2
        else:
            out.append(s[i])
            i += 1
    return ''.join(out)


def _parse_value(t, i):
    c = t[i]
    if c == '$':
        return None, i + 1
    if c == '*':
        return DERIVED, i + 1
    if c == '#':
        j = i + 1
        while t[j].isdigit():
            j += 1
        return Ref(int(t[i + 1:j])), j
    if c == "'":
        j, buf = i + 1, []
        while True:
            k = t.index("'", j)
            buf.append(t[j:k])
            if k + 1 < len(t) and t[k + 1] == "'":
                buf.append("'")
                j = k + 2
            else:
                return ''.join(buf), k + 1
    if c == '.':
        j = t.index('.', i + 1)
        word = t[i + 1:j]
        if word in ('T', 'F'):
            return word == 'T', j + 1
        return Enum(word), j + 1
    if c == '(':
        items, j = [], i + 1
        if t[j] == ')':
            return items, j + 1
        while True:
            v, j = _parse_value(t, j)
            items.append(v)
            if t[j] == ',':
                j += 1
            elif t[j] == ')':
                return items, j + 1
            else:
                raise ValueError(f"expected , or ) at {j}: {t[j:j + 40]!r}")
    m = re.compile(r'[-+]?\d+(\.\d*)?([Ee][-+]?\d+)?').match(t, i)
    if m and m.end() > i:
        s = m.group(0)
        return (float(s) if ('.' in s or 'E' in s or 'e' in s) else int(s)), m.end()
    m = re.compile(r'[A-Z][A-Z0-9_]*').match(t, i)
    if m and m.end() < len(t) and t[m.end()] == '(':
        v, j = _parse_value(t, m.end() + 1)
        if t[j] != ')':
            raise ValueError(f"typed value {m.group(0)} not closed at {j}")
        return Typed(m.group(0), v), j + 1
    raise ValueError(f"cannot read a value at {i}: {t[i:i + 40]!r}")


def step_file(path):
    """(header text, {n: (ENTITY, [args])}) of an ISO 10303-21 file. Fails on anything malformed: the file must be ASCII,
    start with ISO-10303-21; and end with END-ISO-10303-21;, and every DATA statement must be #n=NAME(args);."""
    raw = open(path, 'rb').read()
    try:
        text = raw.decode('ascii')
    except UnicodeDecodeError as e:
        fail(f"{os.path.basename(path)} is not ASCII (ISO 10303-21 encodes everything else): {e}")
    if not text.startswith('ISO-10303-21;') or not text.rstrip().endswith('END-ISO-10303-21;'):
        fail(f"{os.path.basename(path)} must start with ISO-10303-21; and end with END-ISO-10303-21;")
    h = text.find('HEADER;'); d = text.find('DATA;')
    if h < 0 or d < 0:
        fail("no HEADER; or DATA; section")
    header = text[h:d]
    body = text[d + len('DATA;'):text.rfind('ENDSEC;')]
    inst = {}
    i, t = 0, body
    stmt = re.compile(r'\s*#(\d+)\s*=\s*([A-Z][A-Z0-9_]*)\s*\(')
    while True:
        m = stmt.match(t, i)
        if not m:
            if t[i:].strip():
                fail(f"unreadable DATA statement near: {t[i:i + 80]!r}")
            break
        n, name = int(m.group(1)), m.group(2)
        j = m.end() - 1
        try:
            args, j = _parse_value(t, j)
        except (ValueError, IndexError) as e:
            fail(f"#{n}={name}: unreadable parameters: {e}")
        if t[j] != ';':
            fail(f"#{n}={name}: no ; after the parameters")
        if n in inst:
            fail(f"#{n} is defined twice")
        inst[n] = (name, args)
        i = j + 1
    return header, inst


def refs_of(v):
    if isinstance(v, Ref):
        yield v.n
    elif isinstance(v, list):
        for x in v:
            yield from refs_of(x)
    elif isinstance(v, Typed):
        yield from refs_of(v.value)


def of_type(inst, name):
    return [(n, a) for n, (e, a) in sorted(inst.items()) if e == name]


def deref(inst, r):
    if not isinstance(r, Ref) or r.n not in inst:
        fail(f"{r!r} is not a defined instance")
    return inst[r.n]


def ifc_names(inst, name):
    """Name (third attribute) of every instance of an IfcRoot entity, decoded."""
    return [decode_step_string(a[2]) if isinstance(a[2], str) else None for _, a in of_type(inst, name)]


def contained_in(inst):
    """{element #n: structure #n} from IfcRelContainedInSpatialStructure; fails when an element is contained twice."""
    out = {}
    for _, a in of_type(inst, 'IFCRELCONTAINEDINSPATIALSTRUCTURE'):
        for r in a[4]:
            if r.n in out:
                fail(f"#{r.n} is contained in two spatial structures")
            out[r.n] = a[5].n
    return out


def aggregates(inst):
    """{whole #n: [part #n]} from IfcRelAggregates."""
    out = {}
    for _, a in of_type(inst, 'IFCRELAGGREGATES'):
        out.setdefault(a[4].n, []).extend(r.n for r in a[5])
    return out


def face_set_points(inst, product_args):
    """The points of a product's body (its IfcProductDefinitionShape -> 'Body' IfcShapeRepresentation ->
    IfcTriangulatedFaceSet -> IfcCartesianPointList3D) and its 1-based triangles; fails when there is none."""
    shape = product_args[6]
    if not isinstance(shape, Ref):
        fail(f"product {decode_step_string(product_args[2] or '')!r} has no body")
    _, pds = deref(inst, shape)
    pts, tris = [], []
    for rep in pds[2]:
        _, sr = deref(inst, rep)
        if sr[1] != 'Body':
            continue
        for item in sr[3]:
            e, fs = deref(inst, item)
            if e != 'IFCTRIANGULATEDFACESET':
                fail(f"a body item is {e}, not IFCTRIANGULATEDFACESET")
            _, pl = deref(inst, fs[0])
            base = len(pts)
            pts.extend(pl[0])
            tris.extend([[base + k for k in t] for t in fs[3]])
    if not tris:
        fail(f"product {decode_step_string(product_args[2] or '')!r} has an empty body")
    return pts, tris


def compound_degrees(v):
    """An IfcCompoundPlaneAngleMeasure (degrees, minutes, seconds[, millionths]) as decimal degrees."""
    if not isinstance(v, list) or len(v) < 3:
        fail(f"not a compound plane angle: {v!r}")
    d = v[0] + v[1] / 60 + v[2] / 3600 + (v[3] if len(v) > 3 else 0) / 3.6e9
    return d
