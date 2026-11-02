# Materials in the 3D view: render3d samples the textures with face-aligned UVs. A brick wall seen face-on shows its
# courses (a spread of luminance an untextured wall does not have); siding on a wall that runs along y still runs
# horizontally (rows differ more than columns); the wall's interior face stays untextured.
# needs: p13-doc-finish, p13-texture-gen, p13-arch3d-finish-meshes, p13-render3d-textures
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())

tmp = tempfile.mkdtemp()
ALONG_X = cmd('wall', {"points": [[0, 0], [6, 0]], "thickness": 0.2, "height": 2.7, "exterior": True})   # w0, exterior face -y
ALONG_Y = cmd('wall', {"points": [[0, 6], [0, 0]], "thickness": 0.2, "height": 2.7, "exterior": True})   # w0, exterior face -x
FRONT = ([3, -6, 1.35], [3, 0, 1.35])
BACK = ([3, 6, 1.35], [3, 0, 1.35])
SIDE = ([-6, 3, 1.35], [0, 3, 1.35])

shots = {}
for name, cmds, (eye, target) in [
        ('plain', [ALONG_X], FRONT),
        ('brick', [ALONG_X, cmd('material', {"walls": ["w0"], "finish": "brick"})], FRONT),
        ('brick-inside', [ALONG_X, cmd('material', {"walls": ["w0"], "finish": "brick"})], BACK),
        ('siding-y', [ALONG_Y, cmd('material', {"walls": ["w0"], "finish": "siding"})], SIDE),
        ('plain-y', [ALONG_Y], SIDE)]:
    out = os.path.join(tmp, name + '.png')
    render_face(cmds, eye, target, out)
    crop = luma_grid(out)
    w, h, nch, rows = png(out)
    corner = 0.2126 * rows[1][4] + 0.7152 * rows[1][5] + 0.0722 * rows[1][6]
    mean = sum(map(sum, crop)) / (len(crop) * len(crop[0]))
    if abs(mean - corner) < 20:
        fail(f"{name}: the middle of the frame shows the wall, not the background (mean {mean:.1f}, corner {corner:.1f})")
    shots[name] = grid_stats(crop)
    print(f"{name}: sd {shots[name][0]:.2f}, vertical diff {shots[name][1]:.2f}, horizontal diff {shots[name][2]:.2f}")
    if name in ('brick', 'siding-y'):
        artifact(out, f'{name}.png')

plain_sd = shots['plain'][0]
brick_sd = shots['brick'][0]
if brick_sd < 6 or brick_sd < 3 * plain_sd + 2:
    fail(f"the brick wall shows its texture: luminance sd {brick_sd:.2f} (untextured wall {plain_sd:.2f})")
if shots['brick-inside'][0] > plain_sd + 3:
    fail(f"the brick wall's interior face is untextured: sd {shots['brick-inside'][0]:.2f} vs plain {plain_sd:.2f}")
sd, dv, dh = shots['siding-y']
if sd < 4 or dv < 1.0 or dv < 2 * dh:
    fail(f"siding on a wall along y shows horizontal boards: sd {sd:.2f}, vertical diff {dv:.2f}, horizontal diff {dh:.2f} "
         f"(an untextured wall: sd {shots['plain-y'][0]:.2f})")
print("ok: brick visible outside only; siding horizontal on a y-running wall")
print("PASS 03")
