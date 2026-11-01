# Phase-11 check helpers (the path-traced Finalize Render). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())
# It loads the phase-10 helpers (run, cmd, reply_with, num, near, png, room_cmds, COLUMN, POTS, lights_of, renders,
# rgb_sum, artifact, was_refused, fail) and adds the phase-11 ones.
#
# The commands the checks drive (lengths in metres, angles in degrees):
#   render {…phase 10…, final?: false | true | "raster" | "path", spp?: 1..=65536 (256), bounces?: 0..=32 (4),
#           seed?: 0..=4294967295 (0), denoise?: bool (true), rt?: bool (false), snapshots?: [n, …] (<= 8, ascending, <= spp),
#           furnace?: {albedo: 0 < a < 1, emission: 0 < e <= 1000}}
#     -> phase-10 keys plus "finalMode": "raster" | "path" | null, "pathTraces" (path-traced stills this session), and for a
#        path-traced still "spp", "bounces", "seed", "denoised", "rt" (ray queries used), "pipelineCache": "hit" | "miss" |
#        "unsupported", "radiance": {min, max, mean} (linear nits before exposure), "snapshots": [{spp, out}] (files
#        <out without .png>-spp<n>.png); "fallback": "no compute shaders" when a path trace was asked of a device without them.
#     `final: true` is the default Finalize: "path" since p11-final-default-path ("raster" before).
#   illuminance {point, normal?, method?: "direct" | "path", spp?: (1024), bounces?: (4), seed?: (0), sun?: like render (night)}
#     -> {"lux", "method", …}; path adds "rgb", "spp", "bounces", "seed".
#   lightfixture {type: "LED panel 600", at} (an area light: kind "area", width, length); lightfixture.edit {id, ies: path, width?, length?}
#   arch.inspect {"lights": true} lights add "width", "length" (area) and "ies": {name, lumens, peak} | null.
#   cadcraft-cli render FILE OUT.png --final [raster|path] [--spp N] [--bounces N] [--seed N] [--no-denoise] [--rt]
#     stderr: FINAL renders=1 size=WxH mode=path spp=N cache=hit|miss|unsupported
#   env: CADCRAFT_PIPELINE_CACHE_DIR (the pipeline cache directory), CADCRAFT_RENDER_LIMITS=webgl2 (act as WebGL2: no compute).
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-10', 'p10lib.inc')).read())

NIGHT_CAM = {"kind": "full", "eye": [2.0, 0.3, 2.1], "target": [2.0, 2.6, 0.0], "fov": 90}


def path_renders(rows):
    """The render replies of path-traced stills."""
    return [r for r in rows if isinstance(r, dict) and r.get('finalMode') == 'path']


def radiance_of(r, what):
    rad = r.get('radiance')
    if not isinstance(rad, dict):
        fail(f"{what}: no radiance stats in {json.dumps(r)[:400]}")
    return num(rad.get('min'), 'min'), num(rad.get('max'), 'max'), num(rad.get('mean'), 'mean')


def lux_of(rows, nth=-1):
    hits = [r for r in rows if isinstance(r, dict) and 'lux' in r and 'method' in r]
    if not hits or not (-len(hits) <= nth < len(hits)):
        fail(f"no illuminance reply with a method; replies: {json.dumps(rows)[:600]}")
    return hits[nth]


def pixels(path):
    """[(r, g, b)] of a PNG, row-major."""
    w, h, nch, rows = png(path)
    out = []
    for row in rows:
        for x in range(w):
            out.append(tuple(row[x * nch:x * nch + 3]))
    return w, h, out


def rmse(a, b):
    if len(a) != len(b) or not a:
        fail(f"images differ in size: {len(a)} vs {len(b)} pixels")
    s = sum((p[k] - q[k]) ** 2 for p, q in zip(a, b) for k in range(3))
    return math.sqrt(s / (3 * len(a)))


def mean_abs(a, b):
    if len(a) != len(b) or not a:
        fail(f"images differ in size: {len(a)} vs {len(b)} pixels")
    return sum(abs(p[k] - q[k]) for p, q in zip(a, b) for k in range(3)) / (3 * len(a))


def digest(path):
    import hashlib
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()


IES_TEST = """IESNA:LM-63-2002
[TEST] cadcraft phase-11 test downlight
[LUMINAIRE] Test downlight 1000 lm
TILT=NONE
1 1000 1 5 1 1 2 0.15 0.15 0.1
1.0 1.0 12
0 22.5 45 67.5 90
0
1000 900 600 200 0
"""


def ies_file(tmp, text=IES_TEST, name='downlight.ies'):
    p = os.path.join(tmp, name)
    with open(p, 'w') as f:
        f.write(text)
    return p
