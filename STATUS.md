# arch3d build status

Updated 2026-11-15 23:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 21: Light, sky and cameras — 74 of 77 segments merged · 15 of 17 acceptance checks**

10,682 workspace tests passing at the last merge · 1230 segments merged in all · 0 of 15 seats working

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
| 19 | Light, sky and cameras | building | 15 / 17 | 108.7 h so far · expected done Nov 16 01:52–Nov 16 03:20 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | done | 13 / 13 | 84.1 h |

## Phase 19 segments

74 merged, 9 replaced, 2 gate, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p19r-hdri-icon` | An HDR backdrop's display picture and library icon are exposed for the typical pixel and tone-mapped, not flat | gate | bonsai-a |
| `p19r-overview-cutaway` | An overview camera's cutaway is computed for its own level; the phase-19 demo builds on level 0 and its overview centres on a floor | merging | claude-sonnet-b |
| `upstream-sync-0e3f62f50` | merge upstream upstream/main up to 0e3f62f50 (19 commits) | gate | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 15 23:11 | `harden-p19q-raster-meter-backdrop-only` | merged | 10,682 pass | tturbo-mac2 |
| Nov 15 22:31 | `upstream-sync-7064a685a` | merged | 10,675 pass | mai-flash-esc |
| Nov 15 22:12 | `p19q-strip-mesh-under-ceiling` | merged | 10,614 pass | darwin-ws3 |
| Nov 15 21:49 | `p19q-demo-dusk-camera-closer` | merged | 10,610 pass | claude-sonnet-b |
| Nov 15 21:49 | `p19q-raster-meter-backdrop-only` | merged | 10,610 pass | bonsai-ws2-a |
| Nov 15 21:32 | `p19q-final-raster-frame-look` | merged | 10,602 pass | claude-sonnet-d |
| Nov 15 21:15 | `p19q-pt-auto-exposure-night-backdrop` | merged | 10,600 pass | mai-flash |
| Nov 15 20:16 | `harden-p18-room-finishes-1` | merged | 10,595 pass | tturbo-mac |
| Nov 15 20:16 | `harden-p18-shower-niche-4` | merged | 10,595 pass | tturbo-mac |
| Nov 15 20:16 | `harden-p19-hdri-3` | merged | 10,595 pass | tturbo-mac |
| Nov 15 20:16 | `harden-p19g-final-raster-env` | merged | 10,595 pass | tturbo-mac |
| Nov 15 20:16 | `harden-p21g-sheet-camera-new-sheet` | merged | 10,595 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 1355 |
| bonsai-b | bonsai2-27b | gate | 1278 |
| bonsai-ws2-a | bonsai2-27b | gate | 1449 |
| bonsai-ws2-b | bonsai2-27b | agent | 1384 |
| claude-sonnet-a | sonnet | idle | 159 |
| claude-sonnet-b | sonnet | idle | 161 |
| claude-sonnet-c | sonnet | idle | 97 |
| claude-sonnet-d | sonnet | gate | 94 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1267 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 127 |
| mai-flash-b | mai-code-1.1-flash | idle | 74 |
| mai-flash-esc | mai-code-1.1-flash | idle | 172 |
| tturbo-mac | tturbo-ultraheretic | gate | 968 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 650 |
