"""Tests for sh3d.py: Sweet Home 3D furniture libraries (.sh3f) to plain glTF 2.0 binaries.

Run from the repository root: python3 -m unittest tools/library/test_sh3d.py -v
"""
import json
import math
import os
import struct
import sys
import tempfile
import unittest
import zipfile
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sh3d import GLB_MAGIC, obj_to_glb, read_sh3f, write_glb  # noqa: E402
from sh3d import keywords_of  # noqa: E402


def png_2x2():
    """A 2 x 2 RGB PNG, built by hand."""
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    raw = b''.join(b'\x00' + bytes([255, 0, 0, 0, 255, 0]) for _ in range(2))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 2, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


CUBE_V = """v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1
vt 0 0
vt 1 0
vt 1 1
vt 0 1
"""
CUBE_OBJ = "mtllib cube.mtl\n" + CUBE_V + """usemtl red
f 1 4 3 2
f 1 2 6 5
f 2 3 7 6
f 3 4 8 7
usemtl texpng
f 5/1 6/2 7/3 8/4
usemtl texbmp
f 1/1 5/2 8/3 4/4
"""
CUBE_MTL = """newmtl red
Kd 1 0 0
newmtl texpng
Kd 0.5 0.5 0.5
map_Kd tex.png
newmtl texbmp
Kd 0 0 1
map_Kd tex.bmp
"""
PROPS = """# a test library
id#1=test#cube
name#1=Cube
category#1=Living room
model#1=/cube.obj
width#1=60
depth#1=55
height#1=85
creator#1=tester
"""


def make_sh3f(path, obj=CUBE_OBJ, mtl=CUBE_MTL, props=PROPS, extra=None):
    with zipfile.ZipFile(path, 'w') as z:
        for name, data in (extra or {}).items():
            z.writestr(name, data)
        z.writestr('PluginFurnitureCatalog.properties', props)
        z.writestr('cube.obj', obj)
        z.writestr('cube.mtl', mtl)
        z.writestr('tex.png', png_2x2())
        z.writestr('tex.bmp', b'BM' + bytes(60))


class GlbParts:
    @staticmethod
    def json_chunk(glb):
        n, kind = struct.unpack('<I4s', glb[12:20])
        assert kind == b'JSON'
        return json.loads(glb[20:20 + n])

    @staticmethod
    def bin_chunk(glb):
        n = struct.unpack('<I', glb[12:16])[0]
        off = 20 + n
        m, kind = struct.unpack('<I4s', glb[off:off + 8])
        assert kind == b'BIN\x00'
        return glb[off + 8:off + 8 + m]

    @staticmethod
    def index_accessors(doc):
        return [doc['accessors'][p['indices']] for m in doc['meshes'] for p in m['primitives']]

    def floats(self, glb, doc, acc_index):
        a = doc['accessors'][acc_index]
        bv = doc['bufferViews'][a['bufferView']]
        width = {'VEC2': 2, 'VEC3': 3, 'SCALAR': 1}[a['type']]
        data = self.bin_chunk(glb)[bv.get('byteOffset', 0) + a.get('byteOffset', 0):][:a['count'] * width * 4]
        flat = struct.unpack(f'<{a["count"] * width}f', data)
        return [flat[k:k + width] for k in range(0, len(flat), width)]

    def ints(self, glb, doc, acc_index):
        a = doc['accessors'][acc_index]
        bv = doc['bufferViews'][a['bufferView']]
        width, code = (2, 'H') if a['componentType'] == 5123 else (4, 'I')
        data = self.bin_chunk(glb)[bv.get('byteOffset', 0) + a.get('byteOffset', 0):][:a['count'] * width]
        return list(struct.unpack(f'<{a["count"]}{code}', data))

    def triangles(self, glb):
        """Every triangle as a sorted tuple of its three rounded positions, over all primitives."""
        doc = self.json_chunk(glb)
        tris = []
        for m in doc['meshes']:
            for p in m['primitives']:
                pos = self.floats(glb, doc, p['attributes']['POSITION'])
                idx = self.ints(glb, doc, p['indices'])
                for k in range(0, len(idx), 3):
                    tris.append(tuple(sorted(tuple(round(c, 6) for c in pos[i]) for i in idx[k:k + 3])))
        return sorted(tris)


class Sh3dTests(unittest.TestCase, GlbParts):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.sh3f = os.path.join(self.dir.name, 'test.sh3f')
        make_sh3f(self.sh3f)

    def tearDown(self):
        self.dir.cleanup()

    def test_reads_entries_in_metres(self):
        e = read_sh3f(self.sh3f)[0]
        self.assertEqual((e["id"], e["name"], e["category"]), ("test#cube", "Cube", "Living room"))
        self.assertEqual((e["width"], e["depth"], e["height"]), (0.6, 0.55, 0.85))
        self.assertEqual((e["model"], e["creator"]), ("cube.obj", "tester"))

    def test_glb_is_plain_gltf2(self):
        glb = obj_to_glb(zipfile.ZipFile(self.sh3f), "cube.obj")
        magic, version, length = struct.unpack("<4sII", glb[:12])
        self.assertEqual((magic, version, length), (GLB_MAGIC, 2, len(glb)))
        doc = self.json_chunk(glb)
        self.assertNotIn("extensionsUsed", doc)
        self.assertEqual({i["mimeType"] for i in doc["images"]}, {"image/png"})   # the BMP is dropped
        self.assertEqual(sum(a["count"] for a in self.index_accessors(doc)), 36)  # 6 quads -> 12 triangles
        colors = sorted(tuple(m['pbrMetallicRoughness']['baseColorFactor']) for m in doc['materials'])
        self.assertIn((0.0, 0.0, 1.0, 1.0), colors)                                # the BMP face keeps its Kd
        self.assertIn((1.0, 0.0, 0.0, 1.0), colors)

    def test_flat_normals_when_the_obj_has_none(self):
        glb = obj_to_glb(zipfile.ZipFile(self.sh3f), "cube.obj")
        doc = self.json_chunk(glb)
        normals = [n for m in doc['meshes'] for p in m['primitives'] for n in self.floats(glb, doc, p['attributes']['NORMAL'])]
        self.assertTrue(normals)
        for n in normals:
            self.assertAlmostEqual(math.sqrt(sum(c * c for c in n)), 1.0, places=5)

    def test_a_flat_polygon_shares_its_corners(self):
        p = os.path.join(self.dir.name, 'quad.sh3f')
        make_sh3f(p, obj="v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n")
        glb = obj_to_glb(zipfile.ZipFile(p), "cube.obj")
        doc = self.json_chunk(glb)
        prim = doc['meshes'][0]['primitives'][0]
        self.assertEqual(doc['accessors'][prim['attributes']['POSITION']]['count'], 4)   # one quad, two triangles
        self.assertEqual(doc['accessors'][prim['indices']]['count'], 6)

    def test_small_primitives_use_16_bit_indices(self):
        glb = obj_to_glb(zipfile.ZipFile(self.sh3f), "cube.obj")
        doc = self.json_chunk(glb)
        self.assertEqual({a['componentType'] for a in self.index_accessors(doc)}, {5123})
        big = write_glb([{'positions': [(float(k), 0.0, 0.0) for k in range(70000)], 'normals': [(0.0, 0.0, 1.0)] * 70000,
                          'uvs': None, 'indices': [0, 1, 69999], 'color': [1, 1, 1, 1], 'image': None}])
        self.assertEqual({a['componentType'] for a in self.index_accessors(self.json_chunk(big))}, {5125})

    def bounds(self, glb):
        doc = self.json_chunk(glb)
        acc = [doc['accessors'][p['attributes']['POSITION']] for m in doc['meshes'] for p in m['primitives']]
        return ([min(a['min'][k] for a in acc) for k in range(3)], [max(a['max'][k] for a in acc) for k in range(3)])

    def normals_at(self, glb, point):
        doc = self.json_chunk(glb)
        out = []
        for p in doc['meshes'][0]['primitives']:
            pos = self.floats(glb, doc, p['attributes']['POSITION'])
            nor = self.floats(glb, doc, p['attributes']['NORMAL'])
            out += [tuple(round(c, 4) for c in n) for q, n in zip(pos, nor) if all(abs(a - b) < 1e-6 for a, b in zip(q, point))]
        return sorted(set(out))

    def test_fit_puts_the_mesh_bottom_centre_at_its_size(self):
        # Sweet Home 3D meshes come in any unit with any origin (a table at x 19.55..20.45 m in centimetres, 11-03).
        p = os.path.join(self.dir.name, 'far.sh3f')
        far = ''.join(f'v {x + 1955} {y} {z + 755}\n' for x, y, z in
                      [(0, 0, 0), (90, 0, 0), (90, 44, 0), (0, 44, 0), (0, 0, 90), (90, 0, 90), (90, 44, 90), (0, 44, 90)])
        make_sh3f(p, obj=far + 'f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n')
        lo, hi = self.bounds(obj_to_glb(zipfile.ZipFile(p), 'cube.obj', fit=(0.9, 0.9, 0.44)))
        for got, want in zip(lo + hi, [-0.45, 0.0, -0.45, 0.45, 0.44, 0.45]):
            self.assertAlmostEqual(got, want, places=6)

    def test_a_smoothing_group_averages_its_corner_normals(self):
        # Two faces folded 90 degrees along x = 1 under `s 1` share the fold's normals; under `s off` they stay flat.
        fold = 'v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 1 0 -1\nv 1 1 -1\n'
        for group, want in (('s 1', [(0.7071, 0.0, 0.7071)]), ('s off', [(0.0, 0.0, 1.0), (1.0, 0.0, 0.0)])):
            p = os.path.join(self.dir.name, group.replace(' ', '') + '.sh3f')
            make_sh3f(p, obj=fold + group + '\nf 1 2 3 4\nf 2 5 6 3\n')
            self.assertEqual(self.normals_at(obj_to_glb(zipfile.ZipFile(p), 'cube.obj'), (1.0, 0.0, 0.0)), want, group)

    def test_index_65535_is_kept_for_primitive_restart(self):
        n = 65536            # glTF: an index may not be the component type's maximum, so 65535 needs uint32
        glb = write_glb([{'positions': [(float(k), 0.0, 0.0) for k in range(n)], 'normals': [(0.0, 0.0, 1.0)] * n,
                          'uvs': None, 'indices': [0, 1, 65535], 'color': [1, 1, 1, 1], 'image': None}])
        self.assertEqual({a['componentType'] for a in self.index_accessors(self.json_chunk(glb))}, {5125})

    def test_a_texture_path_from_another_machine_falls_back_to_its_name(self):
        p = os.path.join(self.dir.name, 'abs.sh3f')
        make_sh3f(p, mtl=CUBE_MTL.replace('map_Kd tex.png', 'map_Kd /Users/someone/3D/ TEXTURES/tex.png'))
        doc = self.json_chunk(obj_to_glb(zipfile.ZipFile(p), 'cube.obj'))
        self.assertEqual([i['mimeType'] for i in doc['images']], ['image/png'])

    def test_a_texture_override_fills_a_map_that_does_not_resolve(self):
        p = os.path.join(self.dir.name, 'rug.sh3f')
        make_sh3f(p, mtl=CUBE_MTL.replace('map_Kd tex.png', 'map_Kd Living room rug.jpg'))
        self.assertNotIn('images', self.json_chunk(obj_to_glb(zipfile.ZipFile(p), 'cube.obj')))
        doc = self.json_chunk(obj_to_glb(zipfile.ZipFile(p), 'cube.obj', texture='tex.png'))
        self.assertEqual([i['mimeType'] for i in doc['images']], ['image/png'])

    def test_keywords_drop_library_names(self):
        self.assertEqual(keywords_of('Dodecagon Chair Bar'), ['chair', 'bar'])
        self.assertEqual(keywords_of('Blend Swap, Appliance, Cooker'), ['appliance', 'cooker'])
        self.assertEqual(keywords_of('Sofa, Seat'), ['sofa', 'seat'])
        self.assertEqual(keywords_of(''), [])

    def test_reads_elevation_and_light_source_in_metres(self):
        p = os.path.join(self.dir.name, 'lamp.sh3f')
        make_sh3f(p, props=PROPS + 'elevation#1=140\nlightSourceX#1=30\nlightSourceY#1=27.5\nlightSourceZ#1=80\n'
                                   'lightSourceColor#1=#998764\nlightSourceDiameter#1=15\n')
        e = read_sh3f(p)[0]
        self.assertEqual(e['elevation'], 1.4)
        self.assertEqual(e['light'], {'position': [0.0, 0.0, 0.8], 'diameter': 0.15, 'colour': '#998764'})
        self.assertIsNone(read_sh3f(self.sh3f)[0]['light'])

    def test_negative_indices(self):
        pos_obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n"
        neg_obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf -4 -3 -2 -1\n"
        tris = []
        for k, obj in enumerate((pos_obj, neg_obj)):
            p = os.path.join(self.dir.name, f'q{k}.sh3f')
            make_sh3f(p, obj=obj)
            tris.append(self.triangles(obj_to_glb(zipfile.ZipFile(p), "cube.obj")))
        self.assertEqual(len(tris[0]), 2)
        self.assertEqual(tris[0], tris[1])


if __name__ == '__main__':
    unittest.main()
