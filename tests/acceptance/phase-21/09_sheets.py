#!/usr/bin/env python3
# Brief numbers: in a sheet's SVG export the phase-1 house's 10.2 m south face at 1:50 is 204.0 mm +- 0.5; the PDF of
# the sheets has as many pages as sheets, each 914.4 x 609.6 mm (ARCH D, 2592 x 1728 pt) +- 0.5 mm, in sheet order.
# Also: a sheet is a CADCraft layout whose viewport of a plan view draws that view (the Floor Plan hides the E-LITE
# light, the Reflected Ceiling shows it), a schedule goes on a sheet, the master title block (ARCH-TITLE-BLOCK) carries
# each sheet's number, title, project, date and scale from its macros and follows a sheet-set edit, and
# sheet.edit "order" reorders the PDF.
# A stub fails: a missing layout or viewport, a wrong scale, plan viewports that ignore their view, a one-page PDF,
# title blocks without the macro values.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-sheet-command, p21-title-block, p21-plan-viewports, p21-plot-sheets, p21-schedule-tables, p20-plan-views, p20-rcp
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
P = {k: os.path.join(tmp, k) for k in ('plans.svg', 'ceiling.svg', 'elev.svg', 'set.pdf', 'set2.pdf', 'set.dxf')}
GREEN, LIGHT = '#2a7f3c', '#c8a000'
SET = HOUSE + [SH_WINDOWS[0],
               cmd('layer.set', {"name": "A-ELEV", "color": GREEN}),
               cmd('layer.new', {"name": "E-LITE", "color": LIGHT}),
               cmd('library.place', {"item": "light-ceiling", "at": [3.0, 5.5]}),
               cmd('elevation', {"side": "south", "dimensions": False}),                 # v0
               cmd('schedule', {"kind": "window"}),                                        # sc0
               cmd('sheetset', {"project": "Phase 21 House", "date": "2026-11-09"}),
               cmd('sheet', {"name": "Plans", "number": "A-101", "title": "Floor Plan", "size": "ARCH D",
                             "views": [{"view": "Floor Plan", "level": 0, "scale": "1:50", "at": [300, 330]}]}),
               cmd('sheet', {"name": "Ceiling", "number": "A-102", "title": "Reflected Ceiling Plan",
                             "views": [{"view": "Reflected Ceiling", "level": 0, "scale": "1:50", "at": [300, 330]}]}),
               cmd('sheet', {"name": "Elevations", "number": "A-201", "title": "Elevations",
                             "views": [{"view": "v0", "scale": "1:50", "at": [300, 420]},
                                       {"view": "sc0", "scale": 50, "at": [300, 150]}]})]
PLOTS = [cmd('plot', {"layout": "Plans", "output": "svg", "path": P['plans.svg']}),
         cmd('plot', {"layout": "Ceiling", "output": "svg", "path": P['ceiling.svg']}),
         cmd('plot', {"layout": "Elevations", "output": "svg", "path": P['elev.svg']}),
         cmd('plot', {"sheets": "all", "path": P['set.pdf']})]
rr = sh_run_ok(SET + PLOTS + [INSPECT21, cmd('sheetset', {"project": "Renamed House"}), INSPECT21,
                              cmd('sheet.edit', {"name": "Elevations", "order": 0}),
                              cmd('plot', {"sheets": "all", "path": P['set2.pdf']})], "the sheets", save=P['set.dxf'])
made_ = [r for r in rr if isinstance(r, dict) and 'sheet' in r and 'views' in r]
if [r.get('layout') for r in made_[:3]] != ['Plans', 'Ceiling', 'Elevations']:
    fail(f"each sheet reply names its layout: {[r.get('layout') for r in made_]}")
for r in made_[:3]:
    if r.get('paper') is None or abs(r['paper'][0] - 914.4) > 0.5 or abs(r['paper'][1] - 609.6) > 0.5:
        fail(f"sheet {r.get('sheet')} is ARCH D landscape (914.4 x 609.6 mm): {r.get('paper')}")
    for sv in r.get('views') or []:
        if not sv.get('viewport') or abs(num(sv.get('scale'), 'scale') - 50) > 1e-9:
            fail(f"sheet {r.get('sheet')}: every view gets a viewport at 1:50: {sv}")
ev = [sv.get('source', {}).get('kind') for sv in made_[2].get('views') or []]
if ev != ['elevation', 'schedule'] or [sv.get('number') for sv in made_[2]['views']] != [1, 2]:
    fail(f"the Elevations sheet holds view 1 (elevation) and view 2 (schedule): {made_[2].get('views')}")

# 10.2 m at 1:50 = 204.0 mm on the sheet.
ext = sh_svg_extent(P['elev.svg'], GREEN)
strokes, vb = sh_svg_strokes(P['elev.svg'])
if ext is None or vb is None:
    fail(f"the Elevations sheet's SVG draws nothing in A-ELEV's colour {GREEN} (or has no viewBox)")
mm = ext[0] * 914.4 / vb[0]
if abs(mm - 204.0) > 0.5:
    fail(f"the 10.2 m south face measures {mm:.2f} mm on the sheet, want 204.0 +- 0.5 (1:50)")
if LIGHT in sh_svg_colours(P['plans.svg']):
    fail("the Floor Plan viewport draws the E-LITE light (#c8a000), which that view hides")
if LIGHT not in sh_svg_colours(P['ceiling.svg']):
    fail("the Reflected Ceiling viewport does not draw the E-LITE light (#c8a000)")

# One PDF, one ARCH D page per sheet, in sheet order; reordered by sheet.edit.
insp = inspected(rr, 0)
sheets = sh_list(insp, 'sheets')
if [s.get('name') for s in sheets] != ['Plans', 'Ceiling', 'Elevations']:
    fail(f"arch.inspect lists the sheets in order: {[s.get('name') for s in sheets]}")
pdf = sh_pdf(P['set.pdf'])
if pdf['pages'] != len(sheets) or len(pdf['boxes']) != len(sheets):
    fail(f"the PDF has {pdf['pages']} pages ({len(pdf['boxes'])} media boxes) for {len(sheets)} sheets")
for w, h in pdf['boxes']:
    if abs(w * 25.4 / 72 - 914.4) > 0.5 or abs(h * 25.4 / 72 - 609.6) > 0.5:
        fail(f"a page is {w * 25.4 / 72:.1f} x {h * 25.4 / 72:.1f} mm, want 914.4 x 609.6")
if pdf['labels'] != ['Plans', 'Ceiling', 'Elevations']:
    fail(f"the pages follow the sheet order: {pdf['labels']}")
if sh_pdf(P['set2.pdf'])['labels'] != ['Elevations', 'Plans', 'Ceiling']:
    fail(f"after sheet.edit order 0 the Elevations sheet comes first: {sh_pdf(P['set2.pdf'])['labels']}")

# The master title block: one per sheet, its attributes the expanded macros.
WANT = {'Plans': ("A-101", "Floor Plan"), 'Ceiling': ("A-102", "Reflected Ceiling Plan"), 'Elevations': ("A-201", "Elevations")}
for s in sheets:
    tb = s.get('titleBlock') or {}
    n, t = WANT[s.get('name')]
    want = {"sheetNumber": n, "sheetTitle": t, "project": "Phase 21 House", "date": "2026-11-09", "scale": "1:50"}
    if {k: tb.get(k) for k in want} != want:
        fail(f"sheet {s.get('name')}'s title block: want {want}, got {tb}")
for s in sh_list(inspected(rr, 1), 'sheets'):
    if (s.get('titleBlock') or {}).get('project') != "Renamed House":
        fail(f"sheetset project reaches every title block: {s.get('name')} {s.get('titleBlock')}")
att = sh_dxf_attribs(P['set.dxf'])
for tag, vals in (('SHEETNO', {"A-101", "A-102", "A-201"}), ('PROJECT', {"Renamed House"}), ('SCALE', {"1:50"})):
    got = [v for t, v in att if t.upper() == tag]
    if set(got) != vals or len(got) != 3:
        fail(f"the saved DXF has three {tag} attributes {sorted(vals)}: {got}")
print(f"ok: 3 sheets on ARCH D layouts; the south face {mm:.2f} mm at 1:50; plan viewports draw their views; a {pdf['pages']}"
      f"-page PDF in sheet order (reordered after sheet.edit); title blocks from the macros")
print("PASS 09")
