# arch3d — design log

Branch `arch3d` (fork of `storytold/cadcraft`, v0.10.0). Running log for "draw a plan, get the house in 3D". One commit per phase, each ending at a ⛔ checkpoint.

The sections for phases 1–15 were condensed at the phase-16 checkpoint (2026-11-11); their full text is `docs/arch3d/DESIGN.md` at commit 97e939f25 and before. The sections for phases 16, 17 and 20 were shortened at the phase-21 checkpoint (2026-11-15); their full text is this file at 131a10fa7. The checks under `tests/acceptance/phase-N/` hold the numbers.

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
| `doc/src/arch.rs`, `doc/src/arch/*` | L1 | `ArchModel` on `Drawing::arch` with `arch_revision`/`touch_arch()`: walls, openings, rooms, levels, roofs, stairs, railings, cameras, views, `site` (location, terrain, modifiers, plants), `light` fixtures, `finish`, `casework` (cabinets, fixtures), `furnishing` + `symbols`/`images`, `paint`, `backdrop`, `defaults`, `electrical` (devices, connections, light sets), `LevelKind`, `Wall.top_level`, `plan_level`. **Phase 16:** `Room.flat_ceiling`/`ceiling_stack`, `Tray` (tr<n>, +0x78000), `Soffit` (sf<n>, +0x80000), `Foundation` (fd<n>, +0x88000). **Phase 17:** `detail.rs` (stair risers/stringers/underside/plan cut and per-side rails; railing `inset`, `host`, `auto`; deck board direction and rails; opening grille, top, sill and casing sides; door style `fixed`). **Phase 20:** `planview.rs` (`PlanView`, `PlanKind`, `AnnotationSet`, `RoomLabelShow`, the seven templates; `ArchModel.plan_views` / `open_plan_views` / `active_plan_view`), `autodim.rs` (`ArchModel.auto_dims`), `Room.label_area`; `doc/src/tables.rs` `LayerState` gains wildcard `LayerRule`s. **Phase 21:** `sheet.rs` (`Schedule`, `Sheet`, the sheet set; `ArchModel.schedules` / `sheets` / `sheet_set`), `ViewKind::Interior`, `ArchView` room / wall / `notes`, `ViewGeometry` patterns / indicators / labels, `ArchKind::Cabinet` / `Fixture`, `RoomLabelShow.numbers`. Phases 18 and 19 (segments merged, checkpoints pending) add `countertop`, `molding`, `look`. Handle offsets by kind below `ARCH_HANDLE_SPAN 0x100000` (decision 10). |
| `geom` | L0 | `skeleton` (straight and weighted/roof skeleton, inset/outset), `region::Arrangement`, `tin`, `cdt`, `tin_ops`. |
| `arch` | L2 | `wall`, `graph`, `join`, `opening`, `symbol`, `room` (detection, `room_at`), `poly`, `area`, `label`, `roof`, `roofedge`, `stair`, `railing`, `sun`, `detect` (ARCHDETECT pipeline + `score`), `terrain` (`SiteSurface`, mesh, contours, volumes, csv, pad, retaining), `plant`, `site`, `light`, `ies`, `casework`, `casework_layout`, `library` (records, search, history, fits, place), `furnishing`, `paint`, `electrical`, `connections`, `outlets`, `grade`. **Phase 16:** `ceiling` (`mod`: the ceiling point query; `stack`, `vault`, `tray`, `soffit`, `overhead`), `foundation`. **Phase 17:** `stair_geom`, `stair_symbol`, `flight_cut`, `deck`, `railing` (`RailSurface`, posts), `railing_auto`, `opening_top`. **Phase 20:** `plan/` (`annotate`, `PlanAnnotation`, line and label kinds, `active_view`; `labels`, `ceiling`, `roof`, `reference`, `layers`), `dimension` (arch attachments), `autodim`, `drive`. **Phase 21:** `schedule` (rows, grouping, numbers), `interior` (the room box of an interior elevation), `plan/tags` (schedule numbers in plan), `plan/callouts`. Phases 18, 19: `countertop`, `backsplash`, `casework_corner`, `casework_face`, `molding_profile`, `molding_runs`, `room_finish`, `hdr`. |
| `arch3d` | L2 | `build`/`SceneCache` → `Scene3d`; `mesh`, `walls` (elevation profiles + holes), `openings`, `floors`, `roof`, `stairs`, `railing`, `probe` + `ray`, `camera`, `pick` (BVH), `edges`/`hlr`/`section`, `site`, `plant`, `fixture`, `light` (illuminance), `incremental`, `finish`, `uv`, `texture`, `casework`, `furnishing`, `paint`, `icon`, `electrical`. **Phase 16:** `ceiling` (`stack`, `vault`, `tray`, `overhead` clipping), `soffit`, foundation meshes, wall-rise meshes; `build3d` `parts`. **Phase 17:** `stair_parts`, `flight_cut`, `railing_parts`, `deck_frame`, grilles and shaped tops in `openings`; `texture::board_finish`. **Phase 21:** `views.rs` became `views/` (refresh, interior elevations, patterns, indicators and tags, interior dimensions, note resolution), `hlr::visible_segments`, `texture::tile_size`. Phase 20 added nothing here. |
| `render` | L2 | `arch.rs` (arch geometry → DisplayList tagged with proxy handles; views with, since phase 21, their patterns, indicators, tags and notes on their layers; the plan of `plan_level`; the stair symbol; the plan annotation pipeline's lines and labels, reference lines beneath the poché, auto dimensions filtered by shown level; since phase 21 a plan-view viewport drawn from a copy of the drawing with that view applied), `arch_site.rs`, `arch_casework.rs`, `arch_furnishing.rs`, `arch_electrical.rs`, `arch_ceiling.rs`. |
| `io` | L3 | `dxf_arch.rs` (`CADCRAFT_ARCH` XRECORD v9, accepts 1–9, payload up to 64 MiB; save-time proxies incl. `CADCRAFT-STAIR` symbol roles), `dxf_site`, `dxf_casework`, `dxf_furnishing`, `dxf_electrical`, `dxf_ceiling`, dimension assoc xdata snap code 7 (phase 20), entity-link xdata `CADCRAFT_ARCH_LINK` kept raw by the reader (phase 21: schedule TABLEs, sheet viewports, title-block INSERTs), `glb.rs` (own glTF 2.0; `node_names`, `shares_a_node`), `obj`, `png`, `step`, `ifc/*`, `gltf_read`, `image_read`, `library` (embedded sample set). |
| `render3d` | L5 | `pipeline` (PBR), `light`, `shadow`, `shadow_atlas`, `edges`, `adapter` (one adapter request at a time per process), `offscreen` (`Style`: shaded, shaded-edges, hidden-line, glass-house, clay, technical), `final_render`, `pathtrace/*` + WGSL `pt/*`, `textures`, `backdrop`. |
| `gpu-optin` | — | The workspace's only unsafe code: wgpu's experimental-feature token for ray queries, a pipeline cache read from disk. |
| `engine/src/cmd/arch/*` | L4 | One module per command family. **Phase 16:** `tray`, `soffit`, `foundation` (+ `.edit`, `.delete`), `room {flatCeiling, ceilingStack}`, `build3d {ceiling, level}` and `parts`. **Phase 17:** `stair` detail params and `stair.edit`, `deck {boards, rails}` and `deck.edit`, `railing {postSpacing, inset}`, `window {grille, top}`, door style `fixed`, `arch.inspect {decks}`. **Phase 20:** `planview` (+ `.edit`, `.delete`, `.open`, `.close`, `.list`), `room.label`, `dimauto`, `dimdrive`, `export {planView}`, `view.electrical` as a plan view, `arch.inspect {plan, planViews, dimensions, autoDims}`. **Phase 21:** `opening.edit` (`opening_edit.rs`), `schedule` / `.edit` / `.delete` (`schedule.rs`), `elevation {room, wall | side}` (`views.rs`), `view.note` / `view.dim` / `view.note.delete` (`view_notes.rs`), `sheet` / `.edit` / `.delete` / `.open` and `sheetset` (`sheets.rs`, `title_block.rs`, `sheet_camera.rs`), `plot` / `exportpdf {sheets}` (`cmd/layout.rs`), `arch.inspect {schedules, sheets, sheetSet, numbers}` (`sheet_inspect.rs`); post-command passes `refresh_schedules` and `prune_sheets`. Phase 7's `viewsheet` (`sheet.rs`) is unchanged. `engine::library` (catalogs, `LibraryIo`). |
| `ui-egui` | L5 | `pane3d.rs` (dockable 3D pane, six styles, walkthrough, pick → 2D selection, Finalize, own `SceneCache`), `archdetect_review.rs`, `library_panel.rs`, `electrical_tools.rs`; the plan-view tab strip (phase 20); a double-click on a callout runs `sheet.open` (phase 21). |
| `cadcraft-cli` | — | `convert` (detects first when there are no arch objects; `.glb`, `.obj`, `.ifc`), `run --cmd / --commands FILE.json / --export`, `render … --camera --style --sun --ground --adapter --final [raster|path]` (exit 3 `NO_ADAPTER`), the session renderer (`render_hook`; it also renders a sheet's camera image). |

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
10. **Proxy handles, not entities:** `ARCH_HANDLE_BASE = 0xFFFF_FFFF_0000_0000` + kind offset + id. Offsets: walls `+0`, openings `+0x8000`, rooms `+0x10000`, roofs `+0x18000`, stairs `+0x20000`, railings `+0x28000`, views `+0x30000`, terrain `+0x38000`, plants `+0x40000`, light fixtures `+0x48000`, cabinets `+0x50000`, fixtures `+0x58000`, furnishings `+0x60000`, electrical `+0x68000`, connections `+0x70000`, trays `+0x78000`, soffits `+0x80000`, foundations `+0x88000`; phase 18: countertops `+0xC0000`, backsplashes `+0xC8000`, molding lines `+0xD0000`. Phases 20 and 21 add no offset: schedules, sheets and title blocks are real CADCraft entities (TABLE, layout, viewport, INSERT, IMAGE) linked to records. JSON spells every handle one way (`handle_json`). `grips.rs::arch_id_from_handle` is the single disambiguator.
11. **Rough vs finished separate** (`ROUGH_ALLOWANCE` one constant).
12. **Placement limits, area rules and code limits are data in `arch`**, so commands/CLI/MCP/`ARCHDETECT` agree. Commands validate through these and **refuse — never clamp**.
13. **Shared structs** are built outside their declaring file only as `Type{required,..Default::default()}` or via a constructor; `cargo xtask literals` enforces it.
14. **Derived, never stored,** when a quantity follows from others (`Level::floor_to_floor()`; grade classes; the finished ceiling; a deck's frame; a room's ceiling text; schedule numbers and rows).
15. **Probing, not counting, proves geometry.** Any new 3D feature adds probeable solids.
16. **Roofs are generated, then owned.** A `ROOFAUTO` roof regenerates while its walls change; `roofplane {edit}` sets `hand_edited`. Generation is all-or-nothing.
17. **A stair's well is derived from headroom, and the hole is applied from the stair** (`well_holes_for`); the code never invents a room to hold a hole.
18. **One camera, below the renderer.** The camera math lives in `arch3d` and `render3d` re-exports it; `ray_at` inverts `project_point`.
19. **Picking is a CPU ray cast** against `Scene3d` (BVH, deterministic ties). The struck *part* picks the handle range, since ids number per kind.
20. **WebGL2 limits everywhere.** Devices use `downlevel_webgl2_defaults()` (the path tracer asks for its own compute device); shadow maps are read with `textureLoad`; `--adapter fallback` always exists.
21. **Views are linked objects stored with their geometry** (`ArchView` + generated lines, rings, dims, and since phase 21 patterns, indicators, tags and notes, regenerated in `after_command` inside the command's undo step). `render` cannot call `arch3d`, so it draws the stored geometry.
22. **`ArchKind` travels with every generated line and ring;** `ArchKind::from_part(part_of(mesh))` is the one mapping. Since phase 21 casework maps to `Cabinet` / `Fixture`, not to a wall.
23. **Arch linework shows through paper-space viewports** and so in layout plots.
24. **One meaning of opening offset (the centre), and arch drawings are metre drawings** (`$INSUNITS 6`); accepting detection candidates in a non-metre drawing converts the whole drawing in the same undo step.
25. **Detection is a pure pipeline in `arch::detect`**; candidates carry confidence and evidence; layer names raise a score and are never required.
26. **The terrain is a surface; the mesh samples it.** `SiteSurface` is the one answer to "how high is the ground here". The mesh must be closed, and every mesh change carries a closed-mesh test.
27. **Site location is data the sun reads** (`north_direction`, `true_to_drawing`).
28. **Site objects have their own handles and probe parts.**
29. **The path tracer accumulates sums; every reader divides.**
30. **GPU process state is touched one thread at a time.** Headless instances are made under one lock, never with wgpu's DEBUG flag; adapter requests also take turns.
31. **Every export names one object alike and is reproducible** (`glb::node_names`; GlobalIds are hashes of keys; `SOURCE_DATE_EPOCH`; no `HashMap` iteration order in output).
32. **A writer of a text format is tested by parsing what it wrote** (`crates/engine/tests/ifc_step_grammar.rs`); a new IFC entity adds its count there.
33. **An incremental build is the full build, cached by its inputs.** A new 3D object kind adds its section, its inputs, and its edits to `crates/engine/tests/scene_cache_equals_full.rs`.
34. **A timing bar measures the work, not the host,** and a performance check also bounds the work done.
35. **A process-wide hook has one installer** (`LibraryIo`, `IoHooks`: `OnceLock`); unit tests use `library::testing::install_io`.
36. **Coverage comes from what the kernel knows** (the path tracer's per-pixel hit count decides where the backdrop goes).
37. **Meshes compare within a tolerance** (1e-5), never by rounded keys.
38. **Levels are free; a wall's vertical extent is derived.** `Wall::spans_level` is the one selector; `bottom_z`/`top_z` the one source of heights.
39. **Up and down come from elevations, never from level order.** An integer `level` param is the level's index.
40. **Grade classes and areas are computed on read.**
41. **A light set is a view of the lights; a circuit's brightness is an edit.**
42. **A check runs against today's CLI before it is locked, and must stop at a product assertion** (an exception, or a pass, is a check defect).
43. **A scripted model is checked for where things are, and a test segment never edits what its test judges.** Containment is measured against **walls**, and the lead writes the assertion's geometry (which box, which room) into the spec.
44. **A room stores its rough ceiling; the finished ceiling is derived.** Finished = rough − the finish stack, or the roof underside for a vaulted room, plus trays, minus soffits. One point query in `arch::ceiling` answers it everywhere.
45. **What lies over a point is one query** (`arch::ceiling::overhead`): the floor of the level above, an Open Below over it, or the roof underside. Room, ceiling and level edits that would cross it are refused naming the room; roof edits are never refused, and leave a clipped ceiling and a warning.
46. **Open Below rooms are owned, not detected.** `roomdetect` keeps them, with their ids, and cuts them again.
47. **Foundations are derived from the exterior walls with no level below them.** `FOUNDATION` stores a type and a footing depth and creates no walls.
48. **A demo the brief puts on the showcase house goes on the showcase house.** Phase 16 did not; phase 17 did (`p17f-showcase-phase17`); phase 18 did (`p18i-showcase-phase18`); phases 20 and 21 did through `phase-20-showcase.json` and `phase-21-showcase.json`, replayed after `bilevel.json`.
49. **A stair's `start` is the centre of its bottom nosing line, everywhere.** One module (`arch::stair_geom`) gives tread spans and the nosing line.
50. **Automatic objects are made by the command that makes their host, never on load.** A drawing saved before the feature keeps exactly what it had.
51. **A mesh's owner comes from its material prefix, its part from `part_of`.** A part name may be shared by kinds (`post`, `footing`), so a per-object count is read from `arch.inspect`.
52. **A merge message is not evidence.** A segment's stated deliverable (a guide section, a test, a README line) is confirmed in the tree at the checkpoint. At the phase-21 checkpoint the three phase-21 guide sections are present with runnable examples; the phase-17 guide text is still absent.
53. **A plan view is a CADCraft layer state with rules, not a second layer system.** The templates are implicit defaults, and the plan with no view open draws as before (but see decision 60 for callouts).
54. **Plan annotation is one pipeline, made at the shown level.** `arch::plan::annotate(model, view, level)` produces every generated plan line and label (room labels, roof plan, reflected ceiling, reference display; phase 21's schedule numbers and callouts). `render` draws it, `arch.inspect {plan}` reports it and `export {planView}` writes it.
55. **Building dimensions are CADCraft dimensions with an arch attachment.** Auto rows are ordinary associative dimensions regenerated inside the command's own undo step; a row edited by hand is never regenerated. Measurements in `arch.inspect` are computed from the geometry on read.
56. **A script that a later phase builds on is replayed against the commands as built before that phase scripts on it.** It happened twice in phase 21: the plan-view steps (`p21f-showcase-planviews`) and the drawing-set script itself (`p21g-showcase-drawing-set`). A demo script is replayed whole by its own segment's gate.
57. **A schedule is a CADCraft TABLE linked to a record; its content is derived.** The `Schedule` record holds kind, columns, level filter and position; the TABLE is found by link xdata (`CADCRAFT_ARCH_LINK`, role and key), which survives handle changes on load. Rows, grouping and numbers come from `arch::schedule` on every read, so nothing stored can go stale. A post-command pass rewrites a TABLE whose rows changed, inside the command's undo step, and drops a schedule whose TABLE was erased.
58. **A sheet is a CADCraft layout; its content is ordinary entities linked to the sheet record.** Locked viewports, a title-block INSERT of the block `ARCH-TITLE-BLOCK` (made once, never overwritten; macros expand into attributes), an IMAGE for a camera render. The multi-page PDF is the existing `layouts` plot path fed in sheet order. No second layout, viewport, table or plot system.
59. **A viewport of a plan view draws a copy of the drawing with that view applied** (its layers created, its rules, its level, `active_plan_view`), so sheets can show different levels and views whatever the session shows, and the session is never touched.
60. **Annotation on a linked view belongs to the view, not the model.** Notes are stored in view coordinates or attached to a corner of an object's lines and resolved after each regeneration; they change no 3D and do not bump `arch_revision`. Likewise callouts belong to placed sheets: they draw in the plan whether or not a plan view is open (`p21g-callouts-without-view`), which is the one exception to "no view open draws as before".
61. **View extras never change a view's extent.** Patterns, indicators, tags and notes sit on their own layers and are left out of the extent and the overall dimensions, so phase 7's numbers hold.

## Phase 1 — walls, joins, poché, first 3D (landed)

`solve_joins` trims/extends centre-line ends to neighbour face-line intersections (L/T/X/angled/collinear); `stretch_wall` is a capped transitive BFS; `render::arch` expands solved rings + layer bands; `arch3d::build` extrudes elevations with `object: ArchId` and box UVs; `io::glb` writes glTF 2.0; wall grips run `arch.wall.stretch`. Checks 01–09 PASS. Deferrals: `is_main` joins, pony walls (phase 18), `reversed`, arc walls.

## Phase 2 — openings (landed)

`Opening` (host, offset = centre, finished + rough dims, style, hinge/swing, frame/casing/sill). `arch::opening` holds the limits (`MIN_HINGED_WIDTH 0.3`, `MIN_GARAGE_WIDTH 1.8`, `MIN_SEPARATION 0.05`, `ROUGH_ALLOWANCE 0.12`) with `validate_placement`/`place_opening`. Generated plan symbols; frame/casing/glass/leaf/sill/header meshes. `door`/`window`/`garagedoor`/`arch.opening.move`. Checks 01–09 PASS. Deferrals: bay/box/bow geometry, mullions, hardware, pocket doors, arc-wall openings. (Schedules and `opening.edit` came in phase 21.)

## Phase 3 — rooms, floors, ceilings, levels (landed)

- `render3d` really draws (depth, one light + ambient); `render` exits 3 with `NO_ADAPTER`; `build3d {probe}` via `arch3d::ray`.
- `Level{elevation, ceiling_height, structure}`, `floor_to_floor()` derived (2.4 + 0.3 = 2.7).
- `detect_rooms`: faces of the solved poché, dividers as centre lines, holes from islands.
- Areas (`arch::area`): interior, standard, living.
- 3D: a floor slab per room, a 20 mm ceiling solid under the ceiling height; Open Below no floor; Deck/Porch no ceiling.
- Numbers: interior 27.93 / 28.3725 / 18.6725; standard 31.62 / 31.11 / 20.91 = 83.64; loop → 26.72 + 0.81; Level 2 probes (2,1,2.55) floor, (2,1,1.0) nothing.
- Checks 01–09 PASS. Lesson: run every check against a stub before locking it.

## Phase 4 — automatic roofs (landed)

- `Roof{planes, edges, pitch, overhang, thickness, fascia, soffit, hand_edited}`; per-wall `roof_directive`/`roof_pitch`/`roof_overhang`.
- Generator: footprint from the outer faces of the solved poché of the top level's exterior walls, `geom::roof_skeleton`, overhang by `outset`. All-or-nothing (`RoofError`).
- Wall-to-roof clipping and opening holes on the wall's elevation profile (`mesh::push_profile_prism`), no CSG. One `underside_z` serves the roof solid, fascia, soffit and the wall-top clip.
- Numbers: hip 6/12 ridge x 4…6 at z 4.45; 4/12 3.766667; 12/12 6.5; two gables ridge −0.1…10.1; overhang 0.5 → 103.04 m²; L one valley, T two.
- Property tests on the invariants. Checks 01–11 PASS.
- Gaps: dormers, crickets, Dutch/half-hip, returns, skylights, gutters, interactive plane joining, per-plane material.

## Phase 5 — stairs, railings and decks (landed)

- `plan_stair` (closest to 168.75 mm under `MAX_RISER 0.196`; `MIN_TREAD 0.254`; `MIN_HEADROOM 2.032`), `headroom_ok`, `well_start`, `flight_polygon`, `well_polygon`, `well_guard_path`; railing limits (`GUARD_HEIGHT 0.914`, handrail 0.864–0.965, `MAX_BALUSTER_GAP 0.1016`).
- `stair` cuts the well in one mutation; `deck` is a `RoomType::Deck` room with `deck_height`.
- Numbers: 2.7 → 16 × 0.16875, 15 × 0.254, run 3.81; deck 0.9 → 5 × 0.18; well area 3.302.
- Checks 01–09 PASS (03 and 07 rewritten for phase 17's parts and hosts).
- Still open: L and U shapes in 3D, no stair/railing grips. Out of scope: winders, curved/spiral, ramps, goosenecks.

## Phase 6 — the 3D view, properly (landed)

- `arch::sun` (NOAA form); PBR; one 1024² sun shadow map; MSAA; styles shaded, shaded-edges, hidden-line, glass-house.
- `Camera3d` saved in the model (`camera {save|use|delete|list}`), full camera + `walkthrough`; `pick3d` on a CPU BVH; `render --style --sun --ground --adapter fallback`.
- Numbers: 51.05° N, 2026-06-21 solar noon → elevation 62.39°, azimuth 180°.
- Checks 01–09 PASS.
- Gaps: no sun in the pane (phase 19's looks), hard-edged shadow, `camera`'s echo form in radians.

## Phase 7 — generated elevations and sections (landed)

- `ArchView` (elevation or section) with stored `ViewGeometry`; `arch3d::views::refresh` uses the assisted `hlr` and `section` kernels (no second one).
- `elevation`, `section`, `view.delete`, `viewsheet` (a layout with one locked viewport per view at 1000/scale). Proxies `CADCRAFT-VIEW` on save.
- Numbers: south elevation 11.2 m wide, 5.474 m high; door 0.9 × 2.1; section rings 0.48 m²; ARCH D PDF 2592 × 1728.
- Checks 01–09 PASS.
- Gaps: scope is a plan rectangle. (View titles, the title block, interior elevations, patterns and notes came in phase 21.)
- Planner rule: a spec that points at internals names them, lists their file, and says to expose them when private.

## Phase 8 — recognition from plain DXF (landed)

- `arch::detect`: `flatten` (INSERTs to depth 16, arcs refitted) → `walls` (parallel faces 0.04–0.6 m apart, merged across gaps ≤ 2.6 m unless a crossing face spans the gap) → `junctions` → `openings` → `rooms` (labels as names) → `layers`.
- `archdetect {dryRun | accept, minConfidence, units, layers}`; `convert` runs it first; the Detection Review panel.
- `geom::region::Arrangement` built once per group (450 s → 353 ms on 11,271 entities).
- Numbers: apartment.dxf P = R = 1.0; synthetic walls 1.0/1.0, doors 1.0/0.958, windows 0.972/1.0.
- Checks 01–10 PASS; 07 reports the three real plans and does not gate (the two Moorfields plans find 1 and 0 walls).
- Gaps: unitless plans read as metres; no editing of candidates; curved walls.

## Phase 9 — terrain and site (landed)

- `SiteLocation`, `Terrain` (boundary, points, elevation lines, modifiers Flat/Hill/Path, pad), `Plant`/`PlantSpecies` (14 built in), `Wall.retaining/freeboard/footing`.
- `SiteSurface` (decision 26) → `site_mesh` → terrain solid with a 1 m skirt. Pads flatten to first floor − structure with a margin and a 1:2 slope; basements cut.
- Plan on `C-TOPO-*`, `C-ROAD`, `L-PLNT`, `A-ANNO-NRTH`. `site`, `terrain` (+ `.modify`, `.query`, `.contours`, `.volumes`, `.delete`), `plant`, `plant.species`.
- Numbers: 1.25 at (25,12); contours x = 5…35, major at 20 labelled 1.00; cut/fill 6.25/6.25; retaining 2.5 / 1.4995, top 2.65.
- Checks 01–12 PASS.
- Gaps: no terrain/plant tools or grips in the UI; one label per major contour; the pad slope is sampled.
- Planner rules: a spec lists every file it leans on; each required test gets its numbers; a mesh refinement is points unless a kink needs an edge.

## Phase 10 — lighting, light fixtures and final renders (landed)

- `LightFixture` on `E-LITE` (`lf{n}`), catalogue and photometry in `arch::light` (800 lm in 60° = 950.36 cd). Housings; ceiling cut-outs for recessed cans.
- `arch3d::light::illuminance` (direct, shadow rays). The lit raster: a shadow atlas cached per light key and occluder key.
- `FinalRender`; the CLI's session renderer; `sunstudy`.
- Numbers: 165.0 lx at (1,1,0), 122.1 at (1.8,1.8,0), 0 behind the column.
- Checks 01–11 PASS.

## Phase 11 — ray-traced Finalize Render (landed)

- `pathtrace` on its own compute device; without compute shaders the raster Finalize with `fallback: "no compute shaders"`.
- Units in nits; NEE + MIS, Russian roulette; Lambert + GGX, mirrors, glass. IES LM-63 (`arch::ies`); area lights. À-trous denoiser.
- Hardware ray queries behind `rt` (`gpu-optin`), verified equal to the compute path on llvmpipe and an RTX 4070. Vulkan pipeline cache.
- `final: true` is path-traced, 256 spp (64 on a CPU adapter).
- Checks 01–13 PASS.

## Phase 12 — exports and interoperability (landed)

- `io::step`; `io::ifc` (IFC4: project → site → building → storeys; walls, openings with void + fill relations, slabs, roofs, stairs, railings, spaces with areas, site, lights); bodies are `IfcTriangulatedFaceSet`; GlobalIds from keys; `SOURCE_DATE_EPOCH`.
- `io::obj` + MTL. `export {path | format}`. `ifc_step_grammar.rs` (decision 32).
- Checks 01–15 PASS.
- Gaps: no IFC materials or colours; triangle bodies only; `OwnerHistory` `$`.

## Phase 13 — polish (landed)

- **Finishes:** `Finish` on walls (exterior face), roofs, floors; `material`; 256² procedural textures in `arch3d::texture`.
- **Casework:** `Cabinet` (base, wall, tall; counter at 0.914), `Fixture` (eight kinds); plan symbols; IFC classes.
- **Incremental rebuild** (decision 33): `SceneCache`, `ObjectKey`, `Scene3d::digest`; `build3d {incremental}`.
- **Benchmark house:** `arch.sample {house}` (310 walls, 160 openings, 108 rooms): full build 16.8 ms, median update 11.4 ms with 13 objects rebuilt.
- **Docs:** USER-GUIDE.md `sh` blocks are run by check 09; `docs/arch3d/mcp-examples/` replays.
- Checks 01–13 PASS.

## Phase 14 — the library (landed)

- **One library, nine categories;** `ItemRecord`; catalogs `builtin`, `sample` (49 items, embedded), `installed:<name>`, `local`, `user`. An item with no licence never loads.
- **Search:** word substrings + AND chips; `FilterHistory`; saved filters; Favorites; `library {for: surface key}`.
- **Symbols:** `io::gltf_read` (plain .glb only); `SymbolMesh` keyed by file hash, stored in the drawing.
- **Furnishings:** `Furnishing` (f<n>), layers by category, IFC class by category, convex-hull plan outline.
- **Placing** (`arch::library::place`), in order: part swap, faucet on sink, cut-out item into a base cabinet, appliance into a gap, wall item on the nearest face within 0.3 m, ceiling item at the hang height from `arch::ceiling`, else at the click.
- **Cabinet parts:** `casework_layout` (fronts, pulls, counter runs, gaps, false fronts); slab, shaker and raised-panel fronts.
- **Painters:** surface keys, `ArchModel.paints`, scopes component/object/room/floor/plan, `blend` tint, eyedropper.
- **User catalog:** `library.add`, `library.copy`, `.edit`, `.delete`, `.trash`. `arch.default`.
- **Icons:** `arch3d::icon` (CPU rasterizer, byte-identical). **Backdrops:** colour, gradient, image.
- Checks 01–13 PASS.
- Open: `move` silently moves nothing for handles it cannot resolve; `arch.inspect` prints furnishing handles in the short form; `library_user::relative_value` indexes and unwraps; hull outlines; symbol lights do not emit; the path tracer uses texture means; flat icons.

## Phase 15 — electrical, split levels and the showcase house (landed)

- **Electrical:** `ElectricalDevice` (`el<n>`) on a wall face at 0.3 / 1.2 m, counter outlets at counter top + 0.15; plan symbols on `E-POWR`/`E-LITE`; cover plates; `IfcOutlet`/`IfcSwitchingDevice`. `Connection` (`cn<n>`) arcs; circuits; `circuit.brightness`. `outlets.auto` (1.8 m rule; kitchen GFCI 1.2 m / 0.6 m). `view.electrical`, `level.show`, `export {level | view}`, light sets.
- **Levels:** free elevations, `LevelKind` floor | landing, `Wall.top_level` spanning walls (decision 38). **Grade** (`arch::grade`, decision 40). **Pads** for landings, side splits and an entry apron. **Styles:** Clay and Technical.
- **Showcase:** `docs/arch3d/showcase/bilevel.json` replayed by `cadcraft-cli run --commands`; `tools/showcase/render_all.py`; `crates/engine/tests/showcase.rs`.
- Checks 01–14 PASS after nine were corrected by the operator (decision 42).
- **Still open:** `ifc_step_grammar.rs` has no counts for the electrical entities; connections are not exported; a level edit rebuilds every object.

## Phase 16 — ceilings and foundations (landed)

20/20 checks on 2026-11-11 after two fix cycles and the showcase rework. 37 segments and 2 upstream syncs; the lead revised the checks twice (371fd74a6, 7893ad59c); the operator landed the earlier-phase checks that phases 16–19 change (bfaa016c6).

- **Observables:** `build3d {"ceiling": [[x,y], …], "level": n}` (finished ceiling elevation, `null` outside every room); `build3d` `parts` (meshes per part); `arch.inspect` ceiling rows and `trays`, `soffits`, `foundations`.
- **Ceiling build-up** (decision 44): `Room.ceiling_height` is the rough ceiling; `Room.ceiling_stack` is layers top to bottom (an air gap has no material). An empty stack keeps the phase-3 20 mm solid. Templates `drywall` (12.7 mm) and `suspended` (drywall 12.7, air 200, tile 16).
- **Vaults and the guard** (decision 45): `room {flatCeiling: false}` follows the roof underside; every bounding wall rises to it (wall-rise meshes in their own incremental section); ceiling items hang plumb. Edits that would put a flat ceiling above the underside are refused naming the room; roof edits never are, and leave `ceilingWarning`.
- **Overhead:** ceilings and wall tops are clipped at the slab underside of the level above; under an Open Below room the ceiling is that room's.
- **Trays:** `Tray` (`tr<n>`): a room or a drawn polygon, `inset`, `depth`, `sides`; nest; refused whole if the top would cross the roof underside or the room has a level above. `tray`, `tray.edit`, `tray.delete`.
- **Soffits:** `Soffit` (`sf<n>`): rectangle or polygon, bottom elevation or depth, own material (`soffit-box…`); snaps to wall faces; `soffit {over: [cabinets]}`; dashed on `A-CLNG`; ceiling items hang from its underside and are re-seated when it moves.
- **Foundations** (decision 47): `foundation {type: stem | slab, footingDepth?: 1.2}` (`fd<n>`). Stem: 0.2 m concrete wall flush with the wall above down to a 0.6 × 0.25 m footing. Slab: 0.1 m slab with a 0.3 × 0.3 m thickened edge. Basement walls take "Concrete 0.2" and a footing under the basement floor structure. Footings dashed in plan, filled in sections.
- **Rebuild and exports:** new incremental sections; `scene_cache_equals_full` gains the edits; IFC `IfcFooting`, foundation `IfcWall`, soffit `IfcCovering` (CEILING); DXF proxies (`io::dxf_ceiling`).
- **The showcase rework (checks 14–20):** stairs and wells by elevation (decision 39); owned Open Below rooms (decision 46); above-grade area by Chief's living-area convention; technical silhouettes from the camera; Clay and Technical in the pane; the split entry's flights and Foyer; 20 renders, `contact-sheet.png` and `manifest.json` committed.
- **Demo:** `docs/arch3d/showcase/phase-16-demo.json` (a 12 × 9 m house with a vaulted Living room, a suspended Utility ceiling, a Dining tray, wall cabinets with `soffit {over}`, a stem foundation). USER-GUIDE.md "Ceilings and foundations".

### Numbers locked
Phase-1 house, walls 2.4, 6/12 gable roof with gables on x = 0 and x = 10 (underside z = 2.4 + 0.5·(y + 0.1) for y ≤ 4).
- **Vault:** 3.45 at (5,2), 2.70 at (5,0.5), 2.4 at (3,5); wall hits at (5,3.0,3.9) and (5,0.05,2.45); a ceiling item at (5,2) tops at 3.45.
- **Tray:** 2.7 at (3.025,5.475), 2.4 at (0.4,5.0); nested 2.85; depth 0.5 refused (2.9 against 2.8 at y = 7.3).
- **Suspended:** 2.1713 at (8,5); `ceiling` at (8,5,2.18), nothing at (8,5,2.3).
- **Soffit** x 7…9, y 7.3…7.9, bottom 2.1: hit at (8,7.6,2.2), nothing at (8,7.2,2.2); a ceiling item tops at 2.1.
- **Stem:** wall at (5,0,−0.5), footing at (5,0.25,−1.0), nothing at (5,0.25,−0.5) and (5,0.35,−1.0); basement footing at (5,0.25,−3.1).
- **Slab:** (5,1,−0.05) slab, (5,0.15,−0.2) edge, nothing at (5,0.25,−0.2).
- **Wells** x 1.5…2.5, y 1.508…4.81 (3.302 m²) whatever the level order. **Landing:** ceiling 2.4, walls stop at 2.7; with the foyer, Main's room is 68.175 m² and the landing's ceiling 5.1.
- **Above-grade area** of the phase-1 house: 74.975 → 56.3025 → 27.93 → 0 as rooms become garage, porch, deck.

### Phase-16 gaps
- **Out of scope by the brief:** manual ceiling planes with their own pitch, barrel and shelf ceilings, framing and trusses, garage curbs, piers and grade beams.
- **The demo is not on the showcase house** (decision 48). At the phase-21 checkpoint `bilevel.json` still has no `tray`, `soffit`, `foundation` or ceiling-stack step.
- **The showcase layout** was found at this checkpoint not to be a designed house (rooms outside the shell, cabinets outside the kitchen, loose containment tests in `showcase.rs`). Since then `bilevel.json` has grown (23 `wall`, 15 `room`, 14 `cabinet` steps, the phase-18 kitchen and bath features) and `showcase.rs` gained `the_showcase_has_windows_on_every_side` and a phase-18 test; the layout and the containment tests were not re-audited at the phase-21 checkpoint.
- **Smaller items:** no grips on trays, soffits or foundations; a level edit rebuilds every object; `ifc_step_grammar.rs` lacks `IfcFooting` and the electrical entities.

## Phase 17 — stairs, decks, railings and openings in detail (landed)

14/14 checks on 2026-11-12 after one fix cycle. The 13 planned segments, the fix cycle (p17f-railing-auto-contact, p17f-grille-top-part, p17f-showcase-phase17), two upstream syncs and harden passes; checks revised once (3e4c8b77e).

- **One stair convention** (decision 49): tread k has its top at k·rh and spans u in [(k−1)·td − 0.025, k·td]; nosing_v(u) = rh + (u + 0.025)·rh / td.
- **Stairs in detail:** treads 25 mm with a 25 mm nosing; risers closed or open; stringers `two`, `centre` or `none` (a skirt on a wall side). `stair {…, openRisers?, stringers?, underside?, railLeft?, railRight?, planCut?}` and `stair.edit`.
- **Wall under a flight:** `arch::flight_cut` finds the walls; `arch3d::flight_cut::apply` cuts the wall's elevation profile `underside` (default 0.3 m) below the nosing line. No CSG.
- **Plan symbol** (`arch::stair_symbol`): solid nosing lines under the plan cut (`Stair.plan_cut`, default 1.2), a break line, dashed beyond, a walk arrow `UP`; on the upper level the part seen through the well, `DN`. In the plan, exports and DXF proxies (`CADCRAFT-STAIR`).
- **Railings as built:** 90 mm posts at ends, corners and turns and at most `post_spacing` (default 1.8) apart in equal bays; top rail; bottom rail 0.05 m up; balusters or a panel; `inset`. Fields `inset`, `host`, `auto`.
- **Automatic railings** (decision 50): `railing_auto::regenerate` from `stair`, `stair.edit`, `deck`, `deck.edit` only. Per side `auto`, `on` or `off`. Wall contact is real contact with a wall face.
- **Decks you can stand under:** the frame is derived (`arch::deck::frame`, `arch3d::deck_frame`): ledger, 38 × 184 joists in bays ≤ 0.406, rims, a 76 × 235 drop beam 0.3 m back, 0.14 m posts (ends 0.3 m in, bays ≤ 2.4) on 0.3 m piers, 25 mm boards. No wall: joists span the shorter side with a beam at each end. Grade is `SiteSurface` or z = 0. `deck {boards?, rails?}`; `arch.inspect {decks}`.
- **Openings in detail:** grilles (`grille {across, down, part?: "top"}`, 20 × 20 mm bars); tops `square`, `round`, `segment`, `roof` (`arch::opening_top`; the rough opening is cut to the shape; a roof top's height is derived when the window is placed or moved); door style `fixed`; sill and casing sides.
- **Parts and owners** (decision 51): `tread`, `riser`, `stringer`, `skirt`, `stair`, `post`, `railing`, `ledger`, `joist`, `rim`, `beam`, `footing`, `deck`, `grille`, `sill`, `casing`.
- **Exports:** GLB/OBJ one node per stair, railing and deck-structure kind; IFC `IfcMember`, `IfcColumn`, `IfcFooting`, `IfcRailing`. `scene_cache_equals_full` gained the edits.
- **Demo:** `phase-17-demo.json`; the showcase got railed half flights, a framed rear deck, 2 × 2 grilles and one round top.

### Numbers locked
- **Deck** (2,8.1)–(6,11.1), 0.9 up: 9 joists at x = 2.4 … 5.6 plus rims; posts at x = 2.3, 4.0, 5.7 on y = 10.8. Post at (4.0,10.8,0.2); nothing at (3.15,10.8,0.2); beam at (3.15,10.8,0.6); joist at (2.8,9.5,0.8); nothing at (3.0,9.5,0.8); boards at (3.0,9.5,0.89).
- **Railing** path (0,0) (0,3) (4,3) (4,0): 8 posts, the four path points plus (0,1.5), (1.3333,3), (2.6667,3), (4,1.5).
- **Stair plan** (16 risers of 0.16875): 7 solid nosing lines, one break line, 8 dashed on level 1; `DN` and no dashed line on level 2.
- **Wall under a flight:** stair at (1,1) heading +x, 0.9 wide; wall (3,0)–(3,3). Nosing line 1.5141 at x = 3.0, wall top 1.2141: hit at (3.0,1.0,1.15), nothing at (3.0,1.0,1.27), full height at (3.0,2.5,2.0). 16 `riser` parts closed, 0 open.
- **Grilles** 2 × 2 on the phase-4 window: `grille` at the glass centre and 0.15 m beside it; `glass` 0.15 m beside and 0.15 m above it.
- **Round top** 0.9 × 1.5, sill 0.6: no wall at (4.0,8.0,2.0), wall at (3.6,8.0,2.05).
- **Roof top** at y = 1.5 in the x = 0 gable, sill 1.2: top 3.05 at y = 1.5 and 2.825 at y = 1.05; no wall at (0,1.5,3.0), wall at (0,1.05,2.9).

### Phase-17 gaps
- **Out of scope by the brief:** winders, curved, spiral and wrapped stairs, flights of different widths, starter and flared treads, horizontal and cable rails, deck border and transition boards, dormers, shed roofs.
- **USER-GUIDE.md has no phase-17 section** (decision 52); still so at the phase-21 checkpoint (no heading for stair details, deck frames, railings, grilles or window tops).
- **`ifc_step_grammar.rs` has no counts for `IfcMember`, `IfcColumn`, `IfcRailing` or `IfcFooting`** (confirmed absent at the phase-21 checkpoint).
- **Stair shapes:** the detailed parts appear to be built for straight flights only.
- **Roof-top windows** keep their stored height after a roof re-pitch until placed or moved again.
- **Automatic railings** are re-decided only by the stair and deck commands.
- **No grips** for stairs, railings or decks.

## Phase 20 — plan views and dimensions (landed)

13/13 checks on 2026-11-13 after one fix cycle. 16 segments, the fix cycle (p20f-auto-dim-rows, p20f-dim-inspect, p20f-rcp-soffits-once, p20f-demo-scripts) and harden passes. Checks planned at 907b3539c and not revised.

### Plan views (decision 53)
- `PlanView`: name, level (or any level), `PlanKind`, a CADCraft `LayerState`, an `AnnotationSet`, `RoomLabelShow`, an optional reference level and a zoom. Stored in `ArchModel.plan_views`, with `open_plan_views` and `active_plan_view`.
- `LayerState` gained wildcard rules (`LayerRule {pattern, on?, transparency?}`: `*` and `?`, case-insensitive, the last matching rule wins; the snapshot's layers win over the rules).
- The seven templates (Working, Floor Plan, Electrical, Reflected Ceiling, Roof Plan, Kitchen & Bath, Site) are implicit defaults: present in every drawing, not serialized until changed; deleting them sticks.
- Opening a view first creates every layer the plan can draw the model's objects on (`arch::plan::layers_in_use`) and the view's annotation layer.
- Commands: `planview {name, from?, level?, kind?, reference?, labels?, open?}`, `planview.edit`, `.delete`, `.open`, `.close`, `.list`. `export {path, planView}` writes a view from a copy of the drawing. `view.electrical` is the Electrical plan view.
- UI: a tab strip over the plan, all through the `planview` commands.

### The plan annotation pipeline (decision 54)
- `arch::plan::annotate(model, view, level) -> PlanAnnotation {lines, labels}`. Line kinds: ridge, hip, valley, gableEnd, eave, rake, slopeArrow, slopeArrowHead, trayOutline, trayTop, vaultLine, ceilingItem, soffit, reference (phase 21: callout). Label kinds: roomName, roomArea, roomDimensions, roomCeiling, pitch, trayHeight (phase 21: scheduleNumber, callout).
- `arch.inspect {"plan": true | "<view>"}` reports it; `arch.inspect {"planViews": true}` lists the views. Made at the shown level (`plan_level`).

### Reference display, reflected ceiling plan, roof plan, room labels
- A view can draw another level's walls beneath in a muted colour (`reference: {level, color}`).
- **Reflected ceiling:** tray outlines, the pitched top ring and a height label (base finished ceiling + the tray's and its ancestors' depths); vault lines; soffits and ceiling items, each drawn once; a room's ceiling text is its finished ceiling or, vaulted, its pitch. Nothing is mirrored.
- **Roof plan:** ridges, hips and valleys; eaves and rakes from the plane rings; each plane's pitch as a label (`x/12`) with a slope arrow.
- **Room labels:** name, area, dimensions and ceiling height per plan view (`RoomLabelShow`); `room.label {room, area}` picks the area. Label text uses `m2` and ` x `.

### Dimensions (decision 55)
- `AssocSnap::Arch {feature: centre | leftFace | rightFace | edge(i), along}` attaches a definition point to an arch proxy handle (DXF assoc xdata snap code 7). `arch.inspect {"dimensions": true}` computes each measurement from the geometry.
- **Auto exterior dimensions:** `dimauto {level?, on?, rows?}`; per level and side up to three rows beyond the outer face (openings 1.0 m, walls 1.5 m, overall 2.0 m) on `A-ANNO-DIMS` in the style `Arch`; the sides are those of the exterior walls' bounding box; the walls row splits at interior walls' centre lines. Regenerated in the command's own undo step; a row edited by hand is marked edited and left alone. `arch.inspect {"autoDims": true}`.
- **Driving:** `dimdrive {dimension, value, object?}` moves the object at the near end; the far end stays. It moves walls (with joined and T neighbours), openings, cabinets, fixtures and furnishings; it does not re-detect rooms.

### Demo
`phase-20-demo.json` (the phase-1 house with a second level and views per level) and `phase-20-showcase.json` (applied after `bilevel.json`: floor plan, electrical and reflected ceiling views for Lower and Main, and `dimauto` on each). USER-GUIDE.md "Plan views" and "Dimensions".

### Numbers locked
- **Auto dimensions:** overall 10.2 on south and north, 8.2 on east and west; the north walls row 6.1 + 4.1.
- **Driving:** the 6.1 dimension set to 5.6 → the wall at x = 5.5; north-west interior 25.9475 m², north-east 21.0975 m²; the row reads 5.6 + 4.6.
- **Associative:** the phase-4 window moved 1 m changes its locating dimension by 1.0. A base cabinet on w2 at offset 8.0 reads 1.6, then 2.1.
- **Roof plan** (hip at 6/12): one ridge of 2.0 m, four hips of 5.79828, four `6/12` labels.
- **Reflected ceiling:** trays tr0 0.7…5.35 × 3.65…7.3 and tr1 1.3…4.75 × 4.25…6.7; the label is 2.70; the suspended room reads 2.17.
- **Plan views:** the Reflected Ceiling SVG has no entity on `I-FURN`; the Electrical view matches phase 15's results.

### Phase-20 gaps
- **Out of scope by the brief:** framing plans, story poles, construction lines, shadow plans, interior auto-dimensions other than phase 21's cabinet runs.
- **The gallery has no phase-20 images** (no reflected ceiling, roof or site plan in `renders/`); still so at the phase-21 checkpoint, though the same views are now pages of the drawing-set PDF.
- **USER-GUIDE.md "Dimensions"** names none of `dimauto`, `dimdrive`, `room.label`, `export {planView}` or the reference display.
- **The Entry landing has no plan views**, and the reflected ceiling plans have little to show without phase-16 ceilings on the showcase.
- **README vs script:** at the phase-20 checkpoint `showcase/README.md` described phase-18 features the script lacked; `p18i-showcase-phase18` (297b0801e) has since added them to `bilevel.json` (backsplash, `cabinet.face`, `cabinet.fill`, `countertop.detach`/`.edit`, `niche`, `room.moldings`, `wall.split`). A line-by-line comparison was not made.
- **Auto dimensions** cover only the bounding box's four sides; **driving** is a command (no in-place text edit confirmed) and leaves rooms stale until `roomdetect`; **no grips** on plan-view objects or auto rows.

## Phase 21 — schedules, elevations and sheets (landed)

13/13 checks on 2026-11-15 after three fix rounds. The checks were not re-run by the lead at the checkpoint; the results are the swarm's record.

- **What merged:** the 20 planned packages as 22 segments (p21-observables-1..3, p21-opening-edit, p21-view-plumbing, p21-schedule-model, p21-schedule-tables, p21-plan-numbers, p21-interior-elevation, p21-interior-dims, p21-patterns, p21-elevation-marks, p21-view-render, p21-view-notes, p21-sheet-command, p21-title-block, p21-sheet-camera, p21-plan-viewports, p21-plot-sheets, p21-callouts, p21-ui-callout-open, p21-demo), three fix rounds, and harden passes:
  - `p21f`: -pattern-line-material (a pattern line names the material of the mesh that made it), -interior-cabinet-front (a base cabinet shows its whole front, tagged as the cabinet), -showcase-planviews;
  - `p21g`: -callouts-without-view, -sheet-camera-new-sheet (a camera view on a new sheet, the image sized in paper mm), -showcase-drawing-set (the script replays with no refusal and gives the full set);
  - `p21h`: -showcase-kitchen-cabinets (cabinets on the wall the kitchen's interior elevation shows).
- **Checks:** planned from the brief against arch3d at d294323a2 (74319172d); revised by the lead twice (86ec898b5, acbc591ec). `p21lib.inc` loads phase 20's check library (else 19's, 17's or 16's). No earlier check was rewritten; phase-7 check 06, phase-15 check 04 and phase-20 checks 01/02/13 keep passing.
- **Operator:** the `# needs:` lines of checks 04, 09 and 10 name the phase-20 segments; p21-observables was split in three for the engineer budget.

### Schedule numbers and schedules (decisions 14, 57)
- `arch::schedule` (pure): identical items share a number. Doors (with garage doors and cased openings) group by kind, style, width, height; windows by style, width, height, sill; cabinets by kind, width, depth, height, fronts, doors, drawers; lengths compared to 1e-6. Numbers `D01` / `W01` / `C01` per prefix in order of each group's lowest id, derived on every read.
- Room-finish rows read `arch::paint::effective_material` (floor, wall sides facing the room, ceiling) through `material_name`, and the ceiling height from `plan::ceiling::room_ceiling_text`. Cells: 3-decimal lengths, 2-decimal areas.
- `Schedule` record (`ArchModel.schedules`: id `sc<n>`, name, kind door | window | cabinet | room, columns, level filter, at) and one model-space TABLE on `A-ANNO-SCHD` linked by xdata (app `CADCRAFT_ARCH_LINK`, role `schedule`, key `sc<n>`).
- `schedule {kind, at, columns?, level?}`, `schedule.edit`, `schedule.delete`. The post-command pass `refresh_schedules` (after `refresh_views`) rewrites a TABLE whose rows changed, inside the command's undo step, and drops a schedule whose TABLE was erased.
- `opening.edit {id, width?, height?, sill?, style?}` re-validates with `opening::validate_placement`; style lists come from `opening.rs`'s `valid_styles(kind)`. It leaves phase 17's top and grille alone.

### Labels in plan and in elevations
- **Plan** (the phase-20 pipeline, `plan/tags.rs`): `PlanLabelKind::ScheduleNumber` on `A-DOOR-IDEN` / `A-GLAZ-IDEN` / `A-FLOR-CASE-IDEN`; openings 0.35 m beyond the wall's right face, cabinets at their plan centre; only in plan views whose `labels.numbers` is on (Working, Floor Plan, Kitchen & Bath).
- **Elevations** (`arch3d::views`): tags on `A-ELEV-IDEN` in the opening or cabinet front; indicators on `A-ELEV-SWNG` (door, casement, awning, hopper and cabinet-door swing triangles, apex at the hinge). A fixed door has no indicator.

### Interior elevations
- `elevation {room, wall | side, level?, insert}` makes a `ViewKind::Interior` view storing room, `room_point`, wall and level.
- `arch3d::views::interior` restricts the scene to the room box in view axes (u across the room, depth from just inside the opposite boundary to the wall face, z floor to finished ceiling, the room's level only; pieces outside a non-convex room dropped) and resolves the room again by `room_point` after `roomdetect` renumbers rooms.
- Automatic dimensions: each cabinet run (per cabinet and the run), the overall width and the floor-to-ceiling height.
- `ArchKind` gained `Cabinet` and `Fixture`, so casework lines in elevations and sections carry their own ids.

### Material patterns
- Every elevation gets pattern lines on `A-ELEV-PATT`: courses and joints from the procedural texture tiles (`texture::tile_size`; 8 courses per tile), world-anchored, clipped by hidden-line removal through `hlr::visible_segments`. Each line names its mesh's material. Plain paint draws none.

### View notes (decision 60)
- `ArchView.notes`, serialized with the view: text, marker and dimension notes in view coordinates (u = p · (ly, −lx), v = z) or attached to a corner of an object's lines (resolved after every regeneration; a detached note keeps its last position).
- `view.note {view, text | marker, at | attach}`, `view.dim {view, from, to}`, `view.note.delete {view, id}`.

### Sheets (decisions 58, 59)
- `Sheet` record per layout (`ArchModel.sheets`, in plot order) and a sheet set (`ArchModel.sheet_set`: project, date).
- `sheet {name, number?, title?, size?: "ARCH D", views: [{view, scale, at?} | {camera, width, at?}]}`, with the shorthands `planView` + `scale` and `camera` + `width`. A `view` names a linked view (`v<n>`), a plan view or a schedule; an ambiguous name is refused. `sheet.edit {name, rename?, number?, title?, add?, remove?, order?}`, `sheet.delete`, `sheet.open {name | view}`, `sheetset {project, date}`.
- One locked viewport per view at 1000/scale paper mm per metre, linked by xdata (role `sheet-view`). A viewport that would fall off the sheet is refused.
- A plan-view viewport is drawn by `render` from a copy of the drawing with that view applied; phase 20's auto dimensions follow through the copy.
- A camera's render is an IMAGE (the raster kept in the drawing), rendered exactly as `render {"camera"}` does, with the camera's own look.
- The master title block is the block `ARCH-TITLE-BLOCK` on `A-ANNO-TTLB`, whose ATTDEF defaults are macros (`%sheet_number%`, `%sheet_title%`, `%project%`, `%date%`, `%scale%`); each sheet has one linked INSERT with the expansions, refreshed by `sheet`, `sheet.edit` and `sheetset`.
- **Callouts** (`plan/callouts.rs`; `PlanLabelKind` / `PlanLineKind::Callout` on `A-ANNO-SYMB`): every placed elevation (outside the building on the viewer's side), section (both cut ends) and interior elevation (in its room) is marked `<view number>/<sheet number>`. They draw with or without a plan view open. `sheet.open {view}` and a double-click open the sheet.
- `plot` / `exportpdf {"sheets": "all" | [names]}` feed the existing `layouts` path in sheet order: one PDF, a page and a page label per sheet.
- Phase 7's `viewsheet` stays as it was.

### New layers
`A-ANNO-SCHD`, `A-ANNO-TTLB`, `A-ANNO-SYMB`, `A-ELEV-PATT`, `A-ELEV-SWNG`, `A-ELEV-IDEN`, `A-DOOR-IDEN`, `A-GLAZ-IDEN`, `A-FLOR-CASE-IDEN`.

### Contracts with phases 18 and 19
- `casework_layout::fronts(model, cabinet)` answers the cabinet door swings; `Cabinet`'s identity fields and their `name()` spellings make the cabinet schedule.
- New opening kinds (niche) and the shower door get no schedule number (phase 21 uses `matches!`, never an exhaustive match); a new style must be in `opening.rs`'s lists for `opening.edit` to accept it.
- Room-type finishes derived inside `effective_material` flow into the room schedule; a `tile` finish gets a tile pattern.
- A glass wall or molding part needs its own `ArchKind::from_part` arm if it should not read as a wall.
- `p21-patterns` only added `tile_size()` to `texture.rs`, which phase 19's normal maps also edit.

### Demo
- `docs/arch3d/showcase/phase-21-showcase.json`, replayed after `bilevel.json` and `phase-20-showcase.json`: five kitchen cabinets on the wall the interior elevation shows, the four schedules, interior elevations of the Kitchen (west) and Main Bath (east), `sheetset`, and 13 sheets: A-000 Cover (Front camera + a view), A-001 Site Plan, A-101 / E-101 / A-102 Lower floor plan, electrical and reflected ceiling, A-111 / E-111 / A-112 the same for Main, A-201 Roof Plan, A-301 Elevations, A-302 Sections, A-401 Interior Elevations, A-501 Schedules.
- `cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json --commands docs/arch3d/showcase/phase-20-showcase.json --commands docs/arch3d/showcase/phase-21-showcase.json --cmd 'plot {"sheets": "all", "path": "showcase-set.pdf"}'` (with `CADCRAFT_RENDER_ADAPTER=fallback` where there is no GPU). Check 13 does the same and saves `phase-21-drawing-set.pdf` as an artifact.
- USER-GUIDE.md gained "Schedules and labels", "Interior elevations and view notes" and "Sheets", each with a runnable example; `showcase/README.md` gives the command line.

### Numbers locked
- **Schedules:** four 0.9 × 1.2 windows and one 1.2 × 1.2 → two rows, counts 4 and 1; one resized to 1.2 × 1.2 → 3 and 2.
- **Interior elevation:** the north-west room's elevation of its east wall has every segment within y 3.05…7.9 (± 1e-3). `cabinet {"kind": "base", "wall": "w5", "offset": 2.5}` stands at (5.95, 5.5), along y 5.2…5.8, and shows as its front rectangle u −5.8…−5.2 (u = −y), v 0…0.914 (box 0.876).
- **Annotations:** a text on that view keeps its view coordinates after w5 is stretched to x = 5.5 and the view regenerates (the room is renumbered r2 → r5 by `roomdetect`; the view finds it by `room_point`).
- **Patterns:** a brick wall shows horizontal pattern lines across its face in the front elevation; plain paint none.
- **Sheets:** a PDF page per sheet, 914.4 × 609.6 mm (2592 × 1728 pt); the 10.2 m south face at 1:50 measures 25.0…229.0 = 204.0 mm in the sheet's SVG.

### Acceptance (all PASS)
`01_observables` · `02_schedule_numbers` · `03_schedule_kinds` · `04_plan_numbers` · `05_interior_elevation` · `06_elevation_marks` · `07_view_notes` · `08_material_patterns` · `09_sheets` · `10_callouts` · `11_hostile` · `12_layers_wasm_i18n` · `13_demo`.

### Phase-21 gaps
- **Out of scope by the brief:** framing plans, revision tables, CAD detail libraries, multi-file projects, LiDAR room capture.
- **No drawing-set PDF is committed, and the gallery predates phases 20 and 21:** `renders/` has no plan-view, interior-elevation or sheet image.
- **The showcase still has no phase-16 ceilings or foundation** (decision 48), so its reflected ceiling sheets are thin and its sections show no footings.
- **The kitchen cabinets of the interior elevation live in `phase-21-showcase.json`,** not in `bilevel.json`: the house alone does not have them.
- **The Entry landing has no sheets** (the set covers the floor levels).
- **Two sheet commands:** `sheet` and phase 7's `viewsheet` both make layouts of views; `sheet` has more params than the brief's form (`number`, `title`, `planView`, `camera`).
- **Callouts change the bare plan** of a drawing with placed sheets (decision 60).
- **Not numbered or marked:** niches and shower doors (no schedule number), fixed doors (no indicator).
- **Patterns** exist only for materials with a procedural texture tile.
- **Web:** check 12 passes; a camera image on a sheet in the web build was not confirmed.
- **UI:** no schedule or sheet panel and no grips on view notes; commands, menu entries and the callout double-click only.

## Open items vs Chief Architect

- **Showcase:** the phase-16 ceilings and foundation on `bilevel.json`; the kitchen cabinets in the house script; an audit of the layout with containment tests against walls; a gallery re-rendered with the plan views, interior elevations and sheets; the drawing set committed (PDF or page images); a README checked against the script.
- **Docs:** USER-GUIDE.md sections for phase 17 and for phase 20's dimension commands.
- **3D view and rendering:** SSAO/TAA/cascaded shadows; raster window glass opaque; Russian roulette without a survival floor; textures in the path tracer; shaded icons. (Sun in the pane and looks are phase 19.)
- **Walls and openings:** arc walls and their openings; main-layer joins and the standard-area basis; `reversed`; curtain walls; opening sub-items (mullions, jambs, hardware, display states); door classification; bay, box and bow geometry.
- **Roofs:** dormers, crickets, roof returns, skylights, gutters, shed roofs; interactive roof-plane joining; dynamic roof topology.
- **Stairs:** winders, curved and spiral stairs, ramps; L and U flights in detail; flights of different widths; starter treads; grips.
- **Railings and decks:** horizontal and cable rails, goosenecks; deck border and transition boards; railings re-decided when walls move; grips.
- **Ceilings:** manual ceiling planes, barrel and shelf ceilings.
- **Structure:** wall, floor and roof framing; piers, grade beams, garage curbs.
- **Services:** plumbing and HVAC; load calculations, panel schedules, wiring runs, low-voltage; connections and circuits in exports.
- **Site:** terrain/plant tools, grips and a site panel in the UI.
- **Plans and dimensions:** framing plans, story poles, construction lines, shadow plans; auto dimensions on recessed faces of stepped footprints; interior auto-dimensions beyond cabinet runs; room re-detection after a driven wall.
- **Schedules and sheets:** revision tables, CAD detail libraries, multi-file projects; schedule numbers for niches; one sheet command instead of `sheet` and `viewsheet`; a sheet and schedule panel; patterns for non-procedural materials.
- **Exports:** IFC materials and colours; parametric IFC bodies; grammar-test counts for footings, deck members, railings and electrical entities.
- **Recognition:** `ARCHDETECT` on real plans; scale from dimensions; editing candidates before accept.
- **Rooms:** room boxes, dynamic defaults.
- **Library:** `move` refusing handles it cannot resolve; `arch.inspect` handles in the form commands take; furnishing outlines beyond the hull; emitting symbol lights.
- **Rebuild:** a level edit that rebuilds only what it touches.
- **Georeferencing:** transforms between coordinate systems.

## Phase plan

0–17 ✅ → 20 ✅ plan views and dimensions (2026-11-13) → **21 ✅ schedules, elevations and sheets** (2026-11-15; the showcase's phase-16 features, the gallery and a committed drawing set still open) → 18 interior finish, kitchen and bath (planned segments and fix rounds through `p18i-showcase-phase18` merged; checkpoint pending) → 19 light, sky and cameras (planned segments and fix rounds through `p19n-pt-pipelines-once` merged; checkpoint pending).

## Building blocks added outside the swarm (assisted)

- **`geom::skeleton`, `roof_skeleton`, `weighted_skeleton`** (2026-10-25): straight skeleton, mitred inset/outset, per-edge pitch and gable ends; phase 4 is built on it.
- **`render3d` drawing and the 3D pane** (2026-10-25): `Offscreen` (`create`/`with_device`/`encode`/`render`/`read_png`), flat shading from each triangle's plane, `headless_instance` on `Backends::PRIMARY`, one shared GPU test device; wgpu 30.
- **Feature edges, HLR, section cuts** (2026-10-25): `arch3d::edges` (`feature_edges`, crease 20°, silhouettes with a view direction), `hlr` (`OrthoView`, exact `visible_lines`, `merge_collinear`), `section` (`Plane`, `section_cut`, `section_view`). Phase 21 added `hlr::visible_segments` on the same kernel.
- **`geom::tin`** (2026-10-30): lifted from zorgglubb/cadcraft's `cadcraft-terrain` (MIT OR Apache-2.0, credited in `ATTRIBUTION.md`); the site data model and phase-9 stubs.
- **Phase-10 groundwork** (2026-10-31): the light data model, catalogue and photometry, the lit pipelines' group 2 and shadow atlas contract, `RenderHooks`.
- **Phase-11 groundwork** (2026-11-01): area lights, IES, `render3d::pathtrace` skeleton, `crates/gpu-optin`.
- **Phase-12 groundwork** (2026-11-01): `io::step`, `io::ifc` spine, walls in full, element stubs.
- **Phase-14 groundwork** (2026-11-03): the sample set `assets/library/sample/` (49 items, CC0 and CC-BY, `catalog.json`), `tools/library/` (run by the operator), CHIEF-REFERENCE §6.
- **Operator fixes, phases 10–15:** recorded in this file's history (97e939f25). Recurring causes: tests asserting substrings malformed output also contains (decision 32), a cache patched beside the build (33), process-wide hooks (35), checks never run before locking (42), a test segment editing what it judges (43).
- **Phase-16 groundwork** (2026-11-08, 78f95deaf): room `flatCeiling`/`ceilingStack` and the finish stack; the ceiling point query with stubs; `build3d` `ceiling`, `probe` and `parts`; the Tray/Soffit/Foundation data types; the planar geometry kit; empty incremental sections; checks 01 and 02, `p16lib.inc`. Also CHIEF-REFERENCE §7 for phases 16–21 (0dcace523). A CC-0 sky HDRI for phase 19 was added (576538153) and reverted (6f0b65d23); phase 19's HDRI segments restored it (`assets/library/sample/hdri/`).
- **Phase-16 operator work:** the earlier-phase checks that phases 16–19 change (bfaa016c6); phase-14 check 01 pinning the nine finishes for phase 18 (52a669156).
- **Phase-17 operator work:** phase-5 checks 03 and 07; a stray scratch test removed (7efb9bd81); the showcase gallery re-rendered (b97e23f24); phase-16 check 20 notes, rather than fails, renders of an earlier script (f111186e3).
- **Phase 20:** no operator groundwork and no operator fix recorded.
- **Phase 21:** no operator groundwork; the operator fixed the `# needs:` lines of checks 04, 09 and 10 and split p21-observables in three.

## Phase 18 plan

Phase 18 (interior finish, kitchen and bath) was planned against arch3d at febe33d on top of the phase-17 plan. 20 segments, several split by size into numbered parts: p18-observables (-1..-3), p18-scene-plumbing (-1..-4), p18-room-finishes (-1..-3), p18-molding-kit (-1, -2), then p18-room-moldings, p18-molding-lines (-1, -2), p18-cabinet-moldings, p18-countertops, p18-backsplash, p18-cabinet-fronts (-1, -2), p18-corners, p18-fillers-shelves, p18-glass-walls (-1..-3); second-level p18-countertop-shapes, p18-face-appliances, p18-shower-niche (-1..-4), p18-crown-wrap; last p18-exports, p18-rebuild, p18-demo. Checks 01-20 under tests/acceptance/phase-18 with p18lib.inc. Status at the phase-21 checkpoint: the planned segments are merged with fix rounds `p18g-*` (countertop-nose, crown-soffit-tray, fill-labels-shelves, room-moldings-refusals, demo-script), `p18h-showcase-digest` (showcase tests share one replay and print a digest) and `p18i-showcase-phase18` (the showcase house gains the phase-18 kitchen and bath features; the demo a door-less glass panel); the checks were revised by the lead (d63b89feb); the checkpoint is pending.

- Earlier checks, landed by the operator: phase-13 check 02's last part exports the walls alone (a detected room's floor now takes its type's template finish); phase-14 check 01 pins the nine finish items.
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

Phase 19 (light, sky and cameras) was planned against arch3d at febe33d37 and the phase-17 plan. 19 segments in five waves, several split by size into numbered parts; checks 01-17 under tests/acceptance/phase-19 with p19lib.inc. Status at the phase-21 checkpoint: the planned segments are merged with fix rounds `p19f-*` (cli-render-reply, cutaway-interactive, demo-script, glb-strip-node-normals, hdr-resolution-line, pt-horizon-lit, pt-strip-clear-of-housing, sky-render-time, sunangle-date-form), `p19k-pt-resolve-hable` (the path tracer's resolve applies the Hable curve), `p19m-env-radiance-linear` (the environment map is no longer re-validated per call) and `p19n-pt-pipelines-once` (compute pipelines built once per device); the checks were revised by the lead (2e7bf6d00); the checkpoint is pending.

- Waves: 0 (no deps) p19-foundation (look data, the `look` command, apply_look_params as the one look parser, `backdrop {kind: sky}`), p19-r3d-groundwork (GPU layout: PbrUniform 128 B, frame uniform 256 B, GpuFrame fields and flags, shader and tracer hook stubs; byte-identical images), p19-horizon, p19-strip-model, p19-normal-maps; 1 p19-exposure, p19-sun, p19-env-path, p19-seeing-in, p19-strip-tray; 2 p19-env-raster, p19-strip-render, p19-normal-maps-render, p19-seeing-in-render; 3 p19-sky, p19-hdri, p19-cameras; 4 p19-pane, p19-demo. Critical path: foundation -> exposure -> env-raster -> sky -> demo.
- The render look: `Look` (crates/doc/src/arch/look.rs) is stored globally (ArchModel.look; its backdrop stays ArchModel.backdrop) and complete per saved camera (Camera3d.look, captured at save; Camera3d.saved for camera.reset). A camera without a look follows the global look. Every render, illuminance and pick3d resolves the effective look and overlays its own params through apply_look_params. Named sun angles live in ArchModel.sun_angles.
- Exposure: a pixel's linear value = luminance (nits) x 1/(1.2 x 2^EV100), then the tone curve (ACES default, Hable, none). Auto: 0.18 / log-average luminance; daylight clamps to [pi/1e5, 0.01], other nights to [pi/1e5, 53.33]; the legacy `render {sun: false}` keeps 0.01. The lit raster is physical (sun and fixtures in lux, ambient from SH or the night stand-in 6.2832 lx) and tone-mapped in the pane as in Finalize; picture backdrops and edge lines stay display colours. The calibration param `albedo` makes every surface Lambertian.
- Environments: render3d Environment (equirect, linear nits, row 0 = zenith, u = 0.5 + atan2(d.y, d.x)/(2 pi)), from an HDRI backdrop or the Preetham sky (HDRI wins); visible only under a Sky or Hdri backdrop, else light only. The path tracer importance-samples it with MIS (tables in the aux `ies` buffer: the tracer stays at 8 storage buffers); the raster uses 9 SH coefficients. The moon is a directional light only with the sun below the horizon and the sky on (arch3d::sky::moon_dir).
- HDR images are stored in the drawing (ArchModel.hdr_images, hex RGBE, <= 1024 x 512 each, <= 4 images / 8 MiB, pruned to what backdrops and camera looks reference).
- The horizon plane is Scene3d.horizon: never in meshes or bounds (fits, shadow bounds, HLR and exports never see it); probes answer part `horizon`.
- Strip lights: LightKind::Strip, sampled every 0.15 m as Lambertian emitters (CPU illuminance; raster: one drawn light with up to 128 segments, never shadow-mapped; path tracer: one segment light each). LightFixture.tray is declared by p19-strip-model. LightKind is matched exhaustively in render3d lights.rs, pathtrace/scene_gpu.rs, io ifc/lights.rs and glb_lights.rs.
- Seeing in: arch3d ViewFilter (cutaway hidden set, clip box, near clip, overview levels) applied to a filtered scene cached per filter key; the clip box also clips shadow passes.
- GPU layout ownership: p19-r3d-groundwork fixed PbrUniform (128 B), the frame uniform (256 B), GpuFrame and the shader hooks; later changes add fields only at the end.
- The CC-0 HDRI: p19-hdri restores the .hdr and its README from commit 576538153 with `git show` (md5 c69498687e876bf68d2a8b8a62234eb9) and adds both ATTRIBUTION rows; check 13 requires the file and its row.
- Earlier checks, landed by the operator: phase-13 checks 02 and 13 leave `-normal` images out of the GLB image lists.
- Check notes: the strip round a tray is its own check 17; check 10 drops the siding case (mirrored-azimuth suns cannot see a row-only tilt; the siding map is checked in 09); check 03(d) uses EV100 8; check 08(e) renders the inline clay/technical views before the global look changes; check 11 requires the demo's all-off light set `Day` and a level dusk camera.
- Review fixes kept: sun_lux zero at night, the meter told about the ground quad, Lambert calibration through PbrUniform.lambert_albedo, the legacy night pin, composite_misses returning early for Sky/Hdri, sky:false pinned on every sun measurement, FRAME_SKY only with look.sky.on, `-normal` image names, library.place of an .hdr item as hdri, set_backdrop for camera pictures.
- Phase-17 names relied on: probe::pick_part and probe::arch_kind, the new parts, ui-egui pane3d.rs proxy_handle/locate, arch3d texture::board_finish. Phase 16: arch ceiling::tray::outline for p19-strip-tray.
- Shared files (union merges; each package edits only its own functions): engine cmd/arch/mod.rs registration, inspect.rs dispatch, uk.tsv rows; render3d Lighting fields; apps/cadcraft-cli/src/render_hook.rs; tracer.rs (groundwork owns frame_lighting; each package fills its own frame_* stub); frame_look.rs.
