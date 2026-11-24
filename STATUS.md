# arch3d build status

Updated 2026-11-24 00:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 11 of 35 segments merged · 1 of 15 acceptance checks**

11,451 workspace tests passing at the last merge · 1333 segments merged in all · 0 of 15 seats working

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
| 13 | Polish | done | 13 / 13 | 29.0 h |
| 14 | The library | done | 13 / 13 | 36.9 h |
| 15 | Electrical, technical illustration and the showcase house | done | 14 / 14 | 115.8 h |
| 16 | Ceilings and foundations | done | 20 / 20 | 63.2 h |
| 17 | Stairs, decks, railings and openings in detail | done | 14 / 14 | 49.6 h |
| 18 | Interior finish, kitchen and bath | done | 20 / 20 | 96.7 h |
| 19 | Light, sky and cameras | done | 17 / 17 | 118.1 h |
| 20 | Plan views and dimensions | done | 13 / 13 | 54.9 h |
| 21 | Schedules, elevations and sheets | done | 13 / 13 | 105.8 h |
| 22 | Photoreal stills | building | 1 / 15 | 7.3 h so far · expected done Nov 24 14:45–Nov 25 15:23 UTC |

## Phase 22 segments

13 queued, 11 merged, 10 merging, 1 working

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-05d4e64ad` | merge upstream upstream/main up to 05d4e64ad (27 commits) | merging | mai-flash-esc |
| `p22-r3d-material-map` | Fill the GPU material's extension fields from MaterialExt | merging | claude-sonnet-c |
| `p22-engine-render-params` | Engine: the photoreal render params, the preset, focus pick and calibrate on RenderRequest | merging | claude-sonnet-b |
| `p22-r3d-linear-out` | Path tracer: keep the linear image (radiance and film stages) | merging | mai-flash |
| `p22-r3d-lens-path` | Path tracer ray generation: Panini, shift and the thin lens | merging | claude-sonnet-d |
| `p22-engine-pick-lens` | pick3d maps the pixel back through the lens | merging | mai-flash-b |
| `p22-r3d-sun-disc` | The sun as a 0.53 degree disc in the path tracer | merging | claude-sonnet-a |
| `p22-r3d-clamp` | Per-sample clamp of indirect contributions, with its energy loss reported | merging | mai-flash-b |
| `p22-r3d-adaptive` | Adaptive sampling, the noise target and the time budget | working | tturbo-mac2 |
| `p22-r3d-roughness-guide` | The edge-aware filter gains the roughness buffer as a guide | merging | claude-sonnet-b |
| `p22-r3d-tza` | The .tza weight loader and the embedded denoiser weights | merging | bonsai-b |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 23 23:58 | `p22-r3d-tza` | refused | 0 pass | mai-flash-b |
| Nov 23 23:58 | `harden-p13-io-finish-round-trip` | merged | 11,451 pass | tturbo-mac |
| Nov 23 23:58 | `p22-ui-pane-photoreal` | merged | 11,451 pass | claude-sonnet-a |
| Nov 23 23:58 | `p22-r3d-lens-raster` | merged | 11,451 pass | claude-sonnet-b |
| Nov 23 23:58 | `p22-io-glb-ext` | merged | 11,451 pass | bonsai-b |
| Nov 23 23:58 | `p22-r3d-kernel-hooks` | merged | 11,451 pass | claude-sonnet-d |
| Nov 23 23:37 | `p22-arch3d-calibrate` | merged | 11,426 pass | claude-sonnet-c |
| Nov 23 23:10 | `p22-engine-look-keys` | merged | 11,421 pass | claude-sonnet-c |
| Nov 23 22:49 | `p22-r3d-types` | merged | 11,415 pass | claude-sonnet-d |
| Nov 23 22:15 | `p22-arch3d-lens` | merged | 11,410 pass | claude-sonnet-d |
| Nov 23 21:58 | `p22-arch3d-material-ext` | merged | 11,404 pass | claude-sonnet-c |
| Nov 23 21:24 | `p22-doc-look` | merged | 11,399 pass | claude-sonnet-a |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1405 |
| bonsai-b | bonsai2-27b | agent | 1313 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 167 |
| claude-sonnet-b | sonnet | idle | 174 |
| claude-sonnet-c | sonnet | idle | 108 |
| claude-sonnet-d | sonnet | idle | 105 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 1296 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 129 |
| mai-flash-b | mai-code-1.1-flash | idle | 84 |
| mai-flash-esc | mai-code-1.1-flash | idle | 177 |
| tturbo-mac | tturbo-ultraheretic | agent | 1200 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 705 |
