# arch3d build status

Updated 2026-11-15 15:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 21: Light, sky and cameras — 65 of 68 segments merged · 12 of 17 acceptance checks**

10,535 workspace tests passing at the last merge · 1207 segments merged in all · 0 of 15 seats working

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
| 19 | Light, sky and cameras | building | 12 / 17 | 101.2 h so far · expected done Nov 15 23:45–Nov 17 08:09 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 12 / 13 | — |

## Phase 19 segments

65 merged, 6 replaced, 2 working, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-67514021a` | merge upstream upstream/main up to 67514021a (6 commits) | merging | claude-sonnet-a |
| `p19m-sky-cli-time` | Sky renders through the CLI: a timing test for the 24 renders of check 03, and a report of checks 03 and 11 wall times | working | mai-flash-esc |
| `p18i-showcase-phase18` | The phase-18 demo gains a door-less glass panel; the showcase house gains the phase-18 kitchen and bath features | working | claude-sonnet-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 15 15:17 | `p19m-sky-cli-time` | refused | 10,536 pass | mai-flash-esc |
| Nov 15 12:51 | `p19n-pt-pipelines-once` | merged | 10,535 pass | mai-flash |
| Nov 15 12:07 | `p21g-sheet-camera-new-sheet` | merged | 10,534 pass | claude-sonnet-b |
| Nov 15 12:07 | `p19m-env-radiance-linear` | merged | 10,534 pass | claude-sonnet-d |
| Nov 15 11:42 | `p21g-callouts-without-view` | merged | 10,526 pass, 1 fail | claude-sonnet-c |
| Nov 15 07:56 | `harden-p19f-pt-horizon-lit` | merged | 10,525 pass | tturbo-mac |
| Nov 15 07:07 | `harden-p21f-interior-cabinet-front` | merged | 10,521 pass | tturbo-mac |
| Nov 15 06:26 | `harden-p19k-pt-resolve-hable` | merged | 10,504 pass | tturbo-mac |
| Nov 15 05:48 | `harden-p21-view-notes` | merged | 10,499 pass | tturbo-mac |
| Nov 15 05:48 | `harden-p18h-showcase-digest` | merged | 10,499 pass | tturbo-mac2 |
| Nov 15 05:48 | `harden-p14-arch-library` | merged | 10,499 pass | tturbo-mac2 |
| Nov 15 05:06 | `harden-p21f-interior-cabinet-front` | refused | 10,474 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 1337 |
| bonsai-b | bonsai2-27b | merging | 1263 |
| bonsai-ws2-a | bonsai2-27b | gate | 1421 |
| bonsai-ws2-b | bonsai2-27b | merging | 1368 |
| claude-sonnet-a | sonnet | idle | 159 |
| claude-sonnet-b | sonnet | agent | 158 |
| claude-sonnet-c | sonnet | idle | 96 |
| claude-sonnet-d | sonnet | idle | 93 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1251 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 125 |
| mai-flash-b | mai-code-1.1-flash | idle | 74 |
| mai-flash-esc | mai-code-1.1-flash | agent | 167 |
| tturbo-mac | tturbo-ultraheretic | agent | 949 |
| tturbo-mac2 | tturbo-ultraheretic | gate | 633 |
