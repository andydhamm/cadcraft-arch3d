#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals, the web build, assets, translations; no GPU code below L5; the only
# unsafe code is the two calls in crates/gpu-optin; render3d still forbids unsafe; every phase-11 stub filled.
# needs: p11-arch-ies, p11-arch-panel, p11-arch3d-panel, p11-arch3d-ies-area, p11-r3d-gpu-scene, p11-r3d-pt-traverse, p11-r3d-pt-bsdf, p11-r3d-pt-lights, p11-r3d-pt-kernel, p11-r3d-pt-driver, p11-r3d-denoise, p11-r3d-pipeline-cache, p11-r3d-rt, p11-r3d-final-path, p11-cmd-render-path, p11-cmd-lightfixture-area-ies, p11-cli-render-path, p11-cli-render-args, p11-io-area-ies, p11-ui-pane-path
set -uo pipefail
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/engine/src crates/io/src | head; exit 1; fi
grep -q 'forbid(unsafe_code)' crates/render3d/src/lib.rs || { echo "FAIL: render3d must still forbid unsafe code"; exit 1; }
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
[ "$(grep -c 'unsafe {' crates/gpu-optin/src/lib.rs)" -le 2 ] || { echo "FAIL: crates/gpu-optin holds only its two unsafe calls"; exit 1; }
# The groundwork's stubs say "filled by p11-…" (Rust and WGSL) or return after `let _ = (args);`; a filled one does neither.
stubs="crates/arch/src/ies.rs crates/arch/src/light.rs crates/arch/src/light_symbol.rs crates/arch3d/src/light.rs crates/arch3d/src/fixture.rs
  crates/render3d/src/pathtrace crates/render3d/src/shaders/pt crates/render3d/src/final_render.rs crates/engine/src/cmd/arch/render.rs
  crates/engine/src/cmd/arch/illuminance.rs crates/engine/src/cmd/arch/lightfixture.rs crates/engine/src/cmd/arch/light_inspect.rs
  crates/io/src/glb_lights.rs crates/io/src/dxf_light.rs apps/cadcraft-cli/src crates/ui-egui/src/pane3d.rs"
if grep -rnE "filled by .?p11-|let _ = \(" $stubs >/dev/null 2>&1; then
  echo "FAIL: a phase-11 stub is still unfilled:"; grep -rnE "filled by .?p11-|let _ = \(" $stubs | head; exit 1; fi
stray=$(git ls-files | grep -E '(^|/)[^/]*(_tmp\.rs|\.tmp)$' | head -5)
[ -z "$stray" ] || { echo "FAIL: stray files: $stray"; exit 1; }
echo "PASS 11"
