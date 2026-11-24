# arch3d build status

Updated 2026-11-24 04:00 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 22 of 36 segments merged · 1 of 15 acceptance checks**

11,632 workspace tests passing at the last merge · 1345 segments merged in all · 0 of 15 seats working

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
| 22 | Photoreal stills | building | 1 / 15 | 10.5 h so far · expected done Nov 24 09:37–Nov 25 00:18 UTC |

## Phase 22 segments

22 merged, 8 merging, 4 queued, 1 working, 1 gate

| Segment | What | State | Seat |
|---|---|---|---|
| `p22-cli-hook` | CLI renderer: photoreal options into the renderers, calibrate scenes, PFM output and the reply keys | merging | claude-sonnet-b |
| `p22-r3d-edges` | Rounded-edge shading in the path tracer | merging | mai-flash-b |
| `p22-r3d-bsdf-ext` | Path tracer BSDF: clear coat, sheen and anisotropy | working | bonsai-a |
| `p22-r3d-bsdf-transmission` | Path tracer BSDF: thin translucency and rough glass with volume | merging | claude-sonnet-b |
| `p22-r3d-variation` | Procedural surface variation (roughness and colour) in the path tracer | merging | claude-sonnet-d |
| `p22-r3d-sun-disc` | The sun as a 0.53 degree disc in the path tracer | merging | mai-flash |
| `p22-r3d-unet-cpu` | The denoiser network on the CPU: transforms, autoexposure and a reference forward pass | merging | claude-sonnet-a |
| `p22-engine-denoiser-info` | denoiser.info: the loaded weights and whether the learned denoiser can run | merging | claude-sonnet-d |
| `p22-engine-ladder` | render.ladder: one PNG per rung and a labelled contact sheet | merging | claude-sonnet-a |
| `upstream-sync-bf1d872e5` | merge upstream upstream/main up to bf1d872e5 (51 commits) | gate | claude-sonnet-c |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 24 02:27 | `harden-p22-engine-look-keys` | merged | 11,632 pass | tturbo-mac |
| Nov 24 02:27 | `p22-r3d-clamp` | merged | 11,632 pass | mai-flash-b |
| Nov 24 02:27 | `p22-r3d-roughness-guide` | merged | 11,632 pass | claude-sonnet-b |
| Nov 24 02:27 | `p22-r3d-film` | merged | 11,632 pass | claude-sonnet-a |
| Nov 24 02:27 | `p22-r3d-adaptive` | merged | 11,632 pass | mai-flash-esc |
| Nov 24 02:05 | `p22-r3d-tza` | merged | 11,591 pass | bonsai-b |
| Nov 24 01:39 | `p22-r3d-sun-disc` | refused | 0 pass | claude-sonnet-a |
| Nov 24 01:39 | `p22-engine-pick-lens` | merged | 11,586 pass | mai-flash-b |
| Nov 24 01:39 | `p22-r3d-lens-path` | merged | 11,586 pass | claude-sonnet-d |
| Nov 24 01:39 | `p22-r3d-linear-out` | merged | 11,586 pass | mai-flash |
| Nov 24 01:39 | `p22-r3d-material-map` | merged | 11,586 pass | claude-sonnet-c |
| Nov 24 01:39 | `p22-engine-render-params` | merged | 11,586 pass | claude-sonnet-b |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1415 |
| bonsai-b | bonsai2-27b | agent | 1325 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 170 |
| claude-sonnet-b | sonnet | idle | 177 |
| claude-sonnet-c | sonnet | gate | 109 |
| claude-sonnet-d | sonnet | idle | 108 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 1296 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 131 |
| mai-flash-b | mai-code-1.1-flash | idle | 85 |
| mai-flash-esc | mai-code-1.1-flash | idle | 178 |
| tturbo-mac | tturbo-ultraheretic | agent | 1205 |
| tturbo-mac2 | tturbo-ultraheretic | gate | 710 |
