# Docs: every fenced ```sh example in docs/arch3d/USER-GUIDE.md runs as written (in order, in one empty directory with
# examples/ linked, under `set -eo pipefail`; no failure, panic or `error` line), and the guide covers this phase:
# materials, cabinets and fixtures, large models (the benchmark house and the incremental build), MCP examples.
# needs: p13-docs-user-guide, p13-mcp-examples
import os, re
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p13lib.inc')).read())
ROOT = os.getcwd()

guide = open('docs/arch3d/USER-GUIDE.md', encoding='utf-8').read()
heads = [h.strip().lower() for h in re.findall(r'^##\s+(.+)$', guide, re.M)]
for need in ('materials', 'cabinets and fixtures', 'large models', 'mcp examples for agents'):
    if not any(h.startswith(need) for h in heads):
        fail(f"USER-GUIDE.md has a '## {need[0].upper() + need[1:]}' section: headings {heads}")
blocks = re.findall(r'```sh\n(.*?)```', guide, re.S)
joined = '\n'.join(blocks)
for need, what in (("'material ", 'material'), ("'cabinet ", 'cabinet'), ("'fixture ", 'fixture'),
                   ('arch.sample {"house":true}', 'arch.sample {"house":true}'), ('"incremental":true', 'build3d {"incremental":true}')):
    if need not in joined.replace('": ', '":').replace(', "', ',"'):
        fail(f"USER-GUIDE.md has a runnable ```sh example using {what}")
if 'docs/arch3d/mcp-examples' not in guide and 'mcp-examples/' not in guide:
    fail("USER-GUIDE.md points at docs/arch3d/mcp-examples/")
oos = guide.lower().split('## out of scope', 1)
if len(oos) == 2 and re.search(r'cabinet|furniture', oos[1].split('\n## ', 1)[0]):
    fail("cabinets are built now: drop them from the guide's out-of-scope list")

tmp = tempfile.mkdtemp()
os.symlink(os.path.join(ROOT, 'examples'), os.path.join(tmp, 'examples'))
exe = os.environ.get('CADCRAFT_CLI')
mf = os.path.join(ROOT, 'Cargo.toml')
ran = 0
for i, b in enumerate(blocks):
    if 'cadcraft-cli' not in b:
        continue
    script = b
    for pre in ('cargo run -q --release -p cadcraft-cli --', 'cargo run --release -q -p cadcraft-cli --',
                'cargo run -q -p cadcraft-cli --', 'cargo run -p cadcraft-cli --'):
        rel = '--release' in pre
        script = script.replace(pre, exe if exe else f'cargo run -q {"--release " if rel else ""}--manifest-path {mf} -p cadcraft-cli --')
    env = dict(os.environ)
    env.setdefault('CADCRAFT_RENDER_ADAPTER', 'fallback')
    p = subprocess.run(['bash', '-c', 'set -eo pipefail\n' + script], cwd=tmp, capture_output=True, text=True, timeout=1800,
                       env=env)
    out = p.stdout + p.stderr
    if p.returncode != 0 or 'panicked' in out or re.search(r'^error', out, re.M):
        fail(f"USER-GUIDE.md sh block {i + 1} does not run as written (exit {p.returncode}):\n{b.strip()[:600]}\n--- output ---\n{out[-800:]}")
    ran += 1
for f in sorted(os.listdir(tmp)):
    if f.endswith('.png'):
        artifact(os.path.join(tmp, f), 'guide-' + f)
print(f"ok: USER-GUIDE.md: {ran} sh blocks run; sections {', '.join(h for h in heads if h.startswith(('materials', 'cabinets', 'large', 'mcp')))}")
print("PASS 09")
