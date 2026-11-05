# arch3d build status

Updated 2026-11-05 10:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 0 of 31 segments merged · 0 of 13 acceptance checks**

7,473 workspace tests passing at the last merge · 758 segments merged in all · 0 of 14 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 0 / 13 | 2.1 h so far · expected done Nov 05 18:12–Nov 09 11:50 UTC |

## Phase 15 segments

25 queued, 4 gate, 1 merging, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-doc-electrical` | doc: electrical devices, connections, light sets, light brightness | gate | bonsai-ws2-b |
| `p15-doc-levels` | doc: level kinds, walls spanning levels, room types, planting beds, the shown plan level | gate | bonsai-a |
| `p15-i18n-labels` | ui-egui: translations for every phase-15 command and menu entry | merging | claude-sonnet-a |
| `p15-cmd-export-view` | engine: export one view or one level's plan | gate | claude-sonnet-b |
| `p15-render3d-clay` | render3d: the clay style | gate | darwin-ws3 |
| `p15-cli-commands-file` | cadcraft-cli: run --commands FILE.json | working | bonsai-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 05 09:01 | `harden-p14-docs-user-guide` | merged | 7,473 pass | tturbo-mac |
| Nov 05 07:46 | `upstream-sync-c4d9e6ac6` | merged | 7,468 pass | bonsai-b |
| Nov 05 07:14 | `harden-p14-cache-furnishing-edits` | merged | 7,453 pass | tturbo-mac |
| Nov 05 05:59 | `upstream-sync-5bfcee629` | merged | 7,449 pass | claude-sonnet-c |
| Nov 05 04:42 | `harden-p14-cmd-user-catalog` | merged | 7,457 pass | tturbo-mac |
| Nov 05 01:23 | `harden-p14-r3d-pt-backdrop` | refused | 0 pass | tturbo-mac |
| Nov 05 01:00 | `upstream-sync-5f8999f30-v2` | merged | 7,447 pass | mai-flash |
| Nov 05 00:16 | `p14-cmd-user-catalog` | merged | 7,374 pass | mai-flash-esc |
| Nov 04 23:49 | `harden-p14-arch3d-cabinet-parts` | merged | 7,365 pass | tturbo-mac |
| Nov 04 22:43 | `p14-r3d-pt-backdrop` | merged | 7,359 pass | bonsai-b |
| Nov 04 22:02 | `p14-ui-library-place` | merged | 7,357 pass | claude-sonnet-a |
| Nov 04 21:39 | `p14-cache-furnishing-edits` | merged | 7,352 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 884 |
| bonsai-b | bonsai2-27b | agent | 823 |
| bonsai-ws2-a | bonsai2-27b | gate | 913 |
| bonsai-ws2-b | bonsai2-27b | gate | 877 |
| claude-sonnet-a | sonnet | merging | 79 |
| claude-sonnet-b | sonnet | gate | 80 |
| claude-sonnet-c | sonnet | idle | 34 |
| claude-sonnet-d | sonnet | idle | 20 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 771 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 60 |
| mai-flash-esc | mai-code-1.1-flash | idle | 25 |
| tturbo-mac | tturbo-ultraheretic | gate | 618 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
