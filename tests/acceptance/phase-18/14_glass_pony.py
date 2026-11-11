#!/usr/bin/env python3
# Glass and pony walls (the brief's numbers: "a "Glass" wall is hit as `glass` at mid-height and nothing above 2.0;
# split at 1.07 into a solid lower part and glass above, a probe at 0.5 hits `wall`, at 1.5 `glass`, at 2.2 nothing").
# `wall {"type": "glass"}` makes a 10 mm, 2.0 m wall with no poché fill; `wall.split` at 1.07 with an interior lower
# type gives a 0.1 solid base and 10 mm glass above (probes 30 mm off the centre line tell the two thicknesses apart);
# removing the split gives one solid wall; bad types and heights are refused.
# A stub fails: a 0.2 m exterior wall, no glass part, probes above 2.0.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-scene-plumbing-1, p18-scene-plumbing-2, p18-scene-plumbing-3, p18-scene-plumbing-4, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p18lib.inc')).read())

G = cmd('wall', {"points": [[1, 1], [3, 1]], "type": "glass"})
P0 = [[2, 1, 1.0], [2, 1, 2.05], [2, 1, 2.2]]
P1 = [[2, 1, 0.5], [2, 1, 1.5], [2, 1, 2.2], [2, 1.03, 0.5], [2, 1.03, 1.5]]
SPLIT = cmd('wall.split', {"id": "w0", "height": 1.07, "lower": "interior"})

rr = run_ok([G, INSPECT18, probe_cmd(P0), SPLIT, INSPECT18, probe_cmd(P1), cmd('wall.split', {"id": "w0", "height": None}), INSPECT18, probe_cmd([[2, 1, 1.5]])],
            "a glass wall, split, unsplit")
w = by_id(walls_rows(inspected(rr, 0)), 'w0', 'walls')
if str(w.get('kind', '')).lower() != 'glass' or w.get('glass') is not True:
    fail(f"the wall's type is Glass and its row says glass true: {w}")
near(w.get('thickness'), 0.01, 1e-9, "a glass wall is 10 mm thick")
near(num(w.get('top'), 'top') - num(w.get('bottom'), 'bottom'), 2.0, 1e-9, "a glass wall is 2.0 m high by default")
if w.get('split') is not None:
    fail(f"no split yet: {w.get('split')}")
g = probed(rr, P0, nth=0, what="glass wall")
hit(g[0], 'glass', 'w0', "the glass wall at mid-height")
nohit(g[1], "above 2.0 (2.05)")
nohit(g[2], "above 2.0 (2.2)")
rep = reply_with(rr, 'split', -1, 'wall.split')
w = by_id(walls_rows(inspected(rr, 1)), 'w0', 'walls')
sp = w.get('split')
if not isinstance(sp, dict):
    fail(f"the split wall's row carries its split: {w}")
near(sp.get('height'), 1.07, 1e-9, "the split height")
if str(sp.get('upper', '')).lower() != 'glass':
    fail(f"the upper type defaults to Glass: {sp}")
near(sp.get('upperThickness'), 0.01, 1e-9, "the glass upper's thickness")
near(w.get('thickness'), 0.1, 1e-9, "the interior lower part is 0.1 thick")
g = probed(rr, P1, nth=1, what="pony wall")
hit(g[0], 'wall', 'w0', "the solid base at 0.5")
hit(g[1], 'glass', 'w0', "the glass above the split at 1.5")
nohit(g[2], "above 2.0")
hit(g[3], 'wall', 'w0', "30 mm off the centre line at 0.5 (the 0.1 base)")
nohit(g[4], "30 mm off the centre line at 1.5 (10 mm glass)")
w = by_id(walls_rows(inspected(rr, 2)), 'w0', 'walls')
if w.get('split') is not None:
    fail("height null removes the split")
hit(probed(rr, [[2, 1, 1.5]], nth=2, what="unsplit")[0], 'wall', 'w0', "one solid wall again at 1.5")
print("ok: glass 0.01 x 2.0; split 1.07 -> wall / glass / nothing; unsplit")

refused([cmd('wall', {"points": [[0, 0], [1, 0]], "type": "Glasss"})], "a misspelt type")
refused([G, cmd('wall.split', {"id": "w0", "height": 9})], "a split above the wall")
refused([G, cmd('wall.split', {"id": "w0", "height": 1.0, "upper": "marble"})], "an unknown upper type")
print("ok: refusals")
print("PASS 14")
