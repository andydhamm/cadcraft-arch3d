#!/usr/bin/env bash
# The 3D pane's style selector offers Clay and Technical (the phase-15 checkpoint, decision 2): Style3d::ALL lists
# Shaded, Shaded Edges, Hidden Line, Glass House, Clay and Technical in that order, next() cycles through all six and
# wraps, each maps to the renderer's style of that name (cadcraft_render3d::Style::Clay / Style::Technical), and the two
# new labels ("Clay", "Technical") have Ukrainian rows. The selector is UI, so the check runs the named ui-egui test that
# pins it (pane3d::tests::style_selector_offers_clay_and_technical) and reads uk.tsv.
# A stub fails: the test does not exist (0 tests run) or fails, or uk.tsv lacks a row.
# needs: p16-rw-pane-styles
set -uo pipefail
cd "$(dirname "$0")/../../.." || { echo "FAIL: no repository root"; exit 1; }
log=$(mktemp); trap 'rm -f "$log"' EXIT
cargo test -p cadcraft-ui-egui --lib -- pane3d::tests::style_selector_offers_clay_and_technical --exact >"$log" 2>&1
if ! grep -q "test result: ok. 1 passed" "$log"; then
  echo "FAIL: the ui-egui test pane3d::tests::style_selector_offers_clay_and_technical runs and passes:"
  grep -E "^test |test result|error(\[|:)" "$log" | head -20
  exit 1
fi
for label in "Clay" "Technical"; do
  grep -qP "^\t\Q$label\E\t\S" crates/ui-egui/src/i18n/uk.tsv || { echo "FAIL: uk.tsv has no row for the pane style '$label'"; exit 1; }
done
echo "ok: the pane's style selector offers Clay and Technical (the pinning test passes; uk.tsv rows exist)"
echo "PASS 19"
