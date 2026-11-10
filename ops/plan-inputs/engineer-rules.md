Project rules engineers follow:
# Engineer rules — arch3d on CADCraft

Read `CLAUDE.md` and `AGENTS.md` first; they outrank this file. These are the rules that matter for
this feature arc.

## Hard non-negotiables

- **Never crash.** No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, no
  `unsafe` (`unsafe_code = "forbid"`). Every production crate root carries
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`.
  Return `Result<T, E>` through the crate's error type. Unfinished features return an error or say
  "not available yet" — never panic.
- **Hostile input.** DXF linework, command JSON and MCP params are adversarial: `get()` not `[]`,
  checked arithmetic, reject non-finite numbers, cap allocations and loop counts derived from input,
  bound recursion. The straight skeleton, join solver and wall detector must survive garbage.
- **Clean room.** Implement from first principles and public documentation of *behaviour* (Chief's
  manuals/videos, IFC ISO 16739, AIA/NCS layer names, standard stair and roof geometry). Never
  decompile, disassemble, read or copy code or assets from Chief Architect, Home Designer, Revit,
  ArchiCAD, SketchUp or AutoCAD. Never read or link GPL/LGPL/AGPL geometry code (OpenCASCADE,
  LibreCAD, QCAD, FreeCAD's planegcs, SolveSpace, LibreDWG).
- **Pure Rust, licence-compatible deps only** (MIT, Apache-2.0, BSD, Zlib). No C++ geometry kernels.
  Prefer writing our own small writer (GLB, OBJ, STEP) over adding a dependency.
- **Never break wasm.** WebGPU with a WebGL2 fallback must keep working; no platform-specific code
  outside a `#[cfg]` and never in a crate below L5.
- **No new assets** without a row in `ATTRIBUTION.md` (`cargo xtask assets` enforces it). Icons are
  drawn in code. Test corpora live in separate repos, not here.

## Layering

`cargo xtask layers` enforces: L0 `geom`,`color` → L1 `doc` → L2 `constraints`,`fonts`,`render`,
**`arch`,`arch3d`** → L3 `io` → L4 `engine` → L5 `ui-egui`,`mcp`,**`render3d`** → apps (exempt).
Only L5+ may use `egui`/`eframe`/`egui-wgpu`/`wgpu`/`winit`/`rfd`. When you add a crate you must
add it to `xtask/src/layers.rs::TABLE` (and to `INTRA_LAYER_ORDER` if it sits in the same layer as
something it uses) **in the same commit**. `arch` and `arch3d` must not import wgpu or egui. The UI
crate is swappable: keep the 3D viewport in `render3d`, not in `ui-egui`.

## Where code goes

- **Data** (`Wall`, `Opening`, `Room`, `Level`, `Roof`, `Stair`, `Material`, …) goes in
  `crates/doc/src/arch.rs` and is stored on `Drawing`. Anything not inside `Drawing` does not undo,
  autosave, audit or round-trip.
- **2D algorithms** (joins, room detection, roof skeleton, stair calculator, detection, plan
  symbols) go in `crates/arch`. No rendering, no wgpu, no egui.
- **The 3D builder** (`build(&ArchModel) -> Scene3d`, hidden-line removal) goes in `crates/arch3d`.
  Pure, deterministic, incremental.
- **Formats** (DXF round-trip, glTF, OBJ, IFC) go in `crates/io`.
- **The viewport** (wgpu pipelines, cameras, shadows, SSAO, picking, headless offscreen) goes in
  `crates/render3d` (L5).

## Adding a command (and its MCP reach)

1. Create or extend a module under `crates/engine/src/cmd/arch/` and expose
   `pub fn specs() -> Vec<CommandSpec>`.
2. Build the spec: `CommandSpec::new("wall", "Wall", run_wall).menu(&["Architecture","Wall"])
   .alias(&["w..."]).params("{points: [...], thickness?, height?, type?}").interactive(|s| Ok(Box::new(...)))`
   — ids are lower case; add `.noundo()` only for view-only commands.
3. Register it: append `v.extend(arch::specs());` in `crates/engine/src/cmd/mod.rs::command_specs()`.
4. Implement `run_*(&mut Session, &Value) -> Result<Value>`: parse with the helpers in `cmd/mod.rs`
   (`f64_req`, `size_param`, `point_req`, `points_param`, `targets`, `bad`) and return JSON
   (e.g. `{"handles": [...]}`). **Programmatic calls never open dialogs.**
5. Implement the interactive form as an `Interactive` state machine returning `Prompt`s
   (`crates/engine/src/prompt.rs`); use `Accept::POINT`, keywords, `base` for rubber-banding.
6. **MCP needs no new tool** — `execute`, `list_commands` and `command_line` reach every command
   automatically. Only add a tool in `crates/mcp/src/tools.rs` for scene-level output (e.g.
   `build3d`, `render3d`), with an `inputSchema` and a dispatch arm in `call_tool`.
7. Add a CLI path where the brief asks for one (`apps/cadcraft-cli`).

## Testing

- Unit tests in-module (`#[cfg(test)] mod tests`) for pure logic; integration tests in
  `crates/<crate>/tests/*.rs`, one topic per file, named for the behaviour
  (`xdata_follows_entity.rs` style).
- Required coverage for this feature: join cases (L, T, X, angled, collinear), opening placement
  limits, room detection with holes, area rules against hand calculation, roof-generator geometry
  (planes meet exactly, correct pitch, no gaps), stair code limits.
- **Property/fuzz tests** (see `crates/geom/src/property_tests.rs`): random wall graphs must never
  panic and must produce manifold walls; hostile DXF must never panic (`hostile_dxf_does_not_panic`
  style).
- **Golden scenes**: small `.dxf` plans + expected `ArchModel` JSON and glTF hashes.
- **Image tests**: render goldens with a perceptual tolerance via the headless renderer.
- **Benchmarks**: incremental rebuild timing under edit; the 300-wall / 50 ms target is a test, not
  a hope.
- UI tests use `egui_kittest` frames (see `crates/ui-egui/tests/`).

## Running things

```sh
cargo xtask ci                 # fmt, clippy -D warnings, tests, assets, layers, wasm — before every commit
cargo xtask layers             # dependency layering
cargo xtask wasm               # the web build still compiles
cargo test -p cadcraft-arch -p cadcraft-arch3d
cargo run --release -p cadcraft -- --sample --control PORT   # drive it over JSON lines
cargo run -p cadcraft-cli -- run --sample --script 'WALL ...' --export out.png
cargo run

