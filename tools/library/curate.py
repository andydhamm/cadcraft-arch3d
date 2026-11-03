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
import re
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from own import FILES as OWN_FILES, own_items  # noqa: E402
from sh3d import JPEG_MAGIC, PNG_MAGIC, obj_to_glb, read_sh3f  # noqa: E402

START, END = '<!-- library-sample:start -->', '<!-- library-sample:end -->'
LICENCES = {'CC0-1.0', 'CC-BY-3.0', 'CC-BY-3.0-US', 'CC-BY-4.0'}
LICENCE_URL = {'CC0-1.0': 'https://creativecommons.org/publicdomain/zero/1.0/',
               'CC-BY-3.0': 'https://creativecommons.org/licenses/by/3.0/',
               'CC-BY-3.0-US': 'https://creativecommons.org/licenses/by/3.0/us/',
               'CC-BY-4.0': 'https://creativecommons.org/licenses/by/4.0/'}
MAX_FILE = 1_500_000


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
    with zipfile.ZipFile(os.path.join(src, file)) as zf:
        if pick.get('member'):
            data = zf.read(pick['member'])
            if not (data.startswith(PNG_MAGIC) or data.startswith(JPEG_MAGIC)):
                raise Refused(f'{pick["id"]}: {pick["member"]} is not a PNG or JPEG')
            ext = '.png' if data.startswith(PNG_MAGIC) else '.jpg'
        else:
            data = obj_to_glb(zf, entry['model'], entry['rotation'], textures=pick.get('textures', True))
            ext = '.glb'
    if len(data) > MAX_FILE:
        raise Refused(f'{pick["id"]}: {len(data) / 1e6:.2f} MB (at most 1.5; try "textures": false or another entry)')
    image_only = pick['symbolType'] in ('material', 'backdrop')
    keywords = list(dict.fromkeys([k.lower() for k in pick.get('keywords', [])] + [k.lower() for k in entry['keywords']]))
    item = {'id': pick['id'], 'name': pick.get('name') or entry['name'], 'category': pick['category'],
            'symbolType': pick['symbolType'], 'placement': None if image_only else pick['placement'],
            'size': None if image_only else pick.get('size') or [entry['width'], entry['depth'], entry['height']],
            'keywords': keywords, 'source': f'{s["library"]} ({s["url"]}), entry {entry["id"]}',
            'licence': s['licence'], 'author': entry['creator'], 'file': pick['id'] + ext}
    if 'cutout' in pick:
        item['cutout'] = pick['cutout']
    return item, data, entry['name']


def attribution_rows(items, titles, out):
    rows = ['## Library sample set', '',
            'Generated by `tools/library/curate.py` from `tools/library/picks.json` (Sweet Home 3D libraries fetched by',
            '`tools/library/fetch.py`; the models converted from OBJ to glTF binary, geometry and materials unmodified,',
            'textures other than PNG or JPEG dropped). Regenerate rather than edit by hand.', '',
            '| Asset | Author | Source | Licence | Notes |', '|---|---|---|---|---|',
            f'| `{out}/catalog.json` | CADCraft contributors | generated by `tools/library/curate.py` | MIT OR Apache-2.0 | '
            f'the sample set\'s index |']
    for i in items:
        lic = f'[{i["licence"]}]({LICENCE_URL[i["licence"]]})'
        title = titles.get(i['id'], i['name']).replace('|', '/')
        note = 'made for the sample set' if i['id'].startswith('own:') else f'"{title}", converted to `.glb`' \
            if i['file'].endswith('.glb') else f'texture from "{title}"'
        rows.append(f'| `{out}/{i["file"]}` | {i["author"]} | {i["source"].replace("|", "/")} | {lic} | {note} |')
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
                   - {'catalog.json', *OWN_FILES}) if os.path.isdir(out) else []
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
