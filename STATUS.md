# arch3d build status

Updated 2026-11-16 09:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 21 of 22: Schedules, elevations and sheets — 34 of 39 segments merged · 13 of 13 acceptance checks**

10,816 workspace tests passing at the last merge · 1256 segments merged in all · 0 of 15 seats working

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
| 21 | Schedules, elevations and sheets | building | 13 / 13 | 97.7 h so far · expected done Nov 16 18:14–Nov 17 23:32 UTC |
| 22 | Phase 22 | pending | — | — |

## Phase 21 segments

34 merged, 2 replaced, 2 working, 1 queued, 1 merging, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p21t-entry-landing-views` | The showcase's Entry landing gets its plan views and a sheet in the drawing set | working | mai-flash-esc |
| `p21t-user-guide` | USER-GUIDE: dimensions that know the building, a phase-17 section, a runnable light-sky-cameras example, the showcase as it is | merging | claude-sonnet-b |
| `p21t-wall-window-and-niche` | A wall with both a window and a niche keeps its whole face in renders and picks | working | darwin-ws3 |
| `upstream-sync-ed0fbce27` | merge upstream upstream/main up to ed0fbce27 (35 commits) | gate | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 16 08:57 | `harden-p19g-sky-check-time` | merged | 10,816 pass, 1 fail | tturbo-mac |
| Nov 16 08:40 | `upstream-sync-d267d6580` | merged | 10,803 pass | claude-sonnet-d |
| Nov 16 08:25 | `p21t-showcase-house-b` | merged | 10,803 pass | bonsai-ws2-b |
| Nov 16 08:16 | `p21t-showcase-house-b` | refused | 0 pass | claude-sonnet-c |
| Nov 16 08:16 | `p19t-pane-look-controls` | merged | 10,794 pass | mai-flash-b |
| Nov 16 08:00 | `harden-p19q-pt-auto-exposure-night-backdrop` | merged | 10,786 pass | tturbo-mac |
| Nov 16 07:38 | `p19t-pane-look-scene` | merged | 10,780 pass | mai-flash-b |
| Nov 16 07:20 | `harden-p19q-demo-dusk-camera-closer` | merged | 10,773 pass | tturbo-mac2 |
| Nov 16 07:20 | `harden-p19q-final-raster-frame-look` | merged | 10,773 pass | tturbo-mac |
| Nov 16 07:05 | `harden-p19q-strip-mesh-under-ceiling` | merged | 10,765 pass | tturbo-mac |
| Nov 16 06:47 | `harden-p19r-overview-cutaway` | merged | 10,761 pass | tturbo-mac2 |
| Nov 16 06:29 | `harden-p21t-cli-run-defers-views` | merged | 10,753 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1375 |
| bonsai-b | bonsai2-27b | agent | 1290 |
| bonsai-ws2-a | bonsai2-27b | merging | 1477 |
| bonsai-ws2-b | bonsai2-27b | idle | 1404 |
| claude-sonnet-a | sonnet | idle | 161 |
| claude-sonnet-b | sonnet | idle | 165 |
| claude-sonnet-c | sonnet | gate | 102 |
| claude-sonnet-d | sonnet | idle | 98 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1285 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 127 |
| mai-flash-b | mai-code-1.1-flash | idle | 77 |
| mai-flash-esc | mai-code-1.1-flash | agent | 172 |
| tturbo-mac | tturbo-ultraheretic | agent | 988 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 669 |
