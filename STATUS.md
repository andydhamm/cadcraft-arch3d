# arch3d build status

Updated 2026-11-01 09:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 6 of 23 segments merged · 0 of 13 acceptance checks**

6,259 workspace tests passing at the last merge · 623 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 0 / 13 | 2.1 h so far · expected done Nov 01 15:10–Nov 02 01:03 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

10 queued, 6 merged, 3 merging, 2 gate, 2 working

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-arch3d-panel` | The LED panel in 3D: frame, diffuser and its ceiling opening | merging | darwin-ws3 |
| `p11-arch3d-ies-area` | IES distributions and area lights in the scene lights and the CPU illuminance | gate | bonsai-ws2-a |
| `p11-r3d-pt-lights` | WGSL lights: points, spots, IES tables, rectangular area lights, the sun and the sky | working | kimi-k31 |
| `p11-r3d-pt-driver` | The progressive driver: frames, passes, snapshots, resolve, denoise hook, cancel; irradiance probes | working | bonsai-b |
| `p11-r3d-denoise` | The edge-aware à-trous denoiser in WGSL, guided by albedo and normals | merging | bonsai-a |
| `p11-r3d-pipeline-cache` | The on-disk pipeline cache: directory, file per adapter, open and save | merging | mai-flash |
| `p11-cmd-render-path` | RENDER path-tracer parameters and ILLUMINANCE method path | gate | bonsai-ws2-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 09:30 | `p11-r3d-pt-traverse` | merged | 6,259 pass | mai-flash |
| Nov 01 09:22 | `p11-arch-panel` | merged | 6,256 pass | bonsai-b |
| Nov 01 09:08 | `p11-arch-ies` | merged | 6,252 pass | bonsai-ws2-a |
| Nov 01 08:48 | `p11-r3d-pt-bsdf` | merged | 6,247 pass | bonsai-ws2-b |
| Nov 01 08:40 | `p11-r3d-gpu-scene` | merged | 6,242 pass | mai-flash |
| Nov 01 08:33 | `harden-p10-docs-user-guide` | merged | 6,235 pass | tturbo-mac |
| Nov 01 08:07 | `p11-arch-ies` | refused | 0 pass | bonsai-ws2-a |
| Nov 01 07:40 | `upstream-sync-d7ef7da87` | merged | 6,232 pass | bonsai-ws2-b |
| Nov 01 07:36 | `harden-p10-cli-render-hook` | merged | 6,232 pass | tturbo-mac |
| Nov 01 06:39 | `harden-p10-cmd-illuminance` | merged | 6,227 pass | tturbo-mac2 |
| Nov 01 06:17 | `p10-docs-user-guide` | merged | 6,207 pass | kimi-k31 |
| Nov 01 05:57 | `harden-p10-cli-render-final` | merged | 6,204 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | merging | 756 |
| bonsai-b | bonsai2-27b | agent | 700 |
| bonsai-ws2-a | bonsai2-27b | gate | 757 |
| bonsai-ws2-b | bonsai2-27b | gate | 725 |
| claude-sonnet-a | sonnet | held | 61 |
| claude-sonnet-b | sonnet | held | 60 |
| claude-sonnet-c | sonnet | held | 10 |
| claude-sonnet-d | sonnet | held | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 603 |
| kimi-k31 | k3.1-pro-highspeed | agent | 111 |
| mai-flash | mai-code-1.1-flash | merging | 43 |
| tturbo-mac | tturbo-ultraheretic | merging | 527 |
| tturbo-mac2 | tturbo-ultraheretic | merging | 348 |
