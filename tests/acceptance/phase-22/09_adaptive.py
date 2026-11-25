# Rung 6, quality and time. On the showcase kitchen camera the noise target is reached with at most 60 % of the samples a
# uniform render needs for the same error (both stop on the same metric: the 95th percentile over hit pixels of the
# relative error of the mean, floored at 5 % of the frame mean); two adaptive renders with different seeds really differ
# by no more than that error allows. A 60 s time budget stops within 65 s and reports time budget. The target is 0.08
# on a GPU and 0.40 on the software adapter (the 60 % rule does not depend on it).
# needs: p22-doc-look, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-adaptive
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
# Software adapter (operator 2026-11-24): target 0.40 on a 48 x 30 frame. At 0.25 on 64 x 40 the three kitchen renders took
# 646 s and the whole check 963 s run alone on nb4, over the gate's 900 s, so a working fix could only time out.
T = 0.08 if BIG else 0.40
SIZE = (240, 150) if BIG else (48, 30)
base = dict(final='path', noiseTarget=T, spp=65536, timeBudget=3000, bounces=2, denoise=False)
pa, pb, pu = [os.path.join(tmp, x + '.pfm') for x in ('a1', 'a2', 'u')]
# Fail fast while adaptive sampling is missing. Without it the three kitchen renders below ignore `adaptive` and
# `noiseTarget`, run 65536 uniform samples each and end only at the harness's timeout (2026-11-23: 15 minutes of every
# merge while the phase was being built). A render that knows the keys says why it stopped.
PROBE = {'kind': 'full', 'eye': [2.0, 0.3, 2.1], 'target': [2.0, 2.6, 0.0], 'fov': 90}
rc, rows, so, se = run(ROOM + [render_cmd(os.path.join(tmp, 'probe.png'), PROBE, (32, 20), final='path', adaptive=True, noiseTarget=0.9,
                                          spp=16, seed=1, bounces=1, denoise=False)], expect_ok=False, timeout=600)
pr = [r for r in render_replies(rows) if r.get('finalMode') == 'path'][-1:]
if rc != 0 or not pr or pr[0].get('stop') not in ('samples', 'noise target', 'time budget'):
    fail('an adaptive render says why it stopped (reply `stop`): rc %d, stop %r: %s' % (rc, pr[0].get('stop') if pr else None, errors(se)[-2:]))
cmds = showcase_cmds() + [
    render_cmd(os.path.join(tmp, 'a1.png'), 'Kitchen', SIZE, adaptive=True, seed=1, pfm=pa, **base),
    render_cmd(os.path.join(tmp, 'a2.png'), 'Kitchen', SIZE, adaptive=True, seed=2, pfm=pb, **base),
    render_cmd(os.path.join(tmp, 'u.png'), 'Kitchen', SIZE, adaptive=False, seed=1, pfm=pu, **base)]
rc, rows, so, se = run(cmds, expect_ok=False, timeout=14000)
rs = [r for r in render_replies(rows) if r.get('finalMode') == 'path'][-3:]
if rc != 0 or len(rs) != 3:
    fail('three path-traced renders of the showcase Kitchen camera (rc %d, %d replies): %s' % (rc, len(rs), (errors(se)[-3:] or se[-600:])))
for r, name in zip(rs, ('adaptive seed 1', 'adaptive seed 2', 'uniform')):
    if r.get('stop') != 'noise target':
        fail('%s stops at the noise target: stop %r after sppMax %r, noise %r' % (name, r.get('stop'), r.get('sppMax'), r.get('noise')))
    if not num(r.get('noise'), 'noise') <= T:
        fail('%s reports noise %.4f above its target %.2f' % (name, r['noise'], T))
    print('%-16s samples %d, spp min %s mean %.1f max %s, noise %.4f, %.1f s' % (name, r.get('samplesTotal'), r.get('sppMin'), num(r.get('sppMean'), 'sppMean'), r.get('sppMax'), r['noise'], num(r.get('seconds'), 'seconds')))
a, u = rs[0], rs[2]
if photo_of(a, 'adaptive').get('adaptive') is not True or photo_of(u, 'uniform').get('adaptive') is not False:
    fail('photo.adaptive echoes the request')
pixels_n = SIZE[0] * SIZE[1]
if abs(num(u.get('sppMin'), 'sppMin') - num(u.get('sppMax'), 'sppMax')) > 1e-9 or abs(u['samplesTotal'] - pixels_n * u['sppMax']) > 0.5:
    fail('the uniform render gives every pixel the same samples: min %r max %r total %r' % (u.get('sppMin'), u.get('sppMax'), u.get('samplesTotal')))
if abs(a['samplesTotal'] - pixels_n * a['sppMean']) > 0.01 * a['samplesTotal'] + 1:
    fail('samplesTotal is the pixels times sppMean: %r vs %r' % (a['samplesTotal'], pixels_n * a['sppMean']))
if not num(a.get('sppMax'), 'sppMax') >= 2 * max(1.0, num(a.get('sppMin'), 'sppMin')):
    fail('the adaptive render puts its samples where the variance is: spp min %r, max %r' % (a.get('sppMin'), a.get('sppMax')))
ratio = a['samplesTotal'] / float(u['samplesTotal'])
print('adaptive / uniform samples for the same error: %.3f' % ratio)
if ratio > 0.60:
    fail('the noise target is reached with at most 60 %% of the samples a uniform render needs: %.1f %%' % (100 * ratio))
w, h, la = pfm_lumas(pa)
_, _, lb = pfm_lumas(pb)
mean = sum(la) / len(la)
if mean <= 0:
    fail('the kitchen render is black')
d = [abs(x - y) / math.sqrt(2) / max((x + y) / 2, 0.05 * mean) for x, y in zip(la, lb)]
p95 = percentile(d, 0.95)
print('two seeds differ by %.4f at the 95th percentile (target %.2f)' % (p95, T))
if p95 > 3 * T:
    fail('the reported noise is real: two seeds differ by %.3f at the 95th percentile, more than 3 x the target %.2f' % (p95, T))
if p95 <= 0:
    fail('two seeds give the same image')

# the time budget, in the phase-10 room at night.
NC = {'kind': 'full', 'eye': [2.0, 0.3, 2.1], 'target': [2.0, 2.6, 0.0], 'fov': 90}
SCN = ROOM + [COLUMN, cmd('lightfixture', {'room': 'r0', 'rows': 2, 'cols': 2})]
tb = dict(final='path', adaptive=True, noiseTarget=0.001, spp=65536, seed=1, denoise=False, sun={'on': False}, sky=False)


def budget(seconds):
    t0 = time.time()
    rows, rs = rendered(SCN + [render_cmd(os.path.join(tmp, 'tb%d.png' % seconds), NC, (160, 120), timeBudget=seconds, **tb)], 'time budget %d s' % seconds, timeout=3000)
    return rs[-1], time.time() - t0


r1, wall1 = budget(1)
r60, wall60 = budget(60)
for r, s in ((r1, 1), (r60, 60)):
    if r.get('stop') != 'time budget':
        fail('a %d s budget with an unreachable noise target stops on the time budget: %r' % (s, r.get('stop')))
print('budget 1 s: %.1f s (wall %.1f); budget 60 s: %.1f s (wall %.1f)' % (num(r1.get('seconds'), 'seconds'), wall1, num(r60.get('seconds'), 'seconds'), wall60))
if not r1['seconds'] <= 6:
    fail('a 1 s budget stops within 6 s: %.1f' % r1['seconds'])
if not 55 <= r60['seconds'] <= 65:
    fail('a 60 s budget stops within 65 s (and uses its time): %.1f s' % r60['seconds'])
if wall60 - wall1 > 65 + 30:
    fail('the 60 s render took %.0f s of wall time more than the 1 s one' % (wall60 - wall1))
if not r60['samplesTotal'] > 2 * r1['samplesTotal']:
    fail('the longer budget traces more samples: %r vs %r' % (r60['samplesTotal'], r1['samplesTotal']))
print('PASS 09')
