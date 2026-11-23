# The Photoreal preset, and nothing else moved. (a) a saved camera with technique photoreal renders path-traced without
# `final`, as rung 7: rounded edges, full materials with variation, 12 bounces, the 0.53 degree sun, the clamp, the
# Photoreal film, adaptive sampling to 2 % or 10 min, the roughness guide and the learned denoiser; a key the request
# names wins over the preset. (b) look {} reports the new defaults; lens and film survive a save and a reload on the
# camera. (c) every existing technique renders exactly as before: naming every new control at its neutral value changes
# no pixel of the interactive view, the raster Finalize or the path-traced Finalize.
# needs: p22-doc-look, p22-arch3d-lens, p22-r3d-types, p22-r3d-kernel-hooks, p22-r3d-material-map, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-lens-path, p22-r3d-lens-raster, p22-r3d-edges, p22-r3d-bsdf-ext, p22-r3d-variation, p22-r3d-sun-disc, p22-r3d-clamp, p22-r3d-film, p22-r3d-adaptive, p22-r3d-roughness-guide, p22-r3d-unet-gpu
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (96, 64)
SET = HOUSE1 + [SITE, cmd('window', {'wall': 'w0', 'offset': 2.0, 'width': 1.2, 'sill': 0.9, 'height': 1.2}),
                cmd('lightfixture', {'room': 'r0', 'rows': 1, 'cols': 2}),
                cmd('sunangle', dict({'name': 'noon'}, **NOON)), cmd('look', {'sun': {'angle': 'noon'}})]
POSE = {'kind': 'full', 'eye': [-6, -6], 'eyeHeight': 1.6, 'target': [4, 2.5, 1.2], 'fov': 50}
dxf = os.path.join(tmp, 'photo.dxf')
a, b = os.path.join(tmp, 'a.png'), os.path.join(tmp, 'b.png')
rc, rows, so, se = run(SET + ['look {}',
                              cmd('camera', dict(POSE, save='hero', technique='photoreal', projection='panini', compression=0.5, shift=0.1,
                                                 aperture=8, focus=9.0, film={'vignette': 0.3})),
                              render_cmd(a, 'hero', SIZE, spp=8, seed=1, timeBudget=30),
                              render_cmd(b, 'hero', SIZE, spp=8, seed=1, timeBudget=30, bounces=2, denoise='filter', edges=False),
                              cmd('arch.inspect', {'cameras': True})], save=dxf, expect_ok=False, timeout=3000)
if rc != 0 or errors(se):
    fail('the Photoreal renders are refused or failed: %s' % (errors(se)[:4] or se[-600:]))
lk = [r['look'] for r in rows if isinstance(r, dict) and isinstance(r.get('look'), dict) and 'samples' not in r][0]
lens0, film0 = lk.get('lens') or {}, lk.get('film') or {}
if lens0 != {'projection': 'rectilinear', 'compression': 0.7, 'shift': 0.0, 'aperture': None, 'focus': None}:
    fail('(b) look {} reports the neutral lens: %r' % lens0)
if film0 != {'bloom': 0.0, 'bloomThreshold': 1.0, 'vignette': 0.0, 'whiteBalance': None, 'contrast': 1.0, 'saturation': 1.0, 'lift': 0.0}:
    fail('(b) look {} reports the neutral film: %r' % film0)
if abs(float((lk.get('sun') or {}).get('disc', -1))) > 1e-12:
    fail('(b) look {} reports sun.disc 0: %r' % lk.get('sun'))
rs = render_replies(rows)
if len(rs) != 2:
    fail('two render replies: %d' % len(rs))
r = rs[0]
is_path(r, 'a Photoreal camera renders path-traced without `final`')
if look_of(r, 'photoreal').get('technique') != 'photoreal':
    fail('look.technique echoes photoreal: %r' % r['look'].get('technique'))
p = photo_of(r, 'photoreal')
want = {'rung': 7, 'materials': 'full', 'variation': True, 'bounces': 12, 'adaptive': True, 'roughnessGuide': True, 'denoise': 'learned'}
for key, v in want.items():
    if p.get(key) != v:
        fail('(a) Photoreal sets photo.%s to %r: %r' % (key, v, p.get(key)))
if p.get('edges') is False:
    fail('(a) Photoreal rounds edges')
near(p.get('sunDisc'), 0.53, 1e-9, '(a) photo.sunDisc')
near(p.get('clamp'), 32.0, 1e-9, '(a) photo.clamp')
near(p.get('noiseTarget'), 0.02, 1e-9, '(a) photo.noiseTarget')
near(p.get('timeBudget'), 30.0, 1e-9, '(a) the given time budget')
fm = r['look'].get('film') or {}
near(fm.get('vignette'), 0.3, 1e-9, '(a) the camera own film wins over the preset film')
ln = r['look'].get('lens') or {}
if ln.get('projection') != 'panini' or abs(float(ln.get('compression', 0)) - 0.5) > 1e-9 or abs(float(ln.get('aperture') or 0) - 8) > 1e-9:
    fail('(a) the camera lens is used: %r' % ln)
if r.get('stop') not in ('samples', 'noise target', 'time budget') or r.get('denoiser') != 'learned':
    fail('(a) the reply says why it stopped and names the learned denoiser: %r, %r (%r)' % (r.get('stop'), r.get('denoiser'), r.get('denoiseFallback')))
p2 = photo_of(rs[1], 'overrides')
if p2.get('bounces') != 2 or p2.get('denoise') != 'filter' or p2.get('edges') is not False or p2.get('materials') != 'full':
    fail('(a) a key the request names wins over the preset, the rest stays: %r' % p2)
w, h, px = img(a)
if len(set(px)) < 16:
    fail('(a) the Photoreal still is blank')
rows = run_ok([cmd('arch.inspect', {'cameras': True})], 'the reload', file=dxf)
cams = [r for r in rows if isinstance(r, dict) and isinstance(r.get('cameras'), list)][-1]['cameras']
hero = [c for c in cams if c.get('name') == 'hero']
if not hero or not isinstance(hero[0].get('look'), dict):
    fail('(b) the camera hero and its look survive the save: %r' % cams)
hl = hero[0]['look']
if hl.get('technique') != 'photoreal' or (hl.get('lens') or {}).get('projection') != 'panini' or abs(float((hl.get('lens') or {}).get('focus') or 0) - 9.0) > 1e-9 or abs(float((hl.get('film') or {}).get('vignette') or 0) - 0.3) > 1e-9:
    fail('(b) technique, lens and film round-trip through the DXF: %r' % hl)
print('ok: (a) the Photoreal preset, (b) defaults and round trip')

# (c) neutral values change no pixel.
NEUTRAL = {'projection': 'rectilinear', 'compression': 0.7, 'shift': 0, 'aperture': None, 'focus': None, 'film': None, 'edges': False,
           'materials': 'basic', 'variation': False, 'clamp': None, 'adaptive': False, 'denoise': True, 'roughnessGuide': False}
INL = {'kind': 'full', 'eye': [-6, -6, 1.6], 'target': [4, 2.5, 1.2], 'fov': 50}
cmds, pairs = [], []
for mode, extra in (('interactive', {}), ('raster', {'final': 'raster'}), ('path', {'final': 'path', 'spp': 8, 'seed': 4})):
    x, y = os.path.join(tmp, mode + '-0.png'), os.path.join(tmp, mode + '-1.png')
    cmds += [render_cmd(x, INL, (160, 120), **extra), render_cmd(y, INL, (160, 120), **dict(NEUTRAL, **extra))]
    pairs.append((mode, x, y))
rendered(SET + cmds, 'the neutral renders', timeout=3000)
for mode, x, y in pairs:
    if digest(x) != digest(y):
        fail('(c) %s: naming every new control at its neutral value changes the image' % mode)
print('PASS 12')
