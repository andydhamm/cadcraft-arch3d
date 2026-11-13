#!/usr/bin/env python3
# Door, cabinet and room-finish schedules with chosen columns, as CADCraft TABLEs read back with tableexport:
# identical doors share a row with a count; a new door joins its row live; editing the columns rewrites the TABLE's
# header; a room-finish schedule lists each room's floor, wall and ceiling materials (a wood floor, a painted wall face)
# and interior area; deleting a schedule deletes its TABLE.
# A stub fails: wrong grouping, cells that ignore the chosen columns or the room's materials, a TABLE that survives.
# needs: p21-schedule-model, p21-schedule-tables
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p21lib.inc')).read())

R = house_rooms()
NW = R['NW']
DOORS = [cmd('door', {"wall": "w4", "offset": 2.0, "width": 0.9, "height": 2.1}),   # o0
         cmd('door', {"wall": "w4", "offset": 8.0, "width": 0.8, "height": 2.1}),   # o1
         cmd('door', {"wall": "w3", "offset": 1.5, "width": 0.9, "height": 2.1})]   # o2, identical to o0
DSCHED = cmd('schedule', {"kind": "door", "columns": ["number", "count", "width", "height"], "name": "Doors"})

# (1) Doors: two rows, D01 x 2 (o0, o2) and D02 x 1, exactly the chosen columns and formats.
table = made(HOUSE + DOORS + [DSCHED], 'table', "a door schedule")
rr = sh_run_ok(HOUSE + DOORS + [DSCHED, cmd('tableexport', {"handle": table}),
                                cmd('door', {"wall": "w4", "offset": 4.0, "width": 0.9, "height": 2.1}),
                                cmd('tableexport', {"handle": table}),
                                cmd('schedule.edit', {"id": "sc0", "columns": ["number", "count", "width"]}),
                                cmd('tableexport', {"handle": table}), INSPECT21], "the door schedule")
cells = sh_csv(rr, 0)
if not cells or not cells[0] or cells[0][0].strip() != 'Doors':
    fail(f"the TABLE's title row is the schedule's name 'Doors': {cells[:1]}")
header, data = sh_table_rows(cells, "the door TABLE")
if header != ['No.', 'Count', 'Width', 'Height']:
    fail(f"the door TABLE's header is the chosen columns: want ['No.', 'Count', 'Width', 'Height'], got {header}")
if data != [['D01', '2', '0.900', '2.100'], ['D02', '1', '0.800', '2.100']]:
    fail(f"door rows: want [D01 2 0.900 2.100], [D02 1 0.800 2.100], got {data}")
header, data = sh_table_rows(sh_csv(rr, 1), "the door TABLE after a new door")
if [r[:2] for r in data] != [['D01', '3'], ['D02', '1']]:
    fail(f"a new 0.9 x 2.1 door joins D01 live: {data}")
header, data = sh_table_rows(sh_csv(rr, 2), "the door TABLE after schedule.edit")
if header != ['No.', 'Count', 'Width'] or [len(r) for r in data] != [3, 3]:
    fail(f"schedule.edit columns rewrites the TABLE: header {header}, rows {data}")
sc = sh_one(sh_list(inspected(rr), 'schedules'), 'id', 'sc0', 'schedules')
if sc.get('columns') != ['number', 'count', 'width'] or sc.get('table') != table:
    fail(f"arch.inspect reports the edited columns and the same TABLE: {json.dumps(sc)[:300]}")

# (2) Cabinets: two 0.6 base cabinets and a 0.9 one -> C01 x 2, C02 x 1.
CABS = [SH_CAB, cmd('cabinet', {"kind": "base", "wall": "w5", "offset": 3.1}),
        cmd('cabinet', {"kind": "base", "wall": "w5", "offset": 4.2, "width": 0.9})]
rr = sh_run_ok(HOUSE + CABS + [cmd('schedule', {"kind": "cabinet"}), INSPECT21], "the cabinet schedule")
sc = sh_one(sh_list(inspected(rr), 'schedules'), 'kind', 'cabinet', 'schedules')
if sh_counts(sc) != [('C01', 2), ('C02', 1)]:
    fail(f"cabinet schedule: want C01 x 2 and C02 x 1, got {sh_counts(sc)}")
if sc.get('columns') != ['number', 'count', 'kind', 'width', 'depth', 'height', 'fronts']:
    fail(f"the cabinet schedule's default columns: {sc.get('columns')}")
row = sh_one(sc.get('rows') or [], 'number', 'C02', 'cabinet rows')
if (row.get('cells') or [None] * 4)[3] != '0.900':
    fail(f"C02's width cell is 0.900: {row}")

# (3) Room finishes: the north-west room named Study, a wood floor and its east wall face painted #c04020.
ROOMS = [cmd('room', {"id": NW, "name": "Study"}), cmd('material', {"rooms": [NW], "finish": "wood"}),
         cmd('paint.material', {"material": "#c04020", "target": "wall/w5/left", "scope": "component"}),
         cmd('schedule', {"kind": "room"})]
rr = sh_run_ok(HOUSE + ROOMS + [INSPECT21], "the room-finish schedule")
insp = inspected(rr)
sc = sh_one(sh_list(insp, 'schedules'), 'kind', 'room', 'schedules')
if sc.get('columns') != ['name', 'floor', 'walls', 'ceiling', 'area']:
    fail(f"the room schedule's default columns: {sc.get('columns')}")
rows = sc.get('rows') or []
if len(rows) != len(objects_named(insp, 'rooms', 'rooms')) or any(r.get('count') != 1 for r in rows):
    fail(f"one row per room, count 1: {sh_counts(sc)}")
study = [r for r in rows if (r.get('cells') or [''])[0] == 'Study']
if len(study) != 1 or study[0].get('members') != [NW]:
    fail(f"one row named Study for {NW}: {json.dumps(rows)[:400]}")
c = study[0].get('cells') or []
if len(c) != 5 or c[1] != 'wood' or '#c04020' not in c[2].split(', ') or c[4] != '28.37':
    fail(f"Study's cells: want floor 'wood', walls including '#c04020', area '28.37' (5.85 x 4.85), got {c}")

# (4) schedule.delete removes the TABLE.
rc, rr, so, se = run(HOUSE + DOORS + [DSCHED, cmd('schedule.delete', {"id": "sc0"}), INSPECT21,
                                      cmd('tableexport', {"handle": table})], expect_ok=False)
errs = errors(se)
if len(errs) != 1 or 'tableexport' not in errs[0].lower() and 'table' not in errs[0].lower():
    fail(f"after schedule.delete only tableexport of its TABLE is refused: {errs or se[-300:]}")
if sh_list(inspected(rr), 'schedules') != []:
    fail(f"schedule.delete leaves no schedule: {inspected(rr).get('schedules')}")
print("ok: doors D01 x 2 + D02 x 1 (live D01 x 3), edited columns, cabinets C01 x 2 + C02 x 1, Study wood / #c04020 / "
      "28.37, delete removes the TABLE")
print("PASS 03")
