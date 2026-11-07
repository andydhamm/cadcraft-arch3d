# arch3d build status

Updated 2026-11-07 10:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 40 of 52 segments merged · 2 of 14 acceptance checks**

7,671 workspace tests passing at the last merge · 807 segments merged in all · 0 of 14 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 2 / 14 | 49.8 h so far · expected done Nov 08 05:27–Nov 09 20:10 UTC |

## Phase 15 segments

40 merged, 9 merging, 6 replaced, 3 queued, 2 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-ui-electrical` | ui-egui: electrical tools, connection drag, light sets, level picker | merging | claude-sonnet-a |
| `p15-render-light-set` | render: a render uses its light set (on/off and brightness of each light) | merging | mai-flash-esc |
| `p15-door-on-level` | engine: an opening placed with `level` sits on that level (sill from the wall bottom) | merging | claude-sonnet-a |
| `p15-plan-layer-transparency` | render: the plan export honours layer transparency for architectural linework (the Electrical view dims) | merging | mai-flash-esc |
| `p15-inspect-electrical` | engine: arch.inspect electrical, connections, circuits, outlet gaps and light sets | merging | claude-sonnet-b |
| `p15-inspect-rooms-on-objects` | engine: brightness on light rows, room on light, fixture and furnishing rows | merging | claude-sonnet-d |
| `p15-io-ifc-electrical` | io: outlets and switches as IfcOutlet / IfcSwitchingDevice | merging | claude-sonnet-b |
| `p15-io-dxf-electrical` | io: DXF save proxies for electrical devices and connections | merging | claude-sonnet-c |
| `p15-io-glb-obj-electrical` | io: GLB nodes and OBJ groups for cover plates | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 07 09:36 | `p15-inspect-site-paths-plants` | merged | 7,671 pass | bonsai-a |
| Nov 07 08:32 | `p15-cmd-connections` | merged | 7,668 pass |  |
| Nov 07 07:31 | `p15-inspect-levels-walls` | merged | 7,664 pass | mai-flash |
| Nov 07 07:07 | `upstream-sync-2611bc5f5-v2` | refused | 7,664 pass | bonsai-ws2-a |
| Nov 07 06:03 | `harden-p15-cli-render-styles` | merged | 7,662 pass | tturbo-mac |
| Nov 07 05:02 | `p15-cabinet-along-wall` | merged | 7,654 pass | claude-sonnet-d |
| Nov 07 03:54 | `p15-arch-room-at` | merged | 7,650 pass | claude-sonnet-c |
| Nov 07 02:08 | `harden-p15-render-electrical` | merged | 7,647 pass | tturbo-mac |
| Nov 06 23:34 | `p15-io-electrical` | refused | 0 pass | mai-flash-esc |
| Nov 06 23:25 | `harden-p15-arch-outlet-gaps` | merged | 7,638 pass | tturbo-mac |
| Nov 06 22:23 | `p15-cmd-outlets-auto` | merged | 7,633 pass | mai-flash |
| Nov 06 21:33 | `p15-inspect` | refused | 7,635 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 938 |
| bonsai-b | bonsai2-27b | gate | 870 |
| bonsai-ws2-a | bonsai2-27b | merging | 968 |
| bonsai-ws2-b | bonsai2-27b | gate | 933 |
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
