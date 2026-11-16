# arch3d build status

Updated 2026-11-16 05:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 19 of 22: Light, sky and cameras — 78 of 81 segments merged · 17 of 17 acceptance checks**

10,731 workspace tests passing at the last merge · 1241 segments merged in all · 0 of 15 seats working

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
| 19 | Light, sky and cameras | building | 17 / 17 | 115.2 h so far · expected done Nov 16 08:09–Nov 16 09:30 UTC |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | building | 13 / 13 | — |
| 22 | Phase 22 | pending | — | — |

## Phase 19 segments

78 merged, 9 replaced, 1 merging, 1 working, 1 queued

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-2974d74bd` | merge upstream upstream/main up to 2974d74bd (1 commits) | merging | claude-sonnet-d |
| `p19t-pane-look-scene` | The 3D pane draws the look's scene: cutaway, floor overview, light set and camera backdrop; a camera's technique picks the pane style | working | mai-flash-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 16 05:39 | `harden-p21t-replay-defers-views` | merged | 10,731 pass | tturbo-mac2 |
| Nov 16 05:39 | `p21t-cli-run-defers-views` | merged | 10,731 pass | bonsai-ws2-a |
| Nov 16 05:22 | `p21t-counter-outlets-level` | merged | 10,723 pass | bonsai-b |
| Nov 16 04:44 | `harden-p19h-env-cache-multi` | merged | 10,719 pass | tturbo-mac2 |
| Nov 16 04:44 | `p21t-replay-defers-views` | merged | 10,719 pass | bonsai-ws2-a |
| Nov 16 04:09 | `p19t-pane-look-lighting` | merged | 10,708 pass | claude-sonnet-a |
| Nov 16 03:37 | `p21t-raised-level-openings` | refused | 10,709 pass | claude-sonnet-c |
| Nov 16 02:27 | `harden-p19r-hdri-icon` | merged | 10,702 pass | tturbo-mac2 |
| Nov 16 02:05 | `harden-p19k-hdri-lights-alone` | merged | 10,693 pass | tturbo-mac2 |
| Nov 16 01:46 | `upstream-sync-0e3f62f50` | merged | 10,690 pass | claude-sonnet-d |
| Nov 16 00:18 | `p19r-hdri-icon` | merged | 10,687 pass | bonsai-b |
| Nov 15 23:44 | `p19r-overview-cutaway` | merged | 10,684 pass | claude-sonnet-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 1369 |
| bonsai-b | bonsai2-27b | idle | 1286 |
| bonsai-ws2-a | bonsai2-27b | merging | 1468 |
| bonsai-ws2-b | bonsai2-27b | idle | 1392 |
| claude-sonnet-a | sonnet | idle | 160 |
| claude-sonnet-b | sonnet | idle | 163 |
| claude-sonnet-c | sonnet | idle | 100 |
| claude-sonnet-d | sonnet | idle | 96 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 1279 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 127 |
| mai-flash-b | mai-code-1.1-flash | agent | 74 |
| mai-flash-esc | mai-code-1.1-flash | idle | 172 |
| tturbo-mac | tturbo-ultraheretic | gate | 979 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 664 |
