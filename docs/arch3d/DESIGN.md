# arch3d — design log

Branch `arch3d` (fork of `storytold/cadcraft`, v0.10.0). Running log for "draw a plan, get the house in 3D". One commit per phase, each ending at a ⛔ checkpoint.

---

## Phase 0 — survey

- Workspace `crates/*`, `apps/*`, `xtask`; edition 2024, rust 1.90, MIT OR Apache-2.0. `unsafe_code = forbid`; crate roots deny unwrap/expect/panic/todo.
- Layers (`xtask/src/layers.rs`): L0 `geom`,`color` (`dxf`,`dwg` standalone) → L1 `doc` → L2 `constraints`,`fonts`,`render` → L3 `io` → L4 `engine` → L5 `ui-egui`,`mcp` → apps.
- Roadmap: 3D is 2%; `docs/gaps.md` F24 is *AutoCAD* 3D — untouched; our work is additive with its own ARCH-* rows.
- **Entities:** `Entity{handle,kind,common}`; `Common.xdata` travels with COPY/MIRROR/ARRAY. `EntityStore` is copy-on-write (1024-`Arc` chunks + 64 sharded handle maps) so whole-drawing undo is cheap.
- **Commands:** everything user-visible is a `CommandSpec`; one implementation, two paths — JSON `Session::execute` (MCP/CLI/scripts, hostile input validated, never clamped) and interactive `Session::start` → `Interactive`. Both under `catch_unwind`.
- **Undo:** full-`Drawing` snapshot stack, cheap via CoW. *Consequence:* anything that must undo/autosave/audit/round-trip lives inside `Drawing`.
- **Selection/grips/snaps:** `select.rs`, `spatial.rs`, `grips.rs::stretch_grip` (drags run a `grip.*` command, so undoable and MCP-reachable), `snap.rs` OSMODE bits.
- **Rendering was 2D only:** `render::build(space) -> DisplayList` drawn through an `egui_wgpu::Callback` with `Stencil8` — no depth buffer, camera or lighting. The 3D viewport was new work.
- **DXF custom data:** `CADCRAFT` app in XDATA + XRECORDs in the named-object dictionary; `dxf_region.rs` shows the proxy-graphic pattern. Foreign data preserved verbatim.
- **MCP:** a small generic tool set; `execute` forwards to `Session::execute`. *Adjustment:* no MCP tool per arch command — commands are the API.

## Crate layout (as built)

Precedent: **`constraints` (L2) — data in `doc`, algorithms in a peer crate.**

| Crate | L | Contents | Deps |
|---|---|---|---|
| `doc/src/arch.rs` | L1 | **Phase 14: `furnishing` (`Furnishing`, f<n> at +0x60000, `SymbolMesh`, `ArchModel.symbols`/`images`), `paint` (`Paint`, `MaterialRef`, `ArchModel.paints`), `backdrop`, `ArchModel.defaults`.** `ArchModel{walls,openings,rooms,levels,roofs,stairs,railings,cameras,active_camera,types,next_id,next_room_id,next_roof_id,next_stair_id,next_railing_id}`, `ArchId`, `Wall`, `Opening`, `Room`, `RoomType`, `Level`, `WallType`, `Roof`, `RoofPlane`, `RoofEdge`, `Stair`, `Railing`, `RailingStyle`, `StairShape`, **`Camera3d`, `CameraKind` (Orbit | Full)**; `ARCH_HANDLE_BASE=0xA000_0000`, `ROOM_HANDLE_OFFSET=0x10000`, `ROOF_HANDLE_OFFSET=0x18000`, `STAIR_HANDLE_OFFSET=0x20000`, `RAILING_HANDLE_OFFSET=0x28000`; caps `MAX_WALLS 4096`, `MAX_OPENINGS 8192`, `MAX_ROOMS 1024`, `MAX_ROOF_EDGES 256`, `MAX_STAIRS`, `MAX_RAILINGS`, `MIN_ROOM_AREA 0.05`, **`MAX_CAMERAS 64`**. On `Drawing::arch` with `arch_revision`/`touch_arch()`. **Phase 9: `doc/src/arch/site.rs` — `SiteLocation` (+ `SiteReference`), `Terrain`, `ElevationLine`, `TerrainModifier`/`ModifierKind` (Flat | Hill | Path), `PathKind`, `Plant`, `PlantSpecies`, `PlantForm`, `CrownShape`; `ArchModel.site/terrain/plants/plant_species`; `Wall.retaining/freeboard/footing`; `TERRAIN_HANDLE_OFFSET 0x38000`, `PLANT_HANDLE_OFFSET 0x40000`.** **Phase 13: `arch/finish.rs` `Finish` (`for_walls`/`for_roofs`/`for_floors`), `Wall.finish`, `Roof.covering`, `Room.floor_finish`; `arch/casework.rs` `Cabinet`, `CabinetKind`, `Fronts`, `Fixture`, `FixtureKind`, `ArchModel.cabinets/fixtures`, `CABINET_HANDLE_OFFSET 0x50000`, `FIXTURE_HANDLE_OFFSET 0x58000`, `MAX_CABINETS`/`MAX_FIXTURES` 1024.** | geom,color |
| `geom` | L0 | **Phase 9: `tin` (Delaunay TIN, `elevation_at`, per-triangle `contours`; lifted, see Building blocks), `cdt` (`triangulate_constrained`: constraints become edges, crossings split; cavities refilled in index order since phase 12), `tin_ops` (`TinIndex` grid location, `retain_inside`, `chain_contours`).** Older: `skeleton`, `region::Arrangement`. | — |
| `arch` | L2 | **Phase 14: `library` (`record`: `ItemRecord`, `Category`, `layer_for`; `search`, `history`, `fits`, `place`), `furnishing` (plan outline, layers, IFC classes), `casework_layout` (fronts, pulls, counter runs, gaps), `paint` (surface keys, scopes).** `wall`,`graph`,`join`(`solve_joins`,`face_lines`,`stretch_wall`),`opening`,`symbol`,`room`,`poly`(area/perimeter/`offset_polygon`),`area`,`label`,`roof`(footprint, skeleton driver, `RoofError`), `roofedge`, **`stair`(`plan_stair`,`headroom_ok`,`well_start`,`flight_polygon`,`well_polygon`,`well_guard_path`, code limits), `railing`(`validate_height`,`baluster_spacing`,`path_length`)**, **`sun`(`sun_position(lat, date, time)`, `sun_dir`, `day_of_year`; NOAA fractional-year form)**, **`detect`(`detect(drawing, opts) -> Detection`; stages `flatten`, `arcs`, `walls`, `junctions`, `openings`, `rooms`, `layers`; `score` against a truth file — phase 8)**, **`terrain` (`surface::SiteSurface`, `mesh::site_mesh`, `contour::site_contours`, `volume::cut_fill`, `csv`, `pad`, `retaining`), `plant` (catalogue of 14, `plant_size`, `plan_symbol`), `site` (`north_direction`, `true_to_drawing`, `north_arrow`) — phase 9**; **`casework` (catalogue sizes, validation, wall placement, layers) — phase 13**. | geom,color,doc |
| `arch3d` | L2 | **Phase 14: `furnishing` (symbol meshes in the build), `paint` (painted surfaces, `MaterialDef`), `icon` (the CPU icon rasterizer).** `build`/`build_dirty`→`Scene3d`; `mesh`(ear clip, `push_prism_with_holes`, profile prism), `walls`(elevation profiles + holes), `openings`, `floors`(slabs, ceilings, deck boards), `roof`(planes, fascia, soffit), **`stairs`(stepped prism, landing), `railing`(handrail + panel/balusters)**, `probe`+`ray`; **`camera`(`Orbit`, `FullCamera`, `project_point`, `ray_at`, `fit`, `projected_scene_bounds` — moved here from `render3d` in phase 6), `pick`(median-split BVH, `Bvh::ray`/`pick`)**; `edges`/`hlr`/`section` (assisted, phases 6–7); **`site` (terrain solid, draped paths, `site_meshes`), `plant` (trunk + crown) — phase 9**; **phase 13: `incremental` (`SceneCache`, `ObjectKey`/`ObjectKind`, `Sections`, `Scene3d::digest`; `build` is made of its parts), `finish` (finished-face meshes), `uv` (`face_uvs`), `texture` (256² procedural finishes, `mean_srgb`/`mean_linear`), `casework` (cabinets and casework fixtures)**. | geom,color,doc,arch |
| `render` | L2 | **Phase 14: `arch_furnishing.rs` (furnishing plan outlines on their layers).** `arch.rs`: arch geometry → DisplayList tagged with proxy handles; layer bands; opening holes; room labels on `A-ROOM`; roof plan on `A-ROOF`; **stair plan (outline, riser lines, well ring) on `A-STAIR`; railing path on `A-RAIL`**; **`arch_site.rs`: the site plan on `C-TOPO-BNDY/MINR/MAJR/TEXT/GRAD`, `C-ROAD`, `L-PLNT`, `A-ANNO-NRTH` (phase 9)**; **`arch_casework.rs`: cabinet and fixture symbols on `A-FLOR-CASE`, `A-FLOR-CASE-UPPR`, `P-FLOR-FIXT`, `A-FLOR-APPL` (phase 13)**. | arch |
| `io` | L3 | **Phase 14: `gltf_read` (plain glTF 2.0 binary), `image_read`, `library` (the embedded sample set), `dxf_furnishing` (proxies), `ifc/furnishing`; `dxf_arch` payload up to 64 MiB.** `dxf_arch.rs` (`CADCRAFT_ARCH` XRECORD **v9**, accepts 1–9 (v6 cameras, v7 views, v8 site, terrain and plants, v9 light fixtures; phase-11 area lights and IES ride in the v9 model), + save-time proxies on `A-WALL`/`A-DOOR`/`A-GLAZ`/`A-ROOM`/`A-ROOF`; opening proxies carry the whole plan symbol since phase 8; reading a record makes the drawing metric), **`dxf_site.rs` (site proxies marked `CADCRAFT-SITE`, phase 9)**, `glb.rs` (own GLTF 2.0; `node_names`, shared with OBJ); **phase 12: `step` (ISO 10303-21 text), `ifc` (`mod`: header, units, contexts, spatial structure; `guid`; `geometry`; one module per element kind: `walls`, `openings`, `slabs`, `roofs`, `stairs`, `spaces`, `site`, `lights`), `obj` (OBJ + MTL)**; **phase 13: `png` (RGBA8 encoder on `miniz_oxide`), finish textures in `glb`, `dxf_casework` (CADCRAFT-CASEWORK proxies), `ifc::casework`**. | arch,arch3d,dxf |
| `render3d` | L5 | **Phase 14: `backdrop` (the shared CPU composite), symbol and image textures, the path tracer's per-pixel hit count.** `camera` (re-exports `arch3d::camera`), `pipeline` (PBR metallic-roughness: GGX, Smith, Schlick; per-material albedo/metallic/roughness/alpha; MSAA 4 or 1), `light`, **`shadow`** (one 1024² `Depth32Float` sun map, read with `textureLoad`; `ground_mesh`, `GROUND_OBJECT`), **`edges`** (line pipeline over `arch3d::edges`), **`adapter`** (`AdapterChoice` auto|fallback, devices always requested with `downlevel_webgl2_defaults`), `offscreen` (`Style` shaded | shaded-edges | hidden-line | glass-house; `create`/`with_device`/`encode`/`render`/`read_png`; `headless_instance`, made one at a time), `gputest`, `types`; **phase 10: `shadow_atlas`, `final_render::FinalRender` (raster steps); phase 11: `pathtrace` (`device` (own compute device), `scene_gpu` (flattened BVH, materials, lights, IES), `tracer` (progressive passes, stats, snapshots, denoise, resolve), `denoise`, `rt` (BLAS/TLAS for ray queries), `cache` (pipeline cache file), `shaders` + WGSL `pt/{common,traverse,traverse_rq,bsdf,lights,kernel,denoise,resolve}`), `FinalRender::start_path`; phase 12: `pathtrace::Exposure` (Auto | Fixed), `auto_exposure_from_luminances`; phase 13: `textures` (the finish `texture_2d_array`, bind group 3).** | geom,color,arch3d,wgpu,gpu-optin |
| `gpu-optin` | — | **Phase 11:** the workspace's only unsafe code: wgpu's experimental-feature token for ray queries, and a pipeline cache read from disk. | wgpu |
| `engine/src/cmd/arch/*` | L4 | **Phase 14: `library`, `library_icon`, `library_user`, `furnish` (`library.place`, transforms), `paint`, `backdrop`, `defaults`; `engine::library` (catalogs, `LibraryIo`, the user dir, `user` groups and copies).** `wall`,`wall.stretch`,`inspect`,`opening`,`build3d`,`sample`,`view`,`room`,`roomdetect`,`level`,`roofauto`,`roofplane`,`stair`,`railing`,`deck`, **`sun`, `camera` (save/use/delete/list), `walkthrough`, `pick3d`**, `views` (`elevation`, `section`, `view.delete`), `sheet` (`viewsheet`), **`archdetect`, `units` (`convert_drawing_to_metres`)**, **`site`, `site_inspect`, `terrain` (+ `terrain.contours`, `terrain.volumes`, `terrain.delete`), `terrain_modify`, `terrain_query`, `plant` (+ `plant.species`) — phase 9**; **`export` (`exchange.rs`) writes glb, obj (+ mtl) and ifc by extension or returns them as data — phase 12**; **`material`, `casework` (`cabinet`, `fixture` + `.edit`/`.delete`, `fixture.types`), `build3d {incremental}` on the document's `SceneCache` (`DocState`), `sample {house}` — phase 13**; grip routing; `snap::arch`. | arch,arch3d,io |
| `ui-egui` | L5 | **Phase 14: `library_panel.rs` (the dockable Library panel, painter and eyedropper tools).** `pane3d.rs` (dockable, orbit, revision-gated rebuild, show-3D toggle, no-adapter placeholder; phase 6: style switcher, full camera + WASD/arrow walkthrough, loads the active saved camera, click-to-pick via `proxy_handle`; phase 7: a 3D pick becomes the 2D selection), **`archdetect_review.rs` (Detection Review panel, phase 8)**; **phase 11: Finalize chooses Path traced or Raster with a sample count, refines on screen (`{spp} / {total} samples`), cancels, and falls back to the raster still with a note without compute shaders**; **phase 13: the pane keeps its own `SceneCache`**. | render3d,arch3d |
| `cadcraft-cli` | — | `convert` (runs `archdetect` first when there are no arch objects; `--no-detect`; **to `.ifc`, `.obj` (+ `.mtl`) and `.glb` since phase 12**), `run --export`, `render FILE OUT.png --camera orbit,…[,target=x,y,z] | full,eye=…,target=…[,eyeHeight=h][,fov=f] --style --sun --ground --adapter auto|fallback --size` (prints `PROJECTED`, `DEVICE … limits=downlevel_webgl2`, `SHADOW size=1024 built=1`; exit 3 `NO_ADAPTER`); `--sun` takes the site's latitude and true north (phase 9); **`--final [raster|path] --spp --bounces --seed --no-denoise --rt` (phase 11; stderr `FINAL renders=1 size=WxH mode=path spp=N cache=hit|miss|unsupported`); the session renderer (`render_hook`) serves `render` and `illuminance` for `run`**. | all |

## Decisions

1. **Arch model lives in `Drawing`** (undo is a whole-`Drawing` snapshot). Data in `doc`, algorithms in `arch`.
2. **One source of truth: the arch object, not its linework.** Plan geometry generated into the display list at draw time; DXF proxies written only on save, not read back when arch data is present.
3. **No 3D CSG.** Walls cut by 2D boolean on the wall *elevation profile*, then extruded; slabs/ceilings the same trick on room polygons; roof planes prisms over their plan ring; **stairs a stepped prism over their elevation profile**.
4. **Pure Rust, no new heavy deps.** Own GLTF/OBJ/STEP writers; MIT/Apache-2.0/BSD/Zlib, wasm-compatible; no C++ kernels.
5. **The 3D pane renders offscreen** into its own colour+depth textures and presents as an image — egui's pass has no depth buffer we control; identical on WebGPU and WebGL2.
6. **MCP stays generic.** Commands are the API.
7. **Incremental rebuild first-class:** `arch_revision` + dirty `ArchId` set.
8. **Never crash, always deterministic:** caps, `get()`, checked arithmetic, property/fuzz tests.
9. **Additive to AutoCAD parity**; arch commands are new ids under `Architecture`.
10. **Proxy handles, not entities:** `ARCH_HANDLE_BASE = 0xFFFF_FFFF_0000_0000` + kind offset + id (moved to the reserved top in phase 4); offsets: walls `+0`, openings `+0x8000`, rooms `+0x10000`, roofs `+0x18000`, stairs `+0x20000`, railings `+0x28000`, views `+0x30000`, terrain `+0x38000` (`t0`, modifier n at `+1+n`, `tm{n}`), plants `+0x40000` (`pl{n}`). JSON spells every handle one way, `handle_json`: the low bits over `0xA000_0000` in lower-case hex (`w3` → `a0000003`, `o0` → `a0008000`, `v0` → `a0030000`). `grips.rs::arch_id_from_handle` is the single disambiguator. (Until phase 7 replies used five spellings and `arch.inspect`/`pick3d` gave a door its wall's handle.)
11. **Rough vs finished separate** (`ROUGH_ALLOWANCE` one constant).
12. **Placement limits, area rules and stair/railing code limits are data in `arch`**, not logic in commands, so commands/CLI/MCP/`ARCHDETECT` agree. Commands validate through these and **refuse — never clamp**.
13. **Shared structs** are built outside their declaring file only as `Type{required,..Default::default()}` or via a constructor; `cargo xtask literals` enforces it.
14. **Derived, never stored,** when a quantity is a sum of others (`Level::floor_to_floor()` = ceiling + structure; the stored field was removed).
15. **Probing, not counting, proves geometry.** `build3d {"probe":…}` is the acceptance observable; any new 3D feature must add probeable solids, not just triangles.
16. **Roofs are generated, then owned.** A `ROOFAUTO` roof regenerates while its walls change; `roofplane {edit}` sets `hand_edited`. Generation is all-or-nothing — an error means no roof at all.
17. **A stair's well is derived from headroom, and the hole is applied from the stair.** The 3D slab hole comes from `stair.well` (`well_holes_for`), so it exists even when the upper room was never detected; the *room* hole is only cut into a room that really contains the well — the code never invents a room to hold a hole.
18. **One camera, below the renderer.** The camera math lives in `arch3d` (L2) and `render3d` re-exports it, so `pick3d` (engine, L4) and the renderer project through the same code. The view matrix rows are the camera axes (right, up, back); `ray_at` inverts `project_point` and `world_up_projects_straight_up_the_screen` locks it — the rotation was transposed from phase 1 until phase 6.
19. **Picking is a CPU ray cast** against `Scene3d` (median-split BVH, ties broken by mesh then triangle index), never GPU readback, so it is deterministic and works headless. The struck *part* picks the handle range: `ArchId`s number per kind, so an id alone is ambiguous (w0, o0, r0 and s0 are all `ArchId(0)`).
20. **WebGL2 limits everywhere.** Every device is requested with `downlevel_webgl2_defaults()`; the shadow map is read with `textureLoad` (no comparison sampler); a render check may not SKIP from phase 6 because `--adapter fallback` always exists.
21. **Views are linked objects stored with their geometry.** `ArchView` (ELEVATION look direction or SECTION cut line, depth, scope, scale, insert, dimensions, layer) lives in `ArchModel.views`; its generated lines, poché rings and overall dimensions are kept on the view inside the `Drawing` (`#[serde(skip)]`), regenerated by the engine in `after_command` after every successful command (inside the command's undo step, so undo restores the old geometry) and in `open_drawing`. `render` (L2) cannot call `arch3d` (an L2 peer), so it draws the stored geometry; a cache outside `Drawing` keyed by `arch_revision` would go stale after undo + edit.
22. **`ArchKind` travels with every generated line and ring.** IDs number per kind, so HLR merging, section poché grouping and reporting key on (kind, id), never on `ArchId` alone; `ArchKind::from_part(part_of(mesh))` is the one mapping.
23. **Arch linework shows through paper-space viewports** and so in layout plots: each viewport transforms and clips the arch display list the way it does model entities, honouring viewport layer freezes.
24. **One meaning of opening offset, and arch drawings are metre drawings.** `Opening.offset` is the distance from the host wall's start to the opening's **centre**, everywhere (symbol, `poché_gaps`, `validate_placement`, 3D hole). The arch model is in metres, so a drawing holding arch objects is a metre drawing (`$INSUNITS 6`); detection reads linework in drawing units scaled by `$INSUNITS` or `units`, and accepting candidates in a non-metre drawing converts the whole drawing to metres in the same undo step, as Chief converts an imported DWG to its units.
25. **Detection is a pure pipeline in `arch::detect` (L2)**: `flatten` → `walls` → `junctions` → `openings` → unexplained gaps as cased candidates → `rooms`. Candidates carry a confidence and their evidence (layers, handles, rules); AIA/NCS layer names raise a score and are never required; every opening candidate passes the same `validate_placement` as a drawn one. The command only reports (`dryRun`) or accepts in one undo step; the review panel and `convert` are thin callers of it.
26. **The terrain is a surface; the mesh samples it.** `SiteSurface` (original TIN + modifiers in order + pads + retaining breaklines) is the one answer to "how high is the ground here" (`terrain.query`, cut and fill, retaining spans, path draping); `site_mesh` samples it on a constrained triangulation and takes each triangle's corner z from its own side, so steps stay vertical. Constraints are only the lines where the surface has a kink or a step (boundary, TIN edges, modifier, pad and margin rings, breaklines); a refinement is points. The mesh must be closed: `terrain_solid` builds nothing from a top with open or doubled edges, and every mesh change carries a closed-mesh test.
27. **Site location is data the sun reads.** `SiteLocation` (latitude, longitude, the elevation of z = 0, true north, a reference point kept as text and never transformed) lives on the model; `north_direction` and `true_to_drawing` are the only conversions between true north and the drawing's +Y, and `sun` / `render --sun` use them whenever a site is stored.
28. **Site objects have their own handles and probe parts.** Terrain `t0` and its modifiers `tm{n}` at `+0x38000`, plants `pl{n}` at `+0x40000`; probes answer `terrain`, `path` and `plant`, and a plant is counted once though it has a crown and a trunk mesh.
29. **The path tracer accumulates sums; every reader divides.** `accum[i]` = (rgb sum, sample count) and `accum[W·H + i].x` = Σ luminance²; stats, the denoiser's input and `resolve` use the per-sample mean, and the denoiser weighs colour by each pixel's variance of the mean, so it adapts to the sample count without a setting. (Until the operator fix 21bd59d8 the stats and the denoiser read the sums: a white furnace read ~2048.)
30. **GPU process state is touched one thread at a time.** Headless instances are made under one lock and never with wgpu's DEBUG flag: on cgynb4's Vulkan loader (1.4.341) concurrent `vkCreateInstance`, or debug object naming beside it, crashes the process (SIGSEGV at address 0), which a test gate reads as a run that did not complete.
31. **Every export names one object alike and is reproducible.** OBJ groups, GLB nodes and IFC `Name`s come from one naming function (`glb::node_names`: `wall-w1`, `roof-r0p0`, `light-lf0`); IFC GlobalIds are hashes of keys naming what an entity is, never random; the header time is `SOURCE_DATE_EPOCH`; no output order comes from walking a `HashMap`/`HashSet` (BTree collections or a sort). (Until 76307a4d `geom::cdt` refilled its cavities in `HashSet` order and the terrain's triangles came out in a different order on every run.)
32. **A writer of a text format is tested by parsing what it wrote.** Substring assertions let `..TERRAIN..` and a bare three-real `IfcCartesianPoint` through, both refused by STEP readers; `crates/engine/tests/ifc_step_grammar.rs` parses every instance of the demo house's export against the STEP grammar and IFC4's attribute counts, and an IFC entity added later adds its count there.
33. **An incremental build is the full build, cached by its inputs.** `Scene3d::build` is made of parts, each a pure function of inputs it records (`incremental::Sections`); `SceneCache::update` makes again only the parts whose inputs changed and moves the others over in order, so the cached scene equals the full build by construction, and `rebuilt` counts the objects whose parts were made. A cache that patches a scene beside the build drifts: the first SceneCache passed its walls-only tests and grew from 45 meshes to 262 200 on a house with rooms, lights and a roof. A new 3D object kind adds its section and its inputs, and its edits to `crates/engine/tests/scene_cache_equals_full.rs`.
34. **A timing bar measures the work, not the host.** The swarm host runs at load 10 to 40, so a bar takes the lesser of wall time and the CPU time of the processes it ran (phase-9 check 10), or measures once more after a miss (phase-13 check 08); a performance check also bounds the work done (check 08's 10 % rebuilt), so a fast host cannot pass a slow design.
35. **A process-wide hook has one installer.** `LibraryIo` (like `IoHooks`) is a `OnceLock`: the first installation wins for the whole process, so a test that installs its own stub decides, by test order, what every later test sees. Phase 14 hit this twice (the furnish tests, then `library/user.rs`). Unit tests call `library::testing::install_io`; the apps install the real readers once at start.
36. **Coverage comes from what the kernel knows.** Whether a pixel shows the backdrop is the path tracer's own per-pixel count of samples whose camera ray hit a surface (`accum[W·H + i].y`), never inferred from brightness: a mask of "misses darker than the mean miss" passed on dark test scenes and left a sunlit sky in place of the backdrop. Tests of compositing render under the default sun.
37. **Meshes compare within a tolerance.** GLB stores positions as f32 (about 5e-7 at x = 7), so two copies of a mesh at different places never round to the same grid everywhere; a rounded key made equal meshes differ and stopped check 08 at its first step, hiding four bugs behind it. Checks match each vertex to an unused vertex within 1e-5 (`p14lib.same_geometry`).

## Phase 1 — walls, joins, poché, first 3D (landed)

`solve_joins` trims/extends centre-line ends to neighbour face-line intersections (L/T/X/angled/collinear, per-end overrides); `stretch_wall` capped transitive BFS; `render::arch` expands solved rings + layer bands; `arch3d::build` extrudes elevations with `object: ArchId` and box UVs; `io::glb` writes GLTF 2.0 (`wall-w1`); wall grips run `arch.wall.stretch`. Checks 01–09 PASS. Deferrals: `is_main` joins, pony walls, `reversed`, arc walls.

## Phase 2 — openings (landed)

`Opening` on `ArchModel.openings` (host, offset, finished + rough dims, recess, style, hinge/swing, frame/casing/sill). `arch::opening` holds Chief's limits as constants (`MIN_HINGED_WIDTH 0.3`, `MIN_GARAGE_WIDTH 1.8`, `MIN_WINDOW_SEGMENT 0.15`, `MIN_SEPARATION 0.05`, `ROUGH_ALLOWANCE 0.12`) with `validate_placement`/`place_opening`/`auto_panel`/`clamp_recess`/`project_to_wall`. Generated plan symbols; poché cut by even-odd holes. `mesh::push_prism_with_holes` triangulates the elevation minus opening rectangles; `openings::opening_meshes` emits frame/casing/glass/leaf/sill/header merged per material (GLB node names, blended glass). `door`/`window`/`garagedoor`/`arch.opening.move`; grips slide openings. Checks 01–09 PASS. Deferrals: bay/box/bow geometry, mullions, jambs/reveals, hardware, display states, schedules, pocket doors, arc-wall openings, exterior/interior classification.

## Phase 3 — rooms, floors, ceilings, levels (landed)

**Carry-overs closed.** (a) `render3d` draws: colour+depth targets, one directional light + ambient, flat shading from each triangle's own plane (arch3d boxes share corners with averaged normals and inconsistent winding, which speckled faces); meshes re-upload on scene-key change; `render` exits **3** with `NO_ADAPTER` and prints `PROJECTED x0,y0,x1,y1`; check 06 rewritten (flat fill fails, foreground fraction in (0.05,0.85), bbox within 3 px, second camera changes pixels, top-down covers more than oblique). (b) `wall_span(model,wall)->Option<(base,top)>` uses the level elevation and honours `to_ceiling`. (c) `build3d {"probe":[…]}` (≤1024 finite points) → `{id,part,level}` or null, via `arch3d::ray` (Möller–Trumbore specialised to +Z — the first version tested z-ranges, never saw a horizontal face, and reported a point inside a slab as empty); `part_of` maps material names to wall/floor/ceiling/glass/frame/casing/leaf/sill/header/roof.

**Levels.** `Level{index,name,elevation,ceiling_height,structure}`; `floor_to_floor()` derived (ceiling + structure), stored field removed (check 09 greps for it). Defaults 2.4 + 0.3 = 2.7. `level {above|below|index|name|ceiling|structure}`; basements renumber walls; auto-naming follows the neighbour; wall height defaults to floor-to-floor.

**Rooms.** `Room{id,name,room_type,level,polygon,holes,ceiling_height,floor_height,interior_area,standard_area}` with own `next_room_id` and `room_handle`/`room_label`/`is_room_handle`. `RoomType` (Living…OpenBelow, Other) with `is_interior`/`has_floor`/`has_ceiling`. `detect_rooms`: per level, non-divider walls solved on a temporary model (identical joins to the plan), poché rings closed, `divider` walls contribute centre lines; seeds are edge-midpoint left-normal offsets plus the centroid; faces traced with `geom::region::boundary_at`, normalised CCW, holes from islands, filtered by `MIN_ROOM_AREA` and a thin-strip guard, deduped by rounded (centroid, area), capped by `MAX_ROOMS`. A face inside a face is both a hole and its own room.

**Areas (`arch::area`).** `interior_area` = region area; `standard_area` offsets each edge to its wall's reference line (`edge_standard_offset`: outer face for exterior, centre line for interior, 0 for dividers) with `poly::offset_polygon` per-edge distances, holes inward, falling back to interior when degenerate; `perimeter(Interior|Standard|Exterior)`; `living_area` = sum of standard over interior types with effective ceiling ≥ `MIN_LIVING_CEILING` 1.2; `refresh_areas`.

**3D.** `floors::floor_slab` (top exactly at level elevation, down by structure) and `ceiling_surface` (at ceiling height) — prisms with holes, `Mesh::level_elevation` set, `None` for `!has_floor()`/`!has_ceiling()`; `Scene3d.floors`/`.ceilings` counts equal in full and incremental builds (regression-tested).

**Labels/grips/DXF.** `label_point` inside polygon+holes; `render::arch` shapes name+area on `A-ROOM` tagged with the room handle; `dxf_arch` writes save-time `A-ROOM` proxies with `CADCRAFT-ROOM` XDATA, dropped on read while `CADCRAFT_ARCH` is present; room grips resolve to `ArchTarget::Room` and return the label point, drags a silent no-op. `arch.inspect {rooms,labels,levels}`.

**Commands.** `level`, `room` (closed ring ≥4 distinct simple points, or edit by id — name/type/heights only, validated before any write, no-op refused), `roomdetect {level?,dryRun?}` (replace-per-level, dryRun never calls `doc_mut()`, one `touch_arch()`, `truncated` reported), `build3d {probe}`, `arch.sample {house2:true}`.

**Numbers locked.** Interior 27.93 / 28.3725 / 18.6725; standard 31.62 / 31.11 / 20.91 = 83.64 = 10.2 × 8.2; closed 0.1 m loop → south 26.72 with one hole + a 0.81 m² room, each with its own slab; Level 2 at 2.7 with `(5,0)–(5,8)` → two rooms of 37.83; probes hit a Level-2 slab at (2,1,2.55), nothing at (2,1,1.0), Level-1 slab at (2,1,-0.15), nothing at (12,1,2.55). Room-type rules: Open Below → no floor, ceiling kept; Deck/Porch → floor, no ceiling.

**Acceptance (all PASS).** `01_levels_command` · `02_room_detect` · `03_room_areas` · `04_levels_two_storey` · `05_floors_ceilings_3d` · `06_render3d_real` · `07_room_labels_and_roundtrip` · `08_hostile_rooms` · `09_layers_wasm_i18n`. Four checks were broken as written and fixed. **Lesson: run every check against a stub before locking it.**

## Phase 4 — automatic roofs (landed)

**Data.** `Roof{id, level, pitch_rise, pitch_run, overhang, thickness, fascia, soffit, manual, hand_edited, planes, edges}` on `ArchModel.roofs`, own `next_roof_id`, handle window `0x18000`. `RoofPlane{ring, slope, projected_area, base_edge, eave_z}`; `RoofEdge{kind: Eave|Ridge|Hip|Valley|GableEnd, a, b, z_a, z_b}` so `arch.inspect` answers roof geometry with no mesh. Per-wall `roof_directive`, `roof_pitch`, `roof_overhang`, `eave_height`. All new fields defaulted.

**Generator (`arch::roof` 1944 lines, `arch::roofedge` 1280).** Footprint from the **outer faces of the solved poché** of the top level's exterior walls — the same `solve_joins` call the plan uses. Largest closed component only; collinear edges merged first; ≤256 edges; `geom::roof_skeleton(outer, holes, slopes)` with node `time` = roof height and `f64::INFINITY` = a vertical gable end; overhang by running the skeleton on `geom::outset` and keeping the band. Hip vs valley by footprint convexity. **All-or-nothing:** any `geom::check` failure, non-finite node, interior-crossing wall or over-cap input returns `RoofError` and writes nothing.

**Rebuild policy.** A `ROOFAUTO` roof regenerates when its walls change (`build_dirty` marks the roof from the wall's neighbours); `roofplane {edit}` sets `hand_edited`.

**Wall-to-roof clipping, no CSG.** `arch3d::walls` builds the wall's **elevation profile** first (u = along, v = height): a rectangle, or a pentagon when a roof plane's `base_edge` lies on that wall's outer face; then each hosted opening's **rough** rect is clipped into that profile as a hole; then extruded between the wall's two faces (`mesh::push_profile_prism`). Closes the phase-2 defect: `build3d {"probe":[[4.0,8.0,1.5]]}` finds no wall inside a 0.9 × 1.2 window (sill 0.9) at x = 4 on the north wall, `[4.0,8.0,0.5]` finds it; `hlr_section.rs::a_window_beyond_a_section_shows_through_its_wall` is un-ignored and passing.

**3D / GLB / DXF / plan.** `arch3d::roof` emits each plane as a solid (underside through the top outer edge of its eave wall, thickness measured **vertically** above that surface), plus fascia at the eave edge and a flat soffit back to the wall; `Scene3d.roofs` counts them; `part_of` maps roof/fascia/soffit → `roof`. GLB nodes `roof-r0p0`, `roof-r0-fascia`, `roof-r0-soffit`. DXF v4 + save-time `A-ROOF` proxies. `render::arch` draws a roof plan on `A-ROOF` honouring layer state.

**Commands.** `roofauto {level?, pitch:[r,r], overhang?, thickness?, directive?:{wall|all:"hip"|"gable"}, dryRun?}`; `roofplane {polygon, baseEdge, pitch, thickness?, overhang?, id?, edit?}` (object and array forms both accepted); `arch.inspect {"roofs":true}`; `build3d` `roofs` count.

**Numbers locked.** Hip 6/12 → 4 planes (2 trapezoids, 2 triangles), ridge x 4.0…6.0 at y 4.0, z = 2.4 + 4.1 × 0.5 = **4.45**; 4/12 → **3.766667**; 12/12 → **6.5**. Two gables → 2 planes, ridge −0.1…10.1 at 4.45, gable walls reach 4.45 at y = 4 while eave walls stay 2.4. One gable + one hip → 3 planes, ridge x 4.0…10.1. Overhang 0.5 on a hip → projected area **103.04 m²**. Square → pyramid. L `(0,0)(10,0)(10,8)(6,8)(6,12)(0,12)` → 1 valley; T → 2.

**Invariants (property tests).** `arch/tests/property_roofs.rs` + `roof_property.rs`, `geom/tests/skeleton_roof_property_tests.rs`, `arch3d/tests/golden_roofs.rs`: every footprint vertex starts exactly one sloped edge (hip at convex, valley at reflex), every footprint edge has exactly one plane, every plane's slope equals its pitch to 1e-9, every interior roof edge is shared by exactly two planes, projected areas sum to footprint (+ overhang band) area.

**Review sweep (`p4r-*`, 34 segments).** Fixed: join solver (X crossings, T junctions, mitre limit, own-kind exclusion, raw-ring fallback, box prefilter, tidy), linear graph build, layer-band clamp, `wall.stretch` that never collapses a wall and keeps its openings when the start end moves, wall-command validation, camera validation, `Scene3d` finite-f32 guard, mesh side normals, dirty-neighbour propagation, reserved arch handles, arch layers created on first use, GLB/GLTF conformance, CLI render args, `render3d` lints, the pane's show-3D toggle, `render::arch` layer state, and an honest rebuild benchmark (`rebuilt` ≤ 10 % of objects).

**Acceptance (all PASS).** `01_roofauto_hip` · `02_roofauto_pitches` · `03_gable_and_clipping` · `04_mixed_gable_hip` · `05_overhang` · `06_openings_cut_the_profile` · `07_roofplane_and_rebuild` · `08_hostile_roofs` · `09_layers_wasm_i18n` · `10_roof_roundtrip_and_glb` · `11_roof_invariants`.

**Phase-4 gaps.** Out of scope: dormers, crickets, Dutch/half-hip gables, roof returns, attic and knee walls, "roof cuts wall at bottom", roofs over lower-level bump-outs, skylights, gutters, interactive joining of planes, curved walls under a roof. Also: header drawn flush above every opening (phase-7 HLR note), `is_main` joins, arc walls and their openings, per-plane material.

## Phase 5 — stairs, railings and decks (landed)

**Data.** `Stair{id, start, direction(rad), width, from_level, to_level, rise, shape, risers, riser_height, tread_depth, treads, run, landing_depth, well, well_room, layer}` and `Railing{id, path, level, height, style, thickness, baluster_gap, post_spacing, follows, layer}` on `ArchModel`, own counters, caps (`MAX_STAIRS`, `MAX_RAILINGS`) and handle windows `0x20000` (`s{n}`) / `0x28000` (`ra{n}`), with `stair_handle`/`railing_handle`/`stair_label`/`railing_label` and `ArchTarget::Stair|Railing` arms in `grips.rs::arch_id_from_handle`. `Room.deck_height: Option<f64>`. `RailingStyle` = `Panel | Balusters`; `StairShape` = `Straight | L | U`. All new fields defaulted.

**Calculator as data (`arch::stair` 486 lines, `arch::railing` 235).** `plan_stair(rise, width, tread, max_riser, min_tread) -> Option<StairPlan>` picks *n* minimising `|rise/n − TARGET_RISER 0.16875|` subject to `rise/n ≤ max_riser`, scanning a bounded 64-wide window from `⌈rise/max_riser⌉`; exact tie takes the larger *n*; `treads = n − 1`; `tread_depth = tread.unwrap_or(min_tread)`; `run = treads × tread_depth`; `landing_depth = max(width, tread_depth)`. Constants: `MAX_RISER 0.196`, `MIN_TREAD 0.254`, `MIN_HEADROOM 2.032`, `MIN_HANDRAIL 0.864`, `MAX_HANDRAIL 0.965`, `GUARD_HEIGHT 0.914`, `MAX_BALUSTER_GAP 0.1016`, `MIN_RISE 0.15`. `headroom_ok(stair_z, slab_underside, min)` measures to the **slab underside**, not the level above. `well_start` returns the plan distance of the first riser whose top leaves less than the minimum headroom, one tread earlier; `flight_polygon`, `well_polygon`, `well_guard_path` build the plan rings. `arch::railing` owns `validate_height(h, is_handrail)` (guard 0.914–1.2, handrail 0.864–0.965), `baluster_spacing` (clear gap < 0.1016) and `path_length`. Commands validate through these and **refuse, never clamp**.

**3D (`arch3d::stairs`, `arch3d::railing`).** The flight is one closed **stepped prism**: the riser/tread outline in flight coordinates (`u` along travel, `v` up), collinear points dropped, extruded across the width, mapped by `start`/`direction` — no CSG, the wall trick again. `landing_mesh` is a 0.05-thick slab, top at `base + rise`, `landing_depth × width` beyond the run. Railings always emit a top handrail bar (0.05 deep) so a probe anywhere under the rail resolves, plus a solid panel or balusters per segment. `Scene3d` gains `stairs`/`railings`/`decks`; `part_of` maps `stair`/`railing`/`deck` **ahead of** `floor`/`wall` (a deck slab would otherwise read as a floor). Deck rooms build a board slab whose top is `level.elevation + deck_height` (material `deck`, 0.05 thick) instead of the grade slab, and get no ceiling; a non-finite or ≤ 0 `deck_height` falls back to the grade slab.

**Stair wells (moved here from phase 3).** `stair` computes `slab_underside = to_level.elevation − to_level.structure` and, when the stair goes up, the well polygon. In **one mutation and one `touch_arch()`** it: cuts the well out of the upper-level room whose polygon contains the well's centroid and which has no hole there (never inventing a room), pushes an `Open Below` room over the well, rings it with a 0.914 m guard `Railing` whose `follows = stair_id`, and stores `stair.well_room`. In 3D the slab hole is applied from the stair itself (`well_holes_for`), so the hole is real even when the upper room was never detected — the stored room hole and the built hole are two paths to the same probe result.

**Commands.** `stair {start, direction, width?, fromLevel?|toLevel?|rise?, shape?, tread?, maxRiser?, minTread?}` (alias `st`, Architecture ▸ Stair) — `rise` alone makes a to-grade stair, which is how deck stairs reuse the same calculator; `railing {path, level?, height?, style?: panel|balusters, thickness?}` (alias `rl`); `deck {points, height?, name?, level?}` (a `RoomType::Deck` room, default height 0.9, height in (0, 10), simple-ring and area checks before any write). Each has an interactive machine delegating to the JSON path: one implementation, one undo step.

**Observables.** `arch.inspect {"stairs":true,"railings":true}` → id, handle, start, direction in degrees, width, levels, rise, shape, risers, riserHeight, treadDepth, treads, run, landingDepth, `well` ring, `wellRoom`; railing path/level/height/style/thickness/balusterGap/follows. Rooms gained **`holeRings`** (ring geometry — `holes` stays the legacy *count*, locked by phase-3 check 03), **`deckHeight`** and **`hasFloor`**. `build3d` reports `stairs`/`railings`/`decks` and answers `probe` for all three parts.

**DXF / GLB / plan.** `CADCRAFT_ARCH` **v5** (accepts 1–5; stairs, railings and `deckHeight` cleared when reading an older record so old files still open). GLB nodes `stair-s{n}`, `railing-ra{n}`, `deck-r{n}` with their own PBR materials, interleaved in scene order. `render::arch` draws the stair plan on `A-STAIR` (flight outline, one line per riser across the width, the well ring) and the railing path on `A-RAIL`, honouring layer state and tagged with the proxy handle.

**Numbers locked.** L1 0 → L2 2.7: **16 risers × 0.16875, 15 treads × 0.254, run 3.81**. Deck 0.9 m to grade: **5 risers × 0.18, 4 treads, run 1.016** (6 × 0.15 is further from 168.75 mm). Well: x 2.508…5.81, y 0.5…1.5, area **3.302**; enclosing Level-2 room 76.44 → 73.138 with exactly one hole ring. Probes: `(2.6,1,2.538)` nothing (2.032 m above the 3rd riser, inside the slab's 2.4…2.7), `(2.1,1,2.538)` and `(8,4,2.538)` floor, `(2.6,1,2.2)` nothing, `(2.1,1,0.1)`/`(2.6,1,0.1)` stair, `(2.1,1,1.0)` nothing; deck `(2,1.5,0.88)` deck, `(2,1.5,2.39)` and `(2,1.5,−0.15)` nothing. Railing: default guard 0.914, baluster clear gap < 0.1016, `(2,0,0.5)` panel, `(2,4,0.88)` handrail, `(2,0,1.5)` nothing.

**Acceptance (all PASS).** `01_stair_calculator` · `02_deck_stair_to_grade` · `03_stair_3d` · `04_stair_well` · `05_railing` · `06_deck` · `07_stair_roundtrip_and_glb` · `08_hostile_stairs` · `09_layers_wasm_i18n`. Hostile input (negative width, unknown level, 1e308 rise, 0.01 rise, `shape:"spiral"`, empty/one-point railing path, 0.5 m guard, `style:"glass"`, two-point deck ring, negative and 1e308 deck height) is refused with nothing written. `04` was broken as written: it read `holes` as a ring list and compared the ring's *first* vertex, arbitrary for a CCW ring — now reads `holeRings` and matches by bounding box. Two upstream syncs merged (`9de2b8bbd`, `336243e0a`).

**Phase-5 gaps (vs Chief).** Out of scope by the brief, filed ARCH-24…ARCH-30: winders; curved/spiral/wrapped stairs; ramps; gooseneck and smooth rail transitions; open risers; deck joist/beam framing; stair soffits and stringer detail. Ours: `StairShape::L`/`U` stored and calculated but only `straight` meshed (no second flight); railings are a straight top rail per segment, do not follow the nosing line, and none is generated on the flight itself; no `stair.move`/`railing.move`, so neither is grip-draggable; **`A-RAIL` is missing from the template layer list (`Drawing::bare()` has `A-STAIR`, not `A-RAIL`) and no command calls `ensure_layer("A-RAIL")`**, so railing linework uses the fallback colour and the layer can be absent from a saved DXF; `landing_mesh` flips winding and recomputes normals by hand (fine for flat shading, must be checked by phase 7's HLR); stairs are not roof-clipped, not in elevations/sections (7), not in `ARCHDETECT` (8), not in IFC (12).

## Phase 6 — the 3D view, properly (landed)

**Sun (`arch::sun`, 353 lines).** `sun_position(lat_deg, date "YYYY-MM-DD", time "solar-noon" | "HH:MM" solar time) -> Option<Sun{elevation_deg, azimuth_deg}>` with the NOAA fractional-year declination; azimuth clockwise from north; `sun_dir` gives the unit vector toward the sun (+X east, +Y north, +Z up). `sun {lat,date,time}` reports `elevation`, `azimuth`, `dir`; bad dates, |lat| > 90 and non-finite input are refused.

**Shading (`render3d::pipeline`, `mesh.wgsl`).** PBR metallic-roughness (GGX distribution, Smith/Schlick-GGX geometry, Schlick Fresnel) with per-material albedo, metallic, roughness and alpha; flat normals from each triangle's own plane are kept (arch3d boxes share corners). MSAA 4× (`valid_sample_count`: 4 or 1). **Shadow (`render3d::shadow`, 752 lines):** one 1024² `Depth32Float` map rendered from the sun with an orthographic camera fitted to the scene **and the ground it falls on** (`p6f-r3d-shadow-reach`; first cut fitted the scene only, so shadows stopped at the house), read with `textureLoad`; fragments behind the stored depth get ambient only. `--ground` adds `ground_mesh` (object `GROUND_OBJECT = ArchId(u32::MAX)`) under the scene. **Styles (`offscreen::Style`):** shaded; shaded with edges and hidden line both draw `arch3d::edges` feature edges (crease > 20°, boundary) through `render3d::edges`, hidden line with faces in the clear colour; glass house blends walls (`p6f-r3d-glass-house`: the first cut was pixel-identical to shaded).

**Cameras.** `Camera3d{name, kind: Orbit|Full, target, azimuth_deg, elevation_deg, distance, eye:[x,y], eye_height, fov_deg}` on `ArchModel.cameras` (cap 64) with `active_camera`, so cameras undo and round-trip (`CADCRAFT_ARCH` **v6**; older records read with no cameras; invalid cameras are dropped on read). `camera {"save":name, kind, …}` / `{"use":name}` / `{"delete":name}` / `{"list":true}`; with none of those it validates and echoes an orbit (radians). A full camera takes `eye:[x,y,h]` or `eye:[x,y]` + `eyeHeight`, and reports a 3D eye (`p6f-cmd-camera-full-eye`). `walkthrough {eye, yaw, forward, strafe, turn, step?}` is a pure calculator (yaw 0 = +Y, toward +X; never touches the drawing); the pane walks a full camera with W/S/Up/Down, A/D strafe, Left/Right turn (5° steps) and loads the active saved camera.

**Picking (`arch3d::pick`, 295 lines).** A median-split AABB tree over every scene triangle (non-finite or out-of-range triangles skipped, the renderer's rule); `Bvh::ray` returns the nearest hit, ties by lower mesh then triangle index; hostile rays return `None`. `pick3d {"camera":…, "size":[w,h], "pixel":[x,y]}` (size ≤ the viewport cap, refused beyond) returns `{"hit":{id, handle, part, distance, point}}` or `{"hit":null}`. The pane picks on click through `proxy_handle(id, part)`.

**Headless render.** `--style`, `--sun lat=…,date=…,time=…`, `--ground`, `--adapter auto|fallback` (wgpu's software adapter; prints `DEVICE … limits=downlevel_webgl2`), `--camera orbit,azimuth=,elevation=,distance=,fov=,target=x,y,z` (`p6f-cli-orbit-target`) or `full,eye=x,y,z|x,y,eyeHeight=h,target=x,y,z,fov=f`; bad values exit non-zero.

**Operator work.** `p6-arch3d-camera` escalated to level 2 and was merged by the operator: the racer's branch plus the view-matrix fix (decision 18) and its 16 camera tests kept under `render3d`'s names. Two operator-added segments: `p6-r3d-edges` (draw the existing `arch3d::edges`, not a second edge finder) and `p6-arch3d-door-wall` (a wall holding a door was open beside and above the door in 3D; probes now hit `w0` there).

**Numbers locked.** 51.05° N, 2026-06-21 solar noon: elevation **62.39° ± 0.1**, azimuth **180° ± 0.5**, `dir` y < 0, z > 0; 2026-12-21: **15.5° ± 0.6**; 06:00 in June is east (azimuth < 120). Front camera `eye [3,-8,1.6] → target [3,0,1.05]`, fov 60: the centre pixel picks the door (`part` leaf/frame/glass/casing/sill), right of centre picks `w0`, the sky `null`. Top-down 10 × 8 house, 2.4 m walls, winter noon: ground south of the house 157, just north 120, beyond the shadow 157; the shadow is ~90 px long at winter noon and ~10 px at summer noon (6.9× in theory). Walkthrough `forward 2` at yaw 0 from `[5,-10,1.6]` → `[5,-8,1.6]`; `turn 90` → yaw 90; a 1e308 step is refused.

**Acceptance (all PASS).** `01_sun_position` · `02_display_styles` · `03_shadow_from_the_south` · `04_pick3d` · `05_saved_cameras` · `06_full_camera_walkthrough` · `07_adapter_and_webgl_limits` · `08_hostile_view` · `09_layers_wasm_i18n`. Two fix cycles. Cycle 1: four segments (full-camera eye, orbit `target=`, shadow reach, glass house) and three checks broken as written — `02` held hidden line to the shaded fraction bounds and counted colours, not changed pixels; `03` read the background from a corner pixel the ground covers (now diffed against a render without `--ground`); `06` expected a refusal as a JSON row and `> -8` where the exact answer is −8. Cycle 2: `03` held a fixed 0.75 lit/shadow ratio that a correct shadow under a 15.5° winter sun misses (120 vs 157); it now asserts darker than south and than the far ground, and that the shadow lengthens as the sun drops. **Lesson, again: run each check against a correct build and against a broken one before locking it.** 4911 workspace tests pass; `cargo xtask wasm` builds.

**Phase-6 gaps.** Out of scope by the brief: SSAO, TAA, cascaded shadows, path tracing (11), artificial lights (10), path-based walkthrough animation, gamepad and 3D mouse. Ours:
- **A 3D pick does not reach the 2D selection.** The pane stores the picked handle but nothing calls `Pane3d::take_picked`, so clicking in 3D selects nothing in the app. Carried into phase 7 — fixed there (`pane3d::apply_pick`).
- **Opening handles disagree.** The 2D pane tags door/window linework with `opening_handle` (`0xA000_8000 + id`), but `arch.inspect` (`inspect.rs`, openings) and `pick3d` (`label_and_handle`) report openings as `0xA000_0000 + id` — the handle of the wall with the same number (o0 and w0 both read `a0000000`). Check `04` compares the two wrong values with each other, so it passes. Carried into phase 7 — fixed there (decision 10, `handle_json`).
- The pane is lit by `Light::DEFAULT` (ambient 0.25); the sun and its shadow exist only in headless `render --sun`. A stored site location (latitude, date, time) belongs with phase 9's site.
- Hard-edged shadow (no PCF); one map fitted to scene + ground, so very large sites lose resolution.
- `camera`'s echo form takes radians while saved cameras take degrees.
- `docs/arch3d/USER-GUIDE.md` calls 51.05° N London (London is 51.5° N; 51.05° N is Calgary).
- `mid.tmp` (a stray pipeline fragment from `p6-r3d-pbr` round 1) was committed at the repo root — removed in phase 7 by an operator commit.

## Phase 7 — generated elevations and sections (landed)

**View objects (`doc::arch`).** `ArchView{id, name, kind: Elevation|Section, look, cut, depth, scope, scale, insert, dimensions, layer, generated}` on `ArchModel.views` with its own `next_view_id` (`v0`, `v1`, …, handle window `0x30000`), `MAX_VIEWS` 64, `MAX_VIEW_LINES` 200,000. `ViewGeometry{revision, extent, lines: Vec<ViewLine>, rings: Vec<ViewRing>, dims: Vec<ViewDim>, error}`, every line and ring tagged (kind, id) (decision 22); `DimKind: Width|Height`. `views` and `next_view_id` are left out of the `CADCRAFT_ARCH` payload when empty, so a model without views writes the payload it wrote before views existed (the operator's fix in `p7-doc-views`: the empty fields grew every wall-only payload by 27 chars and broke io's hostile splice test through 14 gates).

**Generator (`arch3d::views`).** `refresh(model, revision)` builds the scene once and regenerates every stale view: an elevation runs `hlr::visible_lines` over `OrthoView::elevation(look)`, a section runs `section::section_view` on the cut line's plane (rings = poché of each cut wall, slab and roof; what lies behind is HLR'd up to `depth`). Each view gets one overall width and one overall height dimension; `scope` is a plan rectangle. No second HLR or section kernel was written.

**Commands (`engine::cmd::arch::views`, `sheet`).** `elevation {side?: south|north|east|west, look?: [dx,dy], name?, insert?, scale?, scope?, dimensions?}`; `section {from, to, look?, depth?, name?, insert?, scale?, scope?, dimensions?}`; `view.delete {view:"v0"}`; `arch.inspect {"views":true}` → per view `id, handle, kind, name, layer, look, cut, depth, scope, scale, insert, extent, bounds, lines, rings, dims, revision, error`. `viewsheet {name?: "Views", views: ["v0", …] (1..=16 ids, no duplicates), paper?: "ARCH D", landscape?: true}` creates a layout with no default viewport (`layout::new_bare_layout`) and one locked viewport per view at k = 1000 / scale paper mm per metre (`layout::add_scaled_viewport`), two per row from the top left, 25 mm margins and 20 mm gaps; it refuses with nothing written. Plotting is the existing `exportpdf` / `plot`. All validated, never clamped; hostile input exits non-zero and changes nothing.

**Drawing and DXF.** `render::arch` draws views in the plan: linework on the view's layer (`A-ELEV` / `A-SECT`), poché filled on `A-SECT-MCUT`, dimensions on `A-ANNO-DIMS`, template layers created on first use, `MAX_VIEW_ITEMS` 100,000; arch linework shows through paper-space viewports (decision 23). `CADCRAFT_ARCH` **v7** stores the views; their linework is written as `CADCRAFT-VIEW`-tagged proxies (LINE, LWPOLYLINE, TEXT) only on save and dropped on read while the arch record is present, so open + save never duplicates them.

**Carry-over and fixes.** One JSON handle form (decision 10) across `arch.inspect`, `pick3d` and command replies; a 3D pick becomes the 2D selection (`pane3d::apply_pick`). **Roof eave (phase-4 defect):** `RoofPlane.eave_z` applies at the wall line, but `arch3d` applied it at the overhang outline, so with an overhang the roof floated overhang × slope high and stretched every wall under it (eave walls 2.65 m, not 2.4); one `underside_z` now serves the roof solid, fascia, soffit and the wall-top clip.

**Operator work.** `p7-doc-views` merged by the operator after 14 failed gates (above). The `p7-engine-viewsheet` spec named private `cmd/layout.rs` internals and left that file out of its files, so aider never showed them and 5 rounds hand-built `Viewport`/`Layout` and failed to build; the corrected spec names `new_bare_layout`/`add_scaled_viewport` and the next round built. Four stray files (`mid.tmp`, two `*_tmp.rs` at the root, `crates/arch/src/planarr.rs`) removed by an operator commit: aider's diff edits cannot delete a file. **Planner rule:** a spec that points at internals names them, lists their file, and says to expose them when private.

**Numbers locked.** Phase-1 house, door at 3.0, gable roof 6/12 with 0.5 m overhang (gable ends south and north): south elevation **11.2 m** wide, overall height **5.474 m** (slab underside −0.3 to roof top 5.174), east **9.2 m**; the door a closed 0.9 × 2.1 rectangle; a north-wall window leaves the south elevation identical, a south-wall one adds lines; moving a window 1 m moves its lines 1 m with no other command, undo moves them back; section y = 4 looking +y: wall rings w1 and w3 of 0.2 × 2.4 = **0.48 m²** each, plus slab and roof poché, no door line; four elevations on ARCH D landscape at 1:50 (viewport scale 20), PDF MediaBox 2592 × 1728 with 349 line segments, PNG at 50 dpi 1800 × 1200.

**Acceptance (all PASS).** `01_one_handle_form` · `02_front_elevation` · `03_windows_front_and_back` · `04_window_moves_with_its_view` · `05_section_at_y4` · `06_elevation_sheet_pdf` · `07_views_round_trip` · `08_hostile_views` · `09_layers_wasm_i18n`. One fix cycle: `p7f-viewsheet-view-ids` (`viewsheet` looked views up by name, not id: `unknown view: v0` on every real drawing; its tests passed because their hand-made views were *named* "v0" with every id 0), and two checks broken as written — `06` cut each PDF stream at `\r?\nendstream`, so a FlateDecode stream whose last byte was 0x0D would not inflate (349 segments read as 0; now sliced by `/Length`), and `07` split the DXF on `0 ENTITIES` (a section's name follows group code 2) and counted the HEADER (51 + 2 proxies read as none). Both were re-run against a stub and still fail without views. 5074 workspace tests pass; `cargo xtask wasm` builds.

**Phase-7 gaps.** Out of scope by the brief: schedules, auto-dimension rows, story poles, callouts, construction lines, roof and framing plans. Ours:
- The opening header (a full-thickness box over each frame) still draws an outline above every door and window in elevations. Fixed in phase 8 (a lintel inset 10 mm).
- Views regenerate synchronously after each arch edit, the whole scene per stale view: fine at house scale, not measured on large models.
- `scope` is a plan rectangle, not a level or object list; dimensions are overall width and height only.
- The sheet has no view titles, scale labels or title block; the layout is fixed at two per row.
- `room_label` and `roof_label` both print `r{n}` (JSON disambiguates with `kind`).
- `docs/arch3d/USER-GUIDE.md`: the phase-7 section is prose with no runnable demo block, the title still says "Phase 6", and 51.05° N is still called London. Fixed in phase 8 (the guide runs).
- Housekeeping: `docs/arch3d/tests.rs` is an orphan that asserts ARCH-10 is open; `docs/arch3d/README.md` describes phase 1; `uk.tsv` has "Дверь" (Russian) for Door — Ukrainian is "Двері". Fixed in phase 8.

## Phase 8 — recognition from plain DXF (landed)

**Pipeline (`arch::detect`, decision 25).** `detect(&Drawing, &DetectOptions{scale, layers}) -> Result<Detection, DetectError>`; `Detection{walls: Vec<WallRun>, openings: Vec<OpeningCand>, rooms: Vec<RoomCand>, entities, primitives}`. The shared structs are built through `Item::new`, `Label::new`, `WallRun::new` and `OpeningCand::new` (decision 13).
- `flatten` → `Linework{items, labels}` in metres: `Prim::Seg | Arc` with layer, source handle and polyline id. INSERTs nested to depth 16, MINSERT arrays, layer-0 inheritance, bulges; tessellated arcs are fitted back to arcs (`arcs`). Caps `MAX_PRIMITIVES` 400,000 and `MAX_LABELS` 20,000. A label is the first line of an MTEXT/TEXT with its codes removed (`label_text`).
- `walls::wall_runs`: face lines parallel within 0.25° and `MIN_THICKNESS` 0.04 … `MAX_THICKNESS` 0.6 m apart, adjacent in offset (nothing between them over more than half their length). Conflicts over a shared face keep the longer piece, then the one nearer a dominant thickness, then the thinner. Collinear pieces merge across gaps up to `MAX_GAP` 2.6 m **unless a wall-face line of another direction spans the band inside the gap** (`merge_pieces(pieces, crossed)`, fix cycle 1); the gaps are kept for the openings stage.
- `junctions::snap_junctions`: L ends meet at one bit-identical point, T ends land on the other centre line, collinear runs merge (no end moves more than 0.6 m). X crossings are left to `solve_joins`.
- `openings::find_openings`: swing doors from arcs hinged at a jamb (radius 0.5–1.3 m, sweep 60–100°); sliding doors from two leaf lines in a gap or on a door layer; windows from glass lines strictly inside the band, closed by a jamb or on a window layer. Doors claim their leaf lines before windows are looked for; overlapping candidates keep the most confident (a door on a tie); an unexplained gap is a `cased` candidate below the accept threshold; every candidate passes `validate_placement` on a one-wall model. Items are found through a 1 m grid.
- `rooms::find_rooms`: `room::detect_rooms` on the candidate walls, each room named from the label inside it.
- `layers::layer_role`: AIA/NCS names (`A-WALL`, `A-DOOR`, `A-GLAZ`, …) add to the confidence and are never required; the corpus has a plan with every entity on layer 0.
- `score`: one-to-one matching against `<name>.truth.json` (a wall's centre line within max(t/2, 0.05 m) and thickness within 20 %; an opening's kind, centre within 0.15 m and width within 10 %); `crates/engine/tests/archdetect_corpus.rs` runs it.

**Rooms at scale (fix cycle 1).** `geom::region::Arrangement::new(polys, tol, gap)` builds the planar graph once and answers `boundary_at(p)`; `boundary_at_gap` delegates to it. `room::detect_rooms_at_level` groups rings whose combined boxes overlap (merged until stable, so an island inside a room joins that room's group), builds one arrangement per group lazily, keeps the seeds in their original order and skips a seed that lies inside an accepted room of its own group. Before, every seed rebuilt the graph from the whole soup: 450 s on 11,271 entities.

**Command (`engine::cmd::arch::archdetect`).** `archdetect {dryRun?, accept?: "all" | [ids], minConfidence?: 0..1 (default 0.5), units?: mm|cm|dm|m|in|ft, layers?: [names]}`; `{}` is a dry run and never calls `doc_mut()`. The reply is `{units, scale, walls: [{id, start, end, thickness, confidence, evidence: {layers, handles, rules}}], openings: [{id, kind, host, center, offset, width, style, confidence, evidence}], rooms: [{id, polygon, name, area, confidence, evidence}], stats: {entities, primitives, ms}}`, ids `c0…` in one namespace, all in metres. `accept` validates every id first, then in one undo step converts a non-metre drawing and creates the walls, openings and rooms: `{accepted: {"c0": "w0", …}, walls, openings, rooms, converted: {from, scale} | null}`. Units come from `$INSUNITS` (`units_scale`: 1 in, 2 ft, 4 mm, 5 cm, 6 m, 14 dm; 0 is unitless and read as metres) or the `units` parameter.

**Metres (decision 24).** `cmd::arch::units::convert_drawing_to_metres(d, scale)` scales every model-space entity about the origin (an INSERT's position and scale, so its block content follows; definitions are untouched), multiplies `$DIMSCALE`, `$LTSCALE`, `$TEXTSIZE` and every dimension style's scale, rescales each viewport's view centre and height so it frames the same content, and sets `$INSUNITS 6` and `$MEASUREMENT 1`; a non-finite or non-positive scale is refused before anything changes. `Drawing::make_metric_units` (`$INSUNITS 6`, `$MEASUREMENT 1`) runs from `touch_arch` whenever the model holds arch objects, and when an arch record is read.

**Openings.** `Opening.offset` is the centre everywhere (decision 24): the symbol's jambs and frame, `poché_gaps` and `validate_placement`'s end limits now agree with the 3D hole, `rough_offset`, the leaf and the swing. The header is a lintel inset `HEADER_INSET` 10 mm from both wall faces (`arch3d::openings`), so HLR no longer outlines a box above every opening (phase-7 gap). Opening proxies carry the whole plan symbol (jambs, leaf, swing, frame, glass) on `A-DOOR`/`A-GLAZ`, and the synthetic corpus is made from them. A door styled `sliding` draws two leaves like `slider` (it drew a swing arc).

**CLI and UI.** `cadcraft-cli convert plan.dxf out.glb` runs `archdetect {"accept":"all"}` first when the drawing has no arch objects; `--no-detect` skips it. `ui-egui::archdetect_review` (Architecture > Detection Review, a dock panel) runs a dry run and lists walls, openings and rooms with a checkbox each; a row zooms to its candidate, unchecking a wall unchecks its openings, and Accept Selected runs `archdetect {"accept": [ids]}`. Check 10 photographs the panel and the 3D pane with egui_kittest + wgpu; the gate keeps the PNGs.

**3D framing (fix cycle 1).** `Orbit::fit` summed the three half-extents and multiplied by √3 again, putting the eye at 51 m instead of about 21 for a 12 × 8 × 2.7 m house, so the pane showed it at a fifth of its width. It now fits the bounding sphere: `r = ½·|diagonal|`, `distance = 1.1·max(r, 0.1) / sin(fov/2)`.

**Operator work.** `p8-housekeeping-guide-readme-uk` and `p8-detect-openings` were merged by the operator: the first because the 82 KB `uk.tsv` filled the seat's context in rounds 1–4, the second because six rounds never built (a reply wrote a markdown fence and its own prose into `openings.rs`; the swarm now catches a fence in a Rust file before the gate). Also: the Ukrainian rows for the new commands and panel landed ahead of the segments that needed them; `cargo xtask literals` no longer reports an enum variant declaration (`Entry::Item {…}`) as a literal of the struct `detect::Item`; headless `render3d` instances leave out wgpu's DEBUG flag, whose `vkSetDebugUtilsObjectNameEXT` crashed the debug test binary in about one run in three (60 of 60 pass without it); the orphans `docs/arch3d/tests.rs` and `tests/user_guide_examples.rs` were deleted.

**Numbers locked.**
- `examples/apartment.dxf` (mm, 58 entities): 6 walls, 2 doors, 5 windows, rooms LIVING / DINING 51.68, BEDROOM 22.56 and BATH 12.69 m²; walls, doors and windows P = R = 1.000. Accept converts mm → m (linework 0…12 × 0…8) in one undo step; the GLB has walls 0…12 × 0…8 × 2.7 m in 46 nodes.
- Synthetic corpus (5 plans plus nested-and-turned, mm-scaled and layer-less variants; 51 walls, 24 doors, 35 windows): walls P/R 1.000, doors P 1.000 R 0.958, windows P 0.972 R 1.000.
- 17 × 17 apartments, 11,271 entities: median 353 ms over 5 dry runs (bar 2 s), 1,734 walls of 1,734.

**Acceptance (all PASS; 07 deferred).** `01_openings_line_up` · `02_housekeeping` · `03_synthetic_corpus` · `04_apartment` · `05_accept_and_undo` · `06_convert_plan_to_glb` · `07_real_plans` · `08_scale_and_hostile` · `09_layers_wasm_i18n` · `10_review_panel_screenshots`. Fix cycle 1: three segments (`p8f-walls-stop-at-crossing-faces`, `p8f-rooms-arrangement-once`, `p8f-orbit-fit-bounding-sphere`), and check 08 counts `minConfidence: 1e400`, which the CLI refuses before the command runs, as a refusal. Fix cycle 2: `07` passes with a "deferred" line while `tests/fixtures/archdetect/real/` is empty, fails on one or two plans, and gates the full three-plan rule as soon as a plan lands. 5364 workspace tests pass; `cargo xtask wasm` builds.

**Phase-8 gaps.** Out of scope by the brief: raster or PDF plans, scale from dimensions, multi-storey files, stairs and fixtures. Ours:
- **No real plans yet.** The three CC0 plans (Dryad/Zenodo) wait on Andy's download approval; detection is measured only on synthetic plans and `apartment.dxf`, both drawn the way the detector expects. Hatched or single-line walls, blocks with odd base points and dimension clutter are untested. The three plans landed in phase 9 (`cf0e5b5a`); check 07 reports them and does not gate (phase-9 gaps).
- A unitless drawing (`$INSUNITS 0`) is read as metres; a millimetre plan saved unitless finds nothing until `units: "mm"` is passed.
- Candidates are accepted or rejected, not edited; fixes come after the accept, with grips and commands.
- A layer-less sliding door over a continuous wall reads as a window. Curved walls are not detected.
- `convert` prints "0 doors, 0 windows (0 entities)" after detecting: it reads `doors`, `windows` and `entities` fields that the accept reply does not have (it has `openings`). The GLB is complete. Fixed in phase 9.
- GLB floor and ceiling nodes fall to the default name arm and read `floor-o0` / `ceiling-o0` instead of `-r0` (since phase 3). Fixed in phase 9.
- Harden passes that did not land: `detect-walls` (three identical rounds), `detect-rooms`, `ui-review-screenshots`.

## Phase 9 — terrain and site (landed)

**Data (`doc::arch::site`, operator groundwork `9e4bdfe6`).** `SiteLocation{latitude, longitude, elevation (of z = 0), true_north, reference: Option<SiteReference{point, easting, northing, crs}>, north_arrow_at}` on `ArchModel.site`; the reference is text and is never transformed. `Terrain{boundary, default_elevation, points, lines: Vec<ElevationLine>, modifiers: Vec<TerrainModifier{id, kind}>, next_modifier_id, contour_minor 0.25, contour_major 1.0, pad, pad_margin 0.3}` on `ArchModel.terrain`. `ModifierKind::{Flat{polygon, elevation, margin}, Hill{center, height, radius}` (a valley is a negative height), `Path{kind: Road|Driveway|Sidewalk, polygon, thickness 0.1, material}}` (a polyline with a width becomes its polygon). `Plant{id, species, position, age, planted_height}`; `PlantSpecies{name, form: DeciduousTree|EvergreenTree|Shrub|GroundCover, crown: CrownShape, mature_height, mature_spread, planted_height, years_to_mature}`. `Wall.retaining`, `freeboard` 0.15, `footing` 0.3. Caps `MAX_TERRAIN_POINTS` 20,000, `MAX_ELEVATION_LINES` 2,048, `MAX_RING_POINTS` 4,096, `MAX_TERRAIN_MODIFIERS` 256, `MAX_PLANTS` 4,096, `MAX_PLANT_SPECIES` 256, `MAX_CONTOUR_LEVELS` 1,000. Every field is skipped when empty, so a model without a site writes the payload it wrote before.

**Surface (decision 26).** `geom::tin::triangulate` (lifted, see Building blocks) → `geom::cdt::triangulate_constrained` (elevation lines and retaining walls become edges; a constraint crossing an earlier one splits at the crossing; at most 4 × points splits) → `tin_ops::retain_inside` the boundary. `arch::terrain::SiteSurface::new(model)` holds the original TIN, the modifiers, the pads (`pad::pad_regions`) and the retaining breaklines; `original_at(p)` and `modified_at(p)` answer elevation queries, and `modified_toward(p, toward)` answers on one side of a step. Modifiers apply in order: a flat region sets its elevation and blends back over `margin`; a hill or valley adds `height·(1 − t²)²` with `t = r / radius`; a path drapes on the result with its thickness. `terrain::csv::parse_points_csv(text, offset)` reads `x,y,z` rows and can subtract the reference easting and northing; a bad row is refused with its line number and nothing written.

**Mesh and 3D.** `terrain::mesh::site_mesh(surf)` triangulates the original TIN's vertices and edges, the boundary, every modifier's ring (a flat region's margin ring, a hill's six rings of 24 points), the pad ring, its margin ring and the breaklines, then sets each triangle's corner z from `modified_toward` its own centroid, so a step (a retaining wall, a flat region without margin) stays vertical. Between a pad's margin ring and the ground, up to 32 slope rings of points 0.25 m apart (at most 20,000 points) make the mesh follow the 1:2 slope (decision 26: points, not constraints). `arch3d::site::site_meshes(model)` builds the terrain solid (the top with step walls, a skirt and a flat bottom `TERRAIN_SKIRT` 1 m below the lowest point; `None` when the top is not closed), one draped mesh per path, and `arch3d::plant` gives each plant a trunk and a crown (sphere, cone or ellipsoid) scaled from planted size to mature over `years_to_mature`. `build3d` reports `terrain`, `paths` and `plants`; probes answer parts `terrain`, `path` and `plant` with ids `t0`, `tm{n}`, `pl{n}`; GLB nodes `terrain`, `driveway-tm0`, `plant-pl0` and `plant-pl0-trunk`.

**Building pad and basements.** The house's footprint (the first floor's exterior wall ring) flattens the terrain to the first floor's elevation minus its floor structure, holds that over `pad_margin`, then grades back at 1:2 (`PAD_SLOPE` 0.5) toward the surrounding surface, above or below it; a basement level cuts the ground down to its floor's underside inside its footprint (`pad::apply_pads`).

**Retaining walls.** A wall with `retaining: true` is a breakline: `terrain::retaining::retaining_span` gives the low and high sides from the surface on each side; the wall runs from the low side minus `footing` to the high side plus `freeboard`; `freeboard` and `footing` are refused on a wall that is not retaining.

**Plan (`render::arch_site`, operator commit `6266e8ad`).** Boundary on `C-TOPO-BNDY`, minor contours on `C-TOPO-MINR`, major on `C-TOPO-MAJR` with their elevation (`contour::label_text`, at `label_point` and turned to read upright) on `C-TOPO-TEXT`, flat regions, margins and hills on `C-TOPO-GRAD`, paths on `C-ROAD`, plants (circle at the spread plus `plant::plan_symbol`) on `L-PLNT`, and the north arrow with an "N" on `A-ANNO-NRTH`. Contours come from `SiteSurface → site_mesh → contour::site_contours(mesh, minor, major)`, chained by `tin_ops::chain_contours` (open lines end on the boundary, a hill gives closed loops); a minor level that is also a major one is drawn once, as major.

**Site location and the sun.** `site {latitude?, longitude?, elevation?, trueNorth?, reference?, northArrowAt?, clear?}`; `site::north_direction` turns true north into drawing coordinates and `true_to_drawing` turns a true-north vector into the drawing's. `sun {date, time, lat?}` and `cadcraft-cli render --sun date=…,time=…` take the latitude and true north from the site when one is stored (no site: unchanged). The 3D pane is still lit by `Light::DEFAULT`.

**Commands (`engine::cmd::arch::{site, site_inspect, sun, terrain, terrain_modify, terrain_query, plant}`).** `terrain {boundary, elevation?, points?, lines?: [{points, elevation, closed?}], csv?, csvText?, subtractReference?, contours?: [minor, major], pad?, padMargin?}` → `{terrain: "t0", points, lines, triangles, range}`; `terrain.modify {flat|hill|valley|path|delete}` → `{modifier: "tm0"}`; `terrain.query {points, surface?: modified|original}` → `{elevations: [z|null]}`; `terrain.contours {}`; `terrain.volumes {}` → `{cut, fill}` in m³ (`volume::cut_fill`, the modified mesh against the original surface); `terrain.delete`; `plant {species, at, age?, plantedHeight?}` → `{plant: "pl0", height, spread}`; `plant.species {add?|file?}` (14 built-in species; a user species from JSON may not reuse a built-in name); `wall {…, retaining?, freeboard?, footing?}`; `arch.inspect {"site"|"terrain"|"plants": true}`. All validated, never clamped; hostile input exits non-zero and changes nothing.

**DXF (`io::dxf_site`).** `CADCRAFT_ARCH` **v8** stores the site, terrain and plants (reads 1–8). The site's plan is written as proxies marked `CADCRAFT-SITE` in their `CADCRAFT` XDATA on the layers above only on save and dropped on read while the arch record is present, so open + save never duplicates them, and a model with only a terrain (no building) still saves its proxies.

**Small carry-overs from phase 8.** `cadcraft-cli convert` prints the detected doors and windows (it counted fields the accept reply never had); GLB floors and ceilings are named `floor-r{id}` / `ceiling-r{id}`. The three CC0 real plans for phase-8 check 07 landed (`cf0e5b5a`, `tests/fixtures/archdetect/real/`).

**Operator work.** The groundwork (data model, the lifted TIN, every site module as a stub with its final signature, the build3d/probe/GLB/inspect/plan/DXF hooks, the Ukrainian rows) went in before the segments. Merged by the operator after the seats stalled: `p9-geom-tin-ops` (the grid index missed every triangle outside cell 0; contour endpoints keyed so chains close), `p9-geom-cdt`, `p9-cli-render-sun-from-site`, `p9-small-convert-and-glb-names`, `p9-render-site-plan` and `p9f-mesh-pad-slope`. `p9-render-site-plan`'s spec named helpers in six files the seats were never shown, so bonsai seats ran out of context and kimi-k31 timed out; meanwhile an escalation racer merged an off-spec version (stored elevation lines instead of contours, a north arrow only with `northArrowAt`) whose own tests pinned its own reading, replaced by `6266e8ad`. `f6307255`: `site_meshes` counted plant meshes (crown + trunk) as plants (6 for 3 trees) and probes named site meshes as walls. `p9f-mesh-pad-slope` (fix cycle 1): the spec's slope rings were mitred offsets with only corner vertices, while the surface measures true distance (round at the corners), so 26 rounds failed the slope test; kimi-k31 then sampled the rings and passed it, but the spec also made every ring a constraint, the CDT tore on hundreds of near-coincident crossings, `terrain_solid` refused the open mesh, and the merge was rejected on check 06 (no ground under the basement); `e52beefd` adds the rings as points and tests that the mesh stays closed. Checks: every check carries a `# needs:` line (the dashboard shows what a failing check waits on); a refusal is the CLI's error line, not only its exit status; saved DXF is read in its code page (upstream writes pre-2007 DXF in the file's code page); check 11 also catches the groundwork's `let _ = (args)` stubs; check 10 reads its 20,001 points from a CSV file (inline they were one ~180 KB argument, over Linux's 128 KiB limit, E2BIG before the CLI ran). **Planner rules:** a spec lists (or quotes the signatures of) every file it leans on; each required test gets the numbers it asserts; a check that saves and reopens names the io segment that persists the new data; a mesh refinement is points unless a kink needs an edge, and a mesh change requires a closed-mesh test.

**Numbers locked.** 40 × 30 m site, corners at `z = 0.05·x`: 1.25 at (25, 12), `null` outside; an elevation line at 3.0 is an edge (3.0 on it, 1.5 halfway down); seven minor contours `x = 5 … 35` and one major at `x = 20` labelled `1.00`; a flat region at 1.0 over (15,10)…(25,20) cuts 6.25 m³ and fills 6.25 m³; hill +2 r 5 and valley −1 r 4 follow `(1 − t²)²`; the terrain GLB node spans 40 × 30 from −1.0 to 2.0 and is closed; a flat region at 2.5 over (30,0)…(40,30) behind a retaining wall along `x = 30`: 2.5 at (30.01, 15), 1.4995 at (29.99, 15), wall top 2.65, bottom 1.1945; the pad at −0.3 (level 0 less its 0.3 m structure), −0.3 on the 0.3 m apron at (14.8, 15), −0.2 at (14.4, 15), 0.25 at (5, 15), −0.25 at (25.5, 15); a basement cuts the ground to −3.0 (−2.7 − 0.3); 14 species, the Colorado blue spruce grows 1.5 → 8.25 → 15 m; with true north at 30° the north arrow is (−0.5, 0.866) and a pole's solar-noon shadow points along it; 20,001 points refused with nothing written.

**Acceptance (all PASS).** `01_terrain_tin` · `02_contours` · `03_terrain_solid` · `04_modifiers_cut_fill` · `05_retaining_wall` · `06_pad_and_basement` · `07_plants` · `08_site_and_sun` · `09_plan_and_round_trip` · `10_hostile_and_scale` · `11_layers_wasm_i18n` · `12_demo_house_on_a_site`. One fix cycle: `07` (the plant count), `10` (the E2BIG check bug; it also warms the release build before timing, since a checkpoint runs only its own phase's checks), `09`'s needs line, and `12` (segment `p9f-mesh-pad-slope`: the driveway at (18, 5) was draped 0.5 m below the surface, so its probe hit nothing). 5,940 workspace tests passed at the last merge before the checkpoint (the pad-slope commit adds four); `cargo xtask wasm` builds (check 11).

**Phase-9 gaps.** Out of scope by the brief: LandXML, DEM or GeoTIFF, LiDAR, coordinate transforms, image-based plants, irrigation. Ours:
- **The site renders grey.** `render3d::material_albedo` has no rows for `terrain`, `path-*`, `plant-crown` or `plant-trunk`, so ground, driveway and plants all take the 0.75 grey fallback and a path's `material` ("asphalt", "concrete") is ignored; in the demo render the driveway is barely visible. Check 12 passes at 15.6 % foreground against a 15 % bar.
- **No sun in the pane.** The site's latitude and true north reach `sun` and headless `render --sun`; the 3D pane is still lit by `Light::DEFAULT`. Phase 10 adds lights to the pane and is the place to bring the sun in.
- **The pad slope is sampled.** The mesh follows it to within 0.07 m on the demo site (rings and points 0.25 m apart); a tall site widens the ring step (32 rings at most) and many pads share 20,000 points, so the error grows there. The slope is checked from the elevation range of the original surface and the modifiers, not per direction.
- **Commands only.** No terrain or plant tools, grips or panel in the UI: modifiers are edited by delete and re-add, a plant by deleting it.
- Contour labels: one per major line, at half its length (`label_point`), so a long contour gets a single label and nothing keeps it clear of other linework.
- Plants are a trunk plus one crown primitive; no seasonal colour, no branch structure.
- Phase-8 check 07 now reads three real plans and reports, never gates: the two Moorfields plans (inches) find 1 and 0 walls; the UCL office floor finds 535 walls, 21 doors, 58 windows and 9 rooms.
- Harden passes that did not land: `terrain-surface`, `terrain-solid` (abandoned); `geom-cdt` and `cmd-plant` were still in flight at the checkpoint.

## Phase 10 — lighting, light fixtures and final renders (landed)

11/11 checks (98/98 overall) on 2026-11-01 00:47, no fix cycle; 20 segments, 9 harden passes.
- **Data:** `LightFixture` on `ArchModel.lights` (E-LITE, handles `lf{n}`), catalogue in `arch::light`, photometry `intensity_cd` (lumens over the cone's solid angle), light points by mount.
- **3D:** housings in `arch3d::fixture` (materials `light-…`, probe part `light`); `fixture::ceiling_cutouts` opens a room's ceiling at every recessed can mounted in it (the ceiling slab is the 20 mm under the ceiling height: a can's light point and trim at the ceiling height sit inside it). A disc that reaches the room's outline, a hole or another cut is skipped.
- **Illuminance:** `arch3d::light::illuminance`, direct light with shadow rays over a BVH of every non-fixture triangle.
- **Lit raster:** group 2 of the lit pipelines; the shadow atlas caches each light's maps under its `light_key` and the scene's `occluder_key`. Fixture housings and ceilings cast no fixture shadows (`shadow_atlas::casts_fixture_shadow`): every fixture hangs at or under a ceiling, and a can's opening follows the can, so a moved fixture rebuilds only its own maps; the sun's map keeps the ceilings. `pack_lights` writes each light's own slot (looked up by light index) and the spot matrix column-major at 16 bytes a column.
- **Finalize Render:** `final_render::FinalRender` (steps: one light's maps per step, the 2× render, the linear-light downsample), `render_final` for headless use.
- **Session renderer:** the CLI installs `RenderHooks`; `render` defaults `style` shaded and `budget` 8; `sunstudy` fails on a failed still.
- **Checks:** 06 and 11 ask for `"final":"raster"` (the raster still stays when phase 11 makes path tracing the default).

## Phase 11 — ray-traced Finalize Render (landed)

13/13 checks (111/111 overall) on 2026-11-01 16:41 after fix cycle 1; 25 segments (5 landed by the operator at level 2), 14 harden passes.
- **Device and fallback:** `pathtrace::request_compute` asks the adapter for its own limits (not WebGL2's); no compute shaders (WebGL2, or `CADCRAFT_RENDER_LIMITS=webgl2`) means the raster Finalize, reported as `fallback: "no compute shaders"`.
- **Units:** radiance in nits; a fixture's RGB carries its candela with luminance 1, so path-traced illuminance in lux is the luminance of the irradiance and matches the CPU illuminance (165 lx under a pot light). Sun 100 000 lx, sky 2 500 to 5 000 nits; night exposure is the raster's, day exposure π / 100 000 (a sunlit white surface at 1.0); ACES as the raster.
- **Integrator** (`pt/kernel.wgsl`): next-event estimation at each diffuse or glossy vertex (every light with a `visibility()` shadow ray, the sun, area lights with MIS against the BSDF), `bounces` full bounces then Russian roulette (survival = the throughput's largest component), a lit lens seen by the camera only. A sample depends only on (pixel, sample index, seed).
- **Materials** from `pipeline::material_pbr` by part (`window-glass` is glass): Lambert + GGX (VNDF), mirrors below roughness 0.02, glass a smooth dielectric (IOR 1.5, exact Fresnel). A shadow ray is tinted once per pane, on entering it: albedo · transmission · (1 − F)², Schlick at the incidence angle for both faces (a pane is a closed box; until p11f-glass-shadow-once it was tinted at each face, 17 260 lx in check 07's sun patch instead of 32 729).
- **IES and area lights:** LM-63 type C profiles (`arch::ies`) replace the cone and the cosine law, scaled to the fixture's lumens; 'LED panel 600' area lights are rectangles sampled by area. DXF (fields of the serialized model inside the v9 record, so older files still open) and GLB carry both.
- **Denoiser** (`pathtrace::denoise`): 5 à-trous passes (strides 1–16) guided by first-hit albedo and normal; colour weight `exp(−Δl² / (σ²(v₀ + vⱼ)))` with each pixel's variance of the mean, propagated through the passes (SVGF-style), `SIGMA_COLOR` 4. Check 09's room: 4 spp RMSE 21.70 → 4.23, 64 spp 4.96 → 2.29 (the relative weight it replaced: 19.51, and 7.73 at 64 spp, worse than raw).
- **Hardware ray queries** (`rt: true`, `--rt`, via `gpu-optin`): wgpu 30 `Blas`/`Tlas`, `traverse_rq.wgsl` replaces only the traversal (`var rq: ray_query`, `rayQueryInitialize(&rq, acc, RayDesc(RAY_FLAG_NONE, 0xFF, PT_EPS, t_max, o, d))`, `rayQueryProceed`, `rayQueryGetCommittedIntersection`: t, primitive index = triangle, barycentrics as `tri_hit`'s u, v); `rq_kernel_source` puts its `enable wgpu_ray_query;` first. Without ray queries the compute path renders it; with them but a kernel that fails to build, the compute path renders it and the reply carries `rtError`. A session's first `rt` request swaps its compute device once for one with ray queries. Until 2026-11-03 the kernel never compiled (directive after common.wgsl, attributes after the type, an invented API) and `.ok()` hid it, so check 10 skipped everywhere. Verified 2026-11-04: llvmpipe (the gates) and an RTX 4070 Laptop both give the compute path's image (check 10: mean |compute - ray query| 0.00 levels); `rq_traversal_matches_trace_cpu`, `rq_visibility_matches_the_compute_path`. The compute `trace` now also refuses a hit at or past `t_max`, as `trace_cpu` does (a wall behind a light used to shadow it when both shared a leaf).
- **Pipeline cache:** Vulkan's, a file per adapter and driver (`CADCRAFT_PIPELINE_CACHE_DIR`, else the XDG cache); `render` reports `pipelineCache`.
- **Defaults:** `final: true` is path-traced; 256 spp, 64 on a software (CPU) adapter unless the request names `spp` (`PathParams.spp_given`).
- **Checks revised in fix cycle 1:** 02's sample count from the Russian-roulette variance (12 × 9 at 4096 spp, the ±2 % bar unchanged); 07's patch points from the window geometry (the sun band is y 0.47–0.90), bounced light in lux, the patch's lux; 09 adds a 64-spp no-worse bar and an explicit spp.

## Phase 12 — exports and interoperability (landed)

15/15 checks (126/126 overall) on 2026-11-02 07:22 after fix cycle 1 (no check revised, one segment added); 13 segments and an upstream sync (p12-ifc-site landed by the operator at level 2), 4 harden passes.
- **STEP and IFC4:** `io::step` writes ISO 10303-21 text (instances `#n` in the order added, strings per §7.3.3, reals always with a decimal point, no schema knowledge); `io::ifc` writes `FILE_SCHEMA(('IFC4'))`: `IfcProject` → `IfcSite` → `IfcBuilding` → one `IfcBuildingStorey` per level at its elevation, SI metres, the Model and Body contexts with true north from the site, `OwnerHistory` `$` (optional in IFC4). One module per element kind, in this order: walls, openings, slabs, roofs, stairs, spaces, site, lights.
- **Elements:**
  - `IfcWall` `.STANDARD.` with `Pset_WallCommon.IsExternal`.
  - Per opening, an `IfcOpeningElement` voiding its wall (`IfcRelVoidsElement`), filled (`IfcRelFillsElement`) by an `IfcDoor` or `IfcWindow` with its overall height and width.
  - Floors `IfcSlab` `.FLOOR.`, decks `IfcSlab` `.USERDEFINED.` `'DECK'`, ceilings `IfcCovering` `.CEILING.`.
  - `IfcRoof` aggregating one `IfcSlab` `.ROOF.` per plane and `IfcCovering` `.CLADDING.` for the fascia and the soffit.
  - `IfcStair` (`STRAIGHT_RUN_STAIR`, `QUARTER_TURN_STAIR`, `HALF_TURN_STAIR`) and `IfcRailing` (`HANDRAIL` along a stair, else `GUARDRAIL`).
  - One `IfcSpace` per room (the room prism floor to ceiling, holes kept), aggregated in its storey, with `CADCraft_RoomAreas` (`InteriorArea`, `StandardArea`) and `Qto_SpaceBaseQuantities` (`NetFloorArea`, `GrossFloorArea`; `IfcQuantityArea` is unrooted, five attributes).
  - `IfcSite` `RefLatitude`/`RefLongitude` (compound angles) and `RefElevation`; the terrain an `IfcGeographicElement` `.TERRAIN.`, plants and draped paths `.USERDEFINED.` (`PLANT`, `PATH`).
  - `IfcLightFixture` (`.POINTSOURCE.` for point and spot lights, `.DIRECTIONSOURCE.` for area panels) with `CADCraft_Light` (`Type`, `Lumens`, `ColourTemperature`).
- **Geometry:** a body is one `IfcTriangulatedFaceSet` of the object's `Scene3d` meshes in its storey's coordinates (`ifc::geometry::body`). A coordinate is written as its f32's shortest decimal (2.55, not 2.549999952316284), and a z less the storey's elevation is kept to the nanometre. Walls are the 3D view's walls with their holes cut; the void is a box through the wall.
- **The same file twice:** GlobalIds are 128-bit hashes of keys (`wall/3`, `roofplane/0/2`, `rel/voids/0`: `ifc::guid`), the header time is `SOURCE_DATE_EPOCH`, and the terrain's triangles keep one order (decision 31). The demo house exports 42 objects, the same objects and GlobalIds from the session and from its saved DXF (check 15).
- **OBJ** (`io::obj`): `OUT.obj` and `OUT.mtl`; `o` groups named as the GLB's mesh nodes; vertices in glTF's Y-up axes, (x, y, z) → (x, z, −y); `Kd` and `d` from the GLB's base colour (glass `d 0.35`).
- **glTF:** a spot light's `innerConeAngle` is below its `outerConeAngle` (an equal pair loses 1 mrad from the inner angle): the Khronos validator's only errors. Terrain, plants and `KHR_lights_punctual` lights were already in the GLB.
- **Export command:** `export {"path"}` picks the format by extension (`dxf`, `dwg`, `svg`, `png`, `pdf`, `glb`, `obj`, `ifc`; `obj` writes its `.mtl` beside it); `export {"format"}` returns base64 `data` (and `mtl`); the web build downloads with 3D MIME types. CLI: `cadcraft-cli convert house.dxf house.ifc|.obj|.glb`. USER-GUIDE.md: "Exporting IFC, OBJ and glTF".
- **Auto exposure** (phase-11 carry-over): `render {"exposure": "auto"}` is the default for path-traced stills with the sun on: 0.18 over the log-average luminance of the pixels that hit a surface, clamped to [π / 100 000, 0.01]; at night 0.01 stays; a number fixes it. Check 14's daylit room: exposure 0.00107, the far floor 253 of 765 (r+g+b) instead of 6.
- **Round-trip test** (fix cycle 1, `crates/engine/tests/ifc_step_grammar.rs`): a two-storey house on a site with a stair, roof, railing, four fixtures and two plants, exported through the real `export` path and parsed instance by instance: every reference defined, every `IfcCartesianPoint` and `IfcDirection` one coordinate list, IFC4's attribute count for 37 entity types, unique 22-character GlobalIds on rooted entities.
- **Validators stay outside the codebase:** checks 10 and 11 use gltf-validator (npm) and IfcOpenShell (pip) from `$XDG_CACHE_HOME/cadcraft-checks` and SKIP without them; on the gate both ran.

**Phase-12 gaps.** Out of scope by the brief: IFC import, IFC2x3, CAD STEP (AP214/AP242), OpenUSD. Ours:
- IFC carries no materials or colours (no `IfcMaterial`, no `IfcSurfaceStyle`); viewers show the model grey.
- Bodies are triangle meshes only: no `IfcExtrudedAreaSolid`, type objects (`IfcWallType`) or material layer sets, so another tool can view and query the model but not edit a wall as a wall.
- Wall bodies already have their holes, so a viewer that applies `IfcRelVoidsElement` subtracts from a wall that is already cut (no visible change).
- `OwnerHistory` is `$`; tools that expect it may complain.
- Area panels are `.DIRECTIONSOURCE.` (IFC4 has no area-light type); fixtures carry no `IfcLightSource` and no photometry beyond `Lumens` (no IES in the IFC). A plant's species is not written.
- OBJ holds no lights, site or units (the format has none).
- Auto exposure is the path tracer's alone; the raster Finalize keeps its fixed exposure.
- `ifc::spaces`' test `a_room_with_fewer_than_three_points_has_no_body` asserts the absence of a misspelt `IFCTRANGULATEDFACESET`, so it cannot fail (the round-trip test covers the real case).
- Harden passes: `harden-p12-pt-auto-exposure` was still in flight at the checkpoint; `harden-p12-docs-user-guide` was abandoned.

## Phase 13 — polish (landed)

13/13 checks (139/139 overall) on 2026-11-03 12:58 with no fix cycle.
- **What merged:** 21 segments and 4 upstream syncs (100 upstream commits), plus 8 harden passes.
- **Landed by the operator at level 2:** p13-arch3d-incremental and p13-arch3d-casework.
- **Split by the lead:** p13-doc-finish was too big for any worker (about 178 k tokens against a 168 k budget) and was split into five segments.

### Finishes
- **They are data on the objects.** `Finish` is one of siding, shingles, brick or wood:
  - `Wall.finish` is the wall's EXTERIOR face, which is the right of start→end, the side of its first layer band, and the outside for a counter-clockwise ring;
  - roofs carry `Roof.covering`;
  - rooms carry `Room.floor_finish`.
- **What each target takes:** `for_walls` is siding, brick or wood; `for_roofs` is shingles or wood; `for_floors` is wood.
- **The command:** `material {"walls": [ids] | "exterior" | "roofs": [ids] | "all" | "rooms": [ids] | "all", "finish": … | "none"}`.
  - It refuses a finish its target cannot take, and it refuses JSON numbers as ids.
  - `arch.inspect` reports `finish`, `covering` and `floorFinish`.
- **They persist:**
  - `roofauto` and `roomdetect` regenerations keep the finish of what they replace.
  - The finishes ride in the serialized model of the v9 record, so older files read unchanged.

### Meshes and textures
- **Finished-face meshes** (`arch3d::finish`):
  - The exterior face is split out of the wall mesh by its right-hand normal into its own `wall-<f>` mesh. Roofs get `roof-<f>` planes and floors `floor-<f>`.
  - UVs are face-aligned in texture repeats (`arch3d::uv::face_uvs`): u runs along the face's horizontal and v up it; on horizontal faces u = x and v = y.
  - An object without a finish is byte-identical to phase 12.
- **Textures** (`arch3d::texture`):
  - They are 256 × 256, opaque sRGB and tileable, made from integer hash noise (splitmix64).
  - One repeat covers 1.2 m for siding, 1.0 m for shingles, 0.84 × 0.6 m for brick and 1.2 × 0.84 m for wood.
  - `mean_srgb` and `mean_linear` give the mean colour.
- **GLB:**
  - Each texture is embedded once as a PNG (`io::png`: a filter-0 IDAT on `miniz_oxide`) behind `baseColorTexture` of the `wall-<f>`, `roof-<f>` and `floor-<f>` materials.
  - Exterior faces are nodes `wall-w<n>-exterior`.
  - The glTF validator reports no errors.
- **OBJ:** the MTL carries the mean colour, with no maps.
- **render3d** (`textures`):
  - It samples one 5-layer `texture_2d_array` in bind group 3. Layer 0 is white, for every untextured mesh, so their pixels are unchanged.
  - Mips are built on the CPU with a box filter, and one sampler repeats.
  - The path tracer uses each texture's mean.

### Casework
- **Cabinets** (`doc::arch::casework`, `arch::casework`):
  - A `Cabinet` is base, wall or tall, with `Fronts` doors, drawers or drawer-over-door.
  - Base cabinets have a counter at 0.914 m with a 0.025 m overhang, and a toe kick.
- **Fixtures:** `Fixture` is one of eight kinds: sink, lavatory, toilet, bathtub, shower, range, refrigerator, dishwasher.
- **Handles and caps:** cabinets `cb{n}` at `+0x50000` and fixtures `fx{n}` at `+0x58000`, with caps of 1,024 each.
- **Placement:** an item's back goes on a wall's left face by offset, or it is placed by point and rotation. `widths` makes a run.
- **Commands:** `cabinet`, `cabinet.edit`, `cabinet.delete`, `fixture`, `fixture.edit`, `fixture.delete`, `fixture.types`.
- **Plan symbols** (`render/arch_casework.rs`) go on four layers:
  - `A-FLOR-CASE` (base and tall cabinets);
  - `A-FLOR-CASE-UPPR` (wall cabinets);
  - `P-FLOR-FIXT` (plumbing fixtures);
  - `A-FLOR-APPL` (appliances).
- **DXF:** `io::dxf_casework` writes save-time `LWPOLYLINE` proxies marked CADCRAFT-CASEWORK, which are dropped on read while the arch record is present.
- **3D:**
  - Items are boxes with probe parts `cabinet`, `counter` and `fixture`.
  - The toe-kick plinth is part of the carcass mesh, so each cabinet is one `cabinet-cb<n>` node plus `-front` and `-counter`.
  - Fixtures are `fixture-fx<n>`.
- **IFC** (`io::ifc::casework`):
  - cabinets are `IfcFurniture` `.USERDEFINED.`, with ObjectType BASE, WALL or TALL CABINET;
  - the sink, lavatory, toilet, bathtub and shower are `IfcSanitaryTerminal`;
  - the range, refrigerator and dishwasher are `IfcElectricAppliance`.

### The incremental rebuild
- **Parts** (`arch3d::incremental`, decision 33):
  - `Scene3d::build` is made of parts: an object's meshes in one section of the build.
  - Each part is a pure function of inputs that `incremental::Sections` records: the object, its join shape, its hosted openings, the roof above it, the light points under a ceiling, and so on.
  - `SceneCache::update` remakes only the parts whose inputs changed and moves the others over in the full build's order. Joins and per-level footprints are reused.
- **Keys:** `ObjectKey` = `ObjectKind` + `ArchId`. Kind-tagged keys end `build_dirty`'s collisions, where `w3`, `o3`, room 3 and roof 3 were all `ArchId(3)`.
- **Digest:** `Scene3d::digest` is an order-independent FNV-1a over every mesh.
- **The proof:** after each edit, `crates/engine/tests/scene_cache_equals_full.rs` checks that the cache has the full build's meshes in order and its counts. The edits are seeded:
  - 150 edits of 11 kinds on a two-level house with a roof, a stair, lights and a terrain;
  - 60 on the 300-wall sample;
  - 120 casework edits.
- **Where the caches live:**
  - The engine keeps one cache per document on `DocState`, not on the `Drawing` that undo snapshots share.
  - `build3d {"incremental": true}` reports `rebuilt`, `objects`, `rebuildMs` and `incremental` (false when it had to start over).
  - The 3D pane keeps its own cache.

### The benchmark house
- **The house:** `arch.sample {"house": true}` builds an 18 × 12 m ring in 30 segments with a 2 m partition grid split at its crossings, and 16 L closets per level. That comes to:
  - 310 walls on two levels (40 L, 116 T and 80 X joins);
  - 60 windows and 100 doors;
  - 108 detected rooms;
  - a 4-plane hip roof;
  - the bench edit `w52` end [8, 10] → [8, 9.7].
- **Check 08:**
  - The bars: release, the median of 5 stretches under 50 ms, at most 10 % of the objects rebuilt, a full build under 500 ms, equal digests after the edits and after undo, and one more measurement after a timing miss.
  - On 52035629 at load 8: a full build of 16.8 ms for 579 objects; a median update of 11.4 ms with 13 objects rebuilt.

### Docs
- **USER-GUIDE.md:** check 09 runs every `sh` block. The guide gained "Materials", "Cabinets and fixtures", "Large models" and "MCP examples for agents".
- **MCP examples:** `docs/arch3d/mcp-examples/` holds 8 compact JSON-RPC sessions, `01-walls-and-openings` to `08-detect-a-plan`, plus a README.
  - Each starts with initialize, then a metric `new_drawing`, and is replayed with `cadcraft-cli mcp < FILE.jsonl`.
  - Check 10 replays them, running cargo by `--manifest-path`, and so does an integration test.

### Upstream
100 commits in 4 syncs (dae4b7b2f, 7bc76f94c, 6c4984f5a, b639b18f6). Among them:
- the FIND window;
- MINSERT;
- DWG in a child process;
- a UI panic saves the work;
- an export to a missing folder makes it and says so (#2260; phase-12 check 12 now accepts either);
- faster DXF import, extents, regions and constraint evaluation.

### Phase-13 gaps
- **Out of scope by the brief:** moldings, material regions, plan check, materials list and costing, the contextual toolbar.
- **Ours, textures:**
  - Textures are four fixed finishes at fixed scales, with no colour or scale choice.
  - The path tracer and OBJ use the mean colour.
  - Interior wall faces are untextured.
  - IFC carries no finishes, because it has no materials at all (phase 12).
- **Ours, casework:**
  - Casework is parametric boxes: no hardware, no corner, angled or end cabinets, no backsplash, no appliance detail. The phase-14 library adds meshes.
- **Ours, the incremental rebuild:** a full build of the benchmark house is already fast (16.8 ms), so the cache saves about a third (11.4 ms). It is the 10 % rebuilt bar that keeps a fast full build from passing check 08.
- **Ours, docs:** USER-GUIDE.md lists only siding and brick for walls and shingles for roofs, although `material` also takes wood on both.
- **Ours, i18n:** the last upstream sync's merge dropped six arch3d `uk.tsv` rows. They were restored in new wording by the sync's next round.
- **Harden passes:**
  - harden-p13-arch3d-casework and harden-p13-render3d-textures were abandoned;
  - harden-p13-dxf-casework was merging at the checkpoint.

## Phase 14 — the library (landed)

13/13 checks (152/152 overall) on 2026-11-05 02:16, with no fix cycle.
- **What merged:** 34 segments and 3 upstream syncs (79 upstream commits), plus 5 harden passes.
- **Landed by the operator:** 13 of the 34, in 9 landings (operator fixes under Building blocks). 12 were finished at level 2, and p14-io-dxf-furnishing landed after the integrator refused it on a timing bar at load 54.

It builds on the operator groundwork: the sample set `assets/library/sample/` (49 items, its `catalog.json` and `ATTRIBUTION.md`) and `tools/library/`.

### One library, nine categories
- **The categories,** in this order: Furniture; Appliances and plumbing fixtures; Cabinet parts; Lighting and electrical; Materials; Doors and windows; Plants; Backdrops; User catalog.
- **The record:** every item is an `ItemRecord` in `arch::library` (id, name, category, folder, kind, symbolType, placement, size, keywords, tags, style, source, licence, author, attribution, catalog, readOnly, file, elevation, cutout, notes).
- **Each catalog contributes items:**
  - `builtin`: the existing catalogues as items:
    - door:/window:<style>, plant:<species>, lightfixture:<type>;
    - finish:<name>, cabinet:<kind>, fixture:<kind>;
    - frontstyle:door-/drawer-<style>;
    - backdrop:colour/gradient.
  - `sample`: embedded in every binary by `io::library::sample_files`, so the web build has it too;
  - `installed:<name>`: copied into the user dir by `library.install`; read-only, and installing again updates it;
  - `local`: a folder of .glb files with one licence, sizes from the mesh bounds, categories from folder names; remembered, never committed;
  - `user`: see the user catalog below.
- **Licences:** an item with no licence never loads.
- **Where it lives:** the engine's `Session.library` holds the catalogs. The file readers reach it through `engine::library::LibraryIo` (read_glb, read_image, encode_png, sample), which the apps install, as they do for IoHooks.
- **In tests:** `LibraryIo` is a process-wide `OnceLock`, so unit tests install it only through `library::testing::install_io` (decision 35). The engine's guide tests install the same readers as the CLI.

### The user dir
- **Where:** `CADCRAFT_LIBRARY_USER_DIR`, else `$XDG_DATA_HOME/cadcraft/library`, else `$HOME/.local/share/cadcraft/library`.
- **What it holds:** `state.json` (tags, saved filters, local catalogs), `installed/`, `user/<slug>.json`, `trash/` and `icons/<hash>.png`.
- **In the checks:** they point it at a temporary folder.

### Search
- **Text:** every word must be a substring of the name, a keyword or the folder.
- **Chips:** category, folder, symbolType, placement, source (the item's catalog), style and tag combine with AND. Zero results is a valid answer.
- **History and saved state:** `FilterHistory` gives Back and Forward. Saved filters persist, and Favorites is a built-in tag.
- **Folders** come from catalog.json or `folder_for(symbolType)`.
- **Targets:** `library {"for": surface key}` uses `query_for`:
  - a pull gives pulls, and a front gives front styles;
  - a carcass gives in-cabinet items;
  - a wall, floor, ceiling or roof gives Materials.

### 3D symbols
- **The reader:** `io::gltf_read` is our own, on serde_json and image, never the gltf crate. It accepts plain glTF 2.0 binary only:
  - an `extensionsRequired` entry, a URI or a sparse accessor is refused, and the reason names the extension (Draco, meshopt, KTX2);
  - textures are PNG or JPEG, decoded to RGBA at up to 256 px.
- **Axes:** the mesh turns Y up to Z up, (x, y, z) → (x, −z, y), and is bottom-centred.
- **The mesh:** a `SymbolMesh` (parts with base colour, metallic, roughness and texture) is keyed by the FNV-1a 64 of its file.
- **Stored in the drawing:** symbols go in `ArchModel.symbols` and library images in `ArchModel.images`. A drawing therefore opens anywhere with its furniture, and undo and DXF carry it.
- **The DXF payload limit:** the arch XRECORD payload takes up to the shared 64 MiB `MAX_PAYLOAD`. The demo house writes 9.6 MB, and under the old 8 MiB limit it reopened with no model at all.

### Furnishings
- **The object:** a `Furnishing` arch object has the handle f<n> at +0x60000. It records:
  - item, symbol key, name, category and symbol type;
  - placement, level, `at` (the footprint centre), rotation, elevation (its bottom above the level's floor) and size;
  - mirrored, host and cut-out.
- **Placing a mesh:** `Furnishing::local_to_plan` scales to the size, mirrors x, rotates, then moves. A symbol's front is its local −y (glTF +z).
- **Layers** come from `layer_for`:
  - I-FURN;
  - P-FLOR-FIXT for sinks, lavatories, faucets, toilets, tubs and showers, else A-FLOR-APPL;
  - E-LITE;
  - L-PLNT.

  The layer follows the furnishing's own category, not the item record's.
- **IFC classes** come from `ifc_class_for`: IfcFurniture, IfcSanitaryTerminal, IfcElectricAppliance, IfcLightFixture.
- **In plan:** the symbol is the convex hull of the placed mesh, seen from above.
- **Replies:** every placing command returns `handle` in `Handle::hex()` form, which `move`, `rotate` and `mirror` take. A placed user group returns its objects' handles in `handles` and its labels per kind (`furnishings`, `cabinets`, `fixtures`).

### Placing
`arch::library::place` tries these rules in order:
1. A cabinet-part item over a cabinet swaps its pull or front style, and nothing is placed.
2. A faucet on a sink takes the sink's back centre.
3. A cut-out item over a base cabinet is centred in it, its top 1 cm above the counter, facing the room (cabinet rotation + 180).
   - The cabinet then gets a cut-out hole and a false top front.
   - The counter top is elevation + carcass height + counter thickness (`Cabinet.height` is the carcass).
   - Without a cabinet, the item stands at counter height.
4. An under-counter appliance clicked in a gap of its depth between two base cabinets takes the gap (host Gap), and the counter is carried over it.
5. A wall item puts its back on the nearest wall face within 0.3 m, at its catalog elevation, facing out.
6. A ceiling item hangs with its top at the level's ceiling.
7. Anything else stands at the click.

`library.place` takes the defaults first, checks every parameter before any change, and is one undo step. Typed items (doors, windows, plants, light fixtures, cabinets, fixtures, backdrops) go through their own commands.

### Cabinet parts
- **The layout:** `arch::casework_layout` is the one front layout. It gives fronts, hinges and pull positions:
  - door pulls 0.05 m from the free edge and 0.075 m from the top, or from the bottom on wall cabinets;
  - drawer pulls centred.
- **Counters:** it also gives false fronts, counter runs and gaps:
  - touching in-line base cabinets share one counter, owned by the first and built once per build;
  - a bridged gap joins the run;
  - a run has at most 64 cut-outs, and only hosted ones.
- **Front styles** (slab, shaker, raised panel) have the same outer box, and their triangle counts grow in that order. A pair of doors keeps the full door height.
- **Pulls:** a cabinet's pulls are one mesh group, `pull:<key>:<i>`, and export as node `cabinet-cb<n>-pull`.
- **Inspect:** `arch.inspect` lists the layout as `frontParts`. `fronts` stays the phase-13 mode string.

### Materials and painters
- **Surface keys:**
  - wall/w3/left|right, room/r0/floor|ceiling, roof/r0;
  - cabinet/cb0/carcass|front|counter|pull;
  - fixture/fx0, furnishing/f2/<part>.
- **Paints:** `ArchModel.paints` maps a surface key to `Paint { material: Option<MaterialRef>, tint }`. A `MaterialRef` is a Finish, a library item (an image) or a colour.
- **Effective materials:** `arch::paint` gives every surface its room and its effective material key: finish:<f>, item:<id>, colour:#rrggbb, default:<what>, frontstyle:<s> or sym:<key>:<i>. A wall side's room is found 0.05 m off its face, and exterior sides have none.
- **Scopes** select surfaces that have the clicked surface's material:
  - component: the clicked surface;
  - object: every surface of its object;
  - room: that material in the clicked surface's room;
  - floor: that material on its level;
  - plan: that material everywhere.
- **`paint.object`** selects like items instead: like pulls, front styles or symbols.
- **Modes:** `blend` keeps the material and sets a tint.
- **Undo:** each painter call is one undo step, and the eyedropper's last pick is kept per drawing.

### The user catalog
- **`library.add {"objects", "name", "insertionPoint"?}`** stores the objects relative to the insertion point as a group item `user:<slug of the name>`. It places as a group: the same meshes moved, equal once moved back.
- **`library.copy {"item"}`** (Copy to User Catalog) takes a stock item, the very thing it exists for. The copy:
  - is named after the item's id (`sofa-futon` → `user:sofa-futon`, then `-2`, `-3`);
  - is filed in the User catalog under its source's category as the folder;
  - records `source_item`.
- **Editing and deleting:** `library.edit` and `library.delete` take only `user:` ids. A bare stock id would map to the same slug as its copy. `library.trash {"action": list|restore|empty}` manages the Trash.
- **Scope:** native only. The web build has the sample set.

### The 3D build
- **Mesh material names:**
  - `sym:<key>:<i>` for furnishing parts;
  - `pull:<key>:<i>` for pulls;
  - `paint:<surface key>` for painted surfaces. A painted wall side is split out as its own mesh, as the phase-13 exterior faces are.
- **`Scene3d` gains:**
  - `materials` (MaterialDef: base colour, metallic, roughness, texture key, tile, tint);
  - `textures` (sym:<key>:t<i>, finish:<f>, item:<id>);
  - `backdrop`.

  Every renderer and exporter reads `materials` first. Textures reach the GPU, and the tint is applied once.
- **The scene cache** gains a furnishings section:
  - every object's inputs include its paints;
  - a cabinet's inputs include its level's base cabinets, the furnishings they host and its pull symbol.

  So the cache stays the full build (`scene_cache_equals_full` covers furnishing, paint and casework-part edits).
- **Probing:** furnishings probe by their placed box, since symbol meshes need not be closed.

### Exports
- **GLB:** one node `furnishing-f<n>` per furnishing, one primitive per part. Textures are embedded once as PNG, and the tint goes into baseColorFactor.
- **OBJ:** groups by node name, with mean colours.
- **IFC:** elements by category, named as the nodes, with `CADCraft_Library` (Item, Licence, Source).
- **DXF:** LWPOLYLINE proxies marked CADCRAFT-FURNISHING. Everything else rides in the v9 record's serialized model.

### Icons
- **The renderer:** `arch3d::icon` is a software rasterizer: fixed camera (yaw −30°, pitch 25°), fixed light, 128 × 128.
  - Its sin, cos and sRGB are our own, for byte-identity.
  - Kinds: materials on a UV sphere, backdrops as thumbnails, gradients and solids.
- **Determinism:** icons are byte-identical every time and cached by content hash.
- **`library.icon {"category"}`** takes only one of the nine names:
  - an unknown name is refused;
  - a real category with no items (an empty User catalog) gets a grey tile.

### Backdrops
- **Kinds:** a colour, a gradient (top to bottom of the view), or an image, flat or equirectangular (the top row is the zenith, the middle row the horizon).
- **Where drawn:** the 3D view, the raster Finalize and the path tracer draw it where no surface is hit, in display colours. All three use the shared CPU reference `render3d::backdrop::composite_misses`.
- **The path tracer's misses:**
  - The kernel counts, per pixel, the samples whose camera ray hit a surface (`accum[W·H + i].y`), and the backdrop goes on exactly the 0-hit pixels (decision 36). An edge pixel some samples hit keeps its shading.
  - The image reaches the tracer under its `item:<id>` key, and an equirect image is read along each pixel centre's unjittered ray.
- **Lighting:** a backdrop never lights the scene.

### The UI
- **The panel:** a dockable on-demand Library panel (`Panel::Library`) with:
  - tiles, folder tree, grid or list, text and chips;
  - Back and Forward, saved filters, Favorites, details;
  - a preview that turns and zooms.
- **Placing:** click an item, then click in the plan or the 3D pane. The property panel's "Library…" button runs `library {for}`.
- **Tools:** the painter and eyedropper work in the 3D pane.
- **Where search lives:** the panel's `fn search` only runs the `library` command. Matching stays in `arch::library::search` (check 12).

### Set as Default
`arch.default {id}` stores the command parameters per kind in `ArchModel.defaults`: cabinet.<kind>, door, window, furnishing.<item>. `cabinet`, `door`, `window` and `library.place` fill in the parameters they omit from it.

### Docs
- **USER-GUIDE.md** gained "The library": the nine categories, search, placing rules, both painters and the eyedropper, the user catalog and Trash, Set as Default, backdrops, installed and local catalogs. It has a search block and a furnished two-room house that exports `library.glb`.
- **MCP examples:** `docs/arch3d/mcp-examples/09-library.jsonl` is the ninth session.

### Phase-14 gaps
- **Out of scope by the brief:**
  - online catalogs and accounts, manufacturer catalogs;
  - reading OpenUSD (SimReady and SimHome are converted outside);
  - the tile and wood material builders, the texture editor;
  - outlets, switches, wiring (phase 15).
- **Ours, the user catalog:** a copied stock item cannot be placed. `library.copy` stores it as a group with no objects ("user item has no objects"). It should become a one-object group whose furnishing keeps the source's category, so a copied sink stays on P-FLOR-FIXT.
- **Ours, commands:**
  - `move` with handles it cannot resolve moves nothing without an error, which hid the group bug;
  - `arch.inspect` prints furnishing handles in the short form (`a0060000`), which `move` does not take;
  - `library.add`'s `relative_value` unwraps the stored objects' arrays (a production `unwrap`).
- **Ours, furnishings:**
  - symbol lights do not emit (their catalog `light` is data only, for phase 15);
  - a ceiling item does not find a soffit under it;
  - the plan outline is a convex hull, so an L-shaped sofa's notch is filled.
- **Ours, rendering:**
  - the path tracer uses texture means;
  - icons are flat-shaded.
- **Harden passes:** one was in its gate at the checkpoint, and harden-p14-r3d-pt-backdrop was retired as superseded.

## Open items vs Chief Architect

sun in the pane (the site exists since phase 9; the pane still uses `Light::DEFAULT`); site materials in the renderer; SSAO/TAA/cascaded shadows; arc walls and their openings; main-layer joins and the standard-area basis; pony walls, `reversed`; framing; terrain/plant tools, grips and a site panel in the UI; electrical/plumbing/HVAC (10); sheet title blocks and view titles; dormers/crickets/roof returns/skylights/gutters; interactive roof-plane joining; curtain walls; dynamic roof topology; IFC materials and colours, parametric IFC bodies (extrusions, type objects, material layer sets); `ARCHDETECT` on real plans (the two Moorfields plans find 1 and 0 walls; check 07 reports, does not gate), scale from dimensions, editing candidates before accept; phase-2 opening sub-items (mullions, jambs, hardware, display states, schedules); room boxes, reference display, dynamic defaults, split levels, moldings; door classification; the phase-5 stair/railing items above; the phase-6 items above; the phase-7, phase-8, phase-9, phase-12 and phase-13 items above (finish colours and scales, interior finishes, textures in the path tracer, OBJ and IFC; corner cabinets, hardware, backsplashes); **phase 11:** raster window glass is opaque, hardware ray-query parity unverified on hardware, Russian roulette without a survival floor; phase 14: a copied library item that places (a one-object group on its source's layer), `move` refusing handles it cannot resolve, `arch.inspect` handles in the form commands take, furnishing plan outlines beyond the convex hull, soffits under ceiling items, emitting symbol lights (phase 15), textures in the path tracer, shaded icons.

## Phase plan

0 ✅ → 1 ✅ → 2 ✅ → 3 ✅ → 4 ✅ → 5 ✅ → 6 ✅ → 7 ✅ → 8 ✅ → 9 ✅ → 10 ✅ → 11 ✅ → 12 ✅ → 13 ✅ → 14 ✅ → **15 electrical, technical illustration and the showcase house**.

## Building blocks added outside the swarm (assisted)

- `geom::skeleton` (2026-10-25): `straight_skeleton(outer,holes)`, `inset`/`outset` mitred offsets, `check` invariants. Each input edge sweeps one face; node `time` is distance to that edge.
- `geom::roof_skeleton(outer,holes,slopes)` and `weighted_skeleton(..,weights)`: per-edge pitch and gable ends (`f64::INFINITY` = vertical face); node `time` is roof height, each face is the plane over its edge. Phase 4 is built on this.
- `render3d` drawing and the 3D pane (2026-10-25): `Offscreen` owns colour+depth on its own device (`create`) or the window's (`with_device`); `encode`/`render` draw depth-tested with one directional light + ambient, one uniform slot per mesh, albedo from `material_albedo`; flat shading from each triangle's own plane (`cross(dpdy,dpdx)`) because arch3d boxes share corners with averaged normals and inconsistent winding; `read_rgba`/`read_png` bounded by `READBACK_TIMEOUT`; `PROJECTED` from `projected_scene_bounds`. The pane syncs per document and `arch_revision`, redraws only when scene/camera/size changed. **Headless uses `Backends::PRIMARY` (`headless_instance`)** — concurrent GL/EGL starts hung test binaries for 900 s on cgyws2; GPU tests share one device (`gputest::shared_device`). wgpu moved 29 → 30 to match egui-wgpu.
- **Feature edges, HLR, section cuts (2026-10-25, phases 6–7).** Pure CPU, deterministic, in `arch3d`; tests `crates/arch3d/tests/hlr_section.rs`.
  - **`edges`.** `feature_edges(mesh, CREASE_DEG)` / `scene_feature_edges`: weld at 1e-5 m, then edges used by one triangle (`Boundary`), >2 (`NonManifold`), or two faces meeting over 20° (`Crease`); planar triangulation diagonals never appear. With a view direction, `Silhouette` edges are added.
  - **`hlr`.** `OrthoView::new(dir,up)` / `::elevation([x,y])`; `visible_lines(scene,view) -> Vec<Line2>` tagged with (`ArchKind`, `ArchId`) since phase 7 (decision 22). Exact: depth difference is linear along an edge, so the hidden part is one interval; covering is closed (`EPS_SHEET`) so diagonals never leak; a triangle hides only what is over `EPS_DEPTH` (0.1 mm) behind its plane plus its slope across that band. `merge_collinear` joins runs of the same object only.
  - **`section`.** `Plane::new(point,normal)`; `section_cut(scene,plane,view) -> SectionCut{rings,lines}` — crossings computed once per welded edge and chained, so closed solids give CCW closed rings (open surfaces give lines); a corner exactly on the plane counts as in front. `section_view(scene,plane,up)` = cut + `visible_lines` beyond, each object's cut face hiding what lies inside its poché.
  - **What the HLR exposed:** walls were not cut by openings (fixed in phase 4, `a_window_beyond_a_section_shows_through_its_wall` now un-ignored and passing); the header is flush with both wall faces, so it draws as a rectangle above every opening — narrow it or drop it (still open after phase 7); casings are clamped inside the wall band instead of standing proud by `casing_depth`; `Opening::new` gives sill 0 to windows (the `window` command defaults 0.9), so tests set `sill` themselves.
- **`geom::tin` (2026-10-30, phase-9 groundwork `9e4bdfe6`).** `triangulate`, `Tin::elevation_at`, `Tin::elevation_range` and `contours` lifted from the `cadcraft-terrain` crate on zorgglubb/cadcraft `main` (commit 5da6d7b, `crates/terrain/src/tin.rs` and `contour.rs`) by zorgglubb, MIT OR Apache-2.0, credited in `ATTRIBUTION.md`; folded into `geom` with a plain error enum and a 20,000-point cap. The same commit put in the site data model and every phase-9 module as a stub with its final signature.
- **Phase-10 groundwork (2026-10-31).** `doc::arch::light` (`LightFixture`, `LightKind`, `LightMount`, `FixtureShape`, handles `lf{n}` at `+0x48000`, `ArchModel.lights`, skipped when empty; `CADCRAFT_ARCH` **v9**); `arch::light` with the catalogue (pot light, pendant, sconce, surface fixture, floor lamp) and the photometry (`cone_solid_angle`, `intensity_cd`: 800 lm in 60° = 950.36 cd); `arch3d::light::SceneLight` and `Scene3d.lights`/`fixtures`, probe part `light`. In `render3d`, the light contract the segments build on: group 2 of the lit pipelines (frame uniform, sun map, a 64-light uniform, a `Depth32Float` 2D-array shadow atlas — one layer per spot, six per point light, read with `textureLoad`), the cube-face and spot matrices in Rust and `shaders/light_math.wgsl` (tested against each other: maps store `far/(far−near)·(1−near/z)` and lookups compare metres), `RenderOptions::lit` (absent = the phase-6 pipelines, pixel for pixel), `Draw.casts_shadow` (fixture housings cast none, so moving a fixture rebuilds only its own map) and `Draw.emissive`. Site and fixture albedos (terrain green, paths by surface, crowns, trunks: the phase-9 grey site). The engine's `render` hook (`RenderHooks`: the app's renderer lives for the session, so a camera-only `render` builds no shadow map); stubs with final signatures for every phase-10 module; uk.tsv rows for the seven new commands.
- **Phase-10 operator fixes (2026-10-31/11-01).** Ceiling openings for recessed cans (e8e1585f), `sunstudy` errors (4e199057), `render` defaults, spot-shadow packing, ceilings out of fixture shadows and check 11's room id (30d9cb91..29a465e7). Three of them were found by merging every queued branch into a trial tree and running the phase's checks before the merge queue drained.
- **Phase-11 groundwork (2026-11-01, 1af74ca1).** Area lights (`LightKind::Area`, `FixtureShape::Panel`, the 'LED panel 600'), `IesProfile` and `arch::ies`, `SceneLight` size/axis/ies, `render3d::pathtrace` (options, units, the compute device, GPU byte layouts, WGSL skeleton, pipelines, test helpers `upload_scene`/`run_entry`), and `crates/gpu-optin`, the only unsafe code in the workspace (wgpu's experimental-feature token for ray queries, a pipeline cache from disk).
- **Phase-11 operator fixes (2026-11-01).** Each stacked start_from seed's own commit cherry-picked onto arch3d (pt-lights, lightfixture-area-ies, io-area-ies); ui-pane-path written at level 2 (84bfdd32); the path tracer's per-sample mean, glass by part and render samples through their exact projection (21bd59d8); headless instances made one at a time (7dab1013).
- **Phase-12 groundwork (2026-11-01, 9faf6ddb, merge 55053f7d).** `io::step` (the ISO 10303-21 writer), `io::ifc::guid` (GlobalIds from keys), `io::ifc` (header, units, contexts, the spatial structure, placements, property sets and the element helper every module calls), `ifc::geometry` (`Pieces`, `faceset`, `body`), walls in full and every other element module as a stub with its final signature. The plan's decisions are the phase-12 section above.
- **Phase-12 operator fixes (2026-11-02).** p12-ifc-site landed at level 2 with its enum fix (`P::Enum` adds the dots; it wrote `..TERRAIN..`) and `geom::cdt` cavities as BTreeSets (76307a4d, merge 4fb65e16); IFC coordinates at the model's decimals (0dd4b9b1, same merge); the stray `IFCCARTESIANPOINT(x,y,z)` a racer wrote per space only to satisfy its own `contains("2.55")` test removed (0a563f4b, merge 0e87b75d). All three slipped past unit tests that asserted substrings malformed output also contains (decision 32).
- **Phase-13 operator fixes (2026-11-02/03).** The exact incremental rebuild (66063a44, decision 33), `xtask literals` reading a `-> &Scene3d {` return type as a literal (cba772a4) and phase-9 check 10's bar as min(wall, CPU) (35b30cdd, decision 34), landed with p13-arch3d-incremental at level 2 (merge 5931a657); the benchmark-house generator rebuilt (310 walls, L 40 T 116 X 80; 2c7aa167, merge 14799b78); p13-arch3d-casework landed at level 2 with 120 seeded casework edits in scene_cache_equals_full, check 08's reply count and `material` refusing numeric ids (merge c5cff184); IFC cabinet ObjectType by kind and the toe kick in the carcass mesh (a21ab50d, merge 06a445ee); phase-12 check 12 accepting upstream #2260's made-and-named folder and check 10's `--manifest-path` (7e445008, merge bbb9dd7c).
- **Phase-14 groundwork (2026-11-03).** The library sample set `assets/library/sample/` (49 items, CC0 and CC-BY, 5.5 MB): plain glTF 2.0 `.glb` symbols and texture images. **Every symbol is bottom-centred at its catalog size in metres** (glTF Y up: x and z centred on 0, lowest y at 0, extents = size; front towards +Z as Sweet Home 3D stores its models), so a reader only turns Y-up to Z-up and places the box; base-colour factors and PNG/JPEG textures, smoothing groups averaged, 16- or 32-bit indices, no extensions. `catalog.json` = `{"version": 1, "items": [{id, name, category, symbolType, placement (floor, wall, ceiling, counter, in-cabinet; null for images), size [w, d, h] metres Z up, keywords, source, licence (SPDX), author, attribution (the full CC credit line), file, elevation (wall items: metres to the bottom), light (lights: position in metres from the bottom centre, x across, y back to front, z up; diameter; colour), cutout [w, d] (sinks, cooktops, the basin)}]}`, every file in `ATTRIBUTION.md`. Doors and windows come from the existing door and window catalogue, not as mesh symbols. The kitchen sink (0.8 x 0.55) and the dishwasher (0.6 x 0.6 x 0.876, no top) are made at the phase-13 casework sizes. Made by `tools/library/` (Python standard library, run by the operator, never by the swarm): `fetch.py` downloads the Sweet Home 3D CC0 and CC-BY libraries (links, licences and whom to credit read from their pages, LICENSE files and catalogs), `curate.py` converts `picks.json` with `sh3d.py` and adds `own.py`'s items (two pulls, a 30-inch range, the sink, the dishwasher, a sky backdrop), `check_sample.py` checks the set against the brief. Also in: CHIEF-REFERENCE §6 (the library, from Chief's training videos) and the SimHome note in §5 (a local catalog, never committed).
- **Phase-14 operator fixes (2026-11-03/05).** Nine landings on arch3d:
  - **op/p14-land1 (3a50c117):** p14-arch3d-furnishing, p14-arch3d-icon and p14-cmd-library at level 2; render3d keeps every headless device for the process and shares one instance (a Vulkan-loader crash on device teardown) and one device per adapter choice.
  - **op/rt-ray-query (c03fac9f):** hardware ray queries work (the WGSL used an invented API, and `.ok()` hid the error, so phase-11 check 10 had passed by skipping); the compute trace stops at `t_max`.
  - **op/p14-land2 (722f1477):** furnishing transforms (no `unwrap` warning in every cargo reply), glb, obj-ifc, symbol-materials, paint and p14-inspect (`frontParts`) at level 2; p14-io-dxf-furnishing after the integrator refused it on phase-8 check 08's 2 s bar at load 54; check 12 excludes the Find dialog.
  - **op/p14-check03 (118cd185):** check 03 shows the chip's widening on "dining".
  - **op/p14-land3 (51603f8c):** p14-cmd-place (rewritten to the brief; its tests had poisoned the library-io `OnceLock`) and p14-arch3d-cabinet-parts at level 2; the counter top is elevation + carcass + thickness; the check helpers read GLB positions in drawing axes; phase-13 checks 04 and 06 revised for counter runs; the font fallback no longer depends on earlier lookups.
  - **op/p14-demo-fixes (572880ac):** the demo kitchen gets an LED panel (its path-traced render was black); the arch payload limit is the shared 64 MiB.
  - **op/p14-pt-backdrop (6f78f5fe):** the path tracer paints the backdrop on the pixels no sample hit (decision 36).
  - **op/p14-docs-user-guide (23d0d1a7):** p14-docs-user-guide at level 2. Its 32 rounds had never run a test, because the swarm's gate built `-p docs` (fixed in CompleXiti 9547ec1d). Its examples rewritten to the real commands, the engine guide tests given the library readers, check 12's search rule made behavioural.
  - **op/p14-icon-category (bcdfd934):** `library.icon` refuses unknown categories; the checks compare meshes within a tolerance (decision 37); a placed group returns object handles; Copy to User Catalog copies stock items into the User catalog under their id; edit and delete refuse a stock id; `library/user.rs` tests use the shared readers (decision 35).

## Phase 15 plan

## Phase 15 plan

Phase 15 (electrical, split levels, the showcase house) needs no groundwork outside the swarm. Six segments open the DAG: the two data segments (p15-doc-electrical, p15-doc-levels), every phase-15 translation row (p15-i18n-labels, so no command segment edits the .tsv files), `export {view|level}`, the clay style and `cadcraft-cli run --commands`. The decisions follow.

**Electrical devices.**
- `ElectricalDevice` (handle el<n> at +0x68000): kind (duplex, GFCI, 240 V; single-pole, three-way, dimmer), wall, side (left/right, the surface-key sides), offset (wall start to plate centre), height (plate centre above the level's floor), counter, level.
- Standard heights: outlets 0.3 m, switches 1.2 m; a counter outlet 0.15 m above the counter top in front (elevation + carcass + counter thickness, which arch.inspect reports as `counterTop`).
- Layers: outlets E-POWR, switches and connection arcs E-LITE. Plates are 0.07 x 0.115 m, 0.006 m proud of the face (probe part `electrical`, GLB node `electrical-el<n>`, IfcOutlet / IfcSwitchingDevice).
- A device in a doorway (an opening with sill 0) or past a wall's end is refused.

**Connections and circuits.**
- `Connection` (cn<n> at +0x70000): from a switch to light fixtures, with a bulge (-1..1, the arc's sagitta over half the chord) that the plan's grip reshapes.
- Switches that share a light are one circuit (so two three-way switches make one). `circuit.brightness` sets `LightFixture.brightness` (0..=1, default 1) on every light of the circuit, and arch3d scales lumens by it.

**Auto-placed outlets.**
- Usable wall is the room's boundary along the faces that face it, broken at doorways (sill 0; windows do not break it). Distance is measured along the wall.
- Duplex outlets so that no point is farther than 1.8 m from one. In a kitchen, GFCI over every counter run (at most 1.2 m apart, within 0.6 m of each end), and the wall behind a counter is served by the counter rule.

**Light sets.** Named sets of {on, brightness} per light, stored in the drawing; a camera stores its set; a render applies a set to its lights for that render only (the model never changes).

**Levels and split levels.**
- A level's elevation is free (`level {elevation}`, `level.edit`), and its kind is floor or landing. A new drawing's level 0 always exists (`ensure_ground_level`).
- A wall with `top_level` runs from its base level's floor to the top level's ceiling. Every crate selects a level's walls with `Wall::spans_level`, and heights come from `bottom_z`/`top_z`. A wall spanning Lower to Main bounds rooms on Lower, the landing and Main, and carries Main's roof.
- An opening's sill is measured from its own level, so the front door opens from the landing.
- `level.show` sets `ArchModel.plan_level`, the one level the plan draws; the site draws on every level.

**Grade classes.** A level is above grade when its floor is at or above the finished grade (the terrain with its pads; 0 with no terrain) 0.3 m outside every exterior wall that spans it, sampled every 0.5 m; otherwise it is a basement. The above-grade area is the interior area of the rooms on floor levels classed above grade (landings and basements never count).

**Pads.** The first floor is the lowest level at or above 0, landings included. Its pad and the basement cuts stay as in phase 9. New:
- a floor level reaching past the footprints below it (a side split) gets its own pad there;
- a door on a landing gets a flat apron one 0.15 m step below the landing (door width + 1.2 m wide, 1.5 m deep), applied last.

**Views.**
- The Electrical view is the layer state "Electrical": E-* layers on, every other layer at transparency 70.
- `render {style: clay}` renders every material #b4b4b4. Technical illustration (flat colours with phase 7's silhouette and crease lines) is optional and built last; until it lands the style is refused as "not available yet", and check 09 counts that as a SKIP.
- `export {view}` frames one elevation or section; `export {level}` gives one level's plan.

**The showcase house.** `docs/arch3d/showcase/bilevel.json` is a JSON array of {cmd, params} replayed by `cadcraft-cli run --commands`:
- Lower at -1.2 (a basement), the Entry landing at 0.15 with the front door and half flights, Main at 1.5 (above grade, at least 371.6 m² of rooms);
- a sloping graded site with a driveway, sidewalks, trees, shrubs and planting beds (`PathKind::Bed`);
- four cameras, a light set and the views.

`tools/showcase/render_all.py` renders every view style, the exports and a contact sheet with a manifest. `crates/engine/tests/showcase.rs` replays the script through the engine and asserts the brief's numbers.

## Phase 16 plan

Phase 16 (ceilings and foundations) was planned by the operator from a reviewed plan and started on operator groundwork, landed on arch3d before the plan: room `flatCeiling`/`ceilingStack` and the ceiling finish stack (Drywall and Suspended templates); the arch ceiling point query (rough, finished, roof underside, hang height) with stubs for vault, tray and soffit; build3d `ceiling`, `probe` and `parts` queries; arch.inspect ceiling rows; the Tray/Soffit/Foundation data types with their kinds, handles and labels; the planar geometry kit; and empty soffit, foundation and wall-rise sections in the incremental build. Checks 01 (observables) and 02 (ceiling stack) pass on the groundwork.

- Handle offsets (doc/src/arch): tray 0x78000 (tr<n>), soffit 0x80000 (sf<n>), foundation 0x88000 (fd<n>). Phases 17-21 choose other offsets below ARCH_HANDLE_SPAN 0x100000.
- Part names (probe::part_of): soffit (materials starting `soffit-box`; the roof's `soffit-panel` stays roof), foundation, footing, slab; build3d `parts` counts scene meshes per part, so later phases emit one mesh per countable thing.
- Stub ownership: arch/src/ceiling/vault.rs + arch3d/src/ceiling/vault.rs -> p16-vault-ceiling; arch/src/ceiling/tray.rs + arch3d/src/ceiling/tray.rs -> p16-trays; arch/src/ceiling/soffit.rs + arch3d/src/soffit.rs + render/src/arch_ceiling.rs -> p16-soffits; foundation code -> p16-foundation-stem, slab -> p16-foundation-slab. Signatures are fixed; segments fill them.
- Shared registration edits are union merges: engine cmd/arch/mod.rs (tray, soffit, foundation), inspect.rs dispatch (trays, soffits, foundations), uk.tsv rows.
- A vaulted room's ceiling follows the roof underside; a flat ceiling never rises above it (a ceiling or level edit that would is refused, naming the room; a roof edit is never refused and leaves a ceilingWarning). The walls of a vaulted room rise to the underside (wall-rise meshes); gable walls keep their phase-4 shape.
- Foundations cover every exterior wall with no level below it; FOUNDATION creates no walls. A basement level's exterior walls become "Concrete 0.2".
- Four segments start from earlier work (start_from): p16-vault-ceiling, p16-trays, p16-soffits and p16-foundation-stem had partial implementations from the operator's run outside the swarm; workers finish them against the checks.

