#!/usr/bin/env python3
# Search: text over names, keywords and folder names (every word must match), filter chips that combine with AND;
# `chair` + chip symbolType chair/stool returns exactly the sample set's chairs and stools, adding `dining` narrows to the
# dining chairs, removing the chip widens again, a chip with no match returns 0 (a valid answer). Back and forward walk
# the filter history; a saved filter applies again in a new session; Favorites is a built-in tag; folders list under a
# category; `library {"for": ...}` narrows to what fits a target.
# needs: p14-arch-library, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-doc-cabinet-parts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

CHAIR = {"kind": "symbolType", "value": "chair/stool"}
rc, rr, so, se = run(['library {}',
                      cmd('library', {"text": "chair", "chips": [CHAIR]}),
                      cmd('library', {"text": "chair dining", "chips": [CHAIR]}),
                      cmd('library.filter', {"save": "dining chairs"}),
                      cmd('library', {"text": "chair dining"}),
                      cmd('library', {"text": "sofa", "chips": [{"kind": "placement", "value": "ceiling"}]}),
                      cmd('library', {"back": True}),
                      cmd('library', {"back": True}),
                      cmd('library', {"forward": True}),
                      cmd('library', {"chips": [{"kind": "category", "value": "Furniture"}]})])
all_items = {i['id']: i for i in lib_reply(rr, 0)['items']}


def words_match(item, words):
    hay = ' '.join([str(item.get('name', '')), ' '.join(item.get('keywords') or []), str(item.get('folder', ''))]).lower()
    return all(w in hay for w in words)


sample_chairs = [i for i in all_items.values() if i.get('catalog') == 'sample' and i.get('symbolType') == 'chair/stool']
if len(sample_chairs) < 3:
    fail(f"the sample set lists {len(sample_chairs)} chairs/stools")
want1 = ids_of([i for i in all_items.values() if i.get('symbolType') == 'chair/stool' and words_match(i, ['chair'])])
got1 = ids_of(lib_reply(rr, 1)['items'])
if got1 != want1 or not set(ids_of(sample_chairs)) <= set(got1):
    fail(f"'chair' + symbolType chair/stool: {got1}, expected {want1} (every sample chair/stool: {ids_of(sample_chairs)})")
want2 = ids_of([i for i in all_items.values() if i.get('symbolType') == 'chair/stool' and words_match(i, ['chair', 'dining'])])
got2 = ids_of(lib_reply(rr, 2)['items'])
if got2 != want2 or not got2 or len(got2) >= len(got1):
    fail(f"adding 'dining' should narrow {got1} to {want2}; got {got2}")
dining = [i for i in got2 if i in SAMPLE_IDS]
if len(dining) < 2:
    fail(f"fewer than two sample dining chairs: {dining}")
got3 = ids_of(lib_reply(rr, 3)['items'])
want3 = ids_of([i for i in all_items.values() if words_match(i, ['chair', 'dining'])])
if got3 != want3 or not set(got2) <= set(got3) or len(got3) <= len(got2):
    fail(f"removing the chip should widen {got2} to {want3}; got {got3}")
z = lib_reply(rr, 4)
if z.get('items') != [] or z.get('total') != 0:
    fail(f"a chip with no match must return 0 items: {z.get('total')} {ids_of(z.get('items', []))[:5]}")
print(f"ok: chair -> {len(got1)}, + dining -> {len(got2)}, chip removed -> {len(got3)}, sofa on the ceiling -> 0")

b1, b2, f1 = lib_reply(rr, 5), lib_reply(rr, 6), lib_reply(rr, 7)
if ids_of(b1['items']) != got3 or ids_of(b2['items']) != got2 or ids_of(f1['items']) != got3:
    fail(f"back/forward: {ids_of(b1['items'])[:4]}…, {ids_of(b2['items'])[:4]}…, {ids_of(f1['items'])[:4]}…")
if b2.get('forward') is not True or not isinstance(b2.get('back'), bool):
    fail(f"the history flags after two steps back: back {b2.get('back')!r}, forward {b2.get('forward')!r}")
print("ok: back and forward walk the filter history")

fur = lib_reply(rr, 9)
folders = {f.get('name'): f.get('items') for f in fur.get('folders') or []}
if not folders or sum(v for v in folders.values() if isinstance(v, int)) != fur.get('total'):
    fail(f"Furniture's folders {folders} do not add up to its {fur.get('total')} items")
if any(i.get('category') != 'Furniture' for i in fur['items']):
    fail("the category chip let other categories through")
print(f"ok: Furniture has {fur.get('total')} items in folders {sorted(folders)}")

# A saved filter applies in a new session; Favorites is a built-in tag and a chip.
rc, rr, so, se = run([cmd('library.filter', {}),
                      cmd('library.filter', {"apply": "dining chairs"}),
                      cmd('library.tag', {"item": "sofa-futon", "tag": "Favorites"}),
                      cmd('library', {"chips": [{"kind": "tag", "value": "Favorites"}]}),
                      cmd('library.tag', {"item": "sofa-futon", "tag": "Favorites", "on": False}),
                      cmd('library', {"chips": [{"kind": "tag", "value": "Favorites"}]})])
filters = reply_with(rr, 'filters', 0, 'library.filter')['filters']
if not any(f.get('name') == 'dining chairs' for f in filters):
    fail(f"the saved filter is not listed in a new session: {filters}")
if ids_of(lib_reply(rr, 0)['items']) != got2:
    fail(f"applying the saved filter: {ids_of(lib_reply(rr, 0)['items'])}, expected {got2}")
if ids_of(lib_reply(rr, 1)['items']) != ['sofa-futon'] or lib_reply(rr, 2)['items']:
    fail(f"Favorites: {ids_of(lib_reply(rr, 1)['items'])} then {ids_of(lib_reply(rr, 2)['items'])}")
print("ok: saved filters survive the session; Favorites tags and untags")

# What fits a target: a cabinet's pull, its door style, a face appliance, a ceiling finish layer.
rc, rr, so, se = run(ROOM + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0}),
                             cmd('library', {"for": "cabinet/cb0/pull"}),
                             cmd('library', {"for": "cabinet/cb0/front"}),
                             cmd('library', {"for": "room/r0/ceiling"})])
pulls, fronts, ceil = lib_reply(rr, 0)['items'], lib_reply(rr, 1)['items'], lib_reply(rr, 2)['items']
if not pulls or any(i.get('symbolType') != 'pull' for i in pulls) or not {'own:pull-bar-128', 'own:pull-knob-32'} <= set(ids_of(pulls)):
    fail(f"for a pull: {ids_of(pulls)}")
if not fronts or any(i.get('kind') != 'frontStyle' for i in fronts):
    fail(f"for a front: {ids_of(fronts)}")
if not ceil or any(i.get('category') != 'Materials' for i in ceil):
    fail(f"for a ceiling: {ids_of(ceil)}")
print("ok: library {for} narrows to pulls, front styles and materials")
print("PASS 03")
