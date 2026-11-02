# arch3d build status

Updated 2026-11-02 06:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 12 of 13: Exports and interoperability — 8 of 12 segments merged · 8 of 15 acceptance checks**

6,609 workspace tests passing at the last merge · 668 segments merged in all · 0 of 13 seats working

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
| 12 | Exports and interoperability | building | 8 / 15 | 6.8 h so far · expected done Nov 02 08:42–Nov 02 22:21 UTC |
| 13 | Polish | planned | — | — |

## Phase 12 segments

8 merged, 2 gate, 1 escalate, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p12-ifc-spaces` | IFC spaces with their areas | escalate |  |
| `p12-ifc-site` | IFC site location and geographic elements | gate | mai-flash-esc |
| `p12-gltf-valid` | glTF passes the Khronos validator: spot cones | merging | bonsai-b |
| `p12-docs-user-guide` | User guide: exporting IFC, OBJ and glTF | gate | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 02 04:16 | `p12-cmd-export` | merged | 6,609 pass | darwin-ws3 |
| Nov 02 04:03 | `p12-pt-auto-exposure` | merged | 6,605 pass | mai-flash |
| Nov 02 03:51 | `p12-ifc-slabs` | merged | 6,601 pass | claude-sonnet-b |
| Nov 02 03:40 | `p12-ifc-openings` | merged | 6,598 pass | claude-sonnet-d |
| Nov 02 03:28 | `p12-ifc-roofs` | merged | 6,595 pass | claude-sonnet-c |
| Nov 02 03:17 | `p12-ifc-stairs` | merged | 6,592 pass | bonsai-a |
| Nov 02 02:44 | `p12-io-obj` | merged | 6,590 pass | claude-sonnet-a |
| Nov 02 01:48 | `p12-ifc-lights` | merged | 6,586 pass | mai-flash |
| Nov 02 00:57 | `harden-p11f-glass-shadow-once` | merged | 6,584 pass | tturbo-mac |
| Nov 01 23:41 | `harden-p11-cli-render-args` | merged | 6,561 pass | tturbo-mac |
| Nov 01 22:41 | `upstream-sync-2034d6899` | merged | 6,550 pass | claude-sonnet-d |
| Nov 01 22:12 | `p11f-glass-shadow-once` | merged | 6,447 pass | darwin-ws3 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 790 |
| bonsai-b | bonsai2-27b | gate | 732 |
| bonsai-ws2-a | bonsai2-27b | gate | 791 |
| bonsai-ws2-b | bonsai2-27b | gate | 774 |
| claude-sonnet-a | sonnet | idle | 69 |
| claude-sonnet-b | sonnet | idle | 67 |
| claude-sonnet-c | sonnet | idle | 15 |
| claude-sonnet-d | sonnet | idle | 9 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 655 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 48 |
| tturbo-mac | tturbo-ultraheretic | gate | 558 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
