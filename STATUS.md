# arch3d build status

Updated 2026-11-04 07:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 14 of 15: The library — 11 of 35 segments merged · 0 of 13 acceptance checks**

7,129 workspace tests passing at the last merge · 726 segments merged in all · 0 of 14 seats working

## Phases

| # | Phase | Status | Acceptance checks | Time |
|---|---|---|---|---|
| 0 | Survey | done | — | 0.2 h |
| 1 | Walls and joins | done | 9 / 9 | 31.0 h |
| 2 | Openings | done | 9 / 9 | 15.8 h |
| 3 | Rooms, floors, ceilings and levels | done | 9 / 9 | 56.2 h |
| 4 | Automatic roofs | done | 11 / 11 | 52.1 h |
| 5 | Stairs, railings and decks | done | 9 / 9 | 16.1 h |
| 6 | The 3D view, properly | done | 9 / 9 | 12.1 h |
| 7 | Generated elevations and sections | done | 9 / 9 | 8.2 h |
| 8 | Recognition from plain DXF | done | 10 / 10 | 16.6 h |
| 9 | Terrain and site | done | 12 / 12 | 19.4 h |
| 10 | Lighting, light fixtures and final renders | done | 11 / 11 | 14.2 h |
| 11 | Ray-traced Finalize Render | done | 13 / 13 | 15.5 h |
| 12 | Exports and interoperability | done | 15 / 15 | 15.2 h |
| 13 | Polish | done | 13 / 13 | 29.0 h |
| 14 | The library | building | 0 / 13 | 11.8 h so far · expected done Nov 05 11:29–Nov 06 23:53 UTC |
| 15 | Electrical, technical illustration and the showcase house | planned | — | — |

## Phase 14 segments

11 merged, 9 gate, 8 queued, 6 merging, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p14-arch3d-cabinet-parts` | Front styles, pulls, counter runs and cut-outs in 3D | gate | bonsai-a |
| `p14-arch3d-paint` | Painted surfaces in 3D: split faces, paint materials, tints | gate | claude-sonnet-b |
| `p14-arch-place` | Where a library item goes: floor, wall, ceiling, counter, gap, swap | merging | claude-sonnet-c |
| `p14-apps-library-hooks` | The apps install the library's readers | merging | claude-sonnet-d |
| `p14-cmd-icons` | library.icon: deterministic icons for items and categories, cached by content hash | gate | bonsai-b |
| `p14-cmd-furnishing-transform` | move, copy (count), rotate and mirror move furnishings by their handles | merging | mai-flash-esc |
| `p14-cmd-paint` | paint.material, paint.object and the eyedropper | gate | bonsai-ws2-a |
| `p14-inspect` | arch.inspect: furnishings, surfaces and the cabinet parts | merging | claude-sonnet-b |
| `p14-cmd-casework-parts` | cabinet and cabinet.edit take door and drawer styles and a pull | gate | claude-sonnet-a |
| `p14-cmd-backdrop` | The backdrop command | gate | claude-sonnet-c |
| `p14-io-glb-furnishing` | GLB: furnishing nodes, pull nodes, painted faces, symbol and paint materials | gate | mai-flash |
| `p14-io-obj-ifc-furnishing` | OBJ groups and IFC elements for furnishings | gate | darwin-ws3 |
| `p14-io-dxf-furnishing` | DXF: furnishing proxies and the round trip | merging | claude-sonnet-d |
| `p14-r3d-symbol-materials` | render3d draws symbol and paint materials: base colours, textures, tints | working | bonsai-ws2-b |
| `p14-r3d-backdrop` | Backdrops behind the 3D view and the raster Finalize | gate | claude-sonnet-d |
| `upstream-sync-5f8999f30` | merge upstream upstream/main up to 5f8999f30 (39 commits) | merging | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 04 04:47 | `p14-arch-paint` | merged | 7,129 pass | claude-sonnet-a |
| Nov 04 03:13 | `p14-io-gltf-read` | merged | 7,117 pass | claude-sonnet-b |
| Nov 04 02:16 | `p14-arch-place` | refused | 0 pass | claude-sonnet-c |
| Nov 04 01:55 | `p14-arch-furnishing-plan` | merged | 7,104 pass | claude-sonnet-d |
| Nov 04 00:40 | `harden-p13-arch3d-incremental` | refused | 7,100 pass, 2 fail | tturbo-mac |
| Nov 03 23:57 | `p14-arch-casework-layout` | merged | 7,098 pass | claude-sonnet-c |
| Nov 03 22:56 | `p14-io-library-files` | merged | 7,086 pass | claude-sonnet-a |
| Nov 03 22:06 | `p14-doc-furnishing` | merged | 7,084 pass | claude-sonnet-c |
| Nov 03 21:49 | `p14-arch-library` | merged | 7,074 pass | darwin-ws3 |
| Nov 03 21:14 | `p14-doc-cabinet-parts` | merged | 7,063 pass | mai-flash |
| Nov 03 20:02 | `harden-p13-arch-casework-symbols` | merged | 7,059 pass | tturbo-mac |
| Nov 03 19:28 | `harden-p13-dxf-casework` | merged | 7,051 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 865 |
| bonsai-b | bonsai2-27b | gate | 797 |
| bonsai-ws2-a | bonsai2-27b | gate | 875 |
| bonsai-ws2-b | bonsai2-27b | agent | 850 |
| claude-sonnet-a | sonnet | gate | 75 |
| claude-sonnet-b | sonnet | gate | 79 |
| claude-sonnet-c | sonnet | gate | 30 |
| claude-sonnet-d | sonnet | gate | 18 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 736 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | gate | 54 |
| mai-flash-esc | mai-code-1.1-flash | idle | 18 |
| tturbo-mac | tturbo-ultraheretic | gate | 606 |
| tturbo-mac2 | tturbo-ultraheretic | down | 348 |
