# arch3d build status

Updated 2026-11-13 00:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 26 of 35 segments merged · 7 of 20 acceptance checks**

9,316 workspace tests passing at the last merge · 1015 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 7 / 20 | 42.8 h so far · expected done Nov 13 06:25–Nov 14 00:10 UTC |
| 19 | Light, sky and cameras | building | 2 / 17 | — |
| 20 | Plan views and dimensions | building | 6 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 1 / 13 | — |

## Phase 18 segments

26 merged, 4 merging, 3 queued, 2 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-room-finishes-3` | Room-type finishes: Concrete / Tile / Carpet / Drywall / Plaster finishes and a room-type template of floor, wall and ceiling materials that rooms take when detected or retyped, painter edits winning | merging | mai-flash |
| `p18-crown-wrap` | Crown wraps soffits and trays and follows the finished ceiling (vaults, stacks); a soffit can opt out of room moldings | merging | mai-flash |
| `p18-countertop-shapes` | Countertop shapes: edges bulge into arcs (a sagitta, with a radius grip), take an edge profile, and an end turns down to the floor as a waterfall of the same material and thickness | working | mai-flash-b |
| `p18-backsplash` | Backsplash: `backsplash {run | wall}` covers the wall face behind a counter from the counter top to the underside of the wall cabinets (else a set height, default 0.45), 10 mm thick, its own material, top to the ceiling on demand, per-cabinet on / off | merging | bonsai-ws2-a |
| `p18-face-appliances` | Face appliances: a microwave drawer, wall oven or warming drawer placed on a cabinet face replaces that front item at the appliance's height; a tall cabinet stacks two | working | mai-flash |
| `p18-corners` | Corner cabinets: a base or wall cabinet in an inside corner where two runs meet becomes blind, diagonal-door or lazy-susan (a plan symbol with its cut-out), and the counter runs round the corner in one piece | merging | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 13 00:03 | `p18-countertops` | merged | 9,316 pass | claude-sonnet-b |
| Nov 12 23:18 | `p18-corners` | refused | 0 pass | claude-sonnet-d |
| Nov 12 23:18 | `p18-cabinet-fronts-2` | merged | 9,294 pass | claude-sonnet-c |
| Nov 12 22:57 | `p20-room-labels` | refused | 0 pass | mai-flash-b |
| Nov 12 22:57 | `p19-cameras-4` | merged | 9,288 pass, 1 fail | darwin-ws3 |
| Nov 12 22:57 | `p20-reference` | merged | 9,288 pass, 1 fail | mai-flash |
| Nov 12 22:57 | `p20-view-export` | merged | 9,288 pass, 1 fail | claude-sonnet-b |
| Nov 12 22:57 | `p18-shower-niche-4` | merged | 9,288 pass, 1 fail | mai-flash-b |
| Nov 12 22:34 | `p18-countertops` | refused | 9,293 pass | claude-sonnet-d |
| Nov 12 22:07 | `p19-cameras-3` | refused | 0 pass | bonsai-ws2-a |
| Nov 12 22:07 | `p20-auto-dims-2` | merged | 9,272 pass | claude-sonnet-b |
| Nov 12 22:07 | `p18-molding-lines-2` | merged | 9,272 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 1212 |
| bonsai-b | bonsai2-27b | gate | 1171 |
| bonsai-ws2-a | bonsai2-27b | merging | 1290 |
| bonsai-ws2-b | bonsai2-27b | agent | 1244 |
| claude-sonnet-a | sonnet | down | 145 |
| claude-sonnet-b | sonnet | idle | 153 |
| claude-sonnet-c | sonnet | idle | 88 |
| claude-sonnet-d | sonnet | down | 81 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 1077 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 102 |
| mai-flash-b | mai-code-1.1-flash | agent | 47 |
| mai-flash-esc | mai-code-1.1-flash | agent | 108 |
| tturbo-mac | tturbo-ultraheretic | gate | 818 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 430 |
