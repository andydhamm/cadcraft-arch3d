# arch3d build status

Updated 2026-11-01 08:15 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 11 of 13: Ray-traced Finalize Render — 1 of 23 segments merged · 0 of 13 acceptance checks**

6,232 workspace tests passing at the last merge · 617 segments merged in all · 0 of 13 seats working

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
| 11 | Ray-traced Finalize Render | building | 0 / 13 | 0.6 h so far · expected done Nov 01 15:04–Nov 03 07:47 UTC |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 11 segments

16 queued, 4 gate, 1 merged, 1 working, 1 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p11-arch-ies` | IES LM-63 files: parse, interpolate with symmetry, scale to a fixture | gate | bonsai-ws2-a |
| `p11-arch-panel` | Area fixtures in plan: validation, the panel symbol, the panel at the ceiling | working | bonsai-b |
| `p11-arch3d-panel` | The LED panel in 3D: frame, diffuser and its ceiling opening | gate | darwin-ws3 |
| `p11-r3d-gpu-scene` | The scene as path-tracer buffers: SAH BVH, materials, lights, IES tables, CPU traversal | gate | mai-flash |
| `p11-r3d-pt-bsdf` | WGSL materials: GGX glossy and mirror from roughness and metalness, glass with Fresnel and refraction | merging | bonsai-ws2-b |
| `p11-r3d-denoise` | The edge-aware à-trous denoiser in WGSL, guided by albedo and normals | gate | bonsai-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 08:07 | `p11-arch-ies` | refused | 0 pass | bonsai-ws2-a |
| Nov 01 07:40 | `upstream-sync-d7ef7da87` | merged | 6,232 pass | bonsai-ws2-b |
| Nov 01 07:36 | `harden-p10-cli-render-hook` | merged | 6,232 pass | tturbo-mac |
| Nov 01 06:39 | `harden-p10-cmd-illuminance` | merged | 6,227 pass | tturbo-mac2 |
| Nov 01 06:17 | `p10-docs-user-guide` | merged | 6,207 pass | kimi-k31 |
| Nov 01 05:57 | `harden-p10-cli-render-final` | merged | 6,204 pass | tturbo-mac |
| Nov 01 05:41 | `p10-docs-user-guide` | refused | 6,200 pass | bonsai-ws2-a |
| Nov 01 05:02 | `p10-docs-user-guide` | refused | 0 pass | bonsai-b |
| Nov 01 04:55 | `p10-docs-user-guide` | refused | 6,201 pass | mai-flash |
| Nov 01 04:28 | `harden-p10-cmd-lightfixture` | merged | 6,195 pass | tturbo-mac |
| Nov 01 04:13 | `p10-cli-render-final` | merged | 6,172 pass | claude-sonnet-b |
| Nov 01 03:58 | `p10-cli-render-hook` | merged | 6,168 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 755 |
| bonsai-b | bonsai2-27b | agent | 698 |
| bonsai-ws2-a | bonsai2-27b | gate | 752 |
| bonsai-ws2-b | bonsai2-27b | merging | 720 |
| claude-sonnet-a | sonnet | held | 61 |
| claude-sonnet-b | sonnet | held | 60 |
| claude-sonnet-c | sonnet | held | 10 |
| claude-sonnet-d | sonnet | held | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 602 |
| kimi-k31 | k3.1-pro-highspeed | idle | 111 |
| mai-flash | mai-code-1.1-flash | gate | 37 |
| tturbo-mac | tturbo-ultraheretic | merging | 526 |
| tturbo-mac2 | tturbo-ultraheretic | gate | 346 |
