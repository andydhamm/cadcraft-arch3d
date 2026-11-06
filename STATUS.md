# arch3d build status

Updated 2026-11-06 23:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 15 of 15: Electrical, technical illustration and the showcase house — 35 of 42 segments merged · 2 of 14 acceptance checks**

7,633 workspace tests passing at the last merge · 799 segments merged in all · 0 of 14 seats working

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
| 15 | Electrical, technical illustration and the showcase house | building | 2 / 14 | 38.8 h so far · expected done Nov 07 05:58–Nov 08 02:30 UTC |

## Phase 15 segments

35 merged, 4 replaced, 4 queued, 2 escalate, 1 merging, 1 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p15-io-electrical` | io: electrical devices and connections in GLB, OBJ, IFC and DXF | merging | mai-flash-esc |
| `p15-cmd-connections` | engine: connection, connection.edit, connection.delete, circuit.brightness | escalate |  |
| `p15-inspect` | engine: arch.inspect for phase 15 | escalate |  |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 06 22:23 | `p15-cmd-outlets-auto` | merged | 7,633 pass | mai-flash |
| Nov 06 21:33 | `p15-inspect` | refused | 7,635 pass | claude-sonnet-a |
| Nov 06 19:58 | `p15-io-electrical` | refused | 0 pass | darwin-ws3 |
| Nov 06 19:54 | `p15-render-electrical` | merged | 7,630 pass | mai-flash-esc |
| Nov 06 19:01 | `harden-p15-cmd-wall-span` | merged | 7,627 pass | tturbo-mac |
| Nov 06 17:37 | `p15-cmd-electrical` | merged | 7,618 pass | mai-flash |
| Nov 06 16:38 | `p15-arch-outlet-gaps` | merged | 7,612 pass | claude-sonnet-a |
| Nov 06 16:09 | `p15-cli-render-styles` | merged | 7,609 pass | claude-sonnet-d |
| Nov 06 15:27 | `p15-cmd-level-free` | merged | 7,607 pass | mai-flash-esc |
| Nov 06 14:52 | `p15-arch-circuits` | merged | 7,602 pass | bonsai-ws2-b |
| Nov 06 14:25 | `p15-cmd-room-bed` | merged | 7,600 pass | bonsai-a |
| Nov 06 14:07 | `upstream-sync-2611bc5f5` | refused | 7,599 pass | bonsai-ws2-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 931 |
| bonsai-b | bonsai2-27b | gate | 857 |
| bonsai-ws2-a | bonsai2-27b | gate | 964 |
| bonsai-ws2-b | bonsai2-27b | agent | 924 |
| claude-sonnet-a | sonnet | idle | 86 |
| claude-sonnet-b | sonnet | idle | 84 |
| claude-sonnet-c | sonnet | idle | 37 |
| claude-sonnet-d | sonnet | idle | 25 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 809 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 67 |
| mai-flash-esc | mai-code-1.1-flash | merging | 37 |
| tturbo-mac | tturbo-ultraheretic | merging | 628 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
