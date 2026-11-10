# Phase 19 plan: Light, sky and cameras

Phase 19 (light, sky and cameras) in 19 packages over 5 waves: two groundwork packages (look data + parser + check lib; GPU layout + hook stubs), then exposure (physical raster, EV100, tone maps), sun (lux, shadows, named angles), environment light in PT (importance sampling) and raster (SH9), sky (Preetham + twilight + moon + stars), HDRI (.hdr reader, intensity/rotation/horizon), ground to the horizon, strip lights (model; round a tray on phase 16's trays; render), normal maps (data+glTF; raster+PT), seeing in (cutaway/clip/near model+pick3d; render), camera looks/reset/overview, pane controls and the demo. Checks 01-16 in tests/acceptance/phase-19.

Source: `the challenge brief (brief-*.md)` phase 19; behaviour `docs/arch3d/CHIEF-REFERENCE.md` §7.3; code studied on `op/next` (a2887a143) and re-checked at 576538153, which adds the CC-0 HDRI; revised after review round 1 (see Notes, REVIEW ROUND 1) and review round 2, re-checked on `op/next` 7f78b7024 (see Notes, REVIEW ROUND 2), and review round 3, re-checked on `op/next` 15d9af8fa (see Notes, REVIEW ROUND 3), and review round 4, which repeated round 3's findings, all already applied (see Notes, REVIEW ROUND 4). The JSON beside this file is the machine-readable plan.

## Packages

| wave | id | title | deps | checks |
|---|---|---|---|---|
| 0 | `p19-foundation` | Render looks in the drawing: Look types, the `look` command, effective looks reaching the renderer; phase-19 check lib and carry-forward checks | - | 12_hostile.py, 13_layers_wasm_i18n.sh, 14_look.py |
| 0 | `p19-r3d-groundwork` | GPU groundwork: reserved uniform fields, shader hook functions and per-frame look plumbing in render3d, no visible change | - | - |
| 0 | `p19-horizon` | Ground to the horizon: a flat ground plane outside the site skirt at the boundary's mean edge elevation, in the site's ground material, reaching the far plane | - | 06_horizon.py |
| 0 | `p19-strip-model` | Strip lights: LightFixture kind strip along a path, under a cabinet run or as a ring along a room's walls, lumens per metre, down or out; plan symbol, housing, CPU illuminance, inspect and exports | - | 05_strip.py |
| 0 | `p19-normal-maps` | Normal maps: materials carry a tangent-space normal map, procedural brick/siding/shingle/wood maps, user catalogue materials with their own, glTF normalTexture | - | 09_normal_maps.py |
| 1 | `p19-exposure` | Exposure: auto or manual EV100 with ACES/Hable/none tone mapping, the same in the pane, the raster Finalize and the path tracer; a physically scaled raster | p19-foundation, p19-r3d-groundwork | 01_exposure.py |
| 1 | `p19-sun` | The sun: intensity in lux, cast shadows on or off, sun off, and named sun angles stored in the drawing (`sunangle`) | p19-foundation, p19-r3d-groundwork | 02_sun.py |
| 1 | `p19-env-path` | Environment light in the path tracer: an Environment type, importance-sampled equirect environments with MIS, visible or light-only | p19-foundation, p19-r3d-groundwork | - |
| 1 | `p19-seeing-in` | Seeing in, the model: cutaway hides camera-facing exterior walls and what they host, a six-plane clip box and a near clip, honoured by pick3d | p19-foundation | 07_cutaway_clip.py |
| 2 | `p19-env-raster` | Environment light in the raster views: nine spherical-harmonic coefficients for diffuse light and the environment drawn behind the scene, exposed and tone-mapped | p19-exposure, p19-env-path | - |
| 2 | `p19-strip-render` | Strip lights rendered: one light against the raster budget sampled every 0.15 m, an emissive strip in the path tracer | p19-exposure, p19-strip-model | 16_strip_render.py |
| 2 | `p19-normal-maps-render` | Normal maps used by the raster views (texture array + derivative tangent frame) and by the path tracer (per-triangle UVs and texels in the aux buffer) | p19-exposure, p19-normal-maps | 10_normal_maps_render.py |
| 2 | `p19-seeing-in-render` | Seeing in, rendered: cutaway, clip box and near clip honoured by the raster views and the path tracer | p19-exposure, p19-seeing-in | 15_seeing_in_render.py |
| 3 | `p19-sky` | A sky that follows the sun: Preetham clear sky, twilight to night from 0 to -6 degrees, a moon light and optional stars, in the pane, raster Finalize and path tracer | p19-sun, p19-env-path, p19-env-raster | 03_sky.py |
| 3 | `p19-hdri` | HDRI backdrops: a Radiance .hdr reader in Rust, `backdrop {kind: hdri}` with intensity, rotation and horizon offset, lighting the scene; horizon offset for 360-degree image backdrops; the CC-0 sample HDRI registered in the catalog | p19-foundation, p19-env-path, p19-env-raster | 04_hdri.py |
| 3 | `p19-strip-tray` | Strip lights round a tray: `lightfixture {type: "LED strip", tray}` runs a closed strip along a phase-16 tray ceiling's outline, inside its recess | p19-strip-model, p16-trays (phase 16) | 05_strip.py (case g) |
| 3 | `p19-cameras` | Cameras keep their look: saved cameras store technique, sun angle, backdrop, exposure, light set, cutaway and clip box; camera.reset/orbit/update; floor overview camera kind | p19-exposure, p19-sun, p19-seeing-in-render | 08_cameras.py |
| 4 | `p19-pane` | The 3D pane: lit by the drawing's and the active camera's look, with exposure/tone-map/sun/sky controls, cutaway toggle, a clip box dragged in the pane, near clip, camera reset and floor overview | p19-cameras, p19-sky | - |
| 4 | `p19-demo` | Demo: a small house at dusk (twilight sky, lit windows and strips, ground to the horizon), at noon under an HDRI, and a floor overview with the cutaway, each from its own saved camera | p19-sky, p19-hdri, p19-horizon, p19-strip-render, p19-strip-tray, p19-normal-maps-render, p19-cameras | 11_demo.py |

## Brief item -> package

| brief item | package |
|---|---|
| Exposure: automatic (log-average toward middle grey) or manual EV100, pixel = L x 1/(1.2*2^EV100) | p19-exposure |
| Exposure: tone mapping ACES (default), Hable or none | p19-exposure (default declared in p19-foundation's Look) |
| Exposure: applied the same in the pane, the raster Finalize and the path tracer, replacing the fixed constant | p19-exposure |
| Sun: intensity in lux (default 100 000), cast shadows on/off, sun off with the backdrop lighting alone | p19-sun (default declared in p19-foundation's Look) |
| Sun: named sun angles (date+time or azimuth+elevation) stored, several per plan, chosen per camera | p19-sun (per-camera storage of the choice rides on p19-cameras' look capture) |
| Sky: analytic clear sky from the sun's position, in pane/raster Finalize/path tracer, lighting and background | p19-sky (machinery: p19-env-path, p19-env-raster) |
| Sky: 0 to -6 deg twilight to night; moon (dim directional, default 0.25 lx, own position); optional stars | p19-sky (moon default declared in p19-foundation's Look) |
| HDRI: Radiance .hdr (RGBE, RLE) read in Rust, 360-degree backdrop that lights the scene | p19-hdri |
| HDRI: importance-sampled in the path tracer | p19-env-path |
| HDRI: raster diffuse light as nine SH coefficients | p19-env-raster |
| HDRI: intensity and rotation per backdrop; horizon offset for any 360-degree backdrop | p19-hdri |
| HDRI: a CC-0 HDRI joins the sample set (groundwork) | p19-hdri registers the Poly Haven CC-0 HDRI already on op/next (576538153) as a Backdrops item |
| Ground to the horizon | p19-horizon |
| Strip lights: kind strip along a path at a height, under a cabinet run, round a tray, lm/m, colour temperature, down or out | p19-strip-model (path, under a cabinet run, room ring); round a tray: p19-strip-tray (on phase 16's trays) |
| Strip lights: raster one light against the budget sampled every 0.15 m; path tracer emissive strip | p19-strip-render |
| Normal maps: a material carries a tangent-space normal map; procedural brick/siding/shingle/wood get matching ones; glTF normalTexture | p19-normal-maps |
| Normal maps: used by the raster views and the path tracer | p19-normal-maps-render |
| Seeing in: cutaway rule (camera-facing walls on the viewed level + hosted items); clip box (six planes) and near clip; pick3d | p19-seeing-in |
| Seeing in: honoured by the raster views and the path tracer | p19-seeing-in-render |
| Seeing in: clip box dragged in the pane | p19-pane |
| Cameras: each saved camera stores technique, sun angle, backdrop, exposure, light set, cutaway, clip box; camera.reset | p19-cameras |
| Cameras: floor overview camera kind (current level and below, no ceilings), lower levels optional | p19-cameras |
| Numbers: exposure (furnace EV100 1 -> 0.8333/235, EV100 2 -> 0.4167/173) | p19-exposure, check 01 |
| Numbers: sun (88 612 lx at solar noon, sky off; column shadow gone with shadows off) | p19-sun, check 02 |
| Numbers: sky (noon b>r, 2 deg r>b toward the sun, -10 deg < 1 % of noon) | p19-sky, check 03 |
| Numbers: HDRI (uniform radiance 1, albedo 0.5, EV100 -1, no tone map: 0.8333, +-2 % path, +-5 % raster) | p19-hdri, check 04 |
| Numbers: strip (2 m, 500 lm/m, down, 2.4 m: 49.72 lx +- 2 %) | p19-strip-model, check 05 |
| Numbers: horizon (row 5 px under the horizon has no sky/clear pixels) | p19-horizon, check 06 |
| Numbers: cutaway and clip (pick3d) | p19-seeing-in, check 07 |
| Numbers: cameras (same image after global changes; camera.reset after an orbit) | p19-cameras, check 08 |
| Demo: dusk, noon under an HDRI, floor overview with cutaway, each from its own saved camera | p19-demo, check 11 |
| Carry-forward: hostile input; layers/wasm/i18n; check lib | p19-foundation, checks 12, 13 (and 14_look.py) |
| Out of scope: DoF, colour grading, upscaling/DLSS, animated water/pools, shadow animation, clear coat, anisotropy, translucency, volumetrics, aperture/shutter/ISO | nobody (EV100 is the only exposure control) |

## Notes

Phase 19 in 19 packages. Every agent reads the work rules (engineer-rules.md) first (worktree from op/next, build/test only through `cb`, TDD, never crash, ..Default::default() for shared structs, serde defaults, a uk.tsv row per new label, small commits, no merging). Checks live in tests/acceptance/phase-19/, load p19lib.inc (05, 06 and 09 the previous lib chain), observe only through the CLI, use the brief's numbers, fail on a stub.
WAVES: 0 p19-foundation, p19-r3d-groundwork, p19-horizon, p19-strip-model, p19-normal-maps | 1 p19-exposure, p19-sun, p19-env-path, p19-seeing-in | 2 p19-env-raster, p19-strip-render, p19-normal-maps-render, p19-seeing-in-render | 3 p19-sky, p19-hdri, p19-cameras, p19-strip-tray (it also waits for phase 16's p16-trays: the workflow's dependency registry spans phases) | 4 p19-pane, p19-demo. Critical path foundation -> exposure -> env-raster -> sky -> demo; start foundation and exposure first. The checks of the wave-0 packages that do not depend on p19-foundation (05 strip, 06 horizon, 09 normal maps) do not load p19lib.inc: they exec the newest existing previous-phase lib (phase-18..15) directly.
GROUNDWORK: p19-foundation owns look DATA (doc Look, the one parser apply_look_params, `look`, effective looks into RenderRequest/Lighting, check lib, carry-forward checks); p19-r3d-groundwork owns GPU LAYOUT (reserved uniform fields, shader/tracer hook stubs each labelled with its filling package), so later packages edit only their own functions; the merger takes unions of registration lines, struct field additions and match arms.
CODE FACTS THAT SHAPED IT: exposure constants 0.01/lux (EV100 6.38) and pi/1e5 (14.70); the PT's auto is clamp(0.18/Lavg, pi/1e5, 0.01) in daylight and 0.01 at night; phase-12 check 14 pins only the legacy `render {sun: false}` night (0.01, auto or not) and daylight auto <= 0.01 -> daylight auto keeps its clamp, the legacy `sun: false` form keeps 0.01 (Lighting.meter_night false), and every other frame without daylight (sun below the horizon or off, moonlit, fixture-lit) is metered toward 0.18 within [pi/1e5, EV100 -6 = 53.33], replacing the fixed constant as the brief says; reply `exposure` stays the scale (code wins); the raster is not physical -> p19-exposure makes the lit raster physical and tone-maps the interactive view (ACES default; a deliberate change); the raster albedo is u.albedo x the texture layer and the raster ground quad exists only at upload -> the calibration albedo reaches the shader through PbrUniform.lambert_albedo + PBR_FLAG_LAMBERT and the meter is told `ground`; SUN_LUX 100 000 already in the PT; the direct illuminance has no sun; the PT is at 8 storage buffers and rq uses @group(1) -> per-still data in the `ies` aux buffer; no known-Lambert surface -> calibration render param `albedo`; picture backdrops stay unexposed display colours (phase-14 check 09), only sky/HDRI are physical; the clear colour stays the default background; render's below-horizon refusal goes (twilight); `sun: false` keeps meaning night (sun and daylight sky off), `sun: {on: false}` = backdrop lights alone; render.rs parse_path_params refuses non-number `exposure` on every render -> p19-foundation lets "auto"/objects through; render3d Lighting derives Copy and ui-egui pane3d.rs copies it -> p19-foundation drops Copy and edits pane3d.rs; render3d does not depend on cadcraft-arch -> the sky model lives in arch3d and moon directions are resolved by the hook/pane (arch3d::sky::moon_dir) and passed in (Lighting::with_moon); the engine reaches io only through LibraryIo fn pointers -> the .hdr reader lives in cadcraft-arch next to ies.rs; the DXF payload is one JSON <= 64 MiB -> HDR bytes are stored hex-encoded, <= 1024x512 each, at most 4 images / 8 MiB, pruned; Catalog::install copies only an item's `file` -> p19-normal-maps teaches it normal maps; Opening.offset is the opening's centre; duplicate camera saves stay refused (camera.update re-saves); the horizon is kept out of Scene3d.meshes/bounds; WebGL2 limits respected (16 KiB uniforms); engine backdrop.rs accepts only colour|gradient|image|none -> p19-foundation adds the `sky` form, p19-hdri `hdri`; arch3d fill_materials loads only model.backdrop's picture -> p19-foundation's set_backdrop loads the effective look's; the PT composites a backdrop over its misses and leaves them black without one, and at bounces 0 glass gets no direct light; cam.kind is matched exhaustively in ui-egui pane3d.rs; p14lib's HOUSE1 is the 8 x 5 two-room house (x = 8 is its east wall).
MAPPING (brief item -> package): Exposure bullet -> p19-exposure; Sun intensity/shadows/off and named angles -> p19-sun (per-camera choice stored by p19-cameras' look capture); Sky (model, twilight, moon, stars) -> p19-sky; HDRI reader/backdrop/intensity/rotation/horizon/sample -> p19-hdri, its PT importance sampling -> p19-env-path, its raster SH9 -> p19-env-raster; Ground to the horizon -> p19-horizon; Strip model (path, under cabinets, room ring, lm/m, kelvin, down/out) -> p19-strip-model, round a tray -> p19-strip-tray (on phase 16's trays), strip raster budget + PT emissive -> p19-strip-render; Normal maps data + glTF -> p19-normal-maps, raster + PT use -> p19-normal-maps-render; Seeing in rule + pick3d -> p19-seeing-in, raster + PT (shadows included) -> p19-seeing-in-render, clip box dragged in the pane -> p19-pane; Cameras keep their look + camera.reset + floor overview -> p19-cameras; defaults (ACES, auto, 100 000 lx, 0.25 lx moon) are declared once in p19-foundation's Look; numbers: exposure 01 p19-exposure, sun 02 p19-sun, sky 03 p19-sky, HDRI 04 p19-hdri, strip 05 p19-strip-model (+ case g p19-strip-tray), horizon 06 p19-horizon, cutaway and clip 07 p19-seeing-in, cameras 08 p19-cameras; demo 11 p19-demo; carry-forward 12 hostile, 13 layers/wasm/i18n (+14 look) p19-foundation; out of scope (DoF, grading, upscaling, water, shadow animation, clear coat, anisotropy, translucency, volumetrics, aperture/shutter/ISO) -> nobody.
OPEN ITEMS: the CC-0 HDRI groundwork is on op/next since 576538153 (assets/library/sample/hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr, 1024x512 RLE RGBE, CC0-1.0, Greg Zaal / Jarod Guest via Poly Haven, ATTRIBUTION row present, not yet in catalog.json): p19-hdri registers it; nobody makes an own HDR. Phase 16's trays are built tonight in parallel: p19-strip-tray depends on p16-trays (a phase-16 id; if op/next has no tray code when it starts, it stops and reports that instead of guessing). Phases 16-18 run tonight: p19lib.inc loads the newest previous lib that exists; normal_map tolerates new Finish variants; wall-hosted objects added by 16-18 join the cutaway only through existing host fields. Check 13 needs the USER-GUIDE section written by p19-demo and the hdri/README.md ATTRIBUTION row (p19-foundation).
REVIEW ROUND 1 (17 findings, each checked against the brief and op/next 576538153; all applied, none rejected; findings 2/13 and 1/10 were the same defects). (1, 10) p19-hdri registers the Poly Haven HDRI already on op/next (no synthetic HDR, no own.py change), embeds it in crates/io/src/library.rs (sample_list_matches_the_catalog), maps .hdr to a Backdrop in catalog.rs entry_record and install, and Library::image tone-maps it, so library.icon and the Backdrops category icon (phase-14 checks 01, 11) keep working. (2, 13) check 04(c) looks along -Y (u = 0.25 red at rotation 0, 0.75 blue at 180) instead of the -X seam. (3) checks 07 and 15 aim at the window's centre x = 3.0 (Opening.offset is the centre: arch symbol.rs:50-53) and 07(a) asserts part glass. (4) new p19-strip-tray (deps p19-strip-model and phase 16's p16-trays) builds the brief's "round a tray"; it follows the tray OUTLINE (arch::ceiling::tray::outline, the recess edge) inset into the recess, not the reviewer's base_ring, which for a top-level tray is the room polygon and so would repeat the {room, inset} form; check 05 case (g) and the demo use it. (5) the calibration albedo reaches the raster through PbrUniform.lambert_albedo (offset 108, named in the groundwork's table) + PBR_FLAG_LAMBERT in base_albedo, ignoring u.albedo and the texture; unit test with the ground quad. (6) auto meters every frame without daylight except the legacy `sun: false` form (Lighting.meter_night; RenderRequest.legacy_night); the night range is [pi/1e5, EV100 -6 = 53.33] rather than the suggested 0.01..10 (a moonlit albedo-0.1 frame needs 22.6, and a bright fixture-lit room needs less than 0.01); checks 01(e) and 03(g) test it. (7) the clip box is honoured by the sun and fixture shadow passes (fragment discard); check 15 pins sun, EV100 and tone map and tests that the removed ceiling casts no shadow. (8) sky.rs moves to arch3d; the moon direction is resolved by the hook/pane through arch3d::sky::moon_dir and passed in by Lighting::with_moon. (9) read_hdr lives in cadcraft-arch (crates/arch/src/hdr.rs, beside ies.rs). (11) Lighting drops Copy; p19-foundation edits ui-egui pane3d.rs and runs its tests. (12) parse_path_params lets "auto"/object exposures through; engine test for `render {exposure: {ev100: 3}}`. (14) frame_look/exposure_look take `ground`, Lighting carries `lambert`, the meter's BVH includes the ground quad (and the horizon); 01(c) states the sun straight down. (15) Catalog::install validates and copies an item's normal map; Library::normal_map_bytes. (16) stored HDRs are at most 1024x512, hex-encoded, capped at 4 images / 8 MiB, pruned to what backdrops and camera looks reference, with a DXF round-trip test at the caps. (17) the wave-0 checks 05, 06 and 09 load the previous phase's lib chain directly. Also fixed while checking: check 15(a)'s sample sat on a wall HOUSE1 does not have (no wall at y = 3): it now uses room A's north wall; 07(b) names room A (HOUSE1 has west and east rooms, no south room); Environment's PartialEq compares its key, never its pixels, because the pane compares Lighting every frame; p19-pane depends on p19-sky (with_moon).
REVIEW ROUND 2 (8 findings; 2 and 3 are the same defect; each checked against the brief and op/next 7f78b7024; all applied, finding 5 with a different fix than the one suggested; none rejected). (1) p19lib.inc no longer redefines HOUSE1: it stays p14lib's 8 x 5 two-room house for 07, 08, 09, 10 and 15, and check 08 is rewritten on it (targets [4,2.5,..]; level 1 is a ring over room A; no PHASE1_HOUSE is needed, since the demo writes its own house). (2, 3) p19-foundation (item 2b) adds the `backdrop {kind: "sky"}` arm (allowed, Backdrop::Sky, reply, describe, refusal text, params doc) with engine test backdrop_sky_sets_reports_and_refuses_extra_keys; p19-hdri adds its `hdri` arm beside it; p19-sky's FACTS name the owner and p19-sky does not edit engine backdrop.rs. (4) 08(d) picks at P = (6.5,1.5,0) in room B: from the overview eye (4, 3.02, 30) the ray stays in room B below z 2.4 and passes level 1 at x >= 6.0; x = 8 is w2's centre line. (5) agreed: at bounces 0 the path tracer gives glass no direct light and the north wall lies in the ceiling's shadow, so 15(a) and 15(c) read 0 against 0 in the path. The suggested fixture would not fix it: the default fixture is a 60-degree spot that misses the north wall, and even a 1 500 lm point fixture gives that wall about 14 lx, which reads 0 at EV100 15. Instead, 15(a) compares the pixel of its own sample: without the cutaway it shows w0's sunlit outer face, with it the shadowed north wall. 15(c) uses a camera inside room A facing w0, which with nearClip 1.0 sees only the backdrop. Check 15 sets `backdrop colour #ff00ff`, because the path tracer leaves misses black without a backdrop (tracer.rs backdrop_composite). (6) p19-cameras adds the Overview arm to ui-egui pane3d.rs load_active_camera and runs cb test -p cadcraft-ui-egui. (7) arch3d `set_backdrop(model, scene, backdrop)` (p19-foundation) loads an image backdrop's pixels under item:<id> from model.images; fill_materials, the hook and the pane use it; check 08(f) saves a camera with an equirect own:backdrop-sky backdrop and requires the same raster and path digests after the global backdrop becomes a colour. (8) p19-foundation adds the ATTRIBUTION.md row for assets/library/sample/hdri/README.md (re-running the xtask's logic on op/next lists exactly that file, so phase-14 check 12 and phase-15 check 13 fail on it today); p19-hdri's curate.py keeps both hdri rows. Also fixed while checking: 07(b) picks from above at room A's centre (2.025, 2.5), as the brief's cutaway-and-clip numbers line says.
REVIEW ROUND 3 (7 findings, all major; each checked against the brief and op/next 15d9af8fa; all applied, none rejected; finding 5 with the reviewer's first option, which the in-flight p19-foundation worktree already implements). (1) p19-hdri edits engine furnish.rs: backdrop_params turns a Backdrop item whose file ends in .hdr into {kind: hdri, item} and forwards intensity, rotation and horizon (the library panel arms library.place for every non-material item, and backdrop_params sent {kind: image}, which p19-hdri refuses for an .hdr item); engine test furnish::tests::hdr_backdrop_item_places_as_hdri; check 04(f) places the sample with `library.place {item, at: [0, 0]}` (library.place requires `at`) and reads kind hdri back. (2) check 06(a) runs every assertion in all three renderers: no CLEAR and no black pixel in row 125, and the magenta-backdrop variant in each (in the path tracer a miss is black without a backdrop, never CLEAR, so only the magenta variant can catch a missing horizon there); new (a2): row 125's mean over columns 180..319 (clear of the room) is green like phase-10 check 08's terrain and within 12 per channel of the site's own terrain on row 140. (3) check 02 pins `sky: false` on every sun-measuring call (direct and path illuminance, every render) and exposure {ev100: 15} + toneMap none on its renders, and says why (from wave 3 path illuminance counts the Preetham sky even at bounces 0); p19-sky's re-run note names the pin. (4) FrameLook gains sun_lux (groundwork item 4; p19-exposure adds it if missing): exposure_look sets it to look.sun.intensity only while lighting.sun is Some, else 0, and fill_exposure writes p.sun_intensity = f.sun_lux, so night frames keep no sun term (the hook passes Light::DEFAULT as the light at night and the shader has no sun-on gate; lit.rs:132-133 zeroing sun_intensity was the only guard); p19-sky sets it from Lighting::directional_lux() for the moon; offscreen test night_lit_frame_has_no_sun_term. (5) render3d composite_misses returns early for Sky/Hdri (p19-foundation item 1), so the path tracer never paints CLEAR_RGB over a visible sky or HDRI; p19-env-path relies on it (with a fallback in backdrop_composite if it is missing) and adds tracer test visible_environment_shows_through_a_sky_backdrop. (6) p19-normal-maps names normal images `<base>-normal` after the base images and edits phase-13 checks 02 and 13_demo to compare only base-colour images, with comments (both list the GLB's images exactly); the in-flight p19-normal-maps worktree already does this. (7) p19-sun's frame_sun sets FRAME_SKY only with look.sky.on, so `sky: false` removes the old 2 500-5 000-nit gradient (9 425 lx on open ground) from the path tracer at bounces > 0; tracer test sky_off_leaves_only_the_sun; checks 01(c) and 01(e) pin bounces 0 on their path renders. In flight when this was written (read-only look at the wave-0 worktrees): p19-foundation already returns early for Sky/Hdri in composite_misses; p19-horizon's check 06 already has the black and three-renderer magenta tests but not (a2); p19-normal-maps already names `-normal` images and edits the two phase-13 checks; p19-r3d-groundwork's FrameLook has no sun_lux yet.
REVIEW ROUND 4 (7 findings, all major; each re-checked against the brief and op/next 15d9af8fa, which has not moved since round 3; all agreed, none rejected, none needing a new change). They are round 3's seven findings again, cited at the line numbers the plan had before round 3 (CHECK 06 at :494, now :502; p19-normal-maps item 4 at :629, now :639), and every fix is already in both files, so this round changes no package id, file list, dependency, test or check. Where each lives: (1) crates/engine/src/cmd/arch/furnish.rs is in p19-hdri's files; item 6 maps an .hdr Backdrop item to {kind: hdri, item} and forwards intensity, rotation and horizon; test furnish::tests::hdr_backdrop_item_places_as_hdri; check 04(f) places the sample with `library.place {item, at: [0, 0]}` and reads kind hdri back (the reviewer's call without `at` would be refused for another reason: furnish.rs:193 "`at` is required"). (2) check 06(a) runs every assertion in the interactive view, final raster and final path, with the black-pixel test and the magenta-backdrop variant (the path tracer's real assertion); (a2) checks the ground material (row 125 green like phase-10 check 08 and within 12 per channel of the site's own terrain on row 140, columns 180..319). (3) check 02 pins `sky: false` on every sun-measuring call in (a), (b) and (d) (direct and path illuminance, every render) and exposure {ev100: 15} + toneMap none on (b)'s renders, and says why; p19-sky item 4 names the pin. (4) FrameLook.sun_lux (p19-r3d-groundwork item 4); exposure_look sets it only while lighting.sun is Some; fill_exposure writes p.sun_intensity = f.sun_lux; p19-sky sets it from Lighting::directional_lux(); test offscreen_tests::night_lit_frame_has_no_sun_term. (5) render3d composite_misses returns early for Sky/Hdri (p19-foundation item 1, test backdrop::tests::sky_and_hdri_are_not_pictures); p19-env-path relies on it, keeps a fallback in backdrop_composite and adds tracer test visible_environment_shows_through_a_sky_backdrop. (6) normal images are named `<base>-normal` after the base images, and p19-normal-maps edits phase-13 checks 02 and 13_demo (both in its files) to compare only base-colour images, with comments. (7) p19-sun's frame_sun sets FRAME_SKY only with look.sky.on (test sky_off_leaves_only_the_sun); checks 01(b), 01(c) and 01(e) pin bounces 0 on their path renders. Code facts re-verified on op/next 15d9af8fa: tracer.rs:500-511 (backdrop_composite returns without a backdrop, else calls composite_misses), :166 and :763 (FRAME_SUN | FRAME_SKY set together), lit.rs:132-133, mesh.wgsl:121, render_hook.rs:283, furnish.rs:193, :247 and :260-265, ui-egui library_panel.rs:141-182, phase-13 02:21 and 13_demo:56. In flight (read-only look at the wave-0 worktrees at 12:46 on 11-08): p19-r3d-groundwork now has FrameLook.sun_lux (294c4e27d); p19-foundation's composite_misses returns early for Sky/Hdri (committed); p19-horizon's check 06 has (a2) as an uncommitted edit; p19-normal-maps' phase-13 edits (02, 13_demo) are made but not yet committed.

## Package specs

These are the FULL specs. Each package's `spec` in phase-19.json is a summary that points here; the section below is part of the package and wins over the summary wherever they differ in detail.

### `p19-foundation` (wave 0)

**Render looks in the drawing: Look types, the `look` command, effective looks reaching the renderer; phase-19 check lib and carry-forward checks**

- deps: none
- crates: cadcraft-doc, cadcraft-engine, cadcraft-render3d, cadcraft-io, cadcraft-arch3d, cadcraft-cli, cadcraft-ui-egui
- checks: tests/acceptance/phase-19/12_hostile.py, tests/acceptance/phase-19/13_layers_wasm_i18n.sh, tests/acceptance/phase-19/14_look.py
- files:
  - `crates/doc/src/arch/look.rs (new)`
  - `crates/doc/src/arch.rs`
  - `crates/doc/src/arch/backdrop.rs`
  - `xtask/src/literals.rs`
  - `crates/engine/src/cmd/arch/look.rs (new)`
  - `crates/engine/src/cmd/arch/mod.rs`
  - `crates/engine/src/cmd/arch/render.rs`
  - `crates/engine/src/cmd/arch/backdrop.rs`
  - `crates/engine/src/cmd/arch/inspect.rs`
  - `crates/render3d/src/look.rs (new)`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/lib.rs`
  - `crates/render3d/src/backdrop.rs`
  - `crates/render3d/src/final_render.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/arch3d/src/lib.rs`
  - `crates/io/src/dxf_furnishing.rs`
  - `crates/io/src/dxf_arch.rs`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `apps/cadcraft-cli/src/render.rs`
  - `crates/ui-egui/src/i18n/uk.tsv`
  - `crates/ui-egui/src/pane3d.rs (Lighting is no longer Copy: .clone() at its copy sites)`
  - `ATTRIBUTION.md (a row for assets/library/sample/hdri/README.md)`
  - `tests/acceptance/phase-19/p19lib.inc`
  - `tests/acceptance/phase-19/12_hostile.py`
  - `tests/acceptance/phase-19/13_layers_wasm_i18n.sh`
  - `tests/acceptance/phase-19/14_look.py`

Data and plumbing only; with the default look every image stays byte-identical; each field is honoured by the package named beside it.
FACTS (op/next a2887a143): the hook builds lighting from RenderRequest.sun (SunChoice) only (apps/cadcraft-cli/src/render_hook.rs private lighting_for); ArchModel is saved whole as JSON in the DXF XRECORD (io dxf_arch.rs v9), so serde-default fields round-trip; Camera3d (doc arch.rs) = name, kind, pose, light_set, hand-written Default; duplicate `camera {save}` is refused (keep); Backdrop = Colour|Gradient|Image{item,projection}, matched in arch3d lib.rs, engine backdrop.rs, io dxf_furnishing.rs, render3d backdrop.rs, final_render.rs, tracer.rs; render's parse_sun refuses a below-horizon sun; render.rs parse_path_params (run on EVERY render, render.rs:344-356) refuses any `exposure` that is not "auto" or a number, objects included; render3d Lighting (lit_frame.rs:44) derives Copy and is copied by value in ui-egui pane3d.rs (lighting() returns the field ~line 421, the paint callback ~796, the ViewState tuple ~1024) and final_render.rs:147 (`lighting: *lighting`). Engine backdrop.rs accepts only kind colour|gradient|image|none (KEYS :12; `allowed` :80-86 refuses anything else with "`kind` must be colour, gradient, image or none"), and its describe() matches Backdrop exhaustively. arch3d fill_materials (lib.rs:279-291) puts an image backdrop's pixels into scene.textures under `item:<id>` only for model.backdrop; the raster (render3d backdrop.rs image_of) and the path tracer (image_textures) find a picture only there and otherwise draw the clear colour; model.images is never pruned. `cargo xtask assets` (run by phase-14 check 12, phase-15 check 13 and this phase's 13) fails on op/next since 576538153: assets/library/sample/hdri/README.md has no ATTRIBUTION.md row (it is the only path the xtask lists).
1 DOC crates/doc/src/arch/look.rs (`pub mod look; pub use look::*;`; add it to SHARED_ROOTS in xtask/src/literals.rs). camelCase serde, structs #[serde(default)] with hand-written Default:
- `enum Technique { #[default] Standard, Path, Clay, Technical }` (lowercase; p19-cameras); `#[serde(tag="mode", rename_all="lowercase")] enum ExposureSetting { #[default] Auto, Manual { ev100: f64 } }`; `enum ToneMap { #[default] Aces, Hable, None }` (p19-exposure);
- `struct SunLook { on: bool = true, intensity: f64 = 100_000.0 /*lux facing the sun*/, shadows: bool = true, angle: Option<String> = None }` (p19-sun); `struct MoonLook { lux: f64 = 0.25, azimuth: f64 = 150.0 /*compass, 0 = true north, 90 = east*/, elevation: f64 = 35.0 }`, `struct SkyLook { on: bool = true, turbidity: f64 = 3.0, stars: bool = false, moon: MoonLook }` (p19-sky); `struct ClipBox { min: [f64;3], max: [f64;3] }` (p19-seeing-in);
- `struct Look { technique, exposure, tone_map, sun: SunLook, sky: SkyLook, backdrop: Option<Backdrop>, light_set: Option<String>, cutaway: bool, clip: Option<ClipBox>, near_clip: Option<f64> }` + `is_default()` + `validate() -> Result<(), String>` naming field and value: ev100 -10..=24, intensity 0..=200_000, turbidity 1.7..=10, moon lux 0..=10, moon elevation -90..=90, all finite, clip min < max per axis with |coord| <= 1e6, near_clip in (0, 1000] (export the limits as consts);
- `struct SunAngle { name: String, date: Option<String>, time: Option<String>, azimuth: Option<f64>, elevation: Option<f64> }`, `MAX_SUN_ANGLES = 64`, `validate()` (non-empty name; exactly one of date+time / azimuth+elevation; elevation -90..=90).
ArchModel gains `#[serde(default, skip_serializing_if="Look::is_default")] look: Look` (global; its backdrop stays None - the global backdrop remains ArchModel::backdrop) and `#[serde(default, skip_serializing_if="Vec::is_empty")] sun_angles: Vec<SunAngle>` (in Default; validate() resets an invalid look and drops bad/extra angles). Camera3d gains `#[serde(default, skip_serializing_if="Option::is_none")] look: Option<Look>` (None = pre-phase-19 camera, follows the global look); camera_is_finite also runs look.validate(). Backdrop gains `Sky` (unit, {"kind":"sky"}; the command form is item 2b below, its drawing p19-sky) and `Hdri { image: String, #[serde(default="one")] intensity: f64, #[serde(default)] rotation: f64, #[serde(default)] horizon: f64 }` (p19-hdri); Image gains `#[serde(default, skip_serializing_if="is_zero")] horizon: f64` (degrees). Picture code treats Sky/Hdri as no picture: the raster backdrop pass draws the clear colour for them (p19-env-raster later draws a visible environment there instead), and in the PATH TRACER render3d backdrop.rs `composite_misses` paints NOTHING for them (it returns early for Sky/Hdri, so tracer.rs backdrop_composite leaves the resolved pixels: today's miss radiance, and env_escape's visible environment once p19-env-path and p19-sky/p19-hdri land). Camera rays that read a visible environment still count as misses (kernel.wgsl trace_main counts only surface hits), so painting CLEAR_RGB over Sky/Hdri misses would hide the sky and the HDRI in every path render (checks 03(a), 04(c)/(d)/(f), the demo's dusk and noon-hdri cameras). Constructions get `horizon: 0.0`.
2 ENGINE crates/engine/src/cmd/arch/look.rs - the only look JSON reader/writer:
- `pub(crate) fn apply_look_params(cmd: &str, params: &Value, model: &ArchModel, look: &mut Look, render: bool) -> Result<Option<[f64;3]>, EngineError>`: keys technique, exposure ("auto" | {"ev100": n}; with render=true a bare number is skipped - it keeps its phase-12 meaning in parse_path_params), toneMap, sun, sky, lightSet (null | existing light set), cutaway, clip (null | {min,max}), nearClip (null | m). sun: false -> on=false (+ with render=true sky.on=false: legacy night); "default" -> nothing; object {on?, intensity?, shadows?, angle?: name|null (must exist)} plus, render only, ONE direction: date+time (needs a site; below the horizon allowed) or azimuth+elevation (compass, through arch::site::true_to_drawing; no site needed). sky: false | true | {on?, turbidity?, stars?, moon?: {lux?, azimuth?, elevation?}}. Anything invalid (unknown keys, types, direction+angle, ranges) -> refused, `look` untouched (edit a copy, validate, write back). Returns the explicit unit toward-sun drawing-space direction if given.
- `pub(crate) fn describe_look(&Look) -> Value` = {"technique","exposure": "auto"|{"ev100"},"toneMap","sun":{"on","intensity","shadows","angle"},"sky":{"on","turbidity","stars","moon":{"lux","azimuth","elevation"}},"backdrop","lightSet","cutaway","clip","nearClip"}.
- `pub(crate) fn sun_angle_dir(model, &SunAngle) -> Option<[f64;3]>` (date/time: arch::sun::sun_position(site.latitude,..) -> sun_dir -> true_to_drawing, None without site; az/el as above) and `effective_sun_dir(model, look, explicit) -> Option<[f64;3]>` = explicit, else look.sun.angle's, else None (= default light Light::DEFAULT, south-west 45 deg).
- `pub(crate) fn effective_look(model, camera: Option<&Camera3d>) -> Look`: camera.look if Some; else model.look with backdrop = model.backdrop and, for a legacy camera, light_set = camera.light_set.
- command `look` (`CommandSpec::new("look", "Render Look", run_look).menu(&["Architecture","Render Look"])`, undoable): {} reports {"look": describe(global + backdrop)}; else apply_look_params(render=false) to model.look, touch_arch, same reply. Direction forms, `backdrop`, bare-number exposure refused here.
2b ENGINE crates/engine/src/cmd/arch/backdrop.rs, the `sky` command form (no other package adds it; p19-hdri adds its `hdri` arm beside it in wave 3): `"sky" => &["kind"]` in `allowed`, `"sky" => Some(Backdrop::Sky)` in `new`, reply and describe() `{"kind": "sky"}`; describe() also gets the Hdri arm (all its fields; the command form is p19-hdri's), the refusal becomes "`kind` must be colour, gradient, image, sky or none" and the params doc `kind?: colour|gradient|image|sky|none`. `{kind: "sky", colour: "#000000"}` is refused ("`colour` does not apply to a sky backdrop") and changes nothing; `backdrop {}` reports {"kind": "sky"}. Until p19-sky lands a Sky backdrop draws as no picture: the clear colour in the raster views; the path tracer keeps its own miss radiance (item 1). This form is what check 03 (`backdrop {kind: sky}`), check 12, p19-sky's `visible` rule and the demo's "dusk" camera use.
3 RENDER: RenderRequest gains `pub look: Look`, `pub sun_dir: Option<[f64;3]>` (new() unchanged, defaults; builder `with_look(look, sun_dir)`) and `pub fn resolved_sun(&self) -> (Look, Option<[f64;3]>)` (sun_dir None + sun Dir(d) -> Some(d); sun Off -> sun.on = sky.on = false) so sunstudy/tests keep their meaning. run_render: effective_look(model, camera by name or the active camera) -> apply_look_params(render=true) -> effective_sun_dir; light_set = explicit lightSet param else look.light_set; legacy SunChoice derived (Off / Dir above horizon / Default); remove render's below-horizon refusal (illuminance keeps parse_sun until p19-sun); parse_path_params treats `exposure` "auto" or an object as 'no legacy number' and leaves its validation to apply_look_params (only a bare number keeps its phase-12 meaning), so `render {exposure: {ev100: 3}}` parses and echoes manual 3; RenderRequest gains `pub legacy_night: bool` = the render's own `sun` param is literally `false` (RenderRequest::new with SunChoice::Off sets it too, for sunstudy and older callers): p19-exposure keeps the 0.01 night pin only for it; after the hook returns add reply "look": describe(effective) and "sunDir": [x,y,z]|null; update the params doc.
4 RENDER3D: Lighting gains `pub look: Look`, `pub sun_dir: Option<[f64;3]>` and DROPS `Copy` (derive Clone, Debug, PartialEq: Look holds Strings and Option<Backdrop>, later an Arc<Environment>); fix every copy site with `.clone()`: final_render.rs:147, ui-egui pane3d.rs (lighting() returns `self.lighting.clone()`, the callback ~796 and the ViewState tuple ~1024 clone), render_hook.rs and apps/cadcraft-cli/src/render.rs where needed; run `cb test -p cadcraft-ui-egui` (p19-pane later rewrites those pane lines). New render3d/src/look.rs `pub fn lighting_for(look: &Look, sun_dir: Option<[f64;3]>, budget: usize) -> Lighting`: sun None if !look.sun.on or dir.z <= 0; Light::DEFAULT if no dir; else Light::from_sun(dir, Light::DEFAULT.ambient); sun_shadow = sun.is_some() && dir given (p19-sun adds shadows flag); exposure DEFAULT_EXPOSURE; look/sun_dir copied. Default look = exactly the old hook lighting (unit test). Hook: delete its lighting_for; render_one/illuminance_one use resolved_sun() + lighting_for; before each render set the cached scene's backdrop to the effective look's with `cadcraft_arch3d::set_backdrop(&model, &mut scene, look.backdrop.as_ref())` (no rebuild). New in arch3d lib.rs: `pub fn set_backdrop(model: &ArchModel, scene: &mut Scene3d, backdrop: Option<&Backdrop>)` sets scene.backdrop and, for an Image backdrop, inserts its pixels under `item:<id>` from model.images when valid (other textures stay); fill_materials calls it with model.backdrop, so full and incremental builds stay equal. Without it a camera whose look holds a picture other than the global backdrop finds no `item:<id>` and draws the clear colour (p19-pane uses it for the pane scene too). `cadcraft-cli render FILE` uses the drawing's look.
5 INSPECT: {"look": true} -> "look"; {"sunAngles": true} -> [{"name","date","time","azimuth","elevation","dir"|null}]. uk.tsv: "Render Look".
6 p19lib.inc: exec the first existing of phase-18/17/16/15 libs, then: NOON = {"date":"2026-06-21","time":"solar-noon"} (elevation 62.39 at lat 51.05); SITE = site {latitude 51.05, longitude -114.07, trueNorth 0}; HOUSE1 is NOT redefined: it stays p14lib.inc:48's 8 x 5 house (ring w0..w5, 0.2 m thick, 2.4 m high, partition w6 at x = 4; room A x 0.1..3.95, room B x 4.05..7.9, y 0.1..4.9; roomdetect), which checks 07, 08, 09, 10 and 15 all build on (no phase 15-18 lib or plan redefines it either; the demo writes its own 10 x 8 phase-1 house in its commands file); render_replies, look_of, srgb8_to_linear, linear_to_srgb8, ev_scale(ev)=1/(1.2*2**ev), aces(x)=clamp(x(2.51x+.03)/(x(2.43x+.59)+.14)), hable(x)=clamp(f(2x)/f(11.2)) with f(x)=(x(.15x+.05)+.004)/(x(.15x+.5)+.06)-.02/.3, write_rgbe(path,w,h,fn,rle=False) ("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y h +X w\n"; 1.0 = (128,128,128,129)), level_cam(eye,yaw,pitch,fov), CLEAR=(39,44,52); a header comment listing phase 19's command forms.
7 CHECKS: 12_hostile.py - each refused with an error line, no panic, and arch.inspect {look, sunAngles, cameras, lights} + `backdrop {}` unchanged after (set-up: ROOM, light set "Day", a camera, a sun angle). look: exposure 5 | {ev100:"1"} | {ev100:99}; toneMap "filmic"; sun {intensity:-1} | {intensity:1e9} | {angle:"Nope"} | {date,time}; sky {turbidity:50} | {moon:{lux:-0.1}}; clip {min:[0,0,0],max:[0,1,1]} | {min:[0,0],max:[1,1,1]}; nearClip 0; technique "watercolor"; cutaway "yes"; lightSet "Nope"; {bogus:1}. sunangle: {name:"",azimuth:0,elevation:10}; {name:"a",azimuth:0,elevation:91}; {name:"b",date:"2026-13-01",time:"12:00"}; {name:"c",azimuth:0}; date+time with azimuth+elevation; {delete:"Nope"}. render: exposure {ev100:null}; toneMap 3; albedo 1.5; sun {azimuth:0,elevation:120}; sun {angle:"x",azimuth:0,elevation:10}. backdrop: {kind:"hdri"}; file "/nonexistent.hdr"; a text file; a valid 4x2 RGBE with intensity -1 / horizon 90; {kind:"image",item:"own:backdrop-sky",projection:"flat",horizon:5}; {kind:"sky",colour:"#000000"}; {kind:"image",item:"polyhaven:kloofendal-puresky"} (an HDR item is not a picture). lightfixture LED strip: path [[0,0]]; lumensPerMetre -5; facing "up"; under ["w0"]; room "r0" inset -1; tray "tr9". camera {save:"x",kind:"overview",level:9}; camera.reset {name:"Nope"}; camera.orbit {azimuth:"10"}; camera.update {name:"Nope"}; pick3d with clip min > max; pick3d camera "Nope"; illuminance sun {intensity:"x"}. needs: p19-foundation, -exposure, -sun, -hdri, -strip-model, -strip-tray, -seeing-in, -cameras.
13_layers_wasm_i18n.sh as phase-15's 13 (its `cargo xtask assets` passes once item 8's row is in), plus look.rs in SHARED_ROOTS, uk rows "Render Look","Sun Angle","Reset Camera","Orbit Camera","Update Camera", USER-GUIDE "## Light, sky and cameras" (needs p19-demo).
14_look.py: `look {}` reports the defaults exactly; `look {exposure:{ev100:3}, toneMap:"hable", cutaway:true}` shows in arch.inspect, survives --save + reopen, a later refused `look` changes nothing, undo restores; a render without look params echoes that look; `render {toneMap:"none"}` affects that render only; `render {sun:false}` echoes sun.on and sky.on false; with SITE `render {sun:{azimuth:90, elevation:30}}` -> sunDir [cos30, 0, sin30] (1e-6), trueNorth 30 turns it 30 deg about +Z; `render {sun:{date:"2026-06-21", time:"02:00"}}` is accepted with sunDir z < 0; `backdrop {kind: "sky"}` is accepted, reported by `backdrop {}` and echoed in a render's look.
8 ATTRIBUTION.md: right after the .hdr row (inside the library-sample markers) add the table row whose cells are: the path `assets/library/sample/hdri/README.md` in backticks, "CADCraft contributors", "written for the sample set", "MIT OR Apache-2.0", "the sample HDRI's provenance note" (the form of the catalog.json row). Run `cb x cargo xtask assets` (as check 13 runs it) and see it pass; this also turns phase-14 check 12 and phase-15 check 13 green again. p19-hdri's curate.py keeps the row.

Unit tests:
- `doc look::tests::defaults_match_the_brief`
- `doc look::tests::serde_round_trip_and_old_payloads_load`
- `doc look::tests::validate_refuses_out_of_range`
- `doc look::tests::sun_angle_needs_one_form`
- `doc backdrop::tests::sky_hdri_and_image_horizon_serde`
- `engine look::tests::look_command_report_set_undo_refuse`
- `engine look::tests::apply_look_params_sun_forms`
- `engine look::tests::effective_look_prefers_the_camera`
- `engine render::tests::look_overrides_one_render_only`
- `engine render::tests::legacy_sun_false_is_night_and_below_horizon_is_accepted`
- `engine render::tests::resolved_sun_keeps_legacy_choices`
- `engine render::tests::exposure_object_passes_path_params_and_echoes_manual`
- `engine backdrop::tests::backdrop_sky_sets_reports_and_refuses_extra_keys`
- `arch3d tests::set_backdrop_loads_the_image_a_look_names`
- `render3d backdrop::tests::sky_and_hdri_are_not_pictures` (backdrop_rgb gives CLEAR_RGB, image_textures is empty, composite_misses leaves a Sky or Hdri miss pixel unchanged; a raster shot with Sky equals one without a backdrop)
- `render3d look::tests::lighting_for_default_matches_old_hook`
- `io dxf_arch::tests::look_and_sun_angles_round_trip_and_bad_look_loads_default`

### `p19-r3d-groundwork` (wave 0)

**GPU groundwork: reserved uniform fields, shader hook functions and per-frame look plumbing in render3d, no visible change**

- deps: none
- crates: cadcraft-render3d
- checks: none
- files:
  - `crates/render3d/src/pipeline.rs`
  - `crates/render3d/src/shaders/mesh.wgsl`
  - `crates/render3d/src/shaders/lit.wgsl`
  - `crates/render3d/src/lit.rs`
  - `crates/render3d/src/lights.rs`
  - `crates/render3d/src/shadow.rs`
  - `crates/render3d/src/offscreen.rs`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/frame_look.rs (new)`
  - `crates/render3d/src/lib.rs`
  - `crates/render3d/src/pathtrace/scene_gpu.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/render3d/src/shaders/pt/common.wgsl`
  - `crates/render3d/src/shaders/pt/kernel.wgsl`
  - `crates/render3d/src/shaders/pt/lights.wgsl`
  - `crates/render3d/src/shaders/pt/traverse.wgsl`
  - `crates/render3d/src/shaders/pt/traverse_rq.wgsl`
  - `crates/render3d/src/shaders/pt/resolve.wgsl`

Pure refactor (every existing test and check renders byte-identical) so later packages edit only their own functions, never a layout. No doc types needed.
FACTS: PbrUniform (96 B) pads _p0:[u32;3] (WGSL 36..44), _p1:[f32;3] (68..76); uniform_stride() rounds only UNIFORM_SIZE yet strides PBR too; FrameU 96 B; lit.rs/shadow.rs rename fs to fs_base by string surgery; the PT uses all 8 storage buffers (PT_STORAGE_BUFFERS) and rq binds @group(1): NO new PT buffers/bind groups - per-still data goes in the `ies` f32 "aux buffer" at frame offsets; tracer.rs sets sun/sky twice; ResolveParams' `spp` is read as `tonemap` by resolve.wgsl.
1 PbrUniform: _p0 -> `tone_map: u32, normal_layer: u32, flags: u32`; _p1 -> `exposure: f32, ambient_lux: f32, normal_strength: f32`; append `clip_min: [f32;3], lambert_albedo: f32, clip_max: [f32;3], _p7: f32` (lambert_albedo = the calibration albedo p19-exposure writes with PBR_FLAG_LAMBERT); PBR_SIZE 128; consts PBR_FLAG_LAMBERT = 1, PBR_FLAG_CLIP = 2; uniform_stride rounds max(UNIFORM_SIZE, PBR_SIZE); WGSL mirrors at 36,40,44,68,72,76, _p6 at 80, clip_min 96, lambert_albedo 108, clip_max 112, _p7 124; layouts updated. Zero = phase-18 behaviour.
2 FrameU: append `ext: vec4<f32>` (x ambient lux, y = 1 when sh valid) and `sh: array<vec4<f32>,9>`; FRAME_UNIFORM_SIZE 256; LitParams gains `ambient_lux: f32`, `sh: Option<[[f32;3];9]>` written by frame_uniform (zeros by default).
3 mesh.wgsl split, same maths: `struct Shade { direct: vec3<f32>, albedo: vec3<f32>, metallic: f32, n: vec3<f32>, alpha: f32, keep: u32 }` (keep = display colour: hidden-line, technical); stubs `fn clip_discard(wpos: vec3<f32>) -> bool {false}` [p19-seeing-in-render], `fn shading_normal(n: vec3<f32>, uv: vec2<f32>, wpos: vec3<f32>) -> vec3<f32> {n}` [p19-normal-maps-render], `fn sun_irradiance() -> f32 { p.sun_intensity*(1-p.ambient)*PI }`, `fn base_ambient(albedo: vec3<f32>, metallic: f32, n: vec3<f32>) -> vec3<f32> { albedo*p.ambient*(1-metallic*0.5) }`, `fn finish_color(rgb: vec3<f32>) -> vec3<f32> {rgb}` [p19-exposure]; `fn shade_parts(v: VOut) -> Shade` (today's fs body; first `if (clip_discard(v.wpos)) { discard; }`; normal through shading_normal; direct = (diffuse+spec)*sun_irradiance()*ndl); fs = keep ? as is : finish_color(direct + base_ambient). lit.wgsl fs: shade_parts; sun-shadowed -> direct 0; `rgb = direct + lit_ambient(albedo, metallic, n)` with stub `fn lit_ambient(..) -> vec3<f32> { base_ambient(..) }` [p19-env-raster]; fixtures `rgb += diffuse_albedo/PI * fixture_light(i, wpos, n_l) * fixture_scale()` with `fn fixture_light(i: u32, wpos: vec3<f32>, n: vec3<f32>) -> vec3<f32>` = today's cone*ndl/d2*shadow in lux [p19-strip-render] and `fn fixture_scale() -> f32 { fr.params.x }` [p19-exposure]; the params.y ACES branch kept; return finish_color. shadow.rs LOOKUP_WGSL uses the same helpers. Update the source-composition unit tests.
4 render3d/src/frame_look.rs: `#[derive(Clone, Debug, Default, PartialEq)] pub struct FrameLook { pub exposure: f32, pub tone_map: u32, pub ambient_lux: f32, pub flags: u32, pub clip: Option<[[f32;3];2]>, pub near: Option<f32>, pub lambert: Option<f32>, pub sun_lux: f32 }` (sun_lux = the lux of the raster's directional light for the sun term, 0 = none: at night the sun term must stay 0, [p19-exposure], moon [p19-sky]); `#[derive(Default)] pub struct Meter {}` [p19-exposure]; `pub fn frame_look(scene: &Scene3d, camera: &Camera, size: (u32,u32), lighting: &Lighting, ground: bool, meter: &mut Meter) -> FrameLook` (default, then exposure_look, then view_look); stubs `pub fn exposure_look(f: &mut FrameLook, scene, camera, size, lighting, ground: bool, meter: &mut Meter)` [p19-exposure] (`ground` = RenderOptions.ground: the raster ground quad exists only at upload, so the meter must be told), `pub fn view_look(f: &mut FrameLook, lighting: &Lighting)` [p19-seeing-in-render]; `pub fn fill_draw(p: &mut PbrUniform, f: &FrameLook, scene: &Scene3d, material: &str)` calling stubs fill_exposure(p,f) [p19-exposure], fill_normal_map(p,f,scene,material) [p19-normal-maps-render], fill_clip(p,f) [p19-seeing-in-render]. Offscreen gains `meter: Meter`, `last_look: FrameLook`, `pub fn last_frame_look(&self) -> &FrameLook`; LitFrame gains `pub lighting: Lighting` (clone set in lit_frame); encode_with computes fl once per frame from opts.lit and opts.ground (default when unlit), stores it, and calls fill_draw per draw (ground quad material "ground").
5 GpuFrame appends in order (WGSL Frame mirrors): env:[u32;4] (offset, width, height, flags 1 present / 2 visible), env_aux:[u32;4] (marginal CDF offset, conditional CDF offset), env_params:[f32;4] (x total weight), moon_dir:[f32;4], moon_rgb:[f32;4] (lux), clip_min:[f32;4], clip_max:[f32;4], view:[f32;4] (xyz forward, w near m), tex:[u32;4] (texel offset, map edge, map count), look:[u32;4] (x tone code). Flags (Rust+WGSL) FRAME_ENV 16, FRAME_MOON 32, FRAME_CLIP 64, FRAME_SUN_NO_SHADOW 128, FRAME_NORMAL_MAPS 256, FRAME_LAMBERT 512.
6 tracer.rs: `fn frame_lighting(frame: &mut GpuFrame, aux: &mut Vec<f32>, lighting: &Lighting, camera: Option<&Camera>, opts: &PathOptions)` used by new() and irradiance() (furnace path unchanged), calling frame_sun (today's sun+sky code), and no-op stubs frame_environment(frame, aux, lighting) [p19-env-path], frame_moon(frame, lighting) [p19-sky], frame_view(frame, lighting, camera) [p19-seeing-in-render], frame_look(frame, lighting, opts) [p19-exposure]. aux = GpuScene::ies, extended before upload_scene; IES tables stay first.
7 PT WGSL stubs with today's behaviour: kernel `sun_visibility(o, dir) -> f32 { visibility(o, dir, 1e6) }` [p19-sun] used in direct_light and irradiance_main; `env_direct(m: Material, p, n, wo, r: ptr<function,Rng>) -> vec3<f32>` = 0, `env_escape(d, depth: u32, mode: u32, prev_pdf: f32) -> vec3<f32> { sky_radiance(d) }` in the miss branch, `env_irradiance(o, n, r) -> vec3<f32>` = 0 [p19-env-path]; `moon_direct(m, p, n, wo)`, `moon_irradiance(o, n)` = 0 [p19-sky]; `pt_shading_normal(tri: u32, n, u: f32, v: f32) -> vec3<f32> {n}` on the hit normal [p19-normal-maps-render]; `hit_clipped(p) -> bool {false}` checked per candidate in traverse.wgsl and traverse_rq.wgsl, `camera_t_min(d) -> f32 {0}` for camera rays [p19-seeing-in-render]. rt_kernel_source_replaces_only_the_traversal must still hold.
8 ResolveParams: `spp` -> `tone_map: u32` = 1; resolve.wgsl `tone_map == 1u` -> ACES, else no curve (p19-exposure adds 2 Hable, 3 none).
TESTS: layouts pinned (offset_of!), stride at 64-byte alignment; existing tests unchanged; run phases 10/11/14/15 render checks.

Unit tests:
- `render3d pipeline::tests::pbr_uniform_layout_128_named_fields`
- `render3d pipeline::tests::stride_covers_the_pbr_block`
- `render3d lit::tests::frame_uniform_256_and_zero_by_default`
- `render3d pathtrace::scene_gpu::tests::gpu_frame_layout_matches_wgsl`
- `render3d frame_look::tests::default_frame_look_changes_no_draw`
- `render3d pathtrace::tracer::tests::frame_lighting_matches_old_setup`

### `p19-exposure` (wave 1)

**Exposure: auto or manual EV100 with ACES/Hable/none tone mapping, the same in the pane, the raster Finalize and the path tracer; a physically scaled raster**

- deps: p19-foundation, p19-r3d-groundwork
- crates: cadcraft-render3d, cadcraft-engine, cadcraft-cli
- checks: tests/acceptance/phase-19/01_exposure.py
- files:
  - `crates/render3d/src/exposure.rs (new)`
  - `crates/render3d/src/frame_look.rs`
  - `crates/render3d/src/shaders/mesh.wgsl`
  - `crates/render3d/src/shaders/lit.wgsl`
  - `crates/render3d/src/shaders/tonemap.wgsl (new)`
  - `crates/render3d/src/lit.rs`
  - `crates/render3d/src/offscreen.rs`
  - `crates/render3d/src/final_render.rs`
  - `crates/render3d/src/lib.rs`
  - `crates/render3d/src/pathtrace/mod.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/render3d/src/pathtrace/scene_gpu.rs`
  - `crates/render3d/src/shaders/pt/resolve.wgsl`
  - `crates/engine/src/cmd/arch/render.rs`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `crates/render3d/src/lit_frame.rs (Lighting.lambert, Lighting.meter_night)`
  - `apps/cadcraft-cli/src/render.rs (--night = the legacy night)`
  - `tests/acceptance/phase-19/01_exposure.py`

BRIEF: Exposure bullet + numbers line "exposure" (no aperture/shutter/ISO).
FACTS (code wins where noted): constants DEFAULT_EXPOSURE 0.01/lux (EV100 6.3808) and DAY_EXPOSURE pi/1e5 (EV100 14.695); PT auto = clamp(0.18/Lavg over hit pixels, DAY, NIGHT) with the sun on, 0.01 at night; phase-12 check 14 pins that (night 0.01 auto or not, byte-identical; bare-number `exposure` used as given; reply `exposure` = that scale) -> keep the daylight clamp and the reply key; keep 0.01 only for the legacy `render {sun: false}` form (and hand-built night Lightings); meter every other frame without daylight, as the brief replaces the fixed constant; add keys. The raster is not physical (white face at ndl 1 = 1.0; ambient 0.25*albedo, 0.02 at night, no exposure; fixtures (albedo/pi)*lux*0.01; only Finalize ACES) -> make the LIT raster physical. No surface has a known Lambert albedo (ground quad 0.75 + GGX, paints F0 0.04) -> add a calibration param `albedo`. The raster albedo is `u.albedo` x the texture layer (mesh.wgsl base_albedo, `_p6.y`): the ground quad (0.75), terrain ([0.33,0.45,0.22], maybe textured) and paints keep their own unless the shader overrides it. The raster's night is dark ONLY because LitFrame.sun_intensity is 0 without a sun (lit.rs:132-133 `if params.sun_on {..} else {(0.0, NIGHT_AMBIENT..)}`): render_hook.rs:283 still passes `lighting.sun.unwrap_or(Light::DEFAULT)` as the light and mesh.wgsl's sun term (:121 `p.sun_intensity*(1-p.ambient)*PI*ndl`) has no sun-on gate, so whatever writes PbrUniform.sun_intensity must write 0 for a frame without a sun. The raster ground quad exists only at upload (offscreen.rs ensure_meshes pushes shadow::ground_mesh(&scene.bounds) onto a clone), never in Scene3d.meshes; the PT's ground is GpuScene triangles (scene_gpu.rs collect_triangles).
1 render3d/src/exposure.rs: `ev100_scale(ev: f64) -> f32` = 1/(1.2*2^ev); `scale_ev100(s: f32) -> f64`; `TONE_ACES 1, TONE_HABLE 2, TONE_NONE 3`, `tone_code(ToneMap) -> u32`, `tone_map(x: f32, code: u32) -> f32` (ACES as lit.wgsl today; Hable clamp(f(2x)/f(11.2)), f(x)=(x(.15x+.05)+.004)/(x(.15x+.5)+.06)-.02/.3; none clamp 0..1); move auto_exposure_from_luminances here (re-export); `auto_scale(lavg: Option<f32>, daylight: bool, meter_night: bool) -> f32` (daylight: clamp(0.18/Lavg, DAY, NIGHT), no hits DAY; no daylight and !meter_night (the legacy `sun: false` form, hand-built night Lightings): exactly NIGHT; no daylight and meter_night: clamp(0.18/Lavg, DAY, NIGHT_AUTO_MAX) with `NIGHT_AUTO_MAX = ev100_scale(-6.0)` = 53.333 (EV100 -6, the pane slider's bottom), no hits NIGHT); `daylight(&Lighting) -> bool` = sun.is_some() (p19-env-path adds daylight environments); `exposure_scale(look, legacy: Option<f64>, lavg: impl FnOnce()->Option<f32>, daylight, meter_night) -> (f32, &'static str)`: legacy number -> "fixed", Manual -> "manual", Auto -> "auto" (meter run only then). WGSL tone curves in shaders/tonemap.wgsl, concatenated where needed (mesh compositions, resolve, later the backdrop pass).
2 Raster: Lighting gains `pub lambert: Option<f32>` (the calibration albedo; None) and `pub meter_night: bool` (false = a frame without daylight keeps 0.01 as before); the hook sets lambert = RenderRequest.albedo and meter_night = !RenderRequest.legacy_night, `cadcraft-cli render FILE` sets meter_night = !--night, the pane (p19-pane) always true. exposure_look sets tone_map, exposure (scale), ambient_lux, lambert (= lighting.lambert) and sun_lux = look.sun.intensity ONLY when lighting.sun is Some (the sun on and above the horizon), else 0 (p19-sky later sets it from Lighting::directional_lux() so the moon can take its place); if FrameLook has no `sun_lux` on op/next when you start (p19-r3d-groundwork item 4 declares it), add it (Default 0). Auto meter: 32x24 rays through pixel centres (arch3d::camera::ray_at over size) against an arch3d::pick::Bvh over the scene's meshes PLUS `shadow::ground_mesh(&scene.bounds)` when `ground` (and `scene.horizon` once p19-horizon's field is on op/next), cached by (scene key, ground) (pass the key in, do not key by pointer), housings excluded; lum(albedo) = lighting.lambert when Some, else the hit material's; per hit L = lum(albedo)/pi*(E_sun*max(0,n.s)*vis + E_amb) with E_sun = f.sun_lux (0 at night) + sum direct_illuminance(fixtures, no shadow); vis = shadow ray when sun_shadow; misses skipped; Lavg = exp(mean ln(1e-4+L)). fill_exposure writes p.tone_map, p.exposure, p.ambient_lux, p.lambert_albedo + PBR_FLAG_LAMBERT when f.lambert is Some, and p.sun_intensity = f.sun_lux when tone_map != 0 (never look.sun.intensity directly: every lit frame, night ones included, is tone-mapped (ACES default), so that would give `render {sun: false}`, `sun {on: false}` and every phase-10 night a 100 000 lx sun from Light::DEFAULT's south-west, and 01(e)'s raster exposure 0.9 could not come out). Shader hooks when p.tone_map != 0: sun_irradiance() = p.sun_intensity; base_ambient = albedo/PI*p.ambient_lux*(1-metallic*0.5); LAMBERT -> base_albedo returns vec3(p.lambert_albedo) (it still samples the texture, for uniform control flow, then ignores it and u.albedo, so the ground quad, terrain and painted surfaces all read the calibration albedo), metallic 0, no specular, kd 1; fixture_scale() = 1; finish_color = tone_map(rgb*p.exposure) per channel; keep fragments (hidden-line, technical) untouched; tone_map == 0 (unlit legacy, e.g. render_png in phase-1/3 checks) unchanged; the lit Finalize-only ACES branch only when tone_map == 0, so interactive and raster Finalize tone-map alike. ambient_lux: night (no sun, no environment) = 0.02*pi/0.01 = 6.2832 lx (keeps phase-10 nights at scale 0.01); sun on + look.sky.on = 0.25*intensity (stand-in until p19-env-raster/p19-sky); sky off 0. Picture backdrops and edge lines stay unexposed (phase-14 check 09).
3 PT: PathOptions.exposure from the look (Manual -> Fixed(ev100_scale), Auto -> auto_scale over the hit pixels with lighting.meter_night (false = today's rule, 0.01 at night), legacy number -> Fixed); PathOptions gains `tone_map: u32` (default ACES; resolve.wgsl 1/2/3; furnace too) and `lambert: Option<f32>` applied by new `pub fn apply_lambert(scene: &mut GpuScene, albedo: f32)` AFTER build_gpu_scene (albedo a, metallic 0, roughness 1, ior 1 = pure Lambert per bsdf.wgsl, no transmission, emission only on light- lenses; do NOT change build_gpu_scene: p19-horizon and p19-normal-maps-render edit it); frame_look stub sets FRAME_LAMBERT and look.x.
4 Engine/hook: render param `albedo` (0<a<1 finite else "`albedo` must be a number with 0 < albedo < 1") -> RenderRequest `albedo: Option<f64>` (`with_albedo`), echoed; every reply (raster via off.last_frame_look(), path) has "exposure" (scale), "ev100", "exposureMode" ("auto"|"manual"|"fixed"), "toneMap"; legacy number exposure applies to raster too.
5 Keep every older render check green without editing it (grep -lE "'render'|render \{|-- render" tests/acceptance/phase-*/); fix code first; only a check encoding the old untone-mapped view may change, with a comment; report each.
CHECK 01_exposure.py (expected values computed with p19lib helpers): (a) phase-11 furnace (ROOM, its camera, 12x9, spp 4096, denoise false, furnace {albedo .5, emission 1}, toneMap none): ev100 1 -> all pixels 0.8333 (sRGB 235) +-2 %, ev100 2 -> 0.4167 (173) +-2 %, mode manual. (b) COLUMN on ground:true, albedo 0.5, sun {azimuth 0, elevation 90} (straight down: shadow-map basis must fall back like FullCamera::view), sky false, camera down at ground 1.5 m from the column: L = 15 915.5 nits; ev100 14 -> 0.80950 +-2 %, 15 -> 0.40475, aces(0.80950)=0.7553 and hable(0.80950) at ev100 14 +-2 %; each in the interactive view, final "raster" and final "path" (bounces 0, spp 16). (c) auto: frame filled by sunlit ground, sun {azimuth 0, elevation 90} (straight down: the scales below hold only then), sky false, none, albedo 0.05 then 0.1 -> ground 0.18 +-3 % raster and path (final path pinned at bounces 0, spp 16, as in (b): the scales assume the sun alone, and at the default bounces an escaping bounce ray would read whatever sky the path tracer still has), mode auto, scales 1.131e-4 / 5.655e-5 +-3 %. (d) the legacy `sun:false` (with and without exposure "auto") -> exposure 0.01, ev100 6.3808 +-0.001, raster and path. (e) a non-legacy night: `sun {on: false}`, sky false, ground:true filling the frame, albedo 0.1, auto, none: raster ground 0.18 +-3 % (the raster's night stand-in ambient 6.2832 lx lights it: exposure 0.18/(0.1/pi*6.2832) = 0.9 +-3 %), mode auto; path (pinned at bounces 0, spp 16): nothing lights the ground, so auto stops at the night limit: exposure ev100_scale(-6) = 53.333 +-0.1 %, ev100 -6 +-0.01 (not 0.01). Check 03(g) (p19-sky) adds the moonlit night in both renderers.

Unit tests:
- `render3d exposure::tests::ev100_scale_and_inverse`
- `render3d exposure::tests::tone_curves`
- `render3d exposure::tests::auto_scale_clamps_legacy_night_and_meters_other_nights`
- `render3d frame_look::tests::meter_of_a_uniform_plane_is_its_luminance`
- `render3d frame_look::tests::meter_sees_the_ground_quad`
- `render3d offscreen_tests::lambert_albedo_overrides_ground_quad_and_texture` (the ground quad with albedo 0.5 reads 0.5/pi*E*scale)
- `render3d offscreen_tests::raster_lambert_plane_reads_ev_scaled_luminance`
- `render3d offscreen_tests::technical_and_hidden_line_not_exposed`
- `render3d offscreen_tests::night_lit_frame_has_no_sun_term` (a tone-mapped lit frame with Lighting.sun None and an upward plane, which Light::DEFAULT would light: the plane reads only the night ambient, albedo/pi*6.2832*scale, and last_frame_look().sun_lux is 0)
- `render3d final_render::tests::finalize_matches_interactive_on_a_plane`
- `render3d pathtrace::tracer::tests::resolve_tone_maps_by_code`
- `render3d pathtrace::scene_gpu::tests::apply_lambert_makes_every_material_diffuse`
- `engine render::tests::albedo_param_parsed_and_refused`
- `cli render_hook::tests::replies_report_exposure_mode_ev100_tone_map`

### `p19-sun` (wave 1)

**The sun: intensity in lux, cast shadows on or off, sun off, and named sun angles stored in the drawing (`sunangle`)**

- deps: p19-foundation, p19-r3d-groundwork
- crates: cadcraft-engine, cadcraft-arch3d, cadcraft-render3d, cadcraft-cli, cadcraft-ui-egui
- checks: tests/acceptance/phase-19/02_sun.py
- files:
  - `crates/engine/src/cmd/arch/sunangle.rs (new)`
  - `crates/engine/src/cmd/arch/mod.rs`
  - `crates/engine/src/cmd/arch/illuminance.rs`
  - `crates/engine/src/cmd/arch/render.rs (IlluminanceRequest)`
  - `crates/engine/src/cmd/arch/inspect.rs`
  - `crates/arch3d/src/light.rs`
  - `crates/render3d/src/look.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/render3d/src/shaders/pt/kernel.wgsl`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `crates/ui-egui/src/i18n/uk.tsv`
  - `tests/acceptance/phase-19/02_sun.py`

BRIEF: The sun bullet + numbers line "sun" (88 612 lx = 100 000*sin 62.39 +-1 %, sky off, direct only; shadows off: the phase-10 column no longer darkens the floor behind it).
FACTS: pathtrace::SUN_LUX = 100 000 is the PT sun already (phase-11 07 measures 88 600 lx via path illuminance); tracer.rs sets FRAME_SUN | FRAME_SKY together whenever lighting.sun is Some (:166 in new(), :763 in irradiance()), so today a sun always brings the constant 2 500 (zenith) - 5 000 (horizon) nit gradient sky (lights.wgsl sky_radiance) on every escaping ray; the direct `illuminance` (engine illuminance.rs -> arch3d light::illuminance) has fixtures only; its `sun` param exists for method path only, absent = night. Look sun fields, sun_angles, sun_angle_dir, effective_sun_dir and the sun param forms come from p19-foundation; the raster's lux sun is p19-exposure's (do not touch raster shaders).
1 command `sunangle` (sunangle.rs, `CommandSpec::new("sunangle","Sun Angle",..).menu(&["Architecture","Sun Angle"])`, undoable): {name, date, time} or {name, azimuth, elevation} adds (azimuth compass via the site's trueNorth; elevation -90..=90; date/time needs a site; SunAngle::validate; duplicates and > MAX_SUN_ANGLES refused); `replace: true` replaces; {delete: name} refused while the global look or a camera look uses it ("sun angle `x` is used by camera `y`"); {list: true}. Rows {"name","date","time","azimuth","elevation","dir"|null,"sunElevation"|null,"sunAzimuth"|null}; arch.inspect sunAngles rows gain the resolved keys. uk.tsv "Sun Angle".
2 shadows/on: render3d lighting_for: sun_shadow &&= look.sun.shadows. tracer.rs frame_sun: sun_rgb = look.sun.intensity (replacing SUN_LUX there), FRAME_SUN only when on and above the horizon, FRAME_SUN_NO_SHADOW when !shadows, and FRAME_SKY (today's gradient) only when look.sky.on as well, so `sky: false` means no sky in the path tracer too (with it kept, every escaping bounce ray reads the gradient: E = 2 pi int_0^1 (5000 - 2500 sqrt(mu)) mu dmu = 9 425 lx on open ground, so path illuminance at bounces > 0 and 01(c)'s path scales come out about 9 % off); frame_environment (p19-env-path) still clears FRAME_SKY whenever an Environment exists; kernel sun_visibility -> 1.0 with that flag. With `sun: {on: false}` the environment (sky/HDRI packages) still lights = "the backdrop lighting the scene alone".
3 direct illuminance: arch3d `pub fn sun_illuminance(scene: &Scene3d, point: Vec3, normal: Vec3, toward_sun: Vec3, lux: f64, shadows: bool) -> f64` = lux*max(0,n.s)*vis (ray from point+n*1e-4 to 1e6 m against all meshes but housings; skipped when !shadows); 0 below horizon or non-finite. illuminance.rs: both methods parse via apply_look_params(render=true) on the global look (all render sun forms + `sky`); `sun` ABSENT stays night (phase-10/11 rely on it); direct reply gains "sun" (lux) added to "lux". IlluminanceRequest gains look, sun_dir, resolved_sun(); hook illuminance_one uses lighting_for.
CHECK 02_sun.py: SKY OFF IN EVERY SUN MEASUREMENT: every call in (a), (b) and (d) that measures the sun - each direct and path `illuminance` and each render - passes `sky: false` (the brief's "sky off"; the script merges a module-level `NOSKY = {"sky": False}` into every such call, not only the first), and every render of (b) also pins `exposure: {ev100: 15}` and `toneMap: "none"`. Written in wave 1 the check would pass without these pins, because the sky is not an Environment yet; from wave 3 p19-sky makes the default sky a Preetham environment, which path illuminance counts even at bounces 0 (p19-env-path env_irradiance: roughly 10-20 klx at noon, so (b)'s < 1 % in the column's shadow and 88 612 +-1 % with shadows off would fail) and which the column partly hides from (1.5,1.8,0) (so the path's "within 6" would fail), and no code change in p19-sky could make a check whose expected values leave the sky out pass with it on. (a) SITE + COLUMN, point (5,5,0): illuminance {sun: NOON, sky: false} -> 88 612 +-1 % (and "sun" key); {sun: NOON + intensity 50 000, sky: false} -> 44 306 +-1 %; method path bounces 0 spp 256, sky false -> the same two +-1 %; sun {azimuth 180, elevation 30}, sky false -> 50 000 +-1 %. (b) the noon shadow lies 1.0/tan 62.39 = 0.523 m north of the column (y 1.6..2.12): direct at (1.5,1.8,0) < 1 % of 88 612 with shadows, 88 612 +-1 % with shadows false; path the same (all with sky false); raster render (ground true, sky false, exposure {ev100: 15}, toneMap none, camera above, samples (1.5,1.8,0),(3.0,1.8,0)): shadowed darker by >= 45 r+g+b with shadows, within 6 without; same with final path (spp 16, bounces 0, seed 0, the same pins). (c) sun {date, time, on: false} -> "sun" 0. (d) sunangle "noon" (NOON) and "low" (az 180, el 30): list shows sunElevation 62.39 +-0.1 and 30; illuminance {sun:{angle:"low"}, sky: false} -> 50 000 +-1 %; look {sun:{angle:"noon"}} -> a render without sun echoes the noon dir (1e-9); deleting noon refused while used, allowed after look {sun:{angle:null}}.

Unit tests:
- `engine sunangle::tests::add_list_replace_delete`
- `engine sunangle::tests::refuses_bad_forms_duplicates_and_used_deletes`
- `engine illuminance::tests::direct_includes_the_sun_with_shadow_ray`
- `engine illuminance::tests::intensity_scales_absent_sun_is_night`
- `arch3d light::tests::sun_illuminance_open_ground_and_behind_column`
- `render3d look::tests::lighting_for_honours_shadows_and_on`
- `render3d pathtrace::tracer::tests::frame_sun_follows_intensity_and_no_shadow_flag`
- `render3d pathtrace::kernel_tests::sun_without_shadows_passes_occluders`
- `render3d pathtrace::tracer::tests::sky_off_leaves_only_the_sun` (irradiance of an upward probe on an open ground plane, sun straight down, bounces 4: 100 000 +-1 % with look.sky.on false; 109 425 +-2 % with it on)

### `p19-env-path` (wave 1)

**Environment light in the path tracer: an Environment type, importance-sampled equirect environments with MIS, visible or light-only**

- deps: p19-foundation, p19-r3d-groundwork
- crates: cadcraft-render3d, cadcraft-cli
- checks: none
- files:
  - `crates/render3d/src/environment.rs (new)`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/lib.rs`
  - `crates/render3d/src/pathtrace/env.rs (new)`
  - `crates/render3d/src/pathtrace/mod.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/render3d/src/shaders/pt/lights.wgsl`
  - `crates/render3d/src/shaders/pt/kernel.wgsl`
  - `crates/render3d/src/pathtrace/kernel_tests.rs`
  - `apps/cadcraft-cli/src/render_hook.rs`

BRIEF: HDRI "importance-sampled in the path tracer"; the sky lighting the scene and filling the background in the path tracer. Generic machinery; p19-sky and p19-hdri only produce an Environment.
FACTS: the PT's only environment is lights.wgsl sky_radiance (constant gradient SKY_HORIZON 5 000 -> SKY_ZENITH 2 500 nits under FRAME_SKY, set whenever the sun is on), added on every escape, no light sampling; camera misses are overpainted by backdrop_composite after resolve. No new PT buffers/bind groups (see p19-r3d-groundwork): tables go into the aux (`ies`) buffer at GpuFrame.env/env_aux offsets. Equirect convention = backdrop.wgsl: view dir d -> u = 0.5 + atan2(d.y,d.x)/(2pi), v = 0.5 - asin(d.z)/pi (row 0 zenith).
1 render3d/src/environment.rs: `#[derive(Clone, Debug, Default)] pub struct Environment { pub width: u32, pub height: u32, pub rgb: Vec<[f32;3]> /*linear nits, row 0 zenith*/, pub intensity: f32, pub rotation_deg: f32 /*ccw about +Z*/, pub horizon_deg: f32 /*+ raises the map's horizon*/, pub visible: bool /*background (sky/HDRI backdrop) or light only*/, pub daylight: bool, pub key: u64 }`; `uniform(rgb, w, h)`, `is_valid()` (1..=4096 x 1..=2048, len, finite >= 0), `radiance(&self, d: [f32;3]) -> [f32;3]` (undo rotation, shift elevation by -horizon (clamped), bilinear, u wraps, v clamps, x intensity; 0 if invalid/non-finite), `compute_key()`, a hand-written `impl PartialEq` comparing width, height, intensity, rotation_deg, horizon_deg, visible, daylight and key, never the pixels (the pane compares Lighting every frame); `pub fn environment_for(model: &ArchModel, look: &Look, sun_dir: Option<[f64;3]>) -> Option<Arc<Environment>>` = hdri_environment(model, look).or_else(|| sky_environment(model, look, sun_dir)).filter(valid); stubs `hdri_environment(model, look) -> Option<Environment>` [p19-hdri] and `sky_environment(model, look, sun_dir) -> Option<Environment>` [p19-sky] returning None. Lighting gains `pub environment: Option<Arc<Environment>>`; exposure::daylight (p19-exposure) also counts `environment.daylight` (if p19-exposure is not merged yet add `Lighting::has_daylight()` and say so).
2 Hook: render_one/illuminance_one set lighting.environment = environment_for(..), cached by (look sky fields, look backdrop, sun_dir bits, arch_revision).
3 pathtrace/env.rs: `pub struct EnvTable { width, height, radiance: Vec<f32>, marginal: Vec<f32> /*h+1*/, conditional: Vec<f32> /*h*(w+1)*/, total: f32 }`; `env_table(e: &Environment) -> EnvTable` bakes WORLD-space radiance via Environment::radiance at texel centres (<= 1024x512), weight luminance*sin(theta); CPU `sample(u) -> (dir, pdf_sa)` and `pdf(dir)` (pdf_sa = pdf_uv/(2 pi^2 sin theta)) for tests.
4 frame_environment (stub): environment Some and no furnace -> append the table to aux, fill env (flags 1 | 2 if visible), env_aux, env_params.x, set FRAME_ENV, clear FRAME_SKY. WGSL: env_lookup(d), env_sample(u) -> {dir, li, pdf}, env_pdf(d) (bounded binary searches). Hooks: env_direct = one env sample per non-glass vertex, visibility to 1e6, MIS power heuristic vs bsdf pdf; env_escape: camera ray (depth 0) -> env only if visible (else 0: the pixel stays a miss for the picture backdrop / clear colour), after specular -> full, after diffuse/glossy -> MIS-weighted; without FRAME_ENV today's sky_radiance; a camera ray that reads a visible environment is still a miss to tracer.rs backdrop_composite (accum hit count 0), and a visible environment exists only under a Sky or Hdri backdrop, for which p19-foundation's composite_misses paints nothing, so the environment stays in the image (if that early return is not on op/next, make backdrop_composite skip compositing when frame.flags has FRAME_ENV and frame.env[3] & 2, and say so in the report); env_irradiance = env NEE with cos/pdf (path illuminance counts sky/HDRI even at bounces 0; `sky:false` + no HDRI -> 0). Furnace untouched. Path replies gain "environment": null | {"width","height","visible"}.
TESTS (GPU via the shared fallback device): rotation/horizon of Environment::radiance; pdf integrates to 1 (+-1 %), one bright texel gets >= 90 % of samples; uniform env 1 over an open Lambert-0.5 plane (apply_lambert or an ior-1 test material) -> radiance 0.5 +-1 % (1024 spp), up probe pi, down 0; NEE+MIS vs BSDF-only +-2 %; invisible env keeps camera misses; a visible uniform env 1 with scene.backdrop Some(Backdrop::Sky), EV100 0, tone map none: camera misses read 0.8333 +-2 % (sRGB 235), never CLEAR_RGB; IES unchanged. No acceptance check (03/04 measure it).

Unit tests:
- `render3d environment::tests::radiance_applies_rotation_and_horizon`
- `render3d pathtrace::env::tests::pdf_integrates_to_one_and_finds_bright_texel`
- `render3d pathtrace::kernel_tests::uniform_environment_lights_a_lambert_plane`
- `render3d pathtrace::kernel_tests::env_irradiance_up_pi_down_zero`
- `render3d pathtrace::kernel_tests::env_mis_matches_bsdf_only`
- `render3d pathtrace::tracer::tests::invisible_environment_keeps_camera_misses`
- `render3d pathtrace::tracer::tests::visible_environment_shows_through_a_sky_backdrop`

### `p19-env-raster` (wave 2)

**Environment light in the raster views: nine spherical-harmonic coefficients for diffuse light and the environment drawn behind the scene, exposed and tone-mapped**

- deps: p19-exposure, p19-env-path
- crates: cadcraft-render3d
- checks: none
- files:
  - `crates/render3d/src/sh.rs (new)`
  - `crates/render3d/src/lib.rs`
  - `crates/render3d/src/lit.rs`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/frame_look.rs`
  - `crates/render3d/src/shaders/lit.wgsl`
  - `crates/render3d/src/backdrop.rs`
  - `crates/render3d/src/shaders/backdrop.wgsl`
  - `crates/render3d/src/offscreen.rs`
  - `crates/render3d/src/offscreen_tests.rs`

BRIEF: HDRI "in the raster views as nine spherical-harmonic coefficients for diffuse light"; sky lighting and background "in the pane, raster Finalize". Generic for any Environment.
FACTS: after p19-exposure the lit raster is physical; lit.wgsl `lit_ambient()` stub returns the constant ambient; FrameU has `ext`/`sh[9]` reserved; the backdrop pass draws pictures only (kinds 0..3) and ignores Sky/Hdri. WebGL2: sample float maps with textureLoad + manual bilinear like backdrop.wgsl's sample_img.
1 sh.rs: `pub fn project(env: &Environment) -> [[f32;3];9]` (real SH l <= 2, texel solid angle (2pi/w)(pi/h)sin theta, via Environment::radiance so rotation/horizon/intensity count; may downsample to <= 256x128) and `pub fn irradiance(sh: &[[f32;3];9], n: [f32;3]) -> [f32;3]` (Ramamoorthi-Hanrahan: A0 pi, A1 2pi/3, A2 pi/4; store convolved coefficients for the GPU).
2 Offscreen::lit_frame fills LitParams.sh from lighting.environment (cached by env.key), ext.y = 1; lit_ambient = albedo/PI * max(sh_irradiance(n), 0) * (1 - metallic*0.5) when ext.y > 0.5, else the stand-in. The meter (p19-exposure) uses sh::irradiance for E_amb when an environment is present.
3 Backdrop pass kind 4 = environment: when lighting.environment is visible, upload a world-space equirect copy (<= 1024x512, Rgba16Float or Rgba32Float, baked with Environment::radiance; re-upload on key change) and draw it with the frame's exposure scale and tone code (concat tonemap.wgsl); a visible environment wins over a picture; pictures stay exact display colours. Raster Finalize shares the path.
TESTS: SH of uniform L -> pi L for every n (+-0.5 %); upper hemisphere -> up pi +-2 %, down |E| < 0.1 pi; rotation honoured; GPU: uniform env 1, lambert 0.5, sun None, EV100 -1, none -> plane 0.8333 +-3 %; visible uniform env 1, EV100 0, none -> background 0.8333 +-2 % (ACES -> aces(0.8333)); a picture backdrop with a light-only environment keeps its exact colour; meter auto scale for the plane = 0.18/0.5. Re-run phase-14 09 and phase-19 01. No acceptance check of its own.

Unit tests:
- `render3d sh::tests::uniform_upper_hemisphere_and_rotation`
- `render3d offscreen_tests::raster_lambert_plane_under_uniform_environment`
- `render3d offscreen_tests::environment_background_exposed_and_tone_mapped`
- `render3d offscreen_tests::picture_backdrop_unchanged_with_light_only_env`
- `render3d frame_look::tests::meter_uses_sh_ambient`

### `p19-sky` (wave 3)

**A sky that follows the sun: Preetham clear sky, twilight to night from 0 to -6 degrees, a moon light and optional stars, in the pane, raster Finalize and path tracer**

- deps: p19-sun, p19-env-path, p19-env-raster
- crates: cadcraft-arch3d, cadcraft-render3d, cadcraft-engine, cadcraft-cli
- checks: tests/acceptance/phase-19/03_sky.py
- files:
  - `crates/arch3d/src/sky.rs (new)`
  - `crates/arch3d/src/lib.rs`
  - `crates/render3d/src/environment.rs (sky_environment)`
  - `crates/render3d/src/look.rs`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/frame_look.rs`
  - `crates/render3d/src/exposure.rs (daylight, directional lux)`
  - `crates/render3d/src/pathtrace/tracer.rs (frame_moon)`
  - `crates/render3d/src/shaders/pt/kernel.wgsl`
  - `crates/engine/src/cmd/arch/illuminance.rs`
  - `crates/arch3d/src/light.rs`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `tests/acceptance/phase-19/03_sky.py`

BRIEF: sky bullet + numbers line "sky" (noon: sky pixels b > r; sun at 2 deg: sky pixels toward the sun r > b; -10 deg: brightest sky pixel < 1 % of noon's).
CLEAN ROOM: from the published equations of Preetham, Shirley, Smits (1999): Perez F = (1+A e^(B/cos theta))(1+C e^(D gamma)+E cos^2 gamma) with coefficients linear in turbidity T for Y, x, y; Y_z = (4.0453T-4.9710) tan chi - 0.2155T + 2.4192 kcd/m2, chi = (4/9-T/120)(pi-2 theta_s); zenith chromaticity polynomials; xyY -> XYZ -> linear Rec.709 (formula constants are allowed).
FACTS: look.sky {on, turbidity 3, stars, moon {lux 0.25, az 150, el 35}}, Backdrop::Sky, the `backdrop {kind: "sky"}` command form (engine backdrop.rs, p19-foundation item 2b: this package does not edit that file) and the default sun dir (Light::DEFAULT, SW 45 deg) from p19-foundation; Environment/sky_environment stub from p19-env-path; raster SH/background from p19-env-raster; sun lux/shadows and direct sun illuminance from p19-sun. render3d depends on geom, color, arch3d, doc and gpu-optin only (crates/render3d/Cargo.toml; pathtrace/lights_tests.rs:7 says cadcraft_arch is not a dependency) and arch3d re-exports nothing of cadcraft_arch, so the sky model lives in ARCH3D (render3d and the engine both depend on it; arch3d depends on arch and may call arch::site::true_to_drawing); lighting_for(look, sun_dir, budget) takes no site, so compass directions are resolved before render3d.
1 crates/arch3d/src/sky.rs (pure; `pub mod sky;` in arch3d lib.rs): `sky_radiance(view: [f64;3], sun: [f64;3], turbidity: f64) -> [f64;3]` (nits; view below the horizon -> grey 0.2-albedo ground: (0.2/pi)*(sky + sun horizontal illuminance)); `zenith_luminance(theta_s, T) -> f64`; `twilight_factor(elev_deg) -> f64` (1 at >= 0, 10^(elev/1.5) to 1e-4 at -6, 0 below; below 0 the day sky is evaluated at elevation 0 and scaled); `NIGHT_SKY: [f64;3] = [0.0004, 0.0006, 0.0012]` nits added always; `star(view) -> f64` (hashed, ~1 % of texels above the horizon, 0.05 nits, scaled by 1 - twilight_factor); Perez cos theta clamped >= 0.01; never NaN; `moon_dir(model: &ArchModel, moon: &MoonLook) -> Option<[f64;3]>` (compass azimuth/elevation -> unit toward-moon vector through cadcraft_arch::site::true_to_drawing; None when not finite).
2 sky_environment: look.sky.on and no HDRI -> bake 512x256 from arch3d::sky with the effective sun dir (or Light::DEFAULT's), turbidity, stars if look.sky.stars; visible = effective backdrop is Sky; daylight = sun elevation > -6; key from inputs; sky off -> None. The sun stays a delta light (no disc needed).
3 Moon: lux from look.sky.moon, slightly blue colour normalised to luminance 1; lights only with the effective sun below the horizon (an explicit or sun-angle direction with z < 0) and the sky on. The DIRECTION is resolved outside render3d: the hook (render_one, illuminance_one; the pane later) calls `cadcraft_arch3d::sky::moon_dir(&model, &look.sky.moon)` and passes it to render3d look.rs `impl Lighting { pub fn with_moon(self, moon_dir: Option<[f64;3]>) -> Lighting }` (lighting_for's signature stays), which sets `moon: Option<([f64;3], f32)>` when those rules hold; raster: with the sun None and the moon up, with_moon makes the moon the directional light (no shadow map) and exposure_look sets FrameLook.sun_lux = `Lighting::directional_lux()` (look.sun.intensity while the sun is up, the moon's lux while the moon is the directional light, else 0) in place of p19-exposure's sun-only rule, so the raster's sun term carries the moon and nothing else at night; exposure::daylight ignores the moon: a moonlit night is metered (meter_night, the night range of p19-exposure) unless it is the legacy `sun: false` form, which turns the sky, and so the moon, off. PT: frame_moon -> FRAME_MOON, moon_dir, moon_rgb; moon_direct/moon_irradiance mirror the sun with a shadow ray (look.sun.shadows). CPU direct illuminance adds "moon" (sun_illuminance with the moon dir) when the sun is down.
4 The PT's default daylight sky changes from the 2 500/5 000-nit gradient to Preetham and the raster stand-in ambient to SH: re-run all render checks of phases 10-15 and phase-19 01/02; keep them green by code changes; report anything that moved (phase-19 02 pins `sky: false` on every sun measurement and 01's sun cases set sky false, so the Preetham default enters neither; if a 02 call without `sky: false` is on op/next, that is a check bug: add the pin there and report it).
CHECK 03_sky.py (raster interactive, final raster, final path spp 4 bounces 0; backdrop {kind: sky}; toneMap none; EV100 15; SITE; positions via sunangle; a small wall behind the camera): (a) NOON, eye (0,0,1.6) looking north pitched +70, fov 30: every pixel b > r, none the clear colour. (b) sunangle low (az 270, el 2), looking west pitch +4, fov 60: mean of sky pixels in the central third within 10 deg above the horizon r > b. (c) el -10, camera (a): brightest linear < 0.01 x noon's brightest. (d) -3 deg (camera b): brightest strictly between the -10 and 2 deg values. (e) sun at -20 (sunangle deep, az 0): illuminance {point (5,5,0), sun:{angle:"deep"}, sky:{moon:{azimuth:180, elevation:30}}} -> moon 0.125 +-1 %; moon lux 0 -> 0; at noon moon 0. (f) -20 deg, looking up, EV100 -6: stars on -> >= 20 pixels brighter than every pixel with stars off. (g) night metering: SITE, sunangle deep (az 0, el -20), ground:true, a camera straight down whose frame is all ground, `sky {moon {azimuth 180, elevation 90}}`, albedo 0.1, exposure auto, toneMap none: ground 0.18 +-3 % in the interactive view, final raster and final path (bounces 0, spp 16), mode auto, exposure 0.18/(0.1/pi*0.25) = 22.6 +-3 % (the night sky adds about 0.8 %); `render {sun: false, exposure: "auto"}` still reports 0.01.

Unit tests:
- `arch3d sky::tests::zenith_luminance_t3_theta27_61_is_11_08_kcd`
- `arch3d sky::tests::noon_bluer_low_sun_redder_toward_sun`
- `arch3d sky::tests::twilight_fades_by_minus_6_and_night_under_1_percent`
- `arch3d sky::tests::stars_only_at_night_deterministic`
- `arch3d sky::tests::no_nan_at_horizon_or_hostile_input`
- `arch3d sky::tests::moon_dir_follows_true_north`
- `render3d environment::tests::sky_visible_only_with_sky_backdrop`
- `render3d look::tests::with_moon_only_when_sun_down_and_sky_on`
- `render3d pathtrace::kernel_tests::moon_direct_light`
- `engine illuminance::tests::direct_moon_is_lux_times_sin_elevation`

### `p19-hdri` (wave 3)

**HDRI backdrops: a Radiance .hdr reader in Rust, `backdrop {kind: hdri}` with intensity, rotation and horizon offset, lighting the scene; horizon offset for 360-degree image backdrops; the CC-0 sample HDRI registered in the catalog**

- deps: p19-foundation, p19-env-path, p19-env-raster
- crates: cadcraft-arch, cadcraft-doc, cadcraft-engine, cadcraft-io, cadcraft-render3d
- checks: tests/acceptance/phase-19/04_hdri.py
- files:
  - `crates/arch/src/hdr.rs (new: read_hdr, fit, display_image)`
  - `crates/arch/src/lib.rs`
  - `crates/doc/src/arch/backdrop.rs (ImageRgbe, hex serde)`
  - `crates/doc/src/arch.rs (hdr_images, caps, prune_hdr_images)`
  - `crates/engine/src/cmd/arch/backdrop.rs`
  - `crates/engine/src/library/catalog.rs (.hdr Backdrop entries; install validates them)`
  - `crates/engine/src/library/mod.rs (Library::image tone-maps an .hdr Backdrop)`
  - `crates/engine/src/cmd/arch/library_icon.rs (only if the icon path needs more than Library::image)`
  - `crates/engine/src/cmd/arch/furnish.rs (backdrop_params: an .hdr Backdrop item places as hdri)`
  - `crates/io/src/library.rs (embed the HDRI)`
  - `crates/io/src/dxf_arch.rs (round-trip test at the HDR caps)`
  - `crates/render3d/src/environment.rs (hdri_environment)`
  - `crates/render3d/src/backdrop.rs`
  - `crates/render3d/src/shaders/backdrop.wgsl`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `assets/library/sample/catalog.json`
  - `assets/library/sample/hdri/README.md`
  - `tools/library/curate.py`
  - `tools/library/check_sample.py`
  - `tools/library/test_curate.py`
  - `tests/acceptance/phase-19/04_hdri.py`

BRIEF: HDRI bullet (reader, 360 backdrop that lights, intensity and rotation per backdrop, horizon offset for any 360 backdrop, a CC-0 HDRI in the sample set "as groundwork, before this phase is planned") + numbers line "HDRI" (uniform radiance 1.0 RGBE written by the check, upward white diffuse plane albedo 0.5, EV100 -1, no tone map: 0.8333 +-2 % path, +-5 % raster). Importance sampling = p19-env-path, SH = p19-env-raster.
FACTS (op/next 576538153): the groundwork HDRI is there: `assets/library/sample/hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr` (1024 x 512 new-style RLE RGBE, midday partly cloudy sky; Greg Zaal (original), Jarod Guest (sky edits); Poly Haven; CC0-1.0; ATTRIBUTION.md row present) with a README saying phase 19 registers it as a Backdrops item; it is NOT in catalog.json yet. Backdrop::Hdri / Image.horizon come from p19-foundation; render3d backdrop.rs loads pictures via s.library.image(item); backdrop.wgsl kind 3 = equirect picture (u = 0.5 + atan2(d.y, d.x)/(2 pi), v = 0.5 - asin(d.z)/pi); the PT composites pictures via backdrop_rgb. Library plumbing: engine library/catalog.rs entry_record maps a file extension to an item kind and refuses anything but .glb/.png/.jpg/.jpeg ("unsupported file type"), so an .hdr entry is skipped with a log line; the sample files are embedded by a hand-written include_bytes! list in crates/io/src/library.rs whose test sample_list_matches_the_catalog requires exactly the catalog's files, sorted; Library::image (library/mod.rs ~481) and library.icon (library_icon.rs ~39) decode every Backdrop file with LibraryIo.read_image (PNG/JPEG), and the Backdrops category icon uses the first item by (folder, name, id); Catalog::install checks images with read_image; `Library::file_bytes(id)` already returns an item's raw bytes. The engine depends on cadcraft-io only as a dev-dependency (the apps install LibraryIo fn pointers), but on cadcraft-arch directly, and the IES parser already lives there (crates/arch/src/ies.rs, used by engine lightfixture.rs): the HDR reader goes beside it. The DXF payload is one JSON XRECORD <= 64 MiB (io dxf_ext MAX_PAYLOAD, dxf_arch.rs MAX_ARCH_PAYLOAD) and a too-big payload reopens the drawing with NO arch model; Vec<u8> serializes as a JSON number array (~3.6 characters per byte), so one 2048 x 1024 RGBE would be ~30 MB.
1 READER, crates/arch/src/hdr.rs (`pub mod hdr;`): `pub fn read_hdr(bytes: &[u8]) -> Result<ImageRgbe, String>`: "#?RADIANCE"/"#?RGBE" header to an empty line; FORMAT=32-bit_rle_rgbe required; resolution "-Y H +X W" only; scanlines flat or new-style RLE (2,2,W>>8,W&255; per channel count > 128 = run of count-128, else literals; every count bounds-checked); old RLE refused; W,H 1..=8192; input <= 64 MiB; truncation, overruns, trailing data refused; no panic; allocation <= W*H*4. `pub fn fit(img: &ImageRgbe, max_w: u32, max_h: u32) -> ImageRgbe` (box downsample in linear light, aspect kept; a drawing stores at most 1024 x 512). `pub fn display_image(img: &ImageRgbe, max_side: u32) -> ImageRgba` (a thumbnail: scale 0.18 / log-average luminance, ACES, sRGB 8-bit, longer side <= max_side; for library icons). io needs nothing (it may re-export read_hdr).
   doc backdrop.rs: `pub struct ImageRgbe { width: u32, height: u32, rgbe: Vec<u8> }` with `rgbe` serialized as a lower-case hex string by a hand-written serde helper (2 characters per byte; bad hex or a wrong length deserializes to an image that is_valid() refuses), `to_rgb()` (e == 0 -> 0 else m*2^(e-136)), `is_valid()` (1..=1024 x 1..=512, len = w*h*4). doc arch.rs: ArchModel `#[serde(default, skip_serializing_if="BTreeMap::is_empty")] hdr_images: BTreeMap<String, ImageRgbe>`; consts `MAX_HDR_IMAGES = 4`, `MAX_HDR_BYTES = 8 * 1024 * 1024` (raw RGBE summed: at most ~16 MiB of hex in the payload); `pub fn prune_hdr_images(&mut self)` keeps only the images referenced by model.backdrop, model.look.backdrop and every camera's look.backdrop (and Camera3d.saved's look once p19-cameras has merged: if `saved` is on op/next when you finish, include it); validate() drops invalid images, prunes, then drops any beyond the caps (in key order).
2 ENGINE, an `hdri` arm beside p19-foundation's `sky` arm in engine backdrop.rs (KEYS gain file, intensity, rotation, horizon; `allowed` gains `"hdri" => [kind, file, item, intensity, rotation, horizon]` and `image` gains horizon; the refusal text and params doc gain hdri): `backdrop {kind:"hdri", file: path | item: id, intensity?: 0..=100 (1), rotation?: deg (0, stored mod 360), horizon?: -45..=45 (0)}`: a file read native-only (#[cfg(not(target_arch="wasm32"))], <= 64 MiB; wasm -> "not available in the web build"), or an item's bytes through the existing `Library::file_bytes(id)` (a Backdrop item whose file ends .hdr; another item -> refused naming it); then `cadcraft_arch::hdr::read_hdr` -> `fit(.., 1024, 512)`; stored under the item id or the file stem; prune_hdr_images after the change; still beyond MAX_HDR_IMAGES / MAX_HDR_BYTES -> refused ("a drawing holds at most 4 HDR images (8 MiB)"); every refusal changes nothing. `{kind:"image", projection:"equirect", horizon?: -45..=45}`; horizon with flat refused; `{kind:"image", item}` naming an .hdr item refused ("`<id>` is an HDR image: use {\"kind\": \"hdri\"}"). describe() reports all fields.
3 hdri_environment (fills the stub): the effective backdrop is Hdri and its image is in hdr_images -> Environment {to_rgb, intensity, rotation, horizon, visible true, daylight true}; a missing image -> None (no HDRI, the sky rules apply). The sun keeps lighting unless `sun:{on:false}` (backdrop alone).
4 Equirect pictures: backdrop.wgsl kind 3 and CPU backdrop_rgb shift the sampled elevation by -horizon (+ raises the picture's horizon).
5 SAMPLE SET: register the HDRI already on op/next (no own-made HDR, no own.py change). catalog.json gains {"id": "polyhaven:kloofendal-puresky", "name": "Partly cloudy sky (HDR)", "category": "Backdrops", "symbolType": "backdrop", "placement": null, "size": null, "keywords": ["sky", "hdri", "hdr", "clouds", "day", "backdrop"], "file": "hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr", "source": "https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky", "licence": "CC0-1.0", "author": "Greg Zaal (original), Jarod Guest (sky edits)", "attribution": "Kloofendal 48d Partly Cloudy (Pure Sky) by Greg Zaal and Jarod Guest, Poly Haven, CC0-1.0"} (same fields as the other sample items; keep the existing ATTRIBUTION.md row). catalog.rs entry_record: `("hdr", Some(Category::Backdrops)) => ItemKind::Backdrop`, `("hdr", _)` -> Err("an HDR item belongs in Backdrops"); Catalog::install validates an .hdr Backdrop with cadcraft_arch::hdr::read_hdr instead of read_image. crates/io/src/library.rs FILES gains `("hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr", include_bytes!("../../../assets/library/sample/hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr"))` in sorted order (after faucet-kitchen-chrome.glb, before lamp-desk.glb) so sample_list_matches_the_catalog holds. Library::image for a Backdrop item whose file ends .hdr decodes with read_hdr and returns display_image(.., 1024), so library.icon, the Backdrops category icon (this item may now be its first) and thumbnails work. hdri/README.md: say it is registered. tools/library: curate.py keeps this item, its hdri/ folder and both its ATTRIBUTION rows (the .hdr, and hdri/README.md, which p19-foundation adds) when it regenerates the catalog (a test in test_curate.py); check_sample.py accepts a Radiance .hdr backdrop (magic `#?RADIANCE` or `#?RGBE`). Re-run phase-14 checks 01, 02, 03, 11 and 13 and the io/engine library tests; keep them green without editing them.
6 PLACING IT FROM THE LIBRARY: the library panel arms `library.place` for every item that is not a material or finish (ui-egui library_panel.rs `arm`, :141-182), and place_record sends an ItemKind::Backdrop item through furnish.rs backdrop_params (:247), which builds {"kind": "image", "item": id} for every backdrop but backdrop:colour and backdrop:gradient (:260-265). So once item 5 makes the HDRI a Backdrop item, picking it in the library, the normal way to choose a backdrop, would hit this package's own refusal of an .hdr item as a picture. backdrop_params maps a Backdrop item whose `file` ends in `.hdr` (ASCII case-insensitive) to {"kind": "hdri", "item": id} and forwards `intensity`, `rotation` and `horizon` from the call (library.place already parses `rotation` as a number); image items also forward `horizon` beside `projection` (only when given, so backdrop_items_name_the_backdrop_kind stays green); library.place's params doc gains `intensity?, horizon? (backdrop items; rotation also turns an HDR backdrop)`.
CHECK 04_hdri.py: (a) 32x16 uniform 1.0 flat RGBE; flat terrain 40x30 (or ground:true + a far COLUMN); backdrop hdri file; render {albedo 0.5, sun {on:false}, exposure {ev100:-1}, toneMap none}, camera 3 m above looking down at a patch >= 5 m from objects: 0.8333 +-2 % path (spp 256, denoise false), +-5 % raster (interactive and final raster). (b) intensity 2 + EV100 0 -> 0.8333 both. (c) a file red (1,0,0) for u < 0.5, blue otherwise (u = 0.5 + atan2(d.y, d.x)/(2 pi), the backdrop.wgsl convention): a level camera looking along -Y (d = (0,-1,0): u = 0.25) sees red at rotation 0 and blue at rotation 180 (u = 0.75) over the central third of the image, raster and path (never look along +-X: that is the u = 0 / 0.5 seam). (d) a file bright (1.0) above its middle row, dark (0.01) below: the row 5 deg above the true horizon is bright at horizon 0 and dark at horizon 10; an equirect `own:backdrop-sky` image with horizon 10 changes that row versus 0 (raster). (e) the same uniform file RLE-encoded renders as (a) +-0.5 %. (f) the sample item: `backdrop {kind: "hdri", item: "polyhaven:kloofendal-puresky"}` loads and renders a non-uniform background (raster and path); then `backdrop {kind: "colour", colour: "#204060"}` and `library.place {item: "polyhaven:kloofendal-puresky", at: [0, 0]}` (how the library panel applies a backdrop; `at` is required by library.place and unused by a backdrop) replies via "backdrop", after which `backdrop {}` reports kind "hdri" with image "polyhaven:kloofendal-puresky"; `library` lists it in Backdrops with licence CC0-1.0; `library.icon {item}` gives a non-uniform PNG; the drawing saved as DXF reopens with its arch model and the HDRI backdrop (the image round-trips). Stub-proof: a constant environment fails (c), (d) and (f); a backdrop_params that still builds {kind: image} for the HDR item fails (f)'s library.place.

Unit tests:
- `arch hdr::tests::flat_and_rle_scanlines`
- `arch hdr::tests::refuses_truncated_overrun_trailing_bad_header_orientation_huge`
- `arch hdr::tests::fit_downsamples_to_1024_by_512`
- `arch hdr::tests::rgbe_one_is_one`
- `arch hdr::tests::display_image_bounded_and_not_uniform`
- `arch hdr::tests::hostile_hdr_does_not_panic`
- `doc backdrop::tests::image_rgbe_hex_round_trip_and_bad_hex_is_invalid`
- `doc arch::tests::hdr_images_pruned_and_capped`
- `engine backdrop::tests::hdri_from_file_and_item_and_refusals_change_nothing`
- `engine backdrop::tests::equirect_image_horizon`
- `engine backdrop::tests::hdr_item_refused_as_a_picture_and_caps_refuse`
- `engine furnish::tests::hdr_backdrop_item_places_as_hdri` (backdrop_params of polyhaven:kloofendal-puresky with {intensity 2, rotation 90, horizon 5} -> {kind hdri, item, intensity 2, rotation 90, horizon 5}; library.place {item, at [0, 0]} sets Backdrop::Hdri and one undo restores the previous backdrop)
- `engine catalog::tests::hdr_backdrop_entry_and_install`
- `engine library::tests::hdr_backdrop_image_is_tone_mapped`
- `io library::tests::sample_list_matches_the_catalog` (existing; now with the HDRI)
- `io dxf_arch::tests::hdr_images_at_the_caps_round_trip`
- `render3d environment::tests::hdri_environment_intensity_rotation_horizon`
- `render3d backdrop::tests::equirect_picture_horizon_offset`
- `tools/library/test_curate.py::test_keeps_the_hdri_item`

### `p19-horizon` (wave 0)

**Ground to the horizon: a flat ground plane outside the site skirt at the boundary's mean edge elevation, in the site's ground material, reaching the far plane**

- deps: none
- crates: cadcraft-arch3d, cadcraft-arch, cadcraft-engine, cadcraft-render3d
- checks: tests/acceptance/phase-19/06_horizon.py
- files:
  - `crates/arch3d/src/site.rs`
  - `crates/arch3d/src/lib.rs`
  - `crates/arch3d/src/camera.rs (FAR_PLANE const)`
  - `crates/arch3d/src/incremental.rs`
  - `crates/arch3d/src/probe.rs`
  - `crates/arch/src/terrain/surface.rs`
  - `crates/engine/src/cmd/arch/build3d.rs`
  - `crates/render3d/src/offscreen.rs (ensure_meshes)`
  - `crates/render3d/src/pathtrace/scene_gpu.rs (build_gpu_scene/collect_triangles)`
  - `tests/acceptance/phase-19/06_horizon.py`

BRIEF: Ground to the horizon bullet + numbers line "horizon" (camera 1.6 m above the site looking level: the row 5 px under the horizon has no sky or clear-colour pixels across its full width).
FACTS: the terrain solid (arch3d site.rs) = surface + vertical skirt to lowest - TERRAIN_SKIRT (1.0 m) along the boundary, material "terrain" (raster albedo [0.33,0.45,0.22], probe part "terrain"); FullCamera::proj and Orbit::proj hardcode z_near 0.1, z_far 10 000; Scene3d.bounds drives camera fits, projected_scene_bounds (image checks), and the sun shadow map's receiver bounds - so the horizon must NOT be in Scene3d.meshes or bounds (it would wreck shadow resolution, fits, HLR and exports). No phase-19 dependency.
1 arch3d camera.rs `pub const FAR_PLANE: f64 = 10_000.0` used by both proj(); arch `SiteSurface::mean_edge_elevation(&self) -> Option<f64>` = length-weighted mean of the surface sampled every <= 0.5 m along the boundary.
2 Scene3d gains `pub horizon: Option<Mesh>` (Default None, not in meshes/bounds); `site::horizon_mesh(model) -> Option<Mesh>`: with a terrain, mesh.push_prism_with_holes(square centred on the boundary centroid, half-side HORIZON_RADIUS = 0.95*FAR_PLANE = 9 500 m, holes [boundary ring], z0 = mean - TERRAIN_SKIRT, z1 = mean), material "terrain", level_elevation = mean; None without terrain. build() sets it; the incremental cache rebuilds it with the site; digest and scene_cache_equals_full include it.
3 probe::probe: after every real mesh, a point in the slab (inside the square, outside the boundary, z0..z1) -> {id "horizon", part "horizon", level null}. build3d reply gains "horizon": 0|1.
4 Raster ensure_meshes uploads it after the ground quad (opaque in glass-house); the PT appends its triangles with the terrain material. Shadow bounds, pick3d, HLR, exports never see it.
CHECK LIB: 06_horizon.py does not load p19lib.inc (p19-foundation runs in the same wave and may not have merged): it execs the newest existing of ../phase-18..15/pNlib.inc (the same few-line loader p19lib.inc uses) and defines CLEAR = (39, 44, 52) itself.
CHECK 06_horizon.py: (a) terrain {boundary [[0,0],[40,0],[40,30],[0,30]], elevation 0} + room_cmds at (16,10) (walls x 15.9..20.1, y 9.9..14.1: they fill row 125 left of about column 165); camera full eye (20,5,1.6) target (20,1000,1.6) fov 60 (vertical), 320x240; every assertion of (a) in ALL THREE renderers: interactive, final raster, final path (spp 4). Row 125 (5 px under row 120; it meets the ground about 60 m ahead, past the site's north edge) has no pixel within 8 per channel of CLEAR (39,44,52) and no pixel with every channel <= 8. The CLEAR test cannot fail in the path tracer: without a backdrop it composites nothing over its misses (tracer.rs backdrop_composite returns early) and sky_radiance is 0 below the horizon, so a missing horizon reads black there, never CLEAR. Then with `backdrop {kind: "colour", colour: "#ff00ff"}` (a backdrop the path tracer composites over its misses too; first assert the top-right sky pixel reads magenta, so the variant can see a miss) row 125 has no magenta pixel (r - g > 100 and b - g > 100) in each of the three renderers: that is the path tracer's real assertion. (a2) the site's ground material: in each renderer the mean of row 125 over columns 180..319 (clear of the room; the horizon slab about 60 m ahead) is green as phase-10 check 08 reads the terrain (mean g > mean r + 8 and mean g > mean b + 8) and within 12 per channel of the mean of row 140 over the same columns (the site's own sunlit terrain about 16 m ahead, x ~21.6..32.4, y ~21.2, clear of the room's north-east shadow under the default south-west sun): a grey or wrong-material slab fails. (b) probes (500,500,-0.5) -> horizon, (500,500,0.5) -> null, (20,15,-0.5) -> terrain; build3d horizon 1; after terrain.delete horizon 0 and the far probe null. (c) points (0,0,0),(40,0,2),(40,30,2),(0,30,0): mean edge (40+40+0+60)/140 = 1.0: (500,500,0.9) horizon, (500,500,1.1) null (1e-3). (d) build3d bounds stay within x -1..41, y -1..31 and the meshes count is unchanged by the horizon.

Unit tests:
- `arch terrain::surface::tests::mean_edge_elevation_length_weighted`
- `arch3d site::tests::horizon_slab_at_mean_edge_elevation_outside_boundary`
- `arch3d site::tests::horizon_not_in_bounds_or_meshes_and_none_without_terrain`
- `arch3d probe::tests::probe_reports_horizon`
- `arch3d incremental::tests::scene_cache_equals_full_with_horizon`
- `engine build3d::tests::build3d_reports_horizon`
- `render3d offscreen_tests::horizon_fills_below_the_horizon_line`
- `render3d pathtrace::scene_gpu::tests::gpu_scene_includes_horizon`

### `p19-strip-model` (wave 0)

**Strip lights: LightFixture kind strip along a path, under a cabinet run or as a ring along a room's walls, lumens per metre, down or out; plan symbol, housing, CPU illuminance, inspect and exports**

- deps: none
- crates: cadcraft-doc, cadcraft-arch, cadcraft-arch3d, cadcraft-engine, cadcraft-io, cadcraft-render3d
- checks: tests/acceptance/phase-19/05_strip.py
- files:
  - `crates/doc/src/arch/light.rs`
  - `crates/arch/src/light.rs`
  - `crates/arch/src/light_symbol.rs`
  - `crates/arch3d/src/fixture.rs`
  - `crates/arch3d/src/light.rs`
  - `crates/arch3d/src/incremental.rs`
  - `crates/engine/src/cmd/arch/lightfixture.rs`
  - `crates/engine/src/cmd/arch/light_inspect.rs`
  - `crates/io/src/ifc/lights.rs (+ the light DXF proxy writer)`
  - `crates/render3d/src/lights.rs (skip arm)`
  - `crates/render3d/src/pathtrace/scene_gpu.rs (gpu_light/pack_lights skip arm)`
  - `tests/acceptance/phase-19/05_strip.py`

BRIEF: Strip lights bullet, model part (path at a height, under a cabinet run, round a tray, lm/m, colour temperature, down Lambertian or out) + numbers line "strip": 2 m, 500 lm/m, down, 2.4 m, dark room: illuminance under its centre 49.72 lx +-2 %. Rendering = p19-strip-render.
Worked: I'(theta) = (500/pi) cos theta cd/m, h = 2.4, a = 1: E = (500/pi)(a/(h^2+a^2) + atan(a/h)/h) = 159.155*(0.147929+0.164496) = 49.72; 0.15 m midpoint sampling is within 0.1 %.
FACTS: LightKind = Point|Spot|Area; FixtureShape has no strip; LightFixture fields position/height/rotation/tilt/lumens/kelvin/cone/falloff/on/brightness/width/length/ies (serde defaults); arch light.rs catalogue/intensity_cd/validate_fixture/light_point; arch3d SceneLight (built with ..Default::default()), scene_lights_with, direct_illuminance, illuminance() (shadow rays via a BVH that skips housings, floors, ceilings); fixture.rs housings (`light-...`, part "light"); `lightfixture {room, rows, cols}` = a grid; Cabinet = position (centre of back edge), rotation, width, depth, elevation (carcass bottom), kind. Phase 16's trays arrive later: the brief's "round a tray" is p19-strip-tray's `{tray}` form, so keep the form parser in lightfixture.rs one match over the exclusive forms (path | under | room) that p19-strip-tray extends by one arm.
1 doc: LightKind::Strip, FixtureShape::Strip ("strip"); `#[derive(Default)] enum StripFacing { #[default] Down, Out }`; LightFixture gains serde-default `path: Vec<Vec2>`, `closed: bool`, `lumens_per_metre: f64`, `facing: StripFacing` (position = path midpoint; lumens kept = lm/m * length); consts MAX_STRIP_POINTS 256, MAX_STRIP_LENGTH 100 m, MAX_LUMENS_PER_METRE 10 000, STRIP_SAMPLE_SPACING 0.15.
2 arch: catalogue "LED strip" (Strip, Ceiling, 3000 K, 500 lm/m, diameter 0.012); validate: >= 2 finite points (<= 256, >= 0.01 m apart), length 0.05..=100, lm/m 0..=10 000, height 0..=100; `strip_length(f)`; `strip_samples(model, f) -> Vec<(Vec3 point, Vec3 normal, f64 lumens)>`: each segment in ceil(len/0.15) equal pieces at midpoints, z = level elevation + height, normal (0,0,-1) for Down or the horizontal unit vector RIGHT of the segment for Out (outward on a counter-clockwise ring); intensity_cd = lm/m / pi; light_point = midpoint.
3 plan_symbol: the path (+ ticks every 0.3 m on the emitting side); housing: 12 x 6 mm box along each segment, emitting face at the height (`light-strip`), 1 mm `light-lens` on it.
4 arch3d: SceneLight gains `strip: Option<Arc<Vec<StripSample>>>`, `pub struct StripSample { position: Vec3, normal: Vec3, intensity: f64 /*lumens/pi*/ }`; scene_lights_with builds Strip lights (brightness/light sets applied); direct_illuminance and illuminance() sum samples as Lambertian emitters (I*max(0,n_e.l)*max(0,n_r.l)/d^2) with a shadow ray each (area-light margins). render3d packing and the PT skip Strip until p19-strip-render.
5 engine lightfixture with type "LED strip": {path, closed?, height? (default: ceiling height of the room under the midpoint, else the level's), level?, lumensPerMetre?, kelvin?, facing?: "down"|"out", on?}; {under: [cabinet ids]}: wall cabinets on one level whose front edges join (0.01 m), path along their front-bottom edges 0.05 m behind the front face, height = elevation - 0.005, down; gaps or base cabinets refused naming them; {room, inset, height?}: closed path inset from the room outline, height default ceiling - 0.05, facing out (a ring along the room's walls; "round a tray" is p19-strip-tray's). lightfixture.edit {id, path?, closed?, height?, lumensPerMetre?, facing?, kelvin?, on?}; refusals change nothing.
6 inspect lights rows for strips: "kind":"strip","path","closed","height","lumensPerMetre","facing","length","lumens". Exports: DXF proxy of the plan symbol on the lights' layer, glTF node light-lf{n}, IfcLightFixture as other fixtures; scene_cache_equals_full with a strip.
CHECK LIB: 05_strip.py does not load p19lib.inc (p19-foundation runs in the same wave): it execs the newest existing of ../phase-18..15/pNlib.inc directly (room_cmds, run, cmd, glb helpers come from that chain).
CHECK 05_strip.py: ROOM (phase-10 room_cmds, ceiling 2.4) + {type "LED strip", path [[1,2],[3,2]], height 2.4, lumensPerMetre 500, facing down}: illuminance {point [2,2,0]} -> 49.72 +-2 %; 250 lm/m -> 24.86 +-2 %; facing out -> < 1 lx under it; inspect kind strip, length 2.0 (1e-9), lumens 1000; {room "r0", inset 0.5} -> closed, length 12.0 (1e-6), out; {under: two joined wall cabinets} -> path length = their widths' sum, height = their elevation - 0.005; plan DXF export has the polyline (1,2)-(3,2); GLB node light-lf0. Stub-proof: a midpoint point light gives ~55.3 lx; an isotropic emitter fails "out".

Unit tests:
- `doc light::tests::strip_serde_defaults_and_old_files_load`
- `arch light::tests::strip_samples_spacing_validate_and_catalogue`
- `arch light_symbol::tests::strip_symbol_is_the_path`
- `arch3d light::tests::strip_direct_illuminance_matches_line_integral`
- `arch3d light::tests::out_facing_normal_right_of_path`
- `arch3d fixture::tests::strip_housing_follows_path`
- `arch3d incremental::tests::scene_cache_equals_full_with_strips`
- `engine lightfixture::tests::strip_path_under_cabinets_room_inset_and_refusals`
- `engine light_inspect::tests::strip_rows`
- `io tests::strip_dxf_proxy_and_ifc_light_fixture`

### `p19-strip-tray` (wave 3; also waits for phase 16's `p16-trays`)

**Strip lights round a tray: `lightfixture {type: "LED strip", tray}` runs a closed strip along a phase-16 tray ceiling's outline, inside its recess**

- deps: p19-strip-model, p16-trays (phase 16's tray package; the workflow's dependency registry spans phases)
- crates: cadcraft-arch, cadcraft-engine
- checks: tests/acceptance/phase-19/05_strip.py (case g)
- files:
  - `crates/arch/src/light.rs (tray_strip_path)`
  - `crates/engine/src/cmd/arch/lightfixture.rs (the tray form)`
  - `tests/acceptance/phase-19/05_strip.py (case g)`

BRIEF: Strip lights bullet: a strip "along a 2D path at a height (or under a cabinet run, or round a tray)". CHIEF-REFERENCE §7.3: strips run in trays (saguarotrayceiling 03:40-04:23). p19-strip-model builds the strip and its path / under / room forms; this package adds the tray form on top of phase 16's trays.
FACTS (phase 16's p16-observables and p16-trays; read them on op/next before starting): ArchModel.trays (Tray {id, room, level, inset, depth, sides, polygon, parent}); `cadcraft_arch::ceiling::tray::outline(model, t) -> Result<Vec<Vec2>, String>` (base_ring inset by t.inset: the recess's edge; base_ring of a top-level tray is the room polygon, so following base_ring would repeat the {room, inset} form); `cadcraft_arch::ceiling::finished_ceiling_in(model, room, p)`; build3d `ceiling: [[x,y]]` reports the finished ceiling per point; p16lib.inc has HOUSE, GABLE and house_rooms(). If op/next has no `ceiling::tray` module when you start (the dependency was not honoured), stop and report it as not done instead of inventing trays. lightfixture.rs's exclusive forms (path | under | room) are one match from p19-strip-model: add one arm.
1 arch light.rs `pub fn tray_strip_path(model: &ArchModel, tray: &str, inset: f64) -> Result<(u32, Vec<Vec2>, f64), String>` = (the tray's level, a counter-clockwise closed ring, the default height above that level): the tray's outline inset by `inset` with cadcraft_geom::skeleton::inset (exactly one region without holes, else "the inset leaves no strip path inside tray `<id>`"), made counter-clockwise so facing out points at the recess sides; default height = the finished ceiling just outside the outline (finished_ceiling_in at the midpoint of the outline's first edge moved 0.01 m outward, perpendicular to it) - the level's elevation + 0.02 (the strip sits on the recess's bottom edge, 2 cm up). Unknown tray, an outline that errs, no ceiling there -> Err naming the tray.
2 engine lightfixture `{type: "LED strip", tray: id, inset?: 0..=1 m (0.05), height?: m above the level, lumensPerMetre?, kelvin?, facing?: "out" (the default for this form) | "down", on?}`: closed true, level = the tray's; two forms at once -> refused; the path is computed once at creation (a later tray.edit does not move the strip); every refusal changes nothing. Inspect strip rows gain "tray": id | null.
CHECK 05_strip.py case (g) (`# needs:` gains p19-strip-tray, p16-trays): p16lib HOUSE + GABLE + `tray {"room": NW, "inset": 0.6, "depth": 0.3}` (outline x 0.7..5.35, y 3.65..7.3; the tray id from its reply) + `lightfixture {"type": "LED strip", "tray": <id>}`: inspect row kind strip, closed true, facing out, tray <id>, path = the ring x 0.75..5.30, y 3.70..7.25 (1e-6, any start vertex, counter-clockwise), length 16.2 (1e-6), lumens 8100 (500 lm/m), height = build3d `ceiling` at (0.65, 5.475) + 0.02 = 2.42 (1e-6); `{tray, inset: 0.3}` -> length 2 x (4.05 + 3.05) = 14.2 (1e-6); `{tray: "tr9"}` and `{tray, inset: 2.0}` refused with error lines, no light added; a GLB export has node light-lf<n> for it. Stub-proof: the room form {room NW, inset 0.05} gives another ring (x 0.15..5.90, y 3.10..7.85), and a strip at the lower ceiling's plane misses the height.

Unit tests:
- `arch light::tests::tray_strip_path_insets_the_outline_ccw`
- `engine lightfixture::tests::strip_round_a_tray_and_refusals`

### `p19-strip-render` (wave 2)

**Strip lights rendered: one light against the raster budget sampled every 0.15 m, an emissive strip in the path tracer**

- deps: p19-exposure, p19-strip-model
- crates: cadcraft-render3d, cadcraft-cli
- checks: tests/acceptance/phase-19/16_strip_render.py
- files:
  - `crates/render3d/src/lights.rs`
  - `crates/render3d/src/lit.rs`
  - `crates/render3d/src/lit_frame.rs`
  - `crates/render3d/src/shaders/lit.wgsl`
  - `crates/render3d/src/pathtrace/scene_gpu.rs (gpu_light, pack_lights)`
  - `crates/render3d/src/shaders/pt/lights.wgsl`
  - `crates/render3d/src/shaders/pt/common.wgsl`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `tests/acceptance/phase-19/16_strip_render.py`

BRIEF: Strip lights bullet, render part: in the raster view a strip counts as one light against the budget (sampled as points 0.15 m apart); in the path tracer it is an emissive strip.
FACTS: raster packs <= MAX_GPU_LIGHTS 64 GpuLights (LightsU 16+64*64+64*64 = 8 208 bytes) chosen by select_drawn/select_shadowed (budget 8); lit.wgsl per-light maths is `fixture_light(i, wpos, n)` (groundwork); WebGL2 uniform limit 16 384. PT GpuLight {position, kind, direction, cos_inner, rgb, cos_outer, axis, width, length, ies_*}, kinds 0 point / 1 spot / 2 area, light_sample() in lights.wgsl, `light-lens` materials emit for camera rays only (MAT_EMIT_CAMERA_ONLY). Strip SceneLights come from p19-strip-model.
1 Raster: LightsU appends `strip_segs: array<StripSeg, 128>`, `struct StripSeg { a: vec4<f32> /*start, w cd per metre*/, b: vec4<f32> /*end, w pieces*/, n: vec4<f32> /*emitting normal*/ }` (LIGHTS_UNIFORM_SIZE 8 208 + 6 144 = 14 352 <= 16 384; test it). A strip = ONE GpuLight (dir_cos_outer.w = -3 marker, pos_range.xyz midpoint, kelvin colour, shadow.x = -1 (raster-unshadowed; shadows are the path tracer's), shadow.z/w = first segment and count) + its segments; select_drawn counts it once, select_shadowed never picks it; > 128 segments per frame drops the farthest strips first (deterministic). fixture_light for a strip sums pieces (<= 0.15 m, midpoints): I*piece_len*max(0,n_e.l)*max(0,n_r.l)/d^2; CPU mirror `strip_irradiance_cpu` tested against 49.72.
2 PT: LIGHT_STRIP = 3; one GpuLight per segment (position start, axis unit along, length, direction emitting normal, rgb = cd per metre * colour/luminance(colour)); light_sample picks a point uniformly along (u.x), li = rgb*length*max(0,n_e.(-dir))/d^2, pdf 1, delta 1 (no MIS: the lens is camera-only emissive), sample point 2 mm below the housing face on the emitting side; lens radiance for camera rays = (lm/m / 0.012)/pi if per-light lens radiance is available, else LENS_NITS (say which).
3 Hook: "drawnLights" counts fixtures (a strip once).
CHECK 16_strip_render.py: ROOM + the strip of 05 + sun false: (a) illuminance {point [2,2,0], method path, bounces 0, spp 1024} -> 49.72 +-2 %. (b) raster (camera inside looking down, samples (2,2,0),(2,3.8,0), EV100 4, none): with the strip on vs `lightfixture.edit {id lf0, on false}` the (2,2,0) sample is brighter by >= 60 r+g+b and brighter than (2,3.8,0); drawnLights 1, shadowedLights 0; an L-shaped 4 m strip [[0.5,0.5],[3.5,0.5],[3.5,1.5]] still drawnLights 1. (c) path render (spp 16, bounces 1): floor brighter with the strip on; looking up at it, lens pixels near white.

Unit tests:
- `render3d lights::tests::strip_one_drawn_light_never_shadowed`
- `render3d lights::tests::lights_uniform_fits_webgl2`
- `render3d lights::tests::strip_irradiance_cpu_matches_line_integral`
- `render3d offscreen_tests::raster_strip_lights_the_floor`
- `render3d pathtrace::lights_tests::pt_strip_irradiance_matches_line_integral`
- `render3d pathtrace::scene_gpu::tests::strip_lens_camera_only_emissive`

### `p19-normal-maps` (wave 0)

**Normal maps: materials carry a tangent-space normal map, procedural brick/siding/shingle/wood maps, user catalogue materials with their own, glTF normalTexture**

- deps: none
- crates: cadcraft-arch3d, cadcraft-arch, cadcraft-engine, cadcraft-io
- checks: tests/acceptance/phase-19/09_normal_maps.py
- files:
  - `crates/arch3d/src/texture.rs`
  - `crates/arch3d/src/lib.rs (MaterialDef)`
  - `crates/arch3d/src/paint.rs`
  - `crates/arch/src/library/record.rs`
  - `crates/engine/src/cmd/arch/paint.rs`
  - `crates/engine/src/library/catalog.rs (entry check; install copies the normal map)`
  - `crates/engine/src/library/mod.rs (Library::normal_map_bytes)`
  - `crates/io/src/glb.rs`
  - `tests/acceptance/phase-13/02_material_textures_glb.py (base-colour images only)`
  - `tests/acceptance/phase-13/13_demo.py (base-colour images only)`
  - `tests/acceptance/phase-19/09_normal_maps.py`

BRIEF: Normal maps bullet, data part: a material can carry a tangent-space normal map; phase 13's procedural brick, siding, shingle and wood textures gain matching ones; exported as glTF normalTexture. (Raster/PT use = p19-normal-maps-render.)
FACTS: arch3d texture.rs makes 256x256 tileable sRGB textures per Finish from integer hash noise; MaterialDef = base_color, metallic, roughness, texture ("finish:<name>" or a Scene3d.textures key), tile, tint (..Default::default()); paint.rs finish_def()/ref_def() ("item:<id>" from ArchModel.images); io glb.rs writes finish/item textures as PNG baseColorTexture; ItemRecord has `file` only; `library.install {from}` installs a catalog folder: Catalog::install (catalog.rs ~583-633) validates each entry and copies only its `file` (accepted = (entry, file, bytes); write_catalog writes those), and Library::file_bytes/dir_file read only `r.file`, so a normal map next to the file is lost unless this package copies it. Phase 18 may add Finish variants tonight: give unknown finishes the flat map (a final `_ =>` arm with a comment is fine).
1 texture.rs `pub fn normal_map(finish: Finish) -> Texture` (256x256 LINEAR RGBA8, rgb = (n*0.5+0.5)*255, n unit with n.z > 0, tileable, deterministic, same tile as the albedo) from height fields matching the albedo: brick joints recessed (1-texel bevel), siding laps (rising down each board, step at its bottom edge, rows only), shingles (course butt steps + vertical gaps), wood (low grain ridges); central differences with wrap; mean |n.xy| 0.05..0.4. `pub fn flat_normal_map() -> Texture` (128,128,255).
2 MaterialDef gains `normal_texture: Option<String>` ("finish:<name>:normal" for built-ins, else a Scene3d.textures key) and `normal_scale: f32` (default 1.0; update its Default). finish_def sets it; ref_def sets "item:<id>:normal" when ArchModel.images has "<id>:normal".
3 ItemRecord gains `#[serde(default)] normal_map: Option<String>` (PNG next to `file`); painting with such an item loads it into model.images as "<id>:normal" (image caps; a bad file refuses the paint, nothing changed). catalog.rs entry_record checks it like `file` (safe_relative, .png/.jpg/.jpeg); Catalog::install validates it (a regular file in the folder + read_image) and copies it beside `file` (write_catalog writes both; an invalid map skips the item with its reason); `Library::normal_map_bytes(&self, id) -> Result<Option<Vec<u8>>, String>` reads it for the sample, local, installed and user catalogs under file_bytes' licence rule; paint uses it.
4 glb.rs: materials with a normal map get "normalTexture": {"index": t, "scale": s (omitted when 1)} + the PNG (finish maps from normal_map(f)), deduplicated like base colour; normal-mapped meshes carry TEXCOORD_0; no tangents. Each normal image is NAMED after its base image with `-normal` (`brick-normal`, `<item image>-normal`) and comes after all base-colour images, so the base images keep their names and order. Phase-13 checks 02 (:21-24: `sorted(imgs) != ['brick', 'shingles', 'siding', 'wood']` and "the four textures are different images") and 13_demo (:56) list a GLB's images exactly, so the extra PNGs fail both (and p13lib glb_images keys images by name: reusing the finish names would make 02 measure the normal maps instead of the albedo). Change both to keep only the images whose names do not end in `-normal`, each with a comment that phase 19 adds normalTexture images; this is a check encoding the old export, so editing it is allowed (note it in the report). Phase-14 07 looks images up by the base texture's name and needs no change. Re-run phase-13 02 and 13 and phase-14 07.
CHECK LIB: 09_normal_maps.py does not load p19lib.inc (p19-foundation runs in the same wave): it execs the newest existing of ../phase-18..15/pNlib.inc directly (HOUSE1 is p14lib's).
CHECK 09_normal_maps.py: (a) HOUSE1 + material {walls exterior, finish brick}, export .glb: the brick material's normalTexture image decodes to 256x256, mean r,g 128+-12, mean b >= 220, r and g sd >= 4, joints both ways (phase-13 check 02 style); siding's map varies far more by rows than columns. (b) a catalog folder written by the check (one Material item "test-mat" with file + normalMap PNGs), library.install {from}, paint.material {material "test-mat", target "wall/w0/right", scope "component"}, GLB: that material's normalTexture image equals the check's PNG (size, first row). (c) two exports give byte-identical normal images.

Unit tests:
- `arch3d texture::tests::normal_maps_unit_deterministic_tileable`
- `arch3d texture::tests::brick_joints_both_ways_siding_rows`
- `arch3d paint::tests::finish_def_carries_normal_map`
- `arch library::record::tests::item_record_reads_normal_map`
- `engine paint::tests::item_normal_map_loads_with_paint`
- `engine catalog::tests::install_copies_and_validates_the_normal_map`
- `io glb::tests::normal_texture_for_brick_and_item_materials` (also pins the image names: base images first, then `<name>-normal`)

### `p19-normal-maps-render` (wave 2)

**Normal maps used by the raster views (texture array + derivative tangent frame) and by the path tracer (per-triangle UVs and texels in the aux buffer)**

- deps: p19-exposure, p19-normal-maps
- crates: cadcraft-render3d
- checks: tests/acceptance/phase-19/10_normal_maps_render.py
- files:
  - `crates/render3d/src/textures.rs`
  - `crates/render3d/src/pipeline.rs`
  - `crates/render3d/src/frame_look.rs (fill_normal_map)`
  - `crates/render3d/src/shaders/mesh.wgsl (shading_normal)`
  - `crates/render3d/src/pathtrace/scene_gpu.rs`
  - `crates/render3d/src/pathtrace/tracer.rs`
  - `crates/render3d/src/shaders/pt/kernel.wgsl (pt_shading_normal)`
  - `crates/render3d/src/shaders/pt/common.wgsl`
  - `tests/acceptance/phase-19/10_normal_maps_render.py`

BRIEF: Normal maps bullet, use part: used by the raster views and the path tracer.
FACTS: raster group 3 = one Rgba8UnormSrgb array (layer 0 white, 1..=4 finishes, then scene textures) + sampler; base_albedo samples it at uv*per-repeat; the face normal is cross(dpdy(wpos), dpdx(wpos)) (vertex normals untrustworthy). Groundwork stubs: PbrUniform.normal_layer/normal_strength, mesh.wgsl shading_normal, frame_look fill_normal_map, PT pt_shading_normal (trace() gives u, v), free GpuTri._pad and GpuMaterial._pad, GpuFrame.tex, aux buffer. The PT samples no albedo textures (means) - this is its first texture lookup.
1 Raster: a second texture array in Rgba8Unorm (LINEAR): layer 0 flat (128,128,255), 1..=4 finish normal maps (arch3d normal_map), then the scene's ":normal" textures; group 3 binding for it (update every pipeline layout: mesh, shadow, lit). fill_normal_map: material def/finish with normal_texture -> p.normal_layer = layer + 1, p.normal_strength = normal_scale, else 0. shading_normal: if normal_layer > 0, cotangent frame from dpdx/dpdy of wpos and of uv*per-repeat (Schueler 2013, public technique), sample in uniform control flow (sample always, select), n_t = rgb*2-1, xy *= strength, renormalise, to world, keep on the camera side; else n unchanged (bit-identical).
2 PT: build_gpu_scene appends to aux, for normal-mapped meshes, per-triangle UVs (6 floats, divided by tile) with GpuTri._pad = offset + 1 (0 none) and the map texels (packed RGBA8 bits per f32, bitcast in WGSL) with GpuMaterial._pad = layer + 1; frame.tex = (texel offset, 256, count, 0) + FRAME_NORMAL_MAPS. pt_shading_normal: barycentric UVs, tangent frame from edges and UV deltas (degenerate -> n), bilinear wrap fetch, perturb, renormalise, flip to n's side; shading normal for BSDF and light cosines, geometric for offsets; the ray-query path passes its committed barycentrics the same way.
CHECK 10_normal_maps_render.py: HOUSE1 with brick exterior walls, SITE, camera full eye (5,-6,1.2) target (5,0,1.2); mirrored suns {azimuth 135, elevation 25} and {azimuth 225, elevation 25}, sky false, manual EV100 for mid-grey, none; wall patch = central 40x20 px: (a) raster: mean |A-B| >= 1.5 levels, means within 3 %, same sun twice identical; (b) path (spp 16, bounces 0, seed 0, denoise false): mean |A-B| >= 1.0, means within 3 %; (c) siding: rows differ more than columns. Stub-proof: flat shading is identical under mirrored suns.

Unit tests:
- `render3d textures::tests::normal_array_linear_flat_layer_zero`
- `render3d frame_look::tests::fill_normal_map_layer_and_strength`
- `render3d offscreen_tests::flat_normal_layer_changes_nothing`
- `render3d offscreen_tests::brick_differs_under_mirrored_suns`
- `render3d pathtrace::scene_gpu::tests::uv_table_matches_mesh_uvs`
- `render3d pathtrace::kernel_tests::pt_normal_map_differs_under_mirrored_suns`

### `p19-seeing-in` (wave 1)

**Seeing in, the model: cutaway hides camera-facing exterior walls and what they host, a six-plane clip box and a near clip, honoured by pick3d**

- deps: p19-foundation
- crates: cadcraft-arch3d, cadcraft-engine
- checks: tests/acceptance/phase-19/07_cutaway_clip.py
- files:
  - `crates/arch3d/src/view_filter.rs (new)`
  - `crates/arch3d/src/lib.rs`
  - `crates/arch3d/src/camera.rs (view_proj_with_near, forward)`
  - `crates/arch3d/src/pick.rs (pick_filtered)`
  - `crates/engine/src/cmd/arch/pick.rs`
  - `tests/acceptance/phase-19/07_cutaway_clip.py`

BRIEF: Seeing in bullet (cutaway rule, clip box, near clip, pick3d) + numbers line "cutaway and clip". Raster/PT = p19-seeing-in-render; dragging in the pane = p19-pane.
FACTS: look.cutaway/clip/near_clip and their parsing from p19-foundation; pick3d (engine pick.rs) builds Scene3d and calls scene.pick (nearest hit only); parse_camera handles inline cameras; objects by Mesh.object + probe::part_of ("wall" incl. paint:wall/.., leaf/frame/glass/casing/sill/header = opening, cabinet/counter/pull, electrical, furnishing, light); hosting: Opening.host, ElectricalDevice.wall, FurnishingHost::Wall{wall}, wall-mounted light fixtures and cabinets carry no wall id (use proximity); Mesh.level_elevation; ArchModel.plan_level (None = 0); projection near hardcoded 0.1.
1 arch3d view_filter.rs: `pub type Hidden = BTreeSet<(&'static str, ArchId)>` (kinds "wall","opening","cabinet","electrical","furnishing","light"); `#[derive(Clone, Debug, Default, PartialEq)] pub struct ViewFilter { pub hidden: Hidden, pub clip: Option<ClipBox>, pub near: Option<f64>, pub eye: Vec3, pub forward: Vec3 }` with `from_look(look: &Look, model: &ArchModel, camera: &Camera) -> Self`, `is_empty()`, `key() -> u64`, `apply(&self, scene: &Scene3d) -> Scene3d` (drops meshes and furnishing/electrical boxes of hidden objects), `accepts(&self, p: Vec3) -> bool` (inside the box inclusive and (p-eye).forward >= near); `pub fn cutaway_hidden(model, eye: Vec3, level: u32) -> Hidden`: EXTERIOR walls on `level` whose outer face points at the eye (outer normal . (eye - closest centre-line point, plan) > 0); outer side = away from every room of the level (room containment of midpoint +- normal*(thickness/2+0.05)); if neither side is in a room, the side farther from the centroid of the level's exterior walls; plus hosted openings, devices on it, furnishings hosted on it, wall-mounted lights within thickness/2+0.05 of its centre line, cabinets on the level whose back edge is within 0.05 m of its inner face. Interior walls never; an eye inside the building hides nothing. Viewed level = plan_level or 0 (p19-cameras adds overview levels).
2 camera.rs `pub fn forward(camera: &Camera) -> Vec3`, `pub fn view_proj_with_near(camera: &Camera, aspect: f64, near: f64) -> [[f32;4];4]`; pick.rs `Scene3d::pick_filtered(&self, origin, dir, accept: &dyn Fn(&PickHit) -> bool) -> Option<PickHit>` (re-cast past rejected hits, <= 64 times).
3 pick3d: `camera` may be a saved camera NAME (live pose as render converts it + effective_look); params cutaway?, clip?, nearClip? parsed with apply_look_params; scene ViewFilter::apply'd and picked with pick_filtered(accepts); reply adds "filter": {"hidden": n, "clip": bool, "nearClip": m|null}; params doc updated.
CHECK 07_cutaway_clip.py (HOUSE1 = p14lib's: ring w0 = (0,0)-(4,0) .. w5, partition w6 at x = 4; room A x 0.1..3.95, room B x 4.05..7.9, y 0.1..4.9; + window {wall w0, offset 3.0, width 1.2, sill 0.9, height 1.2}: Opening.offset is the CENTRE (arch symbol.rs:50-53, opening.rs), so the window spans x 2.4..3.6, z 0.9..2.1; size [200,150]; centre pixel): (a) camera full eye (3.0,-8,1.5) target (3.0,0,1.5) fov 40 (the window's centre): without cutaway the hit is o0 with part glass; with cutaway it is neither w0 nor o0 (behind or null); an eye inside the house hides nothing. (b) at room A's centre, as the brief's numbers line says: eye (2.025,2.5,20) target (2.025,2.5,0) fov 30: unclipped hit = ceiling or roof; clip {min [-100,-100,-100], max [100,100,2.0]} -> part floor of room A. (c) eye (3.0,-0.6,1.5) target (3.0,0,1.5): without nearClip the hit is o0 (glass) 0.4..0.7 m away; nearClip 1.0 -> distance >= 1.0, not w0/o0. (d) camera {save "south", kind full, eye [3.0,-8], eyeHeight 1.5, target [3.0,0,1.5], fov 40}; pick3d {camera "south", cutaway true} behaves as (a).

Unit tests:
- `arch3d view_filter::tests::cutaway_hides_facing_exterior_walls_and_hosted`
- `arch3d view_filter::tests::from_inside_hides_nothing_and_level_only`
- `arch3d view_filter::tests::outer_side_without_rooms_uses_centroid`
- `arch3d view_filter::tests::accepts_clip_and_near`
- `arch3d pick::tests::pick_filtered_skips_rejected`
- `arch3d camera::tests::view_proj_with_near`
- `engine pick::tests::pick3d_cutaway_clip_near_and_named_camera`
- `engine pick::tests::pick3d_refuses_bad_clip`

### `p19-seeing-in-render` (wave 2)

**Seeing in, rendered: cutaway, clip box and near clip honoured by the raster views and the path tracer**

- deps: p19-exposure, p19-seeing-in
- crates: cadcraft-render3d, cadcraft-cli
- checks: tests/acceptance/phase-19/15_seeing_in_render.py
- files:
  - `crates/render3d/src/frame_look.rs (view_look, fill_clip)`
  - `crates/render3d/src/shaders/mesh.wgsl (clip_discard)`
  - `crates/render3d/src/offscreen.rs (near projection)`
  - `crates/render3d/src/shadow.rs (the sun depth pass honours the clip box)`
  - `crates/render3d/src/shadow_atlas.rs (the fixture depth passes honour the clip box)`
  - `crates/render3d/src/pathtrace/tracer.rs (frame_view)`
  - `crates/render3d/src/shaders/pt/traverse.wgsl`
  - `crates/render3d/src/shaders/pt/traverse_rq.wgsl`
  - `crates/render3d/src/shaders/pt/kernel.wgsl`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `tests/acceptance/phase-19/15_seeing_in_render.py`

BRIEF: Seeing in - honoured by the raster views and the path tracer.
FACTS: groundwork stubs clip_discard, PbrUniform.clip_min/max + PBR_FLAG_CLIP, view_look/fill_clip, GpuFrame.clip_min/clip_max/view + FRAME_CLIP, WGSL hit_clipped (both traversals) and camera_t_min; the hook caches one Scene3d per (revision, light set) and samples visibility with scene.pick; ViewFilter and view_proj_with_near/forward from p19-seeing-in. The sun shadow map (shadow.rs) and the fixture shadow atlas (shadow_atlas.rs) draw depth-only pipelines (`fragment: None`, a 64-byte light-matrix uniform).
1 Hook: per render `ViewFilter::from_look(&look, &arch, &camera)`; when it hides objects render a filtered scene cached by (revision, light-set key, filter.key()) under its own scene key (uploads/shadow maps follow; an unchanged hidden set reuses it); the PT gets the same scene; samples[].visible uses the filtered scene + pick_filtered(accepts); reply "filter": {"hidden","clip","nearClip"}.
2 Raster: view_look copies clip/near from lighting.look; fill_clip sets PBR_FLAG_CLIP + box; clip_discard discards outside the box (inclusive) in every pipeline using shade_parts; encode_with uses view_proj_with_near when FrameLook.near is Some. Shadows honour the clip box too, as in the path tracer (no exception): while FrameLook.clip is Some, the sun shadow map and the fixture shadow atlas draw with a second depth pipeline whose vertex stage passes the world position and whose fragment stage discards fragments outside the box (pass uniform 64 -> 96 bytes: light matrix + clip min/max; the fragment-free pipeline stays the default), and both shadow caches key on the clip box. The near clip is camera-only (no shadow change); cutaway-hidden objects are gone from the filtered scene, so they cast nothing.
3 PT: frame_view sets FRAME_CLIP, clip_min/max, view = (forward, near); hit_clipped(p) skips that candidate and continues for camera, bounce and shadow rays; camera_t_min(d) = near / max(dot(d, forward), 1e-4) for camera rays only; same in traverse_rq.wgsl (non-opaque candidates or re-query; if impossible fall back to the compute path while a clip box is on and say so).
CHECK 15_seeing_in_render.py (house, window and cameras of 07; 200x150; interactive, final raster, final path spp 4 bounces 0; every render pins sun {azimuth 180, elevation 60} (from the south), sky false, exposure {ev100 15}, toneMap none, so raster and path compare like with like; set-up `backdrop {kind: "colour", colour: "#ff00ff"}`, so a pixel that sees nothing reads (255,0,255) in every view: the path tracer composites a backdrop over its misses but leaves them black without one (tracer.rs backdrop_composite), and a colour backdrop lights nothing). THE PATH AT BOUNCES 0 (kernel.wgsl path_radiance): a camera ray that hits glass gets no direct light and stops, so the window reads black, and room A's interior is black wherever the sun does not reach it directly (its north wall lies in the ceiling's shadow: toward a sun at elevation 60 a point at z 1.5 meets the 2.4 m ceiling 0.52 m south of the wall). So every pixel assertion sets a sunlit surface, a shadowed one or the backdrop against another, never the window or the dark interior against each other (a fixture would not help: at EV100 15 its ~14 lx reads 0). (a) cutaway from the 07(a) camera: sample (1.0,4.89,1.2) (room A's north wall, inner face; the ray crosses w0 at x 1.77, beside the window, so only solid wall hides it) visible false without, true with; that sample's pixel (samples[].rgb; the camera does not move) changes by >= 30 r+g+b: without the cutaway it shows w0's sunlit outer face (cos 0.5, wall albedo 0.80: about sRGB 153 per channel), with it the north wall inside, in the ceiling's shadow (black in the path, ambient only in the raster). (b) clip max z 2.0 from above (07(b) camera): sample (3.0,1.5,0.0) visible only with the box; its pixel differs from the unclipped ceiling pixel by >= 30 (both sunlit: floor albedo 0.55/0.42/0.30, ceiling 0.92). (c) near clip from a camera inside room A facing w0: full eye (1.0,0.7,1.5) target (1.0,-5,1.5) fov 40 (the whole frame is solid wall, x 0.6..1.4 and z 1.2..1.8 at the outer face, clear of the window): without nearClip the centre pixel is w0's inner face (unlit: it looks north, away from the sun) and differs from (255,0,255) by >= 30 r+g+b, and sample (1.0,0.11,1.5) is visible; with nearClip 1.0 (beyond w0's outer face, 0.8 m away) the centre pixel is (255,0,255) +-2 and that sample is not visible; and from the 07(c) camera nearClip 1.0 makes the interior sample (3.0,4.89,1.5) visible. (d) shadows of clipped geometry, box of (b): floor samples (1.0,1.5,0) (sunlit: the cut south wall's 2.0 m top shades only up to y = 0.1 + 2.0/tan 60 = 1.255) and (1.0,0.6,0) (in that shadow): the first is brighter than the second by >= 60 r+g+b in the interactive view, raster Finalize and path. Stub-proof: a renderer that ignores the cutaway keeps w0's sunlit face at (a)'s sample pixel; one that ignores the near clip shows the wall, not the backdrop, at (c); a raster shadow pass that ignores the box keeps the removed ceiling's shadow on both (d) samples.

Unit tests:
- `render3d frame_look::tests::fill_clip_box_and_flag`
- `render3d offscreen_tests::raster_clip_discards_and_near_hides`
- `render3d offscreen_tests::clipped_geometry_casts_no_raster_shadow`
- `render3d pathtrace::kernel_tests::pt_clip_box_and_near_clip`
- `cli render_hook::tests::filtered_scene_cached_per_filter_and_samples_honour_it`

### `p19-cameras` (wave 3)

**Cameras keep their look: saved cameras store technique, sun angle, backdrop, exposure, light set, cutaway and clip box; camera.reset/orbit/update; floor overview camera kind**

- deps: p19-exposure, p19-sun, p19-seeing-in-render
- crates: cadcraft-doc, cadcraft-engine, cadcraft-arch3d, cadcraft-cli, cadcraft-io, cadcraft-ui-egui
- checks: tests/acceptance/phase-19/08_cameras.py
- files:
  - `crates/doc/src/arch.rs (CameraKind::Overview, Camera3d fields; prune_hdr_images keeps saved looks' images)`
  - `crates/engine/src/cmd/arch/view.rs`
  - `crates/engine/src/cmd/arch/render.rs`
  - `crates/engine/src/cmd/arch/pick.rs`
  - `crates/engine/src/cmd/arch/inspect.rs`
  - `crates/arch3d/src/view_filter.rs (level filter)`
  - `apps/cadcraft-cli/src/render_hook.rs`
  - `crates/io/src/dxf_arch.rs`
  - `crates/ui-egui/src/pane3d.rs (load_active_camera: an Overview arm that loads like Orbit)`
  - `crates/ui-egui/src/i18n/uk.tsv`
  - `tests/acceptance/phase-19/08_cameras.py`

BRIEF: Cameras keep their look bullet + numbers line "cameras" (a saved camera re-rendered after changing the global exposure, sun and light set gives the same image, fixed seed; camera.reset after an orbit gives the saved image again); floor overview camera kind, lower levels optional.
FACTS: Camera3d (doc arch.rs) = name, kind Orbit|Full, pose, light_set, + look: Option<Look> (p19-foundation); `camera {save|use|delete|list}` in view.rs (duplicate save refused - keep); render.rs camera_from_saved(); effective_look already renders a camera with look Some through that look; ViewFilter (+ hook filtering) exists; technique names match render `style` (shaded, clay, technical) and `final: "path"`. `cam.kind` is matched exhaustively (Full, Orbit) in ui-egui pane3d.rs load_active_camera (~443-470), render.rs camera_from_saved and view.rs: a new variant without its arms stops cadcraft-ui-egui compiling (E0004) until p19-pane, so this package adds them all. The cached scene's backdrop is swapped per render by arch3d::set_backdrop (p19-foundation), which also loads a look's picture.
1 doc: CameraKind::Overview (match the existing serde casing); Camera3d gains serde-default `saved: Option<Box<Camera3d>>` (its own saved always None), `level: Option<u32>` (None = plan level), `lower_levels: Option<bool>` (None = true); Default, camera_is_finite and dxf_arch.rs validation updated. ui-egui pane3d.rs load_active_camera gets an `Overview` arm that loads like Orbit (azimuth, elevation, distance, target; p19-pane adds the overview filter); run `cb test -p cadcraft-ui-egui` (with the doc, engine, arch3d and io crates).
2 Capture: `camera {save, pose..., kind?: orbit|full|overview, level?: name|index, lowerLevels?, + look keys}` -> look = effective global look (model.look + model.backdrop) overlaid by apply_look_params(render=false), stored COMPLETE; light_set mirrors look.light_set; saved = copy. Overview pose = orbit (defaults azimuth 30, elevation 50, target/distance fitted to the level's bounds at render time when absent).
3 Commands (uk.tsv "Reset Camera", "Orbit Camera", "Update Camera"; undoable; reply = camera listing): `camera.reset {name?}` (default active) copies saved -> live (pose and look); `camera.orbit {name?, azimuth?: delta deg, elevation?: delta deg, distance?: factor > 0}` changes the LIVE pose only (orbit/overview angles, elevation clamped +-89; full: rotate eye about target around +Z and pitch, scale distance); `camera.update {name?, + pose and look keys}` applies them live then saves live.
4 Technique: rendering through a camera (or the global look) with neither `style` nor `final` given: standard -> style shaded, path -> final "path", clay -> style clay, technical -> style technical; explicit params win.
5 Overview: ViewFilter gains `levels: Option<LevelFilter>`, `pub struct LevelFilter { pub elevation: f64, pub lower_levels: bool }`, `with_overview(self, model, level: u32, lower_levels: bool) -> Self`; apply() drops meshes of higher levels (level_elevation > e + 1e-6), every ceiling and roof part, and with !lower_levels the lower levels; NaN-level meshes kept unless roof/ceiling; site/horizon kept; cutaway level = the overview level. RenderRequest gains `overview: Option<(u32, bool)>`; hook and pick3d call with_overview.
6 arch.inspect {"cameras": true} -> [{"name","kind",pose...,"level","lowerLevels","look"|null,"saved"|null,"modified"}], "activeCamera".
7 HDR images: if p19-hdri's `ArchModel::prune_hdr_images` is on op/next when you finish, make it also keep the images that `Camera3d.saved` looks reference (p19-hdri does the same for `saved` if this package merged first).
CHECK 08_cameras.py (HOUSE1 = p14lib's 8 x 5 house: ring w0..w5, partition w6 at x = 4, room A x 0.1..3.95, room B x 4.05..7.9, y 0.1..4.9; + windows + lightfixture grids + SITE; light sets "Evening" all on / "Day" all off; sun angles "noon" (NOON), "evening" (az 300, el 8); 160x120): (a) camera {save "dusk", kind full, eye [-6,-6], eyeHeight 1.6, target [4,2.5,1.2], fov 50, technique standard, exposure {ev100 6}, sun {angle "evening"}, lightSet "Evening"} -> digest A; then look {exposure {ev100 12}, sun {angle "noon"}, lightSet "Day"} and backdrop colour #204060: through "dusk" -> A again; the same pose inline (no camera) differs from A. (b) technique "path" (spp 4, seed 7, bounces 1): equal digests before/after the global changes, finalMode "path" with no `final` param. (c) camera.orbit {name dusk, azimuth 30} -> different; camera.reset {name dusk} -> A; inspect modified true then false. (d) a smaller second level over room A: `level {above: 0}`, `wall {points [[0,0],[4,0],[4,5],[0,5],[0,0]], thickness 0.2, height 2.4, exterior true, level "Level 2"}`, `roomdetect {level: 1}`, `roofauto {pitch [6,12], overhang 0.5, level "Level 2"}`; overview cameras looking down (azimuth 0, elevation 89, target [4,2.5,0], distance 30: eye (4, 3.02, 30) by camera.rs Orbit::eye); P = (6.5,1.5,0) in room B, its pixel found via render samples: the eye-to-P ray is at (6.30, 1.62) at z 2.4, inside room B and clear of every level-0 wall, and at x >= 6.0 up to z 5.5, clear of level 1 (x <= 4.1). ov0 {kind overview, level 0, cutaway true}: pick3d at P's pixel = room B's floor (part floor; no roof, ceiling or level-1 part); ov1 {kind overview, level 1, lowerLevels true}: the same pick = room B's level-0 floor; ov1 with lowerLevels false -> null (no terrain here), never a level-0 part. Never pick over x = 8: that is w2's centre line (7.9..8.1), and the slanted ray meets its inner face at z ~1.0. (e) technique clay/technical through cameras = style clay/technical replies. (f) a camera keeps its picture backdrop: `backdrop {kind: "image", item: "own:backdrop-sky", projection: "equirect"}`, then camera {save "skycam", kind full, eye [-6,-6], eyeHeight 1.6, target [4,2.5,3.0], fov 60} (pitched up: the top of the frame is backdrop) -> raster digest S and path digest S' (spp 4, seed 7, bounces 1); then `backdrop {kind: "colour", colour: "#204060"}`: through "skycam" -> S and S' again, while the same pose inline differs from S (its sky reads (32,64,96)). Stub-proof: a hook that swaps scene.backdrop without arch3d::set_backdrop loses `item:own:backdrop-sky` and draws the clear colour, so S changes.

Unit tests:
- `doc arch::tests::overview_camera_and_saved_state_serde`
- `engine view::tests::save_captures_global_look_and_ignores_later_changes`
- `engine view::tests::orbit_changes_live_reset_restores_update_saves`
- `engine render::tests::technique_picks_style_and_final`
- `arch3d view_filter::tests::overview_drops_higher_levels_ceilings_roofs_lower_optional`
- `engine inspect::tests::cameras_rows`
- `io dxf_arch::tests::camera_look_and_saved_round_trip`
- `ui-egui pane3d::tests::overview_camera_loads_like_orbit`

### `p19-pane` (wave 4)

**The 3D pane: lit by the drawing's and the active camera's look, with exposure/tone-map/sun/sky controls, cutaway toggle, a clip box dragged in the pane, near clip, camera reset and floor overview**

- deps: p19-cameras, p19-sky
- crates: cadcraft-ui-egui, cadcraft-engine
- checks: none
- files:
  - `crates/ui-egui/src/pane3d.rs`
  - `crates/ui-egui/src/i18n/uk.tsv`
  - `crates/engine/src/cmd/arch/look.rs (pub view_look)`
  - `crates/engine/src/lib.rs`

BRIEF: everything applies "in the pane"; the clip box is "dragged in the pane". The UI is thin (act through app.run(id, params); colours from theme::Tokens).
FACTS: pane3d.rs (2 819 lines) renders through an Offscreen on the window device (compute_lighting(site, night, date, time), own orbit/full cameras, load_active_camera(), Style3d buttons, tool_row, Finalize UI); off.lit_frame + render_with run the groundwork's FrameLook/exposure/environment code once the pane passes a look-built Lighting. Engine pieces exist: effective_look/effective_sun_dir, environment_for, ViewFilter (+overview), commands look, sunangle, camera.reset/orbit/update.
1 engine `pub fn view_look(doc: &Drawing) -> ViewLook`, `pub struct ViewLook { pub look: Look, pub sun_dir: Option<[f64;3]>, pub overview: Option<(u32, bool)> }` for the active camera or the global look; re-exported.
2 Pane: on revision/camera change, Lighting = lighting_for(look, sun_dir, budget).with_moon(cadcraft_arch3d::sky::moon_dir(&model, &look.sky.moon)) (p19-sky) + environment_for (cached by inputs), meter_night = true (the pane never uses the legacy night form), lambert None (p19-foundation already turned the old Lighting copies in pane3d.rs into .clone(): Lighting is no longer Copy) and a ViewFilter (cutaway/clip/near/overview) applied to the pane scene (cached by key); the pane scene's backdrop is the effective look's, set with `cadcraft_arch3d::set_backdrop` (p19-foundation) so a camera's own picture backdrop finds its pixels; load_active_camera's Overview arm (p19-cameras, loads like Orbit) gains the overview filter. Old controls map to commands: night = look {sun {on false}, sky {on false}}; a date/time = sunangle {name "Pane", date, time, replace true} + look {sun {angle "Pane"}}.
3 A collapsible "Look" row; each change = ONE command (`look {...}`, or `camera.update {...}` when a saved camera is active): Technique; Exposure Auto + EV100 slider (-6..20); Tone map (ACES, Hable, None); Sun on / intensity / Cast shadows; Sun angle combo; Sky on / Stars; Cutaway; Clip box (initialised to scene bounds); Near clip slider (0 = off .. 10 m); Reset camera (camera.reset); Floor overview (saves/updates camera "Floor overview" for the plan level) + Lower levels.
4 Clip box: draw its 12 edges and six face handles projected with the pane camera; dragging a handle moves only that plane along its axis by (screen delta . axis_screen_unit)/|axis_screen|, clamped so min < max - 0.01; preview while dragging, one command on release (one undo step).
5 uk.tsv rows for every new label (Look, Technique, Exposure, Auto, EV100, Tone map, Sun intensity, Cast shadows, Sun angle, Sky, Stars, Cutaway, Clip box, Near clip, Reset camera, Floor overview, Lower levels, technique and tone-map names).
TESTS (no GPU needed: see pane3d_builds_without_an_adapter): pane lighting follows the active camera's look (EV100 3 -> Lighting.look.exposure Manual 3; environment and filter follow); cutaway toggle runs one look command (undo +1); dragging the top face handle by a known screen delta changes only clip.max.z by the projected amount and runs one command on release; Reset camera runs camera.reset; Floor overview saves an overview camera; translations test. No acceptance check.

Unit tests:
- `ui-egui pane3d::tests::lighting_follows_active_camera_look`
- `ui-egui pane3d::tests::cutaway_toggle_one_command`
- `ui-egui pane3d::tests::clip_face_drag_moves_one_plane`
- `ui-egui pane3d::tests::reset_and_overview_buttons`
- `ui-egui translations_cover_commands_menus_tools_and_snap_labels`
- `engine look::tests::view_look_uses_active_camera`

### `p19-demo` (wave 4)

**Demo: a small house at dusk (twilight sky, lit windows and strips, ground to the horizon), at noon under an HDRI, and a floor overview with the cutaway, each from its own saved camera**

- deps: p19-sky, p19-hdri, p19-horizon, p19-strip-render, p19-strip-tray, p19-normal-maps-render, p19-cameras
- crates: docs, tests
- checks: tests/acceptance/phase-19/11_demo.py
- files:
  - `docs/arch3d/showcase/phase-19-demo.json (new)`
  - `docs/arch3d/USER-GUIDE.md`
  - `tests/acceptance/phase-19/11_demo.py`

BRIEF: Demo bullet. Do not depend on phase 15's showcase script (docs/arch3d/showcase/ does not exist on op/next): a commands file for `cadcraft-cli run --commands FILE.json` (JSON array of {"cmd": id, "params": {...}}).
1 docs/arch3d/showcase/phase-19-demo.json: site (lat 51.05, lon -114.07, trueNorth 0); terrain 40x30 flat at 0 around the house; level 0 = the phase-1 house plan (exterior 10x8, interior walls, roomdetect) with brick/siding finishes (`material`), windows on every exterior wall, a front door; a kitchen with base + wall cabinets (phase-13 casework) and a strip `under` the wall-cabinet run; a tray ceiling in the living room (`tray {room, inset 0.5, depth 0.2}` after roofauto; shallower if the roof refuses it) with a strip round it (`lightfixture {type: "LED strip", tray}`, p19-strip-tray); pot lights (lightfixture room grids); level 1 = a smaller upper floor with a room and windows; roofauto; light sets "Evening" (all on) and "Day" (all off); sunangle "noon" (2026-06-21 solar-noon) and "dusk" (azimuth 300, elevation -3); set the backdrop before each save so each camera captures its own: "dusk" (full, outside south-west, technique standard, sun angle dusk, backdrop sky, manual EV100 tuned so windows read lit (about 3..5), lightSet Evening), "noon-hdri" (orbit, technique path, sun angle noon, HDRI backdrop `{kind: "hdri", item: "polyhaven:kloofendal-puresky"}` (the Poly Haven sky p19-hdri registers), rotation for a pleasant sky, exposure auto, lightSet Day), "overview" (kind overview, level 0, lowerLevels false, cutaway true, exposure auto); end with camera {use "dusk"}.
2 USER-GUIDE.md "## Light, sky and cameras": exposure/EV100, tone maps, sun intensity/shadows/angles, sky/twilight/moon/stars, HDRI intensity/rotation/horizon, ground to the horizon, strip lights, normal maps, cutaway/clip/near clip, camera looks, camera.reset, floor overview; examples fenced ```json or ```text (phase-13 check 09 RUNS ```sh blocks; use ```sh only for a block that finishes in < 30 s with --adapter fallback).
3 CHECK 11_demo.py: replay with --commands (CADCRAFT_RENDER_ADAPTER=fallback), no "error:" lines, save a DXF; from it render each camera (raster 320x240; noon-hdri also through its path technique, spp 8, 160x120): each PNG >= 40 distinct colours, per-channel sd >= 8; dusk: the row 5 px under the horizon of a level view from the dusk camera has no clear-colour pixel, its sky is darker than noon's, a window-centre sample is brighter (r+g+b) than a wall sample beside it; noon-hdri: finalMode "path", look echo has the hdri backdrop (the polyhaven:kloofendal-puresky image) and sun angle noon; overview: filter.hidden > 0 and a pick3d through it at the centre hits a floor; each look echo shows that camera's own exposure, sun angle and light set. Artifacts dusk.png, noon-hdri.png, overview.png (p14lib `artifact`).

Unit tests:

