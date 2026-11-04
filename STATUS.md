# arch3d build status

Updated 2026-11-04 18:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 14 of 15: The library — 23 of 34 segments merged · 1 of 13 acceptance checks**

7,287 workspace tests passing at the last merge · 739 segments merged in all · 0 of 14 seats working · PAUSED

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
| 14 | The library | building | 1 / 13 | 23.0 h so far · expected done Nov 05 14:48–Nov 07 15:39 UTC |
| 15 | Electrical, technical illustration and the showcase house | planned | — | — |

## Phase 14 segments

23 merged, 5 queued, 3 merging, 2 escalate, 1 gate, 1 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p14-arch3d-cabinet-parts` | Front styles, pulls, counter runs and cut-outs in 3D | escalate |  |
| `p14-arch3d-paint` | Painted surfaces in 3D: split faces, paint materials, tints | merging | claude-sonnet-b |
| `p14-cmd-place` | library.place, furnishing.edit / .delete / .centre | escalate |  |
| `p14-cmd-backdrop` | The backdrop command | merging | claude-sonnet-c |
| `p14-r3d-pt-backdrop` | The path tracer shows the backdrop where camera rays miss | gate | bonsai-b |
| `p14-ui-library-panel` | The Library side panel: categories, folders, search, chips, history, tags, details | merging | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 04 17:56 | `harden-p14-arch-paint` | merged | 7,287 pass | tturbo-mac |
| Nov 04 17:22 | `p14-r3d-backdrop` | merged | 7,268 pass | claude-sonnet-d |
| Nov 04 16:30 | `p14-arch3d-cabinet-parts` | refused | 7,271 pass | bonsai-a |
| Nov 04 15:43 | `p14-cmd-icons` | merged | 7,261 pass | mai-flash-esc |
| Nov 04 15:10 | `p14-cmd-casework-parts` | merged | 7,254 pass | claude-sonnet-a |
| Nov 04 14:36 | `p14-apps-library-hooks` | merged | 7,250 pass | claude-sonnet-d |
| Nov 04 13:36 | `p14-inspect` | refused | 7,199 pass | claude-sonnet-b |
| Nov 04 12:57 | `p14-io-dxf-furnishing` | refused | 7,199 pass | claude-sonnet-d |
| Nov 04 11:07 | `p14-cmd-furnishing-transform` | refused | 7,200 pass | mai-flash-esc |
| Nov 04 09:45 | `upstream-sync-5f8999f30` | refused | 0 pass | mai-flash |
| Nov 04 09:45 | `p14-arch-place` | merged | 7,194 pass | claude-sonnet-c |
| Nov 04 07:48 | `harden-p13-arch3d-incremental` | refused | 7,175 pass, 3 fail | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 872 |
| bonsai-b | bonsai2-27b | gate | 813 |
| bonsai-ws2-a | bonsai2-27b | gate | 892 |
| bonsai-ws2-b | bonsai2-27b | gate | 861 |
| claude-sonnet-a | sonnet | merging | 77 |
| claude-sonnet-b | sonnet | idle | 80 |
| claude-sonnet-c | sonnet | idle | 32 |
| claude-sonnet-d | sonnet | idle | 19 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 746 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 58 |
| mai-flash-esc | mai-code-1.1-flash | idle | 20 |
| tturbo-mac | tturbo-ultraheretic | merging | 608 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
