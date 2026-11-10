# arch3d build status

Updated 2026-11-10 05:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 16 of 21: Ceilings and foundations — 17 of 33 segments merged · 9 of 13 acceptance checks**

8,258 workspace tests passing at the last merge · 873 segments merged in all · 0 of 15 seats working

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
| 15 | Electrical, technical illustration and the showcase house | done | 14 / 14 | 115.8 h |
| 16 | Ceilings and foundations | building | 9 / 13 | 26.0 h so far · expected done Nov 10 20:51–Nov 11 20:26 UTC |
| 17 | Stairs, decks, railings and openings in detail | planned | — | — |
| 18 | Interior finish, kitchen and bath | planned | — | — |
| 19 | Light, sky and cameras | planned | — | — |
| 20 | Plan views and dimensions | planned | — | — |
| 21 | Schedules, elevations and sheets | planned | — | — |

## Phase 16 segments

17 merged, 9 queued, 7 working, 3 replaced, 2 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p16f-tray-outline-reply` | tray: the reply's outline is a ring of [x, y] points | working | darwin-ws3 |
| `p16f-tray-roof-guard` | tray validate: a new tray above the roof underside is refused | working | bonsai-ws2-b |
| `p16f-soffit-faces` | soffit: rects snap to the room-side wall face; `over` starts at the cabinets' backs | working | bonsai-b |
| `p16f-slab-edge-mesh` | slab foundation: one footing mesh per founded wall | working | bonsai-ws2-a |
| `p16f-demo-replays` | phase-16 demo: replays without refusals | working | mai-flash |
| `p16-rw-roof-top-level` | roof: the top level is the highest by elevation | working | mai-flash-b |
| `p16-rw-open-below-rooms` | Open Below rooms: drawn ones cut holes, roomdetect keeps them | working | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 10 03:46 | `p16-rebuild` | merged | 8,258 pass | mai-flash-b |
| Nov 10 03:29 | `upstream-sync-a3d9348f6` | merged | 8,256 pass | claude-sonnet-b |
| Nov 10 03:09 | `p16-exports` | merged | 8,252 pass | claude-sonnet-a |
| Nov 10 02:49 | `p16-demo` | merged | 8,243 pass | mai-flash |
| Nov 10 02:32 | `p16-demo` | refused | 0 pass | darwin-ws3 |
| Nov 10 02:32 | `p15-docs-user-guide` | merged | 8,243 pass |  |
| Nov 10 02:10 | `p16-trays-check` | merged | 8,242 pass | bonsai-a |
| Nov 10 01:37 | `p16-render3d-adapter-turns` | merged | 8,241 pass | claude-sonnet-d |
| Nov 10 01:03 | `upstream-sync-d847eac13` | merged | 8,240 pass | claude-sonnet-d |
| Nov 10 00:20 | `p15-docs-user-guide` | refused | 0 pass | mai-flash-esc |
| Nov 10 00:20 | `harden-p15-showcase-test` | merged | 8,197 pass | tturbo-mac |
| Nov 09 23:14 | `p15-check10-surfaces` | refused | 0 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1018 |
| bonsai-b | bonsai2-27b | agent | 963 |
| bonsai-ws2-a | bonsai2-27b | agent | 1063 |
| bonsai-ws2-b | bonsai2-27b | agent | 1030 |
| claude-sonnet-a | sonnet | down | 94 |
| claude-sonnet-b | sonnet | down | 99 |
| claude-sonnet-c | sonnet | down | 44 |
| claude-sonnet-d | sonnet | down | 45 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 922 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 70 |
| mai-flash-b | mai-code-1.1-flash | agent | 10 |
| mai-flash-esc | mai-code-1.1-flash | idle | 65 |
| tturbo-mac | tturbo-ultraheretic | agent | 713 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
