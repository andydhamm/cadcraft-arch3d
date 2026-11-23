# The demo on the showcase house: docs/arch3d/showcase/phase-22-showcase.json (replayed after bilevel.json) saves three
# Photoreal cameras: Kitchen-photo, Living-photo and Dusk-photo (the exterior at the `dusk` sun angle). Here a small
# realism ladder of the kitchen (eight non-blank columns) and small Photoreal stills of the other two; the gallery tool
# renders them at full size (render_all.py --full: a ladder per camera and three 1920 x 1080 finals).
# needs: p22-doc-look, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-r3d-lens-path, p22-r3d-edges, p22-r3d-bsdf-ext, p22-r3d-bsdf-transmission, p22-r3d-variation, p22-r3d-sun-disc, p22-r3d-clamp, p22-r3d-film, p22-r3d-adaptive, p22-r3d-unet-gpu, p22-engine-ladder, p22-showcase-cameras, p22-gallery-tool
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
NAMES = ['Kitchen-photo', 'Living-photo', 'Dusk-photo']
SIZE = (64, 40)
lad = os.path.join(tmp, 'kitchen')
stills = [os.path.join(tmp, 'living.png'), os.path.join(tmp, 'dusk.png')]
q = dict(spp=4, seed=1, timeBudget=10, noiseTarget=0.5)
cmds = showcase_cmds() + script_cmds('phase-22-showcase.json') + [
    cmd('arch.inspect', {'cameras': True}),
    cmd('render.ladder', dict(q, camera=NAMES[0], out=lad, size=list(SIZE))),
    render_cmd(stills[0], NAMES[1], SIZE, **q), render_cmd(stills[1], NAMES[2], SIZE, **q)]
rc, rows, so, se = run(cmds, expect_ok=False, timeout=14000)
if rc != 0 or errors(se):
    fail('the showcase scripts replay and render without a refused step: %s' % (errors(se)[:4] or se[-600:]))
cams = [r for r in rows if isinstance(r, dict) and isinstance(r.get('cameras'), list) and 'activeCamera' in r]
if not cams:
    fail('arch.inspect {cameras: true} lists the cameras')
by = {c.get('name'): c for c in cams[-1]['cameras'] if isinstance(c, dict)}
for n in NAMES:
    lk = (by.get(n) or {}).get('look')
    if not isinstance(lk, dict) or lk.get('technique') != 'photoreal':
        fail('the showcase has the Photoreal camera %s: %r' % (n, lk))
if ((by['Dusk-photo']['look'].get('sun') or {}).get('angle')) != 'dusk':
    fail('Dusk-photo uses the dusk sun angle: %r' % by['Dusk-photo']['look'].get('sun'))
for n in ('Front', 'Kitchen', 'Living', 'dusk', 'noon-hdri', 'overview'):
    if n not in by:
        fail('the phase-22 script keeps the earlier camera %s' % n)
if cams[-1].get('activeCamera') != 'Front':
    fail('Front stays the active camera: %r' % cams[-1].get('activeCamera'))
ladder = [r for r in rows if isinstance(r, dict) and isinstance(r.get('rungs'), list)]
if len(ladder) != 1 or len(ladder[0]['rungs']) != 8:
    fail('one realism ladder of eight rungs')
for i in range(8):
    f = os.path.join(lad, 'rung-%d.png' % i)
    if not os.path.exists(f) or luma_sd(f) < 1.0:
        fail('rung %d of the kitchen ladder is a non-blank image' % i)
w, h, _ = img(os.path.join(lad, 'ladder.png'))
if (w, h) != (8 * SIZE[0], SIZE[1] + 24):
    fail('the kitchen ladder sheet is eight columns: %dx%d' % (w, h))
rs = [r for r in render_replies(rows) if 'rung' not in r][-2:]
for f, r, n in zip(stills, rs, NAMES[1:]):
    if r.get('finalMode') != 'path' or photo_of(r, n).get('rung') != 7:
        fail('%s renders as Photoreal (path-traced, rung 7): %r, %r' % (n, r.get('finalMode'), r.get('photo')))
    if luma_sd(f) < 1.0:
        fail('the Photoreal still of %s is blank' % n)
tool = open(os.path.join(P22_ROOT, 'tools', 'showcase', 'render_all.py'), encoding='utf-8').read()
for needle in ('render.ladder', 'phase-22-showcase.json', 'photoreal-'):
    if needle not in tool:
        fail('tools/showcase/render_all.py does not mention %s: the full gallery has no ladders or Photoreal finals' % needle)
print('PASS 13')
