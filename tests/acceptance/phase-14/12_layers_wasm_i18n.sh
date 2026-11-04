#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new furnishing, paint, backdrop and casework structs included), the
# web build, assets (the sample set attributed), translations for every new command label and menu entry; no GPU code
# below L5; unsafe code only in crates/gpu-optin; the glTF reader is our own (no gltf crate) and the library's pure parts
# stay out of the UI crate.
# needs: p14-doc-furnishing, p14-doc-cabinet-parts, p14-io-gltf-read, p14-cmd-library, p14-cmd-place, p14-cmd-paint, p14-cmd-user-catalog, p14-cmd-defaults, p14-cmd-backdrop, p14-cmd-icons, p14-cmd-furnishing-transform, p14-ui-library-panel, p14-ui-library-place
set -uo pipefail
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
for f in crates/doc/src/arch/furnishing.rs crates/doc/src/arch/paint.rs crates/doc/src/arch/backdrop.rs crates/doc/src/arch/casework.rs; do
  [ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
  grep -q "$(basename "$f")" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
done
if grep -rqE '^\s*gltf\s*=' Cargo.toml crates/*/Cargo.toml apps/*/Cargo.toml; then
  echo "FAIL: the glTF reader must be our own (serde_json + image), not the gltf crate"; exit 1; fi
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Library" "Library Item" "Library Icon" "Tag Library Item" "Library Filters" "Install Catalog" "Local Catalog" \
             "Place From Library" "Add to User Catalog" "Copy to User Catalog" "Edit Library Item" "Delete Library Item" \
             "Library Trash" "Edit Furnishing" "Delete Furnishing" "Center Furnishing" "Material Painter" "Object Painter" \
             "Material Eyedropper" "Set as Default" "Backdrop"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/engine/src crates/io/src | head; exit 1; fi
grep -q 'forbid(unsafe_code)' crates/render3d/src/lib.rs || { echo "FAIL: render3d must still forbid unsafe code"; exit 1; }
grep -q 'forbid(unsafe_code)' crates/io/src/lib.rs || { echo "FAIL: io must still forbid unsafe code"; exit 1; }
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
[ -f crates/arch/src/library/search.rs ] || { echo "FAIL: the library search lives in crates/arch/src/library/search.rs"; exit 1; }
# The library search stays out of the UI crate. finddlg.rs is FIND's Find and Replace window (#2607): it searches the
# drawing's text, not the library, so its `fn search` is not library search logic.
if grep -rqn --exclude=finddlg.rs "fn search\b\|fn matches_chip" crates/ui-egui/src; then echo "FAIL: search logic in the UI crate"; exit 1; fi
stray=$(git ls-files | grep -E '(^|/)[^/]*(_tmp\.rs|\.tmp)$' | head -5)
[ -z "$stray" ] || { echo "FAIL: stray files: $stray"; exit 1; }
echo "PASS 12"
