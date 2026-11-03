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


def make_sh3f(path, obj=CUBE_OBJ, mtl=CUBE_MTL, props=PROPS):
    with zipfile.ZipFile(path, 'w') as z:
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
        data = self.bin_chunk(glb)[bv.get('byteOffset', 0) + a.get('byteOffset', 0):][:a['count'] * 4]
        return list(struct.unpack(f'<{a["count"]}I', data))

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
