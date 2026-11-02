#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new casework and finish structs included), the web build, assets,
# translations for every new command label and menu entry; no GPU code below L5; unsafe code only in crates/gpu-optin;
# the new 3D and texture code stays out of the UI crate.
# needs: p13-doc-finish, p13-doc-casework, p13-cmd-casework, p13-texture-gen, p13-render3d-textures, p13-glb-textures, p13-pane-incremental
set -uo pipefail
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
for f in crates/doc/src/arch/finish.rs crates/doc/src/arch/casework.rs; do
  grep -q "$(basename "$f")" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
done
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Material" "Cabinet" "Edit Cabinet" "Delete Cabinet" "Fixture" "Edit Fixture" "Delete Fixture" "Fixture Types"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/engine/src crates/io/src | head; exit 1; fi
grep -q 'forbid(unsafe_code)' crates/render3d/src/lib.rs || { echo "FAIL: render3d must still forbid unsafe code"; exit 1; }
grep -q 'forbid(unsafe_code)' crates/io/src/lib.rs || { echo "FAIL: io must still forbid unsafe code"; exit 1; }
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
stray=$(git ls-files | grep -E '(^|/)[^/]*(_tmp\.rs|\.tmp)$' | head -5)
[ -z "$stray" ] || { echo "FAIL: stray files: $stray"; exit 1; }
echo "PASS 12"
