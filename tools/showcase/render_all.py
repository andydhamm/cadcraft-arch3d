#!/usr/bin/env python3
"""Render the showcase house in every view style (see README.md). Python 3 standard library only."""
import argparse, hashlib, json, os, shlex, struct, subprocess, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SCRIPT = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'bilevel.json')
CAMERAS = ['Front', 'Back', 'Kitchen', 'Living']
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


def slug(s):
    return ''.join(c if c.isalnum() else '-' for c in s.lower()).strip('-')


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


# ---- the pipeline ----

def main(argv):
    ap = argparse.ArgumentParser(description='Render the showcase house in every view style.')
    ap.add_argument('--out', required=True)
    ap.add_argument('--size', default='960x720')
    ap.add_argument('--spp', type=int, default=64)
    ap.add_argument('--cli-json', default=None)
    ap.add_argument('--no-exports', action='store_true', help='skip GLB/IFC/OBJ export steps and leave exports empty')
    a = ap.parse_args(argv)
    try:
        w, h = (int(v) for v in a.size.lower().split('x'))
        if not (1 <= w <= 16384 and 1 <= h <= 16384) or not (1 <= a.spp <= 65536):
            raise ValueError
    except ValueError:
        print("render_all: --size must be WxH (1..16384) and --spp 1..65536", file=sys.stderr)
        return 2
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    script_sha256 = hashlib.sha256(open(SCRIPT, 'rb').read()).hexdigest()
    size = [w, h]
    dxf = os.path.join(out, 'bilevel.dxf')
    r = Runner(cli_base(a.cli_json), dxf)
    images, exports, skipped = [], {}, False

    def p(f):
        return os.path.join(out, f)

    def add(file, kind, view):
        if not os.path.exists(p(file)):
            raise Step(f"{kind} {view}", f"{file} was not written")
        images.append({'file': file, 'kind': kind, 'view': view})

    try:
        r.run('replay', [], file=False, extra=['--commands', SCRIPT, '--save', dxf])
        if not os.path.exists(dxf):
            raise Step('replay', 'bilevel.dxf was not written')
        rows, _ = r.run('inspect', [cmd('arch.inspect', {"levels": True, "views": True})])
        insp = next((x for x in rows if isinstance(x, dict) and 'levels' in x), {})
        levels = [l['name'] for l in insp.get('levels', []) if isinstance(l, dict) and isinstance(l.get('name'), str)]
        views = {v['name']: v['id'] for v in insp.get('views', []) if isinstance(v, dict) and 'name' in v and 'id' in v}
        if not levels:
            raise Step('inspect', 'the replayed drawing has no levels')
        for lv in levels:
            f = f'plan-{slug(lv)}.png'
            r.run(f'plan {lv}', [cmd('export', {"path": p(f), "level": lv})])
            add(f, 'plan', lv)
        for lv in ELECTRICAL_LEVELS:
            if lv not in levels:
                raise Step(f'electrical {lv}', 'no such level')
            f = f'electrical-{slug(lv)}.png'
            r.run(f'electrical {lv}', [cmd('view.electrical', {}), cmd('export', {"path": p(f), "level": lv})])
            add(f, 'electrical', lv)
        for kind, names in (('elevation', ['South', 'North', 'East', 'West']), ('section', ['Section A', 'Section B'])):
            for n in names:
                if n not in views:
                    raise Step(f'{kind} {n}', 'the drawing has no such view')
                f = f'{kind}-{slug(n.replace("Section ", ""))}.png'
                r.run(f'{kind} {n}', [cmd('export', {"path": p(f), "view": views[n]})])
                add(f, kind, n)
        for c in CAMERAS:
            f = f'camera-{slug(c)}.png'
            r.run(f'camera {c}', [cmd('render', {"out": p(f), "size": size, "camera": c})])
            add(f, 'camera', c)
        r.run('night', [cmd('render', {"out": p('night.png'), "size": size, "sun": False,
                                       "lightSet": "Evening", "camera": "Front"})])
        add('night.png', 'night', 'Front')
        r.run('path', [cmd('render', {"out": p('path.png'), "size": size, "final": "path", "spp": a.spp})])
        add('path.png', 'path', 'default')
        r.run('clay', [cmd('render', {"out": p('clay.png'), "size": size, "style": "clay"})])
        add('clay.png', 'clay', 'default')
        _, errs = r.run('technical', [cmd('render', {"out": p('technical.png'), "size": size, "style": "technical"})],
                        allow_refusal=True)
        if errs:
            if not any('not available yet' in e for e in errs):
                raise Step('technical', ' | '.join(errs[:3]))
            skipped = True
        else:
            add('technical.png', 'technical', 'default')
        if not a.no_exports:
            for k in ('glb', 'ifc', 'obj'):
                f = f'showcase.{k}'
                r.run(f'export {k}', [cmd('export', {"path": p(f)})])
                if not os.path.exists(p(f)):
                    raise Step(f'export {k}', f'{f} was not written')
                exports[k] = f
        try:
            contact_sheet(out, [i['file'] for i in images])
        except (ValueError, OSError, zlib.error, struct.error) as e:
            raise Step('contact sheet', str(e))
        manifest = {'images': images, 'exports': exports, 'contactSheet': 'contact-sheet.png', 'technicalSkipped': skipped,
                    'scriptSha256': script_sha256}
        with open(p('manifest.json'), 'w', encoding='utf-8') as f:
            json.dump(manifest, f, indent=2)
            f.write('\n')
    except Step as e:
        print(f"render_all: step '{e.step}' failed: {e.why}", file=sys.stderr)
        return 1
    print(f"render_all: {len(images)} images, {len(exports)} exports, contact sheet in {out}")
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
