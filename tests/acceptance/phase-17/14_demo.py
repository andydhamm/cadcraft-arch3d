#!/usr/bin/env python3
# The phase-17 demo. (1) docs/arch3d/showcase/phase-17-demo.json (replayed by `cadcraft-cli run --commands`): a small
# two-storey house whose stair sits on its well with two automatic side railings and is drawn with the full symbol in
# both plans (solid and dashed nosing lines, a break line and UP below; DN above), a rear deck framed (a ledger, at
# least 9 joists, posts) and railed, at least three gridded windows, one with a round top, a fixed sidelight; its two
# saved cameras and both level plans render non-blank. (2) The showcase house (docs/arch3d/showcase/bilevel.json):
# its two stairs get their automatic side railings, a rear deck with its railing, grilles on its windows and one
# round top; the Entry plan carries UP and DN stair symbols.
# A stub fails: a missing or partly refused commands file, no railings, no frame, no grille, old stair linework.
# needs: p17-stair-flight, p17-wall-under-flight, p17-stair-plan, p17-railing-auto, p17-deck-frame, p17-grilles-doors, p17-window-tops, p17-demo
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p17lib.inc')).read())

DEMO = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'phase-17-demo.json')
SHOWCASE = os.path.join(ROOT, 'docs', 'arch3d', 'showcase', 'bilevel.json')
tmp = tempfile.mkdtemp()


def steps_of(path):
    if not os.path.exists(path):
        fail(f"{os.path.relpath(path, ROOT)} is missing")
    try:
        steps = json.load(open(path, encoding='utf-8'))
    except ValueError as e:
        fail(f"{os.path.relpath(path, ROOT)} is not JSON: {e}")
    if not isinstance(steps, list) or not steps or not all(
            isinstance(st, dict) and isinstance(st.get('cmd'), str) and isinstance(st.get('params', {}), dict) for st in steps):
        fail(f"{os.path.relpath(path, ROOT)} is a non-empty JSON array of {{\"cmd\", \"params\"}} steps")
    return steps


def cameras_of(steps):
    return [st['params']['save'] for st in steps if st['cmd'] == 'camera' and isinstance(st['params'].get('save'), str)]


def check_house(insp, what, want_dashed):
    """The phase-17 features in an inspected model; returns (stairs, deck rooms)."""
    S = stairs_of(insp)
    if not S:
        fail(f"{what}: a stair")
    for s in S:
        for host in ('stairLeft', 'stairRight'):
            if not [r for r in railings_by_host(insp, host) if r.get('follows') == s['id'] and r.get('auto') is True]:
                fail(f"{what}: stair {s['id']} has an automatic {host} railing: "
                     f"{[(r.get('id'), r.get('host'), r.get('follows')) for r in railings_of(insp)]}")
    decks = decks_of(insp)
    framed = [d for d in decks if isinstance(d.get('ledger'), dict) and len(d.get('joists') or []) >= 9 and d.get('posts')]
    if not framed:
        fail(f"{what}: a deck against the house with a ledger, at least 9 joists and posts: "
             f"{[(d.get('id'), d.get('ledger'), len(d.get('joists') or []), len(d.get('posts') or [])) for d in decks]}")
    D = framed[0]['id']
    if not [r for r in railings_by_host(insp, 'deck') if r.get('follows') == D]:
        fail(f"{what}: deck {D} has its railing: {[(r.get('id'), r.get('host'), r.get('follows')) for r in railings_of(insp)]}")
    ops = openings_of(insp)
    gridded = [o for o in ops if isinstance(o.get('grille'), dict) and o['grille'].get('across', 1) * o['grille'].get('down', 1) > 1]
    if len(gridded) < 3:
        fail(f"{what}: at least three windows with grilles: {[(o.get('id'), o.get('grille')) for o in ops]}")
    if not [o for o in ops if o.get('top') == 'round']:
        fail(f"{what}: a round-top window: {[(o.get('id'), o.get('top')) for o in ops]}")
    return S, D


def check_plan(path, what, want_dashed, want_labels):
    prox = stair_proxies(path)
    if not prox:
        fail(f"{what}: CADCRAFT-STAIR proxies in the plan")
    lab = set(labels(prox))
    if not want_labels <= lab:
        fail(f"{what}: stair labels {sorted(want_labels)} in the plan, got {sorted(lab)}")
    if want_dashed and (not nosings(prox, True) or not nosings(prox, False) or not any(p['role'] == 'break' for p in prox)):
        fail(f"{what}: solid and dashed nosing lines and a break line (the flight is cut at 1.2 m): "
             f"{len(nosings(prox, False))} solid, {len(nosings(prox, True))} dashed, roles {sorted(set(p['role'] for p in prox))}")
    if want_dashed is False and nosings(prox, True):
        fail(f"{what}: no dashed nosing line on the upper level")


# 1. The phase-17 demo house.
steps = steps_of(DEMO)
CAMS = cameras_of(steps)
if len(CAMS) < 2:
    fail(f"the demo saves two cameras (camera {{save: name, ...}}): {CAMS}")
insp = inspected(replay_file(DEMO, [INSPECT17], "inspect"))
S, D = check_house(insp, "the demo", True)
if not [o for o in openings_of(insp) if o.get('kind') == 'door' and o.get('style') == 'fixed']:
    fail(f"the demo has a fixed sidelight: {[(o.get('id'), o.get('kind'), o.get('style')) for o in openings_of(insp)]}")
s = S[0]
lo, hi = s.get('fromLevel'), s.get('toLevel')
l1, l2 = os.path.join(tmp, 'demo-l1.dxf'), os.path.join(tmp, 'demo-l2.dxf')
p1, p2 = os.path.join(tmp, 'demo-plan-lower.png'), os.path.join(tmp, 'demo-plan-upper.png')
shots = [os.path.join(tmp, f'demo-camera-{k}.png') for k in range(2)]
replay_file(DEMO, [cmd('export', {"path": l1, "level": lo}), cmd('export', {"path": l2, "level": hi}),
                   cmd('export', {"path": p1, "level": lo}), cmd('export', {"path": p2, "level": hi})] +
            [cmd('render', {"out": sh, "size": [800, 600], "camera": c}) for sh, c in zip(shots, CAMS[:2])], "plans and renders")
check_plan(l1, "the demo's lower plan", True, {'UP'})
check_plan(l2, "the demo's upper plan", False, {'DN'})
for path, name in list(zip(shots, CAMS)) + [(p1, 'the lower plan'), (p2, 'the upper plan')]:
    if not os.path.exists(path):
        fail(f"{name}: no image written")
    frac, sd = foreground(path)
    if not (0.05 <= frac <= 0.99) or sd <= 2:
        fail(f"{name} renders non-blank: {frac * 100:.1f} % of the pixels differ from the clear colour (5..99 %), luma sd {sd:.2f} (> 2)")
    artifact(path, os.path.basename(path))
print(f"ok: the demo house: stair {s['id']} railed on both sides, deck {D} framed and railed, gridded windows and a round top, a fixed "
      f"sidelight; full stair symbols on both plans; cameras {CAMS[:2]} and both plans render non-blank")

# 2. The showcase house.
steps = steps_of(SHOWCASE)
insp = inspected(replay_file(SHOWCASE, [INSPECT17], "the showcase"))
S, D = check_house(insp, "the showcase", None)
if len(S) != 2:
    fail(f"the showcase keeps its two stairs: {[s.get('id') for s in S]}")
entry = level_row(insp, 'Entry')
le = os.path.join(tmp, 'showcase-entry.dxf')
replay_file(SHOWCASE, [cmd('export', {"path": le, "level": entry.get('index')})], "the showcase's Entry plan")
check_plan(le, "the showcase's Entry plan", None, {'UP', 'DN'})
print(f"ok: the showcase: both stairs railed, deck {D} framed and railed, grilles and a round top; UP and DN symbols on the Entry plan")
print("PASS 14")
