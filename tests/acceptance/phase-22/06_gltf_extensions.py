# The new PBR fields are exported as the matching glTF extensions, and nothing else changes in the export. A room typed
# bedroom (carpet floor) with the phase-13 kitchen and a window: the counter carries KHR_materials_clearcoat (quartz), the
# stainless appliances KHR_materials_anisotropy, the carpet KHR_materials_sheen, the glazing KHR_materials_transmission
# and KHR_materials_volume; every extension used is listed in extensionsUsed; the wall material carries none of them.
# needs: p22-arch3d-material-ext, p22-io-glb-ext
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p22lib.inc')).read())

tmp = tempfile.mkdtemp()
EXT = ['KHR_materials_clearcoat', 'KHR_materials_anisotropy', 'KHR_materials_sheen', 'KHR_materials_transmission', 'KHR_materials_volume']
rr, (j, binc) = export_glb(ROOM + KITCHEN + [cmd('room', {'id': 'r0', 'type': 'bedroom'}),
                                             cmd('window', {'wall': 'w3', 'offset': 2.0, 'width': 0.9, 'sill': 0.9, 'height': 1.2})], tmp, 'ext.glb')
used = j.get('extensionsUsed') or []
for e in EXT:
    if e not in used:
        fail('extensionsUsed lists %s: %r' % (e, used))
mats = j.get('materials') or []
for m in mats:
    for e in (m.get('extensions') or {}):
        if e not in used:
            fail('material %r uses %s, which extensionsUsed does not list' % (m.get('name'), e))


def ext_of(pred, ext, what):
    hits = [m for m in mats if pred(str(m.get('name', '')).lower())]
    if not hits:
        fail('the GLB has no %s material: %r' % (what, [m.get('name') for m in mats]))
    with_ext = [m['extensions'][ext] for m in hits if isinstance(m.get('extensions'), dict) and isinstance(m['extensions'].get(ext), dict)]
    if not with_ext:
        fail('no %s material carries %s: %r' % (what, ext, [(m.get('name'), m.get('extensions')) for m in hits][:4]))
    return with_ext[0]


cc = ext_of(lambda n: n.startswith('counter'), EXT[0], 'counter')
if not num(cc.get('clearcoatFactor'), 'clearcoatFactor') > 0:
    fail('the counter clearcoatFactor is above 0: %r' % cc)
an = ext_of(lambda n: n in ('fixture-range', 'fixture-refrigerator', 'fixture-dishwasher'), EXT[1], 'stainless appliance')
if not num(an.get('anisotropyStrength'), 'anisotropyStrength') > 0:
    fail('a stainless appliance has anisotropyStrength above 0: %r' % an)
sh = ext_of(lambda n: 'carpet' in n, EXT[2], 'carpet')
if not (isinstance(sh.get('sheenColorFactor'), list) and max(sh['sheenColorFactor']) > 0):
    fail('the carpet has a sheenColorFactor above 0: %r' % sh)
tr = ext_of(lambda n: 'glass' in n, EXT[3], 'glass')
if not 0 < num(tr.get('transmissionFactor'), 'transmissionFactor') <= 1:
    fail('the glazing has a transmissionFactor in (0, 1]: %r' % tr)
vo = ext_of(lambda n: 'glass' in n, EXT[4], 'glass')
if not num(vo.get('thicknessFactor'), 'thicknessFactor') > 0:
    fail('the glazing has a thicknessFactor above 0: %r' % vo)
walls = [m for m in mats if str(m.get('name', '')).lower() == 'wall']
if not walls:
    fail('the GLB has no wall material: %r' % [m.get('name') for m in mats])
for m in walls:
    if any(e in (m.get('extensions') or {}) for e in EXT):
        fail('the plain wall material carries a photoreal extension: %r' % m.get('extensions'))
for m in mats:
    if 'pbrMetallicRoughness' not in m:
        fail('material %r lost its pbrMetallicRoughness' % m.get('name'))
validate_glb(os.path.join(tmp, 'ext.glb'))
print('PASS 06')
