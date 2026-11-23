# arch3d build status

Updated 2026-11-23 20:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 1 of 35 segments merged · 0 of 15 acceptance checks**

11,394 workspace tests passing at the last merge · 1322 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | done | 20 / 20 | 96.7 h |
| 19 | Light, sky and cameras | done | 17 / 17 | 118.1 h |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | done | 13 / 13 | 105.8 h |
| 22 | Photoreal stills | building | 0 / 15 | 2.8 h so far · expected done Nov 24 06:48–Dec 04 11:05 UTC |

## Phase 22 segments

29 queued, 4 merging, 1 merged, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-05d4e64ad` | merge upstream upstream/main up to 05d4e64ad (27 commits) | working | mai-flash-esc |
| `p22-doc-look` | Look: lens, film, sun disc, technique photoreal; the rung table | merging | claude-sonnet-a |
| `p22-arch3d-lens` | arch3d::lens: Panini, shift and thin-lens math shared by every renderer and by picking | merging | claude-sonnet-d |
| `p22-arch3d-material-ext` | MaterialExt: the optional PBR fields and the table that gives library materials their values | merging | claude-sonnet-c |
| `p22-r3d-tza` | The .tza weight loader and the embedded denoiser weights | merging | mai-flash-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 23 19:14 | `harden-p18-observables-1` | merged | 11,394 pass | tturbo-mac |
| Nov 23 17:27 | `harden-p18-observables-2` | merged | 11,377 pass | tturbo-mac |
| Nov 23 16:38 | `upstream-sync-0ebb256d3` | merged | 11,363 pass | darwin-ws3 |
| Nov 23 16:00 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | bonsai-a |
| Nov 23 15:33 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | mai-flash-b |
| Nov 23 14:57 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | darwin-ws3 |
| Nov 23 14:23 | `harden-p17-exports-rebuild` | merged | 11,289 pass | tturbo-mac |
| Nov 23 12:57 | `harden-p18-observables-2` | refused | 11,296 pass | tturbo-mac |
| Nov 23 01:34 | `harden-p18-molding-kit-1` | merged | 11,282 pass | tturbo-mac |
| Nov 22 17:13 | `harden-p18-observables-3` | merged | 11,263 pass | tturbo-mac |
| Nov 19 11:52 | `harden-p18-scene-plumbing-1` | merged | 11,256 pass | tturbo-mac |
| Nov 19 11:24 | `harden-p19-foundation-1` | merged | 11,251 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1392 |
| bonsai-b | bonsai2-27b | gate | 1302 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 164 |
| claude-sonnet-b | sonnet | idle | 169 |
| claude-sonnet-c | sonnet | idle | 105 |
| claude-sonnet-d | sonnet | idle | 100 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1295 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 128 |
| mai-flash-b | mai-code-1.1-flash | idle | 81 |
| mai-flash-esc | mai-code-1.1-flash | agent | 175 |
| tturbo-mac | tturbo-ultraheretic | agent | 1193 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 703 |
