#!/usr/bin/env python3
# Brief numbers: four 0.9 x 1.2 windows and one 1.2 x 1.2 give a window schedule of two rows with counts 4 and 1;
# resizing one of the four to 1.2 x 1.2 makes the counts 3 and 2. The schedule is a real CADCraft TABLE (read back with
# tableexport) updated live: the resize is the only command, and undoing it brings 4 and 1 back. Identical items share
# their schedule number (W01 for the four, W02 for the wide one; doors D.., cabinets C..).
# A stub fails: a table that is not updated, counts that do not follow the resize or the undo, numbers that differ
# between identical items.
# needs: p21-schedule-model, p21-schedule-tables, p21-opening-edit
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

SET = HOUSE + SH_WINDOWS + [
    cmd('door', {"wall": "w3", "offset": 1.5, "width": 0.9, "height": 2.1}),            # o5
    SH_CAB,                                                                               # cb0, 0.6 base
    cmd('cabinet', {"kind": "base", "wall": "w5", "offset": 3.1}),                        # cb1, the same
    cmd('cabinet', {"kind": "wall", "wall": "w5", "offset": 2.5}),                        # cb2, a wall cabinet
    cmd('schedule', {"kind": "window"})]


def state(rows, what, nth=-1):
    insp = inspected(rows, nth)
    scheds = sh_list(insp, 'schedules')
    if len(scheds) != 1 or scheds[0].get('kind') != 'window':
        fail(f"{what}: one window schedule: {json.dumps(scheds)[:300]}")
    return insp, scheds[0]


def table_counts(rows, what, nth=-1):
    header, data = sh_table_rows(sh_csv(rows, nth), what)
    ci, ni = sh_col(header, 'Count', what), sh_col(header, 'No.', what)
    return [(r[ni], r[ci]) for r in data]


# (1) Four and one.
rr = sh_run_ok(SET + [INSPECT21], "the window schedule")
made_ = sh_reply(rr, 'schedule', what='schedule')
table = made_.get('table')
if not isinstance(table, str) or not table:
    fail(f"the schedule reply names its TABLE's handle: {made_}")
if sh_counts(made_) != [('W01', 4), ('W02', 1)]:
    fail(f"the schedule reply's rows: want [W01 x 4, W02 x 1], got {sh_counts(made_)}")
insp, sc = state(rr, "after creating it")
if sc.get('table') != table:
    fail(f"arch.inspect names the same TABLE: {sc.get('table')} vs {table}")
if sh_counts(sc) != [('W01', 4), ('W02', 1)]:
    fail(f"window schedule rows: want W01 x 4 and W02 x 1, got {sh_counts(sc)}")
members = {r.get('number'): sorted(r.get('members') or []) for r in sc.get('rows') or []}
if members != {'W01': ['o0', 'o1', 'o2', 'o3'], 'W02': ['o4']}:
    fail(f"row members: want W01 = o0..o3, W02 = o4, got {members}")
nums = {o.get('id'): o.get('number') for o in objects_named(insp, 'openings', 'openings')}
want = {'o0': 'W01', 'o1': 'W01', 'o2': 'W01', 'o3': 'W01', 'o4': 'W02', 'o5': 'D01'}
if nums != want:
    fail(f"opening schedule numbers: want {want}, got {nums}")
cnums = {c.get('id'): c.get('number') for c in objects_named(insp, 'cabinets', 'cabinets')}
if cnums != {'cb0': 'C01', 'cb1': 'C01', 'cb2': 'C02'}:
    fail(f"cabinet schedule numbers (cb0 and cb1 identical): {cnums}")
EXPORT = cmd('tableexport', {"handle": table})
rr = sh_run_ok(SET + [EXPORT], "tableexport of the schedule")
got = table_counts(rr, "the TABLE")
if got != [('W01', '4'), ('W02', '1')]:
    fail(f"the TABLE's rows (No., Count): want [W01 4, W02 1], got {got}")

# (2) Resize o1 to 1.2 x 1.2: three and two, live in the TABLE; undo gives four and one again.
rr = sh_run_ok(SET + [cmd('opening.edit', {"id": "o1", "width": 1.2}), INSPECT21, EXPORT,
                      'undo {}', INSPECT21, EXPORT], "the resize and its undo")
insp, sc = state(rr, "after the resize", 0)
if sh_counts(sc) != [('W01', 3), ('W02', 2)]:
    fail(f"after resizing o1 to 1.2 x 1.2: want W01 x 3 and W02 x 2, got {sh_counts(sc)}")
o1 = by_id(objects_named(insp, 'openings', 'openings'), 'o1', 'openings')
if abs(num(o1.get('width'), 'o1 width') - 1.2) > 1e-9 or o1.get('number') != 'W02':
    fail(f"o1 is 1.2 wide and numbered W02 after the edit: {o1}")
got = table_counts(rr, "the TABLE after the resize", 0)
if got != [('W01', '3'), ('W02', '2')]:
    fail(f"the TABLE follows the resize with no other command: want [W01 3, W02 2], got {got}")
insp, sc = state(rr, "after the undo", 1)
if sh_counts(sc) != [('W01', 4), ('W02', 1)]:
    fail(f"undo restores W01 x 4 and W02 x 1: {sh_counts(sc)}")
got = table_counts(rr, "the TABLE after the undo", 1)
if got != [('W01', '4'), ('W02', '1')]:
    fail(f"undo restores the TABLE too (one undo step): {got}")
print("ok: window schedule W01 x 4 + W02 x 1 -> W01 x 3 + W02 x 2 after the resize, live in the TABLE; undo restores it")
print("PASS 02")
