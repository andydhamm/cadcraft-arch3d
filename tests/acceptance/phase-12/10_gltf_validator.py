# glTF: the demo house's GLB has the terrain and plant nodes and its lights as KHR_lights_punctual (one per fixture that is
# on), and the Khronos glTF validator (the gltf-validator npm package, outside the codebase) reports no error. That part
# SKIPs where node or the package is missing; it looks in $XDG_CACHE_HOME/cadcraft-checks (npm install --prefix there).
# needs: p12-gltf-valid
import os, shutil
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p12lib.inc')).read())

tmp = tempfile.mkdtemp(); glbp = os.path.join(tmp, 'house.glb')
rc, rr, so, se = export(HOUSE + [INSPECT], glbp)
m = model_counts(rr)
j, binc = glb(glbp)
names = {n.get('name') for n in j.get('nodes', [])}
for want in ('terrain', 'plant-pl0', 'plant-pl1'):
    if want not in names:
        fail(f"GLB node {want} missing: {sorted(x for x in names if x)[:40]}")
on = sum(1 for l in m['raw']['lights'] if l.get('on', True))
lights = j.get('extensions', {}).get('KHR_lights_punctual', {}).get('lights', [])
if 'KHR_lights_punctual' not in j.get('extensionsUsed', []) or len(lights) != on:
    fail(f"the {on} fixtures that are on are KHR_lights_punctual lights: {len(lights)}, extensionsUsed {j.get('extensionsUsed')}")
print(f"ok: terrain, 2 plants, {len(lights)} punctual lights")
node = shutil.which('node')
pkg = os.path.join(CHECK_TOOLS, 'node_modules', 'gltf-validator')
if not node or not os.path.isdir(pkg):
    print(f"SKIP: the glTF validator is not installed (node {node}, {pkg}); the GLB checks above passed")
    print("PASS 10"); sys.exit(0)
js = ("const v=require(process.argv[1]);const fs=require('fs');"
      "v.validateBytes(new Uint8Array(fs.readFileSync(process.argv[2]))).then(r=>{console.log(JSON.stringify(r));})"
      ".catch(e=>{console.error(String(e));process.exit(2);});")
p = subprocess.run([node, '-e', js, pkg, glbp], capture_output=True, text=True, timeout=300)
if p.returncode != 0:
    fail(f"the glTF validator did not run: {p.stderr[-400:]}")
rep = json.loads(p.stdout)
iss = rep.get('issues', {})
errs = [x for x in iss.get('messages', []) if x.get('severity') == 0]
print(f"gltf-validator {rep.get('validatorVersion')}: {iss.get('numErrors')} errors, {iss.get('numWarnings')} warnings, {iss.get('numInfos')} infos")
for x in errs[:8]:
    print(f"  {x.get('code')}: {x.get('message')} at {x.get('pointer')}")
if iss.get('numErrors', 1) != 0:
    fail(f"the glTF validator reports {iss.get('numErrors')} error(s): {[x.get('code') for x in errs][:10]}")
print("PASS 10")
