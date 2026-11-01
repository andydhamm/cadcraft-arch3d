# A second Finalize in a new process takes its compiled pipelines from the disk cache: the first reports
# pipelineCache "miss" and writes pt-<adapter>.bin into CADCRAFT_PIPELINE_CACHE_DIR, the second "hit".
# SKIPs (passes) where the backend has no pipeline cache ("unsupported").
# needs: p11-r3d-pipeline-cache, p11-r3d-pt-driver, p11-cmd-render-path, p11-cli-render-path
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p11lib.inc')).read())

cache = tempfile.mkdtemp()
os.environ['CADCRAFT_PIPELINE_CACHE_DIR'] = cache
R = cmd('render', {"size": [32, 24], "camera": NIGHT_CAM, "sun": False, "final": "path", "spp": 1})
first = path_renders(run(room_cmds() + [POTS, R])[1])
if len(first) != 1:
    fail("one path-traced render reply in the first run")
state = first[0].get('pipelineCache')
if state == 'unsupported':
    print(f"SKIP: this backend has no pipeline cache ({first[0].get('device')})")
    print("PASS 06")
    sys.exit(0)
if state != 'miss':
    fail(f"the first Finalize with an empty cache directory misses: {state}")
files = [f for f in os.listdir(cache) if f.startswith('pt-') and f.endswith('.bin')]
if len(files) != 1 or os.path.getsize(os.path.join(cache, files[0])) == 0:
    fail(f"the first Finalize writes one pt-<adapter>.bin: {os.listdir(cache)}")
second = path_renders(run(room_cmds() + [POTS, R])[1])
if len(second) != 1 or second[0].get('pipelineCache') != 'hit':
    fail(f"the second Finalize, in a new process, hits the cache: {second[0].get('pipelineCache') if second else None}")
print(f"ok: miss then hit, {files[0]} ({os.path.getsize(os.path.join(cache, files[0]))} bytes)")
print("PASS 06")
