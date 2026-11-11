# Hostile input to every phase-19 command and parameter is refused with an error line, never a panic, and changes
# nothing: the look, the sun angles, the cameras, the lights and the backdrop stay as they were. (Non-finite numbers
# cannot be sent: JSON has no NaN, and the CLI refuses a non-JSON --cmd before running anything.)
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-exposure-1, p19-exposure-2, p19-exposure-3, p19-sun-1, p19-sun-2, p19-hdri-1, p19-hdri-2, p19-hdri-3, p19-hdri-4, p19-strip-model-1, p19-strip-model-2, p19-strip-tray, p19-seeing-in, p19-cameras-1, p19-cameras-2, p19-cameras-3, p19-cameras-4
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p19lib.inc')).read())

tmp = tempfile.mkdtemp()
TXT = os.path.join(tmp, 'notes.hdr')
open(TXT, 'w').write("this is not a Radiance file\n")
SMALL = write_rgbe(os.path.join(tmp, 'small.hdr'), 4, 2, lambda x, y: (1.0, 1.0, 1.0))
OUT = os.path.join(tmp, 'x.png')
CAM = {"kind": "full", "eye": [3.5, 3.5, 1.6], "target": [0.5, 0.5, 0.8], "fov": 60}
SETUP = ROOM + [SITE, cmd('lightfixture', {"room": "r0", "rows": 1, "cols": 2}),
                cmd('lightset', {"name": "Day", "lights": {"lf0": {"on": False}, "lf1": {"on": False}}}),
                cmd('sunangle', {"name": "low", "azimuth": 180, "elevation": 30}),
                cmd('camera', {"save": "c1", "kind": "full", "eye": [3.5, 3.5], "eyeHeight": 1.6, "target": [0.5, 0.5, 0.8], "fov": 60}),
                cmd('look', {"toneMap": "hable"})]
STRIP = {"type": "LED strip", "height": 2.4}
H = [('look', {"exposure": 5}), ('look', {"exposure": {"ev100": "1"}}), ('look', {"exposure": {"ev100": 99}}),
     ('look', {"toneMap": "filmic"}), ('look', {"sun": {"intensity": -1}}), ('look', {"sun": {"intensity": 1e9}}),
     ('look', {"sun": {"angle": "Nope"}}), ('look', {"sun": {"date": "2026-06-21", "time": "12:00"}}),
     ('look', {"sky": {"turbidity": 50}}), ('look', {"sky": {"moon": {"lux": -0.1}}}),
     ('look', {"clip": {"min": [0, 0, 0], "max": [0, 1, 1]}}), ('look', {"clip": {"min": [0, 0], "max": [1, 1, 1]}}),
     ('look', {"nearClip": 0}), ('look', {"technique": "watercolor"}), ('look', {"cutaway": "yes"}),
     ('look', {"lightSet": "Nope"}), ('look', {"bogus": 1}),
     ('sunangle', {"name": "", "azimuth": 0, "elevation": 10}), ('sunangle', {"name": "a", "azimuth": 0, "elevation": 91}),
     ('sunangle', {"name": "b", "date": "2026-13-01", "time": "12:00"}), ('sunangle', {"name": "c", "azimuth": 0}),
     ('sunangle', {"name": "d", "date": "2026-06-21", "time": "12:00", "azimuth": 0, "elevation": 10}),
     ('sunangle', {"name": "low", "azimuth": 0, "elevation": 10}), ('sunangle', {"delete": "Nope"}),
     ('render', {"out": OUT, "camera": CAM, "size": [8, 6], "exposure": {"ev100": None}}),
     ('render', {"out": OUT, "camera": CAM, "size": [8, 6], "toneMap": 3}),
     ('render', {"out": OUT, "camera": CAM, "size": [8, 6], "albedo": 1.5}),
     ('render', {"out": OUT, "camera": CAM, "size": [8, 6], "sun": {"azimuth": 0, "elevation": 120}}),
     ('render', {"out": OUT, "camera": CAM, "size": [8, 6], "sun": {"angle": "low", "azimuth": 0, "elevation": 10}}),
     ('backdrop', {"kind": "hdri"}), ('backdrop', {"kind": "hdri", "file": "/nonexistent.hdr"}),
     ('backdrop', {"kind": "hdri", "file": TXT}), ('backdrop', {"kind": "hdri", "file": SMALL, "intensity": -1}),
     ('backdrop', {"kind": "hdri", "file": SMALL, "horizon": 90}),
     ('backdrop', {"kind": "image", "item": "own:backdrop-sky", "projection": "flat", "horizon": 5}),
     ('backdrop', {"kind": "sky", "colour": "#000000"}),
     ('backdrop', {"kind": "image", "item": "polyhaven:kloofendal-puresky"}),
     ('lightfixture', dict(STRIP, path=[[0, 0]])), ('lightfixture', dict(STRIP, path=[[1, 1], [3, 1]], lumensPerMetre=-5)),
     ('lightfixture', dict(STRIP, path=[[1, 1], [3, 1]], facing="up")), ('lightfixture', {"type": "LED strip", "under": ["w0"]}),
     ('lightfixture', {"type": "LED strip", "room": "r0", "inset": -1}), ('lightfixture', {"type": "LED strip", "tray": "tr9"}),
     ('lightfixture', {"type": "LED strip", "path": [[1, 1], [3, 1]], "room": "r0", "inset": 0.5}),
     ('camera', {"save": "x", "kind": "overview", "level": 9}), ('camera.reset', {"name": "Nope"}),
     ('camera.orbit', {"name": "c1", "azimuth": "10"}), ('camera.update', {"name": "Nope"}),
     ('pick3d', {"camera": CAM, "size": [10, 10], "pixel": [5, 5], "clip": {"min": [1, 1, 1], "max": [0, 0, 0]}}),
     ('pick3d', {"camera": "Nope", "size": [10, 10], "pixel": [5, 5]}),
     ('illuminance', {"point": [1, 1, 0], "sun": {"intensity": "x"}})]
STATE = [cmd('arch.inspect', {"look": True, "sunAngles": True, "cameras": True, "lights": True}), 'backdrop {}']


def state(rows):
    ins = [r for r in rows if isinstance(r, dict) and isinstance(r.get('look'), dict) and 'sunAngles' in r and 'cameras' in r]
    bd = backdrops(rows)
    if not ins or not bd:
        fail(f"arch.inspect {{look, sunAngles, cameras, lights}} and `backdrop {{}}` report: {json.dumps(rows)[-500:]}")
    i = ins[-1]
    return {k: i.get(k) for k in ('look', 'sunAngles', 'cameras', 'lights')} | {'backdrop': bd[-1]}


rc, rr, so, se = run(SETUP + STATE, expect_ok=False)
if errors(se):
    fail("the set-up is refused:\n" + '\n'.join(errors(se)))
before = state(rr)
rc, rr, so, se = run(SETUP + [cmd(n, p) for n, p in H] + STATE, expect_ok=False)
errs = errors(se)
if len(errs) < len(H):
    fail(f"every one of the {len(H)} hostile commands is refused with an error line; {len(errs)} were:\n" + '\n'.join(errs[:70]))
after = state(rr)
for k in before:
    if json.dumps(before[k], sort_keys=True) != json.dumps(after[k], sort_keys=True):
        fail(f"refused commands changed '{k}': {json.dumps(before[k])[:300]} -> {json.dumps(after[k])[:300]}")
print(f"ok: {len(H)} hostile commands refused; look, sun angles, cameras, lights and backdrop unchanged")
print("PASS 12")
