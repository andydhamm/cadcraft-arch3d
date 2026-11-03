# MCP examples for agents: every example in docs/arch3d/mcp-examples/ (*.jsonl, and any *.json) replays through
# `cadcraft-cli mcp` from an empty directory: one compact JSON-RPC request per line, each with an id, initialize first and
# a metric new_drawing second; every request gets exactly one reply with a result and no error, and no tool result says
# isError. Together they show walls and openings, rooms and roofs, materials, a kitchen, build3d, a render, exports, the
# site and lights, and ARCHDETECT.
# needs: p13-mcp-examples
import os, glob
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())
ROOT = os.getcwd()
DIR = os.path.join(ROOT, 'docs', 'arch3d', 'mcp-examples')

files = sorted(glob.glob(os.path.join(DIR, '*.jsonl')) + glob.glob(os.path.join(DIR, '*.json')))
if len(files) < 8 or not os.path.exists(os.path.join(DIR, 'README.md')):
    fail(f"docs/arch3d/mcp-examples/ holds a README.md and at least 8 examples: {[os.path.basename(f) for f in files]}")
used_tools, used_cmds, texts = set(), set(), []
for path in files:
    name = os.path.basename(path)
    lines = [l for l in open(path, encoding='utf-8').read().splitlines() if l.strip()]
    reqs = []
    for k, l in enumerate(lines):
        try:
            r = json.loads(l)
        except ValueError:
            fail(f"{name} line {k + 1} is one compact JSON request: {l[:120]}")
        if not isinstance(r, dict) or r.get('jsonrpc') != '2.0' or 'id' not in r or not isinstance(r.get('method'), str):
            fail(f"{name} line {k + 1} is a JSON-RPC 2.0 request with an id: {l[:160]}")
        reqs.append(r)
    if len(reqs) < 3 or reqs[0]['method'] != 'initialize':
        fail(f"{name} starts with initialize")
    nd = reqs[1]
    if nd['method'] != 'tools/call' or (nd.get('params') or {}).get('name') != 'new_drawing' or \
            ((nd.get('params') or {}).get('arguments') or {}).get('metric') is not True:
        fail(f"{name} line 2 is tools/call new_drawing {{metric: true}}: {lines[1][:160]}")
    for r in reqs:
        if r['method'] == 'tools/call':
            p = r.get('params') or {}
            used_tools.add(p.get('name'))
            a = p.get('arguments') or {}
            if p.get('name') == 'execute' and isinstance(a.get('command'), str):
                used_cmds.add(a['command'])
            if p.get('name') in ('command_line', 'script') and isinstance(a.get('text'), str):
                used_cmds.update(w.split()[0].lower() for w in a['text'].splitlines() if w.strip())
    tmp = tempfile.mkdtemp()
    os.symlink(os.path.join(ROOT, 'examples'), os.path.join(tmp, 'examples'))
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    # From the empty directory cargo needs the workspace's manifest (as 09 does), else 'could not find Cargo.toml'.
    exe = os.environ.get('CADCRAFT_CLI')
    args = [exe, 'mcp'] if exe else ['cargo', 'run', '-q', '--manifest-path', os.path.join(ROOT, 'Cargo.toml'), '-p',
                                     'cadcraft-cli', '--', 'mcp']
    p = subprocess.run(args, input='\n'.join(lines) + '\n', cwd=tmp, capture_output=True, text=True,
                       timeout=1800, env=env)
    if 'panicked' in p.stdout + p.stderr:
        fail(f"{name}: panic: {(p.stderr + p.stdout)[-600:]}")
    replies = {}
    for l in p.stdout.splitlines():
        try:
            r = json.loads(l)
        except ValueError:
            continue
        for x in (r if isinstance(r, list) else [r]):
            if isinstance(x, dict) and 'id' in x:
                if x['id'] in replies:
                    fail(f"{name}: two replies to id {x['id']}")
                replies[x['id']] = x
    for r in reqs:
        x = replies.get(r['id'])
        if x is None:
            fail(f"{name}: no reply to id {r['id']} ({r['method']}); stderr: {p.stderr[-300:]}")
        if 'error' in x or 'result' not in x:
            fail(f"{name}: request {r['id']} ({json.dumps(r)[:160]}) failed: {json.dumps(x)[:400]}")
        if r['method'] == 'tools/call' and (x['result'] or {}).get('isError') is True:
            fail(f"{name}: tool call {r['id']} ({json.dumps(r)[:200]}) reports isError: {json.dumps(x['result'])[:400]}")
    print(f"ok: {name}: {len(reqs)} requests replayed")
for need in ('wall', 'door', 'window', 'roomdetect', 'roofauto', 'material', 'cabinet', 'fixture', 'build3d', 'export',
             'lightfixture', 'archdetect'):
    if need not in used_cmds:
        fail(f"the examples run {need} (through execute or command_line); they run {sorted(used_cmds)}")
if 'render' not in used_tools:
    fail(f"an example renders with the render tool; tools used: {sorted(t for t in used_tools if t)}")
print(f"ok: {len(files)} examples, {len(used_cmds)} commands, tools {sorted(t for t in used_tools if t)}")
print("PASS 10")
