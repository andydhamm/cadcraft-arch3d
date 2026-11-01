# White furnace: inside the closed room with every surface diffuse at albedo 0.5 emitting radiance 1, every pixel
# converges to 1 / (1 - 0.5) = 2.0 (± 2 %). Tests the integrator's energy balance: bounces, Russian roulette, the BSDF.
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'furnace.png')
CAM = {"kind": "full", "eye": [1.0, 1.0, 1.2], "target": [3.0, 3.0, 1.0], "fov": 100}
rc, rr, so, se = run(room_cmds() + [cmd('render', {"out": out, "final": "path", "size": [32, 24], "camera": CAM, "spp": 1024,
                                                   "denoise": False, "furnace": {"albedo": 0.5, "emission": 1.0}})])
r = path_renders(rr)
if len(r) != 1:
    fail(f"one path-traced render reply, got {len(r)}: {json.dumps(rr)[-600:]}")
lo, hi, mean = radiance_of(r[0], 'furnace')
print(f"furnace radiance over 768 pixels: min {lo:.4f}, max {hi:.4f}, mean {mean:.4f} (want 2.0 ± 2 %)")
if not (1.96 <= lo and hi <= 2.04):
    fail(f"every pixel converges to 2.0 ± 2 %: min {lo}, max {hi}")
near(mean, 2.0, 0.01, 'mean furnace radiance')
if r[0].get('denoised') is not False or num(r[0].get('spp'), 'spp') != 1024:
    fail(f"the reply echoes spp 1024 and denoised false: {r[0]}")
w, h, nch, rows = png(out)
if (w, h) != (32, 24):
    fail(f"furnace.png is 32 x 24, got {w} x {h}")
print("PASS 02")
