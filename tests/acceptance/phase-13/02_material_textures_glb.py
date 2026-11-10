# Materials in the GLB: each finish used is one embedded PNG texture (256 x 256, generated in Rust, the same bytes every
# export), the finished faces are their own nodes with a textured material and TEXCOORD_0, unfinished faces stay
# untextured, and the textures look like what they are (siding rows, brick courses). gltf-validator when installed.
# needs: p13-doc-finish, p13-texture-gen, p13-arch3d-finish-meshes, p13-glb-textures, p18-room-finishes
import os, hashlib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp()
cmds = ROOM + [cmd('roofauto', {"pitchRise": 6, "pitchRun": 12, "overhang": 0.3}),
               cmd('material', {"walls": "exterior", "finish": "siding"}),
               cmd('material', {"walls": ["w0"], "finish": "brick"}),
               cmd('material', {"roofs": "all", "finish": "shingles"}),
               cmd('material', {"rooms": ["r0"], "finish": "wood"})]
a, b = os.path.join(tmp, 'a.glb'), os.path.join(tmp, 'b.glb')
export(cmds, a)
export(cmds, b)
if open(a, 'rb').read() != open(b, 'rb').read():
    fail("two exports of one model are byte-identical (textures included)")
j, binc = glb(a)
# Phase 19 adds a normalTexture image per finish, named <base>-normal after the base-colour images; this check is about
# the base-colour textures, so it leaves those out.
imgs = {k: v for k, v in glb_images(j, binc).items() if not k.endswith('-normal')}
if sorted(imgs) != ['brick', 'shingles', 'siding', 'wood']:
    fail(f"one PNG per finish used, named by finish: {sorted(imgs)}")
if len({hashlib.sha256(v).hexdigest() for v in imgs.values()}) != 4:
    fail("the four textures are different images")
sam = j.get('samplers') or []
if not sam or sam[0].get('wrapS') != 10497 or sam[0].get('wrapT') != 10497:
    fail(f"textures repeat (wrapS/wrapT 10497): {sam}")
stats = {}
for name, data in imgs.items():
    w, h, g = png_bytes_grid(data, tmp, name + '.png')
    if (w, h) != (256, 256):
        fail(f"texture {name} is 256 x 256: {w} x {h}")
    stats[name] = grid_stats(g)
    print(f"texture {name}: sd {stats[name][0]:.1f}, vertical diff {stats[name][1]:.2f}, horizontal diff {stats[name][2]:.2f}")
for name, (sd, dv, dh) in stats.items():
    if sd < 4:
        fail(f"texture {name} has a pattern (luminance sd >= 4): {sd:.2f}")
sd, dv, dh = stats['siding']
if dv < 2 * dh:
    fail(f"siding is horizontal boards: rows differ more than columns (vertical {dv:.2f} vs horizontal {dh:.2f})")
sd, dv, dh = stats['brick']
if dh < 1 or dv < 1:
    fail(f"brick has mortar joints both ways: vertical {dv:.2f}, horizontal {dh:.2f}")
print("ok: four deterministic 256 x 256 PNG textures, siding horizontal, brick jointed")

# Finished faces are their own textured nodes; the rest stays untextured.
expect = {'wall-w0-exterior': 'brick', 'wall-w1-exterior': 'siding', 'wall-w2-exterior': 'siding',
          'wall-w3-exterior': 'siding', 'floor-r0': 'wood'}
for node, finish in expect.items():
    mat, prim = material_of_node(j, node)
    if texture_image_name(j, mat) != finish:
        fail(f"node {node} is textured with {finish}: material {mat.get('name')}, texture {texture_image_name(j, mat)}")
    if 'TEXCOORD_0' not in prim.get('attributes', {}):
        fail(f"node {node} has TEXCOORD_0")
    uv = j['accessors'][prim['attributes']['TEXCOORD_0']]
    if 'min' not in uv or 'max' not in uv or max(uv['max'][k] - uv['min'][k] for k in range(2)) < 1.0:
        fail(f"node {node}'s UVs span at least one texture repeat (accessor min/max): {uv.get('min')} {uv.get('max')}")
roof_nodes = [n['name'] for n in j['nodes'] if 'mesh' in n and re.fullmatch(r'roof-r0p\d+', n.get('name', ''))]
if len(roof_nodes) < 4 or any(texture_image_name(j, material_of_node(j, n)[0]) != 'shingles' for n in roof_nodes):
    fail(f"every roof plane is shingled: {roof_nodes}")
for node in ('wall-w0', 'wall-w1', 'ceiling-r0'):
    names = [n.get('name') for n in j['nodes'] if 'mesh' in n]
    if node not in names:
        continue
    mat, prim = material_of_node(j, node)
    if texture_image_name(j, mat) is not None:
        fail(f"{node} (the wall's other faces, the ceiling) is not textured: {mat}")
if 'wall-w0' not in [n.get('name') for n in j['nodes']]:
    fail("wall w0 keeps an untextured node 'wall-w0' for its other faces")
print("ok: exterior faces, roof planes and the wood floor are textured nodes; the rest is not")

# Without finishes or rooms the GLB carries no images (phase 18: a detected room's floor takes its type's finish, so
# the walls alone, with no room, are the model that uses none).
plain = os.path.join(tmp, 'plain.glb')
export(ROOM[:1], plain)
pj, _ = glb(plain)
if pj.get('images') or pj.get('textures'):
    fail("a model without finishes or rooms exports no images or textures")

validate_glb(a)
print("PASS 02")
