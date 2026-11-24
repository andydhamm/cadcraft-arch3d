# The realism ladder: render.ladder {camera, out} writes one PNG per rung (same camera, same seed) and one contact sheet
# with the rung names. Rung 0 is pixel-identical to a plain path-traced final of that camera; each rung's reply shows
# the settings it added; the sheet is the eight images side by side under a label band.
# needs: p22-doc-look, p22-arch3d-lens, p22-r3d-types, p22-r3d-kernel-hooks, p22-r3d-material-map, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-lens-path, p22-r3d-edges, p22-r3d-bsdf-ext, p22-r3d-variation, p22-r3d-sun-disc, p22-r3d-clamp, p22-r3d-film, p22-r3d-adaptive, p22-r3d-roughness-guide, p22-r3d-unet-gpu, p22-engine-ladder
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
SIZE = (96, 64)
W, H = SIZE
SET = HOUSE1 + [SITE, cmd('window', {'wall': 'w0', 'offset': 2.0, 'width': 1.2, 'sill': 0.9, 'height': 1.2}),
                cmd('lightfixture', {'room': 'r0', 'rows': 1, 'cols': 2}),
                cmd('sunangle', dict({'name': 'noon'}, **NOON)), cmd('look', {'sun': {'angle': 'noon'}})]
POSE = {'kind': 'full', 'eye': [-6, -6], 'eyeHeight': 1.6, 'target': [4, 2.5, 1.2], 'fov': 50}
plain_dir, lens_dir = os.path.join(tmp, 'plain'), os.path.join(tmp, 'lens')
q = {'size': list(SIZE), 'spp': 8, 'seed': 5, 'timeBudget': 30}
ref, ref2 = os.path.join(tmp, 'ref.png'), os.path.join(tmp, 'ref2.png')
rc, rows, so, se = run(SET + [
    cmd('camera', dict(POSE, save='plain', technique='path')),
    cmd('camera', dict(POSE, save='wide', technique='photoreal', projection='panini', shift=0.1)),
    render_cmd(ref, 'plain', SIZE, final='path', spp=8, seed=5),
    render_cmd(ref2, 'wide', SIZE, final='path', technique='path', projection='rectilinear', shift=0, aperture=None, film=None, spp=8, seed=5),
    cmd('render.ladder', dict(q, camera='plain', out=plain_dir)),
    cmd('render.ladder', dict(q, camera='wide', out=lens_dir))], expect_ok=False, timeout=6000)
if rc != 0 or errors(se):
    fail('render.ladder is refused or failed: %s' % (errors(se)[:4] or se[-600:]))
ladders = [r for r in rows if isinstance(r, dict) and isinstance(r.get('rungs'), list)]
if len(ladders) != 2:
    fail('two render.ladder replies with rungs: %d' % len(ladders))


def check_ladder(rep, d, what):
    rungs = rep['rungs']
    if [r.get('rung') for r in rungs] != list(range(8)) or [r.get('name') for r in rungs] != RUNGS:
        fail('%s: eight rungs named %r: %r' % (what, RUNGS, [(r.get('rung'), r.get('name')) for r in rungs]))
    files = []
    for i, r in enumerate(rungs):
        f = os.path.join(d, 'rung-%d.png' % i)
        if not os.path.exists(f) or os.path.abspath(str(r.get('out'))) != os.path.abspath(f):
            fail('%s: rung %d is written to %s (reply says %r)' % (what, i, f, r.get('out')))
        w, h, px = img(f)
        if (w, h) != SIZE or len(set(px)) < 8:
            fail('%s: rung %d is a %dx%d image with %d colours' % (what, i, w, h, len(set(px))))
        if r.get('seed') != 5 or r.get('finalMode') != 'path':
            fail('%s: rung %d is path-traced with the same seed 5: seed %r, finalMode %r' % (what, i, r.get('seed'), r.get('finalMode')))
        files.append(f)
    ph = [photo_of(r, '%s rung %d' % (what, i)) for i, r in enumerate(rungs)]
    for i in range(8):
        if ph[i].get('rung') != i:
            fail('%s: photo.rung of rung %d is %r' % (what, i, ph[i].get('rung')))
        if (ph[i].get('edges') is not False) != (i >= 2):
            fail('%s rung %d: rounded edges from rung 2 on: %r' % (what, i, ph[i].get('edges')))
        if ph[i].get('materials') != ('full' if i >= 3 else 'basic') or ph[i].get('variation') is not (i >= 3):
            fail('%s rung %d: full materials and variation from rung 3 on: %r, %r' % (what, i, ph[i].get('materials'), ph[i].get('variation')))
        if ph[i].get('bounces') != (12 if i >= 4 else 4) or abs(float(ph[i].get('sunDisc', -1)) - (0.53 if i >= 4 else 0.0)) > 1e-6:   # 0.53 as a 32-bit float is 0.52999997
            fail('%s rung %d: 12 bounces and the 0.53 degree sun from rung 4 on: %r, %r' % (what, i, ph[i].get('bounces'), ph[i].get('sunDisc')))
        if (ph[i].get('clamp') is not None) != (i >= 4):
            fail('%s rung %d: the clamp from rung 4 on: %r' % (what, i, ph[i].get('clamp')))
        vig = (look_of(rungs[i], 'rung').get('film') or {}).get('vignette')
        if abs(float(vig if vig is not None else -1) - (0.15 if i >= 5 else 0.0)) > 1e-9:
            fail('%s rung %d: the film (vignette 0.15) from rung 5 on: %r' % (what, i, vig))
        if ph[i].get('adaptive') is not (i >= 6) or ph[i].get('roughnessGuide') is not (i >= 6):
            fail('%s rung %d: adaptive sampling and the roughness guide from rung 6 on: %r, %r' % (what, i, ph[i].get('adaptive'), ph[i].get('roughnessGuide')))
        if i >= 6 and (abs(float(ph[i].get('noiseTarget') or 0) - 0.02) > 1e-9 or abs(float(ph[i].get('timeBudget') or 0) - 30) > 1e-9):
            fail('%s rung %d: noise target 0.02 and the given 30 s budget: %r, %r' % (what, i, ph[i].get('noiseTarget'), ph[i].get('timeBudget')))
        if ph[i].get('denoise') != ('learned' if i == 7 else 'filter'):
            fail('%s rung %d: the learned denoiser on rung 7 only: %r' % (what, i, ph[i].get('denoise')))
    if rungs[7].get('denoiser') != 'learned':
        fail('%s: rung 7 ran the learned denoiser: %r (%r)' % (what, rungs[7].get('denoiser'), rungs[7].get('denoiseFallback')))
    for i in (4, 5, 7):
        if digest(files[i]) == digest(files[i - 1]):
            fail('%s: rung %d (%s) is the same image as rung %d' % (what, i, RUNGS[i], i - 1))
    sheet = os.path.join(d, 'ladder.png')
    if not os.path.exists(sheet) or os.path.abspath(str(rep.get('sheet'))) != os.path.abspath(sheet):
        fail('%s: the contact sheet is %s (reply says %r)' % (what, sheet, rep.get('sheet')))
    sw, sh, sp = img(sheet)
    if (sw, sh) != (8 * W, H + 24):
        fail('%s: the sheet is %dx%d, not %dx%d' % (what, sw, sh, 8 * W, H + 24))
    bands = []
    for i, f in enumerate(files):
        _, _, px = img(f)
        cell = [sp[(24 + y) * sw + i * W + x] for y in range(H) for x in range(W)]
        if cell != px:
            fail('%s: column %d of the sheet is not rung-%d.png' % (what, i, i))
        band = [sp[y * sw + i * W + x] for y in range(24) for x in range(W)]
        ink = sum(1 for c in band if c != (24, 24, 24))
        if ink < 10 or ink > len(band) // 2:
            fail('%s: the label band over rung %d carries its name on a (24, 24, 24) background: %d inked pixels' % (what, i, ink))
        bands.append(tuple(band))
    if len(set(bands)) != 8:
        fail('%s: the eight rung labels differ from one another' % what)
    return files


plain = check_ladder(ladders[0], plain_dir, 'plain camera')
wide = check_ladder(ladders[1], lens_dir, 'lens camera')
if digest(plain[0]) != digest(ref):
    fail('rung 0 is pixel-identical to a plain path-traced final of that camera (spp 8, seed 5)')
if digest(plain[1]) != digest(plain[0]):
    fail('a camera with no lens settings gives the same image on rung 1 as on rung 0')
if digest(wide[0]) != digest(ref2):
    fail('rung 0 of a camera with a lens is the plain path-traced final with the lens neutral (pinhole)')
if digest(wide[1]) == digest(wide[0]):
    fail('rung 1 applies the camera lens (Panini, shift 0.1): the image is rung 0')
print('PASS 11')
