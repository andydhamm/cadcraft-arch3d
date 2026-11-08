#!/usr/bin/env python3
# The showcase house rendered in every view style: tools/showcase/render_all.py replays bilevel.json and writes, with a
# manifest.json and contact-sheet.png: a plan per level and its Electrical view, four elevations, two sections, standard 3D
# from four saved cameras, the night render, the path-traced final, clay, technical illustration if built, and the glTF,
# IFC and OBJ exports. Every image renders non-blank and the exports validate.
# needs: p15-showcase-house, p15-showcase-render, p15-cmd-export-view, p15-render3d-clay, p15-cmd-electrical-view, p15-render-plan-level
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

out = tempfile.mkdtemp()
tool = os.path.join(ROOT, 'tools', 'showcase', 'render_all.py')
if not os.path.exists(tool):
    fail("tools/showcase/render_all.py is missing")
exe = cli_args('run')
p = subprocess.run(['python3', tool, '--out', out, '--size', '480x360', '--spp', '16', '--cli-json', json.dumps(exe[:-1])],
                   capture_output=True, text=True, timeout=5400, cwd=ROOT,
                   env=dict(os.environ, CADCRAFT_LIBRARY_USER_DIR=tempfile.mkdtemp()))
if p.returncode != 0:
    fail(f"render_all.py exits 0: {p.returncode}\n{p.stdout[-600:]}\n{p.stderr[-900:]}")
man = json.load(open(os.path.join(out, 'manifest.json')))
imgs = man.get('images', [])
kinds = {}
for im in imgs:
    kinds.setdefault(im.get('kind'), []).append(im)
need = {'plan': 3, 'electrical': 2, 'elevation': 4, 'section': 2, 'camera': 4, 'night': 1, 'path': 1, 'clay': 1}
for k, n in need.items():
    if len(kinds.get(k, [])) < n:
        fail(f"at least {n} '{k}' image(s): {sorted((a, len(b)) for a, b in kinds.items())}")
for im in imgs:
    f = os.path.join(out, im['file'])
    if not os.path.exists(f) or luma_sd(f) < 3:
        fail(f"{im['file']} ({im.get('kind')}) renders non-blank")
    artifact(f, 'showcase-' + im['file'])
sheet = os.path.join(out, man.get('contactSheet', 'contact-sheet.png'))
if not os.path.exists(sheet) or luma_sd(sheet) < 3:
    fail("the contact sheet is written and not blank")
artifact(sheet, 'showcase-contact-sheet.png')
if 'technical' not in kinds and not man.get('technicalSkipped'):
    fail("technical illustration is rendered or recorded as skipped (technicalSkipped: true)")
ex = man.get('exports', {})
for k in ('glb', 'ifc', 'obj'):
    if k not in ex or not os.path.exists(os.path.join(out, ex[k])):
        fail(f"the {k} export: {ex}")
validate_glb(os.path.join(out, ex['glb']))
_, inst = step_file(os.path.join(out, ex['ifc']))
if len(of_type(inst, 'IFCBUILDINGSTOREY')) < 3 or not of_type(inst, 'IFCOUTLET') or not of_type(inst, 'IFCSWITCHINGDEVICE'):
    fail("the IFC has the storeys (lower, landing, main), IfcOutlet and IfcSwitchingDevice elements")
print(f"ok: {len(imgs)} images ({', '.join(f'{k} {len(v)}' for k, v in sorted(kinds.items()))}), a contact sheet, GLB/IFC/OBJ")
print("PASS 11")
