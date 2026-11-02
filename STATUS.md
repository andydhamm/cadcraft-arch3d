# arch3d build status

Updated 2026-11-02 16:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 13 of 13: Polish — 1 of 21 segments merged · 0 of 13 acceptance checks**

6,681 workspace tests passing at the last merge · 680 segments merged in all · 0 of 14 seats working

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
| 13 | Polish | building | 0 / 13 | 2.3 h so far · expected done Nov 03 11:14–Nov 10 04:46 UTC |

## Phase 13 segments

14 queued, 4 gate, 2 working, 1 replaced, 1 merged

| Segment | What | State | Seat |
|---|---|---|---|
| `p13-doc-casework` | Cabinets and fixtures: data model and catalogue | gate | claude-sonnet-c |
| `p13-arch3d-incremental` | Incremental 3D rebuild: kind-tagged dirty set, scene cache, digest | gate | mai-flash-esc |
| `p13-sample-house` | arch.sample {"house":true}: the 300-wall benchmark house | working | mai-flash |
| `p13-cmd-material` | The `material` command: assign a finish to walls, roofs or rooms | working | claude-sonnet-d |
| `p13-inspect-finishes` | arch.inspect reports wall finish, roof covering and room floor finish | gate | claude-sonnet-b |
| `p13-finish-regeneration` | Finishes survive roofauto and roomdetect regeneration | gate | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 02 16:23 | `p13-doc-casework` | refused | 0 pass | claude-sonnet-c |
| Nov 02 16:23 | `p13-doc-finish-data` | merged | 6,681 pass | claude-sonnet-d |
| Nov 02 13:21 | `p12f-ifc-step-grammar` | merged | 6,678 pass | darwin-ws3 |
| Nov 02 12:36 | `upstream-sync-ebc67fa81` | merged | 6,674 pass | bonsai-ws2-a |
| Nov 02 11:32 | `harden-p12-gltf-valid` | merged | 6,662 pass | tturbo-mac |
| Nov 02 10:51 | `harden-p12-ifc-site` | merged | 6,655 pass | tturbo-mac |
| Nov 02 10:02 | `harden-p12-ifc-spaces` | merged | 6,642 pass | tturbo-mac |
| Nov 02 09:37 | `harden-p12-cmd-export` | merged | 6,636 pass | tturbo-mac |
| Nov 02 09:14 | `p12-ifc-spaces` | merged | 6,625 pass | bonsai-ws2-a |
| Nov 02 08:49 | `p12-gltf-valid` | merged | 6,622 pass | darwin-ws3 |
| Nov 02 08:20 | `harden-p12-cmd-export` | refused | 6,630 pass, 1 fail | tturbo-mac |
| Nov 02 08:03 | `p12-docs-user-guide` | merged | 6,620 pass | bonsai-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 812 |
| bonsai-b | bonsai2-27b | gate | 749 |
| bonsai-ws2-a | bonsai2-27b | gate | 803 |
| bonsai-ws2-b | bonsai2-27b | gate | 795 |
| claude-sonnet-a | sonnet | gate | 69 |
| claude-sonnet-b | sonnet | gate | 67 |
| claude-sonnet-c | sonnet | gate | 17 |
| claude-sonnet-d | sonnet | agent | 10 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 674 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 50 |
| mai-flash-esc | mai-code-1.1-flash | gate | 4 |
| tturbo-mac | tturbo-ultraheretic | gate | 573 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
