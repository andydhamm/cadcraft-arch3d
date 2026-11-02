# arch3d build status

Updated 2026-11-02 11:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 12 of 13: Exports and interoperability — 12 of 13 segments merged · 14 of 15 acceptance checks**

6,655 workspace tests passing at the last merge · 676 segments merged in all · 0 of 14 seats working

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
| 12 | Exports and interoperability | building | 14 / 15 | 12.3 h so far · expected done Nov 02 12:22–Nov 03 00:17 UTC |
| 13 | Polish | planned | — | — |

## Phase 12 segments

12 merged, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p12f-ifc-step-grammar` | IFC export round-trip: every instance parses and has IFC4's attribute count | gate | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 02 10:51 | `harden-p12-ifc-site` | merged | 6,655 pass | tturbo-mac |
| Nov 02 10:02 | `harden-p12-ifc-spaces` | merged | 6,642 pass | tturbo-mac |
| Nov 02 09:37 | `harden-p12-cmd-export` | merged | 6,636 pass | tturbo-mac |
| Nov 02 09:14 | `p12-ifc-spaces` | merged | 6,625 pass | bonsai-ws2-a |
| Nov 02 08:49 | `p12-gltf-valid` | merged | 6,622 pass | darwin-ws3 |
| Nov 02 08:20 | `harden-p12-cmd-export` | refused | 6,630 pass, 1 fail | tturbo-mac |
| Nov 02 08:03 | `p12-docs-user-guide` | merged | 6,620 pass | bonsai-a |
| Nov 02 07:37 | `p12-docs-user-guide` | refused | 6,620 pass | bonsai-ws2-a |
| Nov 02 07:06 | `harden-p11-arch-panel` | merged | 6,619 pass | tturbo-mac |
| Nov 02 06:16 | `p12-gltf-valid` | refused | 0 pass | bonsai-b |
| Nov 02 04:16 | `p12-cmd-export` | merged | 6,609 pass | darwin-ws3 |
| Nov 02 04:03 | `p12-pt-auto-exposure` | merged | 6,605 pass | mai-flash |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 799 |
| bonsai-b | bonsai2-27b | idle | 745 |
| bonsai-ws2-a | bonsai2-27b | idle | 799 |
| bonsai-ws2-b | bonsai2-27b | agent | 789 |
| claude-sonnet-a | sonnet | idle | 69 |
| claude-sonnet-b | sonnet | idle | 67 |
| claude-sonnet-c | sonnet | idle | 15 |
| claude-sonnet-d | sonnet | idle | 9 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 667 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 48 |
| mai-flash-esc | mai-code-1.1-flash | idle | 1 |
| tturbo-mac | tturbo-ultraheretic | merging | 564 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
