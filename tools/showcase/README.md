# tools/showcase

`render_all.py` renders the showcase house (`docs/arch3d/showcase/bilevel.json`) and its drawing set (`phase-20-showcase.json`, `phase-21-showcase.json`) in every view style. Python 3, standard library only.

To refresh the committed gallery, the operator runs one command from the repository root (drop `CADCRAFT_RENDER_ADAPTER` from the environment, or set it to a GPU adapter, to render on the GPU):

```sh
python3 tools/showcase/render_all.py --full --out docs/arch3d/showcase/renders
```

Other options:

```sh
python3 tools/showcase/render_all.py --out DIR \
  [--size WxH] [--spp N] [--cli-json '["cargo","run","-q","-p","cadcraft-cli","--"]'] [--no-exports] [--full] [--no-set]
```

- `--size` (default `960x720`) and `--spp` (default `64`) apply to the 3D renders, the path-traced final and the `noon-hdri` look camera.
- `--no-exports` skips the GLB/IFC/OBJ export steps and leaves `exports` empty.
- `--full` renders the full gallery: the look cameras (dusk, noon-hdri, overview), every plan view, the interior elevations, the sheet images and the drawing-set PDF. A plain run renders none of these: it is the set phase-15 check 11 has always run, in about the same time.
- `--no-set` (with `--full`) skips the drawing set after all: no `set.dxf`, plan views, interior elevations, sheets or PDF, and `drawingSet` is `null`. The look cameras still render from the house file.
- `--cli-json` is the command that runs `cadcraft-cli` (default: the cargo command); the `CADCRAFT_CLI` environment variable (a binary path) wins when set.
- The script sets `CADCRAFT_RENDER_ADAPTER=fallback` for every render step unless your environment already selects a renderer.

It replays `bilevel.json` into `DIR/bilevel.dxf`, writes the house images from it, then (with `--full`, unless `--no-set`) replays `bilevel.json`, `phase-20-showcase.json` and `phase-21-showcase.json` in one run into `DIR/set.dxf` and writes the rest from that file. The step list is `plan(levels, views, sheets, with_set)`, a pure function with its own tests: `python3 -m unittest tools/showcase/test_render_all.py`.

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
| `camera-dusk.png`, `camera-noon-hdri.png`, `camera-overview.png` (the saved look cameras) | `look` |
| `planview-<level>-floor.png` for every floor or landing level | `planview` |
| `planview-<level>-rcp.png` (reflected ceiling) | `rcp` |
| `planview-roof.png`, `planview-site.png` | `roofplan`, `siteplan` |
| `interior-<view name>.png` for every interior elevation | `interior` |
| `sheet-<sheet number>.png` for every sheet, in sheet order (`plot` of the sheet's layout to PNG) | `sheet` |
| `drawing-set.pdf` (`plot {"sheets": "all"}`) | `drawingSet` in the manifest, not an image |
| `set.dxf` | the replayed drawing set (input to the steps above) |
| `manifest.json`, `contact-sheet.png` | index, and a grid of every image at 320 px wide |

The electrical plan views are the `electrical-*.png` images; no second set is written. A look camera the drawing does not have fails the step naming it.

Manifest keys: `images` (`file`, `kind`, `view`), `exports`, `contactSheet`, `technicalSkipped`, `scriptSha256` (of `bilevel.json`), `setScripts` (SHA-256 of `phase-20-showcase.json` and `phase-21-showcase.json`; empty with `--no-set`), `drawingSet` (`"drawing-set.pdf"` or `null`) and `sheetImages` (`true` when a PNG is written per sheet).

Technical illustration refused as "not available yet" is recorded as `technicalSkipped: true`. Any other failure exits non-zero and names the step.

The acceptance check writes to a temporary directory instead of the committed one.
