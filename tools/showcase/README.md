# tools/showcase

`render_all.py` renders the showcase house (`docs/arch3d/showcase/bilevel.json`) in every view style. Python 3, standard library only.

```sh
python3 tools/showcase/render_all.py --out docs/arch3d/showcase/renders \
  [--size WxH] [--spp N] [--cli-json '["cargo","run","-q","-p","cadcraft-cli","--"]'] [--no-exports]
```

- `--size` (default `960x720`) and `--spp` (default `64`) apply to the 3D renders and the path-traced final.
- `--no-exports` skips the GLB/IFC/OBJ export steps and leaves `exports` empty. The `--cli-json`/`CADCRAFT_CLI` flow is unchanged.
- `--cli-json` is the command that runs `cadcraft-cli` (default: the cargo command); the `CADCRAFT_CLI` environment variable (a binary path) wins when set.
- The script sets `CADCRAFT_RENDER_ADAPTER=fallback` for every render step unless your environment already selects a renderer.
- The manifest records `scriptSha256` for the rendered `bilevel.json`, so the committed output matches today's script.

It replays the script once into `DIR/bilevel.dxf`, then writes from that file:

| Files | Kind |
|---|---|
| `plan-<level>.png` for every level | `plan` |
| `electrical-lower.png`, `electrical-main.png` | `electrical` |
| `elevation-<side>.png` (south, north, east, west) | `elevation` |
| `section-<name>.png` (a, b) | `section` |
| `camera-<name>.png` for the four saved cameras | `camera` |
| `night.png` (no sun, light set Evening, camera Front) | `night` |
| `path.png` (path-traced final) | `path` |
| `clay.png`, `technical.png` | `clay`, `technical` |
| `showcase.glb`, `showcase.ifc`, `showcase.obj` (unless `--no-exports`) | exports |
| `manifest.json`, `contact-sheet.png` | index, and a grid of every image at 320 px wide |

Technical illustration refused as "not available yet" is recorded as `technicalSkipped: true` in the manifest. Any other failure exits non-zero and names the step.

With `--out docs/arch3d/showcase/renders` it refreshes the committed images; the acceptance check writes to a temporary directory instead.
