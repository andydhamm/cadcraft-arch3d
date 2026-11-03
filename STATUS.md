# arch3d build status

Updated 2026-11-03 05:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 13 of 13: Polish — 15 of 23 segments merged · 3 of 13 acceptance checks**

6,775 workspace tests passing at the last merge · 697 segments merged in all · 0 of 14 seats working

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
| 13 | Polish | building | 3 / 13 | 14.6 h so far · expected done Nov 03 10:54–Nov 04 03:07 UTC |

## Phase 13 segments

15 merged, 3 merging, 3 queued, 1 replaced, 1 gate, 1 escalate, 1 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p13-glb-textures` | GLB embeds the finish textures (PNG) and names finished faces | merging | claude-sonnet-b |
| `p13-render3d-textures` | render3d samples the finish textures | gate | claude-sonnet-d |
| `p13-arch3d-casework` | Cabinets and fixtures in 3D | escalate |  |
| `p13-dxf-casework` | DXF: cabinets and fixtures as plan proxies | merging | claude-sonnet-c |
| `upstream-sync-7bc76f94c` | merge upstream upstream/main up to 7bc76f94c (24 commits) | merging | mai-flash-esc |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 03 04:10 | `harden-p13-build3d-incremental` | merged | 6,775 pass | tturbo-mac |
| Nov 03 02:11 | `p13-arch3d-casework` | refused | 0 pass | mai-flash-esc |
| Nov 03 02:11 | `p13-build3d-incremental` | merged | 6,771 pass | bonsai-ws2-a |
| Nov 03 01:36 | `p13-arch3d-finish-meshes` | merged | 6,764 pass | claude-sonnet-b |
| Nov 03 01:20 | `p13-pane-incremental` | merged | 6,751 pass | claude-sonnet-a |
| Nov 03 01:07 | `p13-texture-gen` | merged | 6,749 pass | claude-sonnet-b |
| Nov 03 00:49 | `p13-cmd-casework` | merged | 6,741 pass | claude-sonnet-d |
| Nov 03 00:22 | `p13-arch-casework-symbols` | merged | 6,732 pass | claude-sonnet-c |
| Nov 02 23:50 | `upstream-sync-dae4b7b2f` | merged | 6,726 pass | mai-flash |
| Nov 02 23:22 | `harden-p12-ifc-openings` | merged | 6,726 pass | tturbo-mac |
| Nov 02 22:22 | `p13-sample-house` | merged | 6,707 pass | mai-flash-esc |
| Nov 02 22:03 | `p13-inspect-finishes` | merged | 6,701 pass | claude-sonnet-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 825 |
| bonsai-b | bonsai2-27b | gate | 763 |
| bonsai-ws2-a | bonsai2-27b | idle | 832 |
| bonsai-ws2-b | bonsai2-27b | idle | 818 |
| claude-sonnet-a | sonnet | idle | 72 |
| claude-sonnet-b | sonnet | idle | 73 |
| claude-sonnet-c | sonnet | idle | 20 |
| claude-sonnet-d | sonnet | gate | 12 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 687 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 52 |
| mai-flash-esc | mai-code-1.1-flash | idle | 11 |
| tturbo-mac | tturbo-ultraheretic | gate | 583 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
