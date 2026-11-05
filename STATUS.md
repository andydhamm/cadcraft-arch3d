# arch3d build status

Updated 2026-11-05 18:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 9 of 32 segments merged · 0 of 13 acceptance checks**

7,508 workspace tests passing at the last merge · 768 segments merged in all · 0 of 14 seats working · PAUSED

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
| 15 | Electrical, technical illustration and the showcase house | building | 0 / 13 | 10.1 h so far · expected done Nov 06 17:39–Nov 08 00:14 UTC |

## Phase 15 segments

15 queued, 9 merged, 4 gate, 3 merging, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-arch-connections` | arch: connection arcs and circuits | gate | bonsai-b |
| `p15-arch-outlets-auto` | arch: auto-placed outlets and the spacing rule | gate | bonsai-ws2-b |
| `p15-arch-spanning-walls` | arch: walls spanning levels in rooms, footprints, roofs, areas and placement | merging | claude-sonnet-b |
| `p15-arch3d-electrical` | arch3d: cover plates for outlets and switches | working | darwin-ws3 |
| `p15-render-plan-level` | render: the plan draws the shown level | merging | claude-sonnet-d |
| `p15-cmd-light-sets` | engine: lightset, a camera's light set, render with a light set | gate | mai-flash |
| `p15-cmd-electrical-view` | engine: view.electrical and level.show | gate | bonsai-ws2-a |
| `upstream-sync-df6597445` | merge upstream upstream/main up to df6597445 (12 commits) | merging | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 05 18:12 | `p15-arch-electrical` | merged | 7,508 pass | mai-flash-esc |
| Nov 05 16:48 | `p15-arch3d-light-brightness` | merged | 7,496 pass | claude-sonnet-a |
| Nov 05 16:19 | `p15-render3d-technical` | merged | 7,493 pass | darwin-ws3 |
| Nov 05 15:51 | `p15-cli-commands-file` | merged | 7,491 pass | bonsai-b |
| Nov 05 15:03 | `p15-cmd-export-view` | merged | 7,489 pass | claude-sonnet-b |
| Nov 05 14:00 | `harden-p14-cmd-backdrop` | merged | 7,486 pass | tturbo-mac |
| Nov 05 13:00 | `p15-doc-electrical` | merged | 7,481 pass | bonsai-ws2-a |
| Nov 05 11:59 | `p15-render3d-clay` | merged | 7,477 pass | darwin-ws3 |
| Nov 05 11:12 | `p15-i18n-labels` | merged | 7,474 pass | claude-sonnet-a |
| Nov 05 09:01 | `harden-p14-docs-user-guide` | merged | 7,473 pass | tturbo-mac |
| Nov 05 07:46 | `upstream-sync-c4d9e6ac6` | merged | 7,468 pass | bonsai-b |
| Nov 05 07:14 | `harden-p14-cache-furnishing-edits` | merged | 7,453 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 900 |
| bonsai-b | bonsai2-27b | gate | 830 |
| bonsai-ws2-a | bonsai2-27b | gate | 926 |
| bonsai-ws2-b | bonsai2-27b | gate | 887 |
| claude-sonnet-a | sonnet | idle | 81 |
| claude-sonnet-b | sonnet | merging | 82 |
| claude-sonnet-c | sonnet | merging | 35 |
| claude-sonnet-d | sonnet | merging | 21 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 775 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | gate | 61 |
| mai-flash-esc | mai-code-1.1-flash | idle | 31 |
| tturbo-mac | tturbo-ultraheretic | merging | 620 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
