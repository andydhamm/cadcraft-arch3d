# MCP Examples

These JSONL files are example sessions for agents using the CADCraft MCP server. Each file is a sequence of JSON-RPC 2.0 requests that can be replayed with `cadcraft-cli mcp < FILE.jsonl`.

## Topics

- `01-walls-and-openings.jsonl`: Basic wall creation, doors, and windows.
- `02-rooms-roof-and-levels.jsonl`: Room detection, roof generation, and level management.
- `03-materials.jsonl`: Assigning finishes to walls, roofs, and floors.
- `04-kitchen-casework.jsonl`: Placing cabinets and fixtures in a kitchen.
- `05-build3d-and-render.jsonl`: Building the 3D scene, probing, and rendering.
- `06-exports.jsonl`: Exporting the model to IFC, OBJ, and glTF.
- `07-site-and-lights.jsonl`: Site location, terrain, plants, and lighting.
- `08-detect-a-plan.jsonl`: Detecting arch objects from a DXF plan.
- `09-library.jsonl`: Searching the library, placing furniture, painting surfaces, and exporting.

## Replaying

To replay a session, run:

```sh
cargo run -q -p cadcraft-cli -- mcp < docs/arch3d/mcp-examples/09-library.jsonl
```

Ensure you are in a directory where the output files (e.g., `house.dxf`) can be written. The `examples/` directory should be accessible relative to the working directory for the detection example.

## Tools Used

- `initialize`: Starts the session.
- `new_drawing`: Creates a new metric drawing.
- `execute`: Runs a command (e.g., `wall`, `door`, `roofauto`).
- `inspect_drawing`: Inspects the current drawing state.
- `render`: Renders the 3D view to a PNG.
- `save`: Saves the drawing to a DXF file.
