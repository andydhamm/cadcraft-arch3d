# arch3d build status

Updated 2026-11-01 16:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 18 of 24 segments merged · 8 of 13 acceptance checks**

6,383 workspace tests passing at the last merge · 642 segments merged in all · 0 of 13 seats working · PAUSED

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
| 11 | Ray-traced Finalize Render | building | 8 / 13 | 8.3 h so far · expected done Nov 01 18:40–Nov 02 04:06 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

18 merged, 3 working, 2 queued, 1 escalate

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-r3d-rt` | Optional hardware ray queries: the same traversal through wgpu ray queries, parity with the compute path | working | bonsai-b |
| `p11-cli-render-args` | cadcraft-cli render --final [raster|path] --spp --bounces --seed --no-denoise --rt | working | claude-sonnet-d |
| `p11-io-area-ies` | GLB and DXF for area lights and IES fixtures | escalate |  |
| `p11-ui-pane-path` | The 3D pane's Finalize: path traced or raster, refining on screen with its sample count, cancel | working | kimi-k31 |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 15:57 | `p11-cli-render-path` | merged | 6,383 pass | claude-sonnet-a |
| Nov 01 15:44 | `harden-p11-r3d-final-path` | merged | 6,378 pass | tturbo-mac |
| Nov 01 15:17 | `p11-ui-pane-path` | refused | 6,375 pass | bonsai-ws2-a |
| Nov 01 15:08 | `harden-p11-arch3d-ies-area` | merged | 6,372 pass | tturbo-mac |
| Nov 01 14:25 | `upstream-sync-258c89fbb` | merged | 6,367 pass | claude-sonnet-b |
| Nov 01 14:12 | `p11-r3d-final-path` | merged | 6,365 pass | claude-sonnet-c |
| Nov 01 14:04 | `harden-p11-cmd-lightfixture-area-ies` | merged | 6,361 pass | tturbo-mac |
| Nov 01 13:41 | `p11-r3d-pt-driver` | merged | 6,355 pass | darwin-ws3 |
| Nov 01 13:13 | `harden-p11-arch3d-panel` | merged | 6,348 pass | tturbo-mac |
| Nov 01 12:58 | `p11-r3d-pt-kernel` | merged | 6,337 pass | claude-sonnet-a |
| Nov 01 12:07 | `harden-p11-r3d-pipeline-cache` | merged | 6,321 pass | tturbo-mac |
| Nov 01 11:56 | `p11-arch3d-panel` | merged | 6,317 pass | darwin-ws3 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 777 |
| bonsai-b | bonsai2-27b | agent | 717 |
| bonsai-ws2-a | bonsai2-27b | agent | 772 |
| bonsai-ws2-b | bonsai2-27b | agent | 756 |
| claude-sonnet-a | sonnet | idle | 67 |
| claude-sonnet-b | sonnet | idle | 62 |
| claude-sonnet-c | sonnet | idle | 14 |
| claude-sonnet-d | sonnet | agent | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 625 |
| kimi-k31 | k3.1-pro-highspeed | agent | 111 |
| mai-flash | mai-code-1.1-flash | idle | 44 |
| tturbo-mac | tturbo-ultraheretic | merging | 538 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
