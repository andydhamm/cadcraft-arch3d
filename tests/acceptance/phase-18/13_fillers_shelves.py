#!/usr/bin/env python3
# Fillers and open shelves (the brief's "cabinet.fill closes the gap between a run's end and a wall with a filler up to
# 0.15 m wide; larger gaps are reported, never stretched" and "a wall cabinet with no doors and a shelf count (automatic
# or given), evenly spaced"). A run of two 0.6 base cabinets starting 0.05 from the west wall gets a 0.05 filler that
# joins its counter (area 1.25 x 0.625 = 0.78125; the filler, at the run's start, owns the counter, which a probe over
# the first cabinet finds); a lone cabinet 0.3 from the east wall is reported, not filled, and keeps its width. A 0.762
# open wall cabinet gets 2 shelves with three equal clear gaps; a probe hits a shelf and misses the hollow between them;
# `shelves: 4` gives four; 13 is refused.
# A stub fails: no filler, a stretched cabinet, no shelves rows, a solid box.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-fillers-shelves
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

RUN = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 0.45, "widths": [0.6, 0.6]})   # x 0.05..1.25
LONE = cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 5.5, "width": 0.6})           # x 5.1..5.7, wall at 6
FILL = cmd('cabinet.fill', {"run": "cb1"})

rr = run_ok(KITCHEN + [RUN, LONE, FILL, INSPECT18, probe_cmd([[0.025, 0.59, 0.5], [0.6, 0.3, 0.9]]), cmd('cabinet.fill', {"run": "cb2"}), INSPECT18], "fillers")
rep = reply_with(rr, 'fillers', 0, 'cabinet.fill')
fillers = rep.get('fillers') or []
if len(fillers) != 1:
    fail(f"one filler for the run against the west wall: {rep}")
FID = fillers[0]
gaps = {g.get('end'): g for g in rep.get('gaps') or [] if isinstance(g, dict)}
if set(gaps) != {'start', 'end'}:
    fail(f"the reply lists both ends: {rep}")
near(gaps['start'].get('gap'), 0.05, 1e-9, "the gap at the run's start")
if gaps['start'].get('filled') is not True or gaps['end'].get('gap') is not None or gaps['end'].get('filled') is not False:
    fail(f"the start gap is filled, the end has no wall within 1 m: {gaps}")
insp = inspected(rr, 0)
f = by_id(cabinets_of(insp), FID, 'cabinets')
if f.get('filler') is not True:
    fail(f"{FID} is a filler: {f}")
near(f.get('width'), 0.05, 1e-9, "the filler's width")
auto = [r for r in countertops_of(insp, True) if FID in (r.get('cabinets') or [])]
if len(auto) != 1 or sorted(auto[0]['cabinets']) != sorted(['cb0', 'cb1', FID]):
    fail(f"the filler joins the run's counter: {[(r.get('id'), r.get('cabinets')) for r in countertops_of(insp)]}")
near(auto[0].get('area'), 0.78125, 1e-9, "the counter over the run and its filler (1.25 x 0.625)")
g = probed(rr, [[0.025, 0.59, 0.5], [0.6, 0.3, 0.9]], what="filler")
hit(g[0], 'cabinet', FID, "the filler's panel (x 0..0.05, y 0.581..0.6)")
hit(g[1], 'counter', auto[0]['id'], "the counter over cb0 (the run's counter, built by its owner)")
rep2 = reply_with(rr, 'fillers', 1, 'cabinet.fill cb2')
if rep2.get('fillers'):
    fail(f"a 0.3 gap is reported, not filled: {rep2}")
gaps = {g.get('end'): g for g in rep2.get('gaps') or [] if isinstance(g, dict)}
near(gaps.get('end', {}).get('gap'), 0.3, 1e-9, "the lone cabinet's gap to the east wall")
if gaps.get('end', {}).get('filled') is not False or gaps.get('start', {}).get('gap') is not None:
    fail(f"the 0.3 gap is not filled and the start has no wall within 1 m: {gaps}")
near(by_id(cabinets_of(inspected(rr, 1)), 'cb2', 'cabinets').get('width'), 0.6, 1e-12, "the lone cabinet is never stretched")
print(f"ok: {FID} (0.05) fills the run and joins its counter; the 0.3 gap is reported")

OPEN = cmd('cabinet', {"kind": "wall", "wall": "w2", "offset": 2.0, "width": 0.762, "fronts": "open"})   # x 3.719..4.481, back y 4
rr = run_ok(KITCHEN + [OPEN, INSPECT18], "an open wall cabinet")
c = cabinets_of(inspected(rr))[0]
OID = c['id']
shelves = c.get('shelves')
if not isinstance(shelves, list) or len(shelves) != 2:
    fail(f"a 0.762 open wall cabinet gets 2 shelves (floor(0.724 / 0.25)): {shelves!r}")
bottom, top = num(c.get('elevation'), 'elevation'), num(c.get('elevation'), 'e') + num(c.get('height'), 'h')
bounds = [bottom + 0.019] + [z for s in shelves for z in (num(s.get('bottom'), 'shelf bottom'), num(s.get('top'), 'shelf top'))] + [top - 0.019]
gaps = [bounds[1] - bounds[0], bounds[3] - bounds[2], bounds[5] - bounds[4]]
if max(gaps) - min(gaps) > 1e-9:
    fail(f"the three clear gaps are equal: {gaps}")
mid1 = (shelves[0]['bottom'] + shelves[0]['top']) / 2
between = (shelves[0]['top'] + shelves[1]['bottom']) / 2
rr = run_ok(KITCHEN + [OPEN, probe_cmd([[4.1, 3.85, mid1], [4.1, 3.85, between]]),
                       cmd('cabinet.edit', {"id": OID, "shelves": 4}), INSPECT18], "shelf probes, then four shelves")
g = probed(rr, [[4.1, 3.85, mid1], [4.1, 3.85, between]], what="shelves")
hit(g[0], 'cabinet', OID, "the middle of shelf 1")
nohit(g[1], "halfway between the shelves (a hollow box)")
sh = by_id(cabinets_of(inspected(rr)), OID, 'cabinets').get('shelves') or []
if len(sh) != 4:
    fail(f"shelves: 4 gives four shelves: {sh}")
zs = [num(s['bottom'], 'b') for s in sh]
d = [b - a for a, b in zip(zs, zs[1:])]
if max(d) - min(d) > 1e-9:
    fail(f"four shelves evenly spaced: {zs}")
refused(KITCHEN + [OPEN, cmd('cabinet.edit', {"id": OID, "shelves": 13})], "13 shelves")
print("ok: 2 automatic shelves with equal gaps, 4 on demand, 13 refused")
print("PASS 13")
