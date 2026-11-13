# arch3d build status

Updated 2026-11-13 16:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 36 of 42 segments merged · 13 of 20 acceptance checks**

9,843 workspace tests passing at the last merge · 1086 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 13 / 20 | 58.8 h so far · expected done Nov 14 01:51–Nov 15 09:06 UTC |
| 19 | Light, sky and cameras | building | 6 / 17 | — |
| 20 | Plan views and dimensions | building | 9 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 6 / 13 | — |

## Phase 18 segments

36 merged, 2 working, 2 queued, 1 merging, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p18g-crown-soffit-tray` | Room crown really wraps soffits (with opt-out) and runs round trays | working | darwin-ws3 |
| `p18g-countertop-nose` | Countertop edge profiles round the slab's edge instead of overlaying a full square slab | merging | claude-sonnet-c |
| `p18g-fill-labels-shelves` | cabinet.fill replies with cabinet labels; open shelves are meshed where arch.inspect reports them | working | claude-sonnet-b |
| `p18g-room-moldings-refusals` | room.moldings honours `profile`, the 8-molding limit, and refuses before touching the drawing | gate | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 13 15:56 | `harden-p15-cmd-room-bed` | merged | 9,843 pass | tturbo-mac |
| Nov 13 15:56 | `harden-p19-cameras-3` | merged | 9,843 pass | tturbo-mac |
| Nov 13 15:56 | `harden-p19-hdri-2` | merged | 9,843 pass | tturbo-mac |
| Nov 13 15:56 | `harden-p8-detect-tune-to-targets` | merged | 9,843 pass | tturbo-mac |
| Nov 13 15:56 | `harden-p11-cmd-render-path` | merged | 9,843 pass | tturbo-mac2 |
| Nov 13 15:56 | `harden-p12-ifc-stairs` | merged | 9,843 pass | tturbo-mac2 |
| Nov 13 15:22 | `upstream-sync-ddfa833bc` | merged | 9,799 pass | claude-sonnet-a |
| Nov 13 14:46 | `p19-demo` | merged | 9,789 pass | mai-flash-b |
| Nov 13 14:46 | `p18-rebuild` | merged | 9,789 pass | mai-flash-esc |
| Nov 13 14:46 | `p18-exports` | merged | 9,789 pass | mai-flash-esc |
| Nov 13 14:46 | `p18-demo` | merged | 9,789 pass | mai-flash |
| Nov 13 14:46 | `p19-pane` | merged | 9,789 pass | mai-flash-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1248 |
| bonsai-b | bonsai2-27b | agent | 1197 |
| bonsai-ws2-a | bonsai2-27b | agent | 1319 |
| bonsai-ws2-b | bonsai2-27b | agent | 1279 |
| claude-sonnet-a | sonnet | agent | 151 |
| claude-sonnet-b | sonnet | agent | 153 |
| claude-sonnet-c | sonnet | idle | 91 |
| claude-sonnet-d | sonnet | idle | 82 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1118 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | gate | 109 |
| mai-flash-b | mai-code-1.1-flash | idle | 53 |
| mai-flash-esc | mai-code-1.1-flash | idle | 121 |
| tturbo-mac | tturbo-ultraheretic | agent | 857 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 485 |
