#!/usr/bin/env python3
# Painters. In the phase-1 house with both rooms' walls painted the same material, paint.material at `room` scope changes
# exactly the surfaces in the clicked room that have the clicked material, at `plan` scope every one; `component` and
# `object` scopes; `blend` keeps the texture image and tints it; the eyedropper reads a material back; each painter call is
# one undo step. paint.object at `room` scope replaces exactly the like pulls in the clicked cabinet's room.
# needs: p14-doc-furnishing, p14-doc-cabinet-parts, p14-arch-paint, p14-cmd-paint, p14-arch3d-paint, p14-io-glb-furnishing, p14-inspect, p14-cmd-library, p14-io-library-files, p14-apps-library-hooks, p14-cmd-casework-parts
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p14lib.inc')).read())

tmp = tempfile.mkdtemp()
A_WALLS = {'wall/w0/left', 'wall/w4/left', 'wall/w5/left', 'wall/w6/left'}      # room A (x < 4)
B_WALLS = {'wall/w1/left', 'wall/w2/left', 'wall/w3/left', 'wall/w6/right'}     # room B
OUTSIDE = {f'wall/w{k}/right' for k in range(6)}


def pm(material, target, scope, mode=None):
    p = {"material": material, "target": target, "scope": scope}
    if mode:
        p["mode"] = mode
    return cmd('paint.material', p)


rc, rr, so, se = run(HOUSE1 + [INSPECT14])
s0 = surfaces_of(inspected(rr))
ra, rb = room_at(inspected(rr), 2, 2.5), room_at(inspected(rr), 6, 2.5)
for k in A_WALLS:
    if s0.get(k, {}).get('room') != ra:
        fail(f"{k} should face room {ra}: {s0.get(k)}")
for k in B_WALLS:
    if s0.get(k, {}).get('room') != rb:
        fail(f"{k} should face room {rb}: {s0.get(k)}")
for k in OUTSIDE:
    if s0.get(k, {}).get('room') is not None:
        fail(f"exterior face {k} is given room {s0[k].get('room')}")
for k in (f'room/{ra}/floor', f'room/{ra}/ceiling', f'room/{rb}/floor'):
    if k not in s0:
        fail(f"no surface {k}: {sorted(s0)[:20]}")
print(f"ok: every wall side faces its room ({ra}: 4, {rb}: 4, outside: 6)")

PAINTED = HOUSE1 + [pm('material-oak', 'wall/w0/left', 'room'), pm('material-oak', 'wall/w1/left', 'room')]
rc, rr, so, se = run(PAINTED + [INSPECT14,
                                pm('material-marble', 'wall/w0/left', 'room'), INSPECT14,
                                pm('#3060a0', 'wall/w2/left', 'plan'), INSPECT14,
                                'undo {}', INSPECT14,
                                pm('siding', 'wall/w0/right', 'component'), INSPECT14,
                                pm('material-fabric', 'wall/w6/left', 'object'), INSPECT14,
                                cmd('paint.pick', {"target": "wall/w4/left"})])
states = [surfaces_of(inspected(rr, k)) for k in range(6)]
painted = [r['painted'] for r in rr if isinstance(r, dict) and isinstance(r.get('painted'), list)]


def mats(s, keys):
    return {k: s[k].get('material') for k in keys}


if set(mats(states[0], A_WALLS | B_WALLS).values()) != {'item:material-oak'}:
    fail(f"room scope in each room should paint all 8 wall sides oak: {mats(states[0], A_WALLS | B_WALLS)}")
if any(states[0][k].get('material') == 'item:material-oak' for k in OUTSIDE) or \
        states[0][f'room/{ra}/floor'].get('material') == 'item:material-oak':
    fail("room scope painted a surface that did not have the clicked material (an exterior face or a floor)")
if sorted(painted[2]) != sorted(A_WALLS) or set(mats(states[1], A_WALLS).values()) != {'item:material-marble'} \
        or set(mats(states[1], B_WALLS).values()) != {'item:material-oak'}:
    fail(f"marble at room scope: painted {painted[2]}; A {mats(states[1], A_WALLS)}; B {mats(states[1], B_WALLS)}")
print("ok: room scope changes exactly the clicked room's surfaces with the clicked material")
if sorted(painted[3]) != sorted(B_WALLS) or set(mats(states[2], B_WALLS).values()) != {'colour:#3060a0'} \
        or set(mats(states[2], A_WALLS).values()) != {'item:material-marble'}:
    fail(f"plan scope: painted {painted[3]}; B {mats(states[2], B_WALLS)}; A {mats(states[2], A_WALLS)}")
if mats(states[3], B_WALLS) != mats(states[1], B_WALLS):
    fail(f"undo of a plan-scope paint should restore every surface it changed: {mats(states[3], B_WALLS)}")
print("ok: plan scope changes every surface with the clicked material; one undo restores them all")
if painted[4] != ['wall/w0/right'] or states[4]['wall/w0/right'].get('material') != 'finish:siding' \
        or states[4]['wall/w1/right'].get('material') == 'finish:siding':
    fail(f"component scope: {painted[4]}, w0 right {states[4]['wall/w0/right']}, w1 right {states[4]['wall/w1/right']}")
if sorted(painted[5]) != ['wall/w6/left', 'wall/w6/right'] or states[5]['wall/w6/right'].get('material') != 'item:material-fabric':
    fail(f"object scope on the partition: {painted[5]}")
pick = reply_with(rr, 'material', -1, 'paint.pick')
if pick.get('material') != 'item:material-marble':
    fail(f"the eyedropper on w4's inner side: {pick}")
print("ok: component and object scopes; the eyedropper picks marble up")

# blend keeps the texture image and tints it.
rr, (j, binc) = export_glb(PAINTED, tmp, 'oak.glb')
ma, _ = material_of_node(j, 'wall-w0-left')
img_a = texture_image_name(j, ma)
rr, (j2, binc2) = export_glb(PAINTED + [pm('#c04020', 'wall/w0/left', 'component', 'blend'), INSPECT14], tmp, 'blend.glb')
mb, _ = material_of_node(j2, 'wall-w0-left')
img_b = texture_image_name(j2, mb)
if not img_a or not img_b:
    fail(f"the oak face has no texture image (before {img_a!r}, after {img_b!r})")
bytes_a = glb_images(j, binc).get(img_a); bytes_b = glb_images(j2, binc2).get(img_b)
if bytes_a is None or bytes_a != bytes_b:
    fail("blend changed the texture image")
fa = mb.get('pbrMetallicRoughness', {}).get('baseColorFactor')
if not fa or fa[:3] == [1, 1, 1] or fa == ma.get('pbrMetallicRoughness', {}).get('baseColorFactor'):
    fail(f"blend did not tint: baseColorFactor {fa}")
s = surfaces_of(inspected(rr))['wall/w0/left']
if s.get('material') != 'item:material-oak' or s.get('tint') != '#c04020':
    fail(f"blend: the surface reads {s}")
print("ok: blend keeps the oak image and tints it #c04020")

# paint.object: like pulls in the clicked cabinet's room.
CABS = HOUSE1 + [cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 1.0, "pull": "own:pull-bar-128"}),     # room A
                 cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 2.0, "pull": "own:pull-bar-128"}),     # room A
                 cmd('cabinet', {"kind": "base", "wall": "w0", "offset": 3.0, "pull": "own:pull-knob-32"}),     # room A, unlike
                 cmd('cabinet', {"kind": "base", "wall": "w1", "offset": 1.0, "pull": "own:pull-bar-128"})]     # room B
rc, rr, so, se = run(CABS + [cmd('paint.object', {"item": "frontstyle:door-shaker", "target": "cabinet/cb0/front", "scope": "object"}),
                             cmd('paint.object', {"item": "own:pull-knob-32", "target": "cabinet/cb0/pull", "scope": "room"}), INSPECT14,
                             'undo {}', INSPECT14])
rep = [r['replaced'] for r in rr if isinstance(r, dict) and isinstance(r.get('replaced'), list)]
cabs = {c['id']: c for c in objects_named(inspected(rr, -2), 'cabinets', 'cabinets')}
if sorted(rep[1]) != ['cabinet/cb0/pull', 'cabinet/cb1/pull']:
    fail(f"paint.object at room scope replaced {rep[1]}, expected cb0's and cb1's pulls")
if [cabs[k].get('pull') for k in ('cb0', 'cb1', 'cb2', 'cb3')] != ['own:pull-knob-32'] * 3 + ['own:pull-bar-128']:
    fail(f"pulls after paint.object: {[(k, cabs[k].get('pull')) for k in sorted(cabs)]}")
if cabs['cb0'].get('doorStyle') != 'shaker' or cabs['cb1'].get('doorStyle') != 'slab':
    fail(f"paint.object of a door style at object scope: cb0 {cabs['cb0'].get('doorStyle')}, cb1 {cabs['cb1'].get('doorStyle')}")
cabs = {c['id']: c for c in objects_named(inspected(rr, -1), 'cabinets', 'cabinets')}
if cabs['cb0'].get('pull') != 'own:pull-bar-128':
    fail("undo did not restore the pulls")
print("ok: paint.object replaces exactly the like pulls in the room; styles at object scope; one undo step")
print("PASS 07")
