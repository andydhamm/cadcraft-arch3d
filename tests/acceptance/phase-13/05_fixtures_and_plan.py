# Fixtures (sink, lavatory, toilet, bathtub, shower, range, refrigerator, dishwasher) are parametric: each takes its
# catalogue size, sits where it is put, is a solid in 3D and a symbol in plan on its layer (cabinets on A-FLOR-CASE and
# A-FLOR-CASE-UPPR, plumbing on P-FLOR-FIXT, appliances on A-FLOR-APPL); the DXF carries the symbols as proxies and the
# objects in its record, so cabinets and fixtures survive save and open.
# needs: p13-doc-casework, p13-arch-casework-symbols, p13-arch3d-casework, p13-cmd-casework, p13-dxf-casework
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp(); saved = os.path.join(tmp, 'kitchen-bath.dxf')
BATH = [cmd('fixture', {"kind": "toilet", "at": [1.0, 2.0], "rotation": 0}),
        cmd('fixture', {"kind": "lavatory", "at": [2.0, 2.0]}),
        cmd('fixture', {"kind": "bathtub", "at": [3.0, 2.0]}),
        cmd('fixture', {"kind": "shower", "at": [0.6, 1.0]})]
P = [[1.65, 0.3, 0.924],   # 0 the sink rim on cb1's counter (0.914-0.934)
     [2.6, 3.7, 0.5],      # 1 the range (x 2.22-2.98, y 3.35-4.0)
     [1.1, 3.6, 1.7],      # 2 the refrigerator (to 1.78)
     [1.1, 3.6, 1.82],     # 3 above it: nothing
     [3.3, 0.3, 0.5],      # 4 the dishwasher
     [1.0, 2.1, 0.7],      # 5 the toilet's tank (0.2 deep at the back, 0.76 high)
     [1.0, 2.5, 0.6],      # 6 above its bowl (0.4 high): nothing
     [1.0, 2.5, 0.3],      # 7 the bowl
     [3.0, 2.3, 0.4],      # 8 the bathtub (0.51 high)
     [3.0, 2.3, 0.55]]     # 9 above it: nothing
rc, rr, so, se = run(ROOM + KITCHEN + BATH + [INSPECT_ALL, cmd('fixture.types', {}), cmd('build3d', {"probe": P})], save=saved)
fx = objects_named(inspected(rr), 'fixtures', 'fixtures')
kinds = [f.get('kind') for f in fx]
if kinds != ['sink', 'range', 'refrigerator', 'dishwasher', 'toilet', 'lavatory', 'bathtub', 'shower']:
    fail(f"eight fixtures in order fx0..fx7: {kinds}")
if [f.get('id') for f in fx] != [f'fx{i}' for i in range(8)]:
    fail(f"fixtures are labelled fx0..fx7: {[f.get('id') for f in fx]}")
size = {'sink': (0.8, 0.55, 0.02, 0.914), 'range': (0.76, 0.65, 0.914, 0.0), 'refrigerator': (0.76, 0.76, 1.78, 0.0),
        'dishwasher': (0.6, 0.6, 0.876, 0.0), 'toilet': (0.45, 0.7, 0.76, 0.0), 'lavatory': (0.5, 0.45, 0.85, 0.0),
        'bathtub': (1.52, 0.76, 0.51, 0.0), 'shower': (0.9, 0.9, 0.05, 0.0)}
layer = {'sink': 'P-FLOR-FIXT', 'lavatory': 'P-FLOR-FIXT', 'toilet': 'P-FLOR-FIXT', 'bathtub': 'P-FLOR-FIXT',
         'shower': 'P-FLOR-FIXT', 'range': 'A-FLOR-APPL', 'refrigerator': 'A-FLOR-APPL', 'dishwasher': 'A-FLOR-APPL'}
for f in fx:
    w, d, h, e = size[f['kind']]
    for k, v in (('width', w), ('depth', d), ('height', h), ('elevation', e)):
        near(f.get(k), v, 1e-9, f"{f['id']} ({f['kind']}) {k}")
    if f.get('layer') != layer[f['kind']]:
        fail(f"{f['id']} ({f['kind']}) is on {layer[f['kind']]}: {f.get('layer')}")
rng = by_id(fx, 'fx1', 'fixtures')
if max(abs(num(rng['at'][k], 'range at') - [2.6, 4.0][k]) for k in range(2)) > 1e-6 or \
        abs(((num(rng.get('rotation'), 'range rotation') - 180) + 180) % 360 - 180) > 1e-6:
    fail(f"the range on w2 backs onto its inner face at (2.6, 4.0), facing into the room (rotation 180): {rng}")
types = [r for r in rr if isinstance(r, dict) and isinstance(r.get('types'), list)]
if not types or sorted(t.get('kind') for t in types[-1]['types']) != sorted(size):
    fail(f"fixture.types lists the eight kinds: {types[-1] if types else rr[-3:]}")
print("ok: eight fixtures at their catalogue sizes, on their layers")

hits = probes(rr)['probe']
expect = {0: ('fx0', 'fixture'), 1: ('fx1', 'fixture'), 2: ('fx2', 'fixture'), 3: None, 4: ('fx3', 'fixture'),
          5: ('fx4', 'fixture'), 6: None, 7: ('fx4', 'fixture'), 8: ('fx6', 'fixture'), 9: None}
for i, e in expect.items():
    h = hits[i] if i < len(hits) else 'missing'
    got = None if h is None else (h.get('id'), h.get('part')) if isinstance(h, dict) else h
    if got != e:
        fail(f"probe {P[i]}: expected {e}, got {got}")
print("ok: fixtures are solids where they stand (the toilet's tank higher than its bowl)")

# Plan symbols in the DXF, on their layers, marked as casework proxies.
txt = dxf_text(saved)
on = {}
for kind, lay, pairs in entities(txt):
    if lay:
        on[lay] = on.get(lay, 0) + 1
for lay, least in (('A-FLOR-CASE', 4), ('A-FLOR-CASE-UPPR', 1), ('P-FLOR-FIXT', 5), ('A-FLOR-APPL', 3)):
    if on.get(lay, 0) < least:
        fail(f"the saved DXF draws casework symbols on {lay} (at least {least} entities): {on.get(lay, 0)}; layers {sorted(on)[:30]}")
if 'CADCRAFT-CASEWORK' not in txt:
    fail("casework proxies are marked CADCRAFT-CASEWORK")
print(f"ok: plan symbols on their layers: { {k: on.get(k) for k in ('A-FLOR-CASE', 'A-FLOR-CASE-UPPR', 'P-FLOR-FIXT', 'A-FLOR-APPL')} }")

# Reopen: the record brings back the same cabinets and fixtures; the proxies are not read as extra linework.
rc, rr2, so2, se2 = run([INSPECT_ALL], file=saved)
ins2 = inspected(rr2)
c1 = [(c.get('id'), c.get('kind'), c.get('at')) for c in objects_named(inspected(rr), 'cabinets', '')]
c2 = [(c.get('id'), c.get('kind'), c.get('at')) for c in objects_named(ins2, 'cabinets', 'after reopening')]
f2 = [(f.get('id'), f.get('kind')) for f in objects_named(ins2, 'fixtures', 'after reopening')]
if c1 != c2 or f2 != [(f['id'], f['kind']) for f in fx]:
    fail(f"save and open keep the cabinets and fixtures: {c2} / {f2}")
resaved = os.path.join(tmp, 'again.dxf')
run([], file=saved, save=resaved)
on2 = {}
for kind, lay, pairs in entities(dxf_text(resaved)):
    if lay:
        on2[lay] = on2.get(lay, 0) + 1
if on2.get('P-FLOR-FIXT', 0) != on.get('P-FLOR-FIXT', 0) or on2.get('A-FLOR-CASE', 0) != on.get('A-FLOR-CASE', 0):
    fail(f"open + save does not duplicate the proxies: {on.get('P-FLOR-FIXT')} -> {on2.get('P-FLOR-FIXT')} on P-FLOR-FIXT")
print("ok: cabinets and fixtures survive save and open; proxies are not doubled")
print("PASS 05")
