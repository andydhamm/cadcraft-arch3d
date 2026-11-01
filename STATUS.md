# arch3d build status

Updated 2026-11-01 18:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 22 of 24 segments merged · 8 of 13 acceptance checks**

6,403 workspace tests passing at the last merge · 648 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 8 / 13 | 10.3 h so far · expected done Nov 01 18:58–Nov 02 04:12 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

22 merged, 2 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-r3d-rt` | Optional hardware ray queries: the same traversal through wgpu ray queries, parity with the compute path | merging | claude-sonnet-b |
| `p11-final-default-path` | Path traced is the default Finalize | merging | claude-sonnet-d |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 17:18 | `harden-p11-io-area-ies` | merged | 6,403 pass | tturbo-mac |
| Nov 01 17:02 | `p11-docs-user-guide` | merged | 6,395 pass | bonsai-ws2-a |
| Nov 01 16:24 | `p11-cli-render-args` | merged | 6,394 pass | claude-sonnet-d |
| Nov 01 16:11 | `harden-p11-r3d-denoise` | merged | 6,387 pass | tturbo-mac |
| Nov 01 15:57 | `p11-cli-render-path` | merged | 6,383 pass | claude-sonnet-a |
| Nov 01 15:44 | `harden-p11-r3d-final-path` | merged | 6,378 pass | tturbo-mac |
| Nov 01 15:17 | `p11-ui-pane-path` | refused | 6,375 pass | bonsai-ws2-a |
| Nov 01 15:08 | `harden-p11-arch3d-ies-area` | merged | 6,372 pass | tturbo-mac |
| Nov 01 14:25 | `upstream-sync-258c89fbb` | merged | 6,367 pass | claude-sonnet-b |
| Nov 01 14:12 | `p11-r3d-final-path` | merged | 6,365 pass | claude-sonnet-c |
| Nov 01 14:04 | `harden-p11-cmd-lightfixture-area-ies` | merged | 6,361 pass | tturbo-mac |
| Nov 01 13:41 | `p11-r3d-pt-driver` | merged | 6,355 pass | darwin-ws3 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 782 |
| bonsai-b | bonsai2-27b | idle | 722 |
| bonsai-ws2-a | bonsai2-27b | gate | 777 |
| bonsai-ws2-b | bonsai2-27b | gate | 757 |
| claude-sonnet-a | sonnet | idle | 67 |
| claude-sonnet-b | sonnet | merging | 63 |
| claude-sonnet-c | sonnet | idle | 14 |
| claude-sonnet-d | sonnet | merging | 6 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | agent | 627 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | idle | 44 |
| tturbo-mac | tturbo-ultraheretic | gate | 539 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
