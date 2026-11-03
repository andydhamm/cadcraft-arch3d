"""Check the committed library sample set against the phase-14 brief (run from the repository root):

    python3 tools/library/check_sample.py [--dir assets/library/sample]

Prints `sample set ok: N items, M MB` or every problem found, and exits non-zero on any problem.
"""
import argparse
import json
import os
import re
import shutil
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sh3d import GLB_MAGIC, JPEG_MAGIC, PNG_MAGIC  # noqa: E402

CATEGORIES = ['Furniture', 'Appliances and plumbing fixtures', 'Cabinet parts', 'Lighting and electrical', 'Materials',
              'Doors and windows', 'Plants', 'Backdrops']           # and User, empty in the sample set
# The brief's existing door and window catalogue fills Doors and windows; a door or window as a mesh symbol would be hung on
# a wall face instead of in an opening (review 11-03), so the sample set has none.
FROM_THE_CATALOGUES = {'Doors and windows'}
PLACEMENTS = {'floor', 'wall', 'ceiling', 'counter', 'in-cabinet'}
LICENCES = {'CC0-1.0', 'CC-BY-3.0', 'CC-BY-3.0-US', 'CC-BY-4.0'}
IMAGE_ONLY = {'backdrop', 'material'}                                # an image, not a placed symbol
MAX_FILE, MAX_TOTAL = 1_500_000, 15_000_000
FIELDS = ('id', 'name', 'category', 'symbolType', 'placement', 'size', 'keywords', 'source', 'licence', 'author',
          'attribution', 'file')
FIT = 1e-4          # metres: every symbol is bottom-centred at its catalog size (tools/library/sh3d.py fit_primitives)


def glb_problems(data):
    """Why `data` is not a plain glTF 2.0 binary (no compression or KTX2 extensions, PNG or JPEG images only)."""
    if len(data) < 20:
        return ['shorter than a GLB header']
    magic, version, length = struct.unpack('<4sII', data[:12])
    if (magic, version, length) != (GLB_MAGIC, 2, len(data)):
        return [f'header {magic!r} v{version} length {length} (file {len(data)})']
    n, kind = struct.unpack('<I4s', data[12:20])
    if kind != b'JSON':
        return ['first chunk is not JSON']
    doc = json.loads(data[20:20 + n])
    out = []
    if doc.get('extensionsUsed') or doc.get('extensionsRequired'):
        out.append(f'extensions {doc.get("extensionsUsed") or doc.get("extensionsRequired")}')
    if doc.get('asset', {}).get('version') != '2.0':
        out.append('asset.version is not 2.0')
    for img in doc.get('images', []):
        if img.get('mimeType') not in ('image/png', 'image/jpeg') or 'uri' in img:
            out.append(f'image {img.get("mimeType") or img.get("uri")} is not an embedded PNG or JPEG')
    if not sum(len(m.get('primitives', [])) for m in doc.get('meshes', [])):
        out.append('no mesh primitives')
    return out


def glb_bounds(data):
    """(lo, hi) of a GLB's POSITION accessors, glTF axes (y up)."""
    n = struct.unpack('<I', data[12:16])[0]
    doc = json.loads(data[20:20 + n])
    acc = [doc['accessors'][p['attributes']['POSITION']] for m in doc['meshes'] for p in m['primitives']]
    return ([min(a['min'][k] for a in acc) for k in range(3)], [max(a['max'][k] for a in acc) for k in range(3)])


def fit_problem(size, data):
    """Why a symbol is not bottom-centred at its catalog size [w, d, h] (glTF x and z centred, y from 0), or None."""
    lo, hi = glb_bounds(data)
    w, d, h = size
    want = [-w / 2, 0.0, -d / 2, w / 2, h, d / 2]
    if any(abs(g - e) > FIT for g, e in zip(lo + hi, want)):
        return f'mesh bounds {[round(v, 4) for v in lo + hi]} are not bottom-centred at its size {size}'
    return None


def light_problem(item):
    """Why a light item's light source is missing or outside its box, or None."""
    lt = item.get('light')
    if not isinstance(lt, dict):
        return 'a light without its light source'
    w, d, h = item['size']
    x, y, z = lt.get('position') or (None, None, None)
    if None in (x, y, z) or abs(x) > w / 2 + 1e-3 or abs(y) > d / 2 + 1e-3 or not -1e-3 <= z <= h + 1e-3:
        return f'light position {lt.get("position")} outside its {item["size"]} box'
    if not isinstance(lt.get('diameter'), (int, float)) or lt['diameter'] < 0 or \
            not re.fullmatch(r'#[0-9a-fA-F]{6}', str(lt.get('colour', ''))):
        return f'light diameter {lt.get("diameter")!r} or colour {lt.get("colour")!r}'
    return None


def validator_problems(paths):
    """The Khronos glTF validator (npm, under the checks' tool cache) on every .glb, when it is installed."""
    node = shutil.which('node')
    pkg = os.path.join(os.environ.get('XDG_CACHE_HOME') or os.path.expanduser('~/.cache'), 'cadcraft-checks',
                       'node_modules', 'gltf-validator')
    if not node or not os.path.isdir(pkg) or not paths:
        print(f'SKIP gltf-validator: not installed ({pkg})')
        return []
    js = ("const v=require(process.argv[1]);const fs=require('fs');(async()=>{const o={};for(const p of process.argv.slice(2))"
          "{const r=await v.validateBytes(new Uint8Array(fs.readFileSync(p)));o[p]=r.issues.messages.filter(m=>m.severity===0)"
          ".map(m=>m.code+' '+m.pointer)};console.log(JSON.stringify(o))})().catch(e=>{console.error(String(e));process.exit(2)})")
    p = subprocess.run([node, '-e', js, pkg] + paths, capture_output=True, text=True, timeout=600)
    if p.returncode:
        return [f'the glTF validator did not run: {p.stderr[-300:]}']
    return [f'{os.path.basename(k)}: validator errors {v[:4]}' for k, v in json.loads(p.stdout).items() if v]


def problems(sample_dir, attribution='ATTRIBUTION.md'):
    out = []
    try:
        with open(os.path.join(sample_dir, 'catalog.json'), encoding='utf-8') as f:
            cat = json.load(f)
    except (OSError, ValueError) as e:
        return [f'catalog.json: {e}'], 0, 0
    items = cat.get('items') or []
    if cat.get('version') != 1:
        out.append('catalog.json version is not 1')
    if len(items) < 40:
        out.append(f'{len(items)} items (at least 40)')
    ids = [i.get('id') for i in items]
    if len(set(ids)) != len(ids):
        out.append('item ids are not unique')
    md = open(attribution, encoding='utf-8').read() if os.path.exists(attribution) else ''
    total, glbs = 0, []
    for i in items:
        tag = i.get('id', '?')
        missing = [k for k in FIELDS if k not in i]
        if missing:
            out.append(f'{tag}: missing {missing}')
            continue
        if i['category'] not in CATEGORIES:
            out.append(f'{tag}: category {i["category"]!r}')
        if i['licence'] not in LICENCES:
            out.append(f'{tag}: licence {i["licence"]!r}')
        if not str(i['author']).strip() or not str(i['source']).strip() or not str(i['attribution']).strip():
            out.append(f'{tag}: no author, source or attribution')
        if i['placement'] == 'wall' and not (isinstance(i.get('elevation'), (int, float)) and i['elevation'] >= 0):
            out.append(f'{tag}: a wall item without its elevation')
        if i['symbolType'] in IMAGE_ONLY:
            if i['placement'] is not None:
                out.append(f'{tag}: an image item has placement {i["placement"]!r}')
        else:
            if i['placement'] not in PLACEMENTS:
                out.append(f'{tag}: placement {i["placement"]!r}')
            if not (isinstance(i['size'], list) and len(i['size']) == 3 and all(isinstance(v, (int, float)) and v > 0
                                                                                   for v in i['size'])):
                out.append(f'{tag}: size {i["size"]!r} is not three positive metres')
        path = os.path.join(sample_dir, i['file'])
        if not os.path.isfile(path):
            out.append(f'{tag}: no file {i["file"]}')
            continue
        data = open(path, 'rb').read()
        total += len(data)
        if len(data) > MAX_FILE:
            out.append(f'{tag}: {i["file"]} is {len(data) / 1e6:.2f} MB (at most 1.5)')
        if i['file'].endswith('.glb'):
            found = glb_problems(data)
            out += [f'{tag}: {p}' for p in found]
            if not found and isinstance(i['size'], list) and len(i['size']) == 3:
                bad = fit_problem(i['size'], data)
                if bad:
                    out.append(f'{tag}: {bad}')
                if i['symbolType'] == 'light' and light_problem(i):
                    out.append(f'{tag}: {light_problem(i)}')
            glbs.append(path)
        elif not (data.startswith(PNG_MAGIC) or data.startswith(JPEG_MAGIC)):
            out.append(f'{tag}: {i["file"]} is neither a .glb nor a PNG or JPEG')
        rel = os.path.relpath(path).replace(os.sep, '/')
        if f'`{rel}`' not in md:
            out.append(f'{tag}: `{rel}` is not in {attribution}')
    if total > MAX_TOTAL:
        out.append(f'the set is {total / 1e6:.1f} MB (at most 15)')
    for c in CATEGORIES:
        if c in FROM_THE_CATALOGUES:
            continue
        if not any(i.get('category') == c for i in items):
            out.append(f'no item in {c}')

    def kind(t, keyword=None, without=None):
        return [i for i in items if i.get('symbolType') == t and (keyword is None or keyword in i.get('keywords', []))
                and (without is None or without not in i.get('keywords', []))]
    need = [('two dining chairs', len(kind('chair/stool', 'dining')) >= 2),
            ('another chair', len(kind('chair/stool', without='dining')) >= 1),
            ('a sofa', kind('sofa')), ('a bed', kind('bed')), ('a dining table', kind('table', 'dining')),
            ('a 0.6 m dishwasher', [i for i in kind('dishwasher') if abs(i['size'][0] - 0.6) <= 0.01]),
            ('a range', kind('range')), ('a refrigerator', kind('refrigerator')),
            ('a kitchen sink with its cut-out', [i for i in kind('sink') if isinstance(i.get('cutout'), list)
                                                 and len(i['cutout']) == 2]),
            ('a faucet', kind('faucet')), ('a toilet', kind('toilet')), ('a tub', kind('bathtub')),
            ('a wall item', [i for i in items if i.get('placement') == 'wall']),
            ('a ceiling item', [i for i in items if i.get('placement') == 'ceiling']),
            ('two pull styles', len(kind('pull')) >= 2), ('a backdrop image', kind('backdrop'))]
    out += [f'no {what}' for what, ok in need if not ok]
    out += validator_problems(glbs)
    return out, len(items), total


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--dir', default='assets/library/sample')
    ap.add_argument('--attribution', default='ATTRIBUTION.md')
    a = ap.parse_args(argv)
    found, n, total = problems(a.dir, a.attribution)
    for p in found:
        print('FAIL:', p)
    if found:
        return 1
    print(f'sample set ok: {n} items, {total / 1e6:.1f} MB')
    return 0


if __name__ == '__main__':
    sys.exit(main())
