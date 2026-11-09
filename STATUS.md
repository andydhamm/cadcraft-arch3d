# arch3d build status

Updated 2026-11-09 20:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 21: Electrical, technical illustration and the showcase house — 65 of 69 segments merged · 11 of 14 acceptance checks**

8,175 workspace tests passing at the last merge · 858 segments merged in all · 0 of 15 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 11 / 14 | 108.1 h so far · expected done Nov 10 01:59–Nov 11 05:00 UTC |
| 16 | Ceilings and foundations | building | 6 / 13 | — |
| 17 | Stairs, decks, railings and openings in detail | planned | — | — |
| 18 | Interior finish, kitchen and bath | planned | — | — |
| 19 | Light, sky and cameras | planned | — | — |
| 20 | Plan views and dimensions | planned | — | — |
| 21 | Schedules, elevations and sheets | planned | — | — |

## Phase 15 segments

65 merged, 8 replaced, 3 merging, 3 abandoned, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-showcase-render` | tools/showcase: render the showcase house in every view style | merging | claude-sonnet-b |
| `p15-docs-user-guide` | USER-GUIDE: electrical, split levels, the showcase house; an MCP example | merging | mai-flash-esc |
| `p15-check10-surfaces` | phase-15 check 10 reads the surface rows (operator fix, already on your branch) | merging | claude-sonnet-c |
| `upstream-sync-d847eac13` | merge upstream upstream/main up to d847eac13 (38 commits) | working | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 09 19:56 | `harden-p16-soffit-arch` | merged | 8,175 pass | tturbo-mac |
| Nov 09 19:11 | `upstream-sync-865fa9afb` | merged | 8,169 pass | claude-sonnet-c |
| Nov 09 18:24 | `p16-soffit-commands` | merged | 8,114 pass | claude-sonnet-d |
| Nov 09 17:48 | `p15-showcase-test` | merged | 8,108 pass | mai-flash-esc |
| Nov 09 16:35 | `p16-soffit-commands` | refused | 0 pass | claude-sonnet-d |
| Nov 09 16:35 | `p16-tray-arch` | merged | 8,106 pass | mai-flash-b |
| Nov 09 16:02 | `p16-tray-i18n` | merged | 8,101 pass | bonsai-ws2-a |
| Nov 09 14:42 | `p16-soffit-arch` | merged | 8,101 pass | mai-flash-esc |
| Nov 09 13:55 | `harden-p16-foundation-slab` | merged | 8,097 pass | tturbo-mac |
| Nov 09 13:23 | `upstream-sync-a655cee0c` | merged | 8,090 pass | claude-sonnet-b |
| Nov 09 12:51 | `p15-showcase-site` | merged | 8,090 pass | bonsai-b |
| Nov 09 12:08 | `p16-vault-walls` | merged | 8,089 pass | mai-flash-esc |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 1006 |
| bonsai-b | bonsai2-27b | idle | 957 |
| bonsai-ws2-a | bonsai2-27b | idle | 1041 |
| bonsai-ws2-b | bonsai2-27b | idle | 1019 |
| claude-sonnet-a | sonnet | idle | 93 |
| claude-sonnet-b | sonnet | idle | 98 |
| claude-sonnet-c | sonnet | idle | 43 |
| claude-sonnet-d | sonnet | agent | 43 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | yielding | 900 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 69 |
| mai-flash-b | mai-code-1.1-flash | idle | 9 |
| mai-flash-esc | mai-code-1.1-flash | merging | 63 |
| tturbo-mac | tturbo-ultraheretic | merging | 689 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
