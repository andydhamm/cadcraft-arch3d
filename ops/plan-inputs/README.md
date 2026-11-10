# Phase-plan inputs (not part of the product)

This branch carries what a planner needs to plan one phase of the arch3d challenge ahead of time. Phases are built
on the `arch3d` branch by a team of engineers, each taking one segment; a lead plans each phase as one JSON answer.
These files are that lead's inputs, so a phase can be planned now and handed to the team when it starts.

| File | What it is |
|---|---|
| `brief-general.md` | The challenge: goal, ground rules, target architecture, commands/MCP/CLI, how acceptance checks are written, testing, done. |
| `brief-phases-16-21.md` | The text of phases 16-21: the behaviour, the numbers the checks must use, the demo, what is out of scope. |
| `engineer-rules.md` | The rules every engineer follows (code style, tests, commands, layering). Segment instructions rely on them. |
| `plan-shapes.md` | The shapes of acceptance checks and segments, exactly as the lead is given them. |
| `drafts/phase-17.md` … `phase-20.md` | Earlier drafts of the plans for phases 17-20 (segments and instructions, no checks). They were written on 11-08 against a branch that is NOT `arch3d`: their phase-16 assumptions, file names and function names may be wrong now. Use them for the decisions and the review fixes they record, never as facts about the code. |
| `example/phase-16.answer.json` | The answer the team got for phase 16: the shape and level of detail of a good plan, with its acceptance checks and check library. |

## The answer a planning session writes

`ops/plans/phase-NN.answer.json` on its own branch, and nothing else changed:

```json
{"action": "done", "acceptance": [{"path": "tests/acceptance/phase-NN/01_x.py", "content": "..."}, ...],
 "segments": [ ... ], "design_notes": "what to add to docs/arch3d/DESIGN.md for this phase"}
```

- **Plan against the code on `arch3d` as it is now** (phase 16 is mostly merged). Read the crates you build on
  before naming a file, a type or a function. A segment that names something that does not exist costs the team hours.
- **Write the acceptance checks first.** They are locked: engineers cannot change them. Model them on the phase-16
  checks in `tests/acceptance/phase-16/` (their `p16lib.inc` helper, how they drive `cargo run -q -p cadcraft-cli --
  run --metric --cmd …`, how they print what was wrong). Use the brief's numbers exactly, and check that every number
  follows from the geometry. Each check starts with a comment saying what it proves and a `# needs:` line naming the
  segments it waits on.
- **Run what you can.** If cargo works here, build `cadcraft-cli` and run each new check: it must fail for the right
  reason (the command or field does not exist yet), never on a typo, a wrong path or a Python error. Run the earlier
  phases' checks your phase rewrites. At least `python3 -m py_compile` / `bash -n` every check.
- **Segments:** the shape in `plan-shapes.md`. One engineer's job for a few hours; observables and plumbing first;
  the DAG as wide as possible; each segment's `files` are the only files it may change (never `tests/acceptance/**`
  or `docs/arch3d/DESIGN.md`); instructions name the existing files and functions they plug into, the new types and
  signatures, the edge cases, and the Rust tests (`tests`) it must add. Existing tests a segment breaks are that
  segment's to update, and its instructions say so.
- `emit` is not available here: put every segment in the one answer.
