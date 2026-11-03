# The 300-wall house rebuilds incrementally: after `arch.wall.stretch` of one interior wall by 0.3 m,
# `build3d {"incremental": true}` reports rebuildMs under 50 (release, median of 5 edits) and rebuilt at most 10 % of the
# objects (so a fast full rebuild cannot pass); a full build stays under 500 ms; an update without an edit rebuilds
# nothing; and the incrementally kept scene is the full build's, exactly (equal digests), after edits and after undo.
# needs: p13-sample-house, p13-arch3d-incremental, p13-build3d-incremental
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

SAMPLE = cmd('arch.sample', {"house": True})
rc, rr, so, se = run([SAMPLE], release=True)
bench = [r for r in rr if isinstance(r, dict) and isinstance(r.get('bench'), dict)]
if not bench:
    fail(f"arch.sample {{house: true}} names its bench edit: {json.dumps(rr)[:400]}")
b = bench[-1]['bench']
st = lambda to: cmd('arch.wall.stretch', {"wall": b['wall'], "end": b['end'], "to": to})
INC = cmd('build3d', {"incremental": True})
session = [SAMPLE, cmd('build3d', {}), INC]
for k in range(5):
    session += [st(b['to'] if k % 2 == 0 else b['from']), INC]
session += [cmd('build3d', {"incremental": True, "digest": True}), cmd('build3d', {"digest": True}),
            'undo {}', cmd('build3d', {"incremental": True, "digest": True}), cmd('build3d', {"digest": True})]


def measure():
    rc, rr, so, se = run(session, release=True, timeout=1800)
    if was_refused(rc, se):
        fail(f"the benchmark session ran without a refused step: {se[-500:]}")
    builds = [r for r in rr if isinstance(r, dict) and 'rebuildMs' in r]
    if len(builds) != 11:
        fail(f"eleven build3d replies: {len(builds)}")
    return builds


def judge(builds):
    full, idle, edits = builds[0], builds[1], builds[2:7]
    objects = num(full.get('objects'), 'objects')
    problems = []
    if full.get('incremental') is not False or full.get('rebuilt') != full.get('objects'):
        problems.append(f"a full build reports incremental false and rebuilt = objects: {dict((k, full.get(k)) for k in ('incremental', 'rebuilt', 'objects'))}")
    if objects < 450:
        problems.append(f"the house has at least 450 objects (walls, openings, rooms, the roof): {objects}")
    if num(full.get('rebuildMs'), 'full rebuildMs') >= 500:
        problems.append(f"a full build under 500 ms: {full.get('rebuildMs')}")
    if idle.get('incremental') is not True or idle.get('rebuilt') != 0:
        problems.append(f"an incremental build with no edit rebuilds nothing: {dict((k, idle.get(k)) for k in ('incremental', 'rebuilt'))}")
    for i, e in enumerate(edits):
        if e.get('incremental') is not True or not (1 <= num(e.get('rebuilt'), 'rebuilt') <= 0.10 * objects):
            problems.append(f"edit {i + 1}: incremental, rebuilt 1..{int(0.1 * objects)} of {int(objects)}: "
                            f"{dict((k, e.get(k)) for k in ('incremental', 'rebuilt', 'objects', 'rebuildMs'))}")
    ms = sorted(num(e.get('rebuildMs'), 'rebuildMs') for e in edits)
    med = ms[2]
    if med >= 50:
        problems.append(f"median incremental rebuild under 50 ms: {med:.2f} ({ms})")
    return med, ms, problems


builds = measure()
med, ms, problems = judge(builds)
print(f"full {builds[0].get('rebuildMs')} ms, {builds[0].get('objects')} objects; incremental {ms} ms (median {med:.2f}), "
      f"rebuilt {[e.get('rebuilt') for e in builds[2:7]]}")
timing = [p for p in problems if 'ms' in p and ('median' in p or 'full build' in p)]
if timing and len(timing) == len(problems):
    print(f"timing missed once ({timing}); measuring again (the swarm host may be busy)")
    builds = measure()
    med, ms, problems = judge(builds)
    print(f"again: full {builds[0].get('rebuildMs')} ms; incremental {ms} ms (median {med:.2f})")
if problems:
    fail('; '.join(problems))
d_inc, d_full = builds[7].get('digest'), builds[8].get('digest')
if not (isinstance(d_inc, str) and re.fullmatch(r'[0-9a-f]{16}', d_inc)) or d_inc != d_full:
    fail(f"after five edits the incrementally kept scene is the full build's: digests {d_inc} vs {d_full}")
u_inc, u_full = builds[9].get('digest'), builds[10].get('digest')
if builds[9].get('incremental') is not True or u_inc != u_full:
    fail(f"after undo the incremental scene is the full build's: {u_inc} vs {u_full} (incremental {builds[9].get('incremental')})")
if u_inc == d_inc:
    fail("undo changed the model, so its digest differs from before the undo")
print(f"ok: median {med:.2f} ms, at most {max(e.get('rebuilt') for e in builds[2:7])} of {builds[0].get('objects')} objects "
      f"rebuilt, digests equal after edits and undo")
print("PASS 08")
