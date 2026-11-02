# arch3d build status

Updated 2026-11-02 04:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 12 of 13: Exports and interoperability — 6 of 12 segments merged · 6 of 15 acceptance checks**

6,601 workspace tests passing at the last merge · 666 segments merged in all · 0 of 13 seats working

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
| 12 | Exports and interoperability | building | 6 / 15 | 4.8 h so far · expected done Nov 02 07:49–Nov 02 21:22 UTC |
| 13 | Polish | planned | — | — |

## Phase 12 segments

6 merged, 3 merging, 2 escalate, 1 queued

| Segment | What | State | Seat |
|---|---|---|---|
| `p12-ifc-spaces` | IFC spaces with their areas | escalate |  |
| `p12-ifc-site` | IFC site location and geographic elements | escalate |  |
| `p12-gltf-valid` | glTF passes the Khronos validator: spot cones | merging | bonsai-b |
| `p12-cmd-export` | The export command writes IFC, OBJ (+MTL) and GLB | merging | darwin-ws3 |
| `p12-pt-auto-exposure` | Path-traced daylight stills expose for what they show | merging | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
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
| Nov 01 21:58 | `p11f-glass-shadow-once` | refused | 0 pass | bonsai-b |
| Nov 01 21:56 | `p11f-explicit-spp-kept` | merged | 6,444 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 789 |
| bonsai-b | bonsai2-27b | merging | 732 |
| bonsai-ws2-a | bonsai2-27b | idle | 786 |
| bonsai-ws2-b | bonsai2-27b | idle | 770 |
| claude-sonnet-a | sonnet | idle | 69 |
| claude-sonnet-b | sonnet | idle | 65 |
| claude-sonnet-c | sonnet | idle | 15 |
| claude-sonnet-d | sonnet | idle | 9 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 650 |
| kimi-k31 | k3.1-pro-highspeed | down | 111 |
| mai-flash | mai-code-1.1-flash | merging | 48 |
| tturbo-mac | tturbo-ultraheretic | merging | 558 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
