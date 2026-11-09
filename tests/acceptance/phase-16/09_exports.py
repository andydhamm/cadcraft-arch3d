#!/usr/bin/env python3
# Exports: the phase-16 objects in every format. The house under the gable with S vaulted, a tray and the suspended stack
# in NW, a soffit in NE and a stem foundation: the GLB has a node per object (soffit-sf<n> boxed at x 7..9, y 7.3..7.9,
# z 2.1..2.4; foundation-fd<n>-<k> and footing-fd<n>-<k> per founded wall; ceiling-r<NW>-layer for the stack's upper
# layer; wall-w0-rise and wall-w4-rise for the walls rising to the vault) and validates; the OBJ has groups with the same
# names; the IFC has an IfcFooting footing-fd<n> and an IfcWall foundation-fd<n> with bodies and an IfcCovering
# soffit-sf<n> (.CEILING.), and a slab variant has IfcSlab .BASESLAB. per slab floor; the DXF carries dashed proxies (the
# soffit's ring on A-CLNG, four footing outlines on S-FNDN) that reopen without doubling, keeping every tray, soffit,
# foundation and room ceiling setting.
# A stub fails: no nodes, groups, IFC products or proxies named for the new objects.
# needs: p16-vault-walls, p16-trays, p16-soffits, p16-foundation-stem, p16-foundation-slab, p16-exports
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']
tmp = tempfile.mkdtemp()


def setup(kind):
    return HOUSE + [GABLE, cmd('room', {"id": S, "flatCeiling": False}),
                    cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3}),
                    cmd('room', {"id": NW, "ceilingStack": "suspended"}),
                    cmd('soffit', {"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1}),
                    cmd('foundation', {"type": kind})]


SETUP = setup('stem')
rr = run_ok(SETUP + [INSPECT16], "the set-up (vault, tray, suspended stack, soffit, stem foundation)")
S0 = reply_with(rr, 'soffit', -1, 'soffit').get('soffit')
FD = (replies_with_list(rr, 'foundations') or [{}])[-1].get('foundations') or [None]
FD = FD[0]
if not isinstance(S0, str) or not isinstance(FD, str):
    fail(f"the set-up's soffit and foundation labels: {S0!r}, {FD!r}")
before = inspected(rr)

g, i, o, d = (os.path.join(tmp, n) for n in ('p16.glb', 'p16.ifc', 'p16.obj', 'p16.dxf'))
run_ok(SETUP + [cmd('export', {"path": g}), cmd('export', {"path": i}), cmd('export', {"path": o})], "the exports", save=d)
for p in (g, i, o, d):
    if not os.path.exists(p):
        fail(f"{os.path.basename(p)} was written")

# 1. GLB nodes and the soffit's box.
NAMES = ([f'soffit-{S0}'] + [f'foundation-{FD}-{k}' for k in range(4)] + [f'footing-{FD}-{k}' for k in range(4)] +
         [f'ceiling-{NW}-layer', 'wall-w0-rise', 'wall-w4-rise'])
j, binc = glb(g)
names = set(node_names(j))
missing = [n for n in NAMES if n not in names]
if missing:
    fail(f"GLB nodes missing: {missing} (phase-16 nodes present: "
         f"{sorted(n for n in names if n and n.split('-')[0] in ('soffit', 'foundation', 'footing', 'slab', 'ceiling', 'wall'))[:40]})")
lo, hi = node_box(j, f'soffit-{S0}')
for k, (w0, w1) in enumerate(((7.0, 9.0), (7.3, 7.9), (2.1, 2.4))):
    near(lo[k], w0, 1e-3, f"soffit-{S0}'s box min {'xyz'[k]}")
    near(hi[k], w1, 1e-3, f"soffit-{S0}'s box max {'xyz'[k]}")
validate_glb(g)
print(f"ok: GLB nodes {', '.join(NAMES)}; soffit-{S0} boxed 7..9 x 7.3..7.9 x 2.1..2.4")

# 2. OBJ groups.
lines = {l.strip() for l in open(o, encoding='utf-8', errors='replace').read().splitlines()}
missing = [n for n in NAMES if f'o {n}' not in lines and f'g {n}' not in lines]
if missing:
    fail(f"OBJ groups missing: {missing}")
print("ok: OBJ groups with the same names")

# 3. IFC products.
hdr, inst = step_file(i)


def products(entity, name):
    """The IfcRoot products of `entity` named `name` (each with its nine or more attributes)."""
    return [a for _, a in of_type(inst, entity) if len(a) >= 9 and isinstance(a[2], str) and decode_step_string(a[2]) == name]


for entity, name in (('IFCFOOTING', f'footing-{FD}'), ('IFCWALL', f'foundation-{FD}'), ('IFCCOVERING', f'soffit-{S0}')):
    found = products(entity, name)
    if len(found) != 1:
        fail(f"IFC: one {entity} named {name}; {entity} names: {sorted(str(n) for n in ifc_names(inst, entity) if n)[:20]}")
    face_set_points(inst, found[0])
cov = products('IFCCOVERING', f'soffit-{S0}')[0]
if cov[8] != 'CEILING':
    fail(f"IFC: soffit-{S0} is an IfcCovering .CEILING.: predefined type {cov[8]!r}")
print(f"ok: IFC IfcFooting footing-{FD}, IfcWall foundation-{FD} with bodies, IfcCovering soffit-{S0} .CEILING.")
i2 = os.path.join(tmp, 'slab.ifc')
run_ok(setup('slab') + [cmd('export', {"path": i2})], "the slab variant's IFC")
_, inst = step_file(i2)
for rid in (S, NW, NE):
    found = products('IFCSLAB', f'slab-{rid}')
    if len(found) != 1 or found[0][8] != 'BASESLAB':
        fail(f"IFC (slab variant): one IfcSlab .BASESLAB. named slab-{rid}: "
             f"{[(decode_step_string(a[2]) if len(a) > 2 and isinstance(a[2], str) else None, a[8] if len(a) > 8 else None) for _, a in of_type(inst, 'IFCSLAB')][:12]}")
    face_set_points(inst, found[0])
print(f"ok: IFC (slab variant) IfcSlab .BASESLAB. slab-{S}, slab-{NW}, slab-{NE}")


# 4. DXF proxies, the round trip and a second save.
def lw_points(pairs):
    xs, ys = values(pairs, '10'), values(pairs, '20')
    try:
        return [(float(x), float(y)) for x, y in zip(xs, ys)]
    except ValueError:
        return []


def same_ring(pts, ring, tol=1e-6):
    if len(pts) > 1 and math.dist(pts[0], pts[-1]) <= tol:
        pts = pts[:-1]
    return len(pts) == len(ring) and all(any(math.dist(p, (q[0], q[1])) <= tol for p in pts) for q in ring)


def layer_counts(path):
    ents = entities(dxf_text(path))
    return ents, {l: sum(1 for e in ents if e[1] == l) for l in ('A-CLNG', 'S-FNDN')}


ents, counts = layer_counts(d)
ring = by_id(soffits_of(before), S0, 'arch.inspect soffits').get('polygon')
ring_box(ring, f"{S0}'s polygon")
if not any(e[0] == 'LWPOLYLINE' and e[1] == 'A-CLNG' and same_ring(lw_points(e[2]), ring) for e in ents):
    fail(f"the DXF has an LWPOLYLINE on A-CLNG through {S0}'s four corners {ring}: "
         f"{[lw_points(e[2]) for e in ents if e[1] == 'A-CLNG'][:4]}")
nf = sum(1 for e in ents if e[0] == 'LWPOLYLINE' and e[1] == 'S-FNDN')
if nf != 4:
    fail(f"the DXF has four LWPOLYLINE footing outlines on S-FNDN, got {nf}")
d2 = os.path.join(tmp, 'again.dxf')
after = inspected(run_ok([INSPECT16], "reopening the DXF", file=d, save=d2))
for key, rows_b, rows_a in (('trays', trays_of(before), trays_of(after)), ('soffits', soffits_of(before), soffits_of(after)),
                            ('foundations', foundations_of(before), foundations_of(after))):
    if json.dumps(rows_b, sort_keys=True) != json.dumps(rows_a, sort_keys=True):
        fail(f"the DXF round trip changed '{key}': {json.dumps(rows_b)[:300]} -> {json.dumps(rows_a)[:300]}")
    if not rows_b:
        fail(f"the set-up has {key}")
ceil = lambda insp: sorted((json.dumps(r.get('id')), json.dumps(r.get('flatCeiling')), json.dumps(r.get('ceilingStack'), sort_keys=True))
                           for r in objects_named(insp, 'rooms', 'rooms'))
if ceil(before) != ceil(after):
    fail(f"the DXF round trip changed the rooms' flatCeiling / ceilingStack: {ceil(before)} -> {ceil(after)}")
ents2, counts2 = layer_counts(d2)
if counts2 != counts:
    fail(f"saving the reopened DXF writes the same number of A-CLNG and S-FNDN entities: {counts} -> {counts2}")
print(f"ok: DXF proxies ({counts}); the round trip keeps trays, soffits, foundations and room ceilings; a second save "
      "writes no duplicates")
print("PASS 09")
