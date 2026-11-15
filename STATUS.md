# arch3d build status

Updated 2026-11-15 12:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 21: Light, sky and cameras — 63 of 66 segments merged · 12 of 17 acceptance checks**

10,526 workspace tests passing at the last merge · 1204 segments merged in all · 0 of 15 seats working

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
| 18 | Interior finish, kitchen and bath | incomplete | 19 / 20 | 96.7 h |
| 19 | Light, sky and cameras | building | 12 / 17 | 97.5 h so far · expected done Nov 16 02:24–Nov 23 22:03 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 12 / 13 | — |

## Phase 19 segments

63 merged, 6 replaced, 1 gate, 1 merging, 1 queued

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-67514021a` | merge upstream upstream/main up to 67514021a (6 commits) | gate | claude-sonnet-a |
| `p19m-env-radiance-linear` | Environment::radiance stops re-validating the whole map per call (the O(texels^2) cost under the 512 x 256 sky); the sky cache shares its map by Arc | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 15 11:42 | `p21g-callouts-without-view` | merged | 10,526 pass, 1 fail | claude-sonnet-c |
| Nov 15 07:56 | `harden-p19f-pt-horizon-lit` | merged | 10,525 pass | tturbo-mac |
| Nov 15 07:07 | `harden-p21f-interior-cabinet-front` | merged | 10,521 pass | tturbo-mac |
| Nov 15 06:26 | `harden-p19k-pt-resolve-hable` | merged | 10,504 pass | tturbo-mac |
| Nov 15 05:48 | `harden-p21-view-notes` | merged | 10,499 pass | tturbo-mac |
| Nov 15 05:48 | `harden-p18h-showcase-digest` | merged | 10,499 pass | tturbo-mac2 |
| Nov 15 05:48 | `harden-p14-arch-library` | merged | 10,499 pass | tturbo-mac2 |
| Nov 15 05:06 | `harden-p21f-interior-cabinet-front` | refused | 10,474 pass | tturbo-mac |
| Nov 15 04:25 | `p19k-pt-resolve-hable` | merged | 10,466 pass | bonsai-a |
| Nov 15 03:48 | `harden-p10-r3d-light-pack` | merged | 10,464 pass | tturbo-mac2 |
| Nov 15 03:04 | `harden-p19-cameras-4` | merged | 10,455 pass | tturbo-mac |
| Nov 15 03:04 | `harden-p10-cmd-render` | merged | 10,455 pass | tturbo-mac2 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 1326 |
| bonsai-b | bonsai2-27b | gate | 1262 |
| bonsai-ws2-a | bonsai2-27b | merging | 1405 |
| bonsai-ws2-b | bonsai2-27b | gate | 1366 |
| claude-sonnet-a | sonnet | gate | 157 |
| claude-sonnet-b | sonnet | idle | 157 |
| claude-sonnet-c | sonnet | idle | 95 |
| claude-sonnet-d | sonnet | idle | 93 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 1240 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 124 |
| mai-flash-b | mai-code-1.1-flash | idle | 74 |
| mai-flash-esc | mai-code-1.1-flash | idle | 163 |
| tturbo-mac | tturbo-ultraheretic | agent | 943 |
| tturbo-mac2 | tturbo-ultraheretic | gate | 622 |
