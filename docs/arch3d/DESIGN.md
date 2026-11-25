# arch3d — design log

Branch `arch3d` (fork of `storytold/cadcraft`, v0.10.0). Running log for *draw a plan, get the house in 3D*. One commit per phase, each ending at a ⛔ checkpoint.

The sections for phases 1–15 were condensed at the phase-16 checkpoint (2026-11-11); their full text is `docs/arch3d/DESIGN.md` at commit 97e939f25 and before. The sections for phases 16, 17 and 20 were shortened at the phase-21 checkpoint (2026-11-15); their full text is this file at 131a10fa7. The phase-19 and phase-21 sections and the phase-18 and phase-19 plans were shortened again at the phase-22 checkpoint (2026-11-25), when the phase-22 plan became the phase-22 section; their earlier text is this file at 57ae979e4. The checks under `tests/acceptance/phase-N/` hold the numbers.

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
| `doc/src/arch.rs`, `doc/src/arch/*` | L1 | `ArchModel` on `Drawing::arch` with `arch_revision`/`touch_arch()`: walls, openings, rooms, levels, roofs, stairs, railings, cameras, views, `site` (location, terrain, modifiers, plants), `light` fixtures, `finish`, `casework` (cabinets, fixtures), `furnishing` + `symbols`/`images`, `paint`, `backdrop`, `defaults`, `electrical` (devices, connections, light sets), `LevelKind`, `Wall.top_level`, `plan_level`. **Phase 16:** `Room.flat_ceiling`/`ceiling_stack`, `Tray` (tr<n>, +0x78000), `Soffit` (sf<n>, +0x80000), `Foundation` (fd<n>, +0x88000). **Phase 17:** `detail.rs` (stair risers/stringers/underside/plan cut and per-side rails; railing `inset`, `host`, `auto`; deck board direction and rails; opening grille, top, sill and casing sides; door style `fixed`). **Phase 18** (checkpoint pending): `countertop`, `molding`. **Phase 19:** `look.rs` (`Look`, `Technique`, `ExposureSetting`, `ToneMap`, `SunLook`, `SkyLook`, `MoonLook`, `ClipBox`, `SunAngle`; `ArchModel.look` / `sun_angles` / `hdr_images` / `active_camera`; `Camera3d.look` / `saved` / kind `overview` with `level`, `lower_levels`), `LightKind::Strip` and `LightFixture.tray`, backdrop kinds `sky` and `hdri` with intensity, rotation and horizon. **Phase 20:** `planview.rs` (`PlanView`, `PlanKind`, `AnnotationSet`, `RoomLabelShow`, the seven templates; `ArchModel.plan_views` / `open_plan_views` / `active_plan_view`), `autodim.rs` (`ArchModel.auto_dims`), `Room.label_area`; `doc/src/tables.rs` `LayerState` gains wildcard `LayerRule`s. **Phase 21:** `sheet.rs` (`Schedule`, `Sheet`, the sheet set; `ArchModel.schedules` / `sheets` / `sheet_set`), `ViewKind::Interior`, `ArchView` room / wall / `notes`, `ViewGeometry` patterns / indicators / labels, `ArchKind::Cabinet` / `Fixture`, `RoomLabelShow.numbers`. **Phase 22:** `Look.lens` (projection, compression, shift, aperture, focus), `Look.film`, `SunLook.disc`, `Technique::Photoreal`; `photo.rs` (`PhotoSettings`, the rung table `rung(n)`). Handle offsets by kind below `ARCH_HANDLE_SPAN 0x100000` (decision 10). |
| `geom` | L0 | `skeleton` (straight and weighted/roof skeleton, inset/outset), `region::Arrangement`, `tin`, `cdt`, `tin_ops`. |
| `arch` | L2 | `wall`, `graph`, `join`, `opening`, `symbol`, `room` (detection, `room_at`), `poly`, `area`, `label`, `roof`, `roofedge`, `stair`, `railing`, `sun`, `detect` (ARCHDETECT pipeline + `score`), `terrain` (`SiteSurface`, mesh, contours, volumes, csv, pad, retaining), `plant`, `site`, `light`, `ies`, `casework`, `casework_layout`, `library` (records, search, history, fits, place), `furnishing`, `paint`, `electrical`, `connections`, `outlets`, `grade`. **Phase 16:** `ceiling` (`mod`: the ceiling point query; `stack`, `vault`, `tray`, `soffit`, `overhead`), `foundation`. **Phase 17:** `stair_geom`, `stair_symbol`, `flight_cut`, `deck`, `railing` (`RailSurface`, posts), `railing_auto`, `opening_top`. **Phase 18:** `countertop`, `backsplash`, `casework_corner`, `casework_face`, `molding_profile`, `molding_runs`, `room_finish`. **Phase 19:** `hdr` (Radiance RGBE reader), strip paths and photometry in `light`, `light_symbol`. **Phase 20:** `plan/` (`annotate`, `PlanAnnotation`, line and label kinds, `active_view`; `labels`, `ceiling`, `roof`, `reference`, `layers`), `dimension` (arch attachments), `autodim`, `drive`. **Phase 21:** `schedule` (rows, grouping, numbers), `interior` (the room box of an interior elevation), `plan/tags`, `plan/callouts`. Phase 22 added nothing here. |
| `arch3d` | L2 | `build`/`SceneCache` → `Scene3d`; `mesh` (profile prisms; several holes in one profile are bridged), `walls` (elevation profiles + holes), `openings`, `floors`, `roof`, `stairs`, `railing`, `probe` + `ray`, `camera`, `pick` (BVH), `edges`/`hlr`/`section`, `site`, `plant`, `fixture`, `light` (illuminance), `incremental`, `finish`, `uv`, `texture`, `casework`, `furnishing`, `paint`, `icon`, `electrical`. **Phase 16:** `ceiling` (`stack`, `vault`, `tray`, `overhead` clipping), `soffit`, foundation meshes, `wall_rise`; `build3d` `parts`. **Phase 17:** `stair_parts`, `flight_cut`, `railing_parts`, `deck_frame`, grilles and shaped tops in `openings`; `texture::board_finish`. **Phase 18:** `sweep`, `molding`, `countertop`, `backsplash`, `cabinet_parts`, `wall_glass`, `niche`. **Phase 19:** `sky` (Preetham sky, twilight fade, `moon_dir`, stars), `view_filter` (`ViewFilter`: cutaway hidden set, clip box, near clip, overview levels), `Scene3d.horizon`, strip meshes and strip illuminance, normal-map generation in `texture`. **Phase 21:** `views/` (refresh, interior elevations, patterns, indicators and tags, interior dimensions, note resolution), `hlr::visible_segments`, `texture::tile_size`; `tests/gallery_tool.rs` runs the gallery tool's Python tests from the Rust gate. **Phase 22:** `lens` (the one lens function: `dir_to_frame`, `frame_to_dir`, `project_point_lens`, `ray_at_lens`, `blur_diameter_px`), `material_ext` (`MaterialExt`, the name table `ext_for`, `resolve`), `calibrate` (the test scene of spheres, boxes and point lights). |
| `render` | L2 | `arch.rs` (arch geometry → DisplayList tagged with proxy handles; views with their patterns, indicators, tags and notes on their layers; the plan of `plan_level`; the stair symbol; the plan annotation pipeline's lines and labels, reference lines beneath the poché, auto dimensions filtered by shown level; a plan-view viewport drawn from a copy of the drawing with that view applied), `arch_site.rs`, `arch_casework.rs`, `arch_furnishing.rs`, `arch_electrical.rs`, `arch_ceiling.rs`. |
| `io` | L3 | `dxf_arch.rs` (`CADCRAFT_ARCH` XRECORD, payload up to 64 MiB; save-time proxies incl. `CADCRAFT-STAIR` symbol roles), `dxf_site`, `dxf_casework`, `dxf_furnishing`, `dxf_electrical`, `dxf_ceiling`, dimension assoc xdata snap code 7 (phase 20), entity-link xdata `CADCRAFT_ARCH_LINK` kept raw by the reader (phase 21), `glb.rs` (own glTF 2.0; `node_names`, `shares_a_node`; `normalTexture` and strip nodes since phase 19; the KHR clearcoat, anisotropy, sheen, transmission and volume extensions since phase 22), `glb_lights`, `obj`, `png`, `step`, `ifc/*`, `gltf_read`, `image_read`, `library` (embedded sample set, incl. the sample HDRI). |
| `render3d` | L5 | `pipeline` (PBR), `light`, `lights`, `lit`, `lit_frame`, `shadow`, `shadow_atlas`, `edges`, `adapter` (one adapter request at a time per process), `offscreen` (`Style`: shaded, shaded-edges, hidden-line, glass-house, clay, technical), `final_render`, `pathtrace/*` + WGSL `pt/*`, `textures`, `backdrop`. **Phase 19:** `look` (`lighting_for`), `frame_look`, `exposure` (meter, EV100, tone curves), `environment`, `sh`, `pathtrace/env`, `pathtrace/spec_albedo`. **Phase 22:** `lens` (`LensOptions`), `warp` + `warp.wgsl` (the raster post-warp), `film` (CPU), `tza` (weight reader, `info`), `unet` (CPU reference network), `look::apply_photo`; `pathtrace/lens`, `edge_table`, `adaptive`, `learned`, `post` (denoiser choice, film, kept linear image); WGSL `pt/lens`, `edges`, `bsdf_ext`, `bsdf_trans`, `variation`, `sun_disc`, `clamp`, `adapt`, `unet`; `tests/oidn_reference.rs` with `tests/fixtures/oidn-kitchen`. |
| `gpu-optin` | — | The workspace's only unsafe code: wgpu's experimental-feature token for ray queries, a pipeline cache read from disk. |
| `engine/src/cmd/arch/*` | L4 | One module per command family. **Phase 16:** `tray`, `soffit`, `foundation` (+ `.edit`, `.delete`), `room {flatCeiling, ceilingStack}`, `build3d {ceiling, level}` and `parts`. **Phase 17:** `stair` detail params and `stair.edit`, `deck {boards, rails}` and `deck.edit`, `railing {postSpacing, inset}`, `window {grille, top}`, door style `fixed`, `arch.inspect {decks}`. **Phase 18:** `countertop`, `backsplash`, `cabinet.face`, `cabinet.fill`, `room.moldings`, `moldingline`, `niche`, `wall.split`. **Phase 19:** `look`, `sunangle`, `backdrop {kind: sky | hdri}`, `camera` look keys with `camera.reset` / `.update` / `.orbit` and kind `overview`, `lightfixture {type: LED strip, …}`, look params on `render`, `illuminance` and `pick3d`, `arch.inspect {cameras, sunAngles, lights}`; `engine::view_look`. **Phase 20:** `planview` (+ `.edit`, `.delete`, `.open`, `.close`, `.list`), `room.label`, `dimauto`, `dimdrive`, `export {planView}`, `view.electrical` as a plan view, `arch.inspect {plan, planViews, dimensions, autoDims}`. **Phase 21:** `opening.edit`, `schedule` / `.edit` / `.delete`, `elevation {room, wall | side}`, `view.note` / `view.dim` / `view.note.delete`, `sheet` / `.edit` / `.delete` / `.open` and `sheetset`, `plot` / `exportpdf {sheets}`, `arch.inspect {schedules, sheets, sheetSet, numbers}`; post-command passes `refresh_schedules` and `prune_sheets`; deferred view regeneration. **Phase 22:** lens, film and `sun.disc` look keys; `render_photo` (the render-only keys `rung`, `edges`, `materials`, `variation`, `clamp`, `adaptive`, `noiseTarget`, `timeBudget`, `denoise`, `denoiseModel`, `roughnessGuide`, `pfm`, `pfmStage`, `calibrate` and the reply's `photo`, `stop`, `noise`, `sppMean`, `clampLoss`, `denoiser`, `denoiseFallback`); `denoiser.info`; `render.ladder`; `pick3d` through the lens. Phase 7's `viewsheet` is unchanged. `engine::library` (catalogs, `LibraryIo`). |
| `ui-egui` | L5 | `pane3d.rs` (dockable 3D pane, six styles, walkthrough, pick → 2D selection, Finalize, own `SceneCache`; lit from the look and drawing the look's filtered scene, with a look row whose edits are queued commands run by `apply_look`; since phase 22 the row offers the Photoreal technique and a Projection chooser, the view warps with the look's Panini and shift, and a Photoreal look Finalizes at the top rung), `archdetect_review.rs`, `library_panel.rs`, `electrical_tools.rs`; the plan-view tab strip (phase 20); a double-click on a callout runs `sheet.open` (phase 21). |
| `cadcraft-cli` | — | `convert` (detects first when there are no arch objects; `.glb`, `.obj`, `.ifc`), `run --cmd / --commands FILE.json / --export / --save` (view regeneration deferred across its steps), `render … --camera --style --sun --ground --adapter --final [raster|path]` (exit 3 `NO_ADAPTER`), the session renderer (`render_hook`; it resolves the look and the photo settings, and also renders a sheet's camera image). |
| `tools/showcase` | — | `render_all.py` (Python 3, standard library): replays the showcase scripts through `cadcraft-cli` and writes the gallery, its manifest and contact sheet; the full gallery (look cameras, plan views, interior elevations, sheet images, `drawing-set.pdf`, and since phase 22 three ladders and three 1920 × 1080 Photoreal finals) is behind `--full`; `--no-set` and `--no-photo` skip parts of it. Its step list is a pure function tested by `test_render_all.py`. |

## Decisions

1. **Arch model lives in `Drawing`** (undo is a whole-`Drawing` snapshot). Data in `doc`, algorithms in `arch`.
2. **One source of truth: the arch object, not its linework.** Plan geometry is generated at draw time; DXF proxies are written only on save and not read back when arch data is present.
3. **No 3D CSG.** Walls are cut by 2D boolean on the wall *elevation profile*, then extruded; slabs/ceilings the same on room polygons; roof planes are prisms; stairs a stepped prism. Since phase 17 the same profile carries flight cuts and shaped window tops. A profile with several holes (a window and a niche) is bridged into one ring, and that case has its own test (`arch3d/tests/window_and_niche.rs`).
4. **Pure Rust, no new heavy deps.** Own glTF/OBJ/STEP writers, glTF reader and Radiance HDR reader; MIT/Apache-2.0/BSD/Zlib, wasm-compatible.
5. **The 3D pane renders offscreen** into its own colour+depth textures and presents as an image; identical on WebGPU and WebGL2.
6. **MCP stays generic.** Commands are the API.
7. **Incremental rebuild first-class:** `arch_revision` + per-object inputs (decision 33).
8. **Never crash, always deterministic:** caps, `get()`, checked arithmetic, property/fuzz tests.
9. **Additive to AutoCAD parity**; arch commands are new ids under `Architecture`.
10. **Proxy handles, not entities:** `ARCH_HANDLE_BASE = 0xFFFF_FFFF_0000_0000` + kind offset + id. Offsets: walls `+0`, openings `+0x8000`, rooms `+0x10000`, roofs `+0x18000`, stairs `+0x20000`, railings `+0x28000`, views `+0x30000`, terrain `+0x38000`, plants `+0x40000`, light fixtures `+0x48000`, cabinets `+0x50000`, fixtures `+0x58000`, furnishings `+0x60000`, electrical `+0x68000`, connections `+0x70000`, trays `+0x78000`, soffits `+0x80000`, foundations `+0x88000`; phase 18: countertops `+0xC0000`, backsplashes `+0xC8000`, molding lines `+0xD0000`. Phases 19 to 22 add no offset. JSON spells every handle one way (`handle_json`). `grips.rs::arch_id_from_handle` is the single disambiguator.
11. **Rough vs finished separate** (`ROUGH_ALLOWANCE` one constant).
12. **Placement limits, area rules and code limits are data in `arch`**, so commands/CLI/MCP/`ARCHDETECT` agree. Commands validate through these and **refuse — never clamp**.
13. **Shared structs** are built outside their declaring file only as `Type{required,..Default::default()}` or via a constructor; `cargo xtask literals` enforces it.
14. **Derived, never stored,** when a quantity follows from others (`Level::floor_to_floor()`; grade classes; the finished ceiling; a deck's frame; a room's ceiling text; schedule numbers and rows; a named sun angle's direction; what a Photoreal rung switches on).
15. **Probing, not counting, proves geometry.** Any new 3D feature adds probeable solids.
16. **Roofs are generated, then owned.** A `ROOFAUTO` roof regenerates while its walls change; `roofplane {edit}` sets `hand_edited`. Generation is all-or-nothing.
17. **A stair's well is derived from headroom, and the hole is applied from the stair** (`well_holes_for`); the code never invents a room to hold a hole.
18. **One camera, below the renderer.** The camera math lives in `arch3d` and `render3d` re-exports it; `ray_at` inverts `project_point`.
19. **Picking is a CPU ray cast** against `Scene3d` (BVH, deterministic ties). The struck *part* picks the handle range, since ids number per kind. Since phase 19 the ray is cast against the scene filtered by the view's cutaway, clip box and near clip; since phase 22 it leaves through the view's lens.
20. **WebGL2 limits everywhere.** Devices use `downlevel_webgl2_defaults()` (the path tracer asks for its own compute device); shadow maps are read with `textureLoad`; `--adapter fallback` always exists.
21. **Views are linked objects stored with their geometry** (`ArchView` + generated lines, rings, dims, patterns, indicators, tags and notes, regenerated in `after_command` inside the command's undo step). `render` cannot call `arch3d`, so it draws the stored geometry.
22. **`ArchKind` travels with every generated line and ring;** `ArchKind::from_part(part_of(mesh))` is the one mapping. Since phase 21 casework maps to `Cabinet` / `Fixture`, not to a wall.
23. **Arch linework shows through paper-space viewports** and so in layout plots.
24. **One meaning of opening offset (the centre), and arch drawings are metre drawings** (`$INSUNITS 6`); accepting detection candidates in a non-metre drawing converts the whole drawing in the same undo step.
25. **Detection is a pure pipeline in `arch::detect`**; candidates carry confidence and evidence; layer names raise a score and are never required.
26. **The terrain is a surface; the mesh samples it.** `SiteSurface` is the one answer to *how high is the ground here*. The mesh must be closed, and every mesh change carries a closed-mesh test.
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
48. **A demo the brief puts on the showcase house goes on the showcase house.** Phase 16 did not at first; later phases did, and since `p21t-showcase-house-c` `bilevel.json` itself carries phases 16 to 19, with phases 20, 21 and 22 as scripts replayed after it (`phase-20-showcase.json`, `phase-21-showcase.json`, `phase-22-showcase.json`). The script side is closed. The *rendered* side: the gallery was re-rendered from the redrawn house with the drawing set on 2026-11-16 (5b4e78eca) and again on 2026-11-23 (b21761b85); only phase 22's ladders and finals are owed.
49. **A stair's `start` is the centre of its bottom nosing line, everywhere.** One module (`arch::stair_geom`) gives tread spans and the nosing line.
50. **Automatic objects are made by the command that makes their host, never on load.** A drawing saved before the feature keeps exactly what it had.
51. **A mesh's owner comes from its material prefix, its part from `part_of`.** A part name may be shared by kinds (`post`, `footing`), so a per-object count is read from `arch.inspect`.
52. **A merge message is not evidence.** A segment's stated deliverable is confirmed in the tree at the checkpoint by reading the merge's file list. Cases: `p19-pane` (merged only tests), `p21t-showcase-house-b` (merged only tests), `p21t-gallery-tool` (an empty test file); each was landed by a later segment. A merge whose diff lacks the files its spec lists should be refused by the merger, not found at the checkpoint.
53. **A plan view is a CADCraft layer state with rules, not a second layer system.** The templates are implicit defaults, and the plan with no view open draws as before (but see decision 60 for callouts).
54. **Plan annotation is one pipeline, made at the shown level.** `arch::plan::annotate(model, view, level)` produces every generated plan line and label. `render` draws it, `arch.inspect {plan}` reports it and `export {planView}` writes it.
55. **Building dimensions are CADCraft dimensions with an arch attachment.** Auto rows are ordinary associative dimensions regenerated inside the command's own undo step; a row edited by hand is never regenerated. Measurements in `arch.inspect` are computed from the geometry on read.
56. **A script that a later phase builds on is replayed against the commands as built before that phase scripts on it.** A demo script is replayed whole by its own segment's gate.
57. **A schedule is a CADCraft TABLE linked to a record; its content is derived.** The TABLE is found by link xdata (`CADCRAFT_ARCH_LINK`, role and key), which survives handle changes on load. Rows, grouping and numbers come from `arch::schedule` on every read.
58. **A sheet is a CADCraft layout; its content is ordinary entities linked to the sheet record.** Locked viewports, a title-block INSERT, an IMAGE for a camera render. The multi-page PDF is the existing `layouts` plot path fed in sheet order.
59. **A viewport of a plan view draws a copy of the drawing with that view applied,** so sheets can show different levels and views whatever the session shows, and the session is never touched.
60. **Annotation on a linked view belongs to the view, not the model.** Notes are stored in view coordinates or attached to a corner of an object's lines and resolved after each regeneration; they change no 3D and do not bump `arch_revision`. Likewise callouts belong to placed sheets and draw in the plan whether or not a plan view is open.
61. **View extras never change a view's extent.** Patterns, indicators, tags and notes sit on their own layers and are left out of the extent and the overall dimensions, so phase 7's numbers hold.
62. **A look is data, resolved in one place.** `Look` is stored once for the drawing and complete on each saved camera (captured at save, with `Camera3d.saved` for `camera.reset`); a camera without a look follows the drawing's. Every `render`, `illuminance` and `pick3d` resolves the effective look (`effective_look`) and overlays its own params through the one parser `apply_look_params`; the pane reads the same answer through `engine::view_look`. A render's reply echoes the look it drew with, so a check compares looks, not only pixels.
63. **Light is physical, and exposure is the only scale.** Sun, moon and fixtures are in lux and lumens, radiance in nits; a pixel's linear value is luminance × 1/(1.2 · 2^EV100), then the tone curve (ACES, Hable or none). The same exposure and curve code serves the pane, the raster view, the raster Finalize and the path tracer. Picture backdrops and edge lines stay display colours. The calibration param `albedo` makes every surface Lambertian for the furnace and HDRI numbers.
64. **One environment lights and fills.** `render3d::Environment` is an equirect map in linear nits made from an HDRI backdrop or the analytic sky (HDRI wins). It is visible only under a Sky or Hdri backdrop and otherwise lights only. The path tracer importance-samples it with MIS; the raster reads nine spherical-harmonic coefficients from the same map. The sky is a function of the sun's position, never a stored picture.
65. **The horizon is not a mesh.** `Scene3d.horizon` is a plane the renderers and probes know about and that fits, bounds, shadow bounds, hidden-line views and exports never see, so no earlier number moves.
66. **Seeing in is a filter on the scene, not a renderer feature.** `arch3d::ViewFilter` (cutaway hidden set, clip box, near clip, overview levels) yields a filtered scene cached by the filter's key; the raster, the path tracer, `pick3d` and the pane all draw or cast against that scene, so they cannot disagree.
67. **A UI segment's test must call the UI code it claims.** A UI segment's spec names the function in the UI file that the test must call, and the checkpoint greps for a non-test caller.
68. **A test must be able to fail, and a UI edit is a command.** A presence test asserts the thing is there (a count above zero of the named kind, a named object, a probe), never `>= 0` or a key's existence. A segment that claims a script change lists the script in its files. A pane control never mutates the model: it queues one command, so each edit is one undo step and scripts can do the same.
69. **A document may describe only what is in the tree.** A guide or README segment that depends on another segment's tool or script lands after it, or says what exists today. Open case: USER-GUIDE.md's showcase section still says the plain `render_all.py --out target/showcase` run also writes the plan views, the look cameras, the sheet images and `drawing-set.pdf`; those, and the phase-22 ladders and finals, are written only with `--full` (the `sh` block stays the plain run, since phase-13 check 09 runs it). `showcase/README.md` says its phase-22 images are not committed until the `--full` run.
70. **A slow tool keeps its old run as the default.** A check that has always run a tool keeps running the same set in the same time; new, slow output goes behind a flag (`render_all.py --full`), and the tool's step list is a pure function so its full mode is tested without rendering. The cost: nothing in the gate runs the full mode end to end.
71. **Lens, film and the sun's disc are look data** (`Look.lens`, `Look.film`, `SunLook.disc`, `Technique::Photoreal`), so they are stored per camera, resolved in one place (decision 62), overlaid per render and reach `pick3d` through the same parser. Neutral values are not serialized: older drawings stay byte-identical.
72. **Photoreal is a table, not a mode.** `doc::arch::photo::rung(n)` is the cumulative list of what rungs 0–7 switch on; `technique: photoreal` is rung 7 and `render.ladder` renders `rung: 0..7` with one seed. A key the request names wins over the preset; a camera's own non-neutral film and sun disc win over the preset's.
73. **Rung 0 is today's still with the lens neutral**, so for a camera without lens settings it is pixel-identical to a plain path-traced final; the lens is the camera's own from rung 1 on. Photoreal never invents a lens or an aperture (it must not lie about the design).
74. **One lens function.** `arch3d::lens` (Panini that keeps the horizontal field, shift as a fraction of the height, the 36 mm thin lens) is the reference: the path tracer's ray generation ports it, the raster views post-warp with it, `pick3d`, the pane's pick and `render {samples}` call it. Verticals stay vertical for a level camera at every compression; shift is how a tall house is framed.
75. **The carriers already exist.** `Lighting` (built by `lighting_for` from the look) carries lens, film and sun disc to the interactive view, the raster Finalize, the path tracer and the pane; `render3d::look::apply_photo` maps `PhotoSettings` onto `PathOptions` for the CLI and the pane alike.
76. **Every path-tracer feature is a hook that is inert when off**: its own WGSL file and Rust module behind a frame flag, drawing no random number and doing no arithmetic when off, so existing techniques render exactly as before (check 12 compares pixels with every control named at its neutral value). The GPU material grows to 96 bytes and the frame only at its end.
77. **Linear output is an observable** (`render {pfm, pfmStage}`): radiance after the denoiser, or display-linear after the film and before the tone curve. Energy, furnace, blur and error numbers are measured there, not in 8-bit PNGs. `render {calibrate}` replaces the model with spheres, boxes and point lights for the same reason the furnace and `albedo` params exist.
78. **Material extensions come from a name table** (`arch3d::material_ext`): stainless appliances anisotropy, counters and tile clear coat, carpet and fabric sheen, glazing transmission and volume; the nine finishes of phase-14 check 01 are untouched. The path tracer reads them only under `materials: full`; glTF always exports them as KHR extensions and nothing else in the file moves. Rounded edges and variation are never exported.
79. **Noise is one number with one definition**: the 95th percentile over hit pixels of the standard error of the mean over max(L, 5 % of the frame mean). Adaptive and uniform renders stop on it, the reply reports it, and check 09 confirms it against two seeds. The clamp limits what a path adds after its first hit, in display units at the still's starting exposure, and `clampLoss` is the exact fraction of the frame's light removed.
80. **The learned denoiser is our code on committed weights.** `assets/denoise/README.md` is the specification; `render3d::tza` reads the files, `render3d::unet` is the CPU reference, `pathtrace::learned` the WGSL network tested against it. Native only; anything else falls back to the filter and the reply says so (`denoiseFallback`). Film runs on the CPU so it is deterministic on every adapter.
81. **The network's answer is kept as it is.** Nothing clamps, rescales or re-energises the learned denoiser's output; only a converged pixel keeps its input (the flat-frame rule). Two corrections were tried and removed (57ae979e4): a clamp of each pixel into its own noisy interval lost 21 % of the kitchen's light, and an energy restore after it put the noise back and took the error above the filter's.
82. **A port of a reference is held to the reference before it is tuned.** `crates/render3d/tests/oidn_reference.rs` compares our network with the output image of the official `oidnDenoise` 2.5.1 on the same inputs (five decimal places; the image is a fixture, no Open Image Denoise code is linked). Had that test come first, both corrections of decision 81 would have been seen to be fixing something that was not the network. A fix that moves one acceptance number is measured against the reference before another fix is stacked on it.
83. **A bar on a small software frame is what the reference reaches there.** The brief's half-error figure is for a GPU frame; on the software adapter's 96 × 64 kitchen the official denoiser reaches 0.68 of the filter's error, so the bar there is 0.75, and half stays on a GPU at 320 × 200 (decision 34's kin: a bar measures the work, not the host).
84. **A pixel is not converged because its samples agree.** A zero sample variance in a scene lit by rare bright paths means the pixel has not seen them yet. The adaptive estimate is taken over a neighbourhood, with a floor, and no pixel stays frozen once marked (`p22g-adaptive-robust-error`). Measure points are decided from sample counts, never the clock, so a seed gives the same image.
85. **An error against a noisy reference uses two references.** With independent halves r1 and r2, (x − r1)(x − r2) has the expectation (x − truth)², so the reference's own noise is not added to both sides of a ratio (check 10).

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
- Gaps: hard-edged shadow, `camera`'s echo form in radians.

## Phase 7 — generated elevations and sections (landed)

- `ArchView` (elevation or section) with stored `ViewGeometry`; `arch3d::views::refresh` uses the assisted `hlr` and `section` kernels (no second one).
- `elevation`, `section`, `view.delete`, `viewsheet` (a layout with one locked viewport per view at 1000/scale). Proxies `CADCRAFT-VIEW` on save.
- Numbers: south elevation 11.2 m wide, 5.474 m high; door 0.9 × 2.1; section rings 0.48 m²; ARCH D PDF 2592 × 1728.
- Checks 01–09 PASS.
- Gaps: scope is a plan rectangle.
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

- `pathtrace` on its own compute device; without compute shaders the raster Finalize with `fallback: no compute shaders`.
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
- **Icons:** `arch3d::icon` (CPU rasterizer, byte-identical). **Backdrops:** colour, gradient, image (sky and HDRI in phase 19).
- Checks 01–13 PASS.
- Open: `move` silently moves nothing for handles it cannot resolve; `arch.inspect` prints furnishing handles in the short form; `library_user::relative_value` indexes and unwraps; hull outlines; symbol lights do not emit; the path tracer uses texture means; flat icons.

## Phase 15 — electrical, split levels and the showcase house (landed)

- **Electrical:** `ElectricalDevice` (`el<n>`) on a wall face at 0.3 / 1.2 m, counter outlets at counter top + 0.15 (measured from the level's floor); plan symbols on `E-POWR`/`E-LITE`; cover plates; `IfcOutlet`/`IfcSwitchingDevice`. `Connection` (`cn<n>`) arcs; circuits; `circuit.brightness`. `outlets.auto` (1.8 m rule; kitchen GFCI 1.2 m / 0.6 m). `view.electrical`, `level.show`, `export {level | view}`, light sets.
- **Levels:** free elevations, `LevelKind` floor | landing, `Wall.top_level` spanning walls (decision 38). **Grade** (`arch::grade`, decision 40). **Pads** for landings, side splits and an entry apron. **Styles:** Clay and Technical.
- **Showcase:** `docs/arch3d/showcase/bilevel.json` replayed by `cadcraft-cli run --commands`; `tools/showcase/render_all.py`; `crates/engine/tests/showcase.rs`.
- Checks 01–14 PASS after nine were corrected by the operator (decision 42).
- **Still open:** `ifc_step_grammar.rs` has no counts for the electrical entities; connections are not exported; a level edit rebuilds every object.

## Phase 16 — ceilings and foundations (landed)

20/20 checks on 2026-11-11 after two fix cycles and the showcase rework. 37 segments and 2 upstream syncs; the lead revised the checks twice (371fd74a6, 7893ad59c); the operator landed the earlier-phase checks that phases 16–19 change (bfaa016c6).

- **Observables:** `build3d {ceiling: [[x,y], …], level: n}` (finished ceiling elevation, `null` outside every room); `build3d` `parts` (meshes per part); `arch.inspect` ceiling rows and `trays`, `soffits`, `foundations`.
- **Ceiling build-up** (decision 44): `Room.ceiling_height` is the rough ceiling; `Room.ceiling_stack` is layers top to bottom (an air gap has no material). An empty stack keeps the phase-3 20 mm solid. Templates `drywall` (12.7 mm) and `suspended` (drywall 12.7, air 200, tile 16).
- **Vaults and the guard** (decision 45): `room {flatCeiling: false}` follows the roof underside; every bounding wall rises to it (wall-rise meshes in their own incremental section); ceiling items hang plumb. Edits that would put a flat ceiling above the underside are refused naming the room; roof edits never are, and leave `ceilingWarning`.
- **Overhead:** ceilings and wall tops are clipped at the slab underside of the level above; under an Open Below room the ceiling is that room's.
- **Trays:** `Tray` (`tr<n>`): a room or a drawn polygon, `inset`, `depth`, `sides`; nest; refused whole if the top would cross the roof underside or the room has a level above. `tray`, `tray.edit`, `tray.delete`.
- **Soffits:** `Soffit` (`sf<n>`): rectangle or polygon, bottom elevation or depth, own material (`soffit-box…`); snaps to wall faces; `soffit {over: [cabinets]}`; dashed on `A-CLNG`; ceiling items hang from its underside and are re-seated when it moves.
- **Foundations** (decision 47): `foundation {type: stem | slab, footingDepth?: 1.2}` (`fd<n>`). Stem: 0.2 m concrete wall flush with the wall above down to a 0.6 × 0.25 m footing. Slab: 0.1 m slab with a 0.3 × 0.3 m thickened edge. Basement walls take the `Concrete 0.2` type and a footing under the basement floor structure. Footings dashed in plan, filled in sections.
- **Rebuild and exports:** new incremental sections; `scene_cache_equals_full` gains the edits; IFC `IfcFooting`, foundation `IfcWall`, soffit `IfcCovering` (CEILING); DXF proxies (`io::dxf_ceiling`).
- **The showcase rework (checks 14–20):** stairs and wells by elevation (decision 39); owned Open Below rooms (decision 46); above-grade area by Chief's living-area convention; technical silhouettes from the camera; Clay and Technical in the pane; the split entry's flights and Foyer.
- **Demo:** `phase-16-demo.json`; since `p21t-showcase-house-c` the showcase house has a vaulted Living room, a Dining tray, a soffit over wall cabinets `cb6` and `cb7` (bottom 3.634), a suspended Lower Utility ceiling and a stem foundation, judged by `the_showcase_has_its_phase16_ceilings_and_foundation` and rendered in the gallery of 2026-11-16. USER-GUIDE.md *Ceilings and foundations*.

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
- **The lead has not looked at a render of the redrawn house** (the gallery of 2026-11-16 and 2026-11-23 is the operator's run).
- **Smaller items:** no grips on trays, soffits or foundations; a level edit rebuilds every object; `ifc_step_grammar.rs` lacks `IfcFooting` and the electrical entities.

## Phase 17 — stairs, decks, railings and openings in detail (landed)

14/14 checks on 2026-11-12 after one fix cycle. The 13 planned segments, the fix cycle (p17f-railing-auto-contact, p17f-grille-top-part, p17f-showcase-phase17), two upstream syncs and harden passes (more harden passes on the phase-16 and phase-17 segments merged during phase 22); checks revised once (3e4c8b77e).

- **One stair convention** (decision 49): tread k has its top at k·rh and spans u in [(k−1)·td − 0.025, k·td]; nosing_v(u) = rh + (u + 0.025)·rh / td.
- **Stairs in detail:** treads 25 mm with a 25 mm nosing; risers closed or open; stringers `two`, `centre` or `none` (a skirt on a wall side). `stair {…, openRisers?, stringers?, underside?, railLeft?, railRight?, planCut?}` and `stair.edit`.
- **Wall under a flight:** `arch::flight_cut` finds the walls; `arch3d::flight_cut::apply` cuts the wall's elevation profile `underside` (default 0.3 m) below the nosing line. No CSG.
- **Plan symbol** (`arch::stair_symbol`): solid nosing lines under the plan cut (`Stair.plan_cut`, default 1.2), a break line, dashed beyond, a walk arrow `UP`; on the upper level the part seen through the well, `DN`. In the plan, exports and DXF proxies (`CADCRAFT-STAIR`).
- **Railings as built:** 90 mm posts at ends, corners and turns and at most `post_spacing` (default 1.8) apart in equal bays; top rail; bottom rail 0.05 m up; balusters or a panel; `inset`. Fields `inset`, `host`, `auto`.
- **Automatic railings** (decision 50): `railing_auto::regenerate` from `stair`, `stair.edit`, `deck`, `deck.edit` only. Per side `auto`, `on` or `off`. Wall contact is real contact with a wall face.
- **Decks you can stand under:** the frame is derived (`arch::deck::frame`, `arch3d::deck_frame`): ledger, 38 × 184 joists in bays ≤ 0.406, rims, a 76 × 235 drop beam 0.3 m back, 0.14 m posts (ends 0.3 m in, bays ≤ 2.4) on 0.3 m piers, 25 mm boards. No wall: joists span the shorter side with a beam at each end. Grade is `SiteSurface` or z = 0. `deck {boards?, rails?}`; `arch.inspect {decks}`.
- **Openings in detail:** grilles (`grille {across, down, part?: top}`, 20 × 20 mm bars); tops `square`, `round`, `segment`, `roof` (`arch::opening_top`; the rough opening is cut to the shape; a roof top's height is derived when the window is placed or moved); door style `fixed`; sill and casing sides.
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
- **`ifc_step_grammar.rs` has no counts for `IfcMember`, `IfcColumn`, `IfcRailing` or `IfcFooting`.**
- **Stair shapes:** the detailed parts appear to be built for straight flights only.
- **Roof-top windows** keep their stored height after a roof re-pitch until placed or moved again.
- **Automatic railings** are re-decided only by the stair and deck commands.
- **No grips** for stairs, railings or decks.

## Phase 19 — light, sky and cameras (landed, headless and in the pane; on the showcase script and in the gallery)

17/17 checks on the swarm's record (2026-11-15, again after the `p19t` round); the lead did not re-run them. 81 merges: 19 planned packages as 40 segments, fix rounds `p19f` to `p19t` (36 segments), `p18i-showcase-phase18` and four upstream syncs. Checks planned at 7de67d3fa and revised twice (2e7bf6d00, a6e011a69). Decisions 62–68 came out of it.

- **The look** (decision 62): technique (standard, path, clay, technical; photoreal since phase 22), exposure (auto or EV100), tone map, sun (`SunLook`: on, lux, shadows, a named angle or azimuth and elevation), sky (`SkyLook`: on, turbidity, stars, `MoonLook` 0.25 lx), light set, cutaway, clip box, near clip. `look {…}` edits the drawing's look; `sunangle` stores named angles; `render`, `illuminance` and `pick3d` take the same keys as one-off overrides. A render reply echoes its look, its `finalMode` and how many objects a filter hid.
- **Exposure and physical light** (decision 63): linear value = luminance × 1/(1.2 · 2^EV100), then ACES (default), Hable or none. Auto: 0.18 / log-average luminance, clamped. The lit raster is physical: sun and fixtures in lux, ambient from the environment's SH or a night stand-in. The sun: lux on a facing surface (default 100,000), cast shadows on or off, off altogether.
- **Sky, moon and HDRI** (decision 64): `arch3d::sky` (Preetham's closed form, a twilight fade from 0° to −6°, night with a moon and optional stars). `arch::hdr` reads Radiance RGBE; HDR images are stored in the drawing (at most 1024 × 512 each, 4 images, 8 MiB, pruned). `backdrop {kind: sky}` and `backdrop {kind: hdri, item, intensity, rotation, horizon}`. `render3d::Environment`: equirect, linear nits; the path tracer samples it by importance with MIS, the raster uses 9 SH coefficients. The CC-0 HDRI is library item `polyhaven:kloofendal-puresky`, with its rows in `ATTRIBUTION.md`.
- **Ground to the horizon** (decision 65): `Scene3d.horizon`, part `horizon`.
- **Strip lights:** `LightKind::Strip` (`LED strip`): a 2D path at a height, `under: [cabinets]`, `room` + `inset`, or `tray`; lumens per metre, colour temperature, facing down or out. Sampled every 0.15 m; the raster draws one budgeted light with up to 128 segments, never shadow-mapped; the path tracer makes one segment light each.
- **Normal maps:** brick, siding, shingle and wood textures have generated tangent-space normal maps; glTF materials carry `normalTexture`.
- **Seeing in** (decision 66): `ViewFilter`; the filtered scene is used by the interactive pass, the raster Finalize, the path tracer, `pick3d` and the pane.
- **Cameras keep their look:** `camera {save, kind: full | overview, …look keys}`, `camera.update`, `camera.orbit`, `camera.reset`. An overview camera has `level`, `lowerLevels` and no ceilings or roof. A camera whose technique is `path` path-traces on `render` without `final`.
- **The 3D pane** (the `p19t` round; decisions 67, 68): `refresh_look` and `apply_filter` on every sync; `look_row` in `draw` (Cutaway, Exposure, Tone map, Sun angle, Sky, Clip box as six numeric fields, Near clip, Reset camera, Floor overview), each change one queued command run by `apply_look`.
- **GPU layout:** `PbrUniform` (128 B), the frame uniform (256 B), `GpuFrame` and the shader hooks were fixed by `p19-r3d-groundwork`; later changes add fields only at the end. Compute pipelines are built once per device.
- **Demo:** `phase-19-demo.json` (a small house). On the showcase house since `p21t-showcase-house-c`: sun angles `dusk` (azimuth 300°, elevation −3°) and `noon`, three LED strips, sky and HDRI backdrops, cameras `dusk` (path-traced at EV100 8 since 2026-11-23: the raster at EV100 4 washed the sky out), `noon-hdri` and `overview`; `Front` stays active. Rendered into the gallery as `camera-dusk.png`, `camera-noon-hdri.png` and `camera-overview.png` (b21761b85). USER-GUIDE.md *Light, sky and cameras*.

### Numbers locked
- **Exposure:** the furnace box (radiance 2.0), tone mapping off: 0.8333 (sRGB 235) at EV100 1 and 0.4167 (173) at EV100 2, ± 2 %.
- **Sun:** at solar noon (62.39°), sky off, direct light on level ground 88,612 lx ± 1 %; with shadows off the phase-10 column no longer darkens the floor behind it.
- **Sky:** blue > red at noon; red > blue toward the sun at 2°; at −10° the brightest sky pixel is under 1 % of noon's (compared at EV100 8).
- **HDRI:** a uniform radiance-1.0 environment over an albedo-0.5 plane at EV100 −1, tone mapping off: 0.8333, ± 2 % path-traced and ± 5 % raster.
- **Strip:** 2 m of 500 lm/m facing down at 2.4 m: 49.72 lx ± 2 % under its centre.
- **Horizon:** from 1.6 m looking level, the row 5 px under the horizon has no sky or clear-colour pixel.
- **Cutaway and clip:** a pick at a camera-facing window's pixel never returns that wall or window; with a clip box ending at z = 2.0 a pick from above at a room's centre returns its floor.
- **Cameras:** a saved camera re-renders the same image (fixed seed) after the global exposure, sun and light set change; `camera.reset` after an orbit gives the saved image.

### Phase-19 gaps
- **The clip box is not dragged in the 3D image:** six numeric drag fields (not translated, axes as numbers); no face handles in the viewport.
- **Pane controls not built:** a light-set chooser, a backdrop chooser, sun lux and shadows, stars and moon. The look-row widgets are not clicked by a test.
- **Raster limits:** strip lights cast no shadows; environment light is diffuse SH only; one sun shadow map, hard-edged.
- **Not confirmed by a check or at a checkpoint:** the moon's light and the stars in an image; clay and technical techniques from a saved camera on the web build; a sheet's camera image under a camera look.
- **Out of scope by the brief:** upscaling and DLSS, animated water and pools, shadow animation, volumetrics, physical-camera exposure triangles. (Depth of field, grading, clear coat, anisotropy and translucency came in phase 22.)

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
- `arch.inspect {plan: true | <view>}` reports it; `arch.inspect {planViews: true}` lists the views. Made at the shown level (`plan_level`).

### Reference display, reflected ceiling plan, roof plan, room labels
- A view can draw another level's walls beneath in a muted colour (`reference: {level, color}`).
- **Reflected ceiling:** tray outlines, the pitched top ring and a height label; vault lines; soffits and ceiling items, each drawn once; a room's ceiling text is its finished ceiling or, vaulted, its pitch. Nothing is mirrored.
- **Roof plan:** ridges, hips and valleys; eaves and rakes from the plane rings; each plane's pitch as a label (`x/12`) with a slope arrow.
- **Room labels:** name, area, dimensions and ceiling height per plan view (`RoomLabelShow`); `room.label {room, area}` picks the area. Label text uses `m2` and ` x `.

### Dimensions (decision 55)
- `AssocSnap::Arch {feature: centre | leftFace | rightFace | edge(i), along}` attaches a definition point to an arch proxy handle (DXF assoc xdata snap code 7). `arch.inspect {dimensions: true}` computes each measurement from the geometry.
- **Auto exterior dimensions:** `dimauto {level?, on?, rows?}`; per level and side up to three rows beyond the outer face (openings 1.0 m, walls 1.5 m, overall 2.0 m) on `A-ANNO-DIMS` in the style `Arch`; the walls row splits at interior walls' centre lines. A row edited by hand is marked edited and left alone. `arch.inspect {autoDims: true}`.
- **Driving:** `dimdrive {dimension, value, object?}` moves the object at the near end; the far end stays. It moves walls (with joined and T neighbours), openings, cabinets, fixtures and furnishings; it does not re-detect rooms.

### Demo
`phase-20-demo.json` and `phase-20-showcase.json` (applied after `bilevel.json`: floor plan, electrical and reflected ceiling views for Lower, Main and the Entry landing, and `dimauto` twice). In the gallery since 2026-11-16 as `planview-<level>-floor.png` / `-rcp.png`, `planview-roof.png` and `planview-site.png`. USER-GUIDE.md *Plan views* and *Dimensions*.

### Numbers locked
- **Auto dimensions:** overall 10.2 on south and north, 8.2 on east and west; the north walls row 6.1 + 4.1.
- **Driving:** the 6.1 dimension set to 5.6 → the wall at x = 5.5; north-west interior 25.9475 m², north-east 21.0975 m²; the row reads 5.6 + 4.6.
- **Associative:** the phase-4 window moved 1 m changes its locating dimension by 1.0. A base cabinet on w2 at offset 8.0 reads 1.6, then 2.1.
- **Roof plan** (hip at 6/12): one ridge of 2.0 m, four hips of 5.79828, four `6/12` labels.
- **Reflected ceiling:** trays tr0 0.7…5.35 × 3.65…7.3 and tr1 1.3…4.75 × 4.25…6.7; the label is 2.70; the suspended room reads 2.17.
- **Plan views:** the Reflected Ceiling SVG has no entity on `I-FURN`; the Electrical view matches phase 15's results.

### Phase-20 gaps
- **Out of scope by the brief:** framing plans, story poles, construction lines, shadow plans, interior auto-dimensions other than phase 21's cabinet runs.
- **Auto dimensions** cover only the bounding box's four sides; **driving** is a command (no in-place text edit confirmed) and leaves rooms stale until `roomdetect`; **no grips** on plan-view objects or auto rows.

## Phase 21 — schedules, elevations and sheets (landed)

13/13 checks on the swarm's record (2026-11-15, again after the `p21t` round on 2026-11-16); the lead did not re-run them. 22 planned segments; fix rounds `p21f`, `p21g`, `p21h`, `p21t`; checks planned at 74319172d and revised twice (86ec898b5, acbc591ec).

- **Schedule numbers and schedules** (decisions 14, 57): `arch::schedule` groups identical items and numbers them `D01` / `W01` / `C01` in order of each group's lowest id, on every read. Room-finish rows read `arch::paint::effective_material`. A `Schedule` record (`sc<n>`) and one TABLE on `A-ANNO-SCHD`, linked by xdata. `schedule`, `schedule.edit`, `schedule.delete`; `refresh_schedules` rewrites a changed TABLE inside the command's undo step. `opening.edit {id, width?, height?, sill?, style?}` re-validates placement.
- **Labels:** in plan through the phase-20 pipeline (`plan/tags.rs`, on `A-DOOR-IDEN` / `A-GLAZ-IDEN` / `A-FLOR-CASE-IDEN`, only where a view's `labels.numbers` is on); in elevations as tags on `A-ELEV-IDEN` and swing indicators on `A-ELEV-SWNG`.
- **Interior elevations:** `elevation {room, wall | side, level?, insert}` makes a `ViewKind::Interior` view; `arch3d::views::interior` restricts the scene to the room box and finds the room again by `room_point` after `roomdetect` renumbers. Automatic dimensions for each cabinet run, the overall width and the floor-to-ceiling height.
- **Material patterns:** pattern lines on `A-ELEV-PATT` from the procedural texture tiles (`texture::tile_size`), world-anchored, clipped through `hlr::visible_segments`. Plain paint draws none.
- **View notes** (decision 60): `ArchView.notes` in view coordinates or attached to a corner of an object's lines. `view.note`, `view.dim`, `view.note.delete`.
- **Sheets** (decisions 58, 59): `sheet {name, number?, title?, size?, views: [{view, scale, at?} | {planView, …} | {camera, width, at?}]}`, `sheet.edit`, `sheet.delete`, `sheet.open`, `sheetset {project, date}`. The title block is the block `ARCH-TITLE-BLOCK` on `A-ANNO-TTLB` with macro attributes. **Callouts** (`plan/callouts.rs`, `A-ANNO-SYMB`) mark placed views `<view number>/<sheet number>`. `plot` / `exportpdf {sheets: all | [names]}` write one PDF in sheet order. Phase 7's `viewsheet` stays.
- **Deferred views** (`p21t`): a session can defer regenerating elevations and sections while model-building commands run and catches up before any command that reads them.
- **A wall with a window and a niche** keeps its whole face (hole bridging in `arch3d/src/mesh.rs`).

### The gallery tool (decision 70)
- `tools/showcase/render_all.py --out DIR` writes the set phase-15 check 11 has always run: a plan per level, two electrical plans, four elevations, two sections, the four saved cameras, night, path, clay, technical, the three exports, `manifest.json` and `contact-sheet.png`.
- `--full` adds the look cameras, then one replay of the three scripts into `DIR/set.dxf` and from it the plan views, interior elevations, sheet images and `drawing-set.pdf`; since phase 22 also a replay of the house plus `phase-22-showcase.json` into `photo.dxf` and, per Photoreal camera, a ladder folder (`ladder-kitchen`, `ladder-living`, `ladder-dusk`; `render.ladder` at 640 × 400) and a 1920 × 1080 final (`photoreal-<name>.png`). `--no-set` and `--no-photo` skip those parts.
- The manifest carries `scriptSha256`, `setScripts`, `drawingSet`, `sheetImages` and, when the photoreal run was made, `photoScript`, `ladders` and `photoreal`.
- The operator refreshes the committed gallery with `python3 tools/showcase/render_all.py --full --out docs/arch3d/showcase/renders` (`CADCRAFT_RENDER_ADAPTER` chooses the renderer; the tool defaults it to `fallback`).
- **The committed gallery** (5b4e78eca on 2026-11-16, b21761b85 on 2026-11-23) is the full run of today's `bilevel.json` (manifest `scriptSha256` 668349fe… equals the file's): look cameras, plan views, two interior elevations, 14 sheet images and `drawing-set.pdf`. It has no phase-22 output.

### Demo (the showcase drawing set)
- `phase-21-showcase.json`, replayed after `bilevel.json` and `phase-20-showcase.json`: the four schedules, interior elevations of the Kitchen and Main Bath, `sheetset`, 14 sheets and a final `layout.set`.
- Sheets: A-000 Cover, A-001 Site Plan, A-101 Lower Floor Plan, E-101 Lower Electrical, A-102 Lower Reflected Ceiling, A-111 Main Floor Plan, E-111 Main Electrical, A-112 Main Reflected Ceiling, A-121 Entry Plans, A-201 Roof Plan, A-301 Elevations, A-302 Sections, A-401 Interior Elevations, A-501 Schedules.
- The command line is in `showcase/README.md`; check 13 does the replay and checks page count, page size, sheet content and schedule counts.
- USER-GUIDE.md *Schedules and labels*, *Interior elevations and view notes*, *Sheets*, *The showcase house*.

### Numbers locked
- **Schedules:** four 0.9 × 1.2 windows and one 1.2 × 1.2 → two rows, counts 4 and 1; one resized → 3 and 2.
- **Interior elevation:** the north-west room's elevation of its east wall has every segment within y 3.05…7.9 (± 1e-3). A base cabinet on w5 at offset 2.5 stands at (5.95, 5.5) and shows as its front rectangle u −5.8…−5.2, v 0…0.914 (box 0.876).
- **Annotations:** a text on that view keeps its view coordinates after w5 is stretched to x = 5.5 (the room is renumbered r2 → r5).
- **Patterns:** a brick wall shows horizontal pattern lines in the front elevation; plain paint none.
- **Sheets:** a PDF page per sheet, 914.4 × 609.6 mm (2592 × 1728 pt); the 10.2 m south face at 1:50 measures 204.0 mm in the sheet's SVG.

### Phase-21 gaps
- **Out of scope by the brief:** framing plans, revision tables, CAD detail libraries, multi-file projects, LiDAR room capture.
- **The guide omits `--full`** (decision 69): one docs segment owed.
- **The lead has not looked at the plotted pages or the look renders** in the committed gallery; no check runs the tool's full mode end to end.
- **Two sheet commands:** `sheet` and phase 7's `viewsheet`.
- **Callouts change the bare plan** of a drawing with placed sheets (decision 60).
- **Not numbered or marked:** niches and shower doors, fixed doors (no indicator).
- **Patterns** exist only for materials with a procedural texture tile.
- **Web:** a camera image on a sheet in the web build was not confirmed.
- **UI:** no schedule or sheet panel and no grips on view notes.

## Phase 22 — photoreal stills (landed; the rendered demo is owed)

15/15 checks on the swarm's record (2026-11-25); the lead did not re-run them at the checkpoint. Planned against arch3d at 2674a60f0 (dd170a3c5): 33 segments, all merged; fix rounds `p22f` (ladder-reply-look, adaptive-freeze, learned-keeps-converged, aniso-multiscatter), `p22g` (trans-shadow-side, adaptive-robust-error, learned-kitchen-error) and `p22h` (learned-energy); six upstream syncs; harden passes on most segments; the operator's commit 57ae979e4. Checks revised by the lead three times on 2026-11-24 (a5a4087c6, 140def25d, e899910d5) and corrected by the operator in several measurement set-ups. The spec file the brief names (`docs/superpowers/specs/2026-11-12-…`) is not in the tree, so the brief's text was the whole scope. Decisions 71–85.

### The preset and the ladder (decisions 72, 73)
- `technique: photoreal` on a look, a saved camera or one `render` is rung 7 of `doc::arch::photo::rung(n)`. Rungs, cumulative: 0 today (lens neutral) · 1 lens (the camera's own) · 2 edges · 3 materials full + variation · 4 12 bounces, sun disc 0.53°, clamp 32 · 5 the photoreal film (bloom 0.1, vignette 0.15, contrast 1.05) · 6 adaptive, noise target 0.02, time budget 600 s, roughness guide · 7 learned denoiser.
- `render {rung: n}` renders one rung; `render.ladder {camera, out, size?, spp?, seed?, noiseTarget?, timeBudget?}` writes `out/rung-<n>.png` and `out/ladder.png` (8 w wide, h + 24 high, a (24, 24, 24) label band with the rung names) with one seed; each rung's reply carries its look, like `render`'s (`p22f-ladder-reply-look`).
- Every existing technique renders as before: the path-tracer features are inert hooks (decision 76), and check 12 compares pixels.

### Lens (decision 74)
- Look keys `projection: rectilinear | panini`, `compression` 0..1 (default 0.7), `shift` −1..1 (fraction of the frame height; +0.25 moves the horizon a quarter of the height down the image), `aperture` (null or f-number 0.5..64), `focus` (null, metres, or `{pick: [x, y]}`, resolved to a distance along the view axis and echoed).
- Panini keeps the horizontal field of view. The thin lens is a 36 mm wide sensor: f = 0.018 / tan(hfov / 2), aperture diameter f / N, a round aperture.
- `arch3d::lens` is the reference. The path tracer ports it in `pt/lens.wgsl`; the raster views and the pane draw a wider rectilinear source frame and post-warp it (`render3d::warp`, no compute, so WebGL2 too); `pick3d` and the pane's pick cast `ray_at_lens`. Depth of field is path-tracer only.

### Edges, materials, variation (decision 78)
- **Rounded edges:** `pathtrace/edge_table` lists each object's convex edges; at a hit within the radius (default 2 mm, per material, 0 off) the shading normal blends across the edge (`pt/edges.wgsl`). No geometry; never exported. `render {edges: false | true | {radius}}`.
- **Material extensions:** `MaterialExt` (clear coat, anisotropy, sheen, transmission with thin or volume, edge radius, variation) from the name table `ext_for` or an explicit `MaterialDef`; lobes in `pt/bsdf_ext.wgsl` and `pt/bsdf_trans.wgsl`, read only under `materials: full`. The anisotropic lobe has multiple-scattering compensation (`p22f-aniso-multiscatter`); light samples through thin and rough transmission leave from the far side (`p22g-trans-shadow-side`: thin translucency had lost 20 % in the furnace).
- **Variation:** a procedural roughness and colour variation (scale and amount per material), `pt/variation.wgsl`; off unless asked or in Photoreal.
- **glTF:** `KHR_materials_clearcoat`, `_anisotropy`, `_sheen`, `_transmission` and `_volume` on the materials that have the fields; nothing else in the file moves (check 06).

### Light, film, quality (decision 79)
- **Sun disc:** `sun: {disc: degrees}` (0..10; 0.53 in Photoreal) samples the disc, so shadows soften with distance (`pt/sun_disc.wgsl`).
- **Clamp:** `clamp: null | c` limits what a path adds after its first hit, in display units at the still's starting exposure; the reply's `clampLoss` is the fraction of the frame's light removed.
- **Film** (`render3d::film`, CPU, before the tone curve): white balance (kelvin, divides by that temperature's colour normalised to green), bloom above a threshold (energy conserving), vignette v as 1 − v (d / d_corner)², grade (x − 0.18) · contrast + 0.18, saturation about luminance, lift. `film: null | photoreal | {…}`; per camera; neutral is identical to off.
- **Adaptive sampling** (`pathtrace/adaptive`, `pt/adapt.wgsl`; decision 84): at measure points decided from sample counts, mark converged pixels, report per-pixel sample statistics and the frame noise, and stop at the noise target or the time budget. The reply says `stop: samples | noise target | time budget`, with `noise`, `sppMean`, `sppMin`, `sppMax`, `samplesTotal`, `seconds`. The first version never set the flag (`p22f-adaptive-freeze`); the second froze pixels on a zero sample variance (`p22g-adaptive-robust-error`).
- **Roughness guide:** the edge-aware filter takes the roughness buffer as a third guide (`roughnessGuide`).

### The learned denoiser (decisions 80–83, 85)
- Weights: `assets/denoise/rt_hdr_alb_nrm.tza` (base) and `rt_hdr_alb_nrm_small.tza`, Open Image Denoise's, Apache-2.0, with `LICENSE-oidn-weights.txt`, the README that is the specification, and rows in `ATTRIBUTION.md`. No Open Image Denoise code is linked or copied.
- `render3d::tza` reads the files (32 tensors each; `enc_conv0.weight` 32 × 9 × 3 × 3) and is empty on wasm; `render3d::unet` holds the input transforms, autoexposure, the PU transfer function and a CPU forward pass; `pathtrace::learned` runs the network in `pt/unet.wgsl`, one compute pass per operation, in overlapping tiles when a frame's activations do not fit the device.
- `denoise: false | true | filter | learned`, `denoiseModel: base | small`; the reply's `denoiser` says what ran, `denoiseFallback` why the filter ran instead, `denoiseSeconds` how long. `denoiser.info {}` reports availability and the models.
- **What the fix rounds did.** `p22f-learned-keeps-converged`: a converged pixel keeps its value (the flat frame within 1 %). `p22g-learned-kitchen-error` added averaged guides and a per-pixel clamp of the output into L ± 2 robust sigma to reach the half-error bar; the clamp lost 21 % of the kitchen's light. `p22h-learned-energy` restored local energy and took the error above the filter's (0.42 against 0.34). The operator then measured our base model against the official `oidnDenoise` 2.5.1 on the same inputs (equal to five decimal places, mean within 1 % of the input), removed the clamp and `energy.rs`, and committed `tests/oidn_reference.rs` with its fixture (57ae979e4). `post::keep_consistent` now holds only the converged-pixel rule.

### Observables (decision 77)
- `render {pfm, pfmStage: radiance | film}` writes a linear float image; `render {calibrate: {objects, lights}}` replaces the scene with up to 16 spheres or boxes and 8 point lights in every render mode. The reply gains `photo {…}` (what the still switched on) and `computeAdapter`.
- `tests/acceptance/phase-22/p22lib.inc` has the whole command contract in its header.

### Pane, docs, showcase
- **Pane:** the look row offers Photoreal among the techniques and a Projection chooser; the view warps with the look's Panini and shift; a Photoreal look Finalizes through `apply_photo` at the top rung. `uk.tsv` has the rows. Aperture, focus and film are set by command.
- **Docs:** USER-GUIDE.md *Photoreal stills* (rung table, lens, film, render-only keys, the denoiser, the ladder; its `sh` block runs a 48 × 32 ladder); `docs/arch3d/mcp-examples/11-photoreal.jsonl`.
- **Showcase:** `phase-22-showcase.json` (after `bilevel.json`; cameras only) saves `Kitchen-photo` (fov 70, Panini 0.7, noon, Day), `Living-photo` (fov 70, Panini 0.7, f/8 focused at 6 m, noon, Day) and `Dusk-photo` (fov 60, shift 0.15, the `dusk` angle, sky, Evening, EV100 8), and leaves `Front` active. `render_all.py --full` renders their ladders and 1920 × 1080 finals; `showcase/README.md` lists them.

### Numbers locked
- **Panini:** hfov 100°, a 1 m sphere 20 m away centred 47.5° off axis: aspect 1.48 ± 3 % rectilinear, under 1.15 at compression 1.0, in all three renderers; verticals within ± 0.5 px; `pick3d` at the warped pixel returns the window drawn there.
- **Shift:** 0.25 moves the horizon 100 px of 400 down the image; poles stay in their columns.
- **Depth of field:** 60° horizontal, f/2.8, focus 3 m, 1921 px wide: f = 31.18 mm, aperture 11.13 mm; blur 6.17 px at 1.5 m and 4.32 px at 10 m ± 10 %; in focus ≥ 95 % of the light within 1.5 px; aperture off is the pinhole image.
- **Edges:** 1 m cube, sun at azimuth 120° elevation 10°: radius 0 one step; 5 mm from 0.5 m ≥ 3 px and a highlight above both faces; total energy within 1 %.
- **Materials:** furnace per lobe in [0.95, 1.02] (the loss is printed); coat peak ≥ 3×, sheen rim ≥ 1.15×, anisotropy 0.8 highlight ≥ 3:1, an emitter shows through rough and thin transmission; variation keeps the mean within 2 % (measured over about 33 × 33 noise cells).
- **Sun disc:** 0.3 × 0.3 × 3 m pole, elevation 30°: 10–90 % over 0.687 × 0.0555 m ± 15 % at y = 5.196.
- **Film:** neutral = film off; bloom within 1 % of the frame's energy; vignette 0.15 → corner 0.85, mid edge 0.904; white balance 3000 K neutralises a 3000 K fixture; mid grey 0.18 ± 2 % under the Photoreal film; contrast 1.5 takes 0.36 to 0.45.
- **Adaptive:** showcase `Kitchen` camera, ≤ 60 % of the uniform samples for the same noise; a 60 s budget stops in 55–65 s and reports `time budget`. On the software adapter the frame is 48 × 30 at a 0.40 noise target.
- **Learned denoiser:** 32 tensors per file, `enc_conv0.weight` 32 × 9 × 3 × 3; flat frame within 1 %; kitchen 64 spp error (two independent half references) ≤ half the filter's on a GPU at 320 × 200 and ≤ 0.75 on the software adapter's 96 × 64 (measured 0.68, the official denoiser's own figure there; the reference is 1024 spp on that adapter); 1920 × 1080 under 5 s on a GPU.
- **Ladder:** eight rungs, same seed; rung 0 pixel-identical to a plain path-traced final; sheet 8 w × (h + 24).

### Acceptance (all PASS on the swarm's record)
`01_lens_panini` · `02_lens_shift` · `03_depth_of_field` · `04_rounded_edges` · `05_materials` · `06_gltf_extensions` · `07_light` · `08_film` · `09_adaptive` · `10_learned_denoiser` · `11_ladder` · `12_photoreal` · `13_demo` · `14_hostile` · `15_layers_wasm_i18n`. The checks use the software adapter unless `CADCRAFT_RENDER_ADAPTER` names a GPU; 09 and 10 replay the whole showcase house and are the slow ones (check 10 took 334 s on lavapipe).

### Phase-22 gaps
- **The rendered demo is owed.** The committed gallery has no ladder and no Photoreal final (its manifest has no `photoScript`); check 13 renders a 64 × 40 kitchen ladder and two small stills. The `--full` run on the GPU host is the operator's.
- **The GPU bars are not confirmed on record:** half the filter's error at 320 × 200 and a 1080p denoise under 5 s apply only when a GPU is named; the recorded run of check 10 is on lavapipe.
- **Nobody has looked at a full-size Photoreal still;** whether the finals look right is unjudged.
- **Materials on the model side:** sheen and thin translucency have calibration scenes and a name table but no drapery object; the path tracer still shades library textures by their mean colour.
- **Pane:** only the technique and the projection have controls confirmed at the checkpoint; no aperture, focus or film controls, no focus pick in the viewport. Depth of field and film in the live view are out of scope by the brief.
- **Tiling:** the learned denoiser tiles a frame whose activations do not fit the device; a frame whose tiles do not fit falls back to the filter.
- **Out of scope by the brief:** upscaling, volumetrics, caustics, animated renders, hair or fur, people, an exposure triangle, polygonal bokeh, C++ or closed-binary denoisers.

## Open items vs Chief Architect

- **Showcase (first):** the `render_all.py --full` run with the phase-22 ladders and finals, committed; a look at those, the plotted pages and the look renders; the guide's sentence about `--full`.
- **3D pane:** clip-box face handles dragged in the viewport; light-set and backdrop choosers; sun lux, shadows, stars and moon controls; aperture, focus and film controls; tests that click the look-row widgets; axis names on the clip fields.
- **3D view and rendering:** SSAO/TAA/cascaded and soft shadows in the raster; shadows from strip lights in the raster; specular environment reflection in the raster; raster window glass opaque; textures in the path tracer; shaded icons; upscaling; volumetrics and caustics.
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
- **Library:** `move` refusing handles it cannot resolve; `arch.inspect` handles in the form commands take; furnishing outlines beyond the hull; emitting symbol lights; fabric and drapery items for the sheen and translucency lobes.
- **Rebuild:** a level edit that rebuilds only what it touches.
- **Georeferencing:** transforms between coordinate systems.

## Phase plan

0–17 ✅ → 20 ✅ plan views and dimensions (2026-11-13) → 21 ✅ schedules, elevations and sheets (2026-11-15; `p21t` round 2026-11-16) → 19 ✅ light, sky and cameras (2026-11-16) → the gallery re-rendered with the drawing set (operator, 2026-11-16 and 2026-11-23) → 22 ✅ photoreal stills (2026-11-25; the rendered ladders and finals owed) → 18 interior finish, kitchen and bath (planned segments and fix rounds through `p18i-showcase-phase18` merged; checkpoint pending).

Next, in order: the operator's `render_all.py --full` run on the GPU host with the phase-22 ladders and finals, and the commit; a look at the three finals and ladders, with fix segments for what shows; one docs segment for the guide's `--full` sentence; the phase-18 checkpoint.

## Building blocks added outside the swarm (assisted)

- **`geom::skeleton`, `roof_skeleton`, `weighted_skeleton`** (2026-10-25): straight skeleton, mitred inset/outset, per-edge pitch and gable ends; phase 4 is built on it.
- **`render3d` drawing and the 3D pane** (2026-10-25): `Offscreen` (`create`/`with_device`/`encode`/`render`/`read_png`), flat shading from each triangle's plane, `headless_instance` on `Backends::PRIMARY`, one shared GPU test device; wgpu 30.
- **Feature edges, HLR, section cuts** (2026-10-25): `arch3d::edges`, `hlr` (`OrthoView`, exact `visible_lines`, `merge_collinear`), `section` (`Plane`, `section_cut`, `section_view`). Phase 21 added `hlr::visible_segments` on the same kernel.
- **`geom::tin`** (2026-10-30): lifted from zorgglubb/cadcraft's `cadcraft-terrain` (MIT OR Apache-2.0, credited in `ATTRIBUTION.md`); the site data model and phase-9 stubs.
- **Phase-10 groundwork** (2026-10-31): the light data model, catalogue and photometry, the lit pipelines' group 2 and shadow atlas contract, `RenderHooks`.
- **Phase-11 groundwork** (2026-11-01): area lights, IES, `render3d::pathtrace` skeleton, `crates/gpu-optin`.
- **Phase-12 groundwork** (2026-11-01): `io::step`, `io::ifc` spine, walls in full, element stubs.
- **Phase-14 groundwork** (2026-11-03): the sample set `assets/library/sample/` (49 items, CC0 and CC-BY, `catalog.json`), `tools/library/` (run by the operator), CHIEF-REFERENCE §6.
- **Operator fixes, phases 10–15:** recorded in this file's history (97e939f25). Recurring causes: tests asserting substrings malformed output also contains (decision 32), a cache patched beside the build (33), process-wide hooks (35), checks never run before locking (42), a test segment editing what it judges (43).
- **Phase-16 groundwork** (2026-11-08, 78f95deaf): room `flatCeiling`/`ceilingStack` and the finish stack; the ceiling point query with stubs; `build3d` `ceiling`, `probe` and `parts`; the Tray/Soffit/Foundation data types; the planar geometry kit; empty incremental sections; checks 01 and 02, `p16lib.inc`. Also CHIEF-REFERENCE §7 for phases 16–21 (0dcace523). The CC-0 sky HDRI for phase 19 (restored by `p19-hdri` with its ATTRIBUTION rows).
- **Phase-16 operator work:** the earlier-phase checks that phases 16–19 change (bfaa016c6); phase-14 check 01 pinning the nine finishes for phase 18 (52a669156).
- **Phase-17 operator work:** phase-5 checks 03 and 07; a stray scratch test removed (7efb9bd81); the showcase gallery re-rendered (b97e23f24); phase-16 check 20 notes, rather than fails, renders of an earlier script (f111186e3).
- **Phase 19:** phase-13 checks 02 and 13 leave `-normal` images out of the GLB image lists (operator).
- **Phase 20:** no operator groundwork and no operator fix recorded.
- **Phase 21:** the operator fixed the `# needs:` lines of checks 04, 09 and 10 and split p21-observables in three; in the `p21t` round the operator's branches landed the redrawn house, the niche fix and the gallery tool. The gallery re-rendered from the redrawn house with the drawing set (5b4e78eca, 2026-11-16) and with the path-traced dusk camera at EV100 8 (83a3e679a, b21761b85, 2026-11-23).
- **Phase-22 groundwork:** the denoiser weights and their README, licence and ATTRIBUTION rows under `assets/denoise/`.
- **Phase-22 operator work:** check corrections (08dd05a9a: check 09 fails fast while adaptive sampling is missing; 5cfc02d4b: `obj()` puts segments on the calibrate object; d91625499: the sun disc compared to 1e-6 in checks 11 and 12; 6f2d4b2ff: the kitchen reference is 1024 spp on the software adapter; 67a805706: check 09's software-adapter frame and noise target); and 57ae979e4: the learned denoiser's output is kept as it is, `energy.rs` removed, `tests/oidn_reference.rs` and its fixture (an image made with the official `oidnDenoise` 2.5.1, in `ATTRIBUTION.md`), check 10's 0.75 bar on the software adapter.

## Phase 18 plan

Phase 18 (interior finish, kitchen and bath) was planned against arch3d at febe33d on top of the phase-17 plan: 20 segments, several split by size, checks 01–20 under `tests/acceptance/phase-18` with `p18lib.inc`. Status: the planned segments are merged with fix rounds `p18g-*`, `p18h-*` and `p18i-showcase-phase18` (the showcase house gains the phase-18 kitchen and bath features; the demo a door-less glass panel); the checks were revised by the lead (d63b89feb); **the checkpoint is pending**. The segment list is in this file's history.

- Earlier checks, landed by the operator: phase-13 check 02's last part exports the walls alone (a detected room's floor now takes its type's template finish); phase-14 check 01 pins the nine finish items.
- Room-type finishes are derived, never stored: `cadcraft_arch::room_finish::template(RoomType)` gives floor / wall / ceiling finishes (walls Drywall, ceilings Plaster, floors wood / carpet / tile / concrete by type; Kitchen floors are tile); `unpainted_finish` resolves a surface's material as paint > explicit finish > template > default. A type change clears the phase-13 floor_finish. In 3D only floors take the template texture; wall faces and ceilings keep their unsplit meshes while the surface rows and the eyedropper report the template finish. Finish gains Concrete, Tile, Carpet, Drywall, Plaster (nine in all); render3d's texture array keeps 256 layers.
- Moldings: profiles are library items of kind `molding` (five built in, generated in Rust, folder Moldings of Cabinet parts; a closed LWPOLYLINE through `library.add {kind: molding}`) with a `profile` point list; one mitred sweep (`cadcraft_arch3d::sweep::sweep`) builds room moldings, molding lines and cabinet crowns. Room moldings are a per-room list (RoomMolding { kind, profile, offset }); their runs (`molding_runs::room_runs`) follow the room ring's wall faces, break at opening trims that overlap the band (trim = width + frame + the casing on that face, from `opening_trim`), and a face carries a band only when the wall's top reaches it and the band lies on a solid part. The crown follows the finished ceiling, wraps opted-in soffits and runs round trays; a soffit can opt out (Soffit.no_moldings). Molding lines are objects ml<n> (0xD0000) with inset and flip. Lengths are 2D path lengths.
- Countertops: automatic counters stay derived from `casework_layout::counter_runs`; `countertop.detach` turns one into a Countertop object (ct<n>, 0xC0000) that claims its cabinets; edges move by a typed distance, bulge by a sagitta, take an edge profile and turn down as a waterfall (material countertop-waterfall). `elevation` is the counter's top above the level floor. Areas are outline areas (arcs included, cut-out holes not subtracted). Backsplashes (bs<n>, 0xC8000) are recomputed from the counters: tops at the wall cabinets' bottoms over them and counter top + height elsewhere, or the ceiling; Cabinet.backsplash_off removes a cabinet's stretch.
- Cabinets: Fronts::Open, a stored FaceLayout (rows of door / drawer / false / opening / appliance, heights, across; 3 mm reveals, 0.038 frames for framed inset), Construction / FrontMount, CornerPref, shelves, filler, crown / light rail. Options are parsed once per `cabinet` / `cabinet.edit` call and applied by a fallible `cabinet_opts::apply` before validate_cabinet. Door pairs start above 0.686 m (was 0.6). Corner cabinets are derived from geometry (`casework_corner::corner_info`) and `merge_corners` unions the two runs' outlines into one counter. Fillers are cabinets with `filler: true` (width (0, 0.15]), made only by cabinet.fill. Face appliances are face rows of kind appliance with the item's height (microwave drawer 0.381, wall oven 0.737, warming drawer 0.254; 0.762 wide); a tall cabinet takes two.
- Incremental rebuild: CabinetIn holds projections (CabinetNeighbour) of the cabinets another cabinet's meshes read plus the claimed ids; RoomIn stores the room with its moldings emptied; four new sections (room moldings, molding lines, countertops, backsplashes) hold resolved inputs.
- Owners by material: ObjectKind / ArchKind gain Countertop, Backsplash, MoldingLine; `incremental::object_kind_of(mesh)` and `probe::arch_kind(mesh)` resolve a painted `paint:countertop/..` / `paint:backsplash/..` key first, then material prefixes, then phase 17's deck rule and the part fallback. PickHit gains `owner`. Part names: molding, counter, backsplash, glass, cabinet, wall.
- Glass and pony walls: wall type `Glass` (0.01 thick, 2.0 high, material glass-wall); `wall.split { height, lower?, upper? }` keeps the lower part as the wall's own type with its joins and builds the upper part as a centred straight prism of its own thickness. Niches are OpeningKind::Niche (a `niche` command; refused unless the wall is >= depth + 0.012 thick and the sill >= 0.01), cut exactly width x height and filled back by a back block plus a 1 mm `niche-back:o<n>` skin. Shower doors are door style `shower` (0.7 x 1.9, frameless), only in glass walls or above a glass-topped split.
- Exports: GLB / OBJ nodes molding-r<n>-<kind>-<i>, molding-ml<n>, countertop-ct<n>(-waterfall), backsplash-bs<n>, wall-w<n>-glass, wall-w<n>-niche-o<m>, cabinet-cb<n>-crown | lightrail | frame | shelves | appliance. IFC: IfcCovering SKIRTINGBOARD (base) / MOLDING, one IfcMaterial 'Glass' on glass walls, countertops and backsplashes inside their owner cabinet's IfcFurniture, a niche as an IfcOpeningElement .RECESS. DXF: the arch payload carries the new collections; molding lines on A-FLOR-TRIM.
- Open decisions taken: the template is code, not a file; molding lengths are measured along the wall faces; fillers never resize cabinets; the backsplash's default material is `default:backsplash`; moldings run on solid faces only; the demo house is phase-18-demo.json and the showcase gets the same features additively, its glass walls after the last roomdetect so room ids stay.
- Numbers: trim 0.96 so base 13.04, crown 14.0, chair 13.04; crown 15.2 / 12.0 with the soffit, 23.04 with the tray; island area 1.125, +0.3 x 1.8 = 1.665, segment with R 2.125; backsplash tops 1.372 / 1.364; three drawers 3h + 0.006 = 0.776; 0.762 -> 2 doors, 0.610 -> 1; the oven row 0.737 of a 2.034 face; corner counters 2.046875 (blind) and 2.234375 (lazy susan); the filler run 0.78125; glass probes 1.0 / 2.05; the niche's six probes.

## Planning lessons kept from the phase-19 and phase-22 plans

- **Phase 19** (19 packages in five waves, checks at 7de67d3fa; the plan's text is in this file's history): the last wave's UI package had no acceptance check and its own tests were its only judge (decision 67); it was redone as three narrow segments, each naming the pane function its tests call. The demo was planned on a small house against decision 48 and reached the showcase only two segments later (decisions 52, 68).
- **Phase 22** (33 segments, checks at dd170a3c5; the plan's text is this file at 57ae979e4): the kernel-hooks segment was the critical path (types → hooks → material map → lobes, edges, variation) and held; adaptive, sun disc, clamp, film and the denoiser hung off it in parallel. The checks were written from the code before the commands existed, and their defects showed where the plan said they would: measurement set-ups (the variation window, the calibrate object's segments, the sun-disc comparison, the reference's own noise in check 10), not the product. The one number the plan flagged as not guaranteed (half the filter's error at the software adapter's small frame) was the one that cost three fix rounds; the answer was a reference test, not a fourth tuning round (decisions 81–83). Next time a port of a published network or formula gets its reference test in the first wave.
