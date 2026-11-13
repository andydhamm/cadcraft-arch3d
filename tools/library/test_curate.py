"""Tests for curate.py's refusals: a pick from a library or entry whose licence is not CC0 / CC-BY, or with no creator.

Run from the repository root: python3 -m unittest tools/library/test_curate.py -v
"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from curate import HDRI_FILE, HDRI_README, Refused, convert, curate  # noqa: E402
from sh3d import read_sh3f  # noqa: E402
from test_sh3d import PROPS, Sh3dTests, make_sh3f  # noqa: E402

PICK = {'src': 'lib.sh3f', 'entry': 'test#cube', 'id': 'cube', 'category': 'Furniture', 'symbolType': 'décor',
        'placement': 'floor', 'keywords': []}
CUBE = PROPS.replace('width#1=60', 'width#1=50').replace('depth#1=55', 'depth#1=50').replace('height#1=85', 'height#1=50')
BLENDSWAP = ('------\nThank you for downloading "Country-Kitchen Cycles" by Jay-Artist\nReleased under Creative Commons '
             'Attribution 3.0\nDownloaded from http://www.blendswap.com/blends/view/42851\n')


class CurateTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.dir.cleanup()

    def convert(self, licence='CC0-1.0', props=CUBE, attribution='', extra=None, **pick):
        make_sh3f(os.path.join(self.dir.name, 'lib.sh3f'), props=props, extra=extra)
        sources = {'lib.sh3f': {'licence': licence, 'library': 'Test', 'url': 'https://example.invalid/lib',
                                'attribution': attribution}}
        return convert({**PICK, **pick}, self.dir.name, sources, lambda f: read_sh3f(os.path.join(self.dir.name, f)))

    def test_a_cc0_pick_converts_with_its_attribution(self):
        item, data, title = self.convert()
        self.assertEqual((item['licence'], item['author'], item['file'], title), ('CC0-1.0', 'tester', 'cube.glb', 'Cube'))
        self.assertEqual(item['size'], [0.5, 0.5, 0.5])
        self.assertTrue(data.startswith(b'glTF'))

    def test_the_glb_is_bottom_centred_at_the_catalog_size(self):
        item, data, _ = self.convert()
        lo, hi = Sh3dTests('test_glb_is_plain_gltf2').bounds(data)
        for got, want in zip(lo + hi, [-0.25, 0.0, -0.25, 0.25, 0.5, 0.25]):
            self.assertAlmostEqual(got, want, places=6)

    def test_a_cc_by_item_credits_the_author_the_licensor_and_the_change(self):
        item, _, _ = self.convert(licence='CC-BY-3.0', attribution='Copyright © 2008 A. Licensor')
        for part in ('"Cube"', 'tester', 'Copyright © 2008 A. Licensor', 'CC-BY-3.0', 'converted'):
            self.assertIn(part, item['attribution'])

    def test_a_blendswap_licence_file_gives_the_original_title_and_url(self):
        item, _, _ = self.convert(licence='CC-BY-3.0', extra={'BLENDSWAP_LICENSE.txt': BLENDSWAP})
        self.assertIn('"Country-Kitchen Cycles"', item['attribution'])
        self.assertIn('http://www.blendswap.com/blends/view/42851', item['attribution'])
        self.assertIn('http://www.blendswap.com/blends/view/42851', item['source'])

    def test_a_pick_whose_entry_name_changed_is_refused(self):
        with self.assertRaisesRegex(Refused, 'Sphere'):
            self.convert(expect='Sphere')
        self.assertEqual(self.convert(expect='Cube')[0]['name'], 'Cube')

    def test_a_wall_light_keeps_its_height_and_its_light_source(self):
        props = CUBE + 'elevation#1=140\nlightSourceX#1=25\nlightSourceY#1=25\nlightSourceZ#1=45\nlightSourceColor#1=#998764\n'
        item, _, _ = self.convert(props=props, placement='wall', symbolType='light', category='Lighting and electrical')
        self.assertEqual(item['elevation'], 1.4)
        self.assertEqual(item['light']['position'], [0.0, 0.0, 0.45])

    def test_a_mesh_out_of_its_catalog_proportions_is_refused(self):
        with self.assertRaisesRegex(Refused, 'proportions'):
            self.convert(props=PROPS)                    # a unit cube catalogued 0.6 x 0.55 x 0.85
        self.assertEqual(self.convert(props=PROPS, size=[0.5, 0.5, 0.5])[0]['size'], [0.5, 0.5, 0.5])

    def test_a_library_that_is_not_cc0_or_cc_by_is_refused(self):
        with self.assertRaisesRegex(Refused, 'not CC0 or CC-BY'):
            self.convert(licence='CC-BY-NC-4.0')

    def test_an_entry_without_a_creator_is_refused(self):
        with self.assertRaisesRegex(Refused, 'no creator'):
            self.convert(props=CUBE.replace('creator#1=tester\n', ''))

    def test_an_entry_licence_that_disagrees_is_refused(self):
        with self.assertRaisesRegex(Refused, 'Free Art'):
            self.convert(licence='CC-BY-3.0', props=CUBE + 'license#1=Free Art License\n')

    def test_keeps_the_hdri_item(self):
        out = os.path.join(self.dir.name, 'out')
        os.makedirs(os.path.join(out, 'hdri'))
        for f in (HDRI_FILE, HDRI_README):
            with open(os.path.join(out, f), 'wb') as fh:
                fh.write(b'#?RADIANCE\n')
        for name, text in (('sources.json', '{}'), ('picks.json', '[]'), ('ATTRIBUTION.md', '# x\n')):
            with open(os.path.join(self.dir.name, name), 'w') as fh:
                fh.write(text)
        md = os.path.join(self.dir.name, 'ATTRIBUTION.md')
        items = curate(self.dir.name, os.path.join(self.dir.name, 'picks.json'), out, md)
        self.assertIn('polyhaven:kloofendal-puresky', [i['id'] for i in items])
        self.assertTrue(os.path.isfile(os.path.join(out, HDRI_FILE)))
        text = open(md).read()
        self.assertIn(f'`{out}/{HDRI_FILE}`', text)
        self.assertIn(f'`{out}/{HDRI_README}`', text)


if __name__ == '__main__':
    unittest.main()
