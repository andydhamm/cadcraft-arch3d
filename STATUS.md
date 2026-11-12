# arch3d build status

Updated 2026-11-12 22:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 21 of 35 segments merged · 5 of 20 acceptance checks**

9,244 workspace tests passing at the last merge · 1005 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 5 / 20 | 40.5 h so far · expected done Nov 13 11:26–Nov 14 15:00 UTC |
| 19 | Light, sky and cameras | building | 2 / 17 | — |
| 20 | Plan views and dimensions | building | 3 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 1 / 13 | — |

## Phase 18 segments

21 merged, 6 merging, 6 queued, 2 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-room-finishes-2` | Room-type finishes: Concrete / Tile / Carpet / Drywall / Plaster finishes and a room-type template of floor, wall and ceiling materials that rooms take when detected or retyped, painter edits winning | merging | mai-flash |
| `p18-crown-wrap` | Crown wraps soffits and trays and follows the finished ceiling (vaults, stacks); a soffit can opt out of room moldings | merging | mai-flash |
| `p18-molding-lines-2` | Molding lines: `moldingline` sweeps a profile along a drawn 2D path at an elevation (a tray's edge, a wainscot cap) with mitred corners, drawn in plan, inspectable | merging | claude-sonnet-c |
| `p18-countertops` | Countertops: `countertop.detach` turns an automatic counter into a custom one that keeps its outline, `countertop` draws one from scratch, edges move by a typed distance, sink cut-outs carry over; drawn in plan and saved as DXF proxies | merging | claude-sonnet-d |
| `p18-backsplash` | Backsplash: `backsplash {run | wall}` covers the wall face behind a counter from the counter top to the underside of the wall cabinets (else a set height, default 0.45), 10 mm thick, its own material, top to the ceiling on demand, per-cabinet on / off | merging | bonsai-ws2-a |
| `p18-cabinet-fronts-2` | Cabinet construction and face layouts: framed or frameless boxes, overlay or inset fronts, `cabinet.face` rows of door / drawer / false / opening items with heights, split across or down, door pairs above 0.686 m (27 in) | gate | claude-sonnet-c |
| `p18-corners` | Corner cabinets: a base or wall cabinet in an inside corner where two runs meet becomes blind, diagonal-door or lazy-susan (a plan symbol with its cut-out), and the counter runs round the corner in one piece | gate | claude-sonnet-d |
| `p18-shower-niche-4` | Bath openings: a "Shower" door style (frameless glass, swings both ways) for glass walls, and wall niches: opening kind `niche`, recessed `depth` from one face with a width, height, sill and back material, refused unless the wall is 12 mm thicker; no molding on glass | merging | mai-flash-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 12 20:39 | `p18-cabinet-fronts-2` | refused | 0 pass | bonsai-a |
| Nov 12 20:39 | `p18-fillers-shelves` | merged | 9,244 pass | claude-sonnet-d |
| Nov 12 19:46 | `p20-room-labels` | refused | 0 pass | mai-flash-esc |
| Nov 12 19:46 | `p18-corners` | refused | 0 pass | claude-sonnet-c |
| Nov 12 19:46 | `p18-molding-lines-2` | refused | 0 pass | mai-flash-b |
| Nov 12 19:46 | `p18-countertops` | refused | 0 pass | claude-sonnet-a |
| Nov 12 19:46 | `p19-sky-1` | merged | 9,224 pass | claude-sonnet-c |
| Nov 12 19:46 | `p18-shower-niche-3` | merged | 9,224 pass | claude-sonnet-c |
| Nov 12 19:25 | `p19-horizon-2` | refused | 9,215 pass | claude-sonnet-b |
| Nov 12 18:49 | `p18-backsplash` | refused | 0 pass | claude-sonnet-a |
| Nov 12 18:49 | `p18-countertops` | refused | 0 pass | claude-sonnet-d |
| Nov 12 18:49 | `p18-cabinet-fronts-2` | refused | 0 pass | claude-sonnet-d |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1203 |
| bonsai-b | bonsai2-27b | agent | 1163 |
| bonsai-ws2-a | bonsai2-27b | agent | 1281 |
| bonsai-ws2-b | bonsai2-27b | agent | 1238 |
| claude-sonnet-a | sonnet | agent | 143 |
| claude-sonnet-b | sonnet | agent | 150 |
| claude-sonnet-c | sonnet | gate | 86 |
| claude-sonnet-d | sonnet | gate | 79 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1071 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 101 |
| mai-flash-b | mai-code-1.1-flash | gate | 45 |
| mai-flash-esc | mai-code-1.1-flash | idle | 107 |
| tturbo-mac | tturbo-ultraheretic | agent | 812 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 425 |
