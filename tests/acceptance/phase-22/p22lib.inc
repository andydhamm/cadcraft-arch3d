# Phase-22 check helpers (photoreal stills). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())
# It loads p19lib.inc and through it phases 17..8 (run, cmd, rendered, render_cmd, img, pixels, digest, lum, within, near,
# num, fail, errors, run_ok, level_cam, down_cam, write_rgbe, srgb8_to_linear, MODES, PATH0, ROOM, COLUMN, HOUSE, HOUSE1,
# showcase_cmds, export_glb, luma_sd ...).
#
# What the checks drive (metres, degrees; `fov` is the vertical field of view as before):
#   look keys (look, camera save / camera.update, and for one call on render and pick3d):
#     technique: photoreal (new) | projection: rectilinear | panini | compression: 0..1 (default 0.7) |
#     shift: -1..1 (fraction of the frame height; +0.25 moves the horizon 25 % of the height DOWN the image) |
#     aperture: null | f-number 0.5..64 | focus: null | metres (0, 10000] | {pick: [x, y]} (a pixel of `size`; resolved
#     to the distance along the view axis and echoed as a number) |
#     film: null | photoreal | {bloom 0..1, bloomThreshold > 0, vignette 0..1, whiteBalance null | 1500..15000,
#     contrast 0.5..2, saturation 0..2, lift -0.5..0.5} | sun: {..., disc: 0..10 (angular diameter)}
#     echo: look.lens {projection, compression, shift, aperture, focus}, look.film {...}, look.sun.disc
#     Panini keeps the horizontal field of view; the thin lens is a 36 mm wide sensor: f = 0.018 / tan(hfov / 2),
#     aperture diameter f / N, focus measured along the view axis. Vignette v multiplies by 1 - v (d / d_corner)^2.
#     Grade: (x - 0.18) contrast + 0.18, saturation about luminance, + lift. whiteBalance K divides by the fixture colour
#     of K (arch::light::kelvin_to_rgb) normalised to green.
#   render-only keys (path tracer): edges: false | true | {radius: 0..0.05}, materials: basic | full, variation: bool,
#     clamp: null | c > 0 (display units at the still's starting exposure, on everything a path adds after its first hit),
#     adaptive: bool, noiseTarget: 0.001..1, timeBudget: seconds 1..86400 (spp is then the cap per pixel),
#     denoise: false | true | filter | learned, denoiseModel: base | small, roughnessGuide: bool, rung: 0..7,
#     pfm: path (a PFM 'PF', little endian, rows bottom to top, linear), pfmStage: radiance (nits, after the denoiser) |
#     film (display linear: after exposure and film, before the tone curve),
#     calibrate: {objects: [{shape: sphere | box, at: centre, size: s | [sx, sy, sz] (sphere: diameter), segments?,
#       material: {color: [r, g, b] linear, metallic, roughness, emission (nits, white), clearcoat, clearcoatRoughness,
#       anisotropy, anisotropyRotation, sheen, sheenRoughness, transmission, thin, ior, thickness, edgeRadius,
#       variation: {scale, amount}}}] (1..16), lights: [{at, candela, kelvin?}] (0..8)}: replaces the drawing's scene and
#       fixtures in every render mode; the look still applies.
#   render reply gains: photo {rung, edges, materials, variation, bounces, sunDisc, clamp, adaptive, noiseTarget,
#     timeBudget, roughnessGuide, denoise}, stop: samples | noise target | time budget, noise, sppMean, sppMin, sppMax,
#     samplesTotal, seconds, clampLoss, denoiser: none | filter | learned, denoiseFallback?, denoiseSeconds, pfm,
#     computeAdapter {name, type}. noise = the 95th percentile over hit pixels of
#     sqrt(variance of the mean) / max(L, 0.05 x the frame's mean L).
#   denoiser.info {} -> {available, reason?, models: [{name: base | small, file, bytes, sha256, tensors, encConv0: [o, i, h, w]}]}
#   render.ladder {camera: name, out: dir, size?, spp?, seed?, noiseTarget?, timeBudget?}
#     -> {rungs: [{rung, name, out, photo, look, stop, denoiser, seed, ...}], sheet, size}; files out/rung-<n>.png and
#     out/ladder.png: 8 w wide, h + 24 high, a 24 px label band (background 24, 24, 24) on top, rung n at x = n w.
#   Rungs (cumulative): 0 today (lens neutral) | 1 lens (the camera's) | 2 edges | 3 materials full + variation |
#     4 bounces 12, sun disc 0.53, clamp 32 | 5 film photoreal (bloom 0.1, vignette 0.15, contrast 1.05) |
#     6 adaptive, noise target 0.02, time budget 600 s, roughness guide | 7 learned denoiser. technique photoreal = rung 7.
import hashlib, time
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'phase-19', 'p19lib.inc')).read())

BIG = os.environ.get('CADCRAFT_RENDER_ADAPTER', 'fallback') != 'fallback'      # a real GPU was asked for
RUNGS = ['today', 'lens', 'edges', 'materials', 'light', 'film', 'adaptive', 'learned denoiser']
PT = {'final': 'path', 'seed': 1, 'denoise': False, 'toneMap': 'none', 'sky': False}
P22_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))


def pt(**k):
    d = dict(PT)
    d.update(k)
    return d


def read_pfm(path):
    # (w, h, [(r, g, b)]) with row 0 at the top
    if not path or not os.path.exists(path):
        fail('no PFM at ' + str(path))
    b = open(path, 'rb').read()
    parts = b.split(bytes([10]), 3)
    if len(parts) < 4 or parts[0].strip() != b'PF':
        fail(str(path) + ' is not a colour PFM (PF header)')
    w, h = [int(v) for v in parts[1].split()]
    if float(parts[2]) >= 0:
        fail(str(path) + ': the PFM must be little endian (negative scale)')
    data = parts[3]
    if len(data) != w * h * 12:
        fail(str(path) + ': PFM data is %d bytes, not %d' % (len(data), w * h * 12))
    vals = struct.unpack('<%df' % (w * h * 3), data)
    px = []
    for y in range(h - 1, -1, -1):
        o = y * w * 3
        for x in range(w):
            px.append((vals[o + 3 * x], vals[o + 3 * x + 1], vals[o + 3 * x + 2]))
    for p in px:
        if not (math.isfinite(p[0]) and math.isfinite(p[1]) and math.isfinite(p[2])):
            fail(str(path) + ': a PFM pixel is not finite')
    return w, h, px


def pfm_lumas(path):
    w, h, px = read_pfm(path)
    return w, h, [lum(p) for p in px]


def png_lumas(path):
    w, h, p = img(path)
    return w, h, [lum([srgb8_to_linear(c[k]) for k in range(3)]) for c in p]


def vfov(hfov, size):
    return math.degrees(2 * math.atan(math.tan(math.radians(hfov) / 2) * size[1] / size[0]))


def hcam(eye, yaw, hfov, size, pitch=0.0):
    # a level full camera with a HORIZONTAL field of view of hfov at this size
    return level_cam(eye, yaw, pitch, vfov(hfov, size))


def obj(shape, at, size, **material):
    # `segments` is the object's key (see the header: {shape, at, size, segments?, material}), not a material's. It went
    # into the material until 2026-11-23, and checks 03 and 07 were refused: unknown key `segments` in `material`.
    o = {'shape': shape, 'at': list(at), 'size': size}
    if 'segments' in material:
        o['segments'] = material.pop('segments')
    o['material'] = material
    return o


def calib(objects, lights=None):
    c = {'objects': objects}
    if lights:
        c['lights'] = lights
    return c


def mask(vals, bg, frac=0.25):
    peak = max(abs(v - bg) for v in vals)
    if peak <= 0:
        fail('the image is uniform: nothing was drawn')
    return [1.0 if abs(v - bg) > frac * peak else 0.0 for v in vals]


def moments(w, h, vals, x0=0, x1=None, y0=0, y1=None):
    # (sum, cx, cy, var x, var y, cov) of the weights, pixel centres at +0.5
    x1 = w if x1 is None else x1
    y1 = h if y1 is None else y1
    s = sx = sy = 0.0
    for y in range(y0, y1):
        for x in range(x0, x1):
            v = vals[y * w + x]
            if v > 0:
                s += v
                sx += v * (x + 0.5)
                sy += v * (y + 0.5)
    if s <= 0:
        fail('nothing is drawn in the region x %d..%d, y %d..%d' % (x0, x1, y0, y1))
    cx, cy = sx / s, sy / s
    xx = yy = xy = 0.0
    for y in range(y0, y1):
        for x in range(x0, x1):
            v = vals[y * w + x]
            if v > 0:
                dx, dy = x + 0.5 - cx, y + 0.5 - cy
                xx += v * dx * dx
                yy += v * dy * dy
                xy += v * dx * dy
    return s, cx, cy, xx / s, yy / s, xy / s


def eig_aspect(xx, yy, xy):
    t, d = xx + yy, xx * yy - xy * xy
    r = math.sqrt(max(0.0, t * t / 4 - d))
    l1, l2 = t / 2 + r, t / 2 - r
    if l2 <= 1e-9:
        return float('inf')
    return math.sqrt(l1 / l2)


def percentile(vals, q):
    s = sorted(vals)
    if not s:
        fail('no values for a percentile')
    return s[min(len(s) - 1, int(q * len(s)))]


def uniform_env(tmp):
    # a visible 360 environment of radiance 1.0 that lights the scene
    return cmd('backdrop', {'kind': 'hdri', 'file': write_rgbe(os.path.join(tmp, 'uniform.hdr'), 32, 16, lambda x, y: (1.0, 1.0, 1.0))})


def photo_of(r, what):
    p = r.get('photo') if isinstance(r, dict) else None
    if not isinstance(p, dict):
        fail(what + ': the render reply has no photo object: ' + json.dumps(r)[:500])
    return p


def lens_of(r, what):
    l = look_of(r, what).get('lens')
    if not isinstance(l, dict):
        fail(what + ': look.lens is missing: ' + json.dumps(r.get('look'))[:400])
    return l


def is_path(r, what):
    if r.get('finalMode') != 'path':
        fail(what + ': the still was not path-traced (finalMode %r, fallback %r)' % (r.get('finalMode'), r.get('fallback')))


def run_env(cmds, extra, **kw):
    old = {k: os.environ.get(k) for k in extra}
    os.environ.update(extra)
    try:
        return run(cmds, expect_ok=False, **kw)
    finally:
        for k, v in old.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def script_cmds(name):
    path = os.path.join(P22_ROOT, 'docs', 'arch3d', 'showcase', name)
    if not os.path.exists(path):
        fail('docs/arch3d/showcase/' + name + ' is missing')
    steps = json.load(open(path, encoding='utf-8'))
    if not isinstance(steps, list) or not steps:
        fail(name + ' is not a non-empty JSON array of {cmd, params}')
    return [cmd(st['cmd'], st.get('params', {})) for st in steps]
