#!/usr/bin/env python3
# Auto exterior dimensions (the brief's number "the overall row is 10.2 on the south and north sides and 8.2 on the
# east and west; the north wall row is 6.1 + 4.1, split at the interior wall's centre line x = 6"): dimauto makes per
# side up to three rows (openings; walls, with interior walls at their centre lines; overall), each on a fixed line
# beyond the outer face (openings 1.0, walls 1.5, overall 2.0), as associative dimensions on A-ANNO-DIMS in style Arch.
# They regenerate with the model in the same command (a window adds the openings row, an interior wall a walls row,
# moving w5 re-reads the row; undo restores it); a row edited by hand is marked edited and left alone; dimauto off
# removes them; they survive a DXF round trip; each level has its own.
# A stub fails: no dimauto, no autoDims rows.
# needs: p20-observables-1, p20-observables-2, p20-observables-3, p20-dims-model, p20-auto-dims-1, p20-auto-dims-2
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p20lib.inc')).read())

tmp = tempfile.mkdtemp()
AUTO = cmd('dimauto', {"level": 0})
LOOK = cmd('arch.inspect', {"autoDims": True, "dimensions": True})
WALL4 = cmd('wall', {"points": [[4, 0], [4, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"})
WALL8 = cmd('wall', {"points": [[8, 0], [8, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"})
LINES = {('north', 'openings'): ('y', 9.1), ('north', 'walls'): ('y', 9.6), ('north', 'overall'): ('y', 10.1),
         ('south', 'walls'): ('y', -1.6), ('south', 'overall'): ('y', -2.1),
         ('east', 'walls'): ('x', 11.6), ('east', 'overall'): ('x', 12.1),
         ('west', 'walls'): ('x', -1.6), ('west', 'overall'): ('x', -2.1)}


def on_line(rows, side, kind, nth=-1):
    """Every dimension of the row has its dimension line ("at") on the row's fixed line."""
    row = pv_auto_row(pv_auto(rows, nth), side, kind)
    dims = pv_dims(rows, nth)
    axis, v = LINES[(side, kind)]
    for h in row.get('handles') or []:
        at = pv_xy(pv_dim(dims, h).get('at'), f"{side} {kind} at")
        if abs(at[0 if axis == 'x' else 1] - v) > 1e-6:
            fail(f"the {side} {kind} row's dimension line is at {axis} = {v}: {h} has at {at}")
    return row


# (a) THE NUMBER.
rr = run_ok(HOUSE + [AUTO, LOOK], "dimauto on the house")
auto = pv_auto(rr)
for side, want in (('south', [10.2]), ('north', [10.2]), ('east', [8.2]), ('west', [8.2])):
    pv_values(pv_auto_row(auto, side, 'overall'), want, f"THE NUMBER: the {side} overall row")
pv_values(pv_auto_row(auto, 'north', 'walls'), [6.1, 4.1], "THE NUMBER: the north walls row")
for side in ('east', 'west'):
    pv_values(pv_auto_row(auto, side, 'walls'), [3.1, 5.1], f"the {side} walls row")
if any(r.get('side') == 'south' and r.get('row') == 'walls' for r in auto) or any(r.get('row') == 'openings' for r in auto):
    fail(f"no south walls row (it equals the overall row) and no openings rows: "
         f"{[(r.get('side'), r.get('row')) for r in auto]}")
dims = pv_dims(rr)
for r in auto:
    if r.get('edited') is not False or r.get('level') != 0:
        fail(f"a fresh row is level 0 and not edited: {r}")
    for h in r.get('handles') or []:
        d = pv_dim(dims, h)
        if d.get('layer') != 'A-ANNO-DIMS' or d.get('style') != 'Arch' or len(d.get('attached') or []) != 2:
            fail(f"auto dimensions are on A-ANNO-DIMS in style Arch with both points attached: {d}")
nw = pv_auto_row(auto, 'north', 'walls')
if pv_attached(pv_dim(dims, nw['handles'][0])).get('p14') != ('w5', 'centre'):
    fail(f"the north walls row's split point is on w5's centre line: {pv_dim(dims, nw['handles'][0])}")
for key in LINES:
    if key[1] != 'openings' and key != ('south', 'walls'):
        on_line(rr, *key)
print("ok (a): overall 10.2 / 10.2 / 8.2 / 8.2, north walls 6.1 + 4.1 at w5's centre line, east and west 3.1 + 5.1; "
      "on A-ANNO-DIMS, style Arch, each row on its fixed line")

# (b) regenerated with the model.
rr = run_ok(HOUSE + [AUTO, LOOK, PV_WINDOW, LOOK, WALL4, LOOK,
                     cmd('arch.wall.stretch', {"wall": "w5", "end": "start", "to": [5.5, 3]}),
                     cmd('arch.wall.stretch', {"wall": "w5", "end": "end", "to": [5.5, 8]}), LOOK,
                     'undo {}', 'undo {}', LOOK], "regeneration")
a0, a1, a2 = pv_auto(rr, 0), pv_auto(rr, 1), pv_auto(rr, 2)
op = on_line(rr, 'north', 'openings', 1)
pv_values(op, [4.1, 2.0, 4.1], "the window adds the north openings row")
for kind in ('walls', 'overall'):
    if pv_auto_row(a1, 'north', kind).get('handles') != pv_auto_row(a0, 'north', kind).get('handles'):
        fail(f"adding the window keeps the north {kind} row's dimensions (same handles)")
    on_line(rr, 'north', kind, 1)
sw = on_line(rr, 'south', 'walls', 2)
pv_values(sw, [4.1, 6.1], "the interior wall at x = 4 adds the south walls row")
if pv_auto_row(a2, 'south', 'overall').get('handles') != pv_auto_row(a0, 'south', 'overall').get('handles'):
    fail("the south overall row keeps its dimensions when the walls row appears inside it")
on_line(rr, 'south', 'overall', 2)
pv_values(pv_auto_row(pv_auto(rr, 3), 'north', 'walls'), [5.6, 4.6], "after moving w5 to x = 5.5")
pv_values(pv_auto_row(pv_auto(rr, 4), 'north', 'walls'), [6.1, 4.1], "after undoing the two stretches")
if any(r.get('edited') for r in pv_auto(rr, 4)):
    fail(f"no row is marked edited by edits and undo: {pv_auto(rr, 4)}")
print("ok (b): a window adds the openings row 4.1 + 2.0 + 4.1 on y = 9.1, a wall at x = 4 the south walls row; kept "
      "rows keep their handles and lines; moving w5 reads 5.6 + 4.6; undo restores 6.1 + 4.1")

# (c) a row edited by hand is left alone.
dxf = os.path.join(tmp, 'auto.dxf')
first = pv_auto_row(pv_auto(run_ok(HOUSE + [AUTO, WALL4, LOOK], "the south walls row")), 'south', 'walls')['handles']
rr = run_ok(HOUSE + [AUTO, WALL4, cmd('dimedit', {"handles": [first[0]], "mode": "new", "text": "EDITED"}), LOOK,
                     WALL8, LOOK], "a hand-edited row", save=dxf)
e0, e1 = pv_auto_row(pv_auto(rr, 0), 'south', 'walls'), pv_auto_row(pv_auto(rr, 1), 'south', 'walls')
if e0.get('edited') is not True:
    fail(f"dimedit marks its row edited: {e0}")
if e1.get('handles') != first or e1.get('edited') is not True or pv_dim(pv_dims(rr, 1), first[0]).get('text') != "EDITED":
    fail(f"the edited row is left alone by the wall at x = 8 (same two dimensions, text EDITED): {e1}")
pv_values(pv_auto_row(pv_auto(rr, 1), 'south', 'overall'), [10.2], "the south overall row")
print("ok (c): a hand-edited row is marked edited and not regenerated")

# (d) dimauto off.
rr = run_ok(HOUSE + [AUTO, cmd('dimauto', {"level": 0, "on": False}), LOOK], "dimauto off")
if pv_auto(rr) or any(d.get('layer') == 'A-ANNO-DIMS' for d in pv_dims(rr)):
    fail(f"dimauto off removes every auto row and its dimensions: {pv_auto(rr)}")
print("ok (d): dimauto off removes the rows and their dimensions")

# (e) persistence.
rr = run_ok([LOOK], "the reopened drawing", file=dxf)
r = pv_auto_row(pv_auto(rr), 'south', 'walls')
if r.get('handles') != first or r.get('edited') is not True:
    fail(f"the rows, handles and edited flag survive a DXF round trip: {r}")
print("ok (e): rows, handles and the edited flag survive DXF")

# (f) two levels.
UP = [cmd('level', {"above": 0}),
      cmd('wall', {"points": [[0, 0], [6, 0], [6, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4,
                   "exterior": True, "level": 1})]
rr = run_ok(HOUSE + [AUTO, LOOK] + UP + [cmd('dimauto', {"level": 1}), LOOK, cmd('level.show', {"level": 1}),
                                         cmd('dimauto', {}), cmd('arch.inspect', {"autoDims": True})], "two levels")
up = pv_auto(rr, 1)
for side, want in (('south', [6.2]), ('north', [6.2]), ('east', [8.2]), ('west', [8.2])):
    pv_values(pv_auto_row(up, side, 'overall', level=1), want, f"level 1's {side} overall row")
lv0 = lambda a: sorted((r.get('side'), r.get('row'), tuple(r.get('handles') or [])) for r in a if r.get('level') == 0)
if lv0(up) != lv0(pv_auto(rr, 0)):
    fail("level 0's rows are unchanged by level 1's")
rep = [r for r in rr if isinstance(r, dict) and 'dimensions' in r and 'rows' in r and 'level' in r]
if len(rep) != 3 or rep[-1].get('level') != 1:
    fail(f"dimauto {{}} after level.show 1 addresses level 1: {rep[-1:] }")
print("ok (f): level 1 has its own rows (6.2 / 8.2); level 0's are unchanged; dimauto {} takes the shown level")
print("PASS 08")
