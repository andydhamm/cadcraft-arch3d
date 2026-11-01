# arch3d build status

Updated 2026-11-01 22:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 26 of 28 segments merged · 12 of 13 acceptance checks**

6,444 workspace tests passing at the last merge · 656 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 12 / 13 | 14.3 h so far · expected done Nov 01 23:22–Nov 02 09:22 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

26 merged, 1 working, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p11f-glass-shadow-once` | A shadow ray through a glass pane is tinted once, with Fresnel at both faces | working | bonsai-b |
| `upstream-sync-2034d6899` | merge upstream upstream/main up to 2034d6899 (18 commits) | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 21:58 | `p11f-glass-shadow-once` | refused | 0 pass | bonsai-b |
| Nov 01 21:56 | `p11f-explicit-spp-kept` | merged | 6,444 pass | claude-sonnet-a |
| Nov 01 21:39 | `p11f-denoise-variance` | merged | 6,441 pass | mai-flash |
| Nov 01 21:08 | `p11f-glass-shadow-once` | refused | 0 pass | darwin-ws3 |
| Nov 01 20:31 | `harden-p11-r3d-rt` | merged | 6,436 pass | tturbo-mac |
| Nov 01 19:58 | `harden-p11-ui-pane-path` | merged | 6,430 pass | tturbo-mac |
| Nov 01 19:23 | `harden-p11-final-default-path` | merged | 6,419 pass | tturbo-mac |
| Nov 01 18:56 | `p11-r3d-rt` | refused | 0 pass | claude-sonnet-b |
| Nov 01 18:54 | `harden-p11-docs-user-guide` | merged | 6,412 pass | tturbo-mac |
| Nov 01 18:44 | `p11-final-default-path` | merged | 6,408 pass | claude-sonnet-d |
| Nov 01 18:25 | `p11-r3d-rt` | refused | 6,410 pass | claude-sonnet-b |
| Nov 01 17:18 | `harden-p11-io-area-ies` | merged | 6,403 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 788 |
| bonsai-b | bonsai2-27b | agent | 726 |
| bonsai-ws2-a | bonsai2-27b | idle | 782 |
| bonsai-ws2-b | bonsai2-27b | merging | 765 |
| claude-sonnet-a | sonnet | idle | 68 |
| claude-sonnet-b | sonnet | idle | 64 |
| claude-sonnet-c | sonnet | idle | 14 |
| claude-sonnet-d | sonnet | merging | 8 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 645 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | idle | 45 |
| tturbo-mac | tturbo-ultraheretic | agent | 547 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
