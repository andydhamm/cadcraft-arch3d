# The denoiser: a 4-sample still denoised is far closer to a 1024-sample reference than the same still raw, and it
# keeps the edge where the wall meets the floor; at 64 samples it never moves a still away from the reference; and an
# explicit sample count is the one rendered.
# Fix cycle 1: the edge-stopping colour weight was relative to the centre pixel's own luminance (sigma 0.5), so at 4 spp
# a dark pixel refused every brighter neighbour and only ~10 % of the error went (raw 21.70, denoised 19.51), while at
# 64 spp the filter added error (raw 4.96, denoised 7.73: it blurred the light pools). p11f-denoise-variance weighs each
# tap by its luminance distance against the two pixels' variance of the mean (from the samples), as SVGF does.
# A converging still must not get worse: the 64-spp bar is denoised RMSE <= raw RMSE.
# needs: p11-r3d-denoise, p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path, p11f-denoise-variance, p11f-explicit-spp-kept
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
WALL, FLOOR = [2.0, 4.0, 0.25], [2.0, 3.75, 0.0]          # the north wall just above the floor, the floor just in front
P = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [96, 72], "camera": NIGHT_CAM,
                                         "sun": False, "samples": [WALL, FLOOR]}, **k))
rc, rr, so, se = run(room_cmds() + [POTS, P('noisy.png', spp=4, denoise=False), P('clean.png', spp=4, denoise=True),
                                    P('ref.png', spp=1024, denoise=False), P('noisy64.png', spp=64, denoise=False),
                                    P('clean64.png', spp=64, denoise=True),
                                    cmd('render', {"out": os.path.join(tmp, 'spp256.png'), "final": "path", "size": [16, 12],
                                                   "camera": NIGHT_CAM, "sun": False, "spp": 256})])
r = path_renders(rr)
if len(r) != 6 or [x.get('denoised') for x in r[:5]] != [False, True, False, False, True]:
    fail(f"six path-traced replies, denoised false, true, false, false, true: {[x.get('denoised') for x in r]}")
_, _, noisy = pixels(os.path.join(tmp, 'noisy.png'))
_, _, clean = pixels(os.path.join(tmp, 'clean.png'))
_, _, ref = pixels(os.path.join(tmp, 'ref.png'))
e_noisy, e_clean = rmse(noisy, ref), rmse(clean, ref)
print(f"RMSE against the 1024-spp reference: 4 spp raw {e_noisy:.2f}, denoised {e_clean:.2f}")
if e_clean > 0.6 * e_noisy:
    fail(f"denoising removes at least 40 % of the error: raw {e_noisy:.2f}, denoised {e_clean:.2f}")
edge_ref = abs(rgb_sum(r[2]['samples'][0], 'ref wall') - rgb_sum(r[2]['samples'][1], 'ref floor'))
edge_clean = abs(rgb_sum(r[1]['samples'][0], 'denoised wall') - rgb_sum(r[1]['samples'][1], 'denoised floor'))
print(f"wall/floor contrast (r+g+b): reference {edge_ref:.0f}, denoised {edge_clean:.0f}")
if edge_ref >= 20 and edge_clean < 0.7 * edge_ref:
    fail(f"the denoiser keeps the wall/floor edge (>= 70 % of the reference contrast): {edge_clean} vs {edge_ref}")
_, _, noisy64 = pixels(os.path.join(tmp, 'noisy64.png'))
_, _, clean64 = pixels(os.path.join(tmp, 'clean64.png'))
e64_noisy, e64_clean = rmse(noisy64, ref), rmse(clean64, ref)
print(f"RMSE against the 1024-spp reference: 64 spp raw {e64_noisy:.2f}, denoised {e64_clean:.2f}")
if e64_clean > e64_noisy:
    fail(f"at 64 spp denoising does not move the still away from the reference: raw {e64_noisy:.2f}, denoised {e64_clean:.2f}")
if num(r[5].get('spp'), 'spp') != 256:
    fail(f"an explicit spp 256 is rendered as asked, on a software adapter too: the reply says spp {r[5].get('spp')}")
artifact(os.path.join(tmp, 'noisy.png'), 'denoise-before.png'); artifact(os.path.join(tmp, 'clean.png'), 'denoise-after.png')
print("PASS 09")
