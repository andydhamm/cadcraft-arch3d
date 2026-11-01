# The denoiser: a 4-sample still denoised is far closer to a 1024-sample reference than the same still raw, and it
# keeps the edge where the wall meets the floor.
# needs: p11-r3d-denoise, p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
WALL, FLOOR = [2.0, 4.0, 0.25], [2.0, 3.75, 0.0]          # the north wall just above the floor, the floor just in front
P = lambda name, **k: cmd('render', dict({"out": os.path.join(tmp, name), "final": "path", "size": [96, 72], "camera": NIGHT_CAM,
                                         "sun": False, "samples": [WALL, FLOOR]}, **k))
rc, rr, so, se = run(room_cmds() + [POTS, P('noisy.png', spp=4, denoise=False), P('clean.png', spp=4, denoise=True),
                                    P('ref.png', spp=1024, denoise=False)])
r = path_renders(rr)
if len(r) != 3 or r[0].get('denoised') is not False or r[1].get('denoised') is not True:
    fail(f"three path-traced replies, denoised false then true: {[x.get('denoised') for x in r]}")
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
artifact(os.path.join(tmp, 'noisy.png'), 'denoise-before.png'); artifact(os.path.join(tmp, 'clean.png'), 'denoise-after.png')
print("PASS 09")
