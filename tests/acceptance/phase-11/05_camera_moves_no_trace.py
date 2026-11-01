# The phase-10 gate holds: thirty interactive camera steps start no path trace (and build no shadow map after the
# first frame); a Finalize starts exactly one; camera steps after it start none.
# needs: p11-r3d-pt-driver, p11-r3d-final-path, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

CAM = lambda az: {"kind": "orbit", "target": [2, 2, 1.0], "azimuth": az, "elevation": 30, "distance": 6, "fov": 60}
steps = [cmd('render', {"size": [64, 48], "camera": CAM(200 + k), "sun": False}) for k in range(31)]
fin = cmd('render', {"size": [64, 48], "camera": CAM(230), "sun": False, "final": "path", "spp": 4})
after = [cmd('render', {"size": [64, 48], "camera": CAM(240 + k), "sun": False}) for k in range(5)]
rc, rr, so, se = run(room_cmds() + [POTS] + steps + [fin] + after)
r = [x for x in rr if isinstance(x, dict) and 'pathTraces' in x]
if len(r) != 37:
    fail(f"37 render replies with pathTraces, got {len(r)}: {json.dumps(rr)[-600:]}")
moves = r[1:31]
if any(num(x['pathTraces'], 'pathTraces') != 0 or num(x.get('frameShadowMaps'), 'frameShadowMaps') != 0 for x in moves):
    fail(f"thirty camera steps start no path trace and build no map: {[(x['pathTraces'], x.get('frameShadowMaps')) for x in moves]}")
if r[31].get('finalMode') != 'path' or num(r[31]['pathTraces'], 'pathTraces') != 1:
    fail(f"the Finalize is one path trace: {r[31]}")
if any(num(x['pathTraces'], 'pathTraces') != 1 for x in r[32:]):
    fail(f"camera steps after the Finalize start none: {[x['pathTraces'] for x in r[32:]]}")
print("ok: 30 camera steps, 0 path traces; 1 Finalize, 1 path trace; 5 more steps, still 1")
print("PASS 05")
