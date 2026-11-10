# arch3d build status

Updated 2026-11-10 07:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 16 of 21: Ceilings and foundations — 19 of 33 segments merged · 10 of 20 acceptance checks**

8,259 workspace tests passing at the last merge · 875 segments merged in all · 0 of 15 seats working

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
| 16 | Ceilings and foundations | building | 10 / 20 | 27.5 h so far · expected done Nov 10 19:13–Nov 11 15:40 UTC |
| 17 | Stairs, decks, railings and openings in detail | building | 0 / 14 | — |
| 18 | Interior finish, kitchen and bath | planned | — | — |
| 19 | Light, sky and cameras | planned | — | — |
| 20 | Plan views and dimensions | planned | — | — |
| 21 | Schedules, elevations and sheets | planned | — | — |

## Phase 16 segments

19 merged, 10 merging, 4 queued, 3 replaced, 2 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p16f-tray-outline-reply` | tray: the reply's outline is a ring of [x, y] points | merging | darwin-ws3 |
| `p16f-tray-roof-guard` | tray validate: a new tray above the roof underside is refused | merging | bonsai-ws2-b |
| `p16f-soffit-faces` | soffit: rects snap to the room-side wall face; `over` starts at the cabinets' backs | merging | bonsai-b |
| `p16f-slab-edge-mesh` | slab foundation: one footing mesh per founded wall | merging | bonsai-ws2-a |
| `p16-rw-stair-wells` | stairs: levels ordered by elevation, wells measured from the stair's base | merging | claude-sonnet-b |
| `p16-rw-open-below-rooms` | Open Below rooms: drawn ones cut holes, roomdetect keeps them | merging | mai-flash-esc |
| `p16-rw-overhead-arch` | arch: what lies over a point (the level above's floor, or an Open Below) clips ceilings and walls | merging | claude-sonnet-d |
| `p16-rw-above-grade-living` | above-grade area: Chief's living-area convention | merging | claude-sonnet-c |
| `p16-rw-technical-silhouettes` | technical illustration: silhouettes for the camera's view direction | merging | claude-sonnet-a |
| `p16-rw-pane-styles` | 3D pane: Clay and Technical in the style selector | merging | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 10 06:51 | `p16-rw-roof-top-level` | merged | 8,259 pass | mai-flash-b |
| Nov 10 06:10 | `p16f-demo-replays` | merged | 8,258 pass | mai-flash |
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

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1023 |
| bonsai-b | bonsai2-27b | agent | 966 |
| bonsai-ws2-a | bonsai2-27b | agent | 1070 |
| bonsai-ws2-b | bonsai2-27b | gate | 1036 |
| claude-sonnet-a | sonnet | idle | 98 |
| claude-sonnet-b | sonnet | idle | 102 |
| claude-sonnet-c | sonnet | gate | 47 |
| claude-sonnet-d | sonnet | idle | 46 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 924 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 71 |
| mai-flash-b | mai-code-1.1-flash | idle | 11 |
| mai-flash-esc | mai-code-1.1-flash | idle | 66 |
| tturbo-mac | tturbo-ultraheretic | agent | 715 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
