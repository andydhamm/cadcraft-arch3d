#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new look structs included), the web build, assets (the CC-0 sample
# HDRI with its ATTRIBUTION rows), translations for every new command label, the user guide's phase-19 section; no GPU
# code below L5; unsafe only in crates/gpu-optin.
# needs: p19-foundation-1, p19-foundation-2, p19-foundation-3, p19-foundation-4, p19-foundation-5, p19-sun-1, p19-sun-2, p19-cameras-1, p19-cameras-2, p19-cameras-3, p19-cameras-4, p19-hdri-1, p19-hdri-2, p19-hdri-3, p19-hdri-4, p19-demo
set -uo pipefail
cd "$(dirname "$0")/../../.." || { echo "FAIL: no repository root"; exit 1; }
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
f=crates/doc/src/arch/look.rs
[ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
grep -qF "\"$f\"" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
hdr=assets/library/sample/hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr
[ -f "$hdr" ] || { echo "FAIL: the CC-0 sample HDRI $hdr is missing"; exit 1; }
grep -qF "\`$hdr\`" ATTRIBUTION.md || { echo "FAIL: ATTRIBUTION.md has no row for $hdr"; exit 1; }
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Render Look" "Sun Angle" "Reset Camera" "Orbit Camera" "Update Camera"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
grep -q "^## Light, sky and cameras" docs/arch3d/USER-GUIDE.md || { echo "FAIL: docs/arch3d/USER-GUIDE.md has no '## Light, sky and cameras' section"; exit 1; }
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; exit 1; fi
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
echo "ok: layers, literals, wasm, assets (sample HDRI), translations (look, sun angle, camera labels), user guide, no GPU code below L5, no unsafe"
echo "PASS 13"
