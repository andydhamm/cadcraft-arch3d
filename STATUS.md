# arch3d build status

Updated 2026-11-06 11:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 25 of 43 segments merged · 1 of 14 acceptance checks**

7,583 workspace tests passing at the last merge · 787 segments merged in all · 0 of 14 seats working

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
| 14 | The library | done | 13 / 13 | 36.9 h |
| 15 | Electrical, technical illustration and the showcase house | building | 1 / 14 | 26.8 h so far · expected done Nov 07 00:32–Nov 07 20:54 UTC |

## Phase 15 segments

25 merged, 10 queued, 7 merging, 4 replaced, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-cmd-electrical` | engine: outlet, switch, electrical.edit, electrical.delete | working | mai-flash |
| `p15-arch-outlet-place` | arch: auto_outlets — duplex rule and kitchen GFCI counters | merging | claude-sonnet-a |
| `p15-arch-circuits` | arch: circuit grouping by shared lights | merging | bonsai-ws2-b |
| `p15-cmd-level-free` | level: free elevations, kinds, and level.edit | merging | mai-flash-esc |
| `p15-cmd-wall-span` | wall: level and topLevel params, derived spanning height | merging | claude-sonnet-b |
| `p15-cmd-room-bed` | room type keys and the terrain planting bed | merging | bonsai-a |
| `upstream-sync-2611bc5f5` | merge upstream upstream/main up to 2611bc5f5 (44 commits) | merging | bonsai-ws2-a |
| `p15-cli-render-styles` | cli: the clay and technical render styles reach the renderer | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 06 11:08 | `p15-io-electrical-node` | merged | 7,583 pass | mai-flash |
| Nov 06 10:41 | `p15-arch3d-electrical-scene` | merged | 7,582 pass | claude-sonnet-c |
| Nov 06 09:40 | `p15-io-split-levels` | merged | 7,579 pass | claude-sonnet-d |
| Nov 06 08:34 | `p15-arch-connection-arcs` | merged | 7,576 pass | bonsai-ws2-b |
| Nov 06 07:40 | `upstream-sync-fc157ea91` | merged | 7,573 pass | bonsai-a |
| Nov 06 06:55 | `harden-p15-cmd-library-copy-place` | merged | 7,570 pass | tturbo-mac |
| Nov 06 06:22 | `p15-arch-outlet-runs` | merged | 7,566 pass | claude-sonnet-a |
| Nov 06 05:27 | `p15-arch3d-plate-mesh` | merged | 7,563 pass | mai-flash |
| Nov 06 04:47 | `p15-arch3d-spanning-walls` | merged | 7,560 pass | claude-sonnet-d |
| Nov 06 04:08 | `p15-arch-pad-split` | merged | 7,556 pass | claude-sonnet-c |
| Nov 06 03:16 | `p15-arch-grade` | merged | 7,552 pass | claude-sonnet-b |
| Nov 06 02:23 | `harden-p15-doc-levels` | merged | 7,548 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 923 |
| bonsai-b | bonsai2-27b | gate | 846 |
| bonsai-ws2-a | bonsai2-27b | merging | 956 |
| bonsai-ws2-b | bonsai2-27b | merging | 915 |
| claude-sonnet-a | sonnet | merging | 84 |
| claude-sonnet-b | sonnet | merging | 84 |
| claude-sonnet-c | sonnet | idle | 37 |
| claude-sonnet-d | sonnet | merging | 25 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 794 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 64 |
| mai-flash-esc | mai-code-1.1-flash | merging | 34 |
| tturbo-mac | tturbo-ultraheretic | merging | 626 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
