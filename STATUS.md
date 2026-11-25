# arch3d build status

Updated 2026-11-25 00:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 44 of 46 segments merged · 12 of 15 acceptance checks**

12,084 workspace tests passing at the last merge · 1402 segments merged in all · 0 of 15 seats working · PAUSED

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
| 22 | Photoreal stills | building | 12 / 15 | 31.0 h so far · expected done Nov 25 02:54–Nov 26 00:52 UTC |

## Phase 22 segments

44 merged, 1 escalate, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p22g-adaptive-robust-error` | Adaptive sampling: a neighbourhood error estimate, a higher floor and no sticky freeze, so pixels do not freeze on a zero sample variance | escalate |  |
| `upstream-sync-171ac5112` | merge upstream upstream/main up to 171ac5112 (39 commits) | gate | darwin-ws3 |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 25 00:10 | `upstream-sync-5e4bbe594` | merged | 12,084 pass | claude-sonnet-a |
| Nov 24 23:26 | `p22g-learned-kitchen-error` | merged | 12,062 pass, 1 fail | claude-sonnet-b |
| Nov 24 21:55 | `p22g-trans-shadow-side` | merged | 12,056 pass | claude-sonnet-c |
| Nov 24 21:19 | `harden-p22f-adaptive-freeze` | merged | 12,052 pass | tturbo-mac |
| Nov 24 20:35 | `harden-p22-arch3d-lens` | merged | 12,047 pass | tturbo-mac |
| Nov 24 20:35 | `harden-p22-r3d-linear-out` | merged | 12,047 pass | tturbo-mac2 |
| Nov 24 20:35 | `harden-p22-engine-denoiser-info` | merged | 12,047 pass | tturbo-mac |
| Nov 24 20:35 | `harden-p22-r3d-bsdf-ext` | merged | 12,047 pass | tturbo-mac2 |
| Nov 24 19:28 | `upstream-sync-a1d92d995` | merged | 12,024 pass | claude-sonnet-b |
| Nov 24 18:52 | `harden-p22-r3d-lens-raster` | merged | 12,004 pass | tturbo-mac |
| Nov 24 18:52 | `harden-p22-r3d-edges` | merged | 12,004 pass | tturbo-mac2 |
| Nov 24 18:52 | `p22f-adaptive-freeze` | merged | 12,004 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 1445 |
| bonsai-b | bonsai2-27b | gate | 1370 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 177 |
| claude-sonnet-b | sonnet | idle | 187 |
| claude-sonnet-c | sonnet | idle | 121 |
| claude-sonnet-d | sonnet | idle | 117 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1332 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 131 |
| mai-flash-b | mai-code-1.1-flash | idle | 85 |
| mai-flash-esc | mai-code-1.1-flash | idle | 178 |
| tturbo-mac | tturbo-ultraheretic | agent | 1242 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 747 |
