# arch3d build status

Updated 2026-11-02 02:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 12 of 13: Exports and interoperability — 1 of 12 segments merged · 1 of 15 acceptance checks**

6,586 workspace tests passing at the last merge · 661 segments merged in all · 0 of 13 seats working

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
| 12 | Exports and interoperability | building | 1 / 15 | 3.0 h so far · expected done Nov 02 15:30–Nov 09 03:35 UTC |
| 13 | Polish | planned | — | — |

## Phase 12 segments

5 merging, 4 gate, 1 working, 1 merged, 1 queued

| Segment | What | State | Seat |
|---|---|---|---|
| `p12-ifc-openings` | IFC openings: IfcOpeningElement voiding the wall, filled by IfcDoor or IfcWindow | merging | claude-sonnet-d |
| `p12-ifc-slabs` | IFC floors, decks and ceilings | merging | claude-sonnet-b |
| `p12-ifc-roofs` | IFC roofs: IfcRoof aggregating its planes, fascia and soffit | merging | claude-sonnet-c |
| `p12-ifc-stairs` | IFC stairs and railings | merging | bonsai-a |
| `p12-ifc-spaces` | IFC spaces with their areas | gate | bonsai-ws2-b |
| `p12-ifc-site` | IFC site location and geographic elements | working | bonsai-ws2-a |
| `p12-io-obj` | OBJ export with an MTL file | merging | claude-sonnet-a |
| `p12-gltf-valid` | glTF passes the Khronos validator: spot cones | gate | bonsai-b |
| `p12-cmd-export` | The export command writes IFC, OBJ (+MTL) and GLB | gate | darwin-ws3 |
| `p12-pt-auto-exposure` | Path-traced daylight stills expose for what they show | gate | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 02 01:48 | `p12-ifc-lights` | merged | 6,586 pass | mai-flash |
| Nov 02 00:57 | `harden-p11f-glass-shadow-once` | merged | 6,584 pass | tturbo-mac |
| Nov 01 23:41 | `harden-p11-cli-render-args` | merged | 6,561 pass | tturbo-mac |
| Nov 01 22:41 | `upstream-sync-2034d6899` | merged | 6,550 pass | claude-sonnet-d |
| Nov 01 22:12 | `p11f-glass-shadow-once` | merged | 6,447 pass | darwin-ws3 |
| Nov 01 21:58 | `p11f-glass-shadow-once` | refused | 0 pass | bonsai-b |
| Nov 01 21:56 | `p11f-explicit-spp-kept` | merged | 6,444 pass | claude-sonnet-a |
| Nov 01 21:39 | `p11f-denoise-variance` | merged | 6,441 pass | mai-flash |
| Nov 01 21:08 | `p11f-glass-shadow-once` | refused | 0 pass | darwin-ws3 |
| Nov 01 20:31 | `harden-p11-r3d-rt` | merged | 6,436 pass | tturbo-mac |
| Nov 01 19:58 | `harden-p11-ui-pane-path` | merged | 6,430 pass | tturbo-mac |
| Nov 01 19:23 | `harden-p11-final-default-path` | merged | 6,419 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 789 |
| bonsai-b | bonsai2-27b | gate | 731 |
| bonsai-ws2-a | bonsai2-27b | agent | 784 |
| bonsai-ws2-b | bonsai2-27b | gate | 767 |
| claude-sonnet-a | sonnet | merging | 69 |
| claude-sonnet-b | sonnet | merging | 65 |
| claude-sonnet-c | sonnet | merging | 15 |
| claude-sonnet-d | sonnet | merging | 9 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 647 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | gate | 46 |
| tturbo-mac | tturbo-ultraheretic | agent | 557 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
