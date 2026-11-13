# arch3d build status

Updated 2026-11-13 02:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 28 of 35 segments merged · 8 of 20 acceptance checks**

9,390 workspace tests passing at the last merge · 1022 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 8 / 20 | 45.3 h so far · expected done Nov 13 07:33–Nov 14 00:32 UTC |
| 19 | Light, sky and cameras | building | 2 / 17 | — |
| 20 | Plan views and dimensions | building | 7 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 1 / 13 | — |

## Phase 18 segments

28 merged, 3 merging, 3 queued, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-room-finishes-3` | Room-type finishes: Concrete / Tile / Carpet / Drywall / Plaster finishes and a room-type template of floor, wall and ceiling materials that rooms take when detected or retyped, painter edits winning | merging | mai-flash |
| `p18-crown-wrap` | Crown wraps soffits and trays and follows the finished ceiling (vaults, stacks); a soffit can opt out of room moldings | merging | mai-flash |
| `p18-countertop-shapes` | Countertop shapes: edges bulge into arcs (a sagitta, with a radius grip), take an edge profile, and an end turns down to the floor as a waterfall of the same material and thickness | working | mai-flash-b |
| `p18-face-appliances` | Face appliances: a microwave drawer, wall oven or warming drawer placed on a cabinet face replaces that front item at the appliance's height; a tall cabinet stacks two | merging | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 13 01:39 | `p18-backsplash` | merged | 9,390 pass | claude-sonnet-c |
| Nov 13 01:03 | `p19-normal-maps-render` | refused | 0 pass | claude-sonnet-d |
| Nov 13 01:03 | `p19-sky-2` | merged | 9,371 pass | claude-sonnet-b |
| Nov 13 01:03 | `p21-callouts` | merged | 9,371 pass | darwin-ws3 |
| Nov 13 01:03 | `p19-cameras-2` | merged | 9,371 pass | tturbo-mac |
| Nov 13 01:03 | `p18-corners` | merged | 9,371 pass | claude-sonnet-c |
| Nov 13 01:03 | `p20-room-labels` | merged | 9,371 pass | claude-sonnet-a |
| Nov 13 01:03 | `harden-p12-ifc-roofs` | merged | 9,371 pass | tturbo-mac |
| Nov 13 00:24 | `p18-backsplash` | refused | 9,334 pass | bonsai-ws2-a |
| Nov 13 00:03 | `p18-countertops` | merged | 9,316 pass | claude-sonnet-b |
| Nov 12 23:18 | `p18-corners` | refused | 0 pass | claude-sonnet-d |
| Nov 12 23:18 | `p18-cabinet-fronts-2` | merged | 9,294 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1221 |
| bonsai-b | bonsai2-27b | idle | 1179 |
| bonsai-ws2-a | bonsai2-27b | idle | 1302 |
| bonsai-ws2-b | bonsai2-27b | idle | 1249 |
| claude-sonnet-a | sonnet | idle | 146 |
| claude-sonnet-b | sonnet | idle | 153 |
| claude-sonnet-c | sonnet | idle | 89 |
| claude-sonnet-d | sonnet | idle | 82 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1092 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 104 |
| mai-flash-b | mai-code-1.1-flash | agent | 48 |
| mai-flash-esc | mai-code-1.1-flash | idle | 111 |
| tturbo-mac | tturbo-ultraheretic | gate | 822 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 440 |
