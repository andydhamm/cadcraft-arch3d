# Progressive refinement: the same still kept at 4, 16 and 64 samples per pixel gets closer to the 64-sample image
# as samples accumulate, and each snapshot is written as <out>-spp<n>.png.
# needs: p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp(); out = os.path.join(tmp, 'prog.png')
rc, rr, so, se = run(room_cmds() + [POTS, cmd('render', {"out": out, "final": "path", "size": [96, 72], "camera": NIGHT_CAM, "sun": False,
                                                         "spp": 64, "denoise": False, "snapshots": [4, 16, 64]})])
r = path_renders(rr)
if len(r) != 1:
    fail(f"one path-traced render reply, got {len(r)}: {json.dumps(rr)[-600:]}")
snaps = r[0].get('snapshots')
if not isinstance(snaps, list) or [num(s.get('spp'), 'spp') for s in snaps] != [4, 16, 64]:
    fail(f"three snapshots at 4, 16 and 64 spp: {snaps}")
files = [s.get('out') for s in snaps]
for n, f in zip([4, 16, 64], files):
    if not f or not f.endswith(f'prog-spp{n}.png') or not os.path.exists(f):
        fail(f"snapshot {n} is written as prog-spp{n}.png: {f}")
_, _, final = pixels(out)
_, _, s4 = pixels(files[0]); _, _, s16 = pixels(files[1]); _, _, s64 = pixels(files[2])
d4, d16, d64 = mean_abs(s4, final), mean_abs(s16, final), mean_abs(s64, final)
print(f"mean |difference| from the 64-spp still: 4 spp {d4:.2f}, 16 spp {d16:.2f}, 64 spp {d64:.2f}")
if not (d4 > d16 > d64) or d64 > 0.5:
    fail(f"the image refines as samples accumulate (4 > 16 > 64, and the 64-spp snapshot is the still): {d4}, {d16}, {d64}")
artifact(files[0], 'prog-4spp.png'); artifact(out, 'prog-64spp.png')
print("PASS 04")
