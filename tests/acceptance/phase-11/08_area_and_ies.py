# Area lights and IES files: the LED panel is a 0.6 x 0.6 m Lambertian rectangle (3600 lm: 1145.9 cd along its aim) whose
# illuminance close below follows the rectangle, not a point; an IES file gives a fixture its measured distribution in
# arch.inspect, the direct illuminance and the path tracer alike.
# needs: p11-arch-ies, p11-arch-panel, p11-arch3d-panel, p11-arch3d-ies-area, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-cmd-lightfixture-area-ies, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

tmp = tempfile.mkdtemp()
rc, rr, so, se = run(['lightfixture.types {}'])
types = reply_with(rr, 'types')['types']
panel = next((t for t in types if t.get('name') == 'LED panel 600'), None)
if not panel or panel.get('kind') != 'area' or num(panel.get('width'), 'width') != 0.6 or num(panel.get('length'), 'length') != 0.6 \
        or num(panel.get('lumens'), 'lumens') != 3600 or num(panel.get('kelvin'), 'kelvin') != 4000:
    fail(f"the catalogue has 'LED panel 600': an area light 0.6 x 0.6 m, 3600 lm, 4000 K: {panel}")

# The panel alone in the room; L = 3600 / (π · 0.36) = 3183.1 nits. Exact E below its centre: 194.9 lx at 2.4 m, 4128.4 lx
# at 0.4 m (a point source would give 198.9 and 7161.9).
ILL = lambda p, **k: cmd('illuminance', dict({"point": p}, **k))
rc, rr, so, se = run(room_cmds() + [cmd('lightfixture', {"type": "LED panel 600", "at": [2, 2]}), 'arch.inspect {"lights":true}',
                                    ILL([2, 2, 2.0]), ILL([2, 2, 2.0], method="path", bounces=0, spp=4096),
                                    ILL([2, 2, 0], method="path", bounces=0, spp=1024)])
lf = lights_of(rr)
if len(lf) != 1 or lf[0].get('kind') != 'area' or num(lf[0].get('width'), 'width') != 0.6:
    fail(f"arch.inspect lists the panel as an area light 0.6 wide: {lf}")
near(lf[0].get('candela'), 3600 / math.pi, 0.1, 'panel candela along its aim (lumens / π)')
near(lf[0].get('z'), 2.4, 1e-9, 'panel face at the ceiling')
lx = [x for x in rr if isinstance(x, dict) and 'lux' in x]
near(lx[0]['lux'], 4128.4, 4128.4 * 0.03, 'direct illuminance 0.4 m below the panel (the rectangle, not a point)')
near(lx[1]['lux'], 4128.4, 4128.4 * 0.03, 'path-traced illuminance 0.4 m below the panel')
near(lx[2]['lux'], 194.9, 194.9 * 0.03, 'path-traced illuminance on the floor below the panel')
print(f"ok: panel 1145.9 cd; 0.4 m below: direct {lx[0]['lux']:.0f} lx, path {lx[1]['lux']:.0f} lx (4128); floor {lx[2]['lux']:.1f} lx (194.9)")

# The IES downlight on a pot light at (1, 1): 1000 cd straight down, 900 at 22.5°. At (2, 1, 0): θ = 22.62°, 898.4 cd,
# E = 898.4 · cos θ / 6.76 = 122.7 lx; at (1, 1, 0): 1000 / 5.76 = 173.6 lx.
ies = ies_file(tmp)
rc, rr, so, se = run(room_cmds() + [cmd('lightfixture', {"type": "Recessed pot light", "at": [1, 1]}),
                                    cmd('lightfixture.edit', {"id": "lf0", "ies": ies}), 'arch.inspect {"lights":true}',
                                    ILL([1, 1, 0]), ILL([2, 1, 0]), ILL([2, 1, 0], method="path", bounces=0, spp=256)])
lf = lights_of(rr)[0]
prof = lf.get('ies')
if not isinstance(prof, dict) or prof.get('name') != 'Test downlight 1000 lm' or num(prof.get('lumens'), 'lumens') != 1000 \
        or num(prof.get('peak'), 'peak') != 1000:
    fail(f"arch.inspect shows the IES profile (name from [LUMINAIRE], 1000 lm, peak 1000 cd): {prof}")
if num(lf.get('lumens'), 'lumens') != 1000:
    fail(f"setting an IES file sets the fixture's lumens to the file's: {lf.get('lumens')}")
lx = [x for x in rr if isinstance(x, dict) and 'lux' in x]
near(lx[0]['lux'], 173.6, 173.6 * 0.03, 'IES direct illuminance straight below')
near(lx[1]['lux'], 122.7, 122.7 * 0.03, 'IES direct illuminance at 22.6° (interpolated 898.4 cd)')
near(lx[2]['lux'], 122.7, 122.7 * 0.03, 'IES path-traced illuminance at 22.6°')
print(f"ok: IES 173.6 / 122.7 lx direct, {lx[2]['lux']:.1f} lx path-traced")
print("PASS 08")
