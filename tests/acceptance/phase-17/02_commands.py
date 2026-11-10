#!/usr/bin/env python3
# Commands: every phase-17 parameter is stored, reported and saved. stair takes openRisers, stringers ("centre" and
# "center"), underside, railLeft/railRight and planCut; stair.edit changes only the keys given and is one undo step;
# deck takes boards (degrees, normalised into [0, 180)) and rails (one per edge); railing takes postSpacing and inset
# (posts move left of travel); window takes grille {across, down, part}, sillSides, casingSides and top "round"; door
# takes style "fixed". Everything round-trips through a DXF save. Refusals: stair.edit with nothing to change,
# deck.edit on a room that is not a deck, a door with a shaped top. A stub that ignores the parameters fails.
# needs: p17-observables, p17-commands
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

R = house_rooms()
tmp = tempfile.mkdtemp()
saved = os.path.join(tmp, 'p17.dxf')
STAIR = stair_cmd(openRisers=True, stringers="centre", underside=0.4, railRight="off", planCut=1.5)
EDIT = cmd('stair.edit', {"id": "s0", "stringers": "none"})
DECK = deck_cmd(DECK_RING, boards=120, rails=["auto", "on", "off", "auto"])
RAIL = cmd('railing', {"path": [[12, 0], [16, 0]], "postSpacing": 1.0, "inset": 0.1})
WIN = window4(grille={"across": 2, "down": 3, "part": "top"}, sillSides="both", casingSides="none")
ROUND = cmd('window', {"wall": "w0", "offset": 3.0, "width": 0.9, "height": 1.5, "sill": 0.6, "top": "round"})
FIXED = cmd('door', {"wall": "w4", "offset": 8.0, "width": 0.6, "height": 2.1, "style": "fixed"})
SETUP = HOUSE2 + [STAIR, EDIT, DECK, RAIL, WIN, ROUND, FIXED]


def check_model(insp, what):
    s = by_id(stairs_of(insp), 's0', f'{what}: stairs')
    for key, want in (('openRisers', True), ('stringers', 'none'), ('railRight', 'off'), ('railLeft', 'auto')):
        if s.get(key) != want:
            fail(f"{what}: stair s0 {key} is {want!r}, got {s.get(key)!r}")
    near(s.get('underside'), 0.4, 1e-9, f"{what}: s0 underside")
    near(s.get('planCut'), 1.5, 1e-9, f"{what}: s0 planCut")
    dk = deck_room(insp)
    near(dk.get('boards'), 120.0, 1e-9, f"{what}: the deck's boards")
    if dk.get('rails') != ["auto", "on", "off", "auto"]:
        fail(f"{what}: the deck's rails are ['auto', 'on', 'off', 'auto'], got {dk.get('rails')!r}")
    drawn = railings_by_host(insp, None)
    if len(drawn) != 1:
        fail(f"{what}: one drawn railing (host null): {[(r.get('id'), r.get('host')) for r in railings_of(insp)]}")
    d = drawn[0]
    near(d.get('postSpacing'), 1.0, 1e-9, f"{what}: the drawn railing's postSpacing")
    near(d.get('inset'), 0.1, 1e-9, f"{what}: the drawn railing's inset")
    same_points(d.get('posts'), [[12, 0.1], [13, 0.1], [14, 0.1], [15, 0.1], [16, 0.1]], 1e-6, f"{what}: posts at spacing 1.0, inset 0.1 (left of +x is +y)")
    ops = openings_of(insp)
    w = [o for o in ops if o.get('host') == 'w2']
    rd = [o for o in ops if o.get('host') == 'w0']
    dr = [o for o in ops if o.get('host') == 'w4']
    if len(w) != 1 or len(rd) != 1 or len(dr) != 1:
        fail(f"{what}: one opening on each of w2, w0 and w4: {[(o.get('id'), o.get('host')) for o in ops]}")
    if w[0].get('grille') != {"across": 2, "down": 3, "part": "top"}:
        fail(f"{what}: the w2 window's grille is {{across 2, down 3, part top}}, got {w[0].get('grille')!r}")
    if w[0].get('sillSides') != 'both' or w[0].get('casingSides') != 'none':
        fail(f"{what}: the w2 window's sides: sill both, casing none; got {w[0].get('sillSides')!r}, {w[0].get('casingSides')!r}")
    if rd[0].get('top') != 'round' or rd[0].get('kind') != 'window':
        fail(f"{what}: the w0 window has top round: {rd[0].get('top')!r}")
    if dr[0].get('style') != 'fixed' or dr[0].get('kind') != 'door':
        fail(f"{what}: the w4 door has style fixed: {dr[0].get('style')!r}")
    return s, dk, d


# 1. The replies, the model and the saved drawing.
rc, rr, so, se = run(SETUP + [INSPECT17], save=saved)
if errors(se):
    fail("a set-up step is refused:\n" + '\n'.join(errors(se)))
sr = reply_with(rr, 'risers', -1, 'stair')
for key, want in (('openRisers', True), ('stringers', 'centre'), ('railRight', 'off'), ('railLeft', 'auto')):
    if sr.get(key) != want:
        fail(f"the stair reply echoes {key} {want!r}: {json.dumps(sr)[:300]}")
if not isinstance(sr.get('railings'), list):
    fail(f"the stair reply lists its automatic railings (a list, empty until they exist): {json.dumps(sr)[:300]}")
er = reply_with(rr, 'stair', -1, 'stair.edit')
if er.get('stair') != 's0' or er.get('stringers') != 'none' or er.get('openRisers') is not True:
    fail(f"stair.edit replies with the stair and its values: {json.dumps(er)[:300]}")
dr_ = reply_with(rr, 'deckHeight', -1, 'deck')
if dr_.get('rails') != ["auto", "on", "off", "auto"] or dr_.get('boards') != 120:
    fail(f"the deck reply echoes boards and rails: {json.dumps(dr_)[:300]}")
rl = reply_with(rr, 'balusterGap', -1, 'railing')
same_points(rl.get('posts'), [[12, 0.1], [13, 0.1], [14, 0.1], [15, 0.1], [16, 0.1]], 1e-6, "the railing reply's posts")
wr = [r for r in rr if isinstance(r, dict) and isinstance(r.get('openings'), list) and r['openings'] and 'walls' not in r]
if len(wr) != 3 or wr[0]['openings'][0].get('grille') != {"across": 2, "down": 3, "part": "top"} or wr[1]['openings'][0].get('top') != 'round':
    fail(f"the opening replies carry grille and top: {json.dumps(wr)[:600]}")
if wr[0]['openings'][0].get('sillSides') != 'both' or wr[0]['openings'][0].get('casingSides') != 'none':
    fail(f"the window reply carries sillSides and casingSides: {json.dumps(wr[0])[:300]}")
check_model(inspected(rr), "after the commands")
print("ok: stair, stair.edit, deck, railing, window and door store and echo every phase-17 parameter")

# 2. Reopening the DXF gives the same values.
rc, rr, so, se = run([INSPECT17], file=saved)
check_model(inspected(rr), "after reopening the DXF")
print("ok: the values round-trip through the DXF")

# 3. stair.edit is one undo step: undo restores stringers "centre"; the deck edit path. (The deck's id comes from its own
# reply in the same sequence: the stair's Open Below room takes an id before it.)
D = label_of(run_ok(HOUSE2 + [STAIR, DECK], "the stair and the deck"), 'deckHeight', 'deck')
rr = run_ok(HOUSE2 + [STAIR, EDIT, cmd('undo', {}), INSPECT17, DECK,
                      cmd('deck.edit', {"id": D, "boards": 200, "rails": "off"}), INSPECT17,
                      cmd('deck.edit', {"id": D, "boards": None, "height": 1.2}), INSPECT17], "stair.edit + undo, deck.edit")
s = by_id(stairs_of(inspected(rr, 0)), 's0', 'stairs after undo')
if s.get('stringers') != 'centre' or s.get('openRisers') is not True:
    fail(f"undo of stair.edit restores stringers centre and keeps the other values: {json.dumps(s)[:300]}")
dk = deck_room(inspected(rr, 1))
near(dk.get('boards'), 20.0, 1e-9, "deck.edit boards 200 is stored as 20 (normalised into [0, 180))")
if dk.get('rails') != ["off", "off", "off", "off"]:
    fail(f"deck.edit rails \"off\" stores off for every edge: {dk.get('rails')!r}")
dk = deck_room(inspected(rr, 2))
if dk.get('boards') is not None:
    fail(f"deck.edit boards null clears the direction: {dk.get('boards')!r}")
near(dk.get('deckHeight'), 1.2, 1e-9, "deck.edit height 1.2")
print(f"ok: undo restores centre stringers; deck.edit on {D}: boards 200 -> 20, rails off, boards null, height 1.2")

# 4. Refusals write nothing.
BAD = [cmd('stair.edit', {"id": "s0"}), cmd('deck.edit', {"id": R['S'], "boards": 0}),
       cmd('door', {"wall": "w4", "offset": 2.0, "width": 0.9, "height": 2.1, "top": "round"})]
rc, rr, so, se = run(SETUP + BAD + [INSPECT17], expect_ok=False)
errs = errors(se)
if len(errs) != len(BAD):
    fail(f"each of the {len(BAD)} bad commands is refused with one error line (the set-up runs): {errs}")
insp = inspected(rr)
check_model(insp, "after the refusals")
if len(openings_of(insp)) != 3:
    fail(f"the refused door was not placed: {[(o.get('id'), o.get('host')) for o in openings_of(insp)]}")
print("ok: stair.edit with nothing to change, deck.edit on a plain room and a round-top door are refused")
print("PASS 02")
