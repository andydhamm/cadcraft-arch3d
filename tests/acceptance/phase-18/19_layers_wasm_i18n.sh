#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new molding and countertop structs included), the web build,
# assets, translations for every new command label; the user guide's phase-18 sections; no GPU code below L5; unsafe
# only in crates/gpu-optin.
# needs: p18-observables-1, p18-observables-2, p18-observables-3, p18-room-moldings, p18-molding-lines-1, p18-molding-lines-2, p18-countertops, p18-backsplash, p18-cabinet-fronts-1, p18-cabinet-fronts-2, p18-fillers-shelves, p18-glass-walls-1, p18-glass-walls-2, p18-glass-walls-3, p18-shower-niche-1, p18-shower-niche-2, p18-shower-niche-3, p18-shower-niche-4, p18-demo
set -uo pipefail
cd "$(dirname "$0")/../../.." || { echo "FAIL: no repository root"; exit 1; }
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
for f in crates/doc/src/arch/molding.rs crates/doc/src/arch/countertop.rs; do
  [ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
  grep -qF "\"$f\"" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
done
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Room Moldings" "Molding Line" "Edit Molding Line" "Delete Molding Line" \
             "Countertop" "Detach Countertop" "Edit Countertop" "Delete Countertop" \
             "Backsplash" "Edit Backsplash" "Delete Backsplash" "Cabinet Face" "Fill Cabinet Run" "Split Wall" "Niche"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
for heading in "## Interior finish" "## Kitchen and bath"; do
  grep -qF "$heading" docs/arch3d/USER-GUIDE.md || { echo "FAIL: docs/arch3d/USER-GUIDE.md has no '$heading' section"; exit 1; }
done
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; exit 1; fi
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
echo "ok: layers, literals, wasm, assets, translations (molding, countertop, backsplash, face, fill, split, niche labels), user guide sections, no GPU code below L5, no unsafe"
echo "PASS 19"
