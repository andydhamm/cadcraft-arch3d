#!/usr/bin/env python3
# The phase-14 demo: the phase-1 house's living room (room A) and kitchen (room B) furnished from the library (a sofa, a
# rug, a coffee table, an armchair, a bookcase, lamps, a plant; base and wall cabinets with a sink and faucet, a dishwasher
# in a gap, a range, a refrigerator, a dining table with chairs), restyled with both painters (oak and a blended tint on
# the living-room walls, marble counters, a colour in the kitchen, fabric on the sofa, knob pulls for every bar pull in
# the kitchen) in front of the sky backdrop, and Finalized (raster and path traced). The GLB passes the glTF validator,
# IfcOpenShell reads the IFC (both SKIP when the tool is missing), and the DXF reopens with every furnishing and paint.
# needs: p14-cmd-place, p14-cmd-paint, p14-arch3d-paint, p14-arch3d-cabinet-parts, p14-cmd-casework-parts, p14-r3d-symbol-materials, p14-r3d-backdrop, p14-cmd-backdrop, p14-io-glb-furnishing, p14-io-obj-ifc-furnishing, p14-io-dxf-furnishing, p14-inspect, p14-apps-library-hooks
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
P = lambda item, at, **k: cmd('library.place', dict({"item": item, "at": at}, **k))
LIVING = [P('sofa-futon', [0.55, 2.5], rotation=90),                    # f0, back on the west wall
          P('rug-oriental', [1.8, 2.5], rotation=90),                   # f1
          P('table-coffee-90', [1.6, 2.5]),                             # f2
          P('armchair', [2.9, 2.5], rotation=270),                      # f3
          P('bookcase', [2.0, 4.71]),                                   # f4, back on the north wall
          P('lamp-floor', [0.45, 4.5]),                                 # f5
          P('plant-large', [3.4, 0.6]),                                 # f6
          P('light-pendant', [1.6, 2.5]),                               # f7
          P('light-wall', [3.75, 2.5])]                                 # f8, on the partition's room-A face
# Kitchen along w1 (x 4..8, inner face y = 0.1): cb0 x 4.3..4.9, cb1 x 4.95..5.85 (the sink), a 0.6 m gap, cb2 x 6.45..7.05.
KITCHEN = [cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 0.6, "width": 0.6, "pull": "own:pull-bar-128"}),
           cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 1.4, "width": 0.9, "fronts": "drawerOverDoor", "doors": 2,
                           "pull": "own:pull-bar-128", "doorStyle": "shaker", "drawerStyle": "shaker"}),
           cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 2.75, "width": 0.6, "pull": "own:pull-bar-128"}),
           cmd('cabinet', {"kind": "wall", "wall": "w1", "offset": 0.6, "width": 0.6, "pull": "own:pull-bar-128"}),
           cmd('cabinet', {"kind": "wall", "wall": "w1", "offset": 2.75, "width": 0.6, "pull": "own:pull-bar-128"}),
           P('own:sink-kitchen', [5.4, 0.4]),                           # f9 into cb1
           P('faucet-kitchen-chrome', [5.4, 0.4]),                      # f10 on the sink
           P('own:dishwasher-60', [6.15, 0.4]),                         # f11 in the gap
           P('own:range-30', [7.555, 2.0], rotation=270),               # f12 against the east wall
           P('refrigerator-fridge-freezer', [7.4, 4.565]),              # f13 against the north wall
           P('table-dining-150', [5.6, 3.0]),                           # f14
           P('chair-dining-wood', [5.6, 2.3], rotation=180),            # f15
           P('chair-dining-wood', [5.6, 3.7]),                          # f16
           P('light-ceiling', [5.6, 3.0])]                              # f17
PAINT = [cmd('paint.material', {"material": "material-oak", "target": "wall/w5/left", "scope": "room"}),
         cmd('paint.material', {"material": "#c8a070", "target": "wall/w5/left", "scope": "room", "mode": "blend"}),
         cmd('paint.material', {"material": "#e8e0d0", "target": "wall/w1/left", "scope": "room"}),
         cmd('paint.material', {"material": "material-marble", "target": "cabinet/cb0/counter", "scope": "room"}),
         cmd('paint.material', {"material": "material-fabric", "target": "furnishing/f0/0", "scope": "object"}),
         cmd('paint.material', {"material": "wood", "target": "room/r1/floor", "scope": "component"}),
         cmd('paint.object', {"item": "own:pull-knob-32", "target": "cabinet/cb0/pull", "scope": "room"}),
         cmd('backdrop', {"kind": "image", "item": "own:backdrop-sky", "projection": "equirect"})]
DEMO = HOUSE1 + LIVING + KITCHEN + PAINT
saved = os.path.join(tmp, 'demo.dxf')
living, kitchen, traced = (os.path.join(tmp, n) for n in ('demo-living.png', 'demo-kitchen.png', 'demo-kitchen-path.png'))
LCAM = {"kind": "full", "eye": [3.7, 0.4, 1.6], "target": [0.5, 3.6, 0.7], "fov": 70}
KCAM = {"kind": "full", "eye": [4.4, 4.6, 1.7], "target": [6.6, 0.4, 0.8], "fov": 70}
rc, rr, so, se = run(DEMO + [INSPECT14,
                             cmd('render', {"out": living, "size": [800, 600], "style": "shaded", "final": "raster", "camera": LCAM}),
                             cmd('render', {"out": kitchen, "size": [800, 600], "style": "shaded", "final": "raster", "camera": KCAM}),
                             cmd('render', {"out": traced, "size": [400, 300], "final": "path", "spp": 16, "camera": KCAM})],
                     save=saved)
if was_refused(rc, se):
    fail(f"every demo step runs: {errors(se)[:5]}")
ins = inspected(rr)
fs = {f['id']: f for f in furnishings_of(ins)}
if len(fs) != 18:
    fail(f"18 furnishings placed, got {len(fs)}: {sorted(fs)}")
if (fs['f9'].get('host') or {}).get('kind') != 'cabinet' or (fs['f11'].get('host') or {}).get('kind') != 'gap' \
        or (fs['f8'].get('host') or {}).get('kind') != 'wall' or (fs['f10'].get('host') or {}).get('kind') != 'sink':
    fail(f"hosts: sink {fs['f9'].get('host')}, dishwasher {fs['f11'].get('host')}, wall light {fs['f8'].get('host')}, "
         f"faucet {fs['f10'].get('host')}")
cabs = {c['id']: c for c in objects_named(ins, 'cabinets', 'cabinets')}
if any(c.get('pull') != 'own:pull-knob-32' for c in cabs.values()):
    fail(f"every kitchen pull is a knob after paint.object: {[(k, c.get('pull')) for k, c in sorted(cabs.items())]}")
surf = surfaces_of(ins)
if surf['wall/w5/left'].get('material') != 'item:material-oak' or surf['wall/w5/left'].get('tint') != '#c8a070':
    fail(f"the living-room walls: {surf['wall/w5/left']}")
if surf.get('cabinet/cb1/counter', {}).get('material') != 'item:material-marble':
    fail(f"the kitchen counters (cb1's run): {surf.get('cabinet/cb1/counter')}")
for name, path in (('living', living), ('kitchen', kitchen), ('kitchen-path', traced)):
    if not os.path.exists(path):
        fail(f"the {name} render was written")
    w, h, nch, rows = png(path)
    flat = [0.2126 * r[x * nch] + 0.7152 * r[x * nch + 1] + 0.0722 * r[x * nch + 2] for r in rows[::4] for x in range(0, w, 4)]
    if statistics.pstdev(flat) < 10:
        fail(f"the {name} render shows the scene (luminance sd {statistics.pstdev(flat):.1f})")
    artifact(path, f'demo-{name}.png')
print("ok: the furnished, painted house renders (raster Finalize in both rooms, path traced in the kitchen)")

g, i = os.path.join(tmp, 'demo.glb'), os.path.join(tmp, 'demo.ifc')
export(DEMO, g)
export(DEMO, i)
j, binc = glb(g)
names = set(node_names(j))
missing = [k for k in fs if f'furnishing-{k}' not in names]
if missing:
    fail(f"GLB nodes missing for {missing}")
validate_glb(g)
hdr, inst = step_file(i)
n_ifc = sum(len(of_type(inst, e)) for e in ('IFCFURNITURE', 'IFCSANITARYTERMINAL', 'IFCELECTRICAPPLIANCE', 'IFCLIGHTFIXTURE'))
if n_ifc < 18 + 5:
    fail(f"IFC carries the 18 furnishings and 5 cabinets (and the casework fixtures): {n_ifc} elements")
py = os.path.join(CHECK_TOOLS, 'venv', 'bin', 'python')
if os.path.exists(py) and subprocess.run([py, '-c', 'import ifcopenshell, ifcopenshell.validate'], capture_output=True).returncode == 0:
    code = r'''
import sys, json, ifcopenshell, ifcopenshell.validate
f = ifcopenshell.open(sys.argv[1])
log = ifcopenshell.validate.json_logger()
ifcopenshell.validate.validate(f, log, express_rules=True)
names = sorted(e.Name for e in f.by_type('IfcProduct') if (e.Name or '').startswith('furnishing-'))
print(json.dumps({"schema": f.schema, "nissues": len(log.statements), "issues": [str(x)[:300] for x in log.statements][:8],
                  "furnishings": len(names)}))
'''
    p = subprocess.run([py, '-c', code, i], capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        fail(f"IfcOpenShell could not read demo.ifc: {p.stderr[-600:]}")
    r = json.loads(p.stdout.strip().splitlines()[-1])
    if r['schema'] != 'IFC4' or r['nissues'] or r['furnishings'] != 18:
        fail(f"IfcOpenShell reads demo.ifc as IFC4 without issues with 18 furnishings: {r}")
    print("ok: IfcOpenShell: IFC4, no issues, 18 furnishings")
else:
    print(f"SKIP IfcOpenShell: not installed ({py})")

rc, rr2, so2, se2 = run([INSPECT14], file=saved)
ins2 = inspected(rr2)
if len(furnishings_of(ins2)) != 18 or surfaces_of(ins2)['wall/w5/left'].get('material') != 'item:material-oak':
    fail("the demo DXF reopens with its furnishings and paints")
print("ok: the DXF round trip keeps furnishings and paints")
print("PASS 13")
