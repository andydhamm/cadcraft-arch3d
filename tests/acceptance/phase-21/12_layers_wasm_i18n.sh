#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new sheet and schedule structs included), the web build, assets,
# translations for every new command label, the user guide's new sections; no GPU code below L5; unsafe only in
# crates/gpu-optin.
# needs: p21-observables-1, p21-observables-2, p21-observables-3, p21-opening-edit, p21-schedule-tables, p21-view-notes, p21-sheet-command, p21-demo
set -uo pipefail
cd "$(dirname "$0")/../../.." || { echo "FAIL: no repository root"; exit 1; }
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
f=crates/doc/src/arch/sheet.rs
[ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
grep -qF "\"$f\"" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Edit Opening" "Schedule" "Edit Schedule" "Delete Schedule" "View Note" "View Dimension" \
             "Delete View Note" "Sheet" "Edit Sheet" "Delete Sheet" "List Sheets" "Open Sheet" "Sheet Set"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
for heading in "## Schedules and labels" "## Interior elevations and view notes" "## Sheets"; do
  grep -qxF "$heading" docs/arch3d/USER-GUIDE.md || { echo "FAIL: docs/arch3d/USER-GUIDE.md has no '$heading' section"; exit 1; }
done
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; exit 1; fi
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
echo "ok: layers, literals, wasm, assets, translations, user guide sections, no GPU code below L5, no unsafe"
echo "PASS 12"
