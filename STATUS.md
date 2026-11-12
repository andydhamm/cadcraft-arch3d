# arch3d build status

Updated 2026-11-12 18:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 17 of 35 segments merged · 4 of 20 acceptance checks**

9,182 workspace tests passing at the last merge · 999 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 4 / 20 | 36.5 h so far · expected done Nov 13 15:36–Nov 15 04:45 UTC |
| 19 | Light, sky and cameras | building | 2 / 17 | — |
| 20 | Plan views and dimensions | building | 3 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 1 / 13 | — |

## Phase 18 segments

17 merged, 9 queued, 8 merging, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-room-finishes-1` | Room-type finishes: Concrete / Tile / Carpet / Drywall / Plaster finishes and a room-type template of floor, wall and ceiling materials that rooms take when detected or retyped, painter edits winning | merging | claude-sonnet-d |
| `p18-crown-wrap` | Crown wraps soffits and trays and follows the finished ceiling (vaults, stacks); a soffit can opt out of room moldings | merging | mai-flash |
| `p18-molding-lines-1` | Molding lines: `moldingline` sweeps a profile along a drawn 2D path at an elevation (a tray's edge, a wainscot cap) with mitred corners, drawn in plan, inspectable | merging | mai-flash-esc |
| `p18-countertops` | Countertops: `countertop.detach` turns an automatic counter into a custom one that keeps its outline, `countertop` draws one from scratch, edges move by a typed distance, sink cut-outs carry over; drawn in plan and saved as DXF proxies | merging | claude-sonnet-d |
| `p18-backsplash` | Backsplash: `backsplash {run | wall}` covers the wall face behind a counter from the counter top to the underside of the wall cabinets (else a set height, default 0.45), 10 mm thick, its own material, top to the ceiling on demand, per-cabinet on / off | merging | claude-sonnet-a |
| `p18-cabinet-fronts-2` | Cabinet construction and face layouts: framed or frameless boxes, overlay or inset fronts, `cabinet.face` rows of door / drawer / false / opening items with heights, split across or down, door pairs above 0.686 m (27 in) | merging | claude-sonnet-d |
| `p18-corners` | Corner cabinets: a base or wall cabinet in an inside corner where two runs meet becomes blind, diagonal-door or lazy-susan (a plan symbol with its cut-out), and the counter runs round the corner in one piece | merging | claude-sonnet-c |
| `p18-fillers-shelves` | Fillers and open shelves: `cabinet.fill` closes a run's gap to a wall with a filler up to 0.15 m (larger gaps reported, never stretched); open wall cabinets with an automatic or given shelf count, evenly spaced | merging | claude-sonnet-d |
| `p18-shower-niche-3` | Bath openings: a "Shower" door style (frameless glass, swings both ways) for glass walls, and wall niches: opening kind `niche`, recessed `depth` from one face with a width, height, sill and back material, refused unless the wall is 12 mm thicker; no molding on glass | gate | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 12 17:41 | `p18-room-finishes-1` | refused | 0 pass | claude-sonnet-b |
| Nov 12 17:41 | `p18-shower-niche-2` | merged | 9,182 pass | claude-sonnet-d |
| Nov 12 17:19 | `p19-env-path` | merged | 9,172 pass, 1 fail | claude-sonnet-b |
| Nov 12 17:03 | `p21-schedule-model` | merged | 9,166 pass | mai-flash-esc |
| Nov 12 17:03 | `p21-sheet-command` | merged | 9,166 pass | claude-sonnet-c |
| Nov 12 17:03 | `p18-shower-niche-1` | merged | 9,166 pass | claude-sonnet-c |
| Nov 12 16:40 | `p20-auto-dims-1` | merged | 9,126 pass | mai-flash |
| Nov 12 16:40 | `p19-strip-model-2` | merged | 9,126 pass | claude-sonnet-a |
| Nov 12 16:19 | `p18-molding-lines-1` | refused | 9,120 pass | mai-flash-esc |
| Nov 12 15:31 | `p19-env-path` | refused | 0 pass | claude-sonnet-b |
| Nov 12 15:31 | `p18-glass-walls-2` | merged | 9,112 pass | claude-sonnet-c |
| Nov 12 15:12 | `p21-view-plumbing` | merged | 9,102 pass | claude-sonnet-d |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1195 |
| bonsai-b | bonsai2-27b | agent | 1155 |
| bonsai-ws2-a | bonsai2-27b | agent | 1268 |
| bonsai-ws2-b | bonsai2-27b | agent | 1222 |
| claude-sonnet-a | sonnet | idle | 140 |
| claude-sonnet-b | sonnet | idle | 147 |
| claude-sonnet-c | sonnet | gate | 83 |
| claude-sonnet-d | sonnet | idle | 77 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1061 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | gate | 99 |
| mai-flash-b | mai-code-1.1-flash | gate | 40 |
| mai-flash-esc | mai-code-1.1-flash | agent | 103 |
| tturbo-mac | tturbo-ultraheretic | agent | 806 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 415 |
