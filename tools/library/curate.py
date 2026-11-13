"""Make the committed library sample set from the downloaded Sweet Home 3D libraries (fetch.py) and our own items.

    python3 tools/library/curate.py --list DIR [--grep WORD]       browse: library, entry id, name, category, size, creator
    python3 tools/library/curate.py --src DIR --picks tools/library/picks.json \\
        --out assets/library/sample --attribution ATTRIBUTION.md      convert the picks, write catalog.json and the rows

picks.json is a list of picks: {"src": "<file>.sh3f", "entry": "<catalog id>", "id": "<our id>", "category",
"symbolType", "placement", "keywords": [...], "cutout"?: [w, d], "name"?, "size"?: [w, d, h], "textures"?: false}, or a
texture: {"src", "entry", "member": "<zip member>", "id", "category": "Materials", "symbolType": "material", "keywords"}.
A pick stops the run, with its reason, when its library's licence is not CC0 or CC-BY, its entry states a licence that
is not, it has no creator, or its file is over 1.5 MB. The ATTRIBUTION.md rows between the library-sample markers are
rewritten; the rest of the file is left alone.
"""
import argparse
import json
import os
import posixpath
import re
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from own import FILES as OWN_FILES, own_items  # noqa: E402
from sh3d import JPEG_MAGIC, PNG_MAGIC, extents, fit_primitives, obj_primitives, read_sh3f, write_glb  # noqa: E402

START, END = '<!-- library-sample:start -->', '<!-- library-sample:end -->'
LICENCES = {'CC0-1.0', 'CC-BY-3.0', 'CC-BY-3.0-US', 'CC-BY-4.0'}
LICENCE_URL = {'CC0-1.0': 'https://creativecommons.org/publicdomain/zero/1.0/',
               'CC-BY-3.0': 'https://creativecommons.org/licenses/by/3.0/',
               'CC-BY-3.0-US': 'https://creativecommons.org/licenses/by/3.0/us/',
               'CC-BY-4.0': 'https://creativecommons.org/licenses/by/4.0/'}
MAX_FILE = 1_500_000
PROPORTION_TOLERANCE = 0.05     # a mesh may differ from its catalog size's proportions by 5 % before it is refused
CHANGES = ('converted from OBJ/MTL to glTF binary by arch3d tools/library: triangulated, normals computed where the OBJ '
           'has none (smoothing groups averaged), moved to bottom-centre and scaled to the catalog size in metres, '
           'specular terms dropped, textures other than PNG or JPEG dropped')
HDRI_FILE = 'hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr'     # the one item that is not converted or made here
HDRI_README = 'hdri/README.md'
HDRI_ITEM = {'id': 'polyhaven:kloofendal-puresky', 'name': 'Partly cloudy sky (HDR)', 'category': 'Backdrops',
             'symbolType': 'backdrop', 'placement': None, 'size': None,
             'keywords': ['sky', 'hdri', 'hdr', 'clouds', 'day', 'backdrop'], 'file': HDRI_FILE,
             'source': 'https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky', 'licence': 'CC0-1.0',
             'author': 'Greg Zaal (original), Jarod Guest (sky edits)',
             'attribution': 'Kloofendal 48d Partly Cloudy (Pure Sky) by Greg Zaal and Jarod Guest, Poly Haven, CC0-1.0'}
WALL_DEFAULT_ELEVATION = {'hood': 1.5}     # metres to the bottom of a wall item whose library gives no elevation


class Refused(Exception):
    pass


def entry_licence_ok(text, library_licence):
    """An entry's own `license#n` (often just 'CC-BY' or 'CC0') must agree with its library's."""
    t = (text or '').strip().lower()
    if not t:
        return True
    if re.search(r'non-?commercial|share-?alike|no-?deriv|-(nc|sa|nd)\b|free art', t):
        return False
    if library_licence == 'CC0-1.0':
        return bool(re.search(r'cc-?0|public domain|creative commons zero', t))
    return bool(re.search(r'cc-?by|attribution', t))


def load_sources(src):
    with open(os.path.join(src, 'sources.json'), encoding='utf-8') as f:
        return json.load(f)


def list_entries(src, grep=None):
    for file, s in sorted(load_sources(src).items()):
        for e in read_sh3f(os.path.join(src, file)):
            line = (f'{file}\t{e["id"]}\t{e["name"]}\t{e["category"]}\t{e["width"]:.2f}x{e["depth"]:.2f}x{e["height"]:.2f}'
                    f'\t{e["creator"] or "-"}\t{e["licence"] or s["licence"]}\t{",".join(e["keywords"])}')
            if not grep or re.search(grep, line, re.I):
                print(line)


def blendswap_credit(zf, model):
    """(title, author, url) from the BLENDSWAP_LICENSE file beside a Blend Swap model, or None: the original work's
    title and page, which CC-BY 3.0 section 4(b) asks the credit to name."""
    folder = posixpath.dirname(model)
    for n in zf.namelist():
        if posixpath.dirname(n) == folder and posixpath.basename(n).upper().startswith('BLENDSWAP_LICENSE'):
            t = ' '.join(zf.read(n).decode('latin-1').split())
            m = re.search(r'downloading "([^"]+)" by (.+?) Released', t)
            u = re.search(r'(https?://(?:www\.)?blendswap\.com/\S+)', t)
            return (m.group(1) if m else None, m.group(2).strip() if m else None, u.group(1) if u else None)
    return None


def proportion_problem(size, ext):
    """Why a mesh with extents `ext` (glTF x, y, z) would be stretched by fitting it to `size` [w, d, h], or None."""
    scales = [e / v for e, v in zip((ext[0], ext[2], ext[1]), size) if v > 1e-4 and e > 1e-9]
    if len(scales) == 3 and min(scales) < (1 - PROPORTION_TOLERANCE) * max(scales):
        mesh = ' x '.join(f'{e:.4g}' for e in (ext[0], ext[2], ext[1]))
        return f'size {size} is not in the mesh\'s proportions ({mesh}, width x depth x height); give "size"'
    return None


def convert(pick, src, sources, entries_of):
    file = pick['src']
    s = sources.get(file)
    if not s:
        raise Refused(f'{pick["id"]}: {file} is not in sources.json')
    if s['licence'] not in LICENCES:
        raise Refused(f'{pick["id"]}: {file} is {s["licence"]}, not CC0 or CC-BY')
    entry = next((e for e in entries_of(file) if e['id'] == pick['entry']), None)
    if not entry:
        raise Refused(f'{pick["id"]}: no entry {pick["entry"]!r} in {file}')
    if not entry_licence_ok(entry['licence'], s['licence']):
        raise Refused(f'{pick["id"]}: entry {entry["id"]} says {entry["licence"]!r} (library {s["licence"]})')
    if not entry['creator']:
        raise Refused(f'{pick["id"]}: entry {entry["id"]} has no creator to attribute')
    if pick.get('expect') and entry['name'] != pick['expect']:
        raise Refused(f'{pick["id"]}: entry {entry["id"]} of {file} is {entry["name"]!r} now, not {pick["expect"]!r} '
                      f'(the library changed: pick again)')
    image_only = pick['symbolType'] in ('material', 'backdrop')
    size = None if image_only else pick.get('size') or [entry['width'], entry['depth'], entry['height']]
    with zipfile.ZipFile(os.path.join(src, file)) as zf:
        credit = blendswap_credit(zf, entry['model'])
        if pick.get('member'):
            data = zf.read(pick['member'])
            if not (data.startswith(PNG_MAGIC) or data.startswith(JPEG_MAGIC)):
                raise Refused(f'{pick["id"]}: {pick["member"]} is not a PNG or JPEG')
            ext = '.png' if data.startswith(PNG_MAGIC) else '.jpg'
        else:
            prims = obj_primitives(zf, entry['model'], entry['rotation'], textures=pick.get('textures', True),
                                   texture=pick.get('texture'))
            bad = proportion_problem(size, extents(prims))
            if bad:
                raise Refused(f'{pick["id"]}: {bad}')
            data = write_glb(fit_primitives(prims, size))
            ext = '.glb'
    if len(data) > MAX_FILE:
        raise Refused(f'{pick["id"]}: {len(data) / 1e6:.2f} MB (at most 1.5; try "textures": false or another entry)')
    keywords = list(dict.fromkeys([k.lower() for k in pick.get('keywords', [])] + [k.lower() for k in entry['keywords']]))
    title, author, page = (credit or (None, None, None))
    title, author = title or entry['name'], author or entry['creator']
    source = f'{s["library"]} ({s["url"]}), entry {entry["id"]}' + (f'; original {page}' if page else '')
    work = f'texture from "{title}"' if pick.get('member') else f'"{title}"'
    attribution = (f'{work} by {author}' + (f' ({page})' if page else '')
                   + (f', {s["attribution"]}' if s.get('attribution') else '') + f', {s["licence"]}; '
                   + ('copied unmodified' if pick.get('member') else CHANGES))
    item = {'id': pick['id'], 'name': pick.get('name') or entry['name'], 'category': pick['category'],
            'symbolType': pick['symbolType'], 'placement': None if image_only else pick['placement'],
            'size': size, 'keywords': keywords, 'source': source, 'licence': s['licence'], 'author': author,
            'attribution': attribution, 'file': pick['id'] + ext}
    if item['placement'] == 'wall':
        item['elevation'] = pick.get('elevation', entry['elevation'])
        if item['elevation'] is None:
            item['elevation'] = WALL_DEFAULT_ELEVATION.get(pick['symbolType'])
        if item['elevation'] is None:
            raise Refused(f'{pick["id"]}: a wall item needs its height ("elevation" in metres)')
    if pick['symbolType'] == 'light':
        item['light'] = pick.get('light') or entry['light']
        if not item['light']:
            raise Refused(f'{pick["id"]}: a light needs its light source (position, diameter, colour)')
    if 'cutout' in pick:
        item['cutout'] = pick['cutout']
    return item, data, title


def attribution_rows(items, titles, out):
    rows = ['## Library sample set', '',
            'Generated by `tools/library/curate.py` from `tools/library/picks.json` (Sweet Home 3D libraries fetched by',
            '`tools/library/fetch.py`). Changes to every converted model: ' + CHANGES + '. Texture images are copied',
            'unmodified. The items made for the set are original. Regenerate rather than edit by hand.', '',
            '| Asset | Author | Source | Licence | Notes |', '|---|---|---|---|---|',
            f'| `{out}/catalog.json` | CADCraft contributors | generated by `tools/library/curate.py` | MIT OR Apache-2.0 | '
            f'the sample set\'s index |']
    for i in items:
        lic = f'[{i["licence"]}]({LICENCE_URL[i["licence"]]})'
        if i['id'] == HDRI_ITEM['id']:
            rows.append(f'| `{out}/{i["file"]}` | {i["author"]} | [Poly Haven]({i["source"]}) | {lic} | '
                        f'1k Radiance HDR, unmodified |')
            rows.append(f'| `{out}/{HDRI_README}` | CADCraft contributors | written for the sample set | '
                        f'MIT OR Apache-2.0 | the sample HDRI\'s provenance note |')
            continue
        note = 'made for the sample set' if i['id'].startswith('own:') else i['attribution'].split(', ' + i['licence'])[0]
        rows.append(f'| `{out}/{i["file"]}` | {i["author"]} | {i["source"].replace("|", "/")} | {lic} | '
                    f'{note.replace("|", "/")} |')
    return '\n'.join(rows)


def rewrite_attribution(path, block):
    text = open(path, encoding='utf-8').read()
    wrapped = f'{START}\n{block}\n{END}'
    if START in text and END in text:
        text = text[:text.index(START)] + wrapped + text[text.index(END) + len(END):]
    elif '## Interface translations' in text:
        k = text.index('## Interface translations')
        text = text[:k] + wrapped + '\n\n' + text[k:]
    else:
        text = text.rstrip('\n') + '\n\n' + wrapped + '\n'
    with open(path, 'w', encoding='utf-8') as f:
        f.write(text)


def curate(src, picks_path, out, attribution):
    with open(picks_path, encoding='utf-8') as f:
        picks = json.load(f)
    sources, cache = load_sources(src), {}

    def entries_of(file):
        if file not in cache:
            cache[file] = read_sh3f(os.path.join(src, file))
        return cache[file]
    stale = sorted(set(os.listdir(out)) - {p['id'] + e for p in picks for e in ('.glb', '.png', '.jpg')}
                   - {'catalog.json', 'hdri', *OWN_FILES}) if os.path.isdir(out) else []
    if stale:
        raise Refused(f'{out} holds files no pick makes (remove them first): {stale}')
    os.makedirs(out, exist_ok=True)
    items, titles = [], {}
    for pick in picks:
        item, data, title = convert(pick, src, sources, entries_of)
        with open(os.path.join(out, item['file']), 'wb') as f:
            f.write(data)
        items.append(item)
        titles[item['id']] = title
    items += own_items(out)
    for f in (HDRI_FILE, HDRI_README):      # kept as committed: this item is neither converted nor made here
        if not os.path.isfile(os.path.join(out, f)):
            raise Refused(f'{out}/{f} is missing (restore it from git; curate.py keeps it)')
    items.append(dict(HDRI_ITEM))
    items.sort(key=lambda i: i['id'])
    if len({i['id'] for i in items}) != len(items):
        raise Refused('two items share an id')
    with open(os.path.join(out, 'catalog.json'), 'w', encoding='utf-8') as f:
        json.dump({'version': 1, 'items': items}, f, indent=1, ensure_ascii=False)
        f.write('\n')
    rewrite_attribution(attribution, attribution_rows(items, titles, out.rstrip('/')))
    return items


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--list', metavar='DIR')
    ap.add_argument('--grep')
    ap.add_argument('--src')
    ap.add_argument('--picks', default='tools/library/picks.json')
    ap.add_argument('--out', default='assets/library/sample')
    ap.add_argument('--attribution', default='ATTRIBUTION.md')
    a = ap.parse_args(argv)
    if a.list:
        list_entries(a.list, a.grep)
        return 0
    if not a.src:
        ap.error('--src DIR (or --list DIR)')
    try:
        items = curate(a.src, a.picks, a.out, a.attribution)
    except Refused as e:
        print(f'refused: {e}')
        return 1
    print(f'{len(items)} items written to {a.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
