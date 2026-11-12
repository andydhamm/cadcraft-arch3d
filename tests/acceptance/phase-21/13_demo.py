#!/usr/bin/env python3
# The phase-21 demo: the showcase house's full drawing set as one PDF. docs/arch3d/showcase/bilevel.json, then
# phase-20-showcase.json (its plan views), then phase-21-showcase.json replay with no error line, and
# `plot {"sheets": "all"}` writes one PDF with a page per sheet (ARCH D, in sheet order) holding: a cover with a 3D
# render (a camera image), the site plan, a floor plan, electrical plan and reflected ceiling plan for every floor
# level, the roof plan, four elevations, two sections, interior elevations of the kitchen and a bath (with its
# cabinets drawn), and the door, window, cabinet and room schedules, whose counts add up to the model's.
# A stub fails: missing files, refused steps, a missing kind of sheet content, a page count that is not the sheet count,
# blank pages, schedules that do not count the model.
# needs: p21-demo, p21-sheet-command, p21-sheet-camera, p21-title-block, p21-plan-viewports, p21-plot-sheets, p21-schedule-tables, p21-interior-elevation, p21-callouts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

SHOW = os.path.join(ROOT, 'docs', 'arch3d', 'showcase')
FILES = [os.path.join(SHOW, n) for n in ('bilevel.json', 'phase-20-showcase.json', 'phase-21-showcase.json')]
tmp = sh_tmp()
pdf_path = os.path.join(tmp, 'set.pdf')
for f in FILES:
    if not os.path.exists(f):
        fail(f"{os.path.relpath(f, ROOT)} is missing")
    try:
        steps = json.load(open(f, encoding='utf-8'))
    except ValueError as e:
        fail(f"{os.path.relpath(f, ROOT)} is not JSON: {e}")
    if not isinstance(steps, list) or not steps or not all(
            isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict) for st in steps):
        fail(f"{os.path.relpath(f, ROOT)} is a non-empty JSON array of {{\"cmd\", \"params\"}} steps")

args = cli_args('run') + ['--metric']
for f in FILES:
    args += ['--commands', f]
for c in [INSPECT21, cmd('plot', {"sheets": "all", "path": pdf_path})]:
    args += ['--cmd', c]
env = dict(os.environ)
env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
p = subprocess.run(args, capture_output=True, text=True, timeout=3600, env=env)
if 'panicked' in p.stderr or 'panicked' in p.stdout:
    fail(f"panic replaying the showcase: {(p.stderr + p.stdout)[-800:]}")
if p.returncode != 0 or errors(p.stderr):
    fail(f"a showcase step is refused (exit {p.returncode}): {' | '.join(errors(p.stderr)[:4]) or p.stderr[-400:]}")
rr = parse_rows(p.stdout)
insp = inspected(rr)
artifact(pdf_path, 'phase-21-drawing-set.pdf')
sheets = sh_list(insp, 'sheets')
if len(sheets) < 10:
    fail(f"a full drawing set has at least 10 sheets: {[s.get('name') for s in sheets]}")
views = {v.get('id'): v for v in sh_list(insp, 'views')}
views.update({v.get('name'): v for v in sh_list(insp, 'views')})
scheds = {s.get('id'): s for s in sh_list(insp, 'schedules')}
scheds.update({s.get('name'): s for s in sh_list(insp, 'schedules')})
rooms = {r.get('id'): r for r in objects_named(insp, 'rooms', 'rooms')}
src = [(sv.get('source') or {}) for s in sheets for sv in s.get('views') or [] if isinstance(sv, dict)]
kinds = [x.get('kind') for x in src]
names = [x.get('name') for x in src]
first = [(sv.get('source') or {}).get('kind') for sv in sheets[0].get('views') or []]
if 'camera' not in first or not any(sv.get('image') for sv in sheets[0].get('views') or []):
    fail(f"the first sheet is a cover with a 3D render (a camera image): {sheets[0]}")
for n in ('Site', 'Roof Plan'):
    if ('plan', n) not in [(x.get('kind'), x.get('name')) for x in src]:
        fail(f"the set holds the {n} plan view: {sorted(set(names), key=str)}")
floors = [l.get('name') for l in insp.get('levels') or [] if isinstance(l, dict) and l.get('kind', 'floor') == 'floor']
if len(floors) < 2:
    fail(f"the showcase has at least two floor levels: {insp.get('levels')}")
for lv in floors:
    for kind in ('Floor Plan', 'Electrical', 'Reflected Ceiling'):
        if f"{lv} {kind}" not in names:
            fail(f"the set holds {lv} {kind}: {sorted(set(names), key=str)}")
for kind, n in (('elevation', 4), ('section', 2), ('interior', 2)):
    if kinds.count(kind) < n:
        fail(f"the set holds {n} {kind} views: {kinds.count(kind)}")
inter = [views.get(x.get('name')) for x in src if x.get('kind') == 'interior']
rnames = [str((rooms.get((v or {}).get('room')) or {}).get('name')) for v in inter]
if 'Kitchen' not in rnames or not any('Bath' in r for r in rnames):
    fail(f"interior elevations of the kitchen and a bath: rooms {rnames}")
kv = inter[rnames.index('Kitchen')]
if not isinstance(kv, dict) or not sh_items(kv, 'lines', kind='cabinet'):
    fail("the kitchen's interior elevation draws its cabinets")
sk = sorted({(scheds.get(x.get('name')) or {}).get('kind') for x in src if x.get('kind') == 'schedule'}, key=str)
if sk != ['cabinet', 'door', 'room', 'window']:
    fail(f"the door, window, cabinet and room schedules are on sheets: {sk}")
ops = objects_named(insp, 'openings', 'openings')
cabs = objects_named(insp, 'cabinets', 'cabinets')
total = {'window': sum(1 for o in ops if o.get('kind') == 'window'),
         'door': sum(1 for o in ops if o.get('kind') in ('door', 'garageDoor', 'casedOpening')),
         'cabinet': len(cabs), 'room': len(rooms)}
for s in sh_list(insp, 'schedules'):
    k = s.get('kind')
    got = sum(int(num(c, f"{s.get('name')} count")) for _, c in sh_counts(s))
    if k in total and s.get('level') is None and got != total[k]:
        fail(f"the {k} schedule {s.get('name')} counts {got}, the model has {total[k]}")
pdf = sh_pdf(pdf_path)
if pdf['pages'] != len(sheets) or len(pdf['boxes']) != len(sheets):
    fail(f"one PDF page per sheet: {pdf['pages']} pages for {len(sheets)} sheets")
for w, h in pdf['boxes']:
    if abs(w * 25.4 / 72 - 914.4) > 0.5 or abs(h * 25.4 / 72 - 609.6) > 0.5:
        fail(f"every page is ARCH D 914.4 x 609.6 mm: {w * 25.4 / 72:.1f} x {h * 25.4 / 72:.1f}")
if pdf['labels'] != [s.get('layout') for s in sheets]:
    fail(f"the pages are in sheet order: {pdf['labels']} vs {[s.get('layout') for s in sheets]}")
if sum(1 for o in pdf['ops'] if o >= 20) < len(sheets):
    fail(f"every page draws its content (line operators per stream: {pdf['ops']})")
print(f"ok: {len(sheets)} sheets ({', '.join(str(s.get('number')) for s in sheets)}) in a {pdf['pages']}-page ARCH D PDF "
      f"with the cover render, plans per level, roof and site, elevations, sections, interiors and four schedules")
print("PASS 13")
