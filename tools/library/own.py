"""The sample set's own items, made here (CC-0, ours): a bar pull, a knob pull, a 30-inch range, a kitchen sink and a
dishwasher at the phase-13 casework sizes, and a sky backdrop.

    python3 tools/library/own.py --out DIR     writes FILES into DIR

Every symbol is glTF (Y up, metres), front towards +Z, bottom-centred at its catalog size like the converted ones.
"""
import argparse
import math
import os
import struct
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sh3d import fit_primitives, write_glb  # noqa: E402

SOURCE = 'arch3d tools/library/own.py'
AUTHOR = 'arch3d (made for the sample set)'
METAL = [0.62, 0.62, 0.64, 1.0]
FILES = ('pull-bar.glb', 'pull-knob.glb', 'range-30.glb', 'sink-kitchen.glb', 'dishwasher-60.glb',
         'backdrop-sky.png')   # what own_items writes


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
    return [prim('bar', box(-h - 0.01, h + 0.01, -0.005, 0.005, 0.025, 0.035)),
            prim('post-a', box(-h - 0.004, -h + 0.004, -0.004, 0.004, 0.0, 0.025)),
            prim('post-b', box(h - 0.004, h + 0.004, -0.004, 0.004, 0.0, 0.025))]


def pull_knob(diameter=0.032):
    """A round knob `diameter` across, 25 mm proud, on a 12 mm stem."""
    return [prim('knob', cylinder(diameter / 2, 0.008, 0.025)), prim('stem', cylinder(0.006, 0.0, 0.008))]


def upright(part, cx=0.0, cz=0.0):
    """A Z-axis part stood on the Y axis (-90 degrees about X: (x, y, z) -> (x, z, -y), handedness kept), moved by cx, cz."""
    pos, nor, idx = part
    return ([(x + cx, z, -y + cz) for x, y, z in pos], [(x, z, -y) for x, y, z in nor], idx)


def range_cooker(width=0.762, depth=0.66, height=0.91):
    """A freestanding 30-inch range, front at +Z: a white body, a black glass top with four burners, an oven window and a
    handle."""
    w, d = width / 2, depth / 2
    white, black, steel = [0.92, 0.92, 0.9, 1.0], [0.05, 0.05, 0.06, 1.0], METAL
    burners = []
    for x in (-w / 2, w / 2):
        for z in (-d / 2, d / 2):
            burners.append(upright(cylinder(0.085, 0.0, 0.004, 24), x, z))
    pos, nor, idx = [], [], []
    for bp, bn, bi in burners:              # the four burners as one primitive, lifted onto the top
        k = len(pos)
        pos += [(x, y + height, z) for x, y, z in bp]
        nor += bn
        idx += [k + i for i in bi]
    return [prim('body', box(-w, w, 0.0, height - 0.02, -d, d), white),
            prim('top', box(-w, w, height - 0.02, height, -d, d), black),
            prim('burners', (pos, nor, idx), [0.18, 0.18, 0.2, 1.0]),
            prim('oven-window', box(-w + 0.1, w - 0.1, 0.22, 0.6, d, d + 0.004), black),
            prim('handle', box(-w + 0.08, w - 0.08, 0.68, 0.7, d + 0.004, d + 0.03), steel)]


def merged(parts):
    """Several (positions, normals, indices) parts as one."""
    pos, nor, idx = [], [], []
    for p, n, i in parts:
        k = len(pos)
        pos += p
        nor += n
        idx += [k + j for j in i]
    return pos, nor, idx


def sink_kitchen(width=0.8, depth=0.55, height=0.2, bowl=(0.72, 0.47), flange=0.01):
    """A drop-in stainless kitchen sink at the phase-13 sink's size: a flange `width` x `depth` on the counter (its top
    is the item's top) around one bowl `bowl` across and `height` deep below it, with a drain. The counter cut-out sits
    between the bowl and the flange's outer edge."""
    w, d, bw, bd, t = width / 2, depth / 2, bowl[0] / 2, bowl[1] / 2, 0.008
    top, under = height, height - flange
    rim = merged([box(-w, w, under, top, bd, d), box(-w, w, under, top, -d, -bd),
                  box(-w, -bw, under, top, -bd, bd), box(bw, w, under, top, -bd, bd)])
    basin = merged([box(-bw, bw, 0.0, t, -bd, bd), box(-bw, -bw + t, 0.0, under, -bd, bd), box(bw - t, bw, 0.0, under, -bd, bd),
                    box(-bw, bw, 0.0, under, -bd, -bd + t), box(-bw, bw, 0.0, under, bd - t, bd)])
    return [prim('flange', rim), prim('bowl', basin), prim('drain', upright(cylinder(0.04, 0.0, 0.002, 24), 0.0, 0.0),
                                                             [0.3, 0.3, 0.32, 1.0])]


def dishwasher(width=0.6, depth=0.6, height=0.876):
    """A built-under dishwasher at the phase-13 size (0.6 x 0.6 x 0.876, under the 0.876 counter underside; no top of
    its own, the counter runs over it): a stainless door with a black control strip and a bar handle, a recessed kick."""
    w, d = width / 2, depth / 2
    steel, black = [0.72, 0.72, 0.74, 1.0], [0.05, 0.05, 0.06, 1.0]
    return [prim('body', merged([box(-w, w, 0.1, height, -d, d - 0.02), box(-w, w, 0.0, 0.1, -d, d - 0.07)]), steel),
            prim('controls', box(-w, w, height - 0.07, height, d - 0.02, d - 0.01), black),
            prim('door', box(-w, w, 0.1, height - 0.07, d - 0.02, d - 0.01), steel),
            prim('handle', box(-w + 0.1, w - 0.1, height - 0.11, height - 0.09, d - 0.01, d), METAL)]


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
    common = {'source': SOURCE, 'licence': 'CC0-1.0', 'author': AUTHOR,
              'attribution': f'made for the arch3d library sample set by {AUTHOR}, CC0-1.0'}
    items = [
        {'id': 'own:pull-bar-128', 'name': 'Bar pull, 128 mm', 'category': 'Cabinet parts', 'symbolType': 'pull',
         'placement': 'in-cabinet', 'size': [0.148, 0.035, 0.01], 'keywords': ['pull', 'handle', 'bar', 'modern'],
         'file': 'pull-bar.glb', **common},
        {'id': 'own:pull-knob-32', 'name': 'Knob, 32 mm', 'category': 'Cabinet parts', 'symbolType': 'pull',
         'placement': 'in-cabinet', 'size': [0.032, 0.025, 0.032], 'keywords': ['pull', 'knob', 'round'],
         'file': 'pull-knob.glb', **common},
        {'id': 'own:range-30', 'name': 'Range, 30 in', 'category': 'Appliances and plumbing fixtures', 'symbolType': 'range',
         'placement': 'floor', 'size': [0.762, 0.69, 0.914], 'keywords': ['range', 'stove', 'oven', 'cooker', 'kitchen'],
         'file': 'range-30.glb', **common},
        {'id': 'own:sink-kitchen', 'name': 'Kitchen sink, single bowl', 'category': 'Appliances and plumbing fixtures',
         'symbolType': 'sink', 'placement': 'counter', 'size': [0.8, 0.55, 0.2], 'cutout': [0.76, 0.51],
         'keywords': ['sink', 'kitchen', 'stainless', 'drop-in'], 'file': 'sink-kitchen.glb', **common},
        {'id': 'own:dishwasher-60', 'name': 'Dishwasher, 60 cm', 'category': 'Appliances and plumbing fixtures',
         'symbolType': 'dishwasher', 'placement': 'floor', 'size': [0.6, 0.6, 0.876],
         'keywords': ['dishwasher', 'kitchen', 'appliance', 'built-under'], 'file': 'dishwasher-60.glb', **common},
        {'id': 'own:backdrop-sky', 'name': 'Sky gradient', 'category': 'Backdrops', 'symbolType': 'backdrop',
         'placement': None, 'size': None, 'keywords': ['sky', 'gradient', 'backdrop'], 'file': 'backdrop-sky.png',
         **common},
    ]
    build = {'pull-bar.glb': pull_bar, 'pull-knob.glb': pull_knob, 'range-30.glb': range_cooker,
             'sink-kitchen.glb': sink_kitchen, 'dishwasher-60.glb': dishwasher}
    assert tuple(i['file'] for i in items) == FILES, 'own.FILES lists what own_items writes'
    for i in items:
        data = sky() if i['file'] == 'backdrop-sky.png' else \
            write_glb(fit_primitives(build[i['file']](), i['size']), name=i['file'][:-4])
        with open(os.path.join(out, i['file']), 'wb') as f:
            f.write(data)
    return items


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--out', required=True)
    for item in own_items(ap.parse_args().out):
        print(item['file'], item['id'])
