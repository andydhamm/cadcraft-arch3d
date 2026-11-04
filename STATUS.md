# arch3d build status

Updated 2026-11-04 20:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 14 of 15: The library — 27 of 35 segments merged · 7 of 13 acceptance checks**

7,340 workspace tests passing at the last merge · 744 segments merged in all · 0 of 14 seats working

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
| 14 | The library | building | 7 / 13 | 24.8 h so far · expected done Nov 05 07:13–Nov 06 13:19 UTC |
| 15 | Electrical, technical illustration and the showcase house | planned | — | — |

## Phase 14 segments

27 merged, 2 gate, 2 merging, 2 working, 2 queued, 1 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p14-cmd-user-catalog` | The user catalog: add, copy, edit, delete, Trash, placing groups | gate | bonsai-ws2-a |
| `p14-cmd-defaults` | Set as Default for cabinets, doors, windows and furnishings | merging | claude-sonnet-d |
| `p14-r3d-pt-backdrop` | The path tracer shows the backdrop where camera rays miss | working | bonsai-b |
| `p14-ui-library-panel` | The Library side panel: categories, folders, search, chips, history, tags, details | merging | claude-sonnet-a |
| `p14-cache-furnishing-edits` | The scene cache stays the full build under furnishing, paint and casework-part edits | gate | claude-sonnet-c |
| `upstream-sync-5f8999f30-v2` | merge upstream upstream/main up to 5f8999f30 (39 commits) | working | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 04 20:10 | `harden-p14-inspect` | merged | 7,340 pass | tturbo-mac |
| Nov 04 19:35 | `p14-cmd-backdrop` | merged | 7,334 pass | claude-sonnet-c |
| Nov 04 18:31 | `p14-arch3d-paint` | merged | 7,295 pass | claude-sonnet-b |
| Nov 04 17:56 | `harden-p14-arch-paint` | merged | 7,287 pass | tturbo-mac |
| Nov 04 17:22 | `p14-r3d-backdrop` | merged | 7,268 pass | claude-sonnet-d |
| Nov 04 16:30 | `p14-arch3d-cabinet-parts` | refused | 7,271 pass | bonsai-a |
| Nov 04 15:43 | `p14-cmd-icons` | merged | 7,261 pass | mai-flash-esc |
| Nov 04 15:10 | `p14-cmd-casework-parts` | merged | 7,254 pass | claude-sonnet-a |
| Nov 04 14:36 | `p14-apps-library-hooks` | merged | 7,250 pass | claude-sonnet-d |
| Nov 04 13:36 | `p14-inspect` | refused | 7,199 pass | claude-sonnet-b |
| Nov 04 12:57 | `p14-io-dxf-furnishing` | refused | 7,199 pass | claude-sonnet-d |
| Nov 04 11:07 | `p14-cmd-furnishing-transform` | refused | 7,200 pass | mai-flash-esc |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 874 |
| bonsai-b | bonsai2-27b | agent | 815 |
| bonsai-ws2-a | bonsai2-27b | gate | 893 |
| bonsai-ws2-b | bonsai2-27b | gate | 862 |
| claude-sonnet-a | sonnet | merging | 77 |
| claude-sonnet-b | sonnet | idle | 80 |
| claude-sonnet-c | sonnet | gate | 32 |
| claude-sonnet-d | sonnet | merging | 20 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 746 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 58 |
| mai-flash-esc | mai-code-1.1-flash | idle | 20 |
| tturbo-mac | tturbo-ultraheretic | agent | 608 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
