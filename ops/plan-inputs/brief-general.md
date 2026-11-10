# Task: give CADCraft "draw a plan, get the house in 3D" — Chief Architect–style automatic 3D for floor plans

You work in a fork of CADCraft. Go through the phases in order.

## The goal

Chief Architect's core trick is that you never model in 3D. You draw a 2D plan with *architectural objects*, and the program builds the 3D building automatically:

- walls with real thickness and height
- doors and windows that cut their own openings
- rooms detected from enclosed walls, with floors and ceilings
- roofs generated from the walls and their pitches
- stairs from a start and end point
- cabinets, railings, decks, terrain
- 3D views, elevations and sections that update live as you edit the plan

**Bring that workflow to CADCraft as a first-class feature:** an architectural object layer on top of its 2D drafting, an automatic 3D model builder, a 3D view, and generated elevations and sections. It must also work on ordinary DXF floor plans someone else drew, by recognising walls and openings in plain linework.

This is **not** about reading Chief Architect files. (That's a separate effort, see `~/chief-plan-import-prompt.md`.)

## Ground rules

- **Read the project rules first.** That means CADCraft's `CLAUDE.md`, `AGENTS.md`, `ATTRIBUTION.md`, `ROADMAP.md`, `docs/roadmap.md` and `docs/gaps.md`. Follow its code style, test conventions and command/MCP patterns.
- **Clean room.** CADCraft describes itself as clean-room. Implement from first principles, from public documentation of *behaviour*, and from open standards. Fair sources:
  - Chief's and other tools' user manuals and videos;
  - IFC (ISO 16739);
  - the AIA/NCS CAD layer standard;
  - standard architecture references (stair rules, roof geometry).
- **Never decompile, disassemble or copy code or assets from Chief Architect, Home Designer, Revit, ArchiCAD, SketchUp or AutoCAD.**
- **Open reference projects** (Andy, 2026-10-25). These feed the assisted pre-drafts made outside the swarm, not the
  swarm itself. The swarm has no web access, and reading these projects would be wasted prefill, so don't plan segments
  that need them. Whatever matters from one arrives distilled: in an assisted building block and its line under "Building
  blocks added outside the swarm" in `DESIGN.md`. Licences: code may come only from the MIT ones, with credit in
  `ATTRIBUTION.md`; the others are read for behaviour, never copied or translated line by line.
  - **Sweet Home 3D** (GPL-2.0, Java): walls, rooms, furniture catalogues, the 3D view and virtual visitor, light sources,
    and photo rendering (phases 1–3, 6, 10, 11). Its photo renderer is **SunFlow** (MIT), a readable reference for the
    phase-11 path tracer.
  - **FreeCAD's BIM workbench** (LGPL-2.1): walls, windows, roofs with per-edge angles, stairs, section planes and their 2D
    views, the Site object, and IFC through IfcOpenShell (phases 1–2, 4, 5, 7, 9, 12).
  - **Archipack** (a Blender add-on, GPL): parametric walls, doors, windows, roofs, stairs, fences and floors (phases 2, 4, 5).
  - **Ladybug Tools** (AGPL-3.0): sun path, sunlight hours, radiation and shadow studies (phases 6, 9–11). It can also
    run outside the codebase as an extra check of our sun numbers, like IfcOpenShell, and is skipped when missing.
  - **DesignExplorer** (MIT, web): browsing many design options by their results. It's an idea for later, not a phase.
- **Use REA (`npx rea-agents setup`) only on CADCraft itself and on open-source projects**, for example to understand how an open BIM tool structures its wall-join or roof code before writing ours.
- **Dependencies:** pure Rust, licence-compatible only (MIT, Apache-2.0, BSD, Zlib), and the web build must keep working (WebGPU, falling back to WebGL2). No C++ geometry kernels.
- **Upstream mindset.** Design it so it could be proposed upstream as a crate plus commands: small PRs, tests, docs, MCP coverage.
- **Translations.** `ui-egui`'s test `translations_cover_commands_menus_tools_and_snap_labels` requires a Ukrainian row (`crates/ui-egui/src/i18n/uk.tsv`) for every command label, menu path, UI command and tool label; other languages fall back to English. A segment that adds or renames any of these must list `uk.tsv` in its files and add the rows. A worker's gate tests only its own crates, so without that the failure first shows at merge.
- **Shared structs are built with `..Default::default()` or a constructor, never an exhaustive struct literal** outside the module that defines them (`Scene3d`, `Wall`, `Level`, `Opening` and the like). Segments run in parallel, and a literal that lists every field stops compiling the moment another segment merges a new one (10-23/25: most merge rejections were `E0063: missing field … in initializer of Scene3d / Wall`). A segment that adds a field to a shared struct gives it a default.

## Phase 0: survey

1. Fork and clone CADCraft: `gh repo fork storytold/cadcraft --clone`, into `~/src/cadcraft`, on branch `arch3d`. Build and run the tests. CADCraft 0.10.0 is installed system-wide; don't touch that install.
2. Map the architecture. The crates are `engine`, `geom`, `doc`, `dxf`, `dwg`, `render`, `ui-egui`, `mcp`, `io`, `constraints`, `color`, `fonts`, and the apps are `cadcraft`, `cadcraft-cli` and `cadcraft-web`. Answer:
   - How are entities, commands, undo, selection, grips and object snaps modelled?
   - How does `render` draw: wgpu or egui painter? Is there any 3D, camera or depth buffer?
   - How do DXF custom data (XDATA, dictionaries, custom objects) round-trip?
   - How do MCP tools map to commands?
   The roadmap puts "3D modelling and rendering" at about 2 %, so expect to build the 3D view.
3. Read what's planned for 3D in `docs/gaps.md`, so this work fits that direction rather than fighting it.

⛔ **Checkpoint:** an architecture summary and a proposed crate layout (below) adjusted to what you found.

## Target architecture (adjust after phase 0)

- **`crates/arch` — the architectural object model.** Plain data plus 2D behaviour, no rendering.
  - **`Wall`:** a centre-line segment or arc, plus:
    - thickness and a layered wall type (e.g. siding / sheathing / stud / drywall), with a reference side;
    - base level and height (or "to ceiling"), and a top that can follow the roof;
    - flags for exterior, interior, railing, retaining or knee wall.
  - **Joins:** automatic L, T, X and angled joins with mitred, butted or cleaned-up layers. A *wall graph* lets edits propagate (drag a wall and its neighbours stretch).
  - **`Opening`** (door, window, garage door, cased opening, pass-through):
    - host wall, offset along it, width, height, sill or header height;
    - a style (hinged, sliding, bi-fold, casement, slider, picture, bay/box), swing side, frame and casing;
    - the 2D plan symbol is generated, not drawn.
  - **`Room`:** auto-detected from enclosed wall faces, including faces made by invisible "room divider" walls. Has a name and type, floor and ceiling heights and finishes, and area and perimeter (standard, interior and exterior).
  - **`Level`/`Floor`:** elevation, floor-to-floor and ceiling heights, floor structure thickness. Multiple floors plus a basement.
  - **`Roof`:**
    - per-wall roof settings: pitch, overhang, gable or hip, full or partial gable, eave height;
    - an **automatic roof generator** (straight skeleton) builds roof planes with ridges, hips, valleys and gable ends;
    - roof planes can also be drawn or edited manually.
    Also dormers (later), skylights, fascia and gutters.
  - **`Stair`:** start point, direction, rise and run computed from the floor-to-floor height and code limits (configurable max riser and min tread), straight, L, U or winder, with a landing; plus railings.
  - **Other objects:** `Railing`, `Deck` (joists and decking), `Fence`, `Cabinet` (base, wall and tall, from a parametric box with doors and drawers), `Fixture` (simple parametric symbols), `Terrain` (phase 9, from points, contours and a site boundary), `Plant` (phase 9), `LightFixture` (phase 10).
  - **`Material`:** name, colour and PBR parameters, with an optional texture reference that is our own procedural one or user-supplied.
- **`crates/arch3d` — the automatic 3D builder.** `fn build(model: &ArchModel) -> Scene3d`. Deterministic, incremental (rebuild only what an edit touched), and pure Rust.
  - **Walls:** extrude the 2D wall polygons from the join solver. Openings are cut by 2D boolean of the wall elevation (wall face as a 2D region minus opening rectangles, then extrude). Prefer that to general 3D CSG, which is faster and robust.
  - **Openings:** frame, glazing, sash, door leaf, casing, sill.
  - **Floors and ceilings:** from room polygons, with stair-well holes.
  - **Roofs:** planar polygons from the generator, with thickness, fascia, soffit and overhang. Clip wall tops to the roof (gable walls rise to the rafters).
  - **Stairs:** treads, risers, stringers and rails. Also decks, fences and cabinets.
  - **Output:** a mesh scene with per-object IDs back to the 2D entities (for picking), materials, and normals/UVs (world-space box mapping for siding and brick).
  - **Exports:** glTF 2.0 (`.glb`), OBJ, and later IFC4 (via an own STEP writer for `IfcWall`, `IfcWindow`, `IfcSlab`, `IfcRoof`, `IfcStair`, `IfcSpace`).
- **`crates/render3d` (or extend `render`) — a 3D viewport on wgpu.** Works in native and web builds.
  - **Cameras:**
    - orbit and overview;
    - perspective "full camera" placed in plan (eye height, target, field of view), with a walkthrough mode (WASD) and saved cameras;
    - orthographic elevation and section cameras.
  - **Shading:** PBR, a directional sun from date, time and latitude (shadows with cascaded shadow maps), SSAO, MSAA or TAA, outlines and edges, and display styles (shaded, shaded with edges, hidden-line, glass-house/x-ray). Optional later: a progressive path tracer for "final views".
  - **Picking** from 3D back to the 2D object, with selection kept in sync between the panes.
- **Generated 2D views** (Chief's "auto elevations, sections and plans"):
  - elevations and cross-sections from the 3D scene, using **hidden-line removal** on our own meshes, then 2D vectors placed in a layout or model space as live, linked views;
  - roof plans and framing plans later.
- **Recognition (wall detection) for plain DXF:** turn a drafted floor plan into `arch` objects. That means detecting parallel line pairs as walls (thickness from the spacing), gaps with arcs as door swings, short parallel lines in wall gaps as windows, closed regions as rooms, and text inside as room names. Use AIA/NCS layer names (`A-WALL`, `A-DOOR`, `A-GLAZ`) as strong hints. Interactive: show what was found and let the user accept or fix it.
- **Persistence:** arch objects round-trip in DXF as custom objects or XDATA attached to proxy-graphic entities, so the plan still opens as ordinary 2D linework in other programs.

## Commands, MCP and CLI

Add commands in CADCraft's style, each an MCP tool and usable from `cadcraft-cli run --cmd`:

- **Drawing objects:**
  - `WALL` (thickness, type, height; draw continuous; with ortho, polar and snaps)
  - `DOOR`, `WINDOW`, `GARAGEDOOR` (click a wall to place; grips slide them along it)
  - `ROOM` (with `ROOMDETECT`), `LEVEL`, `ROOFAUTO`, `ROOFPLANE`, `STAIR`, `RAILING`, `DECK`, `CABINET`
- **Converting and viewing:**
  - `ARCHDETECT` (DXF linework to objects)
  - `VIEW3D` (open or focus the 3D pane), `CAMERA`, `WALKTHROUGH`, `PICK3D`, `SUN`
  - `ELEVATION` / `SECTION` (generate a linked view)
  - `EXPORT3D` (`.glb`, `.obj`, `.ifc`)
- **CLI:**
  - `cadcraft-cli convert plan.dxf out.glb` builds 3D from a plan (running `ARCHDETECT` first if there are no arch objects);
  - `cadcraft-cli render plan.dxf out.png --camera …` renders headless with wgpu offscreen.
- **Defaults** (in a template): wall types, door and window catalogues with common North American sizes, and stair-code limits. Imperial and metric.

## Acceptance checks (how the lead writes them)

Every phase's checks are bash scripts in `tests/acceptance/phase-N/` that drive the release or debug CLI and parse its output
with `python3` from the standard library only (no `pip install`, no network). Rules learned in phases 1 and 2:

- **Observe through the CLI.** The surfaces a check may read are: `arch.inspect` JSON, `build3d` JSON (counts, `rebuildMs`, and
  from phase 3 `probe` results), GLB/OBJ/IFC files, SVG/DXF/PDF output and PNG pixels. When a phase needs a new observable,
  add it to `arch.inspect` or `build3d` in that phase's first segment, not a new MCP tool.
- **`build3d {"probe": [[x,y,z], …]}`** (added in phase 3) answers, for each point, the `ArchId` and part (`wall`, `floor`,
  `glass`, …) of the solid that contains it, or `null`. A probe is how a check proves a hole, a slab or a wall top is
  really there: mesh counts alone do not (a wall with an uncut opening still has one mesh).
- **A check must fail on a stub.** Before locking a check, ask: would a function that returns nothing, zero, a constant or
  an empty image pass it? If yes, tighten it. (Phase 1's `06_render3d_orbit.sh` passes on an unrendered, zero-filled
  texture, because black pixels count as "shaded", and on any log line containing `gpu`.)
- **Exact numbers.** Give expected values worked out by hand, with a tolerance of 1e-6 m for geometry the code computes in
  closed form and 1e-3 m for mesh-derived values. Every phase below lists its numbers; use them.
- **No-GPU is explicit.** `cadcraft-cli render` exits with code 3 and prints `NO_ADAPTER` on stderr when no adapter exists;
  a check may SKIP only on that exit code, never on a substring of the log. From phase 6 on, a render check may not SKIP:
  it uses `--adapter fallback` (wgpu's software adapter, e.g. Mesa lavapipe) when there is no GPU.
- **Image checks compare against the clear colour,** never against a fixed brightness: the foreground fraction lies in a
  stated range, and the foreground's bounding box matches the projected scene bounds to within 3 px. No pixel goldens
  across adapters.
- **Timing** is measured inside the command (`rebuildMs`, `as_secs_f64() * 1000.0`), on a release build, median of 5 runs.
- **Carry forward** the hostile-input check (never panics, never leaves a half-applied edit) and the layers / wasm / i18n
  check in every phase.
- **Size.** About 9 checks and at most ~40 segments per phase. A segment touches 1–4 files, in one crate where possible,
  and states the exact function signatures it adds, so 16K-context workers do not have to read whole crates.

## Phases (each ends in a demo and a ⛔ checkpoint)

Before planning a phase, read `docs/arch3d/CHIEF-REFERENCE.md` (a clean-room behaviour reference from Chief Architect X18's

## Testing

- **Unit tests:** join cases (L, T, X, angled, collinear), opening placement limits, room detection with holes, area rules, roof-generator geometry (planes meet exactly, no gaps, correct pitch), stair-calculator code checks.
- **Golden scenes:** small `.dxf` plans plus expected `BuildingModel` JSON and glTF hashes.
- **Image tests:** headless renders checked against the clear colour and the projected scene bounds (see *Acceptance
  checks*); perceptual-tolerance goldens only on one pinned adapter.
- **Property and fuzz tests:** random wall graphs must never panic or produce non-manifold walls.
- **Benchmarks:** incremental-rebuild timing as edits are made.

## Done means

- I can draw a house with `WALL`, `DOOR`, `WINDOW` and `ROOFAUTO` and immediately orbit a believable 3D model with a correct roof.
- I get linked elevations on a sheet, and can export `.glb` and `.ifc`.
- I can set the house on a sloping site and Finalize a night render lit by its own fixtures.
- I can do the same by running `ARCHDETECT` on a plain DXF floor plan.
- I can furnish a house from the library and restyle it with the painters, and the showcase house (a furnished split-entry
  bi-level with over 4,000 sq ft above grade plus its basement, on a landscaped site) renders in every view.
- The showcase house has vaulted and tray ceilings, a foundation, a framed deck, a finished kitchen and baths, renders at
  noon, at dusk and under an HDRI, and comes out as a full drawing set in one PDF.
- Everything is driveable from MCP and the CLI.
- Tests are green and the web build still works.
