# arch3d build status

Updated 2026-11-16 10:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 21 of 22: Schedules, elevations and sheets — 35 of 40 segments merged · 13 of 13 acceptance checks**

10,829 workspace tests passing at the last merge · 1258 segments merged in all · 0 of 15 seats working

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
| 21 | Schedules, elevations and sheets | building | 13 / 13 | 98.7 h so far · expected done Nov 16 18:00–Nov 17 16:53 UTC |
| 22 | Phase 22 | pending | — | — |

## Phase 21 segments

35 merged, 3 replaced, 2 queued, 1 working, 1 gate, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p21t-wall-window-and-niche` | A wall with both a window and a niche keeps its whole face in renders and picks | working | mai-flash-esc |
| `upstream-sync-ed0fbce27` | merge upstream upstream/main up to ed0fbce27 (35 commits) | gate | claude-sonnet-c |
| `p21t-showcase-house-c` | The showcase house is redrawn inside its walls and gains the phase-16 ceilings and foundation, the kitchen cabinets of its drawing set, and the phase-19 look cameras | merging | claude-sonnet-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 16 09:54 | `p21t-showcase-house-c` | refused | 10,835 pass, 1 fail | bonsai-ws2-b |
| Nov 16 09:33 | `p21t-showcase-house-c` | refused | 0 pass | bonsai-ws2-a |
| Nov 16 09:33 | `p21t-showcase-house-c` | refused | 0 pass | bonsai-a |
| Nov 16 09:33 | `harden-p19t-pane-look-controls` | merged | 10,829 pass | tturbo-mac |
| Nov 16 09:19 | `p21t-user-guide` | merged | 10,823 pass | claude-sonnet-b |
| Nov 16 08:57 | `harden-p19g-sky-check-time` | merged | 10,816 pass, 1 fail | tturbo-mac |
| Nov 16 08:40 | `upstream-sync-d267d6580` | merged | 10,803 pass | claude-sonnet-d |
| Nov 16 08:25 | `p21t-showcase-house-b` | merged | 10,803 pass | bonsai-ws2-b |
| Nov 16 08:16 | `p21t-showcase-house-b` | refused | 0 pass | claude-sonnet-c |
| Nov 16 08:16 | `p19t-pane-look-controls` | merged | 10,794 pass | mai-flash-b |
| Nov 16 08:00 | `harden-p19q-pt-auto-exposure-night-backdrop` | merged | 10,786 pass | tturbo-mac |
| Nov 16 07:38 | `p19t-pane-look-scene` | merged | 10,780 pass | mai-flash-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1376 |
| bonsai-b | bonsai2-27b | idle | 1292 |
| bonsai-ws2-a | bonsai2-27b | merging | 1480 |
| bonsai-ws2-b | bonsai2-27b | gate | 1405 |
| claude-sonnet-a | sonnet | idle | 161 |
| claude-sonnet-b | sonnet | idle | 167 |
| claude-sonnet-c | sonnet | gate | 103 |
| claude-sonnet-d | sonnet | idle | 98 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 1287 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 127 |
| mai-flash-b | mai-code-1.1-flash | idle | 77 |
| mai-flash-esc | mai-code-1.1-flash | agent | 173 |
| tturbo-mac | tturbo-ultraheretic | agent | 990 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 671 |
