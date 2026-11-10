#!/usr/bin/env python3
# The showcase house rendered in every view style: tools/showcase/render_all.py replays bilevel.json and writes, with a
# manifest.json and contact-sheet.png: a plan per level and its Electrical view, four elevations, two sections, standard 3D
# from four saved cameras, the night render, the path-traced final, clay, technical illustration if built, and the glTF,
# IFC and OBJ exports. Every image shows the house: its luma sd is over 6 and, against its clear colour (the most common
# corner colour), at least 0.5 % of a line drawing's pixels (plans, electrical views, elevations, sections) and 5..99 % of
# a rendered view's pixels (cameras, night, path, clay, technical) differ (the old bar, luma sd 3 alone, passed an interior
# camera facing a blank wall: 2.3 %); the four camera views are four different pictures; the contact sheet holds a
# 320 x 180 px tile per image at least, and the manifest
# names the script it rendered (scriptSha256 = sha256 of bilevel.json). The exports validate: the GLB, and an IFC with
# the three storeys, IfcOutlet and IfcSwitchingDevice elements and at least four IfcWindow (check 10 places them).
# Revised 2026-11-09 (phase-15 checkpoint, decision 1): luma sd 3 alone was the bar, so blank views passed.
# needs: p15-showcase-house, p15-showcase-render, p15-cmd-export-view, p15-render3d-clay, p15-cmd-electrical-view, p15-render-plan-level, p16-rw-showcase-script, p16-rw-showcase-renders
import os, hashlib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())


def foreground(path, tol=8):
    w, h, nch, rows = png(path)
    px = lambda x, y: tuple(rows[y][x * nch:x * nch + min(3, nch)])
    cs = [px(0, 0), px(w - 1, 0), px(0, h - 1), px(w - 1, h - 1)]
    clear = max(cs, key=cs.count)
    fg = sum(1 for y in range(0, h, 2) for x in range(0, w, 2) if max(abs(a - b) for a, b in zip(px(x, y), clear)) > tol)
    return fg / (((h + 1) // 2) * ((w + 1) // 2)), w, h


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
sha = hashlib.sha256(open(SHOWCASE, 'rb').read()).hexdigest()
if man.get('scriptSha256') != sha:
    fail(f"the manifest names the script it rendered: scriptSha256 {man.get('scriptSha256')!r}, bilevel.json {sha}")
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
    if not os.path.exists(f):
        fail(f"{im['file']} ({im.get('kind')}) is written")
    frac, w, h = foreground(f)
    sd = luma_sd(f)
    lo = 0.005 if im.get('kind') in ('plan', 'electrical', 'elevation', 'section') else 0.05
    if not (lo <= frac <= 0.99) or sd <= 6:
        fail(f"{im['file']} ({im.get('kind')}) shows the house: {frac * 100:.1f} % of its pixels differ from the clear colour "
             f"({lo * 100:g}..99 %), luma sd {sd:.2f} (> 6)")
    artifact(f, 'showcase-' + im['file'])
cams = [os.path.join(out, im['file']) for im in kinds['camera']]
digests = {hashlib.sha256(open(f, 'rb').read()).hexdigest() for f in cams}
if len(digests) < len(cams):
    fail(f"the {len(cams)} camera views are different pictures: {len(digests)} distinct")
sheet = os.path.join(out, man.get('contactSheet', 'contact-sheet.png'))
if not os.path.exists(sheet) or luma_sd(sheet) <= 6:
    fail("the contact sheet is written and not blank")
sw, sh, _, _ = png(sheet)
if sw * sh < len(imgs) * 320 * 180:
    fail(f"the contact sheet ({sw} x {sh}) holds a tile for each of the {len(imgs)} images")
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
if len(of_type(inst, 'IFCWINDOW')) < 4:
    fail(f"the IFC has a window on each side: {len(of_type(inst, 'IFCWINDOW'))} IfcWindow")
print(f"ok: {len(imgs)} images ({', '.join(f'{k} {len(v)}' for k, v in sorted(kinds.items()))}), each showing the house, "
      f"a contact sheet, GLB/IFC/OBJ")
print("PASS 11")
