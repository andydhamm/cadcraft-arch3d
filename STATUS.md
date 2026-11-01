# arch3d build status

Updated 2026-11-01 01:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 10 of 13: Lighting, light fixtures and final renders — 16 of 22 segments merged · 2 of 11 acceptance checks**

6,133 workspace tests passing at the last merge · 603 segments merged in all · 0 of 13 seats working

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
| 10 | Lighting, light fixtures and final renders | building | 2 / 11 | 9.2 h so far · expected done Nov 01 06:05–Nov 01 15:06 UTC |
| 11 | Ray-traced Finalize Render | planned | — | — |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 10 segments

16 merged, 3 merging, 2 gate, 1 queued

| Segment | What | State | Seat |
|---|---|---|---|
| `p10-cmd-illuminance` | ILLUMINANCE: direct light at a point | merging | mai-flash |
| `p10-cli-render-hook` | The CLI's session renderer behind RENDER 3D, SUNSTUDY and MCP | gate | claude-sonnet-a |
| `p10-cli-render-final` | cadcraft-cli render: --final, --night, --budget and the light report lines | gate | claude-sonnet-b |
| `p10-ui-pane-lights` | 3D pane: fixture lights, the site's sun or night, Finalize Render with progress, cancel and save | merging | claude-sonnet-d |
| `upstream-sync-5caaf73e3` | merge upstream upstream/main up to 5caaf73e3 (1 commits) | merging | bonsai-ws2-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 01:39 | `p10-arch3d-fixture-meshes` | merged | 6,133 pass | claude-sonnet-a |
| Nov 01 01:00 | `p10-r3d-finalize` | merged | 6,128 pass | claude-sonnet-b |
| Nov 01 00:37 | `p10-arch3d-scene-lights` | merged | 6,123 pass | claude-sonnet-d |
| Nov 01 00:26 | `p10-cmd-lightfixture` | merged | 6,118 pass | mai-flash |
| Nov 01 00:10 | `p10-r3d-lit-frame` | merged | 6,111 pass | claude-sonnet-b |
| Oct 31 23:59 | `p10-mcp-render-final` | merged | 6,106 pass | kimi-k31 |
| Oct 31 23:45 | `p10-r3d-lit-pipeline` | merged | 6,104 pass | kimi-k31 |
| Oct 31 23:31 | `harden-p10-cmd-sunstudy` | merged | 6,100 pass | tturbo-mac2 |
| Oct 31 23:22 | `harden-p10-arch-light-symbol` | merged | 6,095 pass | tturbo-mac |
| Oct 31 23:06 | `harden-p10-r3d-shadow-atlas` | merged | 6,090 pass | tturbo-mac |
| Oct 31 22:44 | `p10-io-light-proxies` | merged | 6,084 pass | bonsai-a |
| Oct 31 22:17 | `p10-arch-light-placement` | merged | 6,081 pass | kimi-k31 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | gate | 749 |
| bonsai-b | bonsai2-27b | idle | 691 |
| bonsai-ws2-a | bonsai2-27b | idle | 745 |
| bonsai-ws2-b | bonsai2-27b | gate | 712 |
| claude-sonnet-a | sonnet | gate | 60 |
| claude-sonnet-b | sonnet | gate | 58 |
| claude-sonnet-c | sonnet | idle | 10 |
| claude-sonnet-d | sonnet | merging | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 599 |
| kimi-k31 | k3.1-pro-highspeed | idle | 110 |
| mai-flash | mai-code-1.1-flash | merging | 33 |
| tturbo-mac | tturbo-ultraheretic | gate | 518 |
| tturbo-mac2 | tturbo-ultraheretic | merging | 337 |
