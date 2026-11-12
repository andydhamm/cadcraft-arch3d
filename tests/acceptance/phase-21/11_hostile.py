#!/usr/bin/env python3
# Hostile input to every phase-21 command is refused with an error line, and nothing changes: openings, schedules
# (and their TABLEs), views and their notes, sheets and their layouts, and the built scene's digest are the same.
# (Non-finite numbers cannot be sent: JSON has no NaN, and the CLI refuses a non-JSON --cmd before running anything.)
# needs: p21-opening-edit, p21-schedule-tables, p21-interior-elevation, p21-view-notes, p21-sheet-command, p21-title-block, p21-plot-sheets
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

tmp = sh_tmp()
R = house_rooms()
NW = R['NW']
SETUP = HOUSE + [SH_WINDOWS[0], SH_CAB,
                 cmd('elevation', {"room": NW, "wall": "w5"}),                                   # v0
                 cmd('view.note', {"view": "v0", "text": "NOTE", "at": [-6.0, 1.0]}),
                 cmd('schedule', {"kind": "window"}),                                            # sc0
                 cmd('sheet', {"name": "Sheet A", "number": "A-1", "views": [{"view": "v0", "scale": "1:50"}]}),
                 cmd('sheet', {"name": "Sheet B", "number": "A-2", "views": [{"view": "sc0", "scale": "1:50"}]})]
LONG = "x" * 300
H = [('opening.edit', {"id": "o9", "width": 1.0}),
     ('opening.edit', {"id": "o0", "width": -1}),
     ('opening.edit', {"id": "o0", "width": 20}),                 # does not fit its wall
     ('opening.edit', {"id": "o0", "width": "wide"}),
     ('opening.edit', {"id": "w0", "width": 1.0}),
     ('opening.edit', {"id": "o0", "sill": 2.0}),                 # rises above the wall
     ('schedule', {"kind": "stairs"}),
     ('schedule', {"kind": "window", "columns": ["number", "colour"]}),
     ('schedule', {"kind": "window", "columns": "number"}),
     ('schedule', {"kind": "window", "columns": []}),
     ('schedule', {"kind": "room", "columns": ["count"]}),
     ('schedule', {"kind": "window", "level": "Moon"}),
     ('schedule', {"kind": "window", "at": [1]}),
     ('schedule.edit', {"id": "sc9", "columns": ["number"]}),
     ('schedule.edit', {"id": "sc0", "columns": ["bogus"]}),
     ('schedule.delete', {"id": "w0"}),
     ('elevation', {"room": "r99", "wall": "w5"}),
     ('elevation', {"room": NW, "wall": "w0"}),                     # w0 does not bound the north-west room
     ('elevation', {"room": NW, "wall": "w5", "side": "east"}),
     ('elevation', {"room": NW}),
     ('elevation', {"room": NW, "side": "up"}),
     ('elevation', {"room": NW, "wall": "w5", "look": [1, 0]}),
     ('view.note', {"view": "v9", "text": "x", "at": [0, 0]}),
     ('view.note', {"view": "v0", "text": "", "at": [0, 0]}),
     ('view.note', {"view": "v0", "text": "x"}),
     ('view.note', {"view": "v0", "text": "x", "at": [0, 0], "attach": {"object": "cb0", "corner": "topLeft"}}),
     ('view.note', {"view": "v0", "text": "x", "attach": {"object": "cb9", "corner": "topLeft"}}),
     ('view.note', {"view": "v0", "text": "x", "attach": {"object": "cb0", "corner": "middle"}}),
     ('view.note', {"view": "v0", "text": LONG, "at": [0, 0]}),
     ('view.note', {"view": "v0", "text": "x", "at": [1e9, 0]}),
     ('view.dim', {"view": "v0", "a": [0, 0], "b": [0, 0]}),
     ('view.note.delete', {"view": "v0", "note": "n9"}),
     ('sheet', {"name": "Bad", "views": [{"view": "Nope", "scale": "1:50"}]}),
     ('sheet', {"name": "Bad", "size": "ARCH Z", "views": [{"view": "v0", "scale": "1:50"}]}),
     ('sheet', {"name": "Bad", "views": [{"view": "v0", "scale": "1:0"}]}),
     ('sheet', {"name": "Bad", "views": [{"view": "v0", "scale": -50}]}),
     ('sheet', {"name": "Sheet A", "views": [{"view": "v0", "scale": "1:50"}]}),
     ('sheet', {"name": "Bad", "number": "A-1", "views": [{"view": "v0", "scale": "1:50"}]}),
     ('sheet', {"name": "Bad", "views": "v0"}),
     ('sheet', {"name": "", "views": [{"view": "v0", "scale": "1:50"}]}),
     ('sheet', {"name": "Bad", "views": [{"camera": "Nope"}]}),
     ('sheet', {"name": "Bad", "views": [{"view": "v0", "scale": "1:50", "at": ["a", 1]}]}),
     ('sheet.edit', {"name": "Nope", "title": "x"}),
     ('sheet.edit', {"name": "Sheet A", "order": 7}),
     ('sheet.edit', {"name": "Sheet A", "number": "A-2"}),
     ('sheet.delete', {"name": "Nope"}),
     ('sheet.open', {"view": "v9"}),
     ('sheetset', {"project": 5}),
     ('plot', {"sheets": "some", "path": os.path.join(tmp, 'a.pdf')}),
     ('plot', {"sheets": ["Nope"], "path": os.path.join(tmp, 'b.pdf')}),
     ('plot', {"sheets": "all", "output": "svg", "path": os.path.join(tmp, 'c.svg')}),
     ('plot', {"sheets": "all", "layout": "Sheet A", "path": os.path.join(tmp, 'd.pdf')})]
DIGEST = cmd('build3d', {"digest": True})


def state(rows):
    insp = inspected(rows)
    lays = [r for r in rows if isinstance(r, list)]
    digests = [r['digest'] for r in rows if isinstance(r, dict) and isinstance(r.get('digest'), str)]
    if not digests or not lays:
        fail(f"no build3d digest or layout.list reply; replies: {json.dumps(rows)[-300:]}")
    views = [{k: v.get(k) for k in ('id', 'kind', 'room', 'wall', 'notes')} for v in sh_list(insp, 'views')]
    return {'openings': objects_named(insp, 'openings', 'openings'), 'schedules': sh_list(insp, 'schedules'),
            'views': views, 'sheets': sh_list(insp, 'sheets'), 'sheetSet': insp.get('sheetSet'),
            'layouts': sorted(str(l.get('name')) for l in lays[-1] if isinstance(l, dict)), 'digest': digests[-1]}


rc, rr, so, se = run(SETUP + [INSPECT21, 'layout.list {}', DIGEST], expect_ok=False)
if errors(se):
    fail("the set-up is refused:\n" + '\n'.join(errors(se)))
before = state(rr)
if len(before['sheets']) != 2 or len(before['schedules']) != 1:
    fail(f"the set-up makes two sheets and a schedule: {len(before['sheets'])}, {len(before['schedules'])}")
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + [INSPECT21, 'layout.list {}', DIGEST], expect_ok=False)
if 'panicked' in se:
    fail(f"a hostile command panicked: {se[-400:]}")
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:70]))
after = state(rr)
for key in before:
    if json.dumps(before[key], sort_keys=True) != json.dumps(after[key], sort_keys=True):
        fail(f"refused commands changed '{key}': {json.dumps(before[key])[:300]} -> {json.dumps(after[key])[:300]}")
for f in ('a.pdf', 'b.pdf', 'c.svg', 'd.pdf'):
    if os.path.exists(os.path.join(tmp, f)):
        fail(f"a refused plot wrote {f}")
print(f"ok: {len(H)} hostile commands refused; openings, schedules, views, notes, sheets, layouts and the scene unchanged")
print("PASS 11")
