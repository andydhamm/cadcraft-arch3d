# OBJ with an MTL file: one `o` group per object named as the GLB nodes are (wall-w1), every material defined in the MTL,
# the same vertices as the GLB, faces that index them.
# needs: p12-io-obj
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); objp = os.path.join(tmp, 'house.obj'); glbp = os.path.join(tmp, 'house.glb')
export(HOUSE, objp)
export(HOUSE, glbp)
mtlp = os.path.join(tmp, 'house.mtl')
if not os.path.exists(mtlp):
    fail(f"exporting house.obj also writes house.mtl beside it; the directory holds {sorted(os.listdir(tmp))}")
obj = open(objp, encoding='utf-8').read().splitlines()
mtl = open(mtlp, encoding='utf-8').read().splitlines()
if not any(l.strip() == 'mtllib house.mtl' for l in obj):
    fail("house.obj names its material library: 'mtllib house.mtl'")
defined = {l.split(None, 1)[1].strip() for l in mtl if l.startswith('newmtl ')}
groups, used, nv, faces, bad_faces = [], set(), 0, 0, []
for l in obj:
    if l.startswith('o '):
        groups.append(l[2:].strip())
    elif l.startswith('usemtl '):
        used.add(l[7:].strip())
    elif l.startswith('v '):
        if len(l.split()) != 4:
            fail(f"a vertex line has x y z: {l!r}")
        nv += 1
    elif l.startswith('f '):
        faces += 1
        for tok in l.split()[1:]:
            k = int(tok.split('/')[0])
            if not (1 <= k <= nv):          # OBJ indices are 1-based and refer to vertices already read
                bad_faces.append(l)
                break
j, binc = glb(glbp)
nodes = [n.get('name') for n in j.get('nodes', []) if 'mesh' in n]
gverts = sum(j['accessors'][p['attributes']['POSITION']]['count'] for n in j.get('nodes', []) if 'mesh' in n
             for p in j['meshes'][n['mesh']]['primitives'])
print(f"house.obj: {len(groups)} groups, {nv} vertices, {faces} faces, {len(used)} materials ({len(defined)} in house.mtl); "
      f"GLB: {len(nodes)} mesh nodes, {gverts} vertices")
if sorted(groups) != sorted(nodes):
    fail(f"one `o` group per GLB mesh node, named alike: only in OBJ {sorted(set(groups) - set(nodes))[:10]}, "
         f"only in GLB {sorted(set(nodes) - set(groups))[:10]}")
if 'wall-w1' not in groups:
    fail("the group for wall w1 is named wall-w1")
if used - defined:
    fail(f"every usemtl is defined in house.mtl: missing {sorted(used - defined)}")
if nv != gverts:
    fail(f"the OBJ holds the GLB's vertices: {nv} vs {gverts}")
if bad_faces or faces == 0:
    fail(f"faces index existing vertices (1-based): {bad_faces[:3]}")
if not any(l.startswith('Kd ') for l in mtl):
    fail("house.mtl gives its materials a diffuse colour (Kd)")
print("PASS 01")
