# arch3d build status

Updated 2026-11-14 19:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 42 of 48 segments merged · 19 of 20 acceptance checks**

10,314 workspace tests passing at the last merge · 1172 segments merged in all · 0 of 15 seats working

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
| 16 | Ceilings and foundations | done | 20 / 20 | 63.2 h |
| 17 | Stairs, decks, railings and openings in detail | done | 14 / 14 | 49.6 h |
| 18 | Interior finish, kitchen and bath | building | 19 / 20 | 85.8 h so far · expected done Nov 16 00:03–Dec 06 07:25 UTC |
| 19 | Light, sky and cameras | building | 12 / 17 | — |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 11 / 13 | — |

## Phase 18 segments

42 merged, 3 queued, 2 replaced, 2 working, 1 abandoned, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p18h-fix-glass-pony-inspect-test` | Fix inspect_walls_rows_glass_and_split to select the 2x4 wall (Glass pony wall breaks walls[0]) | gate | bonsai-a |
| `p18h-fix-outlet-gaps-null-spacing` | outletGaps: emit maxSpacing 0.0 (not null) when a room has no counter run | working | mai-flash-b |
| `p18h-showcase-kitchen-tests` | Add the two phase-18 kitchen-casework showcase tests (digest-driven) | working | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 14 18:54 | `harden-p19h-pt-diffuse-grazing` | merged | 10,314 pass | tturbo-mac |
| Nov 14 18:54 | `harden-p19h-seeing-in-interactive-pass` | merged | 10,314 pass | tturbo-mac |
| Nov 14 18:54 | `harden-p19g-pt-tone-look` | merged | 10,314 pass | tturbo-mac |
| Nov 14 17:28 | `harden-p21f-interior-cabinet-front` | refused | 10,305 pass | tturbo-mac |
| Nov 14 16:55 | `harden-p19j-demo-overview-standard-look` | merged | 10,298 pass | tturbo-mac |
| Nov 14 16:55 | `p19h-pt-diffuse-grazing` | merged | 10,298 pass | mai-flash |
| Nov 14 16:24 | `harden-p19h-cutaway-hide-by-kind` | merged | 10,291 pass | tturbo-mac |
| Nov 14 16:24 | `p19h-seeing-in-interactive-pass` | merged | 10,291 pass | mai-flash-esc |
| Nov 14 15:47 | `p19h-pt-spec-albedo-table` | merged | 10,282 pass | mai-flash-esc |
| Nov 14 15:12 | `harden-p19g-pt-strip-reaches-floor` | merged | 10,277 pass | tturbo-mac |
| Nov 14 14:37 | `harden-p14-io-obj-ifc-furnishing` | merged | 10,272 pass | tturbo-mac2 |
| Nov 14 14:01 | `harden-p14-arch3d-paint` | merged | 10,269 pass | tturbo-mac2 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 1305 |
| bonsai-b | bonsai2-27b | agent | 1245 |
| bonsai-ws2-a | bonsai2-27b | agent | 1380 |
| bonsai-ws2-b | bonsai2-27b | agent | 1354 |
| claude-sonnet-a | sonnet | down | 157 |
| claude-sonnet-b | sonnet | down | 156 |
| claude-sonnet-c | sonnet | down | 94 |
| claude-sonnet-d | sonnet | down | 92 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1201 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 121 |
| mai-flash-b | mai-code-1.1-flash | agent | 73 |
| mai-flash-esc | mai-code-1.1-flash | agent | 141 |
| tturbo-mac | tturbo-ultraheretic | agent | 903 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 577 |
