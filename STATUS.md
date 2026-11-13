# arch3d build status

Updated 2026-11-13 14:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 18 of 21: Interior finish, kitchen and bath — 32 of 36 segments merged · 11 of 20 acceptance checks**

9,765 workspace tests passing at the last merge · 1073 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | building | 11 / 20 | 56.5 h so far · expected done Nov 13 22:43–Nov 16 13:53 UTC |
| 19 | Light, sky and cameras | building | 4 / 17 | — |
| 20 | Plan views and dimensions | building | 9 / 13 | — |
| 21 | Schedules, elevations and sheets | building | 6 / 13 | — |

## Phase 18 segments

32 merged, 4 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p18-exports` | Exports: IfcCovering (SKIRTINGBOARD, MOLDING) for room moldings and molding lines, glass walls as IfcWall with a Glass material, countertops and backsplashes inside their cabinets' IfcFurniture, niches as recesses; glTF node and OBJ group names for every phase-18 mesh | merging | mai-flash-esc |
| `p18-rebuild` | Incremental rebuild: scene_cache_equals_full gains every phase-18 object and edit; each edit rebuilds only what it touches | merging | mai-flash-esc |
| `p18-demo` | Demo: docs/arch3d/showcase/phase-18-demo.json (a small house with the kitchen, two baths and moldings) and the showcase house's kitchen and baths finished; the user guide's Interior finish and Kitchen and bath sections | merging | mai-flash |
| `upstream-sync-ddfa833bc` | merge upstream upstream/main up to ddfa833bc (2 commits) | merging | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 13 13:25 | `harden-p15-arch-spanning-walls` | merged | 9,765 pass | tturbo-mac |
| Nov 13 13:25 | `p21-ui-callout-open` | merged | 9,765 pass | claude-sonnet-d |
| Nov 13 13:25 | `harden-p20-view-export` | merged | 9,765 pass | tturbo-mac |
| Nov 13 13:25 | `harden-p20-reference` | merged | 9,765 pass | tturbo-mac2 |
| Nov 13 13:25 | `harden-p16-soffit-meshes` | merged | 9,765 pass | tturbo-mac |
| Nov 13 13:25 | `harden-p20-roof-plan` | merged | 9,765 pass | tturbo-mac |
| Nov 13 12:49 | `harden-p19-sky-1` | merged | 9,729 pass | tturbo-mac |
| Nov 13 12:49 | `harden-p17-grilles-doors` | merged | 9,729 pass | tturbo-mac |
| Nov 13 12:49 | `harden-p19-cameras-1` | merged | 9,729 pass | tturbo-mac |
| Nov 13 12:49 | `harden-p21-schedule-model` | merged | 9,729 pass | tturbo-mac |
| Nov 13 12:49 | `p20f-dim-inspect` | merged | 9,729 pass | mai-flash-b |
| Nov 13 12:49 | `p20f-rcp-soffits-once` | merged | 9,729 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1245 |
| bonsai-b | bonsai2-27b | idle | 1196 |
| bonsai-ws2-a | bonsai2-27b | idle | 1316 |
| bonsai-ws2-b | bonsai2-27b | agent | 1274 |
| claude-sonnet-a | sonnet | idle | 149 |
| claude-sonnet-b | sonnet | idle | 153 |
| claude-sonnet-c | sonnet | idle | 89 |
| claude-sonnet-d | sonnet | idle | 82 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1112 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 109 |
| mai-flash-b | mai-code-1.1-flash | idle | 53 |
| mai-flash-esc | mai-code-1.1-flash | down | 121 |
| tturbo-mac | tturbo-ultraheretic | gate | 851 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 478 |
