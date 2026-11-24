# arch3d build status

Updated 2026-11-24 13:30 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 22 of 22: Photoreal stills — 36 of 41 segments merged · 10 of 15 acceptance checks**

11,879 workspace tests passing at the last merge · 1372 segments merged in all · 0 of 15 seats working

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
| 22 | Photoreal stills | building | 10 / 15 | 20.0 h so far · expected done Nov 24 15:30–Nov 25 01:50 UTC |

## Phase 22 segments

36 merged, 3 merging, 1 gate, 1 escalate

| Segment | What | State | Seat |
|---|---|---|---|
| `p22f-ladder-reply-look` | render.ladder: each rung's reply carries look, sunDir and albedo like render's | merging | darwin-ws3 |
| `p22f-adaptive-freeze` | Adaptive sampling that really adapts: set PHOTO_ADAPTIVE, freeze converged pixels, per-pixel sample statistics | gate | claude-sonnet-c |
| `p22f-learned-keeps-converged` | Learned denoiser: a converged pixel keeps its value (flat frame within 1 %), and the device network is checked at 160 x 96 | merging | claude-sonnet-a |
| `p22f-aniso-multiscatter` | Anisotropic specular lobe: multiple-scattering energy compensation so a white anisotropic metal passes the furnace | merging | claude-sonnet-d |
| `upstream-sync-a1d92d995` | merge upstream upstream/main up to a1d92d995 (35 commits) | escalate |  |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 24 13:12 | `harden-p15-render-plan-level` | merged | 11,879 pass | tturbo-mac |
| Nov 24 11:45 | `upstream-sync-bf1d872e5` | merged | 11,874 pass | claude-sonnet-c |
| Nov 24 11:15 | `harden-p22-ui-pane-photoreal` | merged | 11,772 pass | tturbo-mac |
| Nov 24 11:15 | `harden-p22-r3d-unet-gpu` | merged | 11,772 pass | tturbo-mac2 |
| Nov 24 11:15 | `harden-p22-r3d-material-map` | merged | 11,772 pass | tturbo-mac |
| Nov 24 11:15 | `harden-p22-cli-hook` | merged | 11,772 pass | tturbo-mac |
| Nov 24 11:15 | `harden-p22-engine-render-params` | merged | 11,772 pass | tturbo-mac2 |
| Nov 24 11:15 | `p22-gallery-tool` | merged | 11,772 pass | claude-sonnet-c |
| Nov 24 10:14 | `harden-p22-doc-look` | merged | 11,736 pass | tturbo-mac |
| Nov 24 10:14 | `harden-p22-r3d-unet-cpu` | merged | 11,736 pass | tturbo-mac2 |
| Nov 24 10:14 | `harden-p22-r3d-roughness-guide` | merged | 11,736 pass | tturbo-mac |
| Nov 24 10:14 | `harden-p16-vault-ceiling` | merged | 11,736 pass | tturbo-mac |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | agent | 1431 |
| bonsai-b | bonsai2-27b | agent | 1347 |
| bonsai-ws2-a | bonsai2-27b | down | 1488 |
| bonsai-ws2-b | bonsai2-27b | down | 1414 |
| claude-sonnet-a | sonnet | idle | 173 |
| claude-sonnet-b | sonnet | idle | 180 |
| claude-sonnet-c | sonnet | gate | 113 |
| claude-sonnet-d | sonnet | idle | 111 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | gate | 1300 |
| kimi-k31 | k3.1-pro-highspeed | held | 111 |
| mai-flash | mai-code-1.1-flash | idle | 131 |
| mai-flash-b | mai-code-1.1-flash | idle | 85 |
| mai-flash-esc | mai-code-1.1-flash | down | 178 |
| tturbo-mac | tturbo-ultraheretic | agent | 1223 |
| tturbo-mac2 | tturbo-ultraheretic | idle | 727 |
