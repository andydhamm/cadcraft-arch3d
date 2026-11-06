# arch3d build status

Updated 2026-11-06 03:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 15 of 39 segments merged · 1 of 14 acceptance checks**

7,548 workspace tests passing at the last merge · 776 segments merged in all · 0 of 14 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 1 / 14 | 18.8 h so far · expected done Nov 07 02:17–Nov 08 08:00 UTC |

## Phase 15 segments

16 queued, 15 merged, 6 merging, 3 replaced, 1 escalate, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-arch-grade` | arch: grade classes and the above-grade area | merging | claude-sonnet-b |
| `p15-arch-pad-split` | arch: the pad follows each level and the entry landing | merging | claude-sonnet-c |
| `p15-arch3d-spanning-walls` | arch3d: walls from one level's floor to another's ceiling, offset levels | merging | claude-sonnet-d |
| `p15-cmd-levels` | engine: free level elevations, level kinds, spanning walls, room types, beds | escalate |  |
| `p15-arch3d-plate-mesh` | arch3d: the electrical cover-plate mesh, box and material | merging | mai-flash |
| `p15-arch-outlet-runs` | arch: usable wall runs of a room (openings and counters break them) | merging | claude-sonnet-a |
| `p15-arch-connection-arcs` | arch: connection validation and bulged connection arcs | working | bonsai-ws2-b |
| `upstream-sync-fc157ea91` | merge upstream upstream/main up to fc157ea91 (4 commits) | merging | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 06 02:23 | `harden-p15-doc-levels` | merged | 7,548 pass | tturbo-mac |
| Nov 06 01:02 | `p15-cmd-electrical-view` | merged | 7,539 pass | mai-flash-esc |
| Nov 06 00:14 | `p15-cmd-library-copy-place` | merged | 7,536 pass | claude-sonnet-a |
| Nov 05 23:19 | `p15-cmd-light-sets` | merged | 7,530 pass | mai-flash |
| Nov 05 22:18 | `p15-render-plan-level` | merged | 7,527 pass | claude-sonnet-d |
| Nov 05 21:17 | `p15-arch-spanning-walls` | merged | 7,524 pass | claude-sonnet-b |
| Nov 05 20:17 | `upstream-sync-df6597445` | merged | 7,520 pass | claude-sonnet-c |
| Nov 05 19:19 | `harden-p15-i18n-labels` | merged | 7,512 pass | tturbo-mac |
| Nov 05 18:12 | `p15-arch-electrical` | merged | 7,508 pass | mai-flash-esc |
| Nov 05 16:48 | `p15-arch3d-light-brightness` | merged | 7,496 pass | claude-sonnet-a |
| Nov 05 16:19 | `p15-render3d-technical` | merged | 7,493 pass | darwin-ws3 |
| Nov 05 15:51 | `p15-cli-commands-file` | merged | 7,491 pass | bonsai-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 920 |
| bonsai-b | bonsai2-27b | merging | 845 |
| bonsai-ws2-a | bonsai2-27b | gate | 949 |
| bonsai-ws2-b | bonsai2-27b | agent | 909 |
| claude-sonnet-a | sonnet | merging | 83 |
| claude-sonnet-b | sonnet | merging | 83 |
| claude-sonnet-c | sonnet | merging | 36 |
| claude-sonnet-d | sonnet | merging | 22 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 787 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | merging | 63 |
| mai-flash-esc | mai-code-1.1-flash | idle | 33 |
| tturbo-mac | tturbo-ultraheretic | merging | 623 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
