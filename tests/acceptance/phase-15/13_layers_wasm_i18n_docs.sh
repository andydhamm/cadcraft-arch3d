#!/usr/bin/env bash
# Carried forward: layering, exhaustive literals (the new electrical, connection and light-set structs included), the web
# build, assets, translations for every new command label; no GPU code below L5; unsafe only in crates/gpu-optin. Docs:
# USER-GUIDE.md has "Electrical", "Split levels" and "The showcase house" sections (phase-13 check 09 runs their sh
# blocks), and docs/arch3d/mcp-examples/10-electrical.jsonl is listed and replays (cadcraft-cli's mcp_examples tests).
# needs: p15-i18n-labels, p15-doc-electrical, p15-doc-levels, p15-cmd-electrical, p15-cmd-connections, p15-cmd-outlets-auto, p15-cmd-light-sets, p15-cmd-electrical-view, p15-cmd-levels, p15-docs-user-guide
set -uo pipefail
cargo xtask layers   || { echo "FAIL: layers"; exit 1; }
cargo xtask literals || { echo "FAIL: exhaustive struct literals"; exit 1; }
cargo xtask wasm     || { echo "FAIL: the web build broke"; exit 1; }
cargo xtask assets   || { echo "FAIL: assets"; exit 1; }
f=crates/doc/src/arch/electrical.rs
[ -f "$f" ] || { echo "FAIL: $f does not exist"; exit 1; }
grep -q "electrical.rs" xtask/src/literals.rs || { echo "FAIL: $f is not checked by cargo xtask literals (SHARED_ROOTS)"; exit 1; }
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- translations_cover_commands_menus_tools_and_snap_labels >"$log" 2>&1 || { echo "FAIL: translations"; tail -20 "$log"; exit 1; }
for label in "Outlet" "Switch" "Edit Electrical" "Delete Electrical" "Connection" "Edit Connection" "Delete Connection" \
             "Circuit Brightness" "Auto Outlets" "Light Set" "Electrical View" "Show Level" "Edit Level"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for '$label'"; exit 1; }
done
if grep -rn "use wgpu\|use egui\|wgpu::" crates/arch3d/src crates/arch/src crates/render/src crates/geom/src crates/doc/src/arch crates/engine/src crates/io/src >/dev/null 2>&1; then
  echo "FAIL: wgpu/egui below L5"; exit 1; fi
unsafe_sites=$(grep -rn "unsafe {\|unsafe fn\|unsafe impl" crates apps --include=*.rs | grep -v "^crates/gpu-optin/" | head -5)
[ -z "$unsafe_sites" ] || { echo "FAIL: unsafe code outside crates/gpu-optin: $unsafe_sites"; exit 1; }
for h in "## Electrical" "## Split levels" "## The showcase house"; do
  grep -q "^$h" docs/arch3d/USER-GUIDE.md || { echo "FAIL: USER-GUIDE.md has a '$h' section"; exit 1; }
done
[ -f docs/arch3d/mcp-examples/10-electrical.jsonl ] || { echo "FAIL: docs/arch3d/mcp-examples/10-electrical.jsonl"; exit 1; }
grep -q "10-electrical.jsonl" docs/arch3d/mcp-examples/README.md || { echo "FAIL: the MCP examples README lists 10-electrical.jsonl"; exit 1; }
cargo test -p cadcraft-cli --test it mcp_examples >"$log" 2>&1 || { echo "FAIL: the MCP examples replay"; tail -20 "$log"; exit 1; }
echo "ok: layers, literals, wasm, assets, translations, no unsafe; USER-GUIDE electrical/split-level/showcase sections; 10 MCP examples replay"
echo "PASS 13"
