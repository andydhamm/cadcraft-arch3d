# arch3d — design log

Branch `arch3d` (fork of `storytold/cadcraft`, v0.10.0). Running log for "draw a plan, get the house in 3D". One commit per phase, each ending at a ⛔ checkpoint.

The sections for phases 1–15 were condensed at the phase-16 checkpoint (2026-11-11). Their full text (operator narratives, every locked number, per-check histories) is `docs/arch3d/DESIGN.md` at commit 97e939f25 and before; the checks under `tests/acceptance/phase-N/` hold the numbers.

---

## Phase 0 — survey

- Workspace `crates/*`, `apps/*`, `xtask`; edition 2024, rust 1.90, MIT OR Apache-2.0. `unsafe_code = forbid`; crate roots deny unwrap/expect/panic/todo.
- Layers (`xtask/src/layers.rs`): L0 `geom`,`color` (`dxf`,`dwg` standalone) → L1 `doc` → L2 `constraints`,`fonts`,`render` → L3 `io` → L4 `engine` → L5 `ui-egui`,`mcp` → apps.
- Roadmap: 3D is 2%; `docs/gaps.md` F24 is *AutoCAD* 3D — untouched; our work is additive with its own ARCH-* rows.
- **Entities:** `Entity{handle,kind,common}`; `Common.xdata` travels with COPY/MIRROR/ARRAY. `EntityStore` is copy-on-write, so whole-drawing undo is cheap.
- **Commands:** everything user-visible is a `CommandSpec`; one implementation, two paths — JSON `Session::execute` (MCP/CLI/scripts, hostile input validated, never clamped) and interactive `Session::start`. Both under `catch_unwind`.
- **Undo:** full-`Drawing` snapshot stack. *Consequence:* anything that must undo/autosave/audit/round-trip lives inside `Drawing`.
- **Selection/grips/snaps:** `select.rs`, `spatial.rs`, `grips.rs::stretch_grip` (drags run a `grip.*` command), `snap.rs`.
- **Rendering was 2D only:** `render::build(space) -> DisplayList` through an `egui_wgpu::Callback`, no depth buffer. The 3D viewport was new work.
- **DXF custom data:** `CADCRAFT` app XDATA + XRECORDs in the named-object dictionary; `dxf_region.rs` shows the proxy-graphic pattern.
- **MCP:** a small generic tool set; `execute` forwards to `Session::execute`. No MCP tool per arch command — commands are the API.

## Crate layout (as built)

Precedent: **`constraints` (L2) — data in `doc`, algorithms in a peer crate.**

| Crate | L | Contents |
|---|---|---|
| `doc/src/arch.rs`, `doc/src/arch/*` | L1 | `ArchModel` on `Drawing::arch` with `arch_revision`/`touch_arch()`: walls, openings, rooms, levels, roofs, stairs, railings, cameras, views, `site` (location, terrain, modifiers, plants), `light` fixtures, `finish`, `casework` (cabinets, fixtures), `furnishing` + `symbols`/`images`, `paint`, `backdrop`, `defaults`, `electrical` (devices, connections, light sets), `LevelKind`, `Wall.top_level`, `plan_level`. **Phase 16:** `Room.flat_ceiling`/`ceiling_stack` (finish layers), `Tray` (tr<n>, +0x78000), `Soffit` (sf<n>, +0x80000), `Foundation` (fd<n>, +0x88000). **Phase 17:** `detail.rs` (stair risers/stringers/underside/plan cut and per-side rails; railing `inset`, `host`, `auto`; deck board direction and rails; opening grille, top, sill and casing sides; door style `fixed`). Phases 18–20 (in progress) add `countertop`, `molding`, `look`, `planview`. Handle offsets by kind below `ARCH_HANDLE_SPAN 0x100000` (decision 10). |
| `geom` | L0 | `skeleton` (straight and weighted/roof skeleton, inset/outset), `region::Arrangement`, `tin`, `cdt`, `tin_ops`. |
| `arch` | L2 | `wall`, `graph`, `join`, `opening`, `symbol`, `room` (detection, `room_at`), `poly`, `area`, `label`, `roof`, `roofedge`, `stair`, `railing`, `sun`, `detect` (ARCHDETECT pipeline + `score`), `terrain` (`SiteSurface`, mesh, contours, volumes, csv, pad, retaining), `plant`, `site`, `light`, `ies`, `casework`, `casework_layout`, `library` (records, search, history, fits, place), `furnishing`, `paint`, `electrical`, `connections`, `outlets`, `grade`. **Phase 16:** `ceiling` (`mod`: the ceiling point query — rough, finished, roof underside, hang height; `stack`, `vault`, `tray`, `soffit`, `overhead`: what lies over a point), `foundation`. **Phase 17:** `stair_geom` (treads, nosing line), `stair_symbol` (plan symbol), `flight_cut` (walls under flights), `deck` (contact, frame, grade), `railing` (`RailSurface`, posts), `railing_auto`, `opening_top` (round, segment and roof tops). Phases 18–20 in progress: `countertop`, `backsplash`, `casework_corner`, `casework_face`, `molding_profile`, `molding_runs`, `plan`. |
| `arch3d` | L2 | `build`/`SceneCache` → `Scene3d`; `mesh`, `walls` (elevation profiles + holes), `openings`, `floors`, `roof`, `stairs`, `railing`, `probe` + `ray`, `camera`, `pick` (BVH), `edges`/`hlr`/`section`/`views`, `site`, `plant`, `fixture`, `light` (illuminance), `incremental`, `finish`, `uv`, `texture`, `casework`, `furnishing`, `paint`, `icon`, `electrical`. **Phase 16:** `ceiling` (`stack` layer solids, `vault`, `tray`, `overhead` clipping), `soffit`, foundation meshes (stem walls, footings, slabs), wall-rise meshes; probe parts `soffit`, `foundation`, `footing`, `slab`; `build3d` `parts` counts meshes per part. **Phase 17:** `stair_parts` (treads, risers, stringers, skirts), `flight_cut`, `railing_parts` (posts, rails, balusters, panels), `deck_frame` (ledger, joists, rims, beams, posts, piers, boards), grilles and shaped tops in `openings`; `texture::board_finish`. |
| `render` | L2 | `arch.rs` (arch geometry → DisplayList tagged with proxy handles; views; the plan of `plan_level`; the phase-17 stair symbol), `arch_site.rs`, `arch_casework.rs`, `arch_furnishing.rs`, `arch_electrical.rs`, `arch_ceiling.rs` (soffit outlines dashed on `A-CLNG`, footings dashed; phase 16). |
| `io` | L3 | `dxf_arch.rs` (`CADCRAFT_ARCH` XRECORD v9, accepts 1–9, payload up to 64 MiB; save-time proxies incl. `CADCRAFT-STAIR` symbol roles), `dxf_site`, `dxf_casework`, `dxf_furnishing`, `dxf_electrical`, `dxf_ceiling` (phase 16), `glb.rs` (own glTF 2.0; `node_names`, `shares_a_node`), `obj`, `png`, `step`, `ifc/*` (one module per element kind; `foundations` in phase 16; deck members and railings by host in phase 17), `gltf_read`, `image_read`, `library` (embedded sample set). |
| `render3d` | L5 | `pipeline` (PBR), `light`, `shadow`, `shadow_atlas`, `edges`, `adapter` (one adapter request at a time per process since phase 16), `offscreen` (`Style`: shaded, shaded-edges, hidden-line, glass-house, clay, technical), `final_render`, `pathtrace/*` + WGSL `pt/*`, `textures`, `backdrop`. |
| `gpu-optin` | — | The workspace's only unsafe code: wgpu's experimental-feature token for ray queries, a pipeline cache read from disk. |
| `engine/src/cmd/arch/*` | L4 | One module per command family (walls, openings, rooms, levels, roofs, stairs, railings, decks, views, sheets, archdetect, site/terrain/plants, lights, render hooks, exports, material, casework, library, paint, electrical, levels). **Phase 16:** `tray`/`.edit`/`.delete`, `soffit`/`.edit`/`.delete`, `foundation`/`.edit`/`.delete`, `room {flatCeiling, ceilingStack}`, `build3d {ceiling, level}` and `parts`, `arch.inspect {trays, soffits, foundations}` and ceiling rows/warnings. **Phase 17:** `stair` detail params and `stair.edit`, `deck {boards, rails}` and `deck.edit`, `railing {postSpacing, inset}`, `window {grille, top}`, door style `fixed`, `arch.inspect {decks}` and detail rows (`inspect_detail`, `deck_inspect`). `engine::library` (catalogs, `LibraryIo`). |
| `ui-egui` | L5 | `pane3d.rs` (dockable 3D pane, styles incl. Clay and Technical since phase 16, walkthrough, pick → 2D selection, Finalize, own `SceneCache`), `archdetect_review.rs`, `library_panel.rs`, `electrical_tools.rs`. |
| `cadcraft-cli` | — | `convert` (detects first when there are no arch objects; `.glb`, `.obj`, `.ifc`), `run --cmd / --commands FILE.json / --export`, `render … --camera --style --sun --ground --adapter --final [raster|path]` (exit 3 `NO_ADAPTER`), the session renderer (`render_hook`). |

## Decisions

1. **Arch model lives in `Drawing`** (undo is a whole-`Drawing` snapshot). Data in `doc`, algorithms in `arch`.
2. **One source of truth: the arch object, not its linework.** Plan geometry is generated at draw time; DXF proxies are written only on save and not read back when arch data is present.
3. **No 3D CSG.** Walls are cut by 2D boolean on the wall *elevation profile*, then extruded; slabs/ceilings the same on room polygons; roof planes are prisms; stairs a stepped prism. Since phase 17 the same profile carries flight cuts and shaped window tops.
4. **Pure Rust, no new heavy deps.** Own glTF/OBJ/STEP writers and glTF reader; MIT/Apache-2.0/BSD/Zlib, wasm-compatible.
5. **The 3D pane renders offscreen** into its own colour+depth textures and presents as an image; identical on WebGPU and WebGL2.
6. **MCP stays generic.** Commands are the API.
7. **Incremental rebuild first-class:** `arch_revision` + per-object inputs (decision 33).
8. **Never crash, always deterministic:** caps, `get()`, checked arithmetic, property/fuzz tests.
9. **Additive to AutoCAD parity**; arch commands are new ids under `Architecture`.
10. **Proxy handles, not entities:** `ARCH_HANDLE_BASE = 0xFFFF_FFFF_0000_0000` + kind offset + id. Offsets: walls `+0`, openings `+0x8000`, rooms `+0x10000`, roofs `+0x18000`, stairs `+0x20000`, railings `+0x28000`, views `+0x30000`, terrain `+0x38000`, plants `+0x40000`, light fixtures `+0x48000`, cabinets `+0x50000`, fixtures `+0x58000`, furnishings `+0x60000`, electrical `+0x68000`, connections `+0x70000`, trays `+0x78000`, soffits `+0x80000`, foundations `+0x88000`; phase 18 plans countertops `+0xC0000`, backsplashes `+0xC8000`, molding lines `+0xD0000`. JSON spells every handle one way (`handle_json`: the low bits over `0xA000_0000` in lower-case hex). `grips.rs::arch_id_from_handle` is the single disambiguator.
11. **Rough vs finished separate** (`ROUGH_ALLOWANCE` one constant).
12. **Placement limits, area rules and code limits are data in `arch`**, so commands/CLI/MCP/`ARCHDETECT` agree. Commands validate through these and **refuse — never clamp**.
13. **Shared structs** are built outside their declaring file only as `Type{required,..Default::default()}` or via a constructor; `cargo xtask literals` enforces it.
14. **Derived, never stored,** when a quantity follows from others (`Level::floor_to_floor()`; grade classes; the finished ceiling; a deck's frame).
15. **Probing, not counting, proves geometry.** Any new 3D feature adds probeable solids.
16. **Roofs are generated, then owned.** A `ROOFAUTO` roof regenerates while its walls change; `roofplane {edit}` sets `hand_edited`. Generation is all-or-nothing.
17. **A stair's well is derived from headroom, and the hole is applied from the stair** (`well_holes_for`), so it exists even when the upper room was never detected; the code never invents a room to hold a hole.
18. **One camera, below the renderer.** The camera math lives in `arch3d` and `render3d` re-exports it; `ray_at` inverts `project_point`.
19. **Picking is a CPU ray cast** against `Scene3d` (BVH, deterministic ties). The struck *part* picks the handle range, since ids number per kind.
20. **WebGL2 limits everywhere.** Devices use `downlevel_webgl2_defaults()` (the path tracer asks for its own compute device); shadow maps are read with `textureLoad`; `--adapter fallback` always exists.
21. **Views are linked objects stored with their geometry** (`ArchView` + generated lines, rings, dims on the view, regenerated in `after_command` inside the command's undo step). `render` cannot call `arch3d`, so it draws the stored geometry.
22. **`ArchKind` travels with every generated line and ring;** `ArchKind::from_part(part_of(mesh))` is the one mapping.
23. **Arch linework shows through paper-space viewports** and so in layout plots.
24. **One meaning of opening offset (the centre), and arch drawings are metre drawings** (`$INSUNITS 6`); accepting detection candidates in a non-metre drawing converts the whole drawing in the same undo step.
25. **Detection is a pure pipeline in `arch::detect`**; candidates carry confidence and evidence; layer names raise a score and are never required.
26. **The terrain is a surface; the mesh samples it.** `SiteSurface` is the one answer to "how high is the ground here" (deck posts ask it too). Constraints are only kinks and steps; a refinement is points. The mesh must be closed, and every mesh change carries a closed-mesh test.
27. **Site location is data the sun reads** (`north_direction`, `true_to_drawing`).
28. **Site objects have their own handles and probe parts.**
29. **The path tracer accumulates sums; every reader divides.**
30. **GPU process state is touched one thread at a time.** Headless instances are made under one lock, never with wgpu's DEBUG flag; since phase 16 adapter requests also take turns.
31. **Every export names one object alike and is reproducible** (`glb::node_names`; GlobalIds are hashes of keys; `SOURCE_DATE_EPOCH`; no `HashMap` iteration order in output).
32. **A writer of a text format is tested by parsing what it wrote** (`crates/engine/tests/ifc_step_grammar.rs`); a new IFC entity adds its count there.
33. **An incremental build is the full build, cached by its inputs.** `Scene3d::build` is made of parts, each a pure function of recorded inputs; a new 3D object kind adds its section, its inputs, and its edits to `crates/engine/tests/scene_cache_equals_full.rs`.
34. **A timing bar measures the work, not the host,** and a performance check also bounds the work done.
35. **A process-wide hook has one installer** (`LibraryIo`, `IoHooks`: `OnceLock`); unit tests use `library::testing::install_io`.
36. **Coverage comes from what the kernel knows** (the path tracer's per-pixel hit count decides where the backdrop goes).
37. **Meshes compare within a tolerance** (1e-5), never by rounded keys.
38. **Levels are free; a wall's vertical extent is derived.** `Wall::spans_level` is the one selector; `bottom_z`/`top_z` the one source of heights. `LevelKind::Floor` is not written.
39. **Up and down come from elevations, never from level order.** Since phase 16 this holds everywhere: `stair` stores the lower level first and cuts its well by elevation, `well_start` measures from the stair's own base, `roof::top_level` is the highest elevation, and an integer `level` param is the level's index.
40. **Grade classes and areas are computed on read.**
41. **A light set is a view of the lights; a circuit's brightness is an edit.**
42. **A check runs against today's CLI before it is locked, and must stop at a product assertion** (an exception, or a pass, is a check defect).
43. **A scripted model is checked for where things are, and a test segment never edits what its test judges.** Phase 16 showed the second half is not enough: `p16-rw-showcase-test` wrote containment tests the unchanged script passes (the level box is taken from rooms as well as walls; casework is counted per level, not per room). Containment is measured against **walls**, and the lead writes the assertion's geometry (which box, which room) into the spec.
44. **A room stores its rough ceiling; the finished ceiling is derived.** Finished = rough − the finish stack, or the roof underside for a vaulted room, plus trays, minus soffits. One point query in `arch::ceiling` answers it for `build3d {ceiling}`, placing (hang height), the 3D build and, later, moldings and the reflected ceiling plan.
45. **What lies over a point is one query** (`arch::ceiling::overhead`): the floor of the level above, an Open Below over it, or the roof underside. It clips ceilings and wall tops and bounds trays. Room, ceiling and level edits that would cross it are refused naming the room; roof edits are never refused, and leave a clipped ceiling and a warning.
46. **Open Below rooms are owned, not detected.** A stair's well room and a drawn Open Below room cut their ring out of the room around them and out of the floor; `roomdetect` keeps them, with their ids, and cuts them again.
47. **Foundations are derived from the exterior walls with no level below them.** `FOUNDATION` stores a type and a footing depth and creates no walls; the meshes follow the walls.
48. **A demo the brief puts on the showcase house goes on the showcase house.** A side demo script is allowed for the checks, and the difference is recorded as a gap at the checkpoint (phase 16 did not put its features on the showcase; phase 17 did, through `p17f-showcase-phase17`).
49. **A stair's `start` is the centre of its bottom nosing line, everywhere** (command, well, 3D flight, plan symbol, flight cut). One module (`arch::stair_geom`) gives tread spans and the nosing line; nothing else re-derives them.
50. **Automatic objects are made by the command that makes their host, never on load.** Stair and deck railings come from `railing_auto::regenerate` inside `stair`, `stair.edit`, `deck` and `deck.edit`; a drawing saved before the feature keeps exactly what it had. An automatic railing is an ordinary `Railing` with `auto` and `host` set, following its host.
51. **A mesh's owner comes from its material prefix, its part from `part_of`.** `deck-` meshes belong to the deck room, `railing-` to the railing, `stair-` to the stair; a part name may be shared by kinds (`post`, `footing`), so a per-object count is read from `arch.inspect`, not from `build3d parts`.
52. **A merge message is not evidence.** A segment's stated deliverable (a guide section, a test) is confirmed in the tree at the checkpoint; phase 17's user-guide text was claimed and is absent.

## Phase 1 — walls, joins, poché, first 3D (landed)

`solve_joins` trims/extends centre-line ends to neighbour face-line intersections (L/T/X/angled/collinear); `stretch_wall` is a capped transitive BFS; `render::arch` expands solved rings + layer bands; `arch3d::build` extrudes elevations with `object: ArchId` and box UVs; `io::glb` writes glTF 2.0; wall grips run `arch.wall.stretch`. Checks 01–09 PASS. Deferrals: `is_main` joins, pony walls (phase 18), `reversed`, arc walls.

## Phase 2 — openings (landed)

`Opening` (host, offset = centre, finished + rough dims, style, hinge/swing, frame/casing/sill). `arch::opening` holds the limits (`MIN_HINGED_WIDTH 0.3`, `MIN_GARAGE_WIDTH 1.8`, `MIN_SEPARATION 0.05`, `ROUGH_ALLOWANCE 0.12`) with `validate_placement`/`place_opening`. Generated plan symbols; frame/casing/glass/leaf/sill/header meshes. `door`/`window`/`garagedoor`/`arch.opening.move`. Checks 01–09 PASS. Deferrals: bay/box/bow geometry, mullions, hardware, schedules (21), pocket doors, arc-wall openings.

## Phase 3 — rooms, floors, ceilings, levels (landed)

- `render3d` really draws (depth, one light + ambient); `render` exits 3 with `NO_ADAPTER`; `build3d {probe}` via `arch3d::ray`.
- `Level{elevation, ceiling_height, structure}`, `floor_to_floor()` derived (2.4 + 0.3 = 2.7).
- `detect_rooms`: faces of the solved poché, dividers as centre lines, holes from islands (a face inside a face is a hole and a room).
- Areas (`arch::area`): interior, standard (exterior outer face / interior centre line), living (interior types, ceiling ≥ 1.2).
- 3D: a floor slab per room (top at the level, down by the structure), a 20 mm ceiling solid under the ceiling height; Open Below no floor; Deck/Porch no ceiling.
- Numbers: interior 27.93 / 28.3725 / 18.6725; standard 31.62 / 31.11 / 20.91 = 83.64; loop → 26.72 + 0.81; Level 2 probes (2,1,2.55) floor, (2,1,1.0) nothing.
- Checks 01–09 PASS. Lesson: run every check against a stub before locking it.

## Phase 4 — automatic roofs (landed)

- `Roof{planes, edges, pitch, overhang, thickness, fascia, soffit, hand_edited}`; per-wall `roof_directive`/`roof_pitch`/`roof_overhang`.
- Generator: footprint from the outer faces of the solved poché of the top level's exterior walls, `geom::roof_skeleton` (node time = height, infinity = a gable end), overhang by `outset`. All-or-nothing (`RoofError`).
- Wall-to-roof clipping and opening holes on the wall's elevation profile (`mesh::push_profile_prism`), no CSG.
- One `underside_z` serves the roof solid, fascia, soffit and the wall-top clip (fixed in phase 7).
- Numbers: hip 6/12 ridge x 4…6 at z 4.45; 4/12 3.766667; 12/12 6.5; two gables ridge −0.1…10.1; overhang 0.5 → 103.04 m²; L one valley, T two.
- Property tests on the invariants. Checks 01–11 PASS.
- Gaps: dormers, crickets, Dutch/half-hip, returns, skylights, gutters, interactive plane joining, per-plane material.

## Phase 5 — stairs, railings and decks (landed)

- `plan_stair` (closest to 168.75 mm under `MAX_RISER 0.196`; `MIN_TREAD 0.254`; `MIN_HEADROOM 2.032`), `headroom_ok` to the slab underside, `well_start`, `flight_polygon`, `well_polygon`, `well_guard_path`; railing limits (`GUARD_HEIGHT 0.914`, handrail 0.864–0.965, `MAX_BALUSTER_GAP 0.1016`).
- `stair` cuts the well in one mutation: a hole in the upper room that contains it, an Open Below room, a guard `Railing` with `follows`.
- `deck` is a `RoomType::Deck` room with `deck_height`.
- Numbers: 2.7 → 16 × 0.16875, 15 × 0.254, run 3.81; deck 0.9 → 5 × 0.18; well area 3.302.
- Checks 01–09 PASS (03 and 07 rewritten for phase 17's parts and hosts).
- Closed in phase 17: railings a top rail per segment; `A-RAIL` in the template layers; the flight half a width off its well. Still open: L and U shapes in 3D (see phase 17's gaps), no stair/railing grips. Out of scope: winders, curved/spiral, ramps, goosenecks.

## Phase 6 — the 3D view, properly (landed)

- `arch::sun` (NOAA form); PBR; one 1024² sun shadow map fitted to scene + ground; MSAA; styles shaded, shaded-edges, hidden-line, glass-house.
- `Camera3d` saved in the model (`camera {save|use|delete|list}`), full camera + `walkthrough`.
- `pick3d` on a CPU BVH.
- `render --style --sun --ground --adapter fallback`.
- Numbers: 51.05° N, 2026-06-21 solar noon → elevation 62.39°, azimuth 180°.
- Checks 01–09 PASS.
- Gaps: no sun in the pane (phase 19's looks), hard-edged shadow, `camera`'s echo form in radians.

## Phase 7 — generated elevations and sections (landed)

- `ArchView` (elevation or section) with stored `ViewGeometry`; `arch3d::views::refresh` uses the assisted `hlr` and `section` kernels (no second one).
- `elevation`, `section`, `view.delete`, `viewsheet` (a layout with one locked viewport per view at 1000/scale).
- Proxies `CADCRAFT-VIEW` on save.
- Numbers: south elevation 11.2 m wide, 5.474 m high; door 0.9 × 2.1; section rings 0.48 m²; ARCH D PDF 2592 × 1728.
- Checks 01–09 PASS.
- Gaps: scope is a plan rectangle; overall dimensions only; no view titles or title block (phase 21).
- Planner rule: a spec that points at internals names them, lists their file, and says to expose them when private.

## Phase 8 — recognition from plain DXF (landed)

- `arch::detect`: `flatten` (INSERTs to depth 16, arcs refitted) → `walls` (parallel faces 0.04–0.6 m apart, merged across gaps ≤ 2.6 m unless a crossing face spans the gap) → `junctions` → `openings` (swing arcs, sliding leaves, glass lines, cased gaps) → `rooms` (labels as names) → `layers`.
- `archdetect {dryRun | accept, minConfidence, units, layers}`; `convert` runs it first; the Detection Review panel.
- `geom::region::Arrangement` built once per group (450 s → 353 ms on 11,271 entities).
- `Orbit::fit` fits the bounding sphere.
- Numbers: apartment.dxf P = R = 1.0; synthetic walls 1.0/1.0, doors 1.0/0.958, windows 0.972/1.0.
- Checks 01–10 PASS; 07 reports the three real plans and does not gate (the two Moorfields plans find 1 and 0 walls).
- Gaps: unitless plans read as metres; no editing of candidates; curved walls.

## Phase 9 — terrain and site (landed)

- `SiteLocation`, `Terrain` (boundary, points, elevation lines, modifiers Flat/Hill/Path, pad), `Plant`/`PlantSpecies` (14 built in), `Wall.retaining/freeboard/footing`.
- `SiteSurface` (decision 26) → `site_mesh` (steps stay vertical; pad slope rings are points) → terrain solid with a 1 m skirt.
- Pads flatten to first floor − structure with a margin and a 1:2 slope; basements cut.
- Plan on `C-TOPO-*`, `C-ROAD`, `L-PLNT`, `A-ANNO-NRTH`.
- `site`, `terrain` (+ `.modify`, `.query`, `.contours`, `.volumes`, `.delete`), `plant`, `plant.species`.
- Numbers: 1.25 at (25,12); contours x = 5…35, major at 20 labelled 1.00; cut/fill 6.25/6.25; retaining 2.5 / 1.4995, top 2.65.
- Checks 01–12 PASS.
- Gaps: no terrain/plant tools or grips in the UI; one label per major contour; the pad slope is sampled.
- Planner rules: a spec lists every file it leans on; each required test gets its numbers; a mesh refinement is points unless a kink needs an edge.

## Phase 10 — lighting, light fixtures and final renders (landed)

- `LightFixture` on `E-LITE` (`lf{n}`), catalogue and photometry in `arch::light` (800 lm in 60° = 950.36 cd).
- Housings; ceiling cut-outs for recessed cans.
- `arch3d::light::illuminance` (direct, shadow rays).
- The lit raster: a shadow atlas cached per light key and occluder key; fixture housings and ceilings cast no fixture shadows.
- `FinalRender`; the CLI's session renderer; `sunstudy`.
- Numbers: 165.0 lx at (1,1,0), 122.1 at (1.8,1.8,0), 0 behind the column.
- Checks 01–11 PASS.

## Phase 11 — ray-traced Finalize Render (landed)

- `pathtrace` on its own compute device; without compute shaders the raster Finalize with `fallback: "no compute shaders"`.
- Units in nits; NEE + MIS, Russian roulette; Lambert + GGX, mirrors, glass; shadow rays tinted once per pane.
- IES LM-63 (`arch::ies`); area lights.
- À-trous denoiser weighted by each pixel's variance of the mean.
- Hardware ray queries behind `rt` (`gpu-optin`), verified equal to the compute path on llvmpipe and an RTX 4070.
- Vulkan pipeline cache.
- `final: true` is path-traced, 256 spp (64 on a CPU adapter).
- Checks 01–13 PASS.

## Phase 12 — exports and interoperability (landed)

- `io::step`; `io::ifc` (IFC4: project → site → building → storeys; walls, openings with void + fill relations, slabs, roofs, stairs, railings, spaces with areas, site, lights); bodies are `IfcTriangulatedFaceSet`; GlobalIds from keys; `SOURCE_DATE_EPOCH`.
- `io::obj` + MTL.
- `export {path | format}`.
- Auto exposure for path-traced daylight stills.
- `ifc_step_grammar.rs` (decision 32).
- Checks 01–15 PASS.
- Gaps: no IFC materials or colours; triangle bodies only; `OwnerHistory` `$`.

## Phase 13 — polish (landed)

- **Finishes:** `Finish` on walls (exterior face), roofs, floors; `material`; 256² procedural textures in `arch3d::texture`; embedded in the GLB, sampled by render3d from one texture array.
- **Casework:** `Cabinet` (base, wall, tall; counter at 0.914), `Fixture` (eight kinds); plan symbols; IFC classes.
- **Incremental rebuild** (decision 33): `SceneCache`, `ObjectKey`, `Scene3d::digest`; `build3d {incremental}`.
- **Benchmark house:** `arch.sample {house}` (310 walls, 160 openings, 108 rooms): full build 16.8 ms, median update 11.4 ms with 13 objects rebuilt.
- **Docs:** USER-GUIDE.md `sh` blocks are run by check 09; `docs/arch3d/mcp-examples/` replays.
- Checks 01–13 PASS.

## Phase 14 — the library (landed)

- **One library, nine categories;** `ItemRecord`; catalogs `builtin`, `sample` (49 items, embedded), `installed:<name>`, `local`, `user`. An item with no licence never loads. `LibraryIo` hooks (decision 35).
- **Search:** word substrings + AND chips; `FilterHistory`; saved filters; Favorites; `library {for: surface key}`.
- **Symbols:** `io::gltf_read` (plain .glb only); `SymbolMesh` keyed by file hash, stored in the drawing (`ArchModel.symbols`/`images`).
- **Furnishings:** `Furnishing` (f<n>), layers by category, IFC class by category, convex-hull plan outline.
- **Placing** (`arch::library::place`), in order: part swap, faucet on sink, cut-out item into a base cabinet, appliance into a gap, wall item on the nearest face within 0.3 m, ceiling item at the ceiling (since phase 16: the hang height from `arch::ceiling`, under a soffit or a vault), else at the click.
- **Cabinet parts:** `casework_layout` (fronts, pulls, counter runs, gaps, false fronts); slab, shaker and raised-panel fronts.
- **Painters:** surface keys, `ArchModel.paints`, scopes component/object/room/floor/plan, `blend` tint, eyedropper.
- **User catalog:** `library.add`, `library.copy`, `.edit`, `.delete`, `.trash`. `arch.default`.
- **3D:** `Scene3d.materials`/`textures`/`backdrop`.
- **Icons:** `arch3d::icon` (CPU rasterizer, byte-identical).
- **Backdrops:** colour, gradient, image (flat or equirect), composited on misses.
- Checks 01–13 PASS.
- Open: `move` silently moves nothing for handles it cannot resolve; `arch.inspect` prints furnishing handles in the short form; `library_user::relative_value` indexes and unwraps; hull outlines; symbol lights do not emit; the path tracer uses texture means; flat icons.

## Phase 15 — electrical, split levels and the showcase house (landed)

- **Electrical:** `ElectricalDevice` (`el<n>`; duplex, gfci, 240v; single-pole, three-way, dimmer) on a wall face at 0.3 / 1.2 m, counter outlets at counter top + 0.15; plan symbols on `E-POWR`/`E-LITE`; cover plates; `IfcOutlet`/`IfcSwitchingDevice`. `Connection` (`cn<n>`) arcs; circuits; `circuit.brightness`. `outlets.auto` (1.8 m rule; kitchen GFCI 1.2 m / 0.6 m) with `outletGaps` measured back. `view.electrical` (a layer state), `level.show`, `export {level | view}`, light sets (decision 41).
- **Levels:** free elevations, `LevelKind` floor | landing, `Wall.top_level` spanning walls, openings on a level (decision 38).
- **Grade** (`arch::grade`): `finished_grade`, `grade_class`, `above_grade_area` (decision 40).
- **Pads** for landings, side splits and an entry apron.
- **Styles:** Clay and Technical.
- **Showcase:** `docs/arch3d/showcase/bilevel.json` replayed by `cadcraft-cli run --commands`; `tools/showcase/render_all.py`; `crates/engine/tests/showcase.rs`.
- Checks 01–14 PASS after nine were corrected by the operator (decision 42).
- **Gaps closed in phase 16:** stair wells by elevation and from the stair's base; landing walls and ceilings clipped at the level above; `roomdetect` keeping stair-well rooms; the above-grade area without Open Below rooms; technical silhouettes from the camera; Clay and Technical in the pane; the showcase images committed; the split entry's flights and foyer.
- **Still open:** the showcase's rooms outside the house, its kitchen casework, its undivided bays and blank fronts (see phase 16's gaps); `ifc_step_grammar.rs` has no counts for the electrical entities; connections are not exported; a level edit rebuilds every object.

## Phase 16 — ceilings and foundations (landed)

20/20 checks on 2026-11-11 after two fix cycles and the showcase rework.

- **What merged:** 37 segments and 2 upstream syncs (6f0f1fefa, 2f8f18618; one commit each), plus harden passes.
- **Rounds:**
  - the plan on operator groundwork (78f95deaf): vault-ceiling, vault-walls, foundation-stem, foundation-slab, tray-arch/-i18n/-arch3d/-commands, trays-check, soffit-arch/-commands/-input/-meshes, exports, rebuild, demo, render3d-adapter-turns;
  - fix cycle 1: p16f-tray-outline-reply, -tray-roof-guard, -soffit-faces, -slab-edge-mesh, -demo-replays;
  - the showcase rework proposed at the phase-15 checkpoint (p16-rw-*, checks 14–20);
  - fix cycle 2: p16f2-slab-edge-width, -room-level-index, -silhouette-depth, -showcase-entry.
- **Checks:** the lead revised them twice (371fd74a6, 7893ad59c). The operator landed the earlier-phase checks that phases 16–19 change (bfaa016c6: phase-5 03/07, phase-13 02/13, phase-15 10/11).

### Observables
- `build3d {"ceiling": [[x,y], …], "level": n}` gives the finished ceiling elevation above each point on that level (`null` outside every room).
- `build3d` reports `parts`: scene meshes counted per part (`part_of`), so a countable thing is one mesh.
- `arch.inspect` gains ceiling rows per room (rough, finished, stack, `flatCeiling`, `ceilingWarning`) and `trays`, `soffits`, `foundations`.

### Ceiling build-up (decision 44)
- `Room.ceiling_height` is the rough ceiling.
- `Room.ceiling_stack` is a list of layers (material + thickness; an air gap has no material), top to bottom; finished = rough − the stack's thickness.
- An empty stack keeps the phase-3 20 mm ceiling solid; a stack builds one solid per material layer and nothing for air.
- Templates: `drywall` (12.7 mm) and `suspended` (drywall 12.7, air 200, tile 16); `room {ceilingStack: name | layers}`.

### Vaults and the guard (decision 45)
- `room {flatCeiling: false}` on a room under a roof: the ceiling is the underside of the roof planes over it (`arch::ceiling::vault`), re-pitched with the roof.
- Every wall bounding a vaulted room rises to that underside along its thickness (wall-rise meshes in their own incremental section); an interior wall between a vaulted and a flat room rises too, closing the attic side. Gable walls keep their phase-4 profile.
- Ceiling items hang plumb from the vault at their point.
- A room, ceiling or level edit that would put a flat ceiling above the roof underside is refused naming the room. A roof edit is never refused: the ceiling follows the underside there and the room row carries `ceilingWarning`.

### What lies overhead
`arch::ceiling::overhead` answers what is over a point on a level: the floor of the level above (by elevation), an Open Below room over it, or the roof.
- Ceilings and wall tops are clipped at the slab underside of the level above (a landing's walls and ceiling no longer rise through the main floor).
- A lower level's ceiling stops under a landing's slab.
- Under an Open Below room the ceiling is that room's.

### Trays
- `Tray` (`tr<n>`): a room or a drawn polygon, `inset`, `depth`, `sides` vertical or a pitch.
- `arch::ceiling::tray` holds `base_ring`, `outline`, `top_ring`, `lift` and `validate`.
- Trays nest (a tray inside a tray's outline adds its depth).
- A tray is refused, with nothing written, if any part of its top would cross the roof underside, or if the room has a level above it.
- `tray`, `tray.edit`, `tray.delete`; the reply's `outline` is a ring of `[x, y]`.
- `roomdetect` remaps a tray's room.
- 3D: the ceiling pieces inside the outline are raised and side bands close the step.

### Soffits
- `Soffit` (`sf<n>`): a plan rectangle or polygon, a bottom elevation or a depth below the finished ceiling, its own material (materials `soffit-box…`; the roof's `soffit-panel` stays part `roof`).
- A rectangle against a wall or in a corner snaps to the room-side wall faces.
- `soffit {over: [cabinets]}` fills from the wall cabinets' tops to the ceiling, starting at their backs.
- Plan: a dashed outline on `A-CLNG` (`render::arch_ceiling`).
- Ceiling items over a soffit hang from its underside (`bottom_at`), and moving a soffit over placed lights re-seats them.
- `soffit`, `soffit.edit`, `soffit.delete`.

### Foundations (decision 47)
- `foundation {type: stem | slab, footingDepth?: 1.2}` (`fd<n>`), `foundation.edit`, `foundation.delete`.
- It covers every exterior wall with no level below it: the lowest level's, and any on grade higher up.
- **Stem:** a 0.2 m concrete wall, outer face flush with the wall above, from the underside of the floor structure down to a footing 0.6 × 0.25 m centred under it, whose bottom is `footingDepth` below the floor elevation.
- **Slab:** the floor structure becomes a 0.1 m concrete slab (part `slab`) with a thickened edge 0.3 m wide inward from the outer face, reaching 0.3 m below the slab's top; one footing mesh per founded wall.
- **Basements:** exterior walls take the "Concrete 0.2" wall type and a footing directly under the basement's floor structure.
- Footings draw dashed in the plan of the level they support and show filled in sections.
- Footing depth is measured from the floor elevation, with or without terrain.

### Rebuild and exports
- New incremental sections: soffits, foundations, wall rises. A room's inputs include its stack, vault flag, trays and what is overhead.
- `scene_cache_equals_full` gains stack, vault, tray, soffit and foundation edits.
- glTF nodes and OBJ groups per object.
- IFC (`io::ifc::foundations`): `IfcFooting`, foundation walls as `IfcWall`, soffits as `IfcCovering` (CEILING), base slabs, stacked ceilings.
- DXF proxies (`io::dxf_ceiling`) on their layers; the data rides in the v9 record.

### The showcase rework (checks 14–20)
- **Stairs** (decision 39): levels are ordered by elevation; a stair given from the upper level is stored lower level first; the well is measured from the stair's base. `roofauto` with no level takes the highest elevation. An integer `level` on `room` is the level's index.
- **Open Below** (decision 46): a drawn `room {type: openBelow}` cuts a hole in the room around it and opens the floor; `roomdetect` keeps Open Below rooms and stair wells with their ids.
- **Above-grade area:** Chief's living-area convention; Open Below, Garage, Porch and Deck rooms never count.
- **Technical illustration:** silhouettes for the camera's view direction; fills get a slope-scaled depth bias so the lines show. Clay and Technical are in the pane's style selector (`Style3d::ALL`, six styles).
- **The script:** the Lower→Entry flight starts at (5.4, 2.85) heading −y and ends on the landing; the Entry→Main flight starts at (4, 0); a Foyer (Open Below on Main, x 3.05…5.95, y 0.1…2.95) gives the landing full headroom.
- **Renders:** `docs/arch3d/showcase/renders/` holds the 20 images, `contact-sheet.png` and `manifest.json` (with the script's SHA-256), committed.

### Demo
`docs/arch3d/showcase/phase-16-demo.json` is a 12 × 9 m house with a basement under its south half:
- a vaulted Living room;
- a Utility room with the suspended stack;
- a tray in the Dining room (a drawn polygon, inset 0.6, depth 0.3);
- four wall cabinets with `soffit {over}`;
- a stem foundation;
- cameras `SouthWest` and `LivingVault`.

USER-GUIDE.md gained "Ceilings and foundations".

### Numbers locked
Phase-1 house, walls 2.4, 6/12 gable roof with gables on x = 0 and x = 10 (underside z = 2.4 + 0.5·(y + 0.1) for y ≤ 4).
- **Vault:** 3.45 at (5,2), 2.70 at (5,0.5), 2.4 at (3,5); wall hits at (5,3.0,3.9) and (5,0.05,2.45); a ceiling item at (5,2) tops at 3.45.
- **Tray:** 2.7 at (3.025,5.475), 2.4 at (0.4,5.0); nested 2.85; depth 0.5 refused (2.9 against 2.8 at y = 7.3).
- **Suspended:** 2.1713 at (8,5); `ceiling` at (8,5,2.18), nothing at (8,5,2.3).
- **Soffit** x 7…9, y 7.3…7.9, bottom 2.1: hit at (8,7.6,2.2), nothing at (8,7.2,2.2); a ceiling item tops at 2.1.
- **Stem:** wall at (5,0,−0.5), footing at (5,0.25,−1.0), nothing at (5,0.25,−0.5) and (5,0.35,−1.0); basement footing at (5,0.25,−3.1).
- **Slab:** (5,1,−0.05) slab, (5,0.15,−0.2) edge, nothing at (5,0.25,−0.2).
- **Wells** x 1.5…2.5, y 1.508…4.81 (3.302 m²) whatever the level order.
- **Landing:** ceiling 2.4, walls stop at 2.7; with the foyer, Main's room is 68.175 m² and the landing's ceiling 5.1.
- **Above-grade area** of the phase-1 house: 74.975 → 56.3025 → 27.93 → 0 as rooms become garage, porch, deck.

### Acceptance (all PASS)
`01_observables` · `02_ceiling_stack` · `03_vault` · `04_ceiling_guard` · `05_trays` · `06_soffits` · `07_foundations` · `08_foundation_plan_sections` · `09_exports` · `10_rebuild` · `11_hostile` · `12_layers_wasm_i18n` · `13_demo` · `14_stair_wells_by_elevation` · `15_landing_clip_open_below` · `16_roomdetect_keeps_open_below` · `17_above_grade_living_area` · `18_technical_silhouettes` · `19_pane_styles` · `20_showcase_split_entry`.

### Phase-16 gaps
- **Out of scope by the brief:** manual ceiling planes with their own pitch, barrel and shelf ceilings, wall, floor and roof framing and trusses, garage curbs, piers and grade beams.
- **The demo is not on the showcase house** (decision 48). `bilevel.json` has no vault, tray, soffit, ceiling stack or foundation; the brief asks for a vaulted living room, a dining tray, kitchen soffits, a suspended utility ceiling, footings and a concrete basement there, re-rendered on the contact sheet. Still so at the phase-17 checkpoint.
- **The demo house** draws its tray as a polygon (not `tray {room}`) and has no nested tray.
- **The showcase is still not a designed house** (read from the script):
  - Dining, Bedroom 2 and Bedroom 3 are room polygons at y 18–23, outside the 26 × 16 m shell, each with one free-standing wall;
  - the kitchen's cabinets stand on Main's west wall (w7) and at (4, 8), in the x 0–10 bay, not in the kitchen;
  - Main is four large bays;
  - five windows in all; the Front camera shows a blank wall;
  - the plan images draw the whole model small, with the generated views beside it;
  - `README.md`'s room table does not match the script.
- **`crates/engine/tests/showcase.rs` passes on that script** (decision 43): `the_showcase_rooms_lie_inside_their_levels` takes the level box from rooms as well as walls; `the_showcase_kitchen_holds_its_casework` counts cabinets on Main, not in the kitchen. Check 20 covers the split entry, the area and the committed renders, not the layout.
- **Smaller items:**
  - trays, soffits and foundations have commands and menu entries but no grips;
  - a level edit still rebuilds every object;
  - `ifc_step_grammar.rs` should be checked for `IfcFooting` and the electrical entities.

## Phase 17 — stairs, decks, railings and openings in detail (landed)

14/14 checks on 2026-11-12 after one fix cycle.

- **What merged:** the 13 planned segments (p17-observables as -model and -inspect; p17-scene-plumbing as -arch, -scene, -export and -pick; p17-commands, -stair-flight, -wall-under-flight, -stair-plan, -railing-parts, -railing-auto, -deck-frame, -grilles-doors, -window-tops, -exports-rebuild, -demo), the fix cycle (p17f-railing-auto-contact, p17f-grille-top-part, p17f-showcase-phase17), two upstream syncs (e5021f017, ca87680fb) and harden passes.
- **Checks:** planned against arch3d at 820b8d2; revised once by the lead (3e4c8b77e). The operator landed phase-5 check 03 (probing `tread`, `riser`, `stringer`) and 07 (the well guard selected by host), dropped a stray scratch test (7efb9bd81) and re-rendered the showcase gallery (b97e23f24).

### One stair convention (decision 49)
- `start` is the centre of the bottom nosing line for the command, the well, the 3D flight, the plan symbol and the flight cut.
- Tread k has its top at k·rh and spans u in [(k−1)·td − 0.025, k·td]; the nosing line is nosing_v(u) = rh + (u + 0.025)·rh / td, so tread 1's nosing is 25 mm before `start`.

### Stairs in detail
- Treads 25 mm thick with a 25 mm nosing; risers closed or open (`openRisers`); stringers `two` (default), `centre` or `none` (a skirt board on a wall side).
- `stair {…, openRisers?, stringers?, underside?, railLeft?, railRight?, planCut?}` and `stair.edit {id, …}`.
- **Wall under a flight:** `arch::flight_cut::flight_cuts` finds the walls a flight runs over; `arch3d::flight_cut::apply` cuts each wall's elevation profile at the flight's underside, `underside` (default 0.3 m) below the nosing line measured vertically. Outside the flight the wall keeps its height. No CSG (decision 3).
- **Plan symbol** (`arch::stair_symbol`): on the lower level, solid nosing lines for treads whose top is under the plan cut, a diagonal break line, dashed nosing lines beyond and a walk arrow labelled `UP`; on the upper level the part seen through the well, solid, labelled `DN`. On the stair's layer, in the plan, exports and the DXF proxies (`CADCRAFT-STAIR`, each line with a role). The cut height is stored per stair (`Stair.plan_cut`, default 1.2): levels have no plan settings.

### Railings as built
- `arch::railing`: 90 mm posts at every end, corner and turn and at most `post_spacing` apart in equal bays (posts at k·L/n per segment); a top rail; a bottom rail 0.05 m above the walking surface; balusters or a panel between; an optional horizontal `inset`.
- `Railing::default().post_spacing` went 1.2 → 1.8. The field was always serialised, so railings saved before keep 1.2.
- New `Railing` fields `inset`, `host` (well | stairLeft | stairRight | deck) and `auto`; a railing saved before phase 17 loads as host Well, auto false.
- **Automatic railings** (decision 50): `railing_auto::regenerate`, called only by `stair`, `stair.edit`, `deck` and `deck.edit`.
  - Stair sides not against a wall, from the first riser line to the top riser, inset 0.045 into the flight, 0.914 high, a newel at each flight end.
  - Deck free edges, inset 0.045 with the posts on the rim joists; ends against a wall pulled back 0.045; an edge against the house stays open.
  - Per side `auto` (default), `on` or `off` (`railLeft`/`railRight`; `rails` on a deck, one value or one per edge).
  - Wall contact is real contact with a wall face (p17f-railing-auto-contact), not a bounding-box test.

### Decks you can stand under
- The frame is derived with the deck (`arch::deck::frame`, `arch3d::deck_frame`), never stored (decision 14):
  - a ledger on the house wall it touches (the longer contact when it touches two);
  - 38 × 184 mm joists away from the ledger in equal bays of at most 0.406 m, rim joists on the free edges;
  - a 76 × 235 mm drop beam under the joists, 0.3 m back from the outer edge;
  - 0.14 m square posts under the beam from grade to its bottom, end posts 0.3 m in from the sides, bays at most 2.4 m, each on a 0.3 m round pier;
  - 25 mm boards on top.
- A deck touching no wall has no ledger: joists span the shorter side with a beam 0.3 m in from each end.
- Grade is `SiteSurface` under each post when there is terrain, else z = 0 (`deck::site_grade`).
- `deck {boards?: deg | null, rails?}` and `deck.edit`. The board direction (default parallel to the ledger) turns the texture only: boards keep material `deck` but show the Wood board texture (`texture::board_finish`, consulted first by render3d's and io's `finish_of`) with UVs along the direction.
- `arch.inspect {decks}` lists the ledger, joists, rims, beams and posts.

### Openings in detail
- **Grilles:** `grille {across, down, part?: "top"}`: 20 × 20 mm bars in the glass plane dividing the glass evenly; the top part is divided on its own; 1 × n and n × 1 grids work (p17f-grille-top-part). `arch.inspect` reports the glass plane and centre.
- **Tops** (`arch::opening_top`): `square`, `round` (a half circle across the width), `segment` (an arc of a given rise), `roof`. Outlines are built in the host wall's profile coordinates from the wall's bottom with v0 = `sill`; only `walls::opening_holes` keeps its legacy `sill − base`, so square holes stay byte-identical. The rough opening is cut to the shape.
- **Roof tops:** the height is derived (T(c) − 0.12 − sill, i.e. the roof underside less the 0.15 m offset at the rough opening) when the window is placed or moved; `opening::check_height` skips the wall-height head check; no header. A roof re-pitch leaves the stored height until the window is placed or moved again, while the 3D top always follows the roof.
- **Fixed doors:** style `fixed`, a glazed leaf that does not open (a sidelight).
- **Sill and casing sides:** each can be off on either side; the default is the single side used before (the wall's right side), so goldens and old drawings are unchanged.

### Parts, owners, exports, rebuild
- **Part names** (`probe::part_of`): `tread`, `riser`, `stringer`, `skirt`, `stair` (the landing), `post` (railing and deck posts), `railing` (rails, panels, balusters), `ledger`, `joist`, `rim`, `beam`, `footing` (deck piers and phase-16 footings), `deck` (the boards), `grille`, `sill`, `casing`.
- **Owners** (decision 51): by material prefix (`deck-` → the deck room, `railing-` → the railing, `stair-` → the stair). `build3d parts.post` counts both kinds of post; per-deck counts come from `arch.inspect {decks}`. `mesh_label` names stairs and railings.
- **GLB / OBJ:** one node per stair (`stair-s<n>`), per railing (`railing-ra<n>`) and per deck-structure kind (`deck-r<n>-ledger` / `-joists` / `-rims` / `-beams` / `-posts` / `-piers`), each part mesh a primitive of that node (`glb::shares_a_node`; `obj::groups` follows it).
- **IFC:** ledgers, joists, rims and beams as `IfcMember`; deck posts as `IfcColumn`; piers as `IfcFooting`; `IfcRailing` HANDRAIL for stair sides and well guards, GUARDRAIL for deck and drawn railings.
- **Rebuild:** `scene_cache_equals_full` gained deck structure, railings, stair parts and grilles, with dependency bounds.

### Demo
- `docs/arch3d/showcase/phase-17-demo.json`: a two-storey 10 × 8 m house. A stair at (1, 2) heading +x on its well with both side railings; a rear deck (2, 8.1)–(7, 11.1), 0.9 m up, framed and railed; windows with 3 × 2 and 2 × 2 grilles; a round top with a top-part grille; a 0.4 m fixed sidelight; cameras `Deck` and `Stair`. Both plans show the full symbol (`UP` below, `DN` above).
- **The showcase** (`bilevel.json`, p17f-showcase-phase17): both half flights railed on their open sides; a rear deck x 11…17, y 16.1…19.1, 2.7 m above the Lower floor, framed and railed; 2 × 2 grilles on its five windows; one round top (w6). The Entry plan carries `UP` and `DN`. The committed gallery (`renders/`, `manifest.json`) is of this script (the manifest's SHA-256 matches).

### Numbers locked
Phase-1 house, walls 2.4; level 2 at 2.7 for the stairs; the roof-top window under phase 16's gable roof.
- **Deck** (2,8.1)–(6,11.1), 0.9 up: 9 joists at x = 2.4 … 5.6 (10 bays of 0.4) plus rims; posts at x = 2.3, 4.0, 5.7 on y = 10.8. Post at (4.0,10.8,0.2); nothing at (3.15,10.8,0.2); beam at (3.15,10.8,0.6); joist at (2.8,9.5,0.8); nothing at (3.0,9.5,0.8); boards at (3.0,9.5,0.89).
- **Railing** path (0,0) (0,3) (4,3) (4,0): 8 posts, the four path points plus (0,1.5), (1.3333,3), (2.6667,3), (4,1.5); a post at (1.3333,3,0.5).
- **Stair plan** (16 risers of 0.16875): 7 solid nosing lines, one break line, 8 dashed on level 1; `DN` and no dashed line on level 2.
- **Wall under a flight:** stair at (1,1) heading +x, 0.9 wide; wall (3,0)–(3,3). Nosing line 1.5141 at x = 3.0, wall top 1.2141: hit at (3.0,1.0,1.15), nothing at (3.0,1.0,1.27), full height at (3.0,2.5,2.0). 16 `riser` parts closed, 0 open.
- **Grilles** 2 × 2 on the phase-4 window: `grille` at the glass centre and 0.15 m beside it; `glass` 0.15 m beside and 0.15 m above it.
- **Round top** 0.9 × 1.5, sill 0.6 (radius 0.45, spring line 1.65; rough arc 1.9664 at du 0.4): no wall at (4.0,8.0,2.0), wall at (3.6,8.0,2.05). Segment: 2.0235 at du 0.45.
- **Roof top** at y = 1.5 in the x = 0 gable, sill 1.2: top 3.05 at y = 1.5 and 2.825 at y = 1.05 (stored height 1.73); no wall at (0,1.5,3.0), wall at (0,1.05,2.9).

### Acceptance (all PASS)
`01_observables` · `02_commands` · `03_stair_detail` · `04_wall_under_flight` · `05_stair_plan` · `06_railings` · `07_auto_railings` · `08_deck_frame` · `09_grilles_doors` · `10_window_tops` · `11_rebuild_exports` · `12_hostile` · `13_layers_wasm_i18n` · `14_demo`.

### Phase-17 gaps
- **Out of scope by the brief:** winders, curved, spiral and wrapped stairs, flights of different widths, starter and flared treads, horizontal and cable rails, deck border and transition boards, dormers, shed roofs.
- **USER-GUIDE.md has no phase-17 section** (decision 52). `p17-demo`'s merge message names the user guide, but the file has nothing on stair parts, deck frames, automatic railings, grilles or window tops, and no check asks for it.
- **`ifc_step_grammar.rs` has no counts for `IfcMember`, `IfcColumn` or `IfcRailing`** (decision 32); phase-17 check 11 parses the STEP file in Python instead. `IfcFooting` and the electrical entities are still missing there too.
- **Stair shapes:** `arch3d::stairs` and `stair_parts` show no L or U handling; the detailed parts appear to be built for straight flights only (carried from phase 5, not checked here).
- **Roof-top windows** keep their stored height after a roof re-pitch until placed or moved again; `arch.inspect` then disagrees with the 3D top.
- **Automatic railings are decided only by the stair and deck commands:** a wall moved against (or away from) a stair side or deck edge does not change them until the next `stair.edit` or `deck.edit`.
- **Shared part names:** `post` and `footing` each cover two kinds of thing in `build3d parts`.
- **No grips** for stairs, railings or decks.
- **The showcase** has its phase-17 details but is otherwise as phase 16 left it (not a designed house; no phase-16 ceilings or foundation; loose containment tests).

## Open items vs Chief Architect

- **Showcase:** a house designed to the brief (rooms inside its walls, a kitchen with its casework and island, windows on every side, the phase-16 ceilings and foundation), with containment tests against walls.
- **Docs:** a USER-GUIDE.md section for phase 17 (stair details, deck frames, railings, grilles, window tops).
- **3D view and rendering:** sun in the pane and site materials (phase 19's looks); SSAO/TAA/cascaded shadows; raster window glass opaque; Russian roulette without a survival floor; textures in the path tracer; shaded icons.
- **Walls and openings:** arc walls and their openings; main-layer joins and the standard-area basis; `reversed`; curtain walls; opening sub-items (mullions, jambs, hardware, display states); door classification; bay, box and bow geometry.
- **Roofs:** dormers, crickets, roof returns, skylights, gutters, shed roofs; interactive roof-plane joining; dynamic roof topology.
- **Stairs:** winders, curved and spiral stairs, ramps; L and U flights in detail; flights of different widths; starter treads; grips.
- **Railings and decks:** horizontal and cable rails, goosenecks; deck border and transition boards; railings re-decided when walls move; grips.
- **Ceilings:** manual ceiling planes, barrel and shelf ceilings.
- **Structure:** wall, floor and roof framing; piers, grade beams, garage curbs.
- **Services:** plumbing and HVAC; load calculations, panel schedules, wiring runs, low-voltage; connections and circuits in exports.
- **Site:** terrain/plant tools, grips and a site panel in the UI.
- **Exports:** IFC materials and colours; parametric IFC bodies; grammar-test counts for footings, deck members, railings and electrical entities.
- **Recognition:** `ARCHDETECT` on real plans; scale from dimensions; editing candidates before accept.
- **Rooms:** room boxes, dynamic defaults.
- **Library:** `move` refusing handles it cannot resolve; `arch.inspect` handles in the form commands take; furnishing outlines beyond the hull; emitting symbol lights.
- **Rebuild:** a level edit that rebuilds only what it touches.
- **Georeferencing:** transforms between coordinate systems.
- Planned for phases 18–21: pony walls, moldings, reference display, schedules, sheet title blocks and view titles.

## Phase plan

0–16 ✅ → **17 ✅ stairs, decks, railings and openings in detail** (2026-11-12; the user-guide section and the showcase layout still open) → 18 interior finish, kitchen and bath (started: observables, scene plumbing, molding kit) → 19 light, sky and cameras (in progress: foundation, groundwork, exposure 1–3, sun, seeing-in model, normal maps 1–2, strip model) → 20 plan views and dimensions (started: observables 1–3) → 21 schedules, elevations and sheets.

## Building blocks added outside the swarm (assisted)

- **`geom::skeleton`, `roof_skeleton`, `weighted_skeleton`** (2026-10-25): straight skeleton, mitred inset/outset, per-edge pitch and gable ends; phase 4 is built on it.
- **`render3d` drawing and the 3D pane** (2026-10-25): `Offscreen` (`create`/`with_device`/`encode`/`render`/`read_png`), flat shading from each triangle's plane, `headless_instance` on `Backends::PRIMARY`, one shared GPU test device; wgpu 30.
- **Feature edges, HLR, section cuts** (2026-10-25): `arch3d::edges` (`feature_edges`, crease 20°, silhouettes with a view direction), `hlr` (`OrthoView`, exact `visible_lines`, `merge_collinear`), `section` (`Plane`, `section_cut`, `section_view`).
- **`geom::tin`** (2026-10-30): lifted from zorgglubb/cadcraft's `cadcraft-terrain` (MIT OR Apache-2.0, credited in `ATTRIBUTION.md`); the site data model and phase-9 stubs.
- **Phase-10 groundwork** (2026-10-31): the light data model, catalogue and photometry, the lit pipelines' group 2 and shadow atlas contract, `RenderHooks`.
- **Phase-11 groundwork** (2026-11-01): area lights, IES, `render3d::pathtrace` skeleton, `crates/gpu-optin`.
- **Phase-12 groundwork** (2026-11-01): `io::step`, `io::ifc` spine, walls in full, element stubs.
- **Phase-14 groundwork** (2026-11-03): the sample set `assets/library/sample/` (49 items, CC0 and CC-BY, bottom-centred at catalog size, `catalog.json`), `tools/library/` (run by the operator), CHIEF-REFERENCE §6.
- **Operator fixes, phases 10–15:** recorded in the phase sections above and in full in this file's history (97e939f25). Recurring causes: tests asserting substrings malformed output also contains (decision 32), a cache patched beside the build (33), process-wide hooks (35), checks never run before locking (42), a test segment editing what it judges (43).
- **Phase-16 groundwork** (2026-11-08, 78f95deaf):
  - room `flatCeiling`/`ceilingStack` and the ceiling finish stack;
  - the arch ceiling point query with stubs for vault, tray and soffit;
  - `build3d` `ceiling`, `probe` and `parts`; arch.inspect ceiling rows;
  - the Tray/Soffit/Foundation data types, handles and labels;
  - the planar geometry kit; empty soffit, foundation and wall-rise sections in the incremental build;
  - checks 01 and 02, `p16lib.inc`.

  Also CHIEF-REFERENCE §7 for phases 16–21 (0dcace523). A CC-0 sky HDRI for phase 19 was added (576538153) and reverted (6f0b65d23); p19-hdri restores it from that commit.
- **Phase-16 operator work:** the earlier-phase checks that phases 16–19 change (bfaa016c6); phase-14 check 01 pinning the nine finishes for phase 18 (52a669156).
- **Phase-17 operator work:** phase-5 checks 03 and 07 for the new parts and railing hosts; a stray scratch test removed (7efb9bd81); the showcase gallery re-rendered from today's `bilevel.json` (b97e23f24); phase-16 check 20 notes, rather than fails, renders of an earlier script (f111186e3).

## Phase 18 plan

Phase 18 (interior finish, kitchen and bath) was planned against arch3d at febe33d on top of the phase-17 plan. 20 segments, several split by size into numbered parts (each part's listed files under 300 KB): p18-observables (-1..-3), p18-scene-plumbing (-1..-4), p18-room-finishes (-1..-3), p18-molding-kit (-1, -2), then p18-room-moldings, p18-molding-lines (-1, -2), p18-cabinet-moldings, p18-countertops, p18-backsplash, p18-cabinet-fronts (-1, -2), p18-corners, p18-fillers-shelves, p18-glass-walls (-1..-3); second-level p18-countertop-shapes, p18-face-appliances, p18-shower-niche (-1..-4), p18-crown-wrap; last p18-exports, p18-rebuild, p18-demo. Checks 01-20 under tests/acceptance/phase-18 with p18lib.inc (loads p17lib.inc, else p16lib.inc). Status at the phase-17 checkpoint: observables 1-3, scene-plumbing 1-3 and molding-kit 1-2 merged.

- Earlier checks, landed by the operator: phase-13 check 02's last part exports the walls alone (a detected room's floor now takes its type's template finish); phase-14 check 01 pins the nine finish items (off the passing record until the finish segments land).
- Room-type finishes are derived, never stored: `cadcraft_arch::room_finish::template(RoomType)` gives floor / wall / ceiling finishes (walls Drywall, ceilings Plaster, floors wood / carpet / tile / concrete by type; Kitchen floors are tile); `unpainted_finish` resolves a surface's material as paint > explicit finish > template > default. A type change clears the phase-13 floor_finish. In 3D only floors take the template texture; wall faces and ceilings keep their unsplit meshes while the surface rows and the eyedropper report the template finish. Finish gains Concrete, Tile, Carpet, Drywall, Plaster (nine in all); render3d's texture array keeps 256 layers.
- Moldings: profiles are library items of kind `molding` (five built in, generated in Rust, folder Moldings of Cabinet parts; a closed LWPOLYLINE through `library.add {kind: molding}`) with a `profile` point list; one mitred sweep (`cadcraft_arch3d::sweep::sweep`) builds room moldings, molding lines and cabinet crowns. Room moldings are a per-room list (RoomMolding { kind, profile, offset }); their runs (`molding_runs::room_runs`) follow the room ring's wall faces, break at opening trims that overlap the band (trim = width + frame + the casing on that face, from `opening_trim`, which reads phase 17's casing_sides), and a face carries a band only when the wall's top reaches it and the band lies on a solid part. The crown follows the finished ceiling (phase 16's point query), wraps opted-in soffits and runs round trays; a soffit can opt out (Soffit.no_moldings). Molding lines are objects ml<n> (0xD0000) with inset and flip. Lengths are 2D path lengths.
- Countertops: automatic counters stay derived from `casework_layout::counter_runs`; `countertop.detach` turns one into a Countertop object (ct<n>, 0xC0000) that claims its cabinets; edges move by a typed distance, bulge by a sagitta, take an edge profile and turn down as a waterfall (material countertop-waterfall). `elevation` is the counter's top above the level floor. Areas are outline areas (arcs included, cut-out holes not subtracted). Backsplashes (bs<n>, 0xC8000) are recomputed from the counters: tops at the wall cabinets' bottoms over them and counter top + height elsewhere, or the ceiling; Cabinet.backsplash_off removes a cabinet's stretch.
- Cabinets: Fronts::Open, a stored FaceLayout (rows of door / drawer / false / opening / appliance, heights, across; 3 mm reveals, 0.038 frames for framed inset), Construction / FrontMount, CornerPref, shelves, filler, crown / light rail. Options are parsed once per `cabinet` / `cabinet.edit` call and applied by a fallible `cabinet_opts::apply` before validate_cabinet. Door pairs start above 0.686 m (was 0.6). Corner cabinets are derived from geometry (`casework_corner::corner_info`) and `merge_corners` unions the two runs' outlines into one counter. Fillers are cabinets with `filler: true` (width (0, 0.15]), made only by cabinet.fill. Face appliances are face rows of kind appliance with the item's height (microwave drawer 0.381, wall oven 0.737, warming drawer 0.254; 0.762 wide); a tall cabinet takes two.
- Incremental rebuild: CabinetIn holds projections (CabinetNeighbour) of the cabinets another cabinet's meshes read plus the claimed ids; RoomIn stores the room with its moldings emptied; four new sections (room moldings, molding lines, countertops, backsplashes) hold resolved inputs.
- Owners by material: ObjectKind / ArchKind gain Countertop, Backsplash, MoldingLine; `incremental::object_kind_of(mesh)` and `probe::arch_kind(mesh)` resolve a painted `paint:countertop/..` / `paint:backsplash/..` key first, then material prefixes (glass-wall and niche-back -> Wall, molding-room- -> Room, molding-line -> MoldingLine, countertop -> Countertop, backsplash -> Backsplash), then phase 17's deck rule and the part fallback. PickHit gains `owner`. Part names: molding, counter, backsplash, glass, cabinet, wall.
- Glass and pony walls: wall type "Glass" (0.01 thick, 2.0 high, material glass-wall); `wall.split { height, lower?, upper? }` keeps the lower part as the wall's own type with its joins and builds the upper part as a centred straight prism of its own thickness. Niches are OpeningKind::Niche (a `niche` command; refused unless the wall is >= depth + 0.012 thick and the sill >= 0.01), cut exactly width x height (`opening::cut_allowance` is 0 for niches and shower doors) and filled back by a back block plus a 1 mm `niche-back:o<n>` skin. Shower doors are door style `shower` (0.7 x 1.9, frameless), only in glass walls or above a glass-topped split.
- Exports: GLB / OBJ nodes molding-r<n>-<kind>-<i>, molding-ml<n>, countertop-ct<n>(-waterfall), backsplash-bs<n>, wall-w<n>-glass, wall-w<n>-niche-o<m>, cabinet-cb<n>-crown | lightrail | frame | shelves | appliance. IFC: IfcCovering SKIRTINGBOARD (base) / MOLDING, one IfcMaterial 'Glass' on glass walls, countertops and backsplashes inside their owner cabinet's IfcFurniture, a niche as an IfcOpeningElement .RECESS. DXF: the arch payload carries the new collections; molding lines on A-FLOR-TRIM.
- Open decisions taken: the template is code, not a file; molding lengths are measured along the wall faces; fillers never resize cabinets; the backsplash's default material is `default:backsplash`; moldings run on solid faces only; the demo house is phase-18-demo.json and the showcase gets the same features additively, its glass walls after the last roomdetect so room ids stay.
- Numbers: trim 0.96 so base 13.04, crown 14.0, chair 13.04; crown 15.2 / 12.0 with the soffit, 23.04 with the tray; island area 1.125, +0.3 x 1.8 = 1.665, segment with R 2.125; backsplash tops 1.372 / 1.364; three drawers 3h + 0.006 = 0.776; 0.762 -> 2 doors, 0.610 -> 1; the oven row 0.737 of a 2.034 face; corner counters 2.046875 (blind) and 2.234375 (lazy susan); the filler run 0.78125; glass probes 1.0 / 2.05; the niche's six probes.

## Phase 19 plan

Phase 19 (light, sky and cameras) was planned against arch3d at febe33d37 and the phase-17 plan. 19 segments in five waves, several split by size into numbered parts; checks 01-17 under tests/acceptance/phase-19 with p19lib.inc (loads p17lib.inc, else p16lib.inc; not phase 18's). Status at the phase-17 checkpoint: p19-foundation 1, 2 and 4, p19-r3d-groundwork-1, the path-tracer groundwork (frame layout, frame lighting), p19-exposure 1-3, p19-sun-2, p19-seeing-in, p19-normal-maps 1-2 and p19-strip-model-1 merged.

- Waves: 0 (no deps) p19-foundation (look data, the `look` command, apply_look_params as the one look parser, `backdrop {kind: sky}`), p19-r3d-groundwork (GPU layout: PbrUniform 128 B, frame uniform 256 B, GpuFrame fields and flags, shader and tracer hook stubs; byte-identical images), p19-horizon, p19-strip-model, p19-normal-maps; 1 p19-exposure, p19-sun, p19-env-path, p19-seeing-in, p19-strip-tray; 2 p19-env-raster, p19-strip-render, p19-normal-maps-render, p19-seeing-in-render; 3 p19-sky, p19-hdri, p19-cameras; 4 p19-pane, p19-demo. Critical path: foundation -> exposure -> env-raster -> sky -> demo.
- The render look: `Look` (crates/doc/src/arch/look.rs) is stored globally (ArchModel.look; its backdrop stays ArchModel.backdrop) and complete per saved camera (Camera3d.look, captured at save; Camera3d.saved for camera.reset). A camera without a look follows the global look. Every render, illuminance and pick3d resolves the effective look and overlays its own params through apply_look_params. Named sun angles live in ArchModel.sun_angles.
- Exposure: a pixel's linear value = luminance (nits) x 1/(1.2 x 2^EV100), then the tone curve (ACES default, Hable, none). Auto: 0.18 / log-average luminance; daylight clamps to [pi/1e5, 0.01], other nights to [pi/1e5, 53.33]; the legacy `render {sun: false}` keeps 0.01. The lit raster is physical (sun and fixtures in lux, ambient from SH or the night stand-in 6.2832 lx) and tone-mapped in the pane as in Finalize; picture backdrops and edge lines stay display colours. The calibration param `albedo` makes every surface Lambertian.
- Environments: render3d Environment (equirect, linear nits, row 0 = zenith, u = 0.5 + atan2(d.y, d.x)/(2 pi)), from an HDRI backdrop or the Preetham sky (HDRI wins); visible only under a Sky or Hdri backdrop, else light only. The path tracer importance-samples it with MIS (tables in the aux `ies` buffer: the tracer stays at 8 storage buffers); the raster uses 9 SH coefficients. The moon is a directional light only with the sun below the horizon and the sky on (arch3d::sky::moon_dir).
- HDR images are stored in the drawing (ArchModel.hdr_images, hex RGBE, <= 1024 x 512 each, <= 4 images / 8 MiB, pruned to what backdrops and camera looks reference).
- The horizon plane is Scene3d.horizon: never in meshes or bounds (fits, shadow bounds, HLR and exports never see it); probes answer part `horizon`.
- Strip lights: LightKind::Strip, sampled every 0.15 m as Lambertian emitters (CPU illuminance; raster: one drawn light with up to 128 segments, never shadow-mapped; path tracer: one segment light each). LightFixture.tray is declared by p19-strip-model. LightKind is matched exhaustively in render3d lights.rs, pathtrace/scene_gpu.rs, io ifc/lights.rs and glb_lights.rs.
- Seeing in: arch3d ViewFilter (cutaway hidden set, clip box, near clip, overview levels) applied to a filtered scene cached per filter key; the clip box also clips shadow passes.
- GPU layout ownership: p19-r3d-groundwork fixed PbrUniform (128 B), the frame uniform (256 B), GpuFrame and the shader hooks; later changes add fields only at the end.
- The CC-0 HDRI is not in the tree: p19-hdri restores the .hdr and its README from commit 576538153 with `git show` (md5 c69498687e876bf68d2a8b8a62234eb9) and adds both ATTRIBUTION rows; check 13 requires the file and its row.
- Earlier checks, landed by the operator: phase-13 checks 02 and 13 leave `-normal` images out of the GLB image lists.
- Check notes: the strip round a tray is its own check 17; check 10 drops the siding case (mirrored-azimuth suns cannot see a row-only tilt; the siding map is checked in 09); check 03(d) uses EV100 8; check 08(e) renders the inline clay/technical views before the global look changes; check 11 requires the demo's all-off light set `Day` and a level dusk camera.
- Review fixes kept: sun_lux zero at night, the meter told about the ground quad, Lambert calibration through PbrUniform.lambert_albedo, the legacy night pin, composite_misses returning early for Sky/Hdri, sky:false pinned on every sun measurement, FRAME_SKY only with look.sky.on, `-normal` image names, library.place of an .hdr item as hdri, set_backdrop for camera pictures.
- Phase-17 names relied on: probe::pick_part and probe::arch_kind, the new parts, ui-egui pane3d.rs proxy_handle/locate, arch3d texture::board_finish. Phase 16: arch ceiling::tray::outline for p19-strip-tray.
- Shared files (union merges; each package edits only its own functions): engine cmd/arch/mod.rs registration, inspect.rs dispatch, uk.tsv rows; render3d Lighting fields; apps/cadcraft-cli/src/render_hook.rs; tracer.rs (groundwork owns frame_lighting; each package fills its own frame_* stub); frame_look.rs.

## Phase 20 plan: plan views and dimensions

Planned against arch3d at 1f9b04c81 (phase 16 merged: trays, soffits, vaults, foundations, ceiling stacks with the "suspended" template) and against the reviewed phase-17 and phase-19 plans, which merge first; phase 18 (planned in parallel) is not a dependency. Kept from the 11-08 draft (written on op/next 15d9af8fa, an ancestor of 1f9b04c81): its 13 packages, its decisions and both review passes, re-checked against this tree (every file, type and function named below exists here or is created by the segment that names it). 13 segments; checks 01-13 under tests/acceptance/phase-20 with the lead's p20lib.inc. Status at the phase-17 checkpoint: p20-observables 1-3 merged.

### Segments, waves, critical paths
- Wave 0 (no deps): p20-observables (plan-view data on CADCraft layer states, the seven templates, Room.label_area, the arch::plan annotation pipeline with stubs, arch.inspect plan / planViews / rooms labelArea + ceilingText) and p20-dims-model (Arch attachments on CADCraft associative dimensions, DXF, AUDIT, arch.inspect dimensions).
- Wave 1: p20-plan-render, p20-plan-views, p20-roof-plan, p20-room-labels (after observables); p20-auto-dims (after observables and dims-model).
- Wave 2: p20-view-export and p20-ui-plan-tabs (after plan-views), p20-reference (after plan-render and plan-views), p20-rcp (after room-labels), p20-dims-drive (after auto-dims).
- Wave 3: p20-demo. Critical paths: observables -> plan-views -> view-export -> demo, and dims-model -> auto-dims -> dims-drive; start p20-dims-model and p20-observables first.
- Check -> segment: 01 plan-views, 02 view-export (plan-views numbers), 03 reference, 04 rcp, 05 roof-plan, 06 room-labels, 07 dims-model (associative), 08 auto-dims, 09 dims-drive, 10 hostile (every command owner), 11 layers/literals/wasm/assets/i18n/user guide (all), 12 demo, 13 observables. Phase-15 check 04 (electrical view) must keep passing unchanged (p20-view-export). No earlier check is rewritten (no earlier_checks).
- Brief items: plan views (named object, level or any, layer state with transparency, annotation set, zoom) -> p20-observables (data) + p20-plan-views (commands); seven templates -> p20-observables; "view.electrical becomes this view" -> p20-view-export; tabs -> p20-plan-views (state) + p20-ui-plan-tabs (UI); create/duplicate/rename/delete -> p20-plan-views; reference display -> p20-reference; reflected ceiling plan -> p20-rcp (+ room ceiling text in p20-observables, its label in p20-room-labels); roof plan -> p20-roof-plan (ridges/hips/valleys are today's linework moved into the pipeline by p20-observables); dimensions that know the building -> p20-dims-model; driving -> p20-dims-drive; auto exterior dimensions -> p20-auto-dims; room labels -> p20-room-labels; drawing the pipeline -> p20-plan-render; demo -> p20-demo. Out of scope (framing plans, story poles, construction lines, shadow plans, interior auto-dimensions other than phase 21's cabinet runs): nobody; p20-auto-dims makes exterior rows only.

### Design
- Plan views extend CADCraft, they don't duplicate it: a PlanView (crates/doc/src/arch/planview.rs, stored in ArchModel.plan_views with the open tabs open_plan_views and active_plan_view) embeds a CADCraft LayerState that gains wildcard rules (LayerRule {pattern, on?, transparency?}: `*`/`?`, case-insensitive, the last matching rule wins; the snapshot's layers win over the rules; layerstate.restore applies rules too) and restores its zoom through the named-view path. No VIEW/LAYERSTATE records are written per view; view.electrical still saves its 'Electrical' state as phase 15 did (the layers from before the view plus those it creates, so the UI toggle undims everything when it turns the view off).
- The seven templates (Working, Floor Plan, Electrical, Reflected Ceiling, Roof Plan, Kitchen & Bath, Site) are implicit defaults: present in every drawing, not serialized until changed; deleting them sticks ("plan_views": []). Floor Plan hides E-*, A-ROOF*, C-*, L-* and keeps A-CLNG (phase 16 draws soffits dashed and footings in floor plans); Reflected Ceiling, Roof Plan and Site hide A-ANNO-DIMS; Electrical is exactly phase 15's rule set. The plan with no view open is unchanged prim for prim.
- Opening a view first creates every layer the plan can draw the model's objects on (cadcraft_arch::plan::layers_in_use: walls, rooms, stairs, railings, terrain C-TOPO-*/C-ROAD, the north arrow, plants, trays/soffits, foundations, devices, casework, views) and the view's annotation layer, because arch_layer_skipped draws objects on a missing layer and wall/room/stair/terrain/plant commands create none; without it a view's rules could not hide them.
- The plan annotation pipeline (crates/arch/src/plan/): annotate(model, view, level) -> PlanAnnotation { lines, labels } (kinds: ridge, hip, valley, gableEnd, eave, rake, slopeArrow, slopeArrowHead, trayOutline, trayTop, vaultLine, ceilingItem, soffit, reference / roomName, roomArea, roomDimensions, roomCeiling, pitch, trayHeight), drawn by render (reference lines beneath the wall poché) and reported by arch.inspect {"plan": true | "<view>"}. It is made at the shown level (plan_level), the level the walls are drawn at, never the view's own level: level.show and export {"level"} change plan_level while a bound view stays active. A view's own level is used only to inspect it unopened.
- Reflected ceiling plan: trays (outline, pitched top ring, height label = the room's base finished ceiling + its depth + its ancestors' depths), vault lines (the roof's ridges, hips and valleys clipped to a vaulted room), soffits (solid), ceiling items (ceiling-placed furnishings' outlines). A room's ceiling text is its base finished ceiling ("2.40", "2.17" under the suspended stack) or, vaulted, its pitch ("6/12"); label blocks use ring_label_point (vertex average, else the farthest-from-edges cell of a 64 x 64 grid, else label_point) and keep out of the room's top-level trays, because label_point cannot leave a hole centred in its ring. Nothing is mirrored.
- Roof plan: eaves and rakes come from the plane rings (unshared ring edges; eave = on the plane's base-edge line), not roof.edges (roofedge.rs lists every footprint edge as Eave, gable sides included); pitch labels read x/12 in every unit system; the slope arrow points along the negative left normal of the plane's base edge.
- Dimensions stay CADCraft DIMLINEAR/DIMALIGNED entities: AssocSnap::Arch {feature: centre | leftFace | rightFace | edge(i), along} on the arch proxy handle (DXF assoc xdata snap code 7); find_feature ranks opening centres, then lines perpendicular to the dimension, never parallel ones; entity snaps still win; AUDIT keeps attachments to existing objects. arch.inspect {"dimensions": true} computes each measurement from the geometry.
- Auto exterior dimensions (ArchModel.auto_dims, crates/doc/src/arch/autodim.rs): per level and side up to three rows on fixed lines beyond the outer face (openings 1.0, walls 1.5, overall 2.0), associative dimensions on A-ANNO-DIMS in the metre-sized style "Arch"; regenerated in the command's own undo step by two post-command passes (detect_edits before assoc, regenerate after; only for successful undoable commands, so undo never marks a row edited); a kept row keeps its handles and is updated in place; a row edited by hand is marked edited and never regenerated (its points still follow associatively). A plan showing one level hides other levels' auto dims; manual dimensions are level-less.
- Driving: dimdrive moves the object attached to the near end of a linear/aligned dimension (walls with their joined and T neighbours, openings along their wall only, cabinets, fixtures, furnishings); the far end stays; it does not re-detect rooms (no CADCraft edit does). Room label text uses "m2" and " x " (stroke font). room.label picks a room's interior or standard area; roomdetect keeps it.
- No new arch object kind, so no handle offset is reserved in phase 20.

### Changes from the 11-08 draft (current code and this run's rules)
- p20lib.inc and the carry-forward checks 10 and 11 are the lead's acceptance files, not p20-observables'; check 07 now loads p20lib.inc too; a new check 13_observables.py gives p20-observables its own fail-on-stub check.
- Phase 16's trays and soffits are merged (seg ids p16-tray-*, p16-soffit-*), so p20-rcp depends only on p20-observables and p20-room-labels; the tray helpers the draft expected (nesting, shapes_in, descendants, top_z) do not exist: p20-rcp walks the parent chain (MAX_TRAY_NESTING). tray::outline/top_ring and soffit::soffit_ring exist with the draft's signatures.
- The check library chain is phase 19 -> 17 -> 16 (p19lib already falls back to 17 then 16); phase 18's library is never loaded.
- stack::template now knows "suspended": the demo uses the name; the checks keep the list form (PV_SUSPENDED), which works either way.
- bilevel.json is on arch3d, so p20-demo also writes docs/arch3d/showcase/phase-20-showcase.json (views for every floor level, auto dimensions on each, applied after bilevel.json), as the brief's demo asks; check 12 (B) replays it and reads the overall rows against the levels' exterior walls (26.2 x 16.2 today) instead of hard-coding them.
- Facts verified with the CLI on this tree: rooms rows' "level" is the level NAME ("levelIndex" is the index), so the checks use levelIndex; a base cabinet `cabinet {"kind": "base", "wall": "w2", "offset": 8.0}` stands at (2.0, 7.9), rotation 180, 0.6 wide (check 07 (d) dimension 1.6, then 2.1); check 07 (d) passes "rotation": 0 because dimlinear's default orientation from `at` was ambiguous there; dimlinear replies {"handle": hex}, line {"handles": [...]}; `select {"handles": ["FFFFFFFF00000005"]}` selects w5; the hip's ridge (6,4)-(4,4) is 2.0 and its hips 5.79828; the gable's planes are -0.1..10.1 x -0.1..4 and x 4..8.1 with four gableEnd edges and four eave edges in roof.edges; trays tr0 0.7..5.35 x 3.65..7.3 and tr1 1.3..4.75 x 4.25..6.7; soffit ring 7.0..9.0 x 7.3..7.9; library items armchair (I-FURN), light-ceiling and light-pendant (E-LITE, placement ceiling); a Level-2 tray under the demo's hip is accepted; the showcase's floor levels are Lower (0) and Main (1).

### Dependencies on phases 17 and 19 (merged before phase 20)
- No phase-20 segment calls a phase-17 or phase-19 function. p20lib.inc execs tests/acceptance/phase-19/p19lib.inc (phase-19 plan; it defines NOON, SITE, CLEAR, at, img, ... and loads p17lib.inc, which defines labels, at, nosings, HOUSE2, ...), else phase-17/p17lib.inc, else phase-16/p16lib.inc; p20lib defines only pv_/PV_ names and INSPECT20, and uses p16lib's HOUSE, house_walls, GABLE, house_rooms, room_row, trays_of, soffits_of, run_ok, made.
- Shared files are registration-only union merges: doc arch.rs (phase 19's ArchModel look/sun_angles and Camera3d fields, phase 17's detail fields; phase 20's plan_views/open_plan_views/active_plan_view/auto_dims and Room.label_area), engine cmd/arch/mod.rs and inspect.rs dispatch lines (p17 inspect_detail/deck_inspect, p19 look/sunangle; p20 plan_inspect, dim_inspect, autodim), xtask/src/literals.rs SHARED_ROOTS (p17 detail.rs, p19 look.rs; p20 planview.rs, autodim.rs), uk.tsv rows, docs/arch3d/USER-GUIDE.md sections, docs/arch3d/showcase/README.md lines.
- render/src/arch.rs build_arch: p17-stair-plan replaced the stair block; p20-plan-render edits only the room-label and roof blocks, the top and the push list. plan::layers_in_use must cover what phase 17 (stair symbols on the stair's layer, deck/railing linework) and phase 19 (strip lights on E-LITE) draw: p20-plan-views reads every arch_* render pass before finishing.

### Shared registration edits within phase 20 (the merger takes unions)
engine cmd/arch/mod.rs (plan_inspect, dim_inspect by observables/dims-model; planview, roomlabel, autodim, dimdrive); inspect.rs dispatch lines (plan x2 + rooms keys by p20-observables, dimensions by p20-dims-model, autoDims by p20-auto-dims); uk.tsv rows (p20-plan-views 7, p20-ui-plan-tabs 7, p20-auto-dims 1, p20-dims-drive 1, p20-room-labels 1); crates/arch/src/lib.rs (plan, dimension, autodim, drive); doc arch.rs (planview module + fields by p20-observables; autodim module + field by p20-auto-dims); doc tables.rs (LayerRule/rules by p20-observables; wildcard_match/apply by p20-plan-views: different items); xtask SHARED_ROOTS (planview.rs, autodim.rs); crates/arch/src/plan/mod.rs (p20-observables; one `pub mod layers` line by p20-plan-views). Nobody edits arch.inspect's `.params` string.

### Verification at planning time
Built cadcraft-cli (debug) at 1f9b04c81 with CARGO_TARGET_DIR=/usr/local/cx-swarm/targets-checks/op-gate and ran every Python check from the repository root (CADCRAFT_CLI pointing at that binary, which the libraries' cli_args honour; the same binary cargo run uses). Each fails on the missing feature, never on a typo or a Python error: 01 `unknown command planview.list`; 02 the Floor Plan SVG still draws E-LITE (#c8a000: export ignores planView); 03 `unknown command planview`; 04, 05, 06 no arch.inspect plan object; 07 no arch.inspect dimensions list; 08, 09, 10 `unknown command dimauto`; 12 phase-20-demo.json missing; 13 no planViews. p20lib's helpers and template table were exercised separately (template spec round trip, SVG colour parsing, handle spelling, the showcase's exterior-wall spans 26.2 x 16.2). All files pass python3 -m py_compile / bash -n. Check 11 ran in full under nice (cargo xtask layers, literals, wasm and assets all passed on this tree) and failed at its first phase-20 item: crates/doc/src/arch/planview.rs does not exist.
- Splits (engineer context budget, files over 300 KB): p20-observables (635 KB) -> p20-observables-1 (doc data: tables.rs, planview.rs, arch.rs, xtask literals; 237 KB), p20-observables-2 (LayerState literal fixes in engine and io; 247 KB, after -1), p20-observables-3 (arch::plan pipeline and arch.inspect; 151 KB, after -1 and -2); p20-auto-dims (482 KB) -> p20-auto-dims-1 (doc AutoDims, arch exterior_rows, render filter, xtask literals; 219 KB), p20-auto-dims-2 (engine dimauto / regeneration / inspect and uk.tsv; 263 KB, after -1). Dependents and `# needs:` lines name every part.

## Phase 21 plan

## Phase 21 plan: schedules, elevations and sheets

Planned from the brief (no earlier draft) against arch3d at d294323a2 (phase 16 merged) and the reviewed plans of phases 17, 19 and 20, which merge before phase 21 starts; phase 18 is planned in parallel and is never a dependency. 20 segments; checks 01-13 under tests/acceptance/phase-21 with the lead's p21lib.inc (it loads phase 20's check library, else 19's, 17's or 16's, and defines only sh_/SH_ names and INSPECT21, so it needs no phase-17/19/20 helper).

### Segments, waves, critical paths
- Wave 0 (no phase-21 deps): p21-observables (all phase-21 data in doc: crates/doc/src/arch/sheet.rs, ArchKind::Cabinet/Fixture, ViewKind::Interior, ArchView room/wall/notes, ViewGeometry patterns/indicators/labels, ArchModel schedules/sheets/sheet_set, RoomLabelShow.numbers, the new plan line/label kinds and the annotate() hooks with stubs, cadcraft_arch::schedule's column tables with stubs, entity-link xdata, arch.inspect schedules/sheets/sheetSet/numbers/view lists) and p21-opening-edit (independent).
- Wave 1 (after observables): p21-view-plumbing (arch3d views module split + stubs + hlr visible_segments), p21-schedule-model, p21-view-render, p21-sheet-command (stubs title_block.rs and sheet_camera.rs), p21-plan-viewports, p21-plot-sheets, p21-callouts.
- Wave 2: p21-schedule-tables and p21-plan-numbers (after schedule-model); p21-interior-elevation, p21-interior-dims, p21-patterns, p21-view-notes (after view-plumbing); p21-elevation-marks (after view-plumbing and schedule-model); p21-title-block and p21-sheet-camera (after sheet-command); p21-ui-callout-open (after callouts and sheet-command).
- Wave 3: p21-demo.
- Critical paths: observables -> view-plumbing -> interior-elevation -> demo; observables -> sheet-command -> title-block / sheet-camera -> demo; observables -> schedule-model -> schedule-tables -> demo. Start p21-observables and p21-opening-edit first.
- Check -> segments: 01 observables; 02 schedule-model + schedule-tables + opening-edit; 03 schedule-model + schedule-tables; 04 plan-numbers (+ phase 20); 05 interior-elevation + interior-dims + elevation-marks; 06 elevation-marks + view-render; 07 view-notes + interior-elevation; 08 patterns + view-render; 09 sheet-command + title-block + plan-viewports + plot-sheets (+ schedule-tables, phase 20 views); 10 callouts + sheet-command (+ phase 20 plan inspect); 11 hostile (every command owner); 12 layers/literals/wasm/assets/uk.tsv/user guide (all); 13 demo.
- Brief -> segments: schedule numbers D01/W01/C01 -> p21-schedule-model; labels in plan on their own layer -> p21-plan-numbers; in elevations -> p21-elevation-marks (+ p21-view-render draws them); schedules as linked, live TABLEs with chosen columns and counts -> p21-schedule-tables; door/window/cabinet/room finish -> p21-schedule-model; opening indicators in elevations -> p21-elevation-marks; interior elevations -> p21-interior-elevation; their cabinet door swings -> p21-elevation-marks; their automatic dimensions -> p21-interior-dims; annotations surviving regeneration -> p21-view-notes; material patterns -> p21-patterns; sheets with plan views, elevations, sections, interior elevations and schedules -> p21-sheet-command (+ p21-plan-viewports for plan views); the 3D image -> p21-sheet-camera; master title block with macros -> p21-title-block; callout markers -> p21-callouts (+ sheet.open in p21-sheet-command, double-click in p21-ui-callout-open); `plot {"sheets": "all"}` -> p21-plot-sheets; the "resize" in the schedule numbers -> p21-opening-edit (no opening edit command existed); demo -> p21-demo. Out of scope (framing plans, revision tables, CAD detail libraries, multi-file projects, LiDAR room capture): nobody.

### What to add to docs/arch3d/DESIGN.md
- Schedules extend CADCraft's TABLE: a Schedule record (ArchModel.schedules: id sc<n>, name, kind door | window | cabinet | room, columns, level filter, at) and one model-space TABLE on A-ANNO-SCHD linked to it by xdata (app CADCRAFT_ARCH_LINK, role "schedule", key sc<n>; links survive handle changes on DXF load and are kept raw by the reader). A post-command pass (Session::after_command, after refresh_views) rewrites a TABLE whose rows changed, inside the command's undo step, and drops a schedule whose TABLE the user erased. Rows come from cadcraft_arch::schedule (pure): identical items share a number (doors incl. garage doors and cased openings: kind, style, width, height; windows: style, width, height, sill; cabinets: kind, width, depth, height, fronts, doors, drawers; lengths to 1e-6), numbered D01/W01/C01 per prefix in order of each group's lowest id, derived on every read (no stored numbers, so a resize regroups at once); room-finish rows read cadcraft_arch::paint::effective_material (floor, wall sides facing the room, ceiling) through material_name. Cells: 3-decimal lengths, 2-decimal areas.
- Schedule numbers are drawn in plan by the phase-20 pipeline (PlanLabelKind::ScheduleNumber on A-DOOR-IDEN / A-GLAZ-IDEN / A-FLOR-CASE-IDEN, openings 0.35 m beyond the wall's right face, cabinets at their plan centre) only in plan views whose labels.numbers is on (Working, Floor Plan, Kitchen & Bath); the plan with no view open is unchanged.
- opening.edit {id, width?, height?, sill?, style?} re-validates with cadcraft_arch::opening::validate_placement; style lists come from opening.rs' valid_styles(kind).
- Linked views: ViewKind::Interior views (elevation {room, wall | side}) store room, room_point, wall and level; arch3d views::interior restricts the scene to the room box in view axes (u across the room, depth from just inside the opposite boundary to the wall face, z floor to finished ceiling, the room's level only, pieces outside a non-convex room dropped) and resolves the room again by room_point after roomdetect renumbers rooms. Every elevation gets patterns (A-ELEV-PATT: courses and joints from the procedural texture tiles, 8 courses per tile, world-anchored, hidden-line clipped through hlr::visible_segments), indicators (A-ELEV-SWNG: door/casement/awning/hopper and cabinet-door swing triangles, apex at the hinge) and tags (A-ELEV-IDEN: schedule numbers in the opening/cabinet front); none of these grows the extent or the overall dimensions. Interior elevations dimension each cabinet run (per cabinet and the run), the overall width and the floor-to-ceiling height. ArchKind gained Cabinet and Fixture, so casework lines in elevations and sections carry their own ids (before, they read as wall w0).
- View notes (ArchView.notes, serialized with the view): text, marker and dimension notes in view coordinates (u = p . (ly, -lx), v = z), or attached to a corner of an object's lines (resolved after every regeneration; detached keeps the last position). Commands view.note / view.dim / view.note.delete change no 3D and do not bump arch_revision.
- Sheets extend CADCraft layouts, viewports, blocks with attributes and the multi-layout PDF: a Sheet record (ArchModel.sheets, in plot order) per layout; one locked viewport per view at 1000/scale paper mm per metre, linked by xdata (role sheet-view, key sheet name, view number); a plan-view viewport is drawn by render from a copy of the drawing with that view applied (its layers created and its rules applied, its level, active_plan_view), so one sheet can show Lower and another Main whatever the session shows; a camera's render is an IMAGE (raster kept in the drawing). The master title block is the block ARCH-TITLE-BLOCK (made once, never overwritten) whose ATTDEF defaults are macros (%sheet_number%, %sheet_title%, %project%, %date%, %scale%); each sheet has one linked INSERT whose attributes hold the expansions, refreshed by sheet, sheet.edit and sheetset. Callouts (PlanLabelKind/PlanLineKind::Callout on A-ANNO-SYMB) mark every placed elevation (outside the building on the viewer's side), section (both cut ends) and interior elevation (in its room) with "<view number>/<sheet number>"; sheet.open {view} and a double-click open the sheet. plot/exportpdf {"sheets": "all" | [names]} feed the existing `layouts` path in sheet order.
- New layers: A-ANNO-SCHD, A-ANNO-TTLB, A-ANNO-SYMB, A-ELEV-PATT, A-ELEV-SWNG, A-ELEV-IDEN, A-DOOR-IDEN, A-GLAZ-IDEN, A-FLOR-CASE-IDEN.

### Dependencies on phase 20 (merged before phase 21; names used exactly as its plan gives them)
- Data (crates/doc/src/arch/planview.rs, p20-observables): PlanView {name, level, kind: PlanKind, layers: LayerState (with rules), annotation: AnnotationSet, labels: RoomLabelShow, reference, zoom}, RoomLabelShow {name, area, dimensions, ceiling} (phase 21 adds `numbers`), template_plan_views() / is_template_plan_views (phase 21 turns numbers on in Working, Floor Plan, Kitchen & Bath), ArchModel.plan_views / open_plan_views / active_plan_view, ArchModel::plan_view(name); LayerState::apply(&self, &mut [Layer]) (p20-plan-views, crates/doc/src/tables.rs).
- Pipeline (crates/arch/src/plan/, p20-observables): PlanLineKind / PlanLabelKind (phase 21 adds ScheduleNumber, Callout and the line kind Callout, with keys "scheduleNumber" and "callout"), PlanLine::new, PlanLabel::new, PlanAnnotation::push_line / push_label, annotate(model, view, level) (phase 21 adds two calls after the room labels), active_view, ring_label_point (interior elevations' room point), plan::ceiling::room_ceiling_text (the room schedule's ceilingHeight); cadcraft_arch::plan::layers_in_use(model, kind) (p20-plan-views; plan viewports create those layers on their copy). Render: build_arch annotates at plan_level with active_view and draws labels/lines with push_plan_labels / push_plan_lines (p20-plan-render): no phase-21 render change for plan labels or callouts.
- Commands: planview / planview.edit "labels" parsing (crates/engine/src/cmd/arch/planview.rs) and plan_inspect.rs planViews rows (p21-plan-numbers adds "numbers"); export {"path", "planView"} (p20-view-export) and arch.inspect {"plan": true | name} (p20-observables) are what checks 04, 09 and 10 observe; phase 20's auto dimensions (model-space dimension entities filtered by plan_level in render) follow a plan viewport through the drawing copy.
- Showcase: docs/arch3d/showcase/phase-20-showcase.json (p20-demo) makes "<level> Floor Plan", "<level> Electrical" and "<level> Reflected Ceiling" for every floor level; the phase-21 demo replays it and check 13 requires those names.
- Checks that need phase-20 code and therefore could not pass here even with phase 21 done: 04 (planview, export planView, arch.inspect plan), 09 (Floor Plan / Reflected Ceiling plan views on sheets), 10 (arch.inspect plan), 13 (phase-20-showcase.json and its view names); 12 needs nothing from phase 20.

### Dependencies on phases 17 and 19
- Phase 17 (p17-scene-plumbing): cadcraft_arch3d::probe::arch_kind(mesh), which hlr.rs and section.rs call; ArchKind::from_part's new casework arms reach elevation lines through it. Phase 17's opening fields (top, grille) are left alone by opening.edit; its fixed door style has no indicator.
- Phase 19 (p19-normal-maps) edits crates/arch3d/src/texture.rs; p21-patterns only adds tile_size(). Saved cameras keep their own look (p19-cameras): p21-sheet-camera renders a camera by name exactly as `render {"camera"}` does.

### What phase 18 (planned in parallel) must keep stable
- Cabinet { kind: CabinetKind, width, depth, height, elevation, position, rotation, level, fronts: Fronts, doors, drawers } and their name() spellings (the cabinet schedule's identity and cells); cadcraft_arch::casework_layout::fronts(model, cabinet) -> Vec<FrontInfo {kind, rect, hinge, ...}> (cabinet door swings in elevations: if phase 18's face layouts replace fronts(), they must still answer it); a cabinet's back-centre position / rotation frame.
- Opening { kind, style, width, height, sill, hinge, swing } and OpeningKind's Door/Window/GarageDoor/CasedOpening (new kinds such as a niche get no schedule number; phase 21 uses matches!, never an exhaustive match); a new door or window style must be added to opening.rs' lists so opening.edit's valid_styles(kind) follows it; phase 18 should not add a second opening edit command.
- cadcraft_arch::paint::{surfaces, effective_material} and the material keys finish:/colour:/item:/default: (room-finish schedules and elevation patterns): room-type finishes derived inside effective_material flow into the schedule unchanged; a "tile" Finish (by name) gets a tile grid pattern automatically.
- Mesh materials wall-<finish> / roof-<finish> / paint:<key> and probe parts "cabinet", "counter", "pull", "fixture" (ArchKind::from_part maps them to Cabinet/Fixture); a glass wall or molding part needs its own from_part arm if it should not read as a wall.
- If phase 18 merges first, its exhaustive struct literals elsewhere are already ..Default (xtask literals); if after, it must give ViewNote/Schedule/Sheet nothing and only respect RoomLabelShow.numbers.

### Shared registration edits (the merger takes unions)
crates/engine/src/cmd/arch/mod.rs (sheet_inspect by observables; opening_edit, schedule, view_notes, sheets/title_block/sheet_camera registrations); crates/engine/src/session/interactive.rs after_command (refresh_schedules by p21-schedule-tables, prune_sheets by p21-sheet-command, one line each after phase 20's passes); uk.tsv rows (opening-edit 1, schedule-tables 3, view-notes 3, sheet-command 6); crates/arch/src/lib.rs (schedule by observables, interior by p21-interior-elevation); crates/engine/src/cmd/layout.rs (check_name pub(crate) by p21-sheet-command, run_plot/params by p21-plot-sheets); crates/engine/src/cmd/arch/views.rs (views_json by observables, run_elevation by p21-interior-elevation, sequential); crates/arch3d/src/views.rs -> views/ (observables adds the Interior arm, view-plumbing moves the file, five packages fill one sub-file each). The existing `viewsheet` (crates/engine/src/cmd/arch/sheet.rs, phase 7) stays as it is; the new sheet commands live in sheets.rs.

### Earlier checks
None is rewritten (no earlier_checks). Phase-7 check 06 (viewsheet + PDF), phase-15 check 04 and phase-20 checks 01/02/13 must keep passing; the packages that touch their code run them.

### Verification on this run
Built cadcraft-cli (debug) at d294323a2 with CARGO_TARGET_DIR=/usr/local/cx-swarm/targets-checks/op-gate and ran every check from the repository root with CADCRAFT_CLI pointing at that binary (the libraries' cli_args honour it), the files copied into tests/acceptance/phase-21 and removed afterwards. Each fails on the missing feature, never on a typo or a Python error: 01 no 'schedules' list in arch.inspect; 02 and 03 `unknown command schedule`; 04 `unknown command planview` (phase 20; with it, the missing scheduleNumber labels); 05 and 07 `elevation` refuses "room" (`side` or `look` is required; 07 also `unknown command view.note` / `view.dim`); 06 no 'labels' list on the view row; 08 no 'patterns' list; 09 `unknown command schedule` / `sheetset` / `sheet`; 10 `elevation` "room" and `unknown command sheet` / `sheet.open`; 11 the set-up is refused (`unknown command schedule` / `sheet`); 13 docs/arch3d/showcase/phase-20-showcase.json is missing. 12 ran in full under nice: cargo xtask layers, literals, wasm and assets passed on this tree and it failed at its first phase-21 item (crates/doc/src/arch/sheet.rs does not exist). Facts verified with the CLI: `cabinet {"kind": "base", "wall": "w5", "offset": 2.5}` stands at (5.95, 5.5) rotation 90, along y 5.2..5.8, counter top 0.914 (height 0.876); in an elevation looking east its lines span u -5.8..-5.2 (u = -y), v 0..0.914, but are tagged wall w0 today; the NW room is r2 (0.1..5.95 x 3.05..7.9) and roomdetect after stretching w5 to x = 5.5 renumbers it r5 (0.1..5.45); arch.wall.stretch moves one end only and `move` does not move arch walls; the default window style is slider, rough opening = size + 0.12; a sheet SVG's viewBox is in paper mm (914.4 x 609.6) and phase-7's south face measures 25.0..229.0 = 204.0 mm at 1:50; `plot {"layouts": [..]}` writes one PDF with /Count n, a /MediaBox [0 0 2592 1728] per page and a /PageLabel per layout name; `tableexport` returns title, header and data rows; `undo {}` exists; paints of a colour are "colour:#rrggbb" material keys.


- Operator: the `# needs:` lines of checks 04, 09 and 10 named phase-20 work in prose; they now name the phase-20 segments (p20-observables, p20-plan-views, p20-view-export, p20-rcp), since the line is split on commas.
- Operator: split for the engineer budget (files over 300 KB): p21-observables (441 KB) -> p21-observables-1 (doc data + view rows, 228 KB), p21-observables-2 (arch plan hooks/schedule tables + io links, 67 KB), p21-observables-3 (arch.inspect, 146 KB).
