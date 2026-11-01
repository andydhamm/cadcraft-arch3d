# White furnace: inside the closed room with every surface diffuse at albedo 0.5 emitting radiance 1, every pixel
# converges to 1 / (1 - 0.5) = 2.0 (± 2 %). Tests the integrator's energy balance: bounces, Russian roulette, the BSDF.
# Samples per pixel from the variance, not from a run (fix cycle 1): with the default 4 bounces a path's throughput
# reaches 0.5^5 = 1/32 at its fifth hit and Russian roulette keeps it with p = 1/32, so a sample's tail is a Bernoulli(p)
# times a geometric run of hits (mean 2, E[G^2] = 6): per-sample variance 6p - 4p^2 = 0.184, sd 0.43 = 21 % of 2.0.
# At 1024 spp a pixel's sd is 0.67 %, and the extreme of 768 pixels sits near 3.2 sd = 2.1 %: the old 32 x 24 x 1024
# render failed the ± 2 % bar by chance (min 1.965, max 2.048, mean 2.001). At 4096 spp the sd is 0.34 %; over 12 x 9
# = 108 pixels the extreme is near 2.8 sd = 0.9 %, so ± 2 % is about 6 sd, at half the samples of the old render.
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'furnace.png')
CAM = {"kind": "full", "eye": [1.0, 1.0, 1.2], "target": [3.0, 3.0, 1.0], "fov": 100}
rc, rr, so, se = run(room_cmds() + [cmd('render', {"out": out, "final": "path", "size": [12, 9], "camera": CAM, "spp": 4096,
                                                   "denoise": False, "furnace": {"albedo": 0.5, "emission": 1.0}})])
r = path_renders(rr)
if len(r) != 1:
    fail(f"one path-traced render reply, got {len(r)}: {json.dumps(rr)[-600:]}")
lo, hi, mean = radiance_of(r[0], 'furnace')
print(f"furnace radiance over 108 pixels: min {lo:.4f}, max {hi:.4f}, mean {mean:.4f} (want 2.0 ± 2 %)")
if not (1.96 <= lo and hi <= 2.04):
    fail(f"every pixel converges to 2.0 ± 2 %: min {lo}, max {hi}")
near(mean, 2.0, 0.01, 'mean furnace radiance')
if r[0].get('denoised') is not False or num(r[0].get('spp'), 'spp') != 4096:
    fail(f"the reply echoes spp 4096 and denoised false: {r[0]}")
w, h, nch, rows = png(out)
if (w, h) != (12, 9):
    fail(f"furnace.png is 12 x 9, got {w} x {h}")
print("PASS 02")
