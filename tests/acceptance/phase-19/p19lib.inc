# Phase-19 check helpers (light, sky and cameras). A fixture, not a check (the gate runs only *.sh and *.py):
#   exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())
# It loads the phase-17 helpers when they exist, else phase 16's; they load phases 16..8 (run, cmd, cli_args, parse_rows,
# reply_with, num, near, fail, errors, inspected, png, pixels, digest, artifact, glb, node_names, glb_images, room_cmds,
# ROOM (the 4 x 4 room r0, inner faces x/y 0..4, walls 0.2 thick, 2.4 high), COLUMN (0.2 x 0.2 x 1.0 at (1.5, 1.5)),
# HOUSE1 (p14lib: the 8 x 5 two-room house: ring w0 (0,0)-(4,0), w1 (4,0)-(8,0), w2 east x = 8, w3, w4, w5 west x = 0;
# partition w6 at x = 4; room A x 0.1..3.95, room B x 4.05..7.9, y 0.1..4.9; walls 2.4 high), HOUSE (p16lib: the 10 x 8
# phase-1 house), GABLE, house_rooms(), path_renders, lux_of, lights_of, rgb_sum).
#
# What the checks drive (lengths in metres, angles in degrees, compass azimuths: 0 = true north, 90 = east):
#   look {technique?: standard|path|clay|technical, exposure?: "auto" | {"ev100": -10..24}, toneMap?: aces|hable|none,
#         sun?: false | {on?, intensity?: 0..200000 lux, shadows?, angle?: name|null},
#         sky?: false | true | {on?, turbidity?: 1.7..10, stars?, moon?: {lux?: 0..10, azimuth?, elevation?: -90..90}},
#         lightSet?: name|null, cutaway?: bool, clip?: null | {min: [x,y,z], max: [x,y,z]}, nearClip?: null | (0, 1000]}
#        -> {"look": {"technique", "exposure": "auto"|{"ev100"}, "toneMap", "sun": {"on","intensity","shadows","angle"},
#            "sky": {"on","turbidity","stars","moon": {"lux","azimuth","elevation"}}, "backdrop", "lightSet", "cutaway",
#            "clip", "nearClip"}}; `look {}` reports. Defaults: standard, auto, aces, sun on 100000 lx with shadows, sky on
#           turbidity 3 without stars, moon 0.25 lx at azimuth 150 elevation 35, no clip, no near clip, cutaway off.
#   render {..., the look keys above for this render only, sun?: false (the legacy night: sun and sky off, exposure 0.01) |
#           "default" | {date, time} | {azimuth, elevation} | {angle} (+ on?, intensity?, shadows?), albedo?: 0<a<1 (a
#           calibration: every surface becomes Lambertian of that albedo)}
#        -> the phase-10..15 reply plus "look" (the effective look), "sunDir": [x,y,z]|null, "exposure" (display units per
#           nit), "ev100", "exposureMode": "auto"|"manual"|"fixed", "toneMap", "filter": {"hidden","clip","nearClip"},
#           path stills also "environment": null|{"width","height","visible"}. A pixel's linear value is
#           luminance x 1/(1.2 2^EV100), then the tone map.
#   illuminance {..., sun?: as render, sky?: as look} -> direct replies gain "sun" (lux) and "moon" (lux), added to "lux".
#   sunangle {name, date, time} | {name, azimuth, elevation} (+ replace?) | {delete: name} | {list: true}
#        -> {"sunAngles": [{"name","date","time","azimuth","elevation","dir","sunElevation","sunAzimuth"}]}
#   backdrop {kind: "sky"} | {kind: "hdri", file: path | item: id, intensity?: 0..100, rotation?, horizon?: -45..45} |
#            {kind: "image", item, projection: "equirect", horizon?}
#   lightfixture {type: "LED strip", path: [[x,y],...], closed?, height?, lumensPerMetre?, kelvin?, facing?: down|out} |
#                {type: "LED strip", under: [wall cabinet ids]} | {type: "LED strip", room, inset} |
#                {type: "LED strip", tray, inset?}; arch.inspect lights rows of a strip: "kind": "strip", "path",
#                "closed", "height", "lumensPerMetre", "facing", "length", "lumens", "tray"
#   camera {save, kind?: orbit|full|overview, level?, lowerLevels?, + pose + look keys}; camera.reset {name?};
#          camera.orbit {name?, azimuth?, elevation?, distance?}; camera.update {name?, + pose and look keys}
#   pick3d {camera: {...} | name, size, pixel, cutaway?, clip?, nearClip?} -> {"hit", "filter"}
#   build3d -> "horizon": 0|1; probes outside the site boundary answer part "horizon".
import math, os, json, struct, tempfile
_P19_HERE = os.path.dirname(os.path.abspath(__file__))
for _n in ('17', '16'):
    _lib = os.path.join(_P19_HERE, '..', f'phase-{_n}', f'p{_n}lib.inc')
    if os.path.exists(_lib):
        exec(open(_lib).read())
        break
else:
    print("FAIL: no phase-17 or phase-16 check library next to phase-19")
    raise SystemExit(1)

NOON = {"date": "2026-06-21", "time": "solar-noon"}          # elevation 62.39 at latitude 51.05
SITE = cmd('site', {"latitude": 51.05, "longitude": -114.07, "trueNorth": 0})
CLEAR = (39, 44, 52)
NOSKY = {"sky": False}
PATH0 = {"final": "path", "spp": 16, "bounces": 0, "seed": 0, "denoise": False}
MODES = (('interactive', {}), ('raster', {"final": "raster"}), ('path', PATH0))


def srgb8_to_linear(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def linear_to_srgb8(x):
    x = min(max(x, 0.0), 1.0)
    s = x * 12.92 if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055
    return round(s * 255)


def ev_scale(ev):
    """Display value per nit at EV100 `ev`: 1 / (1.2 * 2^ev)."""
    return 1.0 / (1.2 * 2 ** ev)


def aces(x):
    return min(max(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), 0.0), 1.0)


def _hable_f(x):
    return (x * (0.15 * x + 0.05) + 0.004) / (x * (0.15 * x + 0.5) + 0.06) - 0.02 / 0.3


def hable(x):
    return min(max(_hable_f(2.0 * x) / _hable_f(11.2), 0.0), 1.0)


def render_replies(rows):
    """Every render reply (interactive, raster and path), in order."""
    return [r for r in rows if isinstance(r, dict) and r.get('ok') is True and 'samples' in r and 'finalMode' in r]


def nth_render(rows, n, what):
    rs = render_replies(rows)
    if not (-len(rs) <= n < len(rs)):
        fail(f"{what}: render reply #{n} missing ({len(rs)} replies): {json.dumps(rows)[-700:]}")
    return rs[n]


def look_of(r, what):
    lk = r.get('look') if isinstance(r, dict) else None
    if not isinstance(lk, dict):
        fail(f"{what}: the reply has no 'look' object: {json.dumps(r)[:500]}")
    return lk


def render_cmd(out, camera, size, **k):
    p = {"out": out, "size": list(size)}
    if camera is not None:
        p["camera"] = camera
    p.update(k)
    return cmd('render', p)


def rendered(cmds, what, **kw):
    """Run, fail on a refusal, return (rows, render replies)."""
    rc, rr, so, se = run(cmds, expect_ok=False, **kw)
    if rc != 0 or errors(se):
        fail(f"{what}: refused or failed (rc {rc}): {errors(se)[:6] or se[-900:]}")
    return rr, render_replies(rr)


def img(path):
    """(w, h, [(r, g, b)]) of a PNG."""
    if not os.path.exists(path):
        fail(f"no image at {path}")
    return pixels(path)


def at(im, x, y):
    w, h, p = im
    return p[y * w + x]


def box_mean(im, x0, y0, x1, y1, linear=True):
    """Mean (r, g, b) over the pixel box [x0, x1) x [y0, y1), linear light (or sRGB 8-bit)."""
    w, h, p = im
    acc, n = [0.0, 0.0, 0.0], 0
    for y in range(max(0, y0), min(h, y1)):
        for x in range(max(0, x0), min(w, x1)):
            c = p[y * w + x]
            for k in range(3):
                acc[k] += srgb8_to_linear(c[k]) if linear else c[k]
            n += 1
    if n == 0:
        fail(f"empty pixel box {x0},{y0}..{x1},{y1}")
    return [a / n for a in acc]


def centre_mean(path, half=3):
    w, h, p = img(path)
    return box_mean((w, h, p), w // 2 - half, h // 2 - half, w // 2 + half + 1, h // 2 + half + 1)


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def within(got, want, rel, what):
    if not (isinstance(got, (int, float)) and math.isfinite(got)) or abs(got - want) > rel * abs(want):
        fail(f"{what}: {got} is not {want:.6g} +- {rel * 100:g} %")


def level_cam(eye, yaw, pitch, fov):
    """A full camera at eye looking toward drawing angle `yaw` (0 = +X, 90 = +Y) pitched `pitch` up."""
    y, p = math.radians(yaw), math.radians(pitch)
    d = (math.cos(p) * math.cos(y), math.cos(p) * math.sin(y), math.sin(p))
    return {"kind": "full", "eye": list(eye), "target": [eye[i] + 10 * d[i] for i in range(3)], "fov": fov}


def down_cam(x, y, z, fov=40):
    """A full camera at (x, y, z) looking straight down (the target 1 mm north keeps the up vector defined)."""
    return {"kind": "full", "eye": [x, y, z], "target": [x, y + 0.001, 0.0], "fov": fov}


def _rgbe(c):
    v = max(c)
    if v < 1e-32:
        return bytes((0, 0, 0, 0))
    m, e = math.frexp(v)
    s = m * 256.0 / v
    return bytes((min(255, int(c[0] * s)), min(255, int(c[1] * s)), min(255, int(c[2] * s)), e + 128))


def write_rgbe(path, w, h, fn, rle=False):
    """A Radiance .hdr of w x h; fn(x, y) -> linear (r, g, b), row 0 at the top (the zenith of an equirect map).
    1.0 encodes as (128, 128, 128, 129). rle=True writes new-style run-length scanlines (8 <= w <= 32767)."""
    out = bytearray(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n" + f"-Y {h} +X {w}\n".encode())
    for y in range(h):
        px = [_rgbe(fn(x, y)) for x in range(w)]
        if not rle:
            for p in px:
                out += p
            continue
        out += bytes((2, 2, w >> 8, w & 255))
        for ch in range(4):
            vals = [p[ch] for p in px]
            i = 0
            while i < w:
                j = i
                while j < w and j - i < 127 and vals[j] == vals[i]:
                    j += 1
                if j - i >= 2:
                    out += bytes((128 + j - i, vals[i]))
                    i = j
                else:
                    out += bytes((1, vals[i]))
                    i += 1
    with open(path, 'wb') as f:
        f.write(bytes(out))
    return path


def sun_dir(az, el, true_north=0.0):
    """Unit toward-sun vector in drawing space for a compass azimuth/elevation (trueNorth 0: north = +Y)."""
    a, e = math.radians(az - true_north), math.radians(el)
    return [math.cos(e) * math.sin(a), math.cos(e) * math.cos(a), math.sin(e)]


def write_png(path, w, h, fn):
    """An 8-bit RGB PNG; fn(x, y) -> (r, g, b)."""
    import zlib
    raw = b''.join(b'\x00' + bytes(v for x in range(w) for v in fn(x, y)) for y in range(h))
    chunk = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) +
                chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))
    return path


def glb_image_bytes(j, binc, index):
    v = j['bufferViews'][j['images'][index]['bufferView']]
    return binc[v.get('byteOffset', 0):v.get('byteOffset', 0) + v['byteLength']]


def normal_maps(j, binc, tmp):
    """{base-colour image name (or material name): (normal image name, w, h, [(r, g, b)])} for every material with a
    normalTexture."""
    out = {}
    for k, m in enumerate(j.get('materials') or []):
        nt = m.get('normalTexture')
        if not isinstance(nt, dict):
            continue
        tex = j['textures'][nt['index']]
        name = j['images'][tex['source']].get('name') or str(tex['source'])
        p = os.path.join(tmp, f'normal-{k}.png')
        open(p, 'wb').write(glb_image_bytes(j, binc, tex['source']))
        w, h, px = pixels(p)
        out[texture_image_name(j, m) or m.get('name') or str(k)] = (name, w, h, px)
    return out


def backdrops(rows):
    """The `backdrop` objects (or None) of the backdrop and library.place replies, in order."""
    return [r['backdrop'] for r in rows if isinstance(r, dict) and 'backdrop' in r and 'look' not in r and 'samples' not in r]
