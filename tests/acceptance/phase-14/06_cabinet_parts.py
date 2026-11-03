#!/usr/bin/env python3
# Cabinet parts: door and drawer styles (slab, shaker, raised panel) made per front, each front the same size whatever its
# style; pulls (sample-set items) on every door and drawer at the set position (doors 0.05 m in from the free edge and
# 0.075 m from the top, or the bottom on wall cabinets; drawers centred), as GLB node cabinet-cb<n>-pull standing proud of
# the front; a cabinet-part item clicked over a cabinet swaps its pull or style instead of placing anything.
# needs: p14-doc-cabinet-parts, p14-doc-furnishing, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-arch-place, p14-cmd-place, p14-arch3d-cabinet-parts, p14-io-glb-furnishing, p14-inspect, p14-cmd-casework-parts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()


def base(style, extra=None):
    p = {"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.9, "fronts": "drawerOverDoor", "doors": 2,
         "doorStyle": style, "drawerStyle": style}
    p.update(extra or {})
    return cmd('cabinet', p)


tris, boxes = {}, {}
for style in ('slab', 'shaker', 'raised-panel'):
    rr, (j, binc) = export_glb(ROOM + [base(style)], tmp, f'{style}.glb')
    n = 0
    for m in node_meshes(j, 'cabinet-cb0-front'):
        for prim in m['primitives']:
            n += j['accessors'][prim['indices']]['count'] // 3 if 'indices' in prim else 0
    tris[style] = n
    boxes[style] = drawing_box(j, 'cabinet-cb0-front')
if not (tris['slab'] < tris['shaker'] < tris['raised-panel']):
    fail(f"front triangles by style should grow slab < shaker < raised panel: {tris}")
for style in ('shaker', 'raised-panel'):
    for k in range(3):
        for lh in (0, 1):
            if abs(boxes[style][lh][k] - boxes['slab'][lh][k]) > 2e-3:
                fail(f"the {style} fronts' box {boxes[style]} is not the slab fronts' {boxes['slab']} (±2 mm)")
print(f"ok: front styles {tris} (triangles), the same size")

rr, (j, binc) = export_glb(ROOM + [base('shaker', {"pull": "own:pull-bar-128"}),
                                   cmd('cabinet', {"kind": "wall", "wall": "w0", "offset": 1.0, "width": 0.6, "pull": "own:pull-knob-32"}),
                                   INSPECT14], tmp, 'pulls.glb')
cabs = {c['id']: c for c in objects_named(inspected(rr), 'cabinets', 'cabinets')}
for cid, pull in (('cb0', 'own:pull-bar-128'), ('cb1', 'own:pull-knob-32')):
    c = cabs[cid]
    if c.get('pull') != pull:
        fail(f"{cid} pull {c.get('pull')!r}, expected {pull}")
    fronts = c.get('fronts')
    if not isinstance(fronts, list) or not fronts:
        fail(f"{cid} lists no fronts: {c}")
    for fr in fronts:
        u0, v0, u1, v1 = [num(x, f'{cid} front rect') for x in fr.get('rect', [])]
        p = fr.get('pull')
        if fr.get('kind') == 'false':
            continue
        if not isinstance(p, list) or len(p) != 2:
            fail(f"{cid} {fr.get('kind')} front has no pull: {fr}")
        u, v = num(p[0], 'pull u'), num(p[1], 'pull v')
        if fr['kind'] == 'drawer':
            near(u, (u0 + u1) / 2, 1e-6, f"{cid} drawer pull u"); near(v, (v0 + v1) / 2, 1e-6, f"{cid} drawer pull v")
        elif fr['kind'] == 'door':
            free = u1 - 0.05 if fr.get('hinge') == 'left' else u0 + 0.05
            near(u, free, 1e-6, f"{cid} door pull 0.05 m from its free edge (hinge {fr.get('hinge')})")
            near(v, v1 - 0.075 if c['kind'] != 'wall' else v0 + 0.075, 1e-6, f"{cid} door pull height")
        else:
            fail(f"{cid}: unknown front kind {fr.get('kind')!r}")
    n_pulls = sum(1 for fr in fronts if fr.get('kind') in ('door', 'drawer'))
    if cid == 'cb0' and n_pulls != 3:
        fail(f"cb0 (a drawer over two doors) should have 3 pulls, has {n_pulls}")
    lo, hi = drawing_box(j, f'cabinet-{cid}-pull')
    front_y = 0.6 if cid == 'cb0' else num(c['depth'], 'wall cabinet depth')
    if lo[1] < front_y - 1e-4 or hi[1] <= front_y + 0.005:
        fail(f"{cid}'s pulls (y {lo[1]:.4f}..{hi[1]:.4f}) do not stand proud of the front face y = {front_y}")
print("ok: pulls on every door and drawer at the set position, proud of the fronts")

# A cabinet-part item clicked over the cabinet swaps; nothing is placed.
rc, rr, so, se = run(ROOM + [base('slab', {"pull": "own:pull-bar-128"}),
                             cmd('library.place', {"item": "own:pull-knob-32", "at": [0.9, 0.3]}),
                             cmd('library.place', {"item": "frontstyle:door-raised-panel", "at": [0.9, 0.62]}),
                             cmd('library.place', {"item": "frontstyle:drawer-shaker", "at": [0.9, 0.62]}),
                             INSPECT14, 'undo {}', INSPECT14])
sw = [r['swapped'] for r in rr if isinstance(r, dict) and isinstance(r.get('swapped'), dict)]
if [(s.get('cabinet'), s.get('part')) for s in sw] != [('cb0', 'pull'), ('cb0', 'door'), ('cb0', 'drawer')]:
    fail(f"swaps: {sw}")
c = {x['id']: x for x in objects_named(inspected(rr, -2), 'cabinets', 'cabinets')}['cb0']
if (c.get('pull'), c.get('doorStyle'), c.get('drawerStyle')) != ('own:pull-knob-32', 'raised-panel', 'shaker'):
    fail(f"after the swaps: pull {c.get('pull')}, doors {c.get('doorStyle')}, drawers {c.get('drawerStyle')}")
if furnishings_of(inspected(rr, -2)):
    fail("a swap placed a furnishing")
c = {x['id']: x for x in objects_named(inspected(rr, -1), 'cabinets', 'cabinets')}['cb0']
if c.get('drawerStyle') != 'slab' or c.get('doorStyle') != 'raised-panel':
    fail(f"undo should take back only the last swap: {c.get('doorStyle')}, {c.get('drawerStyle')}")
print("ok: pulls and styles swap onto the cabinet; each swap is one undo step")
print("PASS 06")
