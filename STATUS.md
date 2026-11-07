# arch3d build status

Updated 2026-11-07 02:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 35 of 50 segments merged · 2 of 14 acceptance checks**

7,647 workspace tests passing at the last merge · 801 segments merged in all · 0 of 14 seats working · PAUSED

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
| 15 | Electrical, technical illustration and the showcase house | building | 2 / 14 | 42.3 h so far · expected done Nov 07 23:19–Nov 09 10:49 UTC |

## Phase 15 segments

35 merged, 9 queued, 5 replaced, 2 escalate, 2 gate, 1 abandoned, 1 merging, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-io-electrical` | io: electrical devices and connections in GLB, OBJ, IFC and DXF | escalate |  |
| `p15-cmd-connections` | engine: connection, connection.edit, connection.delete, circuit.brightness | escalate |  |
| `p15-arch-room-at` | arch: a public room lookup at a plan point | merging | claude-sonnet-c |
| `p15-inspect-site-paths-plants` | engine: arch.inspect site lists paths and plants with their form | gate | bonsai-a |
| `p15-cabinet-along-wall` | arch+engine: a cabinet's host wall and its span along it | gate | claude-sonnet-d |
| `p15-render-light-set` | render: a render uses its light set (on/off and brightness of each light) | working | darwin-ws3 |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
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
| Nov 06 15:27 | `p15-cmd-level-free` | merged | 7,607 pass | mai-flash-esc |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 934 |
| bonsai-b | bonsai2-27b | idle | 864 |
| bonsai-ws2-a | bonsai2-27b | idle | 966 |
| bonsai-ws2-b | bonsai2-27b | idle | 928 |
| claude-sonnet-a | sonnet | idle | 86 |
| claude-sonnet-b | sonnet | idle | 84 |
| claude-sonnet-c | sonnet | merging | 38 |
| claude-sonnet-d | sonnet | gate | 25 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 812 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 67 |
| mai-flash-esc | mai-code-1.1-flash | idle | 38 |
| tturbo-mac | tturbo-ultraheretic | gate | 632 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
