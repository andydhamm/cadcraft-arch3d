#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new look and photo structs included), the web build (the learned
# denoiser and its weights are native only), assets (the denoiser weights with their ATTRIBUTION rows and hashes),
# translations for the new command labels, the user guide's section, no GPU code below L5, no unsafe outside gpu-optin,
# no Open Image Denoise code.
# needs: p22-doc-look, p22-r3d-types, p22-r3d-kernel-hooks, p22-r3d-tza, p22-r3d-unet-gpu, p22-engine-ladder, p22-engine-denoiser-info, p22-ui-pane-photoreal, p22-docs-user-guide
set -uo pipefail
cd "$(dirname "$0")/../../.." || { echo 'FAIL: no repository root'; exit 1; }
cargo xtask layers   || { echo 'FAIL: layers'; exit 1; }
cargo xtask literals || { echo 'FAIL: exhaustive struct literals'; exit 1; }
cargo xtask wasm     || { echo 'FAIL: the web build broke'; exit 1; }
cargo xtask assets   || { echo 'FAIL: assets'; exit 1; }
f=crates/doc/src/arch/photo.rs
[ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
grep -qF "\"$f\"" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
sum() { sha256sum "$1" | cut -d' ' -f1; }
[ "$(sum assets/denoise/rt_hdr_alb_nrm.tza)" = e586ef2ff48d7fbb7611986405220ed8fc5c13ca79bfc40be4dc742fbf959e1a ] || { echo 'FAIL: rt_hdr_alb_nrm.tza is not the committed weights file'; exit 1; }
[ "$(sum assets/denoise/rt_hdr_alb_nrm_small.tza)" = 980ea307e7825cae23ed8b1addf8de62ece84b61094795a5631ebf1c91b2e1a7 ] || { echo 'FAIL: rt_hdr_alb_nrm_small.tza is not the committed weights file'; exit 1; }
grep -qF 'assets/denoise/rt_hdr_alb_nrm.tza' ATTRIBUTION.md || { echo 'FAIL: ATTRIBUTION.md has no row for the denoiser weights'; exit 1; }
if grep -qi 'oidn\|openimagedenoise' Cargo.lock; then echo 'FAIL: an Open Image Denoise crate is in Cargo.lock'; exit 1; fi
if grep -rn 'oidn\|OpenImageDenoise' --include=Cargo.toml crates apps xtask >/dev/null 2>&1; then echo 'FAIL: a manifest names Open Image Denoise'; exit 1; fi
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo 'FAIL: translations'; tail -20 "$log"; exit 1; }
for label in 'Realism Ladder' 'Denoiser Info'; do
  grep -qP "^\\t\\Q$label\\E\\t\\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
grep -q '^## Photoreal stills' docs/arch3d/USER-GUIDE.md || { echo 'FAIL: docs/arch3d/USER-GUIDE.md has no "## Photoreal stills" section'; exit 1; }
grep -q 'render.ladder' docs/arch3d/USER-GUIDE.md || { echo 'FAIL: the user guide does not show render.ladder'; exit 1; }
[ -f docs/arch3d/showcase/phase-22-showcase.json ] || { echo 'FAIL: docs/arch3d/showcase/phase-22-showcase.json is missing'; exit 1; }
if grep -rn 'use wgpu\|use egui\|wgpu::' crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo 'FAIL: wgpu/egui below L5'; exit 1; fi
unsafe_sites=$(grep -rn 'unsafe {\|unsafe fn\|unsafe impl' crates apps --include=*.rs | grep -v '^crates/gpu-optin/' | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
echo 'ok: layers, literals, wasm, assets (denoiser weights), translations, user guide, no GPU code below L5, no unsafe, no OIDN code'
echo 'PASS 15'
