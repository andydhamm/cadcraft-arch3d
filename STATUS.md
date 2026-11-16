# arch3d build status

Updated 2026-11-16 08:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 22: Light, sky and cameras — 80 of 82 segments merged · 17 of 17 acceptance checks**

10,786 workspace tests passing at the last merge · 1252 segments merged in all · 0 of 15 seats working

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
| 19 | Light, sky and cameras | building | 17 / 17 | 117.7 h so far · expected done Nov 16 09:51–Nov 16 10:45 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 13 / 13 | — |
| 22 | Phase 22 | pending | — | — |

## Phase 19 segments

80 merged, 9 replaced, 1 merging, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p19t-pane-look-controls` | Pane look controls: cutaway, clip box, near clip, exposure, tone map, sun angle, sky, camera reset and floor overview, each one command | merging | mai-flash-b |
| `upstream-sync-d267d6580` | merge upstream upstream/main up to d267d6580 (6 commits) | gate | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 16 08:00 | `harden-p19q-pt-auto-exposure-night-backdrop` | merged | 10,786 pass | tturbo-mac |
| Nov 16 07:38 | `p19t-pane-look-scene` | merged | 10,780 pass | mai-flash-b |
| Nov 16 07:20 | `harden-p19q-demo-dusk-camera-closer` | merged | 10,773 pass | tturbo-mac2 |
| Nov 16 07:20 | `harden-p19q-final-raster-frame-look` | merged | 10,773 pass | tturbo-mac |
| Nov 16 07:05 | `harden-p19q-strip-mesh-under-ceiling` | merged | 10,765 pass | tturbo-mac |
| Nov 16 06:47 | `harden-p19r-overview-cutaway` | merged | 10,761 pass | tturbo-mac2 |
| Nov 16 06:29 | `harden-p21t-cli-run-defers-views` | merged | 10,753 pass | tturbo-mac |
| Nov 16 06:29 | `harden-p21t-counter-outlets-level` | merged | 10,753 pass | tturbo-mac2 |
| Nov 16 06:06 | `upstream-sync-2974d74bd` | merged | 10,747 pass | claude-sonnet-d |
| Nov 16 05:49 | `harden-p11-cli-render-path` | merged | 10,743 pass | tturbo-mac2 |
| Nov 16 05:49 | `p21t-showcase-tests-defer` | merged | 10,743 pass | bonsai-ws2-a |
| Nov 16 05:39 | `harden-p21t-replay-defers-views` | merged | 10,731 pass | tturbo-mac2 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1373 |
| bonsai-b | bonsai2-27b | idle | 1290 |
| bonsai-ws2-a | bonsai2-27b | idle | 1475 |
| bonsai-ws2-b | bonsai2-27b | merging | 1401 |
| claude-sonnet-a | sonnet | idle | 160 |
| claude-sonnet-b | sonnet | idle | 163 |
| claude-sonnet-c | sonnet | idle | 101 |
| claude-sonnet-d | sonnet | gate | 97 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 1284 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 127 |
| mai-flash-b | mai-code-1.1-flash | idle | 77 |
| mai-flash-esc | mai-code-1.1-flash | idle | 172 |
| tturbo-mac | tturbo-ultraheretic | agent | 986 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 667 |
