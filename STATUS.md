# arch3d build status

Updated 2026-11-06 06:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 20 of 39 segments merged · 1 of 14 acceptance checks**

7,566 workspace tests passing at the last merge · 781 segments merged in all · 0 of 13 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 1 / 14 | 22.3 h so far · expected done Nov 06 23:20–Nov 07 23:53 UTC |

## Phase 15 segments

20 merged, 12 queued, 6 merging, 3 replaced, 1 stuck

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-io-split-levels` | io: offset levels and spanning walls in IFC | merging | claude-sonnet-d |
| `p15-arch3d-electrical-scene` | arch3d: electrical plates in the scene, the cache and the probe | merging | claude-sonnet-c |
| `p15-io-electrical-node` | io: GLB/OBJ node names for electrical cover plates | merging | mai-flash |
| `p15-arch-outlet-place` | arch: auto_outlets — duplex rule and kitchen GFCI counters | merging | claude-sonnet-a |
| `p15-arch-connection-arcs` | arch: connection validation and bulged connection arcs | merging | bonsai-ws2-b |
| `upstream-sync-fc157ea91` | merge upstream upstream/main up to fc157ea91 (4 commits) | merging | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 06 06:22 | `p15-arch-outlet-runs` | merged | 7,566 pass | claude-sonnet-a |
| Nov 06 05:27 | `p15-arch3d-plate-mesh` | merged | 7,563 pass | mai-flash |
| Nov 06 04:47 | `p15-arch3d-spanning-walls` | merged | 7,560 pass | claude-sonnet-d |
| Nov 06 04:08 | `p15-arch-pad-split` | merged | 7,556 pass | claude-sonnet-c |
| Nov 06 03:16 | `p15-arch-grade` | merged | 7,552 pass | claude-sonnet-b |
| Nov 06 02:23 | `harden-p15-doc-levels` | merged | 7,548 pass | tturbo-mac |
| Nov 06 01:02 | `p15-cmd-electrical-view` | merged | 7,539 pass | mai-flash-esc |
| Nov 06 00:14 | `p15-cmd-library-copy-place` | merged | 7,536 pass | claude-sonnet-a |
| Nov 05 23:19 | `p15-cmd-light-sets` | merged | 7,530 pass | mai-flash |
| Nov 05 22:18 | `p15-render-plan-level` | merged | 7,527 pass | claude-sonnet-d |
| Nov 05 21:17 | `p15-arch-spanning-walls` | merged | 7,524 pass | claude-sonnet-b |
| Nov 05 20:17 | `upstream-sync-df6597445` | merged | 7,520 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 920 |
| bonsai-b | bonsai2-27b | merging | 846 |
| bonsai-ws2-a | bonsai2-27b | merging | 955 |
| bonsai-ws2-b | bonsai2-27b | merging | 912 |
| claude-sonnet-a | sonnet | merging | 84 |
| claude-sonnet-b | sonnet | idle | 83 |
| claude-sonnet-c | sonnet | merging | 37 |
| claude-sonnet-d | sonnet | merging | 23 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | merging | 64 |
| mai-flash-esc | mai-code-1.1-flash | idle | 33 |
| tturbo-mac | tturbo-ultraheretic | merging | 623 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
