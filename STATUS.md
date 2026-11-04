# arch3d build status

Updated 2026-11-04 12:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 14 of 15: The library — 12 of 35 segments merged · 0 of 13 acceptance checks**

7,194 workspace tests passing at the last merge · 727 segments merged in all · 0 of 14 seats working

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
| 14 | The library | building | 0 / 13 | 17.0 h so far · expected done Nov 06 14:41–Nov 10 09:26 UTC |
| 15 | Electrical, technical illustration and the showcase house | planned | — | — |

## Phase 14 segments

12 merged, 9 merging, 7 queued, 6 escalate, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p14-arch3d-cabinet-parts` | Front styles, pulls, counter runs and cut-outs in 3D | merging | bonsai-a |
| `p14-arch3d-paint` | Painted surfaces in 3D: split faces, paint materials, tints | merging | claude-sonnet-b |
| `p14-apps-library-hooks` | The apps install the library's readers | merging | claude-sonnet-d |
| `p14-cmd-icons` | library.icon: deterministic icons for items and categories, cached by content hash | merging | mai-flash-esc |
| `p14-cmd-place` | library.place, furnishing.edit / .delete / .centre | working | mai-flash |
| `p14-cmd-furnishing-transform` | move, copy (count), rotate and mirror move furnishings by their handles | escalate |  |
| `p14-cmd-paint` | paint.material, paint.object and the eyedropper | escalate |  |
| `p14-inspect` | arch.inspect: furnishings, surfaces and the cabinet parts | merging | claude-sonnet-b |
| `p14-cmd-casework-parts` | cabinet and cabinet.edit take door and drawer styles and a pull | merging | claude-sonnet-a |
| `p14-cmd-backdrop` | The backdrop command | merging | claude-sonnet-c |
| `p14-io-glb-furnishing` | GLB: furnishing nodes, pull nodes, painted faces, symbol and paint materials | escalate |  |
| `p14-io-obj-ifc-furnishing` | OBJ groups and IFC elements for furnishings | escalate |  |
| `p14-io-dxf-furnishing` | DXF: furnishing proxies and the round trip | merging | claude-sonnet-d |
| `p14-r3d-symbol-materials` | render3d draws symbol and paint materials: base colours, textures, tints | escalate |  |
| `p14-r3d-backdrop` | Backdrops behind the 3D view and the raster Finalize | merging | claude-sonnet-d |
| `upstream-sync-5f8999f30` | merge upstream upstream/main up to 5f8999f30 (39 commits) | escalate |  |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 04 11:07 | `p14-cmd-furnishing-transform` | refused | 7,200 pass | mai-flash-esc |
| Nov 04 09:45 | `upstream-sync-5f8999f30` | refused | 0 pass | mai-flash |
| Nov 04 09:45 | `p14-arch-place` | merged | 7,194 pass | claude-sonnet-c |
| Nov 04 07:48 | `harden-p13-arch3d-incremental` | refused | 7,175 pass, 3 fail | tturbo-mac |
| Nov 04 04:47 | `p14-arch-paint` | merged | 7,129 pass | claude-sonnet-a |
| Nov 04 03:13 | `p14-io-gltf-read` | merged | 7,117 pass | claude-sonnet-b |
| Nov 04 02:16 | `p14-arch-place` | refused | 0 pass | claude-sonnet-c |
| Nov 04 01:55 | `p14-arch-furnishing-plan` | merged | 7,104 pass | claude-sonnet-d |
| Nov 04 00:40 | `harden-p13-arch3d-incremental` | refused | 7,100 pass, 2 fail | tturbo-mac |
| Nov 03 23:57 | `p14-arch-casework-layout` | merged | 7,098 pass | claude-sonnet-c |
| Nov 03 22:56 | `p14-io-library-files` | merged | 7,086 pass | claude-sonnet-a |
| Nov 03 22:06 | `p14-doc-furnishing` | merged | 7,084 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 866 |
| bonsai-b | bonsai2-27b | gate | 803 |
| bonsai-ws2-a | bonsai2-27b | gate | 878 |
| bonsai-ws2-b | bonsai2-27b | idle | 855 |
| claude-sonnet-a | sonnet | merging | 76 |
| claude-sonnet-b | sonnet | merging | 80 |
| claude-sonnet-c | sonnet | merging | 32 |
| claude-sonnet-d | sonnet | merging | 19 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 741 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 57 |
| mai-flash-esc | mai-code-1.1-flash | merging | 20 |
| tturbo-mac | tturbo-ultraheretic | merging | 607 |
| tturbo-mac2 | tturbo-ultraheretic | down | 348 |
