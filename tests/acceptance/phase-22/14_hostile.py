# Hostile input: every out-of-range or malformed photoreal parameter is refused (never clamped, never a panic), writes
# no image and leaves the drawing's look as it was.
# needs: p22-doc-look, p22-arch3d-calibrate, p22-engine-look-keys, p22-engine-render-params, p22-cli-hook, p22-engine-ladder, p22-engine-denoiser-info
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
out = os.path.join(tmp, 'never.png')
CAM = {'kind': 'full', 'eye': [2.0, 0.4, 1.4], 'target': [2.0, 3.0, 0.8], 'fov': 60}
BALL = {'shape': 'sphere', 'at': [0, 0, 0], 'size': 1.0, 'material': {}}
BAD_RENDER = [
    {'projection': 'fisheye'}, {'projection': 7}, {'compression': 2}, {'compression': 'x'}, {'compression': -0.1}, {'shift': 5},
    {'aperture': 0}, {'aperture': -1}, {'aperture': 'wide'}, {'focus': 0}, {'focus': -3}, {'focus': {'pick': [99999, 5]}}, {'focus': {'pick': 'x'}},
    {'film': 'vivid'}, {'film': {'bloom': -1}}, {'film': {'vignette': 2}}, {'film': {'whiteBalance': 10}}, {'film': {'contrast': 99}},
    {'film': {'saturation': -1}}, {'film': {'lift': 3}}, {'film': {'bloomThreshold': 0}}, {'film': {'grain': 1}},
    {'sun': {'disc': 90}}, {'sun': {'disc': -1}}, {'edges': {'radius': 1}}, {'edges': {'radius': -0.001}}, {'edges': 'soft'},
    {'materials': 'max'}, {'variation': 'yes'}, {'clamp': 0}, {'clamp': -2}, {'adaptive': 'on'}, {'noiseTarget': 0}, {'noiseTarget': 5},
    {'timeBudget': -5}, {'timeBudget': 1e9}, {'denoise': 'magic'}, {'denoiseModel': 'huge'}, {'rung': 8}, {'rung': -1}, {'rung': 1.5},
    {'pfm': os.path.join(tmp, 'x.pfm'), 'pfmStage': 'tone'}, {'pfm': 5},
    {'calibrate': {'objects': []}}, {'calibrate': {'objects': [dict(BALL, shape='torus')]}}, {'calibrate': {'objects': [BALL] * 100}},
    {'calibrate': {'objects': [dict(BALL, segments=100000)]}}, {'calibrate': {'objects': [dict(BALL, size=0)]}},
    {'calibrate': {'objects': [dict(BALL, size=1e12)]}}, {'calibrate': {'objects': [dict(BALL, at=[0, 0])]}},
    {'calibrate': {'objects': [dict(BALL, material={'clearcoat': 7})]}}, {'calibrate': {'objects': [dict(BALL, material={'ior': 0})]}},
    {'calibrate': {'objects': [BALL], 'lights': [{'at': [0, 0, 1], 'candela': -5}]}},
    {'calibrate': {'objects': [BALL], 'lights': [{'at': [0, 0, 1], 'candela': 10}] * 50}}, {'calibrate': 'cube'},
]
bad = [render_cmd(out, CAM, (64, 48), final='path', spp=1, **b) for b in BAD_RENDER]
bad += [cmd('look', {'film': {'contrast': 99}}), cmd('look', {'projection': 'panini', 'compression': 7}), cmd('look', {'aperture': 0}),
        cmd('look', {'sun': {'disc': 1000}}), cmd('look', {'technique': 'hyperreal'}),
        cmd('camera', {'save': 'c1', 'kind': 'full', 'eye': [0, -5], 'target': [0, 0, 1], 'compression': 7}),
        cmd('camera', {'save': 'c2', 'kind': 'full', 'eye': [0, -5], 'target': [0, 0, 1], 'film': {'bloom': 9}}),
        cmd('pick3d', {'camera': CAM, 'size': [64, 48], 'pixel': [3, 3], 'projection': 'panini', 'compression': 3}),
        cmd('render.ladder', {}), cmd('render.ladder', {'camera': 'nope', 'out': os.path.join(tmp, 'lad')}),
        cmd('render.ladder', {'camera': 5, 'out': os.path.join(tmp, 'lad')})]
rc, rows, so, se = run(ROOM + [COLUMN] + bad + ['look {}', cmd('arch.inspect', {'cameras': True}), 'denoiser.info {}'], expect_ok=False, timeout=3000)
errs = errors(se)
if len(errs) < len(bad):
    fail('%d hostile commands, %d refused: the last errors are %r' % (len(bad), len(errs), errs[-3:]))
if render_replies(rows) or os.path.exists(out) or os.path.exists(os.path.join(tmp, 'lad')):
    fail('a refused render still rendered or wrote a file')
lk = [r['look'] for r in rows if isinstance(r, dict) and isinstance(r.get('look'), dict)][-1]
if lk.get('technique') != 'standard' or (lk.get('lens') or {}).get('projection') != 'rectilinear' or abs(float((lk.get('film') or {}).get('contrast', 0)) - 1.0) > 1e-12 or abs(float((lk.get('sun') or {}).get('disc', -1))) > 1e-12:
    fail('the refused look commands left the look as it was: %r' % lk)
cams = [r for r in rows if isinstance(r, dict) and isinstance(r.get('cameras'), list)][-1]['cameras']
if cams:
    fail('a refused camera save left a camera behind: %r' % [c.get('name') for c in cams])
if not [r for r in rows if isinstance(r, dict) and isinstance(r.get('models'), list)]:
    fail('denoiser.info still answers after the refusals')
print('ok: %d hostile commands refused, nothing rendered, the look and cameras unchanged' % len(bad))
print('PASS 14')
