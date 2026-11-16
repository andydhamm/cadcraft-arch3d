# arch3d build status

Updated 2026-11-16 16:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 21 of 22: Schedules, elevations and sheets — 41 of 42 segments merged · 13 of 13 acceptance checks**

10,873 workspace tests passing at the last merge · 1268 segments merged in all · 0 of 15 seats working

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
| 21 | Schedules, elevations and sheets | building | 13 / 13 | 104.7 h so far · expected done Nov 16 16:48–Nov 17 02:26 UTC |
| 22 | Phase 22 | pending | — | — |

## Phase 21 segments

41 merged, 4 replaced, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p21t-gallery-tool` | tools/showcase/render_all.py regenerates the whole gallery: plan views, roof and site plans, interior elevations, the look cameras, sheet images and the drawing-set PDF; the showcase README matches the script | gate | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 16 15:43 | `harden-p21t-wall-window-and-niche-b` | merged | 10,873 pass | tturbo-mac |
| Nov 16 15:25 | `p21t-gallery-tool` | refused | 10,872 pass | claude-sonnet-b |
| Nov 16 14:56 | `upstream-sync-16341b665` | merged | 10,866 pass | bonsai-a |
| Nov 16 14:23 | `harden-p21t-user-guide` | merged | 10,866 pass | tturbo-mac |
| Nov 16 13:53 | `p21t-gallery-tool` | refused | 10,868 pass | claude-sonnet-a |
| Nov 16 13:17 | `harden-p21t-entry-landing-views-b` | merged | 10,862 pass | tturbo-mac |
| Nov 16 13:09 | `harden-p19f-cutaway-interactive` | merged | 10,856 pass | tturbo-mac |
| Nov 16 12:53 | `upstream-sync-2ccf5874b` | merged | 10,849 pass | bonsai-ws2-b |
| Nov 16 12:37 | `p21t-wall-window-and-niche-b` | merged | 10,847 pass | bonsai-a |
| Nov 16 10:41 | `p21t-entry-landing-views-b` | merged | 10,843 pass | bonsai-ws2-b |
| Nov 16 10:29 | `upstream-sync-ed0fbce27` | merged | 10,841 pass | claude-sonnet-c |
| Nov 16 10:08 | `p21t-showcase-house-c` | merged | 10,836 pass | claude-sonnet-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1381 |
| bonsai-b | bonsai2-27b | gate | 1296 |
| bonsai-ws2-a | bonsai2-27b | gate | 1486 |
| bonsai-ws2-b | bonsai2-27b | idle | 1413 |
| claude-sonnet-a | sonnet | idle | 163 |
| claude-sonnet-b | sonnet | idle | 168 |
| claude-sonnet-c | sonnet | idle | 104 |
| claude-sonnet-d | sonnet | gate | 98 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 1290 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 128 |
| mai-flash-b | mai-code-1.1-flash | idle | 79 |
| mai-flash-esc | mai-code-1.1-flash | idle | 175 |
| tturbo-mac | tturbo-ultraheretic | agent | 1003 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 687 |
