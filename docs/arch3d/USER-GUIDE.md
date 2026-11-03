# arch3d user guide

arch3d turns a plan into a building: walls, doors, windows, rooms and roofs live in the drawing, and the 3D
model, the elevations, the sections and the sheets are built from them. This guide walks through the 3D views
(phase 6) and the drawings made from the model (phase 7).

Every example below is a shell block you can paste into a terminal at the repository root. Each block is
self-contained: it builds the house it needs and writes its outputs (DXF, PNG, PDF) to the current directory.
The command-line form is always the same:

- `cargo run -q -p cadcraft-cli -- run --metric --cmd '<id> <json>' ... [--save out.dxf]` runs commands in a new
  metric drawing, one `--cmd` per command, in order, and prints each command's JSON reply on its own line;
- `cargo run -q -p cadcraft-cli -- render IN.dxf OUT.png [options]` renders the 3D model of a saved drawing;
- `cargo run -q -p cadcraft-cli -- convert IN.dxf OUT.glb` exports the 3D model as glTF.

Lengths are metres. Arch objects are referred to by id (`w0` is the first wall, `v0` the first view).

## The house

The examples use a 10 × 8 m house: one exterior wall loop 0.2 m thick and 2.4 m high, a hinged door in the
south wall, a sliding window in the north wall, the room found from the walls and a 6/12 gable roof.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'window {"wall":"w2","offset":4.0,"width":1.2,"height":1.2,"sill":0.9,"style":"slider"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --cmd 'arch.inspect {"walls":true,"openings":true,"rooms":true}' \
  --save house.dxf
```

## Sun position

`sun` computes the sun's azimuth and elevation for a latitude, a date and a time (`solar-noon` or a clock time
`HH:MM`). The examples use Calgary, 51.05° N. The renderer uses the same sun for shadows (`render --sun`).

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'sun {"lat":51.05,"date":"2026-12-21","time":"solar-noon"}' \
  --cmd 'sun {"lat":51.05,"date":"2026-06-21","time":"solar-noon"}' \
  --cmd 'sun {"lat":51.05,"date":"2026-06-21","time":"06:00"}'
```

## Saved cameras

`camera` saves named views in the drawing; they survive undo, redo and save. A `full` camera is an eye, a
target and a field of view; an `orbit` camera circles a target at an azimuth, an elevation and a distance.
`{"use":"name"}` makes a saved camera current, `{"list":true}` lists them and `{"delete":"name"}` removes one.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'camera {"save":"front","kind":"full","eye":[3,-8,1.6],"target":[3,0,1.05],"fov":60}' \
  --cmd 'camera {"save":"overview","kind":"orbit","azimuth":35,"elevation":30,"distance":25,"target":[5,4,1.2]}' \
  --cmd 'camera {"list":true}' \
  --cmd 'camera {"use":"front"}' \
  --cmd 'camera {"delete":"overview"}' \
  --cmd 'camera {"list":true}' \
  --save cameras.dxf
```

## Walkthrough

In the 3D pane, walkthrough mode moves the eye with W A S D and turns it with the mouse. The `walkthrough`
command is the same step as a calculator: from an eye and a heading (`yaw`, degrees, 0 = north, +Y) it moves
`forward` and `strafe` metres and turns `turn` degrees, and replies with the new eye and yaw. The eye keeps its
height.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'walkthrough {"eye":[5,-10,1.6],"yaw":0,"forward":2,"strafe":0,"turn":0}' \
  --cmd 'walkthrough {"eye":[5,-10,1.6],"yaw":0,"forward":0,"strafe":1,"turn":0}' \
  --cmd 'walkthrough {"eye":[5,-10,1.6],"yaw":0,"forward":0,"strafe":0,"turn":90}'
```

## Picking in 3D

`pick3d` casts a ray through one pixel of a camera's image and replies with the arch object it hits (its id and
handle, the same handle the 2D selection uses) and the hit point; a pixel on the sky hits nothing.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'pick3d {"camera":{"kind":"full","eye":[3,-8,1.6],"target":[3,0,1.05],"fov":60},"size":[480,320],"pixel":[240,160]}' \
  --cmd 'pick3d {"camera":{"kind":"full","eye":[3,-8,1.6],"target":[3,0,1.05],"fov":60},"size":[480,320],"pixel":[8,8]}'
```

## Rendering

`render` draws the 3D model of a saved drawing into a PNG:

- `--camera` takes `orbit,azimuth=A,elevation=E,distance=D[,target=x,y,z][,fov=F]` or
  `full,eye=x,y,z,target=x,y,z[,fov=F]`;
- `--style` is `shaded` (the default), `shaded-edges`, `hidden-line` or `glass-house`;
- `--sun lat=DEG,date=YYYY-MM-DD,time=solar-noon|HH:MM` lights the model from that sun and casts shadows;
- `--ground` draws a ground plane under the model that takes the shadows;
- `--adapter fallback` uses the software renderer, so it works on a machine without a GPU (`auto`, the default,
  prefers a GPU);
- `--size WxH` sets the image size in pixels;
- `--night` renders a night scene with no sun (fixture lights only);
- `--final` runs a Finalize Render: every light shadowed, soft shadow edges, tone mapping, higher resolution.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'window {"wall":"w2","offset":4.0,"width":1.2,"height":1.2,"sill":0.9,"style":"slider"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --save house.dxf
cargo run -q -p cadcraft-cli -- render house.dxf overview.png --size 640x400 \
  --camera 'orbit,azimuth=35,elevation=30,distance=25' --style shaded-edges --adapter fallback
cargo run -q -p cadcraft-cli -- render house.dxf winter.png --size 640x400 \
  --camera 'orbit,azimuth=0,elevation=89,distance=40,target=5,4,0' --ground \
  --sun 'lat=51.05,date=2026-12-21,time=solar-noon' --adapter fallback
cargo run -q -p cadcraft-cli -- render house.dxf front.png --size 640x400 \
  --camera 'full,eye=3,-8,1.6,target=3,0,1.05,fov=60' --style hidden-line --adapter fallback
```

## Elevations and sections

`elevation` and `section` make linked 2D views of the model, placed in model space at `insert`. They are
views, not copies: move a window and its views follow. `elevation {"side":"south"}` looks at the south face
(also `east`, `north`, `west`); `section` cuts along the line `from`–`to` and looks the way `look` points.
Views are numbered `v0`, `v1`, … in the order they are made. `arch.inspect {"views":true}` lists every view
with its geometry and its overall width and height dimensions, and `view.delete {"view":"v4"}` removes one.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --cmd 'elevation {"side":"south","insert":[0,-20]}' \
  --cmd 'elevation {"side":"east","insert":[16,-20]}' \
  --cmd 'elevation {"side":"north","insert":[0,-32]}' \
  --cmd 'elevation {"side":"west","insert":[16,-32]}' \
  --cmd 'section {"from":[-1,4],"to":[11,4],"look":[0,1],"insert":[0,-40]}' \
  --cmd 'arch.inspect {"views":true}' \
  --cmd 'view.delete {"view":"v4"}' \
  --cmd 'arch.inspect {"views":true}' \
  --save views.dxf
```

## Sheets and PDF

`viewsheet` places views on a paper-space layout, one viewport per view at the view's scale (1:50 unless the
view says otherwise, so one model metre is 20 paper mm), on an ARCH D sheet by default. The layout plots like
any other: `exportpdf {"layout":"Elevations","path":"elevations.pdf"}` writes it as a PDF. This is the worked
example: the four elevations of the house on an ARCH D landscape sheet at 1:50.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --cmd 'elevation {"side":"south","insert":[0,-20]}' \
  --cmd 'elevation {"side":"east","insert":[16,-20]}' \
  --cmd 'elevation {"side":"north","insert":[0,-32]}' \
  --cmd 'elevation {"side":"west","insert":[16,-32]}' \
  --cmd 'viewsheet {"name":"Elevations","views":["v0","v1","v2","v3"],"paper":"ARCH D"}' \
  --cmd 'exportpdf {"layout":"Elevations","path":"elevations.pdf"}' \
  --save sheet.dxf
```

## Detecting a plan

`archdetect` reads a plain DXF plan (walls as parallel face lines, doors from swing arcs or sliding leaves,
windows from glass lines between jambs, rooms named from their labels) and proposes arch objects. AIA/NCS layer
names raise the confidence, they are never required; `INSERT`s are read through; units come from `$INSUNITS` or
the `units` parameter.

A dry run (`{"dryRun":true}`) lists candidates with their confidence and evidence. Accepting candidates
(`{"accept":"all"}` or a list of ids) creates the walls, openings and rooms in one undo step. If the drawing is
not in metres, accepting converts it to metres in the same step.

```sh
cargo run -q -p cadcraft-cli -- run examples/apartment.dxf \
  --cmd 'archdetect {"dryRun":true}' \
  --cmd 'archdetect {"accept":"all"}' \
  --cmd 'arch.inspect {"rooms":true}' \
  --save apartment-3d.dxf
```

The reply for a dry run includes `walls`, `openings` and `rooms` arrays. Each candidate has an `id` (`c0`, `c1`,
...), a `confidence` (0.0–1.0), and `evidence` (layers, handles, rules). The `accept` reply maps candidate ids
to created arch ids (`w0`, `o0`, `r0`).

You can also convert a DXF directly to a GLB, which runs detection first if no arch objects are present:

```sh
cargo run -q -p cadcraft-cli -- convert examples/apartment.dxf apartment.glb
```

The **Detection Review** panel (Architecture > Detection Review) lets you inspect, check and accept candidates
interactively.

## Lighting and final renders

The lighting model is built from the same room model as the 3D view: fixtures sit on the ceiling, wall or floor,
with a 3D housing and a light source. The pane shadows only the nearest and brightest eight lights; camera moves
never rebuild shadow maps, and a fixture edit rebuilds the shadow maps only for the changed light. Finalize Render
uses a still from the current camera, renders every light with a high-resolution shadow map, soft shadow edges and
extra antialiasing, then tone-maps the frame before saving it.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[-0.1,-0.1],[4.1,-0.1],[4.1,4.1],[-0.1,4.1],[-0.1,-0.1]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'roomdetect {}' \
  --cmd 'lightfixture {"room":"r0","rows":2,"cols":2}' \
  --cmd 'lightfixture.types {}' \
  --cmd 'lightfixture.edit {"id":"lf3","height":2.3}' \
  --cmd 'illuminance {"point":[1,1,0]}' \
  --save house.dxf
cargo run -q -p cadcraft-cli -- render house.dxf night.png --night --final
```

The 4 × 4 m room has a 2.4 m ceiling and four default pot lights at (1,1), (1,3), (3,1) and (3,3), so the
reply to `illuminance {"point":[1,1,0]}` is about 165 lx for direct light from the nearest fixture. This is the
same control value as the interactive light test and is valid for a 60° cone with 800 lm at 3000 K.

The `render` command can also be used as a JSON command from the engine or as a file render from the CLI. For a
night scene the renderer uses the site and the current camera; `sunstudy` requires a saved site and renders one still
per hour from the current camera.

```text
render {"out":"night.png","size":[1200,900],"camera":{"kind":"orbit","target":[2,2,1.2],"azimuth":215,"elevation":26,"distance":10,"fov":60},"sun":false,"final":true}
sunstudy {"date":"2026-06-21","from":"08:00","to":"18:00","step":"1h","out":"study/house"}
```

The interactive pane exposes the same workflow as the headless renderer: the pane button, `render --final`, and
`render {"final":true}` all run a Finalize Render. A final still keeps every light shadowed, softens the
penumbra, tone-maps the scene and stays cached until the camera or the scene changes. This adds one more step
beyond the pane's live lighting: the sun, the fixtures and the shadow atlas are all still in the interactive view,
but the Finalize Render is a one-off, high-quality still.

## Path-traced Finalize

When a compute device is available, the Finalize Render switches to a path-traced solver that models real light
transport by Monte Carlo sampling. Each pixel traces many rays (samples per pixel, `spp`) and bounces up to a
maximum depth (`bounces`), with Russian roulette terminating chains. This gives natural **bounced light** (indirect
illumination from surfaces), a physically based **sun and sky** (100 000 lx sun, 2 500–5 000 nits sky), **glass**
(smooth dielectric, IOR 1.5, exact Fresnel), **glossy and mirror materials** (GGX metallic-roughness; a mirror
below a roughness of 0.02), **area lights** (a Lambertian emitting rectangle, peak lumens/π cd), and **IES files**
(photometric candela curves over vertical and horizontal angles, scaled to the fixture's lumens). A guided
a-trous **denoiser** (5 passes, guided by first-hit albedo and normal) cleans the result.

The raster path stays as a fallback: when compute shaders are absent (WebGL2, or `CADCRAFT_RENDER_LIMITS=webgl2`)
Finalize runs the raster pipeline and reports `"final":"raster"`.

### Render parameters

The `render` command (CLI flags or the engine JSON) accepts these Finalize parameters:

| Parameter   | Default       | Description                                                                 |
|-------------|---------------|-----------------------------------------------------------------------------|
| `final`     | `path`        | `path` (path-traced) or `raster` (raster pipeline)                          |
| `spp`       | 256           | samples per pixel (max 65536)                                               |
| `bounces`   | 4             | maximum light-bounce depth (max 32)                                         |
| `seed`      | 0             | Monte Carlo seed — same seed gives the same image on the same adapter        |
| `denoise`   | true          | enable the guided a-trous denoiser                                          |
| `rt`        | false         | use hardware ray queries when available (software fallback always works)    |
| `snapshots` | —             | list of sample counts at which to save an intermediate frame (max 8)        |

The reply keys are `final`, `spp`, `bounces`, `seed`, `denoised`, `rt`, `pipeline_cache`, `radiance`,
`path_traces`, and `snapshots`.

### Pipeline cache

Compiled shaders are cached per adapter and driver under `CADCRAFT_PIPELINE_CACHE_DIR` (default: the XDG cache
directory). Re-rendering the same scene on the same adapter does not recompile.

### Area lights and IES files

The catalogue includes an **LED panel 600** (a 0.6 × 0.6 m Lambertian area light, 3600 lm, 4000 K) placed on the
ceiling. Place it in a room with `lightfixture {"room":"r0","rows":1,"cols":1,"type":"LED panel 600"}`.
An IES photometric file (vertical and horizontal candela curves, lumens) describes a fixture's angular distribution;
attach one with `lightfixture.edit {"id":"lf0","ies":"file.ies"}`.

### Path-traced illuminance

`illuminance {"point":[x,y,z],"method":"path"}` returns the path-traced illuminance in lux at a point, computed
with the same units and physics as the direct method but including bounced light. The direct method
(`"method":"direct"`, the default) uses shadow rays only.

### Exposure

When the sun is on, the path tracer uses `render {"exposure": "auto"}` by default, which adjusts the exposure
based on the scene's luminance to prevent blown-out highlights or crushed shadows. You can override this with
a fixed exposure value.

### CLI flags

```
cargo run -q -p cadcraft-cli -- render IN.dxf OUT.png --final path --spp 1024 --bounces 8 --seed 42 --no-denoise --rt
cargo run -q -p cadcraft-cli -- render IN.dxf OUT.png --final raster
```

The path-traced commands and render calls below need a compute device (GPU or a software compute adapter). They are
shown in a `text` block for reference and are not executed by the engine tests.

```text
render house.dxf path.png --final path --spp 1024 --bounces 8 --seed 42 --no-denoise --rt
render house.dxf raster.png --final raster --spp 256 --bounces 4
illuminance {"point":[2,2,0],"method":"path"}
```

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[-0.1,-0.1],[4.1,-0.1],[4.1,4.1],[-0.1,4.1],[-0.1,-0.1]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'roomdetect {}' \
  --cmd 'lightfixture {"room":"r0","rows":2,"cols":1}' \
  --cmd 'lightfixture {"room":"r0","rows":1,"cols":1,"type":"LED panel 600"}' \
  --cmd 'illuminance {"point":[2,2,0]}' \
  --save path-demo.dxf
```

## Exporting IFC, OBJ and glTF

arch3d exports the building model to three interoperable formats: IFC (Industry Foundation Classes), OBJ (Wavefront)
and glTF (GL Transmission Format). The format is chosen by the file extension.

- `cadcraft-cli convert house.dxf house.ifc` writes an IFC4 file.
- `cadcraft-cli convert house.dxf house.obj` writes an OBJ file and a `house.mtl` material library beside it.
- `cadcraft-cli convert house.dxf house.glb` writes a binary glTF file.
- `cadcraft-cli run --cmd '...' --export house.ifc` exports the current session's drawing.

The `export` command is also available via MCP or the engine API:
- `export {"path": "house.ifc"}` writes the file.
- `export {"format": "glb"}` returns the file contents as base64-encoded data in the reply.

### IFC4 contents

The IFC export follows IFC4 (ISO 16739-1). It includes:
- **Spatial structure**: `IfcProject`, `IfcSite` (with latitude, longitude, and elevation from the site location),
  `IfcBuilding`, and one `IfcBuildingStorey` per level, placed at its elevation.
- **Elements**: Walls, doors and windows (in their host walls), slabs, roof planes (in their roof), stairs, railings,
  spaces (rooms with their areas), terrain, plants, and light fixtures.
- **Geometry**: Each body is an `IfcTriangulatedFaceSet` derived from the 3D view's meshes. Voids (like door openings)
  are represented as boxes subtracted from the host wall. Spaces are prisms from floor to ceiling.
- **Determinism**: GlobalIds are hashed from stable keys (e.g., `wall/3`), so two exports of the same model are
  byte-identical if `SOURCE_DATE_EPOCH` is set. OwnerHistory is omitted.

### Checking exports

Validators are external tools:
- **IFC**: Use [IfcOpenShell](https://ifcopenshell.org/) (Python library) to validate and inspect IFC files.
- **glTF**: Use the [Khronos glTF Validator](https://github.khronos.org/glTF-Validator/) to check glTF files.

### Example

This example builds a simple house, exports it to IFC, OBJ, and glTF, and verifies the export command works.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'window {"wall":"w2","offset":4.0,"width":1.2,"height":1.2,"sill":0.9,"style":"slider"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --cmd 'export {"path":"house.ifc"}' \
  --cmd 'export {"path":"house.obj"}' \
  --cmd 'export {"path":"house.glb"}' \
  --cmd 'export {"format":"glb"}' \
  --save house-export.dxf
```

## Materials

The `material` command assigns finishes to exterior walls, roofs, and room floors. These finishes affect the 3D
appearance and are exported with embedded textures in GLB files. The path tracer uses each texture's mean colour
for lighting calculations.

- **Walls**: `siding` or `brick`. Applied to the exterior face of the wall.
- **Roof**: `shingles` or `wood`. Applied to the roof planes.
- **Floors**: `wood`. Applied to the room's floor surface.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'door {"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"}' \
  --cmd 'window {"wall":"w2","offset":4.0,"width":1.2,"height":1.2,"sill":0.9,"style":"slider"}' \
  --cmd 'roomdetect {}' \
  --cmd 'roofauto {"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]}' \
  --cmd 'material {"walls":["w0"],"finish":"siding"}' \
  --cmd 'material {"rooms":["r0"],"finish":"wood"}' \
  --cmd 'material {"roofs":["r0"],"finish":"shingles"}' \
  --cmd 'export {"path":"materials.glb"}' \
  --save materials.dxf
cargo run -q -p cadcraft-cli -- render materials.dxf materials.png --adapter fallback
```

## Cabinets and fixtures

`cabinet` and `fixture` commands place kitchen and bathroom elements. Cabinets are placed with their back against
a wall (by offset or point/rotation) and include carcass, doors, and counters. Fixtures include sinks, toilets,
bathtubs, ranges, refrigerators, and dishwashers.

- **Cabinets**: `base`, `wall`, `tall`. Base cabinets have a counter at 0.914 m.
- **Fixtures**: `sink`, `lavatory`, `toilet`, `bathtub`, `shower`, `range`, `refrigerator`, `dishwasher`.
- **Plan layers**: `A-FLOR-CASE` (base cabinets), `A-FLOR-CASE-UPPR` (wall cabinets), `P-FLOR-FIXT` (plumbing fixtures),
  `A-FLOR-APPL` (appliances).

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[6,0],[6,4],[0,4],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}' \
  --cmd 'roomdetect {}' \
  --cmd 'cabinet {"type":"base","wall":"w0","offset":1.0,"width":1.5}' \
  --cmd 'cabinet {"type":"base","wall":"w0","offset":2.5,"width":1.5}' \
  --cmd 'cabinet {"type":"wall","wall":"w0","offset":1.0,"width":1.5}' \
  --cmd 'fixture {"type":"sink","wall":"w0","offset":1.75}' \
  --cmd 'fixture {"type":"range","wall":"w0","offset":3.25}' \
  --cmd 'fixture {"type":"refrigerator","wall":"w1","offset":1.0}' \
  --cmd 'fixture {"type":"dishwasher","wall":"w0","offset":4.25}' \
  --cmd 'arch.inspect {"cabinets":true,"fixtures":true}' \
  --save kitchen.dxf
```

## Large models

`arch.sample {"house": true}` generates a benchmark house with two levels, 300 walls, 150 openings, rooms, and a hip
roof. `build3d {}` builds the full 3D scene. `build3d {"incremental": true}` rebuilds only changed objects, reporting
`rebuilt` (count of objects rebuilt), `objects` (total objects), and `rebuildMs` (time taken). This is useful for
checking performance on large models.

```sh
cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'arch.sample {"house":true}' \
  --cmd 'build3d {}' \
  --cmd 'wall.stretch {"wall":"w10","end":"start","point":[5,5]}' \
  --cmd 'build3d {"incremental":true}' \
  --save large-model.dxf
```

## Out of scope

Not part of arch3d so far: SSAO, TAA, cascaded shadows, switches, outlets, circuits, animated walkthrough paths,
and gamepad or 3D-mouse input. Moldings, material regions, plan check, materials list and costing, and the contextual
toolbar are also out of scope.
