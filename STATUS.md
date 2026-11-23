# arch3d build status

Updated 2026-11-23 23:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 7 of 35 segments merged · 0 of 15 acceptance checks**

11,426 workspace tests passing at the last merge · 1328 segments merged in all · 0 of 15 seats working

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
| 22 | Photoreal stills | building | 0 / 15 | 6.3 h so far · expected done Nov 24 23:14–Nov 26 12:33 UTC |

## Phase 22 segments

20 queued, 7 merged, 6 merging, 2 working

| Segment | What | State | Seat |
|---|---|---|---|
| `upstream-sync-05d4e64ad` | merge upstream upstream/main up to 05d4e64ad (27 commits) | merging | mai-flash-esc |
| `p22-r3d-kernel-hooks` | Path tracer groundwork: frame and material layouts, WGSL hook files, tracer hook points, all inert | merging | claude-sonnet-d |
| `p22-engine-render-params` | Engine: the photoreal render params, the preset, focus pick and calibrate on RenderRequest | working | claude-sonnet-b |
| `p22-r3d-lens-raster` | Raster views: the wide-angle projection and shift as a post-warp | merging | claude-sonnet-b |
| `p22-engine-pick-lens` | pick3d maps the pixel back through the lens | working | mai-flash-b |
| `p22-r3d-tza` | The .tza weight loader and the embedded denoiser weights | merging | mai-flash-b |
| `p22-io-glb-ext` | glTF export of the new PBR fields as KHR material extensions | merging | bonsai-b |
| `p22-ui-pane-photoreal` | 3D pane: Photoreal in the look row, the lens in the live view and its pick, Finalize as the preset | merging | claude-sonnet-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 23 23:37 | `p22-arch3d-calibrate` | merged | 11,426 pass | claude-sonnet-c |
| Nov 23 23:10 | `p22-engine-look-keys` | merged | 11,421 pass | claude-sonnet-c |
| Nov 23 22:49 | `p22-r3d-types` | merged | 11,415 pass | claude-sonnet-d |
| Nov 23 22:15 | `p22-arch3d-lens` | merged | 11,410 pass | claude-sonnet-d |
| Nov 23 21:58 | `p22-arch3d-material-ext` | merged | 11,404 pass | claude-sonnet-c |
| Nov 23 21:24 | `p22-doc-look` | merged | 11,399 pass | claude-sonnet-a |
| Nov 23 19:14 | `harden-p18-observables-1` | merged | 11,394 pass | tturbo-mac |
| Nov 23 17:27 | `harden-p18-observables-2` | merged | 11,377 pass | tturbo-mac |
| Nov 23 16:38 | `upstream-sync-0ebb256d3` | merged | 11,363 pass | darwin-ws3 |
| Nov 23 16:00 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | bonsai-a |
| Nov 23 15:33 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | mai-flash-b |
| Nov 23 14:57 | `upstream-sync-0ebb256d3` | refused | 11,363 pass | darwin-ws3 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1401 |
| bonsai-b | bonsai2-27b | agent | 1309 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 166 |
| claude-sonnet-b | sonnet | agent | 171 |
| claude-sonnet-c | sonnet | idle | 107 |
| claude-sonnet-d | sonnet | idle | 103 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | merging | 1296 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 128 |
| mai-flash-b | mai-code-1.1-flash | agent | 81 |
| mai-flash-esc | mai-code-1.1-flash | idle | 177 |
| tturbo-mac | tturbo-ultraheretic | agent | 1197 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 703 |
