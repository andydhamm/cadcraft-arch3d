#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals, the web build, assets, translations; no GPU code below L5; the only
# unsafe code is the two calls in crates/gpu-optin; every phase-12 stub filled.
# needs: p12-ifc-openings, p12-ifc-slabs, p12-ifc-roofs, p12-ifc-stairs, p12-ifc-spaces, p12-ifc-site, p12-ifc-lights, p12-io-obj, p12-gltf-valid, p12-cmd-export, p12-pt-auto-exposure, p12-docs-user-guide
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
grep -q 'forbid(unsafe_code)' crates/io/src/lib.rs || { echo "FAIL: io must still forbid unsafe code"; exit 1; }
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
# The groundwork's stubs say "Filled by p12-…" or return after `let _ = (args);`; a filled one does neither.
stubs="crates/io/src/ifc crates/io/src/step.rs"
if grep -rnE "[Ff]illed by .?p12-|let _ = \(" $stubs >/dev/null 2>&1; then
  echo "FAIL: a phase-12 stub is still unfilled:"; grep -rnE "[Ff]illed by .?p12-|let _ = \(" $stubs | head; exit 1; fi
stray=$(git ls-files | grep -E '(^|/)[^/]*(_tmp\.rs|\.tmp)$' | head -5)
[ -z "$stray" ] || { echo "FAIL: stray files: $stray"; exit 1; }
echo "PASS 13"
