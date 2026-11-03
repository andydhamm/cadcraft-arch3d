"""Tests for fetch.py's page parsing and licence rules (no network).

Run from the repository root: python3 -m unittest tools/library/test_fetch.py -v
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch import attribution_line, dodecagon_libraries, dodecagon_site_licence, licence_id, sh3d_libraries  # noqa: E402

SH3D_PAGE = '''<p><a href="https://sourceforge.net/projects/sweethome3d/files/SweetHome3D-models/3DModels-1.9.3/3DModels-Contributions-1.9.3.zip/download" target="_blank">3DModels-Contributions-1.9.3.zip</a> (29 MB &#8211; 511 models &#8211; Free Art License)</p>
<p><a href="https://sourceforge.net/projects/sweethome3d/files/SweetHome3D-models/3DModels-1.9.3/3DModels-Scopia-1.9.3.zip/download" target="_blank" rel="noopener">3DModels-Scopia-1.9.3.zip</a> (69.5 MB &#8211; 500 models &#8211; CC-BY License)</p>
<p><a href="https://sourceforge.net/projects/sweethome3d/files/SweetHome3D-models/3DModels-1.9.3/3DModels-BlendSwap-CC-0-1.9.3.zip/download">3DModels-BlendSwap-CC-0-1.9.3.zip</a> (23.8 MB &#8211; 175 models &#8211; Public domain)</p>
'''

DODECAGON_PAGE = '''<div class="title bold">Bathroom</div>
<p class="card-body">Sweet Home 3D Furniture Library<br>version 2.2 [2026-01-14 10:19:50] - 785.1 kB - 48 models
<form action="download" method="post"><input type="hidden" name="fileid" value="5">
<button class="download-button" type="submit" id="submit"><img src="images/FurnitureLibrary.png"> DodecagonBathroom.sh3f</button></form>'''


class FetchTests(unittest.TestCase):
    def test_sh3d_page_lists_each_library_with_its_licence(self):
        libs = {lib['library']: lib for lib in sh3d_libraries(SH3D_PAGE)}
        self.assertEqual(sorted(libs), ['BlendSwap-CC-0', 'Contributions', 'Scopia'])
        self.assertEqual(libs['Scopia']['page_licence'], 'CC-BY License')
        self.assertEqual(libs['BlendSwap-CC-0']['file'], '3DModels-BlendSwap-CC-0-1.9.3.zip')
        self.assertTrue(libs['Scopia']['url'].endswith('/3DModels-Scopia-1.9.3.zip/download'))

    def test_a_licence_note_after_nbsp_stays_with_its_own_link(self):
        # The live page (11-03): `</a>&nbsp;(29 MB - 511 models - Free Art License)<br><a href=...Scopia...>`; a match
        # that ran on to the next link labelled the Free Art License library 'CC-BY License'.
        page = ('<p><a href="https://x/3DModels-1.9.3/3DModels-Contributions-1.9.3.zip/download">3DModels-Contributions-1.9.3.zip'
                '</a>&nbsp;(29 MB &#8211; 511 models &#8211; Free Art License)<br><a href="https://x/3DModels-1.9.3/'
                '3DModels-Scopia-1.9.3.zip/download">3DModels-Scopia-1.9.3.zip</a>&nbsp;(69.5 MB &#8211; 500 models '
                '&#8211; CC-BY License)</p>')
        libs = {lib['library']: lib['page_licence'] for lib in sh3d_libraries(page)}
        self.assertEqual(libs, {'Contributions': 'Free Art License', 'Scopia': 'CC-BY License'})

    def test_dodecagon_site_statement_with_nbsp(self):
        index = ('<div>All 3D models on this site are in the Public&nbsp;Domain unless specifically indicated otherwise.'
                 '</div><a href="http://creativecommons.org/publicdomain/zero/1.0?ref=chooser-v1">CC0 1.0 Universal</a>')
        self.assertEqual(licence_id(dodecagon_site_licence(index)), 'CC0-1.0')
        self.assertIsNone(dodecagon_site_licence('<div>All rights reserved.</div>'))

    def test_dodecagon_page_gives_the_form_file_id(self):
        self.assertEqual(dodecagon_libraries(DODECAGON_PAGE), [{'fileid': '5', 'file': 'DodecagonBathroom.sh3f'}])

    def test_licence_ids(self):
        self.assertEqual(licence_id('Public domain'), 'CC0-1.0')
        self.assertEqual(licence_id('CC0 1.0 Universal'), 'CC0-1.0')
        self.assertEqual(licence_id('Creative Commons Attribution 3.0 United States license'), 'CC-BY-3.0-US')
        self.assertEqual(licence_id('http://creativecommons.org/licenses/by/3.0/'), 'CC-BY-3.0')
        self.assertEqual(licence_id('Creative Commons Attribution 4.0 International'), 'CC-BY-4.0')
        # BlendSwap-CC-BY's LICENSE.TXT (11-03): 'use' is not the US port
        self.assertEqual(licence_id('This file is under the Creative Commons Attribution 3.0 license: '
                                    'http://creativecommons.org/licenses/by/3.0/ ... to make commercial use of the work'),
                         'CC-BY-3.0')

    def test_a_text_naming_both_cc0_and_cc_by_is_refused(self):
        # A CC-BY library whose text also mentions CC0 or the public domain must not be filed as CC0 (review 11-03).
        for text in ('Models are CC-BY 4.0; textures from cc0textures.com (CC0)',
                     'Creative Commons Attribution 3.0. Some textures are in the public domain.',
                     'CC0 1.0 Universal, except the sofa: Creative Commons Attribution 4.0'):
            self.assertIsNone(licence_id(text), text)

    def test_a_form_without_a_file_name_does_not_borrow_the_next(self):
        page = ('<form action="download" method="post"><input type="hidden" name="fileid" value="4">'
                '<button type="submit">Download</button></form>' + DODECAGON_PAGE)
        self.assertEqual(dodecagon_libraries(page), [{'fileid': '5', 'file': 'DodecagonBathroom.sh3f'}])

    def test_attribution_line_names_whom_the_licence_says(self):
        scopia = ('Attribution: If you want distribute this work, or derivates, you must attribute it to Space Mushrooms '
                  '<info@sweethome3d.com> and Scopia Visual Interfaces Systems, s.l. (http://www.scopia.es) (C) Copyright '
                  '2024 Space Mushrooms')
        self.assertEqual(attribution_line(scopia, ''), 'Space Mushrooms <info@sweethome3d.com> and Scopia Visual '
                                                        'Interfaces Systems, s.l. (http://www.scopia.es)')
        kator = 'Creative Commons Attribution 3.0 United States license  Copyright © 2003-2008 Andrew Kator & Jennifer Legaz'
        self.assertEqual(attribution_line('These files are under the CC BY 3.0 US license', kator),
                         'Copyright © 2003-2008 Andrew Kator & Jennifer Legaz')
        self.assertEqual(attribution_line('CC0 1.0 Universal', ''), '')

    def test_refused_licences(self):
        for text in ('Free Art License', 'CC-BY License', 'Creative Commons Attribution-NonCommercial 3.0',
                     'Creative Commons Attribution-ShareAlike 4.0', 'CC-BY-ND', ''):
            self.assertIsNone(licence_id(text), text)


if __name__ == '__main__':
    unittest.main()
