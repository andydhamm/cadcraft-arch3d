# arch3d build status

Updated 2026-11-12 08:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 9 of 35 segments merged · 1 of 20 acceptance checks**

8,907 workspace tests passing at the last merge · 971 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 1 / 20 | 26.8 h so far · expected done Nov 12 15:42–Nov 17 21:10 UTC |
| 19 | Light, sky and cameras | building | 2 / 17 | — |
| 20 | Plan views and dimensions | building | 1 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 0 / 13 | — |

## Phase 18 segments

16 queued, 10 merging, 9 merged

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-room-finishes-1` | Room-type finishes: Concrete / Tile / Carpet / Drywall / Plaster finishes and a room-type template of floor, wall and ceiling materials that rooms take when detected or retyped, painter edits winning | merging | claude-sonnet-b |
| `p18-room-moldings` | Room moldings: each room's base, chair rail and crown along its wall faces, base and chair stopping at opening trim, crown round a flat ceiling; `room.moldings`, the trim widths in arch.inspect, the brief's molding numbers | merging | claude-sonnet-c |
| `p18-molding-lines-1` | Molding lines: `moldingline` sweeps a profile along a drawn 2D path at an elevation (a tray's edge, a wainscot cap) with mitred corners, drawn in plan, inspectable | merging | mai-flash-esc |
| `p18-cabinet-moldings` | Cabinet moldings: crown and light rail on wall (and tall) cabinets from a profile with a size override, following the door faces or the box front where there is no door, returning at exposed ends | merging | mai-flash-esc |
| `p18-countertops` | Countertops: `countertop.detach` turns an automatic counter into a custom one that keeps its outline, `countertop` draws one from scratch, edges move by a typed distance, sink cut-outs carry over; drawn in plan and saved as DXF proxies | merging | claude-sonnet-d |
| `p18-backsplash` | Backsplash: `backsplash {run | wall}` covers the wall face behind a counter from the counter top to the underside of the wall cabinets (else a set height, default 0.45), 10 mm thick, its own material, top to the ceiling on demand, per-cabinet on / off | merging | claude-sonnet-a |
| `p18-cabinet-fronts-1` | Cabinet construction and face layouts: framed or frameless boxes, overlay or inset fronts, `cabinet.face` rows of door / drawer / false / opening items with heights, split across or down, door pairs above 0.686 m (27 in) | merging | bonsai-b |
| `p18-corners` | Corner cabinets: a base or wall cabinet in an inside corner where two runs meet becomes blind, diagonal-door or lazy-susan (a plan symbol with its cut-out), and the counter runs round the corner in one piece | merging | claude-sonnet-c |
| `p18-fillers-shelves` | Fillers and open shelves: `cabinet.fill` closes a run's gap to a wall with a filler up to 0.15 m (larger gaps reported, never stretched); open wall cabinets with an automatic or given shelf count, evenly spaced | merging | mai-flash-b |
| `p18-glass-walls-1` | Glass and pony walls: wall type "Glass" (10 mm clear glass, no poché fill, its own material, default height 2.0 m) and `wall.split` into a lower and an upper type (a solid base with glass above) | merging | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 12 08:10 | `p20-plan-views` | merged | 8,907 pass | mai-flash |
| Nov 12 08:10 | `p21-observables-1` | merged | 8,907 pass | claude-sonnet-b |
| Nov 12 07:50 | `p19-sun-1` | merged | 8,877 pass | mai-flash |
| Nov 12 07:30 | `p19-seeing-in-render-2` | merged | 8,875 pass | claude-sonnet-d |
| Nov 12 07:30 | `harden-p15-showcase-lib-io` | merged | 8,875 pass | tturbo-mac |
| Nov 12 07:01 | `p18-glass-walls-1` | refused | 8,879 pass | mai-flash |
| Nov 12 06:42 | `upstream-sync-ca87680fb` | merged | 8,871 pass | claude-sonnet-a |
| Nov 12 05:52 | `p18-glass-walls-1` | refused | 8,870 pass | claude-sonnet-d |
| Nov 12 05:32 | `p19-normal-maps-2` | merged | 8,862 pass | mai-flash-b |
| Nov 12 04:42 | `upstream-sync-e5021f017` | merged | 8,858 pass | claude-sonnet-d |
| Nov 12 03:58 | `p19-seeing-in` | merged | 8,837 pass | mai-flash-esc |
| Nov 12 03:58 | `harden-p16-vault-walls` | merged | 8,837 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1159 |
| bonsai-b | bonsai2-27b | agent | 1129 |
| bonsai-ws2-a | bonsai2-27b | agent | 1228 |
| bonsai-ws2-b | bonsai2-27b | agent | 1171 |
| claude-sonnet-a | sonnet | agent | 131 |
| claude-sonnet-b | sonnet | agent | 139 |
| claude-sonnet-c | sonnet | agent | 72 |
| claude-sonnet-d | sonnet | idle | 63 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1027 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 90 |
| mai-flash-b | mai-code-1.1-flash | gate | 30 |
| mai-flash-esc | mai-code-1.1-flash | idle | 95 |
| tturbo-mac | tturbo-ultraheretic | idle | 795 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 399 |
