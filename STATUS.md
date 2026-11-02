# arch3d build status

Updated 2026-11-02 15:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 13 of 13: Polish — 0 of 21 segments merged · 0 of 13 acceptance checks**

6,678 workspace tests passing at the last merge · 679 segments merged in all · 0 of 14 seats working

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
| 13 | Polish | building | 0 / 13 | 1.1 h so far · expected done Nov 02 21:03–Nov 04 23:53 UTC |

## Phase 13 segments

17 queued, 2 working, 1 replaced, 1 gate, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p13-doc-casework` | Cabinets and fixtures: data model and catalogue | gate | claude-sonnet-c |
| `p13-arch3d-incremental` | Incremental 3D rebuild: kind-tagged dirty set, scene cache, digest | working | bonsai-ws2-b |
| `p13-sample-house` | arch.sample {"house":true}: the 300-wall benchmark house | working | mai-flash |
| `p13-doc-finish-data` | Finish enum and the finish fields on Wall / Roof / Room | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
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
| Nov 02 07:37 | `p12-docs-user-guide` | refused | 6,620 pass | bonsai-ws2-a |
| Nov 02 07:06 | `harden-p11-arch-panel` | merged | 6,619 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 810 |
| bonsai-b | bonsai2-27b | gate | 747 |
| bonsai-ws2-a | bonsai2-27b | agent | 801 |
| bonsai-ws2-b | bonsai2-27b | agent | 794 |
| claude-sonnet-a | sonnet | idle | 69 |
| claude-sonnet-b | sonnet | idle | 67 |
| claude-sonnet-c | sonnet | gate | 16 |
| claude-sonnet-d | sonnet | merging | 10 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 673 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | agent | 49 |
| mai-flash-esc | mai-code-1.1-flash | idle | 3 |
| tturbo-mac | tturbo-ultraheretic | gate | 571 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
