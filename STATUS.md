# arch3d build status

Updated 2026-11-01 12:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 13 of 23 segments merged · 0 of 13 acceptance checks**

6,321 workspace tests passing at the last merge · 633 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 0 / 13 | 4.6 h so far · expected done Nov 01 15:54–Nov 02 00:30 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

13 merged, 7 queued, 1 working, 1 escalate, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-r3d-pt-kernel` | The path-tracing kernel: camera paths, next-event estimation, MIS, roulette, AOVs, irradiance probes | working | claude-sonnet-a |
| `p11-r3d-pt-driver` | The progressive driver: frames, passes, snapshots, resolve, denoise hook, cancel; irradiance probes | escalate |  |
| `p11-io-area-ies` | GLB and DXF for area lights and IES fixtures | gate | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 12:07 | `harden-p11-r3d-pipeline-cache` | merged | 6,321 pass | tturbo-mac |
| Nov 01 11:56 | `p11-arch3d-panel` | merged | 6,317 pass | darwin-ws3 |
| Nov 01 11:40 | `p11-arch3d-ies-area` | merged | 6,313 pass | bonsai-ws2-a |
| Nov 01 11:23 | `p11-cmd-render-path` | merged | 6,309 pass | bonsai-ws2-b |
| Nov 01 11:07 | `harden-p10-mcp-render-final` | merged | 6,305 pass | tturbo-mac |
| Nov 01 10:55 | `p11-r3d-pipeline-cache` | merged | 6,300 pass | mai-flash |
| Nov 01 10:42 | `harden-p9-arch-terrain-csv` | merged | 6,296 pass | tturbo-mac2 |
| Nov 01 10:20 | `p11-arch3d-panel` | refused | 0 pass | darwin-ws3 |
| Nov 01 10:11 | `p11-r3d-pipeline-cache` | refused | 6,270 pass | mai-flash |
| Nov 01 09:47 | `p11-r3d-denoise` | merged | 6,266 pass | bonsai-a |
| Nov 01 09:30 | `p11-r3d-pt-traverse` | merged | 6,259 pass | mai-flash |
| Nov 01 09:22 | `p11-arch-panel` | merged | 6,256 pass | bonsai-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 764 |
| bonsai-b | bonsai2-27b | agent | 706 |
| bonsai-ws2-a | bonsai2-27b | agent | 762 |
| bonsai-ws2-b | bonsai2-27b | agent | 740 |
| claude-sonnet-a | sonnet | agent | 61 |
| claude-sonnet-b | sonnet | idle | 60 |
| claude-sonnet-c | sonnet | idle | 10 |
| claude-sonnet-d | sonnet | idle | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 606 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | idle | 44 |
| tturbo-mac | tturbo-ultraheretic | agent | 529 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
