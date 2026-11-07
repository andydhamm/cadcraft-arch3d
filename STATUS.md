# arch3d build status

Updated 2026-11-07 12:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 45 of 52 segments merged · 3 of 14 acceptance checks**

7,686 workspace tests passing at the last merge · 812 segments merged in all · 0 of 14 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 3 / 14 | 52.3 h so far · expected done Nov 07 19:28–Nov 08 16:00 UTC |

## Phase 15 segments

45 merged, 6 replaced, 4 merging, 3 queued, 2 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-ui-electrical` | ui-egui: electrical tools, connection drag, light sets, level picker | merging | claude-sonnet-a |
| `p15-plan-layer-transparency` | render: the plan export honours layer transparency for architectural linework (the Electrical view dims) | merging | mai-flash-esc |
| `p15-inspect-electrical` | engine: arch.inspect electrical, connections, circuits, outlet gaps and light sets | merging | claude-sonnet-b |
| `p15-inspect-rooms-on-objects` | engine: brightness on light rows, room on light, fixture and furnishing rows | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 07 12:23 | `p15-render-light-set` | merged | 7,686 pass | mai-flash-esc |
| Nov 07 11:57 | `p15-io-glb-obj-electrical` | merged | 7,683 pass | claude-sonnet-d |
| Nov 07 11:30 | `p15-io-ifc-electrical` | merged | 7,681 pass | claude-sonnet-b |
| Nov 07 11:03 | `p15-io-dxf-electrical` | merged | 7,677 pass | claude-sonnet-c |
| Nov 07 10:36 | `p15-door-on-level` | merged | 7,674 pass | claude-sonnet-a |
| Nov 07 09:36 | `p15-inspect-site-paths-plants` | merged | 7,671 pass | bonsai-a |
| Nov 07 08:32 | `p15-cmd-connections` | merged | 7,668 pass |  |
| Nov 07 07:31 | `p15-inspect-levels-walls` | merged | 7,664 pass | mai-flash |
| Nov 07 07:07 | `upstream-sync-2611bc5f5-v2` | refused | 7,664 pass | bonsai-ws2-a |
| Nov 07 06:03 | `harden-p15-cli-render-styles` | merged | 7,662 pass | tturbo-mac |
| Nov 07 05:02 | `p15-cabinet-along-wall` | merged | 7,654 pass | claude-sonnet-d |
| Nov 07 03:54 | `p15-arch-room-at` | merged | 7,650 pass | claude-sonnet-c |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 940 |
| bonsai-b | bonsai2-27b | idle | 871 |
| bonsai-ws2-a | bonsai2-27b | merging | 968 |
| bonsai-ws2-b | bonsai2-27b | merging | 934 |
| claude-sonnet-a | sonnet | merging | 89 |
| claude-sonnet-b | sonnet | merging | 86 |
| claude-sonnet-c | sonnet | idle | 39 |
| claude-sonnet-d | sonnet | merging | 29 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 821 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 68 |
| mai-flash-esc | mai-code-1.1-flash | merging | 41 |
| tturbo-mac | tturbo-ultraheretic | merging | 636 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
