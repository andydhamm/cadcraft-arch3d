Look at the code you will build on, then write this phase's acceptance checks FIRST (they are locked: engineers cannot change them) and break the phase into segments.

Acceptance checks: {"path": "tests/acceptance/phase-N/NN_name.sh" (or .py), "content": "..."}.
Each is an executable black-box check run from the repository root with CARGO_TARGET_DIR set; exit 0 = pass. Drive the
product the way the challenge says a user would: `cargo run -q -p cadcraft-cli -- run --cmd "..."`,
`cargo run -q -p cadcraft-cli -- convert plan.dxf out.glb`, and check the outputs (python3 is available: parse the .glb's
JSON chunk for meshes, node names and accessor min/max for bounding boxes, closed meshes, counts). Write checks that
pass only when the phase's demo really works, and that print what was wrong when they fail. Use the CLI syntax you
define in the segments' instructions, so the engineers implement exactly what you check.

Each segment: {"id": "p1-wall-model", "title": "...", "size": "small" | "large",
 "crates": ["arch"], "files": ["crates/arch/src/wall.rs", ...], "deps": ["p1-arch-crate"],
 "instructions": "precise, self-contained instructions: what to add, the types and functions with signatures, how it
   plugs into existing code (name the existing files/functions), and edge cases",
 "tests": ["wall_thickness_is_positive", ...]}
- A segment is ONE engineer's job for a few hours: a coherent change to a handful of files, buildable on its own.
- "small" means it fits a 16K-token context (one or two files, tight instructions); otherwise "large".
- "tests" are names of Rust tests the segment must add in its crate (unit or tests/*.rs) and make pass.
- "deps" lists segments whose code it needs merged first. Make the DAG as wide as possible: several engineers work at once.
- Never assign tests/acceptance/** or docs/arch3d/DESIGN.md to a segment; those are yours.

While you plan, you may hand out segments that are already final, so engineers start on them before your plan
ends: reply {"action": "emit", "segments": [ ... ]} (the same segment shape, checked like the final answer; the reply
says whether they were accepted). Emitted segments are final: never emit them again, change or contradict them, or
repeat them in "done"; later segments may depend on them. "done" then carries only the segments not yet emitted (an
empty list if you emitted them all). Emit groundwork segments as soon as you are sure of them.

