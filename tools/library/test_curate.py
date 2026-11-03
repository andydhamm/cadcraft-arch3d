"""Tests for curate.py's refusals: a pick from a library or entry whose licence is not CC0 / CC-BY, or with no creator.

Run from the repository root: python3 -m unittest tools/library/test_curate.py -v
"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from curate import Refused, convert  # noqa: E402
from sh3d import read_sh3f  # noqa: E402
from test_sh3d import PROPS, make_sh3f  # noqa: E402

PICK = {'src': 'lib.sh3f', 'entry': 'test#cube', 'id': 'cube', 'category': 'Furniture', 'symbolType': 'décor',
        'placement': 'floor', 'keywords': []}


class CurateTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.dir.cleanup()

    def convert(self, licence='CC0-1.0', props=PROPS):
        make_sh3f(os.path.join(self.dir.name, 'lib.sh3f'), props=props)
        sources = {'lib.sh3f': {'licence': licence, 'library': 'Test', 'url': 'https://example.invalid/lib'}}
        return convert(PICK, self.dir.name, sources, lambda f: read_sh3f(os.path.join(self.dir.name, f)))

    def test_a_cc0_pick_converts_with_its_attribution(self):
        item, data, title = self.convert()
        self.assertEqual((item['licence'], item['author'], item['file'], title), ('CC0-1.0', 'tester', 'cube.glb', 'Cube'))
        self.assertEqual(item['size'], [0.6, 0.55, 0.85])
        self.assertTrue(data.startswith(b'glTF'))

    def test_a_library_that_is_not_cc0_or_cc_by_is_refused(self):
        with self.assertRaisesRegex(Refused, 'not CC0 or CC-BY'):
            self.convert(licence='CC-BY-NC-4.0')

    def test_an_entry_without_a_creator_is_refused(self):
        with self.assertRaisesRegex(Refused, 'no creator'):
            self.convert(props=PROPS.replace('creator#1=tester\n', ''))

    def test_an_entry_licence_that_disagrees_is_refused(self):
        with self.assertRaisesRegex(Refused, 'Free Art'):
            self.convert(licence='CC-BY-3.0', props=PROPS + 'license#1=Free Art License\n')


if __name__ == '__main__':
    unittest.main()
