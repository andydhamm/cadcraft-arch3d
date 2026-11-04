# Cabinets: base, wall and tall boxes placed on a wall (backs on its inner face) or by point, a run edge to edge, with
# door and drawer fronts, a counter on base cabinets at 0.914 m, a toe kick; proven by inspect and by probing the 3D
# build. Edits, deletes and undo work; impossible cabinets are refused and place nothing.
# needs: p13-doc-casework, p13-arch3d-casework, p13-cmd-casework
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

P = [[0.9, 0.3, 0.895],    # 0  inside cb0's counter (0.876-0.914)
     [0.9, 0.3, 0.93],     # 1  above it: nothing
     [0.9, 0.3, 0.5],      # 2  cb0's carcass
     [0.9, 0.615, 0.895],  # 3  the counter's front overhang (to y 0.625)
     [0.9, 0.615, 0.5],    # 4  in front of the fronts (they end at y 0.6): nothing
     [0.9, 0.59, 0.5],     # 5  cb0's door front (y 0.581-0.6)
     [0.9, 0.59, 0.05],    # 6  the toe-kick recess under the fronts: nothing
     [0.9, 0.3, 0.05],     # 7  the plinth behind it
     [0.9, 0.15, 1.5],     # 8  the wall cabinet cb3 (z 1.372-2.134, y 0-0.305)
     [0.9, 0.15, 1.35],    # 9  under it: nothing
     [0.9, 0.15, 2.12],    # 10 its top
     [3.7, 0.9, 2.1],      # 11 the tall cabinet cb4 on w1 (x 3.4-4.0, y 0.6-1.2, z 0-2.134)
     [3.7, 0.9, 2.16],     # 12 above it: nothing
     [2.4, 0.3, 0.895]]    # 13 over cb2, the third of the run: the run's one counter, owned by cb0 (since phase 14)
rc, rr, so, se = run(ROOM + KITCHEN + [INSPECT_ALL, cmd('build3d', {"probe": P})])
placed = replies_with_list(rr, 'cabinets')
if [p['cabinets'] for p in placed] != [['cb0', 'cb1', 'cb2'], ['cb3'], ['cb4']]:
    fail(f"cabinet replies list their labels (a run of three, then cb3, cb4): {[p.get('cabinets') for p in placed]}")
cabs = objects_named(inspected(rr), 'cabinets', 'cabinets')
want = {'cb0': ('base', [0.9, 0.0], 0.6, 0.0), 'cb1': ('base', [1.65, 0.0], 0.9, 0.0), 'cb2': ('base', [2.4, 0.0], 0.6, 0.0),
        'cb3': ('wall', [0.9, 0.0], 0.6, 0.0), 'cb4': ('tall', [4.0, 0.9], 0.6, 90.0)}
for cid, (kind, at, width, rot) in want.items():
    c = by_id(cabs, cid, 'cabinets')
    if c.get('kind') != kind:
        fail(f"{cid} is a {kind} cabinet: {c}")
    if not isinstance(c.get('at'), list) or max(abs(num(c['at'][k], f'{cid} at') - at[k]) for k in range(2)) > 1e-6:
        fail(f"{cid}'s back edge centre is {at} (on the wall's inner face): {c.get('at')}")
    near(c.get('width'), width, 1e-9, f"{cid} width")
    if abs(((num(c.get('rotation'), f'{cid} rotation') - rot) + 180) % 360 - 180) > 1e-6:
        fail(f"{cid} rotation is the wall's direction {rot}: {c.get('rotation')}")
for cid in ('cb0', 'cb1', 'cb2'):
    c = by_id(cabs, cid, 'cabinets')
    near(c.get('counterTop'), 0.914, 0.002, f"{cid} counter top")
    if c.get('layer') != 'A-FLOR-CASE' or c.get('fronts') != 'drawerOverDoor':
        fail(f"{cid}: base cabinets are drawer-over-door on A-FLOR-CASE: {c}")
if by_id(cabs, 'cb1', 'cabinets').get('doors') != 2 or by_id(cabs, 'cb0', 'cabinets').get('doors') != 1:
    fail("a 0.9 m base cabinet gets two doors, a 0.6 m one gets one")
c3 = by_id(cabs, 'cb3', 'cabinets')
if c3.get('counterTop') is not None or c3.get('layer') != 'A-FLOR-CASE-UPPR':
    fail(f"the wall cabinet has no counter and sits on A-FLOR-CASE-UPPR: {c3}")
near(c3.get('elevation'), 1.372, 1e-9, "wall cabinet elevation")
near(by_id(cabs, 'cb4', 'cabinets').get('height'), 2.134, 1e-9, "tall cabinet height")
print("ok: five cabinets where they belong, counters at 0.914 m")

hits = probes(rr)['probe']
expect = {0: ('cb0', 'counter'), 1: None, 2: ('cb0', 'cabinet'), 3: ('cb0', 'counter'), 4: None, 5: ('cb0', 'cabinet'),
          6: None, 7: ('cb0', 'cabinet'), 8: ('cb3', 'cabinet'), 9: None, 10: ('cb3', 'cabinet'), 11: ('cb4', 'cabinet'),
          12: None, 13: ('cb0', 'counter')}
for i, e in expect.items():
    h = hits[i] if i < len(hits) else 'missing'
    got = None if h is None else (h.get('id'), h.get('part')) if isinstance(h, dict) else h
    if got != e:
        fail(f"probe {P[i]}: expected {e}, got {got}")
print("ok: probes find counter, carcass, fronts, toe kick, the wall and tall cabinets")

# Edit, delete, undo.
rc, rr2, so2, se2 = run(ROOM + KITCHEN + [cmd('cabinet.edit', {"id": "cb0", "width": 0.45}), INSPECT_ALL,
                                         cmd('cabinet.delete', {"id": "cb3"}), INSPECT_ALL, 'undo {}', INSPECT_ALL])
near(by_id(objects_named(inspected(rr2, -3), 'cabinets', ''), 'cb0', 'cabinets').get('width'), 0.45, 1e-9, "edited width")
if 'cb3' in [c.get('id') for c in objects_named(inspected(rr2, -2), 'cabinets', '')]:
    fail("cabinet.delete removes cb3")
if 'cb3' not in [c.get('id') for c in objects_named(inspected(rr2, -1), 'cabinets', '')]:
    fail("undo brings cb3 back")
print("ok: edit, delete and undo")

# Refusals place nothing.
for params, what in [({"kind": "wall", "wall": "w0", "offset": 1.0, "counter": True}, "a wall cabinet with a counter"),
                     ({"kind": "base", "wall": "w0", "offset": 3.0, "widths": [0.6, 0.9, 0.6]}, "a run past the wall's end"),
                     ({"kind": "base", "at": [1, 1], "width": 9.0}, "a 9 m cabinet"),
                     ({"kind": "island", "at": [1, 1]}, "an unknown kind"),
                     ({"kind": "base", "wall": "w9", "offset": 1.0}, "an unknown wall"),
                     ({"kind": "base", "at": [1, 1], "doors": 40}, "40 doors")]:
    rc, rr3, so3, se3 = run(ROOM + [cmd('cabinet', params), INSPECT_ALL], expect_ok=False)
    if not was_refused(rc, se3):
        fail(f"cabinet refuses {what}")
    if objects_named(inspected(rr3), 'cabinets', 'after a refusal'):
        fail(f"a refused cabinet ({what}) places nothing")
print("ok: six refusals place nothing")
print("PASS 04")
