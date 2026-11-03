"""Tests for own.py: the sample set's own CC-0 items (two pulls and a sky backdrop).

Run from the repository root: python3 -m unittest tools/library/test_own.py -v
"""
import os
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from own import own_items  # noqa: E402
from sh3d import GLB_MAGIC  # noqa: E402
from test_sh3d import GlbParts  # noqa: E402


class OwnItemsTests(unittest.TestCase, GlbParts):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.items = own_items(self.dir.name)
        self.by_file = {i['file']: i for i in self.items}

    def tearDown(self):
        self.dir.cleanup()

    def read(self, name):
        with open(os.path.join(self.dir.name, name), 'rb') as f:
            return f.read()

    def test_writes_its_files_with_entries(self):
        self.assertEqual(sorted(self.by_file), ['backdrop-sky.png', 'dishwasher-60.glb', 'pull-bar.glb', 'pull-knob.glb',
                                                'range-30.glb', 'sink-kitchen.glb'])
        for i in self.items:
            self.assertEqual(i['licence'], 'CC0-1.0')
            self.assertTrue(i['author'] and i['source'] and i['id'] and i['name'])
        self.assertEqual({i['symbolType'] for i in self.items}, {'pull', 'backdrop', 'range', 'sink', 'dishwasher'})
        self.assertEqual(self.by_file['backdrop-sky.png']['category'], 'Backdrops')
        self.assertEqual(self.by_file['pull-bar.glb']['category'], 'Cabinet parts')

    def test_glbs_are_plain_gltf2(self):
        for name in ('pull-bar.glb', 'pull-knob.glb', 'range-30.glb', 'sink-kitchen.glb', 'dishwasher-60.glb'):
            glb = self.read(name)
            self.assertEqual(struct.unpack('<4sII', glb[:12]), (GLB_MAGIC, 2, len(glb)))
            doc = self.json_chunk(glb)
            self.assertNotIn('extensionsUsed', doc)
            self.assertGreater(sum(a['count'] for a in self.index_accessors(doc)), 0)

    def test_bar_pull_posts_are_128_mm_apart(self):
        doc = self.json_chunk(self.read('pull-bar.glb'))
        posts = [p for p in doc['meshes'][0]['primitives'] if doc['materials'][p['material']]['name'].startswith('post')]
        self.assertEqual(len(posts), 2)
        centres = sorted((doc['accessors'][p['attributes']['POSITION']]['min'][0] +
                          doc['accessors'][p['attributes']['POSITION']]['max'][0]) / 2 for p in posts)
        self.assertAlmostEqual(centres[1] - centres[0], 0.128, delta=1e-6)

    def test_knob_is_32_mm_across(self):
        doc = self.json_chunk(self.read('pull-knob.glb'))
        acc = [doc['accessors'][p['attributes']['POSITION']] for p in doc['meshes'][0]['primitives']]
        span = max(a['max'][0] for a in acc) - min(a['min'][0] for a in acc)
        self.assertAlmostEqual(span, 0.032, delta=1e-6)

    def test_range_is_30_inches_wide_on_the_floor(self):
        doc = self.json_chunk(self.read('range-30.glb'))
        acc = [doc['accessors'][p['attributes']['POSITION']] for p in doc['meshes'][0]['primitives']]
        lo = [min(a['min'][k] for a in acc) for k in range(3)]
        hi = [max(a['max'][k] for a in acc) for k in range(3)]
        self.assertAlmostEqual(hi[0] - lo[0], 0.762, delta=1e-6)       # width along X
        self.assertAlmostEqual(lo[1], 0.0, delta=1e-6)                 # stands on y = 0 (glTF Y up)
        item = self.by_file['range-30.glb']
        self.assertEqual((item['category'], item['placement']), ('Appliances and plumbing fixtures', 'floor'))
        for k, v in enumerate((hi[0] - lo[0], hi[2] - lo[2], hi[1] - lo[1])):  # size is width, depth, height (Z up)
            self.assertAlmostEqual(item['size'][k], v, delta=1e-6)

    def test_every_symbol_is_bottom_centred_at_its_catalog_size(self):
        for i in self.items:
            if not i['file'].endswith('.glb'):
                continue
            doc = self.json_chunk(self.read(i['file']))
            acc = [doc['accessors'][p['attributes']['POSITION']] for p in doc['meshes'][0]['primitives']]
            lo = [min(a['min'][k] for a in acc) for k in range(3)]
            hi = [max(a['max'][k] for a in acc) for k in range(3)]
            w, d, h = i['size']
            for got, want in zip(lo + hi, [-w / 2, 0.0, -d / 2, w / 2, h, d / 2]):
                self.assertAlmostEqual(got, want, delta=1e-6, msg=i['id'])

    def test_kitchen_sink_and_dishwasher_fit_phase_13_casework(self):
        # crates/arch/src/casework.rs: base cabinets 0.6 deep, counter underside 0.876; the phase-13 sink is 0.8 x 0.55
        sink, dw = self.by_file['sink-kitchen.glb'], self.by_file['dishwasher-60.glb']
        self.assertEqual((sink['placement'], sink['size'][:2]), ('counter', [0.8, 0.55]))
        self.assertTrue(0 < sink['cutout'][0] < 0.8 and 0 < sink['cutout'][1] < 0.55)
        self.assertEqual((dw['placement'], dw['size']), ('floor', [0.6, 0.6, 0.876]))

    def test_sky_is_a_1024_by_512_png(self):
        png = self.read('backdrop-sky.png')
        self.assertEqual(png[:8], b'\x89PNG\r\n\x1a\n')
        self.assertEqual(struct.unpack('>4sII', png[12:24]), (b'IHDR', 1024, 512))


if __name__ == '__main__':
    unittest.main()
