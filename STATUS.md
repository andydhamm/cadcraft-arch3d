# arch3d build status

Updated 2026-11-01 11:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 9 of 23 segments merged · 0 of 13 acceptance checks**

6,309 workspace tests passing at the last merge · 628 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 0 / 13 | 3.8 h so far · expected done Nov 01 16:51–Nov 02 02:42 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

9 merged, 8 queued, 2 merging, 2 escalate, 2 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-arch3d-panel` | The LED panel in 3D: frame, diffuser and its ceiling opening | merging | darwin-ws3 |
| `p11-arch3d-ies-area` | IES distributions and area lights in the scene lights and the CPU illuminance | merging | bonsai-ws2-a |
| `p11-r3d-pt-lights` | WGSL lights: points, spots, IES tables, rectangular area lights, the sun and the sky | escalate |  |
| `p11-r3d-pt-driver` | The progressive driver: frames, passes, snapshots, resolve, denoise hook, cancel; irradiance probes | working | bonsai-b |
| `p11-cmd-lightfixture-area-ies` | LIGHTFIXTURE area lights and IES files; arch.inspect shows them | escalate |  |
| `p11-io-area-ies` | GLB and DXF for area lights and IES fixtures | working | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 11:23 | `p11-cmd-render-path` | merged | 6,309 pass | bonsai-ws2-b |
| Nov 01 11:07 | `harden-p10-mcp-render-final` | merged | 6,305 pass | tturbo-mac |
| Nov 01 10:55 | `p11-r3d-pipeline-cache` | merged | 6,300 pass | mai-flash |
| Nov 01 10:42 | `harden-p9-arch-terrain-csv` | merged | 6,296 pass | tturbo-mac2 |
| Nov 01 10:20 | `p11-arch3d-panel` | refused | 0 pass | darwin-ws3 |
| Nov 01 10:11 | `p11-r3d-pipeline-cache` | refused | 6,270 pass | mai-flash |
| Nov 01 09:47 | `p11-r3d-denoise` | merged | 6,266 pass | bonsai-a |
| Nov 01 09:30 | `p11-r3d-pt-traverse` | merged | 6,259 pass | mai-flash |
| Nov 01 09:22 | `p11-arch-panel` | merged | 6,256 pass | bonsai-b |
| Nov 01 09:08 | `p11-arch-ies` | merged | 6,252 pass | bonsai-ws2-a |
| Nov 01 08:48 | `p11-r3d-pt-bsdf` | merged | 6,247 pass | bonsai-ws2-b |
| Nov 01 08:40 | `p11-r3d-gpu-scene` | merged | 6,242 pass | mai-flash |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 761 |
| bonsai-b | bonsai2-27b | agent | 703 |
| bonsai-ws2-a | bonsai2-27b | merging | 761 |
| bonsai-ws2-b | bonsai2-27b | gate | 732 |
| claude-sonnet-a | sonnet | idle | 61 |
| claude-sonnet-b | sonnet | idle | 60 |
| claude-sonnet-c | sonnet | idle | 10 |
| claude-sonnet-d | sonnet | idle | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 604 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | idle | 44 |
| tturbo-mac | tturbo-ultraheretic | gate | 528 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 348 |
