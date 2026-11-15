# arch3d build status

Updated 2026-11-15 19:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 21: Light, sky and cameras — 67 of 69 segments merged · 12 of 17 acceptance checks**

10,545 workspace tests passing at the last merge · 1216 segments merged in all · 0 of 15 seats working

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
| 19 | Light, sky and cameras | building | 12 / 17 | 104.7 h so far · expected done Nov 15 23:37–Nov 16 18:35 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 13 / 13 | — |

## Phase 19 segments

67 merged, 6 replaced, 1 merging, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p19m-sky-cli-time` | Sky renders through the CLI: a timing test for the 24 renders of check 03, and a report of checks 03 and 11 wall times | merging | claude-sonnet-c |
| `upstream-sync-7064a685a` | merge upstream upstream/main up to 7064a685a (3 commits) | gate | mai-flash-esc |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 15 19:06 | `harden-p6-r3d-limits` | merged | 10,545 pass | tturbo-mac2 |
| Nov 15 19:06 | `harden-p21g-showcase-drawing-set` | merged | 10,545 pass | tturbo-mac2 |
| Nov 15 19:06 | `harden-p18-room-finishes-3` | merged | 10,545 pass | tturbo-mac |
| Nov 15 19:06 | `harden-p21g-callouts-without-view` | merged | 10,545 pass | tturbo-mac2 |
| Nov 15 19:06 | `p18i-showcase-phase18` | merged | 10,545 pass | claude-sonnet-b |
| Nov 15 19:06 | `p21h-showcase-kitchen-cabinets` | merged | 10,545 pass | mai-flash-esc |
| Nov 15 18:38 | `p19m-sky-cli-time` | refused | 10,513 pass | mai-flash-esc |
| Nov 15 17:15 | `upstream-sync-67514021a` | merged | 10,512 pass | claude-sonnet-a |
| Nov 15 16:24 | `harden-p19n-pt-pipelines-once` | merged | 10,540 pass | tturbo-mac |
| Nov 15 16:24 | `p21g-showcase-drawing-set` | merged | 10,540 pass | claude-sonnet-c |
| Nov 15 15:17 | `p19m-sky-cli-time` | refused | 10,536 pass | mai-flash-esc |
| Nov 15 12:51 | `p19n-pt-pipelines-once` | merged | 10,535 pass | mai-flash |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1349 |
| bonsai-b | bonsai2-27b | agent | 1272 |
| bonsai-ws2-a | bonsai2-27b | agent | 1429 |
| bonsai-ws2-b | bonsai2-27b | merging | 1379 |
| claude-sonnet-a | sonnet | idle | 159 |
| claude-sonnet-b | sonnet | idle | 159 |
| claude-sonnet-c | sonnet | idle | 97 |
| claude-sonnet-d | sonnet | idle | 93 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1261 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 125 |
| mai-flash-b | mai-code-1.1-flash | idle | 74 |
| mai-flash-esc | mai-code-1.1-flash | gate | 169 |
| tturbo-mac | tturbo-ultraheretic | gate | 958 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 641 |
