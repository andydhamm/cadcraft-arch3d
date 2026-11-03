#!/usr/bin/env python3
# Furnishings in every export and in the incremental rebuild: GLB nodes furnishing-f<n> (the glTF validator accepts the
# file), IFC by category (IfcFurniture, IfcSanitaryTerminal, IfcElectricAppliance, IfcLightFixture) named as the nodes,
# DXF proxies on their layers and a round trip that keeps every furnishing with its mesh, OBJ groups; placing, editing or
# painting one furnishing rebuilds only it, and the incremental scene equals the full build.
# needs: p14-doc-furnishing, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-furnishing, p14-io-glb-furnishing, p14-io-obj-ifc-furnishing, p14-io-dxf-furnishing, p14-arch-furnishing-plan, p14-inspect, p14-cmd-paint, p14-arch3d-paint
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
FURN = room_cmds(0, 0, 6, 5) + [cmd('library.place', {"item": "sofa-futon", "at": [2, 4]}),          # f0 Furniture
                                cmd('library.place', {"item": "toilet", "at": [5.5, 0.5]}),           # f1 plumbing
                                cmd('library.place', {"item": "refrigerator-small", "at": [0.5, 0.5]}),   # f2 appliance
                                cmd('library.place', {"item": "light-ceiling", "at": [3, 2.5]}),      # f3 light
                                cmd('library.place', {"item": "plant-large", "at": [5, 4]})]          # f4 plant
IDS = [f'f{k}' for k in range(5)]
LAYERS = {'f0': 'I-FURN', 'f1': 'P-FLOR-FIXT', 'f2': 'A-FLOR-APPL', 'f3': 'E-LITE', 'f4': 'L-PLNT'}
glb_path, ifc_path, obj_path, dxf_path = (os.path.join(tmp, n) for n in ('h.glb', 'h.ifc', 'h.obj', 'h.dxf'))
rc, rr, so, se = run(FURN + [INSPECT14, cmd('export', {"path": glb_path}), cmd('export', {"path": ifc_path}),
                             cmd('export', {"path": obj_path})], save=dxf_path)
before = {f['id']: f for f in furnishings_of(inspected(rr))}
j, binc = glb(glb_path)
names = set(node_names(j))
for k in IDS:
    if f'furnishing-{k}' not in names:
        fail(f"GLB has no node furnishing-{k}")
validate_glb(glb_path)
print("ok: GLB nodes furnishing-f0..f4")

hdr, inst = step_file(ifc_path)
want = {'f0': 'IFCFURNITURE', 'f1': 'IFCSANITARYTERMINAL', 'f2': 'IFCELECTRICAPPLIANCE', 'f3': 'IFCLIGHTFIXTURE'}
for k, t in want.items():
    if f'furnishing-{k}' not in ifc_names(inst, t):
        fail(f"IFC: no {t} named furnishing-{k} ({t}: {sorted(ifc_names(inst, t))[:8]})")
print("ok: IFC elements by category, named as the GLB nodes")

obj = open(obj_path, encoding='utf-8', errors='replace').read()
for k in IDS:
    if f'\no furnishing-{k}\n' not in '\n' + obj:
        fail(f"OBJ has no group 'o furnishing-{k}'")
print("ok: OBJ groups")

text = dxf_text(dxf_path)
ents = entities(text)
for k, layer in LAYERS.items():
    if before[k].get('layer') != layer:
        fail(f"{k} on layer {before[k].get('layer')!r}, expected {layer}")
    if not any(e[1] == layer and e[0] == 'LWPOLYLINE' for e in ents):
        fail(f"the DXF has no LWPOLYLINE proxy on {layer}")
rc, rr, so, se = run([INSPECT14, cmd('export', {"path": os.path.join(tmp, 'h2.glb')})], file=dxf_path)
after = {f['id']: f for f in furnishings_of(inspected(rr))}
if sorted(after) != IDS:
    fail(f"after a DXF round trip: furnishings {sorted(after)} (proxies doubled or lost?)")
j2, binc2 = glb(os.path.join(tmp, 'h2.glb'))
for k in IDS:
    for f in ('item', 'at', 'rotation', 'elevation', 'size', 'layer'):
        if after[k].get(f) != before[k].get(f):
            fail(f"{k}.{f} after the round trip: {after[k].get(f)!r}, before {before[k].get(f)!r}")
    if geometry_key(node_positions(j, binc, f'furnishing-{k}')) != geometry_key(node_positions(j2, binc2, f'furnishing-{k}')):
        fail(f"furnishing-{k}'s mesh changed across the DXF round trip")
print("ok: DXF proxies on I-FURN, P-FLOR-FIXT, A-FLOR-APPL, E-LITE, L-PLNT; the round trip keeps every furnishing and mesh")

INC = {"incremental": True, "digest": True}
rc, rr, so, se = run(FURN + [cmd('build3d', INC),
                             cmd('furnishing.edit', {"id": "f0", "at": [2.5, 4], "rotation": 15}), cmd('build3d', INC),
                             cmd('paint.material', {"material": "material-fabric", "target": "furnishing/f0/0", "scope": "component"}),
                             cmd('build3d', INC),
                             cmd('library.place', {"item": "armchair", "at": [3, 1.5]}), cmd('build3d', INC),
                             cmd('build3d', {"digest": True})])
b = [r for r in rr if isinstance(r, dict) and 'rebuilt' in r and 'digest' in r]
if len(b) != 5:
    fail(f"expected five build3d replies with digests, got {len(b)}")
for k, what in ((1, 'editing f0'), (2, 'painting a part of f0'), (3, 'placing an armchair')):
    if b[k].get('incremental') is not True or b[k].get('rebuilt') != 1:
        fail(f"{what} rebuilt {b[k].get('rebuilt')} objects (incremental {b[k].get('incremental')}), expected 1")
if b[3].get('digest') != b[4].get('digest'):
    fail(f"the incremental scene's digest {b[3].get('digest')} differs from the full build's {b[4].get('digest')}")
print("ok: placing, editing or painting one furnishing rebuilds only it; incremental == full")
print("PASS 10")
