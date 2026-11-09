#!/usr/bin/env python3
# The incremental rebuild: each phase-16 edit rebuilds only what depends on it, and the incremental scene equals a full
# build (same digest). The set-up is the house under the gable with a tray in NW, a soffit in NE (it overlaps NE only)
# and a stem foundation, no lights or library items, built once incrementally; then, one edit at a time, an incremental
# build with a digest and a full build with a digest (the full build replaces the cache, so each incremental build
# measures one edit). The counts are the dependency formula of p16-rebuild applied to this set-up:
#   tray.edit depth                 -> 1 (NW)
#   soffit.edit bottom              -> 1 (the soffit)
#   room NE ceilingStack drywall    -> 2 (NE, and the soffit whose top follows NE's finished ceiling)
#   room S flatCeiling false        -> 3 (S, and w0 and w4, which rise to the vault)
#   foundation.edit footingDepth    -> 1 (the foundation)
#   foundation.edit type slab       -> 4 (the foundation, and the three rooms whose floors become slabs)
# A stub fails: a full rebuild per edit (or none) gives other counts, a stale cache another digest.
# needs: p16-vault-walls, p16-trays, p16-soffits, p16-foundation-slab, p16-rebuild
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p16lib.inc')).read())

R = house_rooms()
S, NW, NE = R['S'], R['NW'], R['NE']
SETUP = HOUSE + [GABLE, cmd('tray', {"room": NW, "inset": 0.6, "depth": 0.3}),
                 cmd('soffit', {"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1}),
                 cmd('foundation', {"type": "stem"}), cmd('build3d', {"incremental": True})]
rr = run_ok(SETUP, "the set-up (tray, soffit, stem foundation)")
T0 = reply_with(rr, 'tray', -1, 'tray').get('tray')
S0 = reply_with(rr, 'soffit', -1, 'soffit').get('soffit')
FD = ((replies_with_list(rr, 'foundations') or [{}])[-1].get('foundations') or [None])[0]
if not all(isinstance(x, str) for x in (T0, S0, FD)):
    fail(f"the set-up's tray, soffit and foundation labels: {T0!r}, {S0!r}, {FD!r}")

EDITS = [(cmd('tray.edit', {"id": T0, "depth": 0.25}), 1, f"{NW}"),
         (cmd('soffit.edit', {"id": S0, "bottom": 2.0}), 1, f"{S0}"),
         (cmd('room', {"id": NE, "ceilingStack": "drywall"}), 2, f"{NE} and {S0}"),
         (cmd('room', {"id": S, "flatCeiling": False}), 3, f"{S}, w0 and w4"),
         (cmd('foundation.edit', {"id": FD, "footingDepth": 1.5}), 1, f"{FD}"),
         (cmd('foundation.edit', {"id": FD, "type": "slab"}), 4, f"{FD} and the three rooms' slab floors")]
steps = list(SETUP)
for edit, _, _ in EDITS:
    steps += [edit, cmd('build3d', {"incremental": True, "digest": True}), cmd('build3d', {"digest": True})]
rr = run_ok(steps, "the six edits, each followed by an incremental and a full build")
b = [r for r in rr if isinstance(r, dict) and 'rebuilt' in r and isinstance(r.get('digest'), str)]
if len(b) != 2 * len(EDITS):
    fail(f"{2 * len(EDITS)} build3d replies with digests, got {len(b)}")
for k, (edit, want, what) in enumerate(EDITS):
    inc, full = b[2 * k], b[2 * k + 1]
    if inc.get('incremental') is not True:
        fail(f"after `{edit}`: the build is incremental, got incremental {inc.get('incremental')!r}")
    if inc.get('digest') != full.get('digest'):
        fail(f"after `{edit}`: the incremental scene's digest {inc.get('digest')} differs from the full build's {full.get('digest')}")
    if inc.get('rebuilt') != want:
        fail(f"`{edit}` rebuilds {want} object(s) ({what}), it rebuilt {inc.get('rebuilt')!r}")
    print(f"ok: `{edit}` rebuilt {want} ({what}); incremental == full")
print("PASS 10")
