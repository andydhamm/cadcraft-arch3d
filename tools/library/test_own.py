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

    def test_writes_the_three_files_with_entries(self):
        self.assertEqual(sorted(self.by_file), ['backdrop-sky.png', 'pull-bar.glb', 'pull-knob.glb'])
        for i in self.items:
            self.assertEqual(i['licence'], 'CC0-1.0')
            self.assertTrue(i['author'] and i['source'] and i['id'] and i['name'])
        self.assertEqual({i['symbolType'] for i in self.items}, {'pull', 'backdrop'})
        self.assertEqual(self.by_file['backdrop-sky.png']['category'], 'Backdrops')
        self.assertEqual(self.by_file['pull-bar.glb']['category'], 'Cabinet parts')

    def test_glbs_are_plain_gltf2(self):
        for name in ('pull-bar.glb', 'pull-knob.glb'):
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

    def test_sky_is_a_1024_by_512_png(self):
        png = self.read('backdrop-sky.png')
        self.assertEqual(png[:8], b'\x89PNG\r\n\x1a\n')
        self.assertEqual(struct.unpack('>4sII', png[12:24]), (b'IHDR', 1024, 512))


if __name__ == '__main__':
    unittest.main()
