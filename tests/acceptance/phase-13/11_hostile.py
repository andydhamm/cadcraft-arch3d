# Hostile input to this phase's commands: material, cabinet, fixture and build3d refuse garbage (strings for numbers,
# huge or negative sizes, enormous lists, unknown kinds, levels and ids) with an error, never panic, never clamp, and
# change nothing: after all of it the model has no cabinet, no fixture and no finish.
# needs: p13-doc-finish, p13-cmd-casework, p13-build3d-incremental
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

H = [('material', {"walls": "everything", "finish": "brick"}),
     ('material', {"walls": [1, 2, 3], "finish": "brick"}),
     ('material', {"walls": {"w0": True}, "finish": "brick"}),
     ('material', {"walls": [f"w{k}" for k in range(4000)], "finish": "brick"}),
     ('material', {"walls": ["w0"], "finish": 7}),
     ('material', {"walls": ["w0"], "finish": "BRICK"}),
     ('material', {"rooms": "all", "finish": "siding"}),
     ('material', {"walls": ["w0"]}),
     ('cabinet', {"kind": "base", "at": [1e308, 0]}),
     ('cabinet', {"kind": "base", "at": [1, 1], "width": -0.6}),
     ('cabinet', {"kind": "base", "at": [1, 1], "width": "wide"}),
     ('cabinet', {"kind": "base", "at": [1, 1], "height": 1e9}),
     ('cabinet', {"kind": "base", "at": [1, 1], "elevation": -5}),
     ('cabinet', {"kind": "base", "at": [1, 1], "doors": -1}),
     ('cabinet', {"kind": "base", "at": [1, 1], "drawers": 1000000}),
     ('cabinet', {"kind": "base", "at": [1, 1], "level": 99}),
     ('cabinet', {"kind": "base", "at": [1, 1], "fronts": "glass"}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 0.5, "widths": [0.3] * 5000}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": -3}),
     ('cabinet', {"kind": "base", "wall": "w0", "offset": 1, "widths": []}),
     ('cabinet', {"kind": "base", "at": "here"}),
     ('cabinet', {"kind": ["base"], "at": [1, 1]}),
     ('cabinet', {"at": [1, 1]}),
     ('cabinet.edit', {"id": "cb0", "width": 0.5}),
     ('cabinet.delete', {"id": "cb999999999999"}),
     ('fixture', {"kind": "", "at": [1, 1]}),
     ('fixture', {"kind": "jacuzzi", "at": [1, 1]}),
     ('fixture', {"kind": "toilet", "at": [1, 1], "width": 0}),
     ('fixture', {"kind": "toilet", "at": [1, 1], "height": 1e9}),
     ('fixture', {"kind": "toilet", "at": [1, 1], "rotation": "north"}),
     ('fixture', {"kind": "sink", "wall": "w0", "offset": 1e300}),
     ('fixture.edit', {"id": "fx0", "kind": "bathtub"}),
     ('fixture.delete', {"id": "w0"}),
     ('build3d', {"incremental": "yes"}),
     ('build3d', {"digest": [1]})]
cmds = ROOM + [cmd(n, p) for n, p in H] + [INSPECT_ALL]
rc, rr, so, se = run(cmds, expect_ok=False)
errors = [l for l in se.splitlines() if l.startswith('error: ')]
if len(errors) < len(H):
    seen = '\n'.join(errors[:40])
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errors)} were:\n{seen}")
ins = inspected(rr)
if objects_named(ins, 'cabinets', 'cabinets') or objects_named(ins, 'fixtures', 'fixtures'):
    fail(f"refused commands place nothing: {ins.get('cabinets')} {ins.get('fixtures')}")
if any(w.get('finish') is not None for w in ins['walls']) or any(r.get('floorFinish') is not None for r in ins.get('rooms') or []):
    fail("refused material commands finish nothing")
print(f"ok: {len(H)} hostile commands refused; nothing placed or finished")
print("PASS 11")
