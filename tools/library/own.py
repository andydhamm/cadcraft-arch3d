"""The sample set's own items, made here (CC-0, ours): a bar pull, a knob pull and a sky backdrop.

    python3 tools/library/own.py --out DIR     writes pull-bar.glb, pull-knob.glb, backdrop-sky.png into DIR

The pulls are glTF (Y up, metres) facing +Z: their back is the plane z = 0, where they meet the cabinet front.
"""
import argparse
import math
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sh3d import write_glb  # noqa: E402

SOURCE = 'arch3d tools/library/own.py'
AUTHOR = 'arch3d (made for the sample set)'
METAL = [0.62, 0.62, 0.64, 1.0]


def box(x0, x1, y0, y1, z0, z1):
    """An axis-aligned box: 6 faces of 4 vertices each, flat normals, counter-clockwise from outside."""
    pos, nor, idx = [], [], []
    faces = [((1, 0, 0), [(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)]),
             ((-1, 0, 0), [(x0, y0, z1), (x0, y1, z1), (x0, y1, z0), (x0, y0, z0)]),
             ((0, 1, 0), [(x0, y1, z0), (x0, y1, z1), (x1, y1, z1), (x1, y1, z0)]),
             ((0, -1, 0), [(x0, y0, z1), (x0, y0, z0), (x1, y0, z0), (x1, y0, z1)]),
             ((0, 0, 1), [(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]),
             ((0, 0, -1), [(x1, y0, z0), (x0, y0, z0), (x0, y1, z0), (x1, y1, z0)])]
    for n, quad in faces:
        k = len(pos)
        pos += quad
        nor += [n] * 4
        idx += [k, k + 1, k + 2, k, k + 2, k + 3]
    return pos, nor, idx


def cylinder(r, z0, z1, segments=16):
    """A cylinder along Z with radius r between z0 and z1: side with radial normals, two flat caps."""
    pos, nor, idx = [], [], []
    ring = [(r * math.cos(2 * math.pi * k / segments), r * math.sin(2 * math.pi * k / segments)) for k in range(segments)]
    for k in range(segments):
        (xa, ya), (xb, yb) = ring[k], ring[(k + 1) % segments]
        na = (xa / r, ya / r, 0.0)
        nb = (xb / r, yb / r, 0.0)
        b = len(pos)
        pos += [(xa, ya, z0), (xb, yb, z0), (xb, yb, z1), (xa, ya, z1)]
        nor += [na, nb, nb, na]
        idx += [b, b + 1, b + 2, b, b + 2, b + 3]
    for z, n, flip in ((z1, (0.0, 0.0, 1.0), False), (z0, (0.0, 0.0, -1.0), True)):
        c = len(pos)
        pos.append((0.0, 0.0, z))
        nor.append(n)
        for x, y in ring:
            pos.append((x, y, z))
            nor.append(n)
        for k in range(segments):
            a, b = c + 1 + k, c + 1 + (k + 1) % segments
            idx += [c, b, a] if flip else [c, a, b]
    return pos, nor, idx


def prim(name, part, color=METAL):
    pos, nor, idx = part
    return {'positions': pos, 'normals': nor, 'uvs': None, 'indices': idx, 'color': color, 'image': None, 'name': name}


def pull_bar(centres=0.128):
    """A bar pull: two 8 mm posts `centres` apart (centre to centre), 25 mm proud, under a 10 mm bar 10 mm longer than
    the centres at each end."""
    h = centres / 2
    return write_glb([prim('bar', box(-h - 0.01, h + 0.01, -0.005, 0.005, 0.025, 0.035)),
                      prim('post-a', box(-h - 0.004, -h + 0.004, -0.004, 0.004, 0.0, 0.025)),
                      prim('post-b', box(h - 0.004, h + 0.004, -0.004, 0.004, 0.0, 0.025))], name='pull-bar')


def pull_knob(diameter=0.032):
    """A round knob `diameter` across, 25 mm proud, on a 12 mm stem."""
    return write_glb([prim('knob', cylinder(diameter / 2, 0.008, 0.025)),
                      prim('stem', cylinder(0.006, 0.0, 0.008))], name='pull-knob')


def png(width, height, row):
    """An RGB PNG whose row y is the 3-byte colour row(y) across its width."""
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    raw = b''.join(b'\x00' + bytes(row(y)) * width for y in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def sky(width=1024, height=512, top=(74, 128, 196), horizon=(214, 228, 240)):
    """A two-colour sky: `top` at the top row, `horizon` at the bottom, linear between."""
    def row(y):
        t = y / (height - 1)
        return tuple(round(a + (b - a) * t) for a, b in zip(top, horizon))
    return png(width, height, row)


def own_items(out):
    """Write the own items into `out`; their catalog entries (sizes in metres, width x depth x height, Z up)."""
    os.makedirs(out, exist_ok=True)
    files = {'pull-bar.glb': pull_bar(), 'pull-knob.glb': pull_knob(), 'backdrop-sky.png': sky()}
    for name, data in files.items():
        with open(os.path.join(out, name), 'wb') as f:
            f.write(data)
    common = {'source': SOURCE, 'licence': 'CC0-1.0', 'author': AUTHOR}
    return [
        {'id': 'own:pull-bar-128', 'name': 'Bar pull, 128 mm', 'category': 'Cabinet parts', 'symbolType': 'pull',
         'placement': 'in-cabinet', 'size': [0.148, 0.035, 0.01], 'keywords': ['pull', 'handle', 'bar', 'modern'],
         'file': 'pull-bar.glb', **common},
        {'id': 'own:pull-knob-32', 'name': 'Knob, 32 mm', 'category': 'Cabinet parts', 'symbolType': 'pull',
         'placement': 'in-cabinet', 'size': [0.032, 0.025, 0.032], 'keywords': ['pull', 'knob', 'round'],
         'file': 'pull-knob.glb', **common},
        {'id': 'own:backdrop-sky', 'name': 'Sky gradient', 'category': 'Backdrops', 'symbolType': 'backdrop',
         'placement': None, 'size': None, 'keywords': ['sky', 'gradient', 'backdrop'], 'file': 'backdrop-sky.png',
         **common},
    ]


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--out', required=True)
    for item in own_items(ap.parse_args().out):
        print(item['file'], item['id'])
