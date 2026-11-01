# Hostile and limit inputs to the path tracer, illuminance, area lights and IES files are refused with a message
# that names the field (never a panic); a device without compute shaders falls back to the raster Finalize.
# needs: p11-arch-ies, p11-cmd-render-path, p11-cmd-lightfixture-area-ies, p11-cli-render-path, p11-r3d-final-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
base = room_cmds() + [POTS]
R = lambda **k: cmd('render', dict({"size": [16, 12], "camera": NIGHT_CAM, "sun": False, "final": "path", "spp": 1}, **k))
bad = [(R(spp=0), 'spp'), (R(spp=65537), 'spp'), (R(bounces=33), 'bounces'), (R(seed=-1), 'seed'), (R(seed=1.5), 'seed'),
       (R(final="foo"), 'final'), (R(denoise="yes"), 'denoise'), (R(rt=1), 'rt'), (R(snapshots=[8, 4]), 'snapshots'),
       (R(snapshots=[2]), 'snapshots'), (R(spp=16, snapshots=list(range(1, 10))), 'snapshots'), (R(snapshots=[0]), 'snapshots'),
       (R(furnace={"albedo": 1.0, "emission": 1}), 'albedo'), (R(furnace={"albedo": 0.5, "emission": -1}), 'emission'),
       (R(furnace={"albedo": 0.5}), 'emission'),
       (cmd('illuminance', {"point": [1, 1, 0], "method": "magic"}), 'method'),
       (cmd('illuminance', {"point": [1, 1, 0], "method": "path", "spp": 0}), 'spp'),
       (cmd('illuminance', {"point": [1, 1, 0], "method": "path", "bounces": 99}), 'bounces'),
       (cmd('lightfixture', {"type": "LED panel 600", "at": [2, 2], "width": 5}), 'width'),
       (cmd('lightfixture.edit', {"id": "lf0", "kind": "area", "width": 0.6, "length": 0.001}), 'length')]
for c, word in bad:
    rc, rr, so, se = run(base + [c], expect_ok=False)
    if not was_refused(rc, se) or word not in se:
        fail(f"refused, naming `{word}`: {c} -> rc {rc}, stderr {se[-300:]!r}")
print(f"ok: {len(bad)} bad render, illuminance and area-light inputs refused")

IES = [("no tilt", IES_TEST.replace("TILT=NONE\n", ""), "TILT"),
       ("tilt include", IES_TEST.replace("TILT=NONE", "TILT=INCLUDE"), "TILT"),
       ("type B", IES_TEST.replace("1 1000 1 5 1 1 2", "1 1000 1 5 1 2 2"), "type"),
       ("negative candela", IES_TEST.replace("1000 900 600 200 0", "1000 900 -600 200 0"), "candela"),
       ("descending angles", IES_TEST.replace("0 22.5 45 67.5 90", "0 45 22.5 67.5 90"), "ascend"),
       ("too few numbers", IES_TEST.replace("1000 900 600 200 0", "1000 900"), "candela"),
       ("too many angles", IES_TEST.replace("1 1000 1 5 1 1 2", "1 1000 1 5000 1 1 2"), "angles"),
       ("not a number", IES_TEST.replace("1000 900", "1000 nine"), "number"),
       ("empty", "", "TILT")]
for what, text, word in IES:
    p = ies_file(tmp, text, what.replace(' ', '-') + '.ies')
    rc, rr, so, se = run(base + [cmd('lightfixture.edit', {"id": "lf0", "ies": p})], expect_ok=False)
    if not was_refused(rc, se) or word.lower() not in se.lower() or 'IES' not in se:
        fail(f"an IES file with {what} is refused naming '{word}' (and 'IES'): rc {rc}, stderr {se[-300:]!r}")
big = os.path.join(tmp, 'huge.ies')
with open(big, 'w') as f:
    f.write(IES_TEST + ' 0' * 600000)
rc, rr, so, se = run(base + [cmd('lightfixture.edit', {"id": "lf0", "ies": big})], expect_ok=False)
if not was_refused(rc, se):
    fail("an IES file over 1 MB is refused")
print(f"ok: {len(IES) + 1} bad IES files refused with the line or reason")

os.environ['CADCRAFT_RENDER_LIMITS'] = 'webgl2'
rc, rr, so, se = run(base + [cmd('render', {"out": os.path.join(tmp, 'gl.png'), "size": [32, 24], "camera": NIGHT_CAM, "sun": False,
                                            "final": "path", "spp": 4})])
del os.environ['CADCRAFT_RENDER_LIMITS']
r = [x for x in rr if isinstance(x, dict) and 'finalMode' in x]
if not r or r[-1].get('finalMode') != 'raster' or r[-1].get('fallback') != 'no compute shaders' or not os.path.exists(os.path.join(tmp, 'gl.png')):
    fail(f"without compute shaders (WebGL2) a path-traced Finalize falls back to the raster still: {r[-1] if r else rr}")
print("ok: WebGL2 limits fall back to the raster Finalize")
print("PASS 12")
