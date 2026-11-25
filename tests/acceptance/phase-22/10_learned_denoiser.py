# Rung 7, the learned denoiser (a U-Net run by our own WGSL compute shaders on the committed Open Image Denoise weights).
# (a) the weight loader reads 32 tensors from each file in assets/denoise and enc_conv0.weight is 32 x 9 x 3 x 3;
# (b) on a flat furnace frame it changes no pixel by more than 1 %; (c) on the showcase kitchen camera at 64 spp its
# mean relative squared error against a 4096 spp reference is at most half the edge-aware filter's; (d) a 1920 x 1080
# frame denoises in under 5 s on a GPU (on the software adapter a 320 x 180 frame is denoised and the time printed);
# (e) without compute shaders the still falls back and says so. No Open Image Denoise code is linked.
# (c) uses two independent half references (2026-11-24): the kitchen is lit by rare bright paths, so even the reference
# is noisy, and with one reference its noise was added to both errors (a floor neither denoiser can go under, which
# hides the ratio the brief asks for). With r1 and r2 independent, (x - r1)(x - r2) has the expectation (x - truth)^2.
# (c) on the software adapter the bar is three quarters, not half (operator 2026-11-24): the official oidnDenoise 2.5.1,
# given this very frame (96 x 64, 64 spp; crates/render3d/tests/fixtures/oidn-kitchen), reaches 0.68 of the filter's
# error, and our base model matches its answer to five decimal places (crates/render3d/tests/oidn_reference.rs). Half is
# the brief's figure for a 320 x 200 frame on a GPU and stays the bar there.
# needs: p22-doc-look, p22-r3d-types, p22-r3d-kernel-hooks, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-linear-out, p22-r3d-tza, p22-r3d-unet-cpu, p22-r3d-unet-gpu, p22-engine-denoiser-info, p22g-learned-kitchen-error
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
FILES = {'base': 'rt_hdr_alb_nrm.tza', 'small': 'rt_hdr_alb_nrm_small.tza'}

# (a)
rows = run_ok(['denoiser.info {}'], 'denoiser.info')
info = reply_with(rows, 'models', what='denoiser.info')
models = {m.get('name'): m for m in info['models'] if isinstance(m, dict)}
for name, fn in FILES.items():
    m = models.get(name)
    if not m:
        fail('denoiser.info lists the %s model: %r' % (name, list(models)))
    data = open(os.path.join(P22_ROOT, 'assets', 'denoise', fn), 'rb').read()
    if m.get('tensors') != 32 or m.get('encConv0') != [32, 9, 3, 3]:
        fail('%s: 32 tensors and enc_conv0.weight 32 x 9 x 3 x 3: %r, %r' % (fn, m.get('tensors'), m.get('encConv0')))
    if m.get('bytes') != len(data) or m.get('sha256') != hashlib.sha256(data).hexdigest():
        fail('%s: the loader read other bytes than the committed file (%r bytes, sha256 %r)' % (fn, m.get('bytes'), m.get('sha256')))
if info.get('available') is not True:
    fail('the learned denoiser is available on this adapter (compute shaders): %r, %r' % (info.get('available'), info.get('reason')))
print('ok: (a) both weight files load: 32 tensors, enc_conv0.weight 32 x 9 x 3 x 3')

# (b) a flat mid-grey furnace frame.
FC = {'kind': 'full', 'eye': [1.0, 1.0, 1.2], 'target': [3.0, 3.0, 1.0], 'fov': 100}
fq = dict(final='path', spp=256, seed=1, toneMap='none', furnace={'albedo': 0.01, 'emission': 0.1782})
p0, p1 = os.path.join(tmp, 'flat0.pfm'), os.path.join(tmp, 'flat1.pfm')
rows, rs = rendered(ROOM + [render_cmd(os.path.join(tmp, 'flat0.png'), FC, (160, 96), denoise=False, pfm=p0, **fq),
                            render_cmd(os.path.join(tmp, 'flat1.png'), FC, (160, 96), denoise='learned', pfm=p1, **fq)], 'the flat frames', timeout=3000)
if rs[0].get('denoiser') != 'none' or rs[1].get('denoiser') != 'learned':
    fail('the replies name the denoiser: %r, %r (fallback %r)' % (rs[0].get('denoiser'), rs[1].get('denoiser'), rs[1].get('denoiseFallback')))
_, _, f0 = read_pfm(p0)
_, _, f1 = read_pfm(p1)
worst = max(abs(x[k] - y[k]) / x[k] for x, y in zip(f0, f1) for k in range(3))
print('flat frame %.4f: the learned denoiser changes a pixel by at most %.3f %%' % (f0[0][1], 100 * worst))
within(f0[0][1], 0.18, 0.02, 'the furnace frame is mid grey (the check set-up)')
if worst > 0.01:
    fail('(b) on a flat mid-grey frame the learned denoiser changes no pixel by more than 1 %%: %.3f %%' % (100 * worst))

# (c) the showcase kitchen at 64 spp against the reference.
SIZE = (320, 200) if BIG else (96, 64)
k = dict(final='path', bounces=2, exposure={'ev100': 12})
# The reference is 4096 spp on a GPU and 1024 on the software adapter (operator 2026-11-24: there the 4096 spp frame
# alone took about 10 of the gate's 15 minutes). It is rendered as two halves with different seeds, see the header.
REF_SPP = 4096 if BIG else 1024
HALF = REF_SPP // 2
pr1, pr2, pn, pf, pl = [os.path.join(tmp, x + '.pfm') for x in ('ref1', 'ref2', 'noisy', 'filter', 'learned')]
cmds = showcase_cmds() + [
    render_cmd(os.path.join(tmp, 'ref1.png'), 'Kitchen', SIZE, spp=HALF, seed=99, denoise=False, pfm=pr1, **k),
    render_cmd(os.path.join(tmp, 'ref2.png'), 'Kitchen', SIZE, spp=HALF, seed=98, denoise=False, pfm=pr2, **k),
    render_cmd(os.path.join(tmp, 'noisy.png'), 'Kitchen', SIZE, spp=64, seed=3, denoise=False, pfm=pn, **k),
    render_cmd(os.path.join(tmp, 'filter.png'), 'Kitchen', SIZE, spp=64, seed=3, denoise='filter', pfm=pf, **k),
    render_cmd(os.path.join(tmp, 'learned.png'), 'Kitchen', SIZE, spp=64, seed=3, denoise='learned', pfm=pl, **k)]
rc, rows, so, se = run(cmds, expect_ok=False, timeout=14000)
rs = [r for r in render_replies(rows) if r.get('finalMode') == 'path'][-5:]
if rc != 0 or len(rs) != 5:
    fail('five path-traced kitchen renders (rc %d, %d replies): %s' % (rc, len(rs), (errors(se)[-3:] or se[-600:])))
if [r.get('denoiser') for r in rs] != ['none', 'none', 'none', 'filter', 'learned']:
    fail('the kitchen renders used none, none, none, filter and learned: %r (fallback %r)' % ([r.get('denoiser') for r in rs], rs[4].get('denoiseFallback')))
_, _, r1 = read_pfm(pr1)
_, _, r2 = read_pfm(pr2)
if len(r1) != len(r2) or r1 == r2:
    fail('the two half references are two different renders of one size')
ref = [tuple((a[c] + b[c]) / 2 for c in range(3)) for a, b in zip(r1, r2)]
ref_mean = sum(lum(p) for p in ref) / len(ref)
if ref_mean <= 0:
    fail('the kitchen reference is black')
eps = (0.05 * ref_mean) ** 2


def mrse(path):
    # the mean relative squared error, with the reference noise cancelled: E[(x - r1)(x - r2)] = (x - truth)^2
    _, _, px = read_pfm(path)
    if len(px) != len(ref):
        fail('image sizes differ')
    return sum((x[c] - a[c]) * (x[c] - b[c]) / (m[c] ** 2 + eps) for x, a, b, m in zip(px, r1, r2, ref) for c in range(3)) / (3 * len(ref))


e_n, e_f, e_l = mrse(pn), mrse(pf), mrse(pl)
print('kitchen, 64 spp against 2 x %d: MRSE noisy %.5f, filter %.5f, learned %.5f (learned / filter %.2f)' % (HALF, e_n, e_f, e_l, e_l / e_f if e_f > 0 else 0))
if not e_n > 0:
    fail('(c) the 64 spp frame differs from the reference (the check set-up): MRSE %.5f' % e_n)
if not 0 < e_f < e_n:
    fail('(c) the edge-aware filter lowers the 64 spp error (the check set-up): %.5f against %.5f' % (e_f, e_n))
_, _, xn = read_pfm(pn)
_, _, xl = read_pfm(pl)
if xn == xl:
    fail('(c) the learned denoiser returned its input unchanged')
m_n, m_l = sum(lum(p) for p in xn) / len(xn), sum(lum(p) for p in xl) / len(xl)
if abs(m_l - m_n) > 0.15 * m_n:
    fail('(c) the learned result keeps the frame brightness: mean %.2f against %.2f nits' % (m_l, m_n))
RATIO = 0.5 if BIG else 0.75
if not e_l <= RATIO * e_f:
    fail('(c) the learned result error is at most %.2f of the edge-aware filter error: %.5f vs %.5f' % (RATIO, e_l, e_f))

# (d) time.
TS = (1920, 1080) if BIG else (320, 180)
NC = {'kind': 'full', 'eye': [2.0, 0.3, 2.1], 'target': [2.0, 2.6, 0.0], 'fov': 90}
rows, rs = rendered(ROOM + [COLUMN, cmd('lightfixture', {'room': 'r0', 'rows': 2, 'cols': 2}),
                            render_cmd(os.path.join(tmp, 'time.png'), NC, TS, final='path', spp=1, bounces=0, seed=1, denoise='learned', sun={'on': False}, sky=False)],
                    'the timing render', timeout=6000)
t = num(rs[-1].get('denoiseSeconds'), 'denoiseSeconds')
if rs[-1].get('denoiser') != 'learned' or t <= 0:
    fail('the timing render ran the learned denoiser: %r in %r s' % (rs[-1].get('denoiser'), t))
print('learned denoiser on %dx%d: %.2f s (%s)' % (TS[0], TS[1], t, json.dumps(rs[-1].get('computeAdapter'))))
if BIG and t >= 5.0:
    fail('(d) a 1920 x 1080 frame denoises in under 5 s: %.2f s' % t)

# (e) no compute shaders: the still falls back and says so.
rc, rows, so, se = run_env(ROOM + [render_cmd(os.path.join(tmp, 'web.png'), NC, (96, 64), final='path', spp=4, denoise='learned')], {'CADCRAFT_RENDER_LIMITS': 'webgl2'})
rs = render_replies(rows)
if rc != 0 or not rs or rs[-1].get('finalMode') != 'raster' or rs[-1].get('denoiser') == 'learned' or not rs[-1].get('denoiseFallback'):
    fail('(e) without compute shaders a learned request falls back and says so (finalMode raster, denoiseFallback): %s' % json.dumps(rs[-1] if rs else se[-400:])[:500])
lock = open(os.path.join(P22_ROOT, 'Cargo.lock'), encoding='utf-8').read().lower()
if 'oidn' in lock or 'openimagedenoise' in lock:
    fail('Cargo.lock names an Open Image Denoise crate: no OIDN code may be linked')
print('PASS 10')
