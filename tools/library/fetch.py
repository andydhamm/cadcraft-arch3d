"""Download the Sweet Home 3D furniture libraries the sample set may use: CC-0 (public domain) and CC-BY only.

    python3 tools/library/fetch.py --out DIR [--refresh]

Writes DIR/<library>.sh3f and DIR/sources.json ({file: {url, library, licence, page_licence, licence_text, sha256,
bytes}}). Keep DIR outside the repository: only the curated, converted items are committed.

Every link is resolved from the pages at run time, never kept here (standing rule 2026-10-18):
- https://www.sweethome3d.com/import-models/ lists "3DModels-<library>-<version>.zip (size - n models - licence)";
  the ZIP holds the .sh3f and a LICENSE.TXT;
- https://www.dodecagon.nl/ ("All 3D models on this site are in the Public Domain ... CC0 1.0 Universal") links a page
  per library whose form posts a file id to `download`.
A library is refused, with its reason, unless its page entry says public domain / CC0 or CC-BY and its own licence text
(LICENSE.TXT, or the catalog's `license=`) names CC0 or a versioned CC-BY (no NC, SA or ND).
"""
import argparse
import hashlib
import html
import io
import json
import os
import re
import sys
import urllib.parse
import urllib.request
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sh3d import CATALOG, parse_properties  # noqa: E402

SH3D_PAGE = 'https://www.sweethome3d.com/import-models/'
SH3D_WANTED = ('BlendSwap-CC-0', 'Scopia', 'KatorLegaz', 'BlendSwap-CC-BY')
DODECAGON = 'https://www.dodecagon.nl/'
DODECAGON_WANTED = ('furniture01', 'kitchen2', 'appliances', 'bathroom', 'bedroom', 'doors')
AGENT = {'User-Agent': 'arch3d-tools-library/1 (+https://github.com/andydhamm/cadcraft-arch3d)'}


def sh3d_libraries(page):
    """[{library, file, url, page_licence}] for each "3DModels-<library>-<version>.zip" link of the import-models page."""
    # Entities first (the page writes `</a>&nbsp;(...)`), and a link's text never runs past its own </a>: a lazy `.*?`
    # did, and labelled the Free Art License library with the next library's 'CC-BY License' (11-03).
    out, page = [], html.unescape(page)
    pat = re.compile(r'<a\s[^>]*href="([^"]*/(3DModels-([A-Za-z0-9-]+?)-\d[\d.]*\.zip)/download)"[^>]*>'
                     r'(?:(?!</a>).)*</a>\s*\(([^)]*)\)', re.S | re.I)
    for url, file, library, note in pat.findall(page):
        parts = [p.strip() for p in re.split(r'\s[–—-]\s', note)]
        out.append({'library': library, 'file': file, 'url': url, 'page_licence': parts[-1]})
    return out


def dodecagon_libraries(page):
    """[{fileid, file}] for each download form of a Dodecagon library page."""
    pat = re.compile(r'<form[^>]*action="download"[^>]*>.*?name="fileid"\s+value="(\d+)".*?([A-Za-z0-9_.-]+\.sh3f)\s*</button>',
                     re.S | re.I)
    return [{'fileid': fid, 'file': name} for fid, name in pat.findall(page)]


def dodecagon_site_licence(index):
    """Dodecagon's site-wide statement ("All 3D models on this site are in the Public Domain ... CC0 1.0 Universal"),
    or None when the index page no longer says so."""
    text = ' '.join(html.unescape(re.sub(r'<[^>]+>', ' ', index)).split())
    m = re.search(r'All 3D models on this site are in the Public Domain[^.]*\.', text)
    return f'{m.group(0)} CC0 1.0 Universal' if m and 'CC0 1.0 Universal' in text else None


def licence_id(text):
    """The SPDX id of a licence statement when it is CC0 or a versioned CC-BY (CC0-1.0, CC-BY-3.0, CC-BY-3.0-US,
    CC-BY-4.0, ...); None for anything else, a CC-BY without a version included."""
    t = ' '.join((text or '').lower().split())
    if not t or re.search(r'non-?commercial|share-?alike|no-?deriv|\bby-(nc|sa|nd)\b|cc[- ]by[- ](nc|sa|nd)|free art', t):
        return None
    if re.search(r'public domain|\bcc-?0\b|publicdomain/zero|creative commons zero', t):
        return 'CC0-1.0'
    m = re.search(r'(?:attribution|licenses/by/|\bcc[- ]by\b)\D{0,40}?(\d\.\d)', t)
    if not m:
        return None
    us = '/by/3.0/us' in t or 'united states' in t     # a bare ' us' matched "commercial use" (11-03)
    return f'CC-BY-{m.group(1)}' + ('-US' if us and m.group(1) == '3.0' else '')


def page_says_free(text):
    """A page entry's licence is acceptable as a first filter: public domain / CC0 or CC-BY, versioned or not."""
    t = (text or '').lower()
    if licence_id(text) is None and re.search(r'non-?commercial|share-?alike|no-?deriv|free art|-(nc|sa|nd)\b', t):
        return False
    return bool(re.search(r'public domain|cc-?0|cc-?by|attribution', t))


def get(url, data=None):
    req = urllib.request.Request(url, data=data, headers=AGENT, method='POST' if data else 'GET')
    with urllib.request.urlopen(req, timeout=600) as r:
        return r.read()


def catalog_licence(sh3f_bytes):
    """The catalog's library-wide `license=` statement, or ''."""
    with zipfile.ZipFile(io.BytesIO(sh3f_bytes)) as z:
        return parse_properties(z.read(CATALOG).decode('latin-1')).get('license', '')


def record(out, file, data, **fields):
    with open(os.path.join(out, file), 'wb') as f:
        f.write(data)
    return {**fields, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}


def cached(out, file, refresh):
    p = os.path.join(out, file)
    if not refresh and os.path.exists(p):
        with open(p, 'rb') as f:
            return f.read()
    return None


def fetch_sh3d(out, refresh, sources, refused):
    page = get(SH3D_PAGE).decode('utf-8', 'replace')
    libs = {lib['library']: lib for lib in sh3d_libraries(page)}
    for name in SH3D_WANTED:
        lib = libs.get(name)
        if not lib:
            refused.append((name, f'not listed on {SH3D_PAGE}'))
            continue
        if not page_says_free(lib['page_licence']):
            refused.append((name, f'the page says {lib["page_licence"]!r}'))
            continue
        zbytes = cached(out, lib['file'], refresh) or get(lib['url'])
        with open(os.path.join(out, lib['file']), 'wb') as f:
            f.write(zbytes)
        with zipfile.ZipFile(io.BytesIO(zbytes)) as z:
            members = [m for m in z.namelist() if m.lower().endswith('.sh3f')]
            if len(members) != 1:
                refused.append((name, f'{lib["file"]} holds {len(members)} .sh3f files'))
                continue
            sh3f = z.read(members[0])
            lic = next((m for m in z.namelist() if os.path.basename(m).upper().startswith('LICENSE')), None)
            text = z.read(lic).decode('latin-1') if lic else catalog_licence(sh3f)
        spdx = licence_id(text) or licence_id(catalog_licence(sh3f))
        if not spdx:
            refused.append((name, f'its licence text does not name CC0 or a versioned CC-BY: {" ".join(text.split())[:160]!r}'))
            continue
        file = f'{name}.sh3f'
        sources[file] = record(out, file, sh3f, url=lib['url'], library=name, licence=spdx,
                               page_licence=lib['page_licence'], licence_text=' '.join(text.split())[:400])


def fetch_dodecagon(out, refresh, sources, refused):
    index = get(DODECAGON).decode('utf-8', 'replace')
    site = dodecagon_site_licence(index)
    if not site:
        refused.append(('Dodecagon', f'{DODECAGON} no longer states public domain / CC0 1.0'))
        return
    pages = sorted(set(re.findall(r'href="(dodecagon[a-z0-9]+)"', index)))
    for want in DODECAGON_WANTED:
        page = f'dodecagon{want}'
        if page not in pages:
            refused.append((f'Dodecagon {want}', f'no page {page} on {DODECAGON}'))
            continue
        forms = dodecagon_libraries(get(DODECAGON + page).decode('utf-8', 'replace'))
        for form in forms:
            data = cached(out, form['file'], refresh) or get(DODECAGON + 'download',
                                                              urllib.parse.urlencode({'fileid': form['fileid']}).encode())
            if not data.startswith(b'PK'):
                refused.append((form['file'], 'the download is not a ZIP'))
                continue
            own = catalog_licence(data)
            spdx = licence_id(own) if own else 'CC0-1.0'
            if not spdx:
                refused.append((form['file'], f'its catalog says {own!r}'))
                continue
            sources[form['file']] = record(out, form['file'], data, url=DODECAGON + page, library=f'Dodecagon {want}',
                                           licence=spdx, page_licence=site,
                                           licence_text=' '.join((own or site).split())[:400],
                                           fileid=form['fileid'])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--out', required=True, help='a folder outside the repository')
    ap.add_argument('--refresh', action='store_true', help='download again even when a file is already there')
    a = ap.parse_args(argv)
    os.makedirs(a.out, exist_ok=True)
    sources, refused = {}, []
    fetch_sh3d(a.out, a.refresh, sources, refused)
    fetch_dodecagon(a.out, a.refresh, sources, refused)
    with open(os.path.join(a.out, 'sources.json'), 'w') as f:
        json.dump(sources, f, indent=1, sort_keys=True)
    for file, s in sorted(sources.items()):
        print(f'{file}: {s["library"]}, {s["licence"]}, {s["bytes"] / 1e6:.1f} MB')
    for name, why in refused:
        print(f'refused {name}: {why}')
    return 0 if sources else 1


if __name__ == '__main__':
    sys.exit(main())
