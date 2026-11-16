"""Tests for render_all.py: the plan, names, manifest and arguments. No CLI, no GPU.

Run from the repository root: python3 -m unittest tools/showcase/test_render_all.py
"""
import json
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import render_all as ra  # noqa: E402

ROOT = ra.ROOT
SHOWCASE = os.path.join(ROOT, 'docs', 'arch3d', 'showcase')
LEVELS = [{'name': 'Lower', 'kind': 'floor'}, {'name': 'Main', 'kind': 'floor'}, {'name': 'Entry', 'kind': 'landing'}]
VIEWS = [{'name': n, 'id': f'v{i}', 'kind': 'elevation'} for i, n in enumerate(['South', 'North', 'East', 'West'])] + [
    {'name': 'Section A', 'id': 'v4', 'kind': 'section'}, {'name': 'Section B', 'id': 'v5', 'kind': 'section'},
    {'name': 'Kitchen West Interior', 'id': 'v6', 'kind': 'interior'},
    {'name': 'Main Bath East Interior', 'id': 'v7', 'kind': 'interior'}]
NUMBERS = ['A-000', 'A-001', 'A-101', 'E-101', 'A-102', 'A-111', 'E-111', 'A-112', 'A-121', 'A-201', 'A-301', 'A-302', 'A-401', 'A-501']
SHEETS = [{'number': n, 'layout': f'Sheet {n}'} for n in NUMBERS]

# Every file name today's tool (before phase 21) writes for these levels and views.
TODAY = ['plan-lower.png', 'plan-main.png', 'plan-entry.png', 'electrical-lower.png', 'electrical-main.png',
         'elevation-south.png', 'elevation-north.png', 'elevation-east.png', 'elevation-west.png',
         'section-a.png', 'section-b.png', 'camera-front.png', 'camera-back.png', 'camera-kitchen.png', 'camera-living.png',
         'night.png', 'path.png', 'clay.png', 'technical.png', 'showcase.glb', 'showcase.ifc', 'showcase.obj']
NEW = (['camera-dusk.png', 'camera-noon-hdri.png', 'camera-overview.png']
       + [f'planview-{l}-{k}.png' for l in ('lower', 'main', 'entry') for k in ('floor', 'rcp')]
       + ['planview-roof.png', 'planview-site.png', 'interior-kitchen-west-interior.png',
          'interior-main-bath-east-interior.png', 'drawing-set.pdf']
       + [f'sheet-{n.lower()}.png' for n in NUMBERS])


def files(rows):
    return [r[0] for r in rows]


class RenderAll(unittest.TestCase):
    def test_plan_lists_every_gallery_step(self):
        got = files(ra.plan(LEVELS, VIEWS, SHEETS, True))
        self.assertEqual(len(got), len(set(got)), 'each file once')
        for f in TODAY + NEW:
            self.assertIn(f, got)
        self.assertEqual(got, files(ra.plan(LEVELS, VIEWS, SHEETS, True)), 'a stable order')
        self.assertLess(got.index('showcase.obj'), got.index('camera-dusk.png'), 'the new steps follow the old')
        sheets = [f for f in got if f.startswith('sheet-')]
        self.assertEqual(sheets, [f'sheet-{n.lower()}.png' for n in NUMBERS], 'sheet order')
        for row in ra.plan(LEVELS, VIEWS, SHEETS, True):
            self.assertEqual(len(row), 4)
        look = {r[2]: r for r in ra.plan(LEVELS, VIEWS, SHEETS, True) if r[1] == 'look'}
        self.assertEqual(sorted(look), ['dusk', 'noon-hdri', 'overview'])
        self.assertEqual(look['noon-hdri'][3][0][1].get('spp'), 64)
        self.assertNotIn('spp', look['dusk'][3][0][1])

    def test_drawing_set_steps_replay_three_scripts(self):
        rows = [r for r in ra.plan(LEVELS, VIEWS, SHEETS, True) if r[1] == 'replay']
        self.assertEqual(len(rows), 1)
        args = rows[0][3]
        names = [os.path.basename(a) for i, a in enumerate(args) if i and args[i - 1] == '--commands']
        self.assertEqual(names, ['bilevel.json', 'phase-20-showcase.json', 'phase-21-showcase.json'])
        self.assertEqual(args[-2:], ['--save', 'set.dxf'])
        self.assertEqual(rows[0][0], 'set.dxf')

    def test_no_set_skips_sheets_and_pdf(self):
        rows = ra.plan(LEVELS, VIEWS, SHEETS, False)
        banned = {'planview', 'rcp', 'roofplan', 'siteplan', 'interior', 'sheet', 'drawingset', 'replay'}
        self.assertFalse([r for r in rows if r[1] in banned])
        self.assertNotIn('drawing-set.pdf', files(rows))
        # A plain run is what phase-15 check 11 has always run: no look cameras either (they are part of --full).
        self.assertEqual([r[0] for r in rows if r[1] == 'look'], [])
        full = ra.plan(LEVELS, VIEWS, SHEETS, True)
        self.assertEqual([r[0] for r in full if r[1] == 'look'], ['camera-dusk.png', 'camera-noon-hdri.png', 'camera-overview.png'])
        a = ra.parse_args(['--out', 'x', '--full', '--no-set'])
        self.assertTrue(a.no_set and a.full)
        plain = ra.parse_args(['--out', 'x'])
        self.assertFalse(plain.no_set or plain.full)

    def test_manifest_kinds_and_script_hashes(self):
        images = [{'file': r[0], 'kind': r[1], 'view': r[2]} for r in ra.plan(LEVELS, VIEWS, SHEETS, True)
                  if r[1] not in ('replay', 'export', 'drawingset')]
        m = ra.manifest(images, {'glb': 'showcase.glb'}, False, 'a' * 64,
                        {n: 'b' * 64 for n in ra.SET_SCRIPT_NAMES}, 'drawing-set.pdf')
        self.assertEqual(m['scriptSha256'], 'a' * 64)
        self.assertEqual(sorted(m['setScripts']), ['phase-20-showcase.json', 'phase-21-showcase.json'])
        self.assertEqual(m['drawingSet'], 'drawing-set.pdf')
        self.assertEqual(m['contactSheet'], 'contact-sheet.png')
        for i in m['images']:
            self.assertTrue(i.get('file') and i.get('kind') and i.get('view'), i)
        kinds = {i['kind'] for i in m['images']}
        self.assertTrue({'plan', 'electrical', 'elevation', 'section', 'camera', 'night', 'path', 'clay', 'look',
                         'planview', 'rcp', 'roofplan', 'siteplan', 'interior', 'sheet'} <= kinds)
        none = ra.manifest(images, {}, False, 'a' * 64, {}, None)
        self.assertIsNone(none['drawingSet'])
        json.dumps(m)

    def test_slug_and_names_are_stable(self):
        self.assertEqual(ra.slug('A-101'), 'a-101')
        self.assertEqual(ra.slug('Main Bath east'), 'main-bath-east')
        hostile = '../../etc/' + 'x.' * 200 + '\\..\\y'
        for name in (hostile, '.' * 300, 'a' * 300, '/' * 5):
            s = ra.slug(name)
            f = ra.file_name('interior', name, '.png')
            for t in (s, f):
                self.assertNotIn('/', t)
                self.assertNotIn('\\', t)
                self.assertFalse(t.startswith('.'))
                self.assertLessEqual(len(t), 80)
        self.assertEqual(ra.file_name('sheet', 'A-101', '.png'), 'sheet-a-101.png')
        views = VIEWS + [{'name': hostile, 'id': 'v9', 'kind': 'interior'}]
        for f in files(ra.plan(LEVELS, views, SHEETS, True)):
            self.assertNotIn('/', f)
            self.assertLessEqual(len(f), 80)

    def test_readme_gallery_matches_the_plan(self):
        with open(os.path.join(SHOWCASE, 'phase-21-showcase.json'), encoding='utf-8') as f:
            script = json.load(f)
        cmds = script if isinstance(script, list) else script.get('commands', [])
        numbers, interiors = [], []
        for c in cmds:
            p = c.get('params', {}) if isinstance(c, dict) else {}
            if c.get('cmd') == 'sheet' and 'number' in p:
                numbers.append(p['number'])
            if c.get('cmd') == 'elevation' and 'room' in p and 'side' in p:
                interiors.append(f"{p['room']} {p['side'].capitalize()} Interior")
        self.assertEqual(len(numbers), 14)
        views = VIEWS[:6] + [{'name': n, 'id': f'v{6 + i}', 'kind': 'interior'} for i, n in enumerate(interiors)]
        sheets = [{'number': n, 'layout': n} for n in numbers]
        names = set(files(ra.plan(LEVELS, views, sheets, True))) | {'contact-sheet.png', 'drawing-set.pdf'}
        with open(os.path.join(SHOWCASE, 'README.md'), encoding='utf-8') as f:
            readme = f.read()
        linked = re.findall(r'renders/([A-Za-z0-9._-]+)', readme)
        self.assertTrue(linked)
        for l in linked:
            self.assertIn(l, names, f'README links renders/{l}, which the tool does not write')
        for l in sorted(names - {'set.dxf'} -{n for n in names if n.startswith('showcase.')}):
            self.assertIn(l, linked, f'the tool writes {l}, which the README does not list')
        for s in ('phase-21-showcase.json', 'plot', 'showcase-set.pdf'):
            self.assertIn(s, readme)


if __name__ == '__main__':
    unittest.main()
