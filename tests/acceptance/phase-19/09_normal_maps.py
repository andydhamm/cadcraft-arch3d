# Normal maps, the data: a material can carry a tangent-space normal map; phase 13's procedural brick, siding, shingle
# and wood textures gain matching ones (256 x 256, linear, flat blue-ish on average, mortar joints tilting both ways for
# brick, laps varying by rows for siding); a catalogue material can bring its own (`normalMap` beside `file`, copied by
# library.install, used by paint.material); every one is exported as glTF `normalTexture`, the same bytes each export.
# needs: p19-normal-maps-1, p19-normal-maps-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()


def stats(px, ch):
    vals = [c[ch] for c in px]
    m = sum(vals) / len(vals)
    return m, math.sqrt(sum((v - m) ** 2 for v in vals) / len(vals))


# (a) brick and siding.
rr, (j, binc) = export_glb(HOUSE1 + [cmd('material', {"walls": "exterior", "finish": "brick"})], tmp, 'brick.glb')
maps = normal_maps(j, binc, tmp)
if 'brick' not in maps:
    fail(f"the brick material carries a normalTexture: materials with one: {sorted(maps)}")
name, w, h, px = maps['brick']
if (w, h) != (256, 256):
    fail(f"the brick normal map is 256 x 256: {w} x {h}")
(mr, sr), (mg, sg), (mb, sb) = stats(px, 0), stats(px, 1), stats(px, 2)
print(f"brick normal map {name}: mean r {mr:.1f} (sd {sr:.1f}), g {mg:.1f} (sd {sg:.1f}), b {mb:.1f}")
if abs(mr - 128) > 12 or abs(mg - 128) > 12 or mb < 220:
    fail(f"a tangent-space normal map averages near (128, 128, >= 220): ({mr:.1f}, {mg:.1f}, {mb:.1f})")
if sr < 4 or sg < 4:
    fail(f"the brick normal map is not flat (r and g sd >= 4): {sr:.2f}, {sg:.2f}")
tilt_x = sum(1 for c in px if abs(c[0] - 128) > 20) / len(px)
tilt_y = sum(1 for c in px if abs(c[1] - 128) > 20) / len(px)
if tilt_x < 0.01 or tilt_y < 0.01:
    fail(f"the mortar joints tilt the normal both ways (>= 1 % of texels each): x {tilt_x:.3f}, y {tilt_y:.3f}")
if name == 'brick' or not name.endswith('-normal'):
    fail(f"normal images are named after their base image with -normal (the base image keeps its name): {name}")
rr, (j2, binc2) = export_glb(HOUSE1 + [cmd('material', {"walls": "exterior", "finish": "siding"})], tmp, 'siding.glb')
maps2 = normal_maps(j2, binc2, tmp)
if 'siding' not in maps2:
    fail(f"the siding material carries a normalTexture: {sorted(maps2)}")
_, w, h, px = maps2['siding']
rowm = [sum(px[y * w + x][1] for x in range(w)) / w for y in range(h)]
colm = [sum(px[y * w + x][1] for y in range(h)) / h for x in range(w)]
sd = lambda v: math.sqrt(sum((a - sum(v) / len(v)) ** 2 for a in v) / len(v))
if not (sd(rowm) >= 2 and sd(rowm) >= 3 * sd(colm)):
    fail(f"siding laps run along rows: the map varies far more by rows than columns: {sd(rowm):.2f} vs {sd(colm):.2f}")

# (c) two exports give byte-identical normal images.
rr, (j3, binc3) = export_glb(HOUSE1 + [cmd('material', {"walls": "exterior", "finish": "brick"})], tmp, 'brick2.glb')
idx = lambda jj: [jj['textures'][m['normalTexture']['index']]['source'] for m in jj['materials'] if 'normalTexture' in m]
if [glb_image_bytes(j, binc, i) for i in idx(j)] != [glb_image_bytes(j3, binc3, i) for i in idx(j3)]:
    fail("two exports of one model carry byte-identical normal images")

# (b) a catalogue material with its own normal map.
kit = os.path.join(tmp, 'kit')
os.makedirs(kit)
write_png(os.path.join(kit, 'test-mat.png'), 64, 64, lambda x, y: (150, 90 + x, 60 + y))
NM = lambda x, y: (128 + (40 if x % 16 < 2 else 0), 128 + (30 if y % 8 == 0 else 0), 230)
write_png(os.path.join(kit, 'test-mat-normal.png'), 64, 64, NM)
json.dump({"version": 1, "items": [{"id": "test-mat", "name": "Test material", "category": "Materials", "symbolType": "material",
                                    "placement": None, "size": None, "keywords": ["test"], "source": "phase-19 check 09",
                                    "licence": "CC0-1.0", "author": "check", "attribution": "", "file": "test-mat.png",
                                    "normalMap": "test-mat-normal.png"}]}, open(os.path.join(kit, 'catalog.json'), 'w'))
rr, (j4, binc4) = export_glb(HOUSE1 + [cmd('library.install', {"from": kit}),
                                       cmd('paint.material', {"material": "test-mat", "target": "wall/w0/right", "scope": "component"})],
                             tmp, 'kit.glb')
want = [NM(x, 0) for x in range(64)]
hit = [k for k, (n, w, h, px) in normal_maps(j4, binc4, tmp).items() if (w, h) == (64, 64) and px[:64] == want]
if not hit:
    fail(f"the painted catalogue material exports its own normal map as normalTexture (64 x 64, same first row): "
         f"{[(k, v[1], v[2]) for k, v in normal_maps(j4, binc4, tmp).items()]}")
print("PASS 09")
