"""Sweet Home 3D furniture libraries (.sh3f) to plain glTF 2.0 binaries (.glb), standard library only.

A .sh3f is a ZIP holding PluginFurnitureCatalog.properties (Java properties: `name#1=...`, sizes in centimetres) and the
models it names (Wavefront OBJ with MTL materials and textures). This is groundwork outside the Rust code (like the
phase-12 validators): arch3d's own reader takes the .glb files it writes, so they stay plain (no Draco or meshopt, no
KTX2; PNG or JPEG textures only, sniffed by their bytes; any other texture is dropped and its material keeps its Kd).

    read_sh3f(path) -> [entry]            one dict per catalog entry, sizes in metres
    obj_to_glb(zf, model, rotation=None)  the OBJ member `model` (and its MTL and textures) as .glb bytes
    write_glb(primitives) -> bytes        primitives: positions, normals, uvs or None, indices, color, image or None
"""
import json
import math
import posixpath
import struct
import zipfile

GLB_MAGIC = b"glTF"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
JPEG_MAGIC = b"\xff\xd8\xff"
CATALOG = "PluginFurnitureCatalog.properties"


# ---------- the catalog ----------

def parse_properties(text):
    """Java .properties: `key=value` or `key: value` or `key value`, `#`/`!` comments, `\\` line continuations and
    `\\uXXXX` / `\\t` / `\\n` escapes."""
    lines, out = text.splitlines(), {}
    k = 0
    while k < len(lines):
        line = lines[k].lstrip()
        k += 1
        if not line or line[0] in '#!':
            continue
        while _continues(line) and k < len(lines):
            line = line[:-1] + lines[k].lstrip()
            k += 1
        key, i = [], 0
        while i < len(line):
            c = line[i]
            if c == '\\' and i + 1 < len(line):
                key.append(line[i:i + 2])
                i += 2
                continue
            if c in '=: \t':
                break
            key.append(c)
            i += 1
        while i < len(line) and line[i] in ' \t':
            i += 1
        if i < len(line) and line[i] in '=:':
            i += 1
        while i < len(line) and line[i] in ' \t':
            i += 1
        out[_unescape(''.join(key))] = _unescape(line[i:])
    return out


def _continues(line):
    n = len(line) - len(line.rstrip('\\'))
    return n % 2 == 1


def _unescape(s):
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == '\\' and i + 1 < len(s):
            n = s[i + 1]
            if n == 'u' and i + 6 <= len(s):
                try:
                    out.append(chr(int(s[i + 2:i + 6], 16)))
                    i += 6
                    continue
                except ValueError:
                    pass
            out.append({'t': '\t', 'n': '\n', 'r': '\r', 'f': '\f'}.get(n, n))
            i += 2
            continue
        out.append(c)
        i += 1
    return ''.join(out)


def _member(name):
    """A catalog path (`/cube.obj`, or a `jar:` URL's tail after `!/`) as a ZIP member name."""
    if '!/' in name:
        name = name.split('!/', 1)[1]
    return name.lstrip('/')


def _floats(s):
    try:
        return [float(x) for x in s.split()]
    except ValueError:
        return None


def read_sh3f(path):
    """One dict per catalog entry, in index order: index, id, name, category, model (ZIP member), width, depth, height
    (metres), creator, keywords, door_or_window, staircase, rotation (the 3 x 3 modelRotation row-major, or None),
    licence and information (as the catalog states them, or '')."""
    with zipfile.ZipFile(path) as zf:
        props = parse_properties(zf.read(CATALOG).decode('latin-1'))
    by_index = {}
    for key, value in props.items():
        if '#' not in key:
            continue
        field, _, n = key.rpartition('#')
        if n.isdigit():
            by_index.setdefault(int(n), {})[field] = value
    entries = []
    for n in sorted(by_index):
        f = by_index[n]
        if 'model' not in f or 'name' not in f:
            continue
        size = []
        for k in ('width', 'depth', 'height'):
            try:
                size.append(round(float(f.get(k, '0')) / 100.0, 6))
            except ValueError:
                size.append(0.0)
        rot = _floats(f.get('modelRotation', ''))
        entries.append({
            'index': n, 'id': f.get('id', f'#{n}'), 'name': f['name'], 'category': f.get('category', ''),
            'model': _member(f['model']), 'width': size[0], 'depth': size[1], 'height': size[2],
            'creator': f.get('creator', '').strip(),
            'keywords': [t.strip() for t in f.get('tags', '').split(',') if t.strip()],
            'door_or_window': f.get('doorOrWindow', 'false').strip().lower() == 'true',
            'staircase': bool(f.get('staircaseCutOutShape', '').strip()),
            'rotation': rot if rot and len(rot) == 9 else None,
            'licence': f.get('license', f.get('licence', '')).strip(),
            'information': f.get('information', '').strip(),
        })
    return entries


# ---------- OBJ and MTL ----------

def _read_member(zf, name):
    """A ZIP member by name, falling back to a case-insensitive match (libraries made on Windows)."""
    try:
        return zf.read(name)
    except KeyError:
        low = name.lower()
        for n in zf.namelist():
            if n.lower() == low:
                return zf.read(n)
    return None


# map_Kd options and how many arguments each takes: numbers (up to the count) or one on/off or channel word.
_MAP_NUMERIC = {'-bm': 1, '-boost': 1, '-mm': 2, '-o': 3, '-s': 3, '-t': 3, '-texres': 1}
_MAP_WORD = ('-blendu', '-blendv', '-cc', '-clamp', '-imfchan')


def _map_file(tokens):
    """The file name of a `map_Kd [options] file` line: the options and their arguments skipped."""
    i = 0
    while i < len(tokens) - 1:
        opt = tokens[i].lower()
        if opt in _MAP_WORD:
            i += 2
        elif opt in _MAP_NUMERIC:
            i += 1
            for _ in range(_MAP_NUMERIC[opt]):
                if i < len(tokens) - 1 and _floats(tokens[i]) is not None:
                    i += 1
        else:
            break
    return ' '.join(tokens[i:])


def parse_mtl(text):
    """{material: {'kd': [r, g, b], 'alpha': a, 'map': file or None}}."""
    mats, cur = {}, None
    for raw in text.splitlines():
        t = raw.strip().split()
        if not t or t[0].startswith('#'):
            continue
        key = t[0].lower()
        if key == 'newmtl':
            cur = mats.setdefault(' '.join(t[1:]), {'kd': [0.8, 0.8, 0.8], 'alpha': 1.0, 'map': None})
        elif cur is None:
            continue
        elif key == 'kd' and len(t) >= 4 and _floats(' '.join(t[1:4])):
            cur['kd'] = [min(1.0, max(0.0, v)) for v in _floats(' '.join(t[1:4]))]
        elif key == 'd' and len(t) >= 2 and _floats(t[-1]):
            cur['alpha'] = min(1.0, max(0.0, _floats(t[-1])[0]))
        elif key == 'tr' and len(t) >= 2 and _floats(t[-1]):
            cur['alpha'] = min(1.0, max(0.0, 1.0 - _floats(t[-1])[0]))
        elif key == 'map_kd' and len(t) >= 2:
            cur['map'] = _map_file(t[1:]) or None
    return mats


def _index(token, count):
    """An OBJ index (1-based, or negative from the end) as a 0-based index, or None when absent or out of range."""
    if not token:
        return None
    i = int(token)
    i = i - 1 if i > 0 else count + i
    return i if 0 <= i < count else None


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _unit(v, fallback=(0.0, 0.0, 1.0)):
    n = math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
    if not n or not math.isfinite(n):
        return fallback
    return (v[0] / n, v[1] / n, v[2] / n)


def _rotate(m, p):
    return (m[0] * p[0] + m[1] * p[1] + m[2] * p[2], m[3] * p[0] + m[4] * p[1] + m[5] * p[2],
            m[6] * p[0] + m[7] * p[1] + m[8] * p[2])


def obj_to_glb(zf, model, rotation=None):
    """The OBJ member `model` of `zf` as plain .glb bytes: one primitive per material in first-use order, faces
    triangulated as fans, flat normals where the OBJ gives none, V flipped (glTF's origin is top-left), the catalog's
    modelRotation (row-major 3 x 3) applied when given."""
    text = (_read_member(zf, model) or b'').decode('utf-8', 'replace')
    base = posixpath.dirname(model)
    verts, uvs, norms, mats = [], [], [], {}
    groups, order, cur = {}, [], ''
    for raw in text.splitlines():
        t = raw.strip().split()
        if not t:
            continue
        key = t[0]
        if key == 'v' and len(t) >= 4:
            verts.append(tuple(float(x) for x in t[1:4]))
        elif key == 'vt' and len(t) >= 2:
            uvs.append((float(t[1]), float(t[2]) if len(t) > 2 else 0.0))
        elif key == 'vn' and len(t) >= 4:
            norms.append(tuple(float(x) for x in t[1:4]))
        elif key == 'mtllib':
            for lib in [' '.join(t[1:])] + t[1:]:
                data = _read_member(zf, posixpath.normpath(posixpath.join(base, lib)))
                if data is not None:
                    mats.update(parse_mtl(data.decode('utf-8', 'replace')))
                    break
        elif key == 'usemtl':
            cur = ' '.join(t[1:])
        elif key == 'f' and len(t) >= 4:
            corners = []
            for c in t[1:]:
                parts = c.split('/')
                corners.append((_index(parts[0], len(verts)),
                                _index(parts[1], len(uvs)) if len(parts) > 1 else None,
                                _index(parts[2], len(norms)) if len(parts) > 2 else None))
            if any(v is None for v, _, _ in corners):
                continue
            if cur not in groups:
                groups[cur] = []
                order.append(cur)
            groups[cur].append(corners)
    prims = []
    for name in order:
        mat = mats.get(name, {'kd': [0.8, 0.8, 0.8], 'alpha': 1.0, 'map': None})
        image = None
        if mat['map']:
            data = _read_member(zf, posixpath.normpath(posixpath.join(base, mat['map'].replace('\\', '/'))))
            if data and (data.startswith(PNG_MAGIC) or data.startswith(JPEG_MAGIC)):
                image = data
        textured = image is not None
        pos, nor, tex, idx, seen = [], [], [], [], {}
        for face_no, corners in enumerate(groups[name]):
            p = [verts[v] for v, _, _ in corners]
            flat = None
            for k in range(1, len(corners) - 1):
                tri = (0, k, k + 1)
                if flat is None or any(corners[j][2] is None for j in tri):
                    flat = _unit(_cross(_sub(p[k], p[0]), _sub(p[k + 1], p[0])), flat or (0.0, 0.0, 1.0))
                for j in tri:
                    v, vt, vn = corners[j]
                    key = (v, vt if textured else None, vn if vn is not None else ('flat', face_no, k))
                    if key not in seen:
                        seen[key] = len(pos)
                        pos.append(p[j])
                        nor.append(_unit(norms[vn]) if vn is not None else flat)
                        if textured:
                            u = uvs[vt] if vt is not None else (0.0, 0.0)
                            tex.append((u[0], 1.0 - u[1]))
                    idx.append(seen[key])
        if not idx:
            continue
        if rotation:
            pos = [_rotate(rotation, q) for q in pos]
            nor = [_unit(_rotate(rotation, q)) for q in nor]
        color = [1.0, 1.0, 1.0, mat['alpha']] if textured else list(mat['kd']) + [mat['alpha']]
        prims.append({'positions': pos, 'normals': nor, 'uvs': tex if textured else None, 'indices': idx,
                      'color': color, 'image': image, 'name': name})
    return write_glb(prims)


# ---------- glTF binary ----------

def _pad(b, fill):
    return b + fill * ((4 - len(b) % 4) % 4)


def write_glb(primitives, name='symbol'):
    """A plain glTF 2.0 binary: one node, one mesh, one primitive and one material per entry (baseColorFactor from
    `color`, baseColorTexture for an `image` of PNG or JPEG bytes), POSITION with min/max, NORMAL, TEXCOORD_0 when
    `uvs` is given, uint32 indices; the BIN chunk 4-byte aligned."""
    bin_, views, accs, meshes_p, materials, images, textures = bytearray(), [], [], [], [], [], []

    def view(data, target=None):
        while len(bin_) % 4:
            bin_.append(0)
        v = {'buffer': 0, 'byteOffset': len(bin_), 'byteLength': len(data)}
        if target:
            v['target'] = target
        bin_.extend(data)
        views.append(v)
        return len(views) - 1

    def acc(values, kind, width):
        flat = [c for v in values for c in v] if width > 1 else list(values)
        if kind == 'f':
            data = struct.pack(f'<{len(flat)}f', *flat)
            a = {'bufferView': view(data, 34962), 'componentType': 5126, 'count': len(values),
                 'type': {2: 'VEC2', 3: 'VEC3'}[width]}
        else:
            data = struct.pack(f'<{len(flat)}I', *flat)
            a = {'bufferView': view(data, 34963), 'componentType': 5125, 'count': len(values), 'type': 'SCALAR'}
        accs.append(a)
        return len(accs) - 1, a

    for p in primitives:
        pos_i, pos_a = acc(p['positions'], 'f', 3)
        pos_a['min'] = [min(v[k] for v in p['positions']) for k in range(3)]
        pos_a['max'] = [max(v[k] for v in p['positions']) for k in range(3)]
        attrs = {'POSITION': pos_i, 'NORMAL': acc(p['normals'], 'f', 3)[0]}
        if p.get('uvs') is not None:
            attrs['TEXCOORD_0'] = acc(p['uvs'], 'f', 2)[0]
        pbr = {'baseColorFactor': [float(c) for c in p['color']], 'metallicFactor': 0.0, 'roughnessFactor': 0.8}
        img = p.get('image')
        if img is not None and p.get('uvs') is not None:
            mime = 'image/png' if img.startswith(PNG_MAGIC) else 'image/jpeg' if img.startswith(JPEG_MAGIC) else None
            if mime:
                images.append({'bufferView': view(img), 'mimeType': mime})
                textures.append({'sampler': 0, 'source': len(images) - 1})
                pbr['baseColorTexture'] = {'index': len(textures) - 1}
        mat = {'name': p.get('name') or f'm{len(materials)}', 'pbrMetallicRoughness': pbr, 'doubleSided': True}
        if p['color'][3] < 1.0:
            mat['alphaMode'] = 'BLEND'
        materials.append(mat)
        meshes_p.append({'attributes': attrs, 'indices': acc(p['indices'], 'i', 1)[0], 'material': len(materials) - 1})
    doc = {'asset': {'version': '2.0', 'generator': 'arch3d tools/library'}, 'scene': 0,
           'scenes': [{'nodes': [0]}], 'nodes': [{'name': name, 'mesh': 0}] if meshes_p else [],
           'meshes': [{'name': name, 'primitives': meshes_p}] if meshes_p else [],
           'materials': materials, 'accessors': accs, 'bufferViews': views,
           'buffers': [{'byteLength': len(_pad(bytes(bin_), b'\x00'))}]}
    if not meshes_p:
        doc['scenes'] = [{'nodes': []}]
    if images:
        doc['images'], doc['textures'] = images, textures
        doc['samplers'] = [{'magFilter': 9729, 'minFilter': 9987, 'wrapS': 10497, 'wrapT': 10497}]
    js = _pad(json.dumps(doc, separators=(',', ':')).encode(), b' ')
    bn = _pad(bytes(bin_), b'\x00')
    total = 12 + 8 + len(js) + 8 + len(bn)
    return (struct.pack('<4sII', GLB_MAGIC, 2, total) + struct.pack('<I4s', len(js), b'JSON') + js
            + struct.pack('<I4s', len(bn), b'BIN\x00') + bn)
