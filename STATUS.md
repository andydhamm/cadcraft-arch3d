# arch3d build status

Updated 2026-11-07 04:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 36 of 53 segments merged · 2 of 14 acceptance checks**

7,650 workspace tests passing at the last merge · 802 segments merged in all · 0 of 14 seats working · PAUSED

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
| 15 | Electrical, technical illustration and the showcase house | building | 2 / 14 | 43.8 h so far · expected done Nov 08 11:38–Nov 10 20:08 UTC |

## Phase 15 segments

36 merged, 7 queued, 6 replaced, 5 gate, 3 merging, 1 escalate, 1 abandoned, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-cmd-connections` | engine: connection, connection.edit, connection.delete, circuit.brightness | escalate |  |
| `p15-inspect-site-paths-plants` | engine: arch.inspect site lists paths and plants with their form | gate | bonsai-a |
| `p15-cabinet-along-wall` | arch+engine: a cabinet's host wall and its span along it | merging | claude-sonnet-d |
| `p15-render-light-set` | render: a render uses its light set (on/off and brightness of each light) | gate | darwin-ws3 |
| `p15-door-on-level` | engine: an opening placed with `level` sits on that level (sill from the wall bottom) | gate | claude-sonnet-a |
| `p15-plan-layer-transparency` | render: the plan export honours layer transparency for architectural linework (the Electrical view dims) | gate | bonsai-b |
| `p15-inspect-levels-walls` | engine: arch.inspect level grade classes and wall z spans | merging | mai-flash |
| `upstream-sync-2611bc5f5-v2` | merge upstream upstream/main up to 2611bc5f5 (44 commits) | merging | bonsai-ws2-a |
| `p15-io-ifc-electrical` | io: outlets and switches as IfcOutlet / IfcSwitchingDevice | working | claude-sonnet-b |
| `p15-io-dxf-electrical` | io: DXF save proxies for electrical devices and connections | gate | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 07 03:54 | `p15-arch-room-at` | merged | 7,650 pass | claude-sonnet-c |
| Nov 07 02:08 | `harden-p15-render-electrical` | merged | 7,647 pass | tturbo-mac |
| Nov 06 23:34 | `p15-io-electrical` | refused | 0 pass | mai-flash-esc |
| Nov 06 23:25 | `harden-p15-arch-outlet-gaps` | merged | 7,638 pass | tturbo-mac |
| Nov 06 22:23 | `p15-cmd-outlets-auto` | merged | 7,633 pass | mai-flash |
| Nov 06 21:33 | `p15-inspect` | refused | 7,635 pass | claude-sonnet-a |
| Nov 06 19:58 | `p15-io-electrical` | refused | 0 pass | darwin-ws3 |
| Nov 06 19:54 | `p15-render-electrical` | merged | 7,630 pass | mai-flash-esc |
| Nov 06 19:01 | `harden-p15-cmd-wall-span` | merged | 7,627 pass | tturbo-mac |
| Nov 06 17:37 | `p15-cmd-electrical` | merged | 7,618 pass | mai-flash |
| Nov 06 16:38 | `p15-arch-outlet-gaps` | merged | 7,612 pass | claude-sonnet-a |
| Nov 06 16:09 | `p15-cli-render-styles` | merged | 7,609 pass | claude-sonnet-d |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 935 |
| bonsai-b | bonsai2-27b | gate | 865 |
| bonsai-ws2-a | bonsai2-27b | merging | 967 |
| bonsai-ws2-b | bonsai2-27b | gate | 928 |
| claude-sonnet-a | sonnet | gate | 86 |
| claude-sonnet-b | sonnet | agent | 84 |
| claude-sonnet-c | sonnet | gate | 38 |
| claude-sonnet-d | sonnet | merging | 27 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 813 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | merging | 68 |
| mai-flash-esc | mai-code-1.1-flash | idle | 38 |
| tturbo-mac | tturbo-ultraheretic | merging | 633 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
