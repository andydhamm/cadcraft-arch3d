# arch3d build status

Updated 2026-11-01 13:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 15 of 24 segments merged · 1 of 13 acceptance checks**

6,355 workspace tests passing at the last merge · 636 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 1 / 13 | 6.1 h so far · expected done Nov 01 17:23–Nov 02 02:41 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

15 merged, 5 queued, 4 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-r3d-rt` | Optional hardware ray queries: the same traversal through wgpu ray queries, parity with the compute path | working | bonsai-b |
| `p11-r3d-final-path` | Finalize in path mode: FinalRender drives the path tracer, falls back to raster without compute | working | claude-sonnet-c |
| `p11-io-area-ies` | GLB and DXF for area lights and IES fixtures | working | bonsai-a |
| `upstream-sync-258c89fbb` | merge upstream upstream/main up to 258c89fbb (1 commits) | working | claude-sonnet-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 13:41 | `p11-r3d-pt-driver` | merged | 6,355 pass | darwin-ws3 |
| Nov 01 13:13 | `harden-p11-arch3d-panel` | merged | 6,348 pass | tturbo-mac |
| Nov 01 12:58 | `p11-r3d-pt-kernel` | merged | 6,337 pass | claude-sonnet-a |
| Nov 01 12:07 | `harden-p11-r3d-pipeline-cache` | merged | 6,321 pass | tturbo-mac |
| Nov 01 11:56 | `p11-arch3d-panel` | merged | 6,317 pass | darwin-ws3 |
| Nov 01 11:40 | `p11-arch3d-ies-area` | merged | 6,313 pass | bonsai-ws2-a |
| Nov 01 11:23 | `p11-cmd-render-path` | merged | 6,309 pass | bonsai-ws2-b |
| Nov 01 11:07 | `harden-p10-mcp-render-final` | merged | 6,305 pass | tturbo-mac |
| Nov 01 10:55 | `p11-r3d-pipeline-cache` | merged | 6,300 pass | mai-flash |
| Nov 01 10:42 | `harden-p9-arch-terrain-csv` | merged | 6,296 pass | tturbo-mac2 |
| Nov 01 10:20 | `p11-arch3d-panel` | refused | 0 pass | darwin-ws3 |
| Nov 01 10:11 | `p11-r3d-pipeline-cache` | refused | 6,270 pass | mai-flash |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 771 |
| bonsai-b | bonsai2-27b | agent | 708 |
| bonsai-ws2-a | bonsai2-27b | idle | 764 |
| bonsai-ws2-b | bonsai2-27b | agent | 747 |
| claude-sonnet-a | sonnet | idle | 63 |
| claude-sonnet-b | sonnet | agent | 60 |
| claude-sonnet-c | sonnet | agent | 10 |
| claude-sonnet-d | sonnet | idle | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 614 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | idle | 44 |
| tturbo-mac | tturbo-ultraheretic | merging | 533 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
