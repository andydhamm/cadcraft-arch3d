#!/usr/bin/env python3
"""Render the showcase house in every view style (see README.md). Python 3 standard library only."""
import argparse, hashlib, json, os, shlex, struct, subprocess, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SCRIPT = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'bilevel.json')
SET_SCRIPT_NAMES = ['phase-20-showcase.json', 'phase-21-showcase.json']
SET_SCRIPTS = [os.path.join(ROOT, 'docs', 'arch3d', 'showcase', n) for n in SET_SCRIPT_NAMES]
CAMERAS = ['Front', 'Back', 'Kitchen', 'Living']
LOOK_CAMERAS = ['dusk', 'noon-hdri', 'overview']
# Kinds that run on set.dxf (the drawing set) rather than bilevel.dxf.
SET_KINDS = ('planview', 'rcp', 'roofplan', 'siteplan', 'interior', 'sheet', 'drawingset')
PLAN_LEVEL_KINDS = ('floor', 'landing')
ELECTRICAL_LEVELS = ['Lower', 'Main']
TILE_W, BAND = 320, 12


class Step(Exception):
    """A failed step: its name and the reason."""

    def __init__(self, step, why):
        super().__init__(f"{step}: {why}")
        self.step, self.why = step, why


def cli_base(cli_json):
    exe = os.environ.get('CADCRAFT_CLI')
    if exe:
        return [exe]
    if cli_json:
        try:
            v = json.loads(cli_json)
        except ValueError as e:
            raise SystemExit(f"--cli-json is not JSON: {e}")
        if not (isinstance(v, list) and v and all(isinstance(a, str) for a in v)):
            raise SystemExit("--cli-json must be a non-empty JSON array of strings")
        return v
    return ['cargo', 'run', '-q', '-p', 'cadcraft-cli', '--']


def cmd(name, params):
    return f'{name} {json.dumps(params)}'


class Runner:
    def __init__(self, base, dxf):
        self.base, self.dxf = base, dxf

    def run(self, step, cmds, file=True, extra=(), allow_refusal=False):
        """One cadcraft-cli run. Returns (replies, error_lines); a refusal fails the step unless allowed."""
        args = self.base + ['run'] + ([self.dxf] if file else ['--metric'])
        for c in cmds:
            args += ['--cmd', c]
        args += list(extra)
        env = dict(os.environ)
        env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
        try:
            p = subprocess.run(args, capture_output=True, text=True, env=env, cwd=ROOT)
        except OSError as e:
            raise Step(step, f"can't start {shlex.join(args[:3])}: {e}")
        errs = [l for l in p.stderr.splitlines() if l.startswith('error: ')]
        if 'panicked' in p.stderr or 'panicked' in p.stdout:
            raise Step(step, 'panic: ' + (p.stderr + p.stdout)[-600:])
        if p.returncode != 0 or (errs and not allow_refusal):
            raise Step(step, f"exit {p.returncode}: " + (' | '.join(errs[:3]) or p.stderr[-600:] or p.stdout[-300:]))
        rows = []
        for line in p.stdout.splitlines():
            line = line.strip()
            if line[:1] in ('{', '['):
                try:
                    rows.append(json.loads(line))
                except ValueError:
                    pass
        return rows, errs


MAX_NAME = 80


def slug(s):
    """A file-name-safe lower-case form of `s`: ASCII letters and digits, every other character a hyphen, at most 80 characters."""
    t = ''.join(c if (c.isascii() and c.isalnum()) else '-' for c in str(s).lower()).strip('-')[:MAX_NAME].strip('-')
    return t or 'x'


def file_name(prefix, name, suffix):
    """`<prefix>-<slug of name><suffix>`, at most 80 characters, with no separator and no leading dot."""
    room = max(1, MAX_NAME - len(prefix) - 1 - len(suffix))
    return f'{prefix}-{slug(name)[:room].strip("-") or "x"}{suffix}'


# ---- PNG decode / encode ----

def read_png(path):
    """(width, height, channels, rows of bytes) of an 8-bit non-interlaced grey/RGB/RGBA PNG."""
    with open(path, 'rb') as f:
        b = f.read()
    if b[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError(f'{path} is not a PNG')
    i, idat, w, h, ct = 8, b'', 0, 0, 6
    while i + 8 <= len(b):
        ln, typ = struct.unpack_from('>I4s', b, i)
        body = b[i + 8:i + 8 + ln]
        i += 12 + ln
        if typ == b'IHDR':
            w, h, depth, ct, _, _, il = struct.unpack('>IIBBBBB', body)
            if depth != 8 or il != 0 or ct not in (0, 2, 4, 6):
                raise ValueError(f'{path}: unsupported PNG (need 8-bit, non-interlaced grey/RGB/RGBA)')
        elif typ == b'IDAT':
            idat += body
        elif typ == b'IEND':
            break
    nch = {0: 1, 2: 3, 4: 2, 6: 4}[ct]
    raw = zlib.decompress(idat)
    stride = w * nch
    if w < 1 or h < 1 or len(raw) < h * (stride + 1):
        raise ValueError(f'{path}: truncated PNG')
    rows, prev = [], bytearray(stride)
    for y in range(h):
        o = y * (stride + 1)
        ft, cur = raw[o], bytearray(raw[o + 1:o + 1 + stride])
        for x in range(stride):
            a = cur[x - nch] if x >= nch else 0
            up = prev[x]
            c = prev[x - nch] if x >= nch else 0
            if ft == 1:
                cur[x] = (cur[x] + a) & 255
            elif ft == 2:
                cur[x] = (cur[x] + up) & 255
            elif ft == 3:
                cur[x] = (cur[x] + (a + up) // 2) & 255
            elif ft == 4:
                pa, pb, pc = abs(up - c), abs(a - c), abs(a + up - 2 * c)
                pr = a if pa <= pb and pa <= pc else (up if pb <= pc else c)
                cur[x] = (cur[x] + pr) & 255
            elif ft != 0:
                raise ValueError(f'{path}: bad PNG filter')
        rows.append(cur)
        prev = cur
    return w, h, nch, rows


def to_rgb(w, nch, row):
    """One decoded row as flat RGB bytes."""
    out = bytearray(w * 3)
    for x in range(w):
        if nch >= 3:
            out[x * 3:x * 3 + 3] = row[x * nch:x * nch + 3]
        else:
            out[x * 3:x * 3 + 3] = bytes([row[x * nch]]) * 3
    return out


def write_png(path, w, h, rows):
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    raw = b''.join(b'\x00' + bytes(r) for r in rows)
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))


def contact_sheet(out_dir, files, name='contact-sheet.png'):
    """A grid of every image scaled to 320 px wide, each tile under a 12 px dark band (the file name's slot)."""
    dark = bytes([24, 24, 28])
    tiles = []
    for f in files:
        w, h, nch, rows = read_png(os.path.join(out_dir, f))
        th = max(1, (h * TILE_W) // w)
        scaled = []
        for ty in range(th):
            src = to_rgb(w, nch, rows[min(h - 1, ty * h // th)])
            r = bytearray(TILE_W * 3)
            for tx in range(TILE_W):
                sx = min(w - 1, tx * w // TILE_W)
                r[tx * 3:tx * 3 + 3] = src[sx * 3:sx * 3 + 3]
            scaled.append(r)
        tiles.append(scaled)
    if not tiles:
        raise ValueError('no images')
    cols = min(4, len(tiles))
    sheet = []
    for r0 in range(0, len(tiles), cols):
        group = tiles[r0:r0 + cols]
        cell_h = BAND + max(len(t) for t in group)
        for y in range(cell_h):
            row = bytearray()
            for t in group:
                if BAND <= y and y - BAND < len(t):
                    row += t[y - BAND]
                else:
                    row += dark * TILE_W
            row += dark * (TILE_W * (cols - len(group)))
            sheet.append(row)
    write_png(os.path.join(out_dir, name), TILE_W * cols, len(sheet), sheet)


# ---- the plan: every step, in a fixed order ----

def _level_rows(levels):
    """(name, kind) for each level; a bare string is a floor."""
    out = []
    for lv in levels:
        if isinstance(lv, str):
            out.append((lv, 'floor'))
        elif isinstance(lv, dict) and isinstance(lv.get('name'), str):
            out.append((lv['name'], lv.get('kind') or 'floor'))
    return out


def plan(levels, views, sheets, with_set, size=(960, 720), spp=64):
    """The gallery as rows of (file, kind, view, commands), in a fixed order. Pure: it runs nothing.

    `levels` are names or {name, kind} rows, `views` {name, id, kind} rows, `sheets` {number, layout} rows.
    A command is (name, params), where a `path` or `out` param is a bare file name that `main` puts under
    the output directory. A `replay` row's commands are cadcraft-cli `run` arguments instead. Rows of
    kinds in SET_KINDS run on set.dxf, the others on bilevel.dxf; `drawingset` is the PDF, not an image.
    """
    size = list(size)
    rows, seen = [], set()

    def add(file, kind, view, commands):
        stem, _, ext = file.rpartition('.')
        n = 2
        while file in seen:
            file = f'{stem}-{n}.{ext}'
            n += 1
        seen.add(file)
        rows.append((file, kind, view, commands))

    lv_rows = _level_rows(levels)
    vws = [v for v in views if isinstance(v, dict) and isinstance(v.get('name'), str) and 'id' in v]
    by_name = {v['name']: v['id'] for v in vws}
    for name, _ in lv_rows:
        f = file_name('plan', name, '.png')
        add(f, 'plan', name, [('export', {'path': f, 'level': name})])
    for name in ELECTRICAL_LEVELS:
        f = file_name('electrical', name, '.png')
        add(f, 'electrical', name, [('view.electrical', {}), ('export', {'path': f, 'level': name})])
    for kind, names in (('elevation', ['South', 'North', 'East', 'West']), ('section', ['Section A', 'Section B'])):
        for n in names:
            f = file_name(kind, n.replace('Section ', ''), '.png')
            add(f, kind, n, [('export', {'path': f, 'view': by_name.get(n, f'<missing view {n}>')})])
    for c in CAMERAS:
        f = file_name('camera', c, '.png')
        add(f, 'camera', c, [('render', {'out': f, 'size': size, 'camera': c})])
    add('night.png', 'night', 'Front', [('render', {'out': 'night.png', 'size': size, 'sun': False, 'lightSet': 'Evening', 'camera': 'Front'})])
    add('path.png', 'path', 'default', [('render', {'out': 'path.png', 'size': size, 'final': 'path', 'spp': spp})])
    add('clay.png', 'clay', 'default', [('render', {'out': 'clay.png', 'size': size, 'style': 'clay'})])
    add('technical.png', 'technical', 'default', [('render', {'out': 'technical.png', 'size': size, 'style': 'technical'})])
    for k in ('glb', 'ifc', 'obj'):
        add(f'showcase.{k}', 'export', k, [('export', {'path': f'showcase.{k}'})])
    for c in LOOK_CAMERAS:
        f = file_name('camera', c, '.png')
        params = {'camera': c, 'size': size, 'out': f}
        if c == 'noon-hdri':
            params['spp'] = spp  # its technique path-traces by itself
        add(f, 'look', c, [('render', params)])
    if not with_set:
        return rows
    replay = ['--commands', SCRIPT]
    for sp in SET_SCRIPTS:
        replay += ['--commands', sp]
    add('set.dxf', 'replay', 'set', replay + ['--save', 'set.dxf'])
    for name, kind in lv_rows:
        if kind not in PLAN_LEVEL_KINDS:
            continue
        for suffix, k, label in (('floor', 'planview', 'Floor Plan'), ('rcp', 'rcp', 'Reflected Ceiling')):
            f = file_name('planview', f'{name}-{suffix}', '.png')
            add(f, k, f'{name} {label}', [('export', {'path': f, 'planView': f'{name} {label}'})])
    add('planview-roof.png', 'roofplan', 'Roof Plan', [('export', {'path': 'planview-roof.png', 'planView': 'Roof Plan'})])
    add('planview-site.png', 'siteplan', 'Site', [('export', {'path': 'planview-site.png', 'planView': 'Site'})])
    for v in vws:
        if v.get('kind') == 'interior':
            f = file_name('interior', v['name'], '.png')
            add(f, 'interior', v['name'], [('export', {'path': f, 'view': v['id']})])
    add('drawing-set.pdf', 'drawingset', 'all', [('plot', {'sheets': 'all', 'path': 'drawing-set.pdf'})])
    for sh in sheets:
        if isinstance(sh, dict) and isinstance(sh.get('number'), str) and isinstance(sh.get('layout'), str):
            f = file_name('sheet', sh['number'], '.png')
            add(f, 'sheet', sh['number'], [('plot', {'layout': sh['layout'], 'device': 'CADCraft PNG', 'path': f})])
    return rows


def manifest(images, exports, skipped, script_sha256, set_scripts, drawing_set, sheet_images=True):
    """The manifest.json content."""
    return {'images': images, 'exports': exports, 'contactSheet': 'contact-sheet.png', 'technicalSkipped': skipped,
            'scriptSha256': script_sha256, 'setScripts': set_scripts, 'drawingSet': drawing_set, 'sheetImages': sheet_images}


def parse_args(argv):
    ap = argparse.ArgumentParser(description='Render the showcase house in every view style.')
    ap.add_argument('--out', required=True)
    ap.add_argument('--size', default='960x720')
    ap.add_argument('--spp', type=int, default=64)
    ap.add_argument('--cli-json', default=None)
    ap.add_argument('--no-exports', action='store_true', help='skip GLB/IFC/OBJ export steps and leave exports empty')
    ap.add_argument('--no-set', action='store_true', help='skip the drawing set: plan views, interior elevations, sheets and the PDF')
    return ap.parse_args(argv)


# ---- the pipeline ----

def sha256_file(path):
    try:
        with open(path, 'rb') as f:
            return hashlib.sha256(f.read()).hexdigest()
    except OSError as e:
        raise Step('script', f"can't read {path}: {e}")


def inspect_rows(r, params):
    rows, _ = r.run('inspect', [cmd('arch.inspect', params)])
    return next((x for x in rows if isinstance(x, dict) and 'levels' in x), {})


def main(argv):
    a = parse_args(argv)
    try:
        w, h = (int(v) for v in a.size.lower().split('x'))
        if not (1 <= w <= 16384 and 1 <= h <= 16384) or not (1 <= a.spp <= 65536):
            raise ValueError
    except ValueError:
        print("render_all: --size must be WxH (1..16384) and --spp 1..65536", file=sys.stderr)
        return 2
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    dxf = os.path.join(out, 'bilevel.dxf')
    set_dxf = os.path.join(out, 'set.dxf')
    r = Runner(cli_base(a.cli_json), dxf)
    rs = Runner(r.base, set_dxf)
    images, exports, skipped, drawing_set = [], {}, False, None

    def p(f):
        return os.path.join(out, f)

    def resolve(c):
        name, params = c
        return cmd(name, {k: (p(v) if k in ('path', 'out') and isinstance(v, str) else v) for k, v in params.items()})

    try:
        script_sha256 = sha256_file(SCRIPT)
        set_scripts = {} if a.no_set else {n: sha256_file(f) for n, f in zip(SET_SCRIPT_NAMES, SET_SCRIPTS)}
        r.run('replay', [], file=False, extra=['--commands', SCRIPT, '--save', dxf])
        if not os.path.exists(dxf):
            raise Step('replay', 'bilevel.dxf was not written')
        insp = inspect_rows(r, {"levels": True, "views": True, "cameras": True})
        if not a.no_set:
            # The set replays the house, then both set scripts, into set.dxf; the rest runs on it.
            rep = next((row for row in plan([], [], [], True) if row[1] == 'replay'), None)
            if rep is None:
                raise Step('replay set', 'the plan has no replay step')
            args = [p(x) if i and rep[3][i - 1] == '--save' else x for i, x in enumerate(rep[3])]
            rs.run('replay set', [], file=False, extra=args)
            if not os.path.exists(set_dxf):
                raise Step('replay set', 'set.dxf was not written')
            insp = inspect_rows(rs, {"planViews": True, "views": True, "sheets": True, "levels": True, "cameras": True})
        levels = [l for l in insp.get('levels', []) if isinstance(l, dict) and isinstance(l.get('name'), str)]
        views = [v for v in insp.get('views', []) if isinstance(v, dict) and 'name' in v and 'id' in v]
        sheets = [s for s in insp.get('sheets', []) if isinstance(s, dict)]
        cams = {c.get('name') for c in insp.get('cameras', []) if isinstance(c, dict)}
        pv_names = {v.get('name') for v in insp.get('planViews', []) if isinstance(v, dict)}
        if not levels:
            raise Step('inspect', 'the replayed drawing has no levels')
        names = {v['name'] for v in views}
        for lv in ELECTRICAL_LEVELS:
            if lv not in {l['name'] for l in levels}:
                raise Step(f'electrical {lv}', 'no such level')
        for file, kind, view, commands in plan(levels, views, sheets, not a.no_set, size=(w, h), spp=a.spp):
            if kind == 'replay':
                continue
            if kind in ('elevation', 'section') and view not in names:
                raise Step(f'{kind} {view}', 'the drawing has no such view')
            if kind == 'look' and view not in cams:
                raise Step(f'camera {view}', 'the drawing has no such camera')
            if kind in ('planview', 'rcp', 'roofplan', 'siteplan') and view not in pv_names:
                raise Step(f'{kind} {view}', 'the drawing has no such plan view')
            if kind == 'export':
                if a.no_exports:
                    continue
                r.run(f'export {view}', [resolve(c) for c in commands])
                if not os.path.exists(p(file)):
                    raise Step(f'export {view}', f'{file} was not written')
                exports[view] = file
                continue
            runner = rs if kind in SET_KINDS else r
            _, errs = runner.run(f'{kind} {view}', [resolve(c) for c in commands], allow_refusal=(kind == 'technical'))
            if kind == 'technical' and errs:
                if not any('not available yet' in e for e in errs):
                    raise Step('technical', ' | '.join(errs[:3]))
                skipped = True
                continue
            if not os.path.exists(p(file)):
                raise Step(f"{kind} {view}", f"{file} was not written")
            if kind == 'drawingset':
                drawing_set = file
            else:
                images.append({'file': file, 'kind': kind, 'view': view})
        try:
            contact_sheet(out, [i['file'] for i in images])
        except (ValueError, OSError, zlib.error, struct.error) as e:
            raise Step('contact sheet', str(e))
        with open(p('manifest.json'), 'w', encoding='utf-8') as f:
            json.dump(manifest(images, exports, skipped, script_sha256, set_scripts, drawing_set), f, indent=2)
            f.write('\n')
    except Step as e:
        print(f"render_all: step '{e.step}' failed: {e.why}", file=sys.stderr)
        return 1
    print(f"render_all: {len(images)} images, {len(exports)} exports, contact sheet in {out}")
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
