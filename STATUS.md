# arch3d build status

Updated 2026-11-02 21:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 13 of 13: Polish — 5 of 22 segments merged · 0 of 13 acceptance checks**

6,699 workspace tests passing at the last merge · 684 segments merged in all · 0 of 14 seats working

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
| 13 | Polish | building | 0 / 13 | 7.3 h so far · expected done Nov 03 16:52–Nov 05 03:18 UTC |

## Phase 13 segments

11 queued, 5 merged, 5 merging, 1 replaced, 1 escalate, 1 abandoned

| Segment | What | State | Seat |
|---|---|---|---|
| `p13-arch3d-incremental` | Incremental 3D rebuild: kind-tagged dirty set, scene cache, digest | escalate |  |
| `p13-sample-house` | arch.sample {"house":true}: the 300-wall benchmark house | merging | mai-flash-esc |
| `p13-arch-casework-symbols` | Plan symbols for cabinets and fixtures | merging | claude-sonnet-c |
| `p13-cmd-casework` | cabinet and fixture commands, inspect | merging | claude-sonnet-d |
| `p13-inspect-finishes` | arch.inspect reports wall finish, roof covering and room floor finish | merging | claude-sonnet-b |
| `upstream-sync-dae4b7b2f` | merge upstream upstream/main up to dae4b7b2f (3 commits) | merging | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 02 21:39 | `p13-io-finish-round-trip` | merged | 6,699 pass | bonsai-ws2-b |
| Nov 02 20:55 | `p13-doc-casework` | merged | 6,697 pass | claude-sonnet-c |
| Nov 02 20:20 | `p13-finish-regeneration` | merged | 6,689 pass | claude-sonnet-a |
| Nov 02 19:54 | `p13-cmd-material` | merged | 6,687 pass | claude-sonnet-d |
| Nov 02 19:03 | `p13-inspect-finishes` | refused | 6,683 pass | claude-sonnet-b |
| Nov 02 17:51 | `p13-doc-casework` | refused | 6,689 pass | bonsai-a |
| Nov 02 16:23 | `p13-doc-casework` | refused | 0 pass | claude-sonnet-c |
| Nov 02 16:23 | `p13-doc-finish-data` | merged | 6,681 pass | claude-sonnet-d |
| Nov 02 13:21 | `p12f-ifc-step-grammar` | merged | 6,678 pass | darwin-ws3 |
| Nov 02 12:36 | `upstream-sync-ebc67fa81` | merged | 6,674 pass | bonsai-ws2-a |
| Nov 02 11:32 | `harden-p12-gltf-valid` | merged | 6,662 pass | tturbo-mac |
| Nov 02 10:51 | `harden-p12-ifc-site` | merged | 6,655 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 813 |
| bonsai-b | bonsai2-27b | agent | 758 |
| bonsai-ws2-a | bonsai2-27b | agent | 820 |
| bonsai-ws2-b | bonsai2-27b | agent | 797 |
| claude-sonnet-a | sonnet | idle | 71 |
| claude-sonnet-b | sonnet | merging | 69 |
| claude-sonnet-c | sonnet | merging | 19 |
| claude-sonnet-d | sonnet | merging | 12 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | yielding | 677 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | merging | 52 |
| mai-flash-esc | mai-code-1.1-flash | merging | 9 |
| tturbo-mac | tturbo-ultraheretic | merging | 579 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
