# arch3d build status

Updated 2026-11-01 03:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 10 of 13: Lighting, light fixtures and final renders — 19 of 22 segments merged · 4 of 11 acceptance checks**

6,163 workspace tests passing at the last merge · 609 segments merged in all · 0 of 13 seats working

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
| 10 | Lighting, light fixtures and final renders | building | 4 / 11 | 11.2 h so far · expected done Nov 01 06:40–Nov 01 14:59 UTC |
| 11 | Ray-traced Finalize Render | planned | — | — |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 10 segments

19 merged, 3 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p10-cli-render-hook` | The CLI's session renderer behind RENDER 3D, SUNSTUDY and MCP | merging | claude-sonnet-a |
| `p10-cli-render-final` | cadcraft-cli render: --final, --night, --budget and the light report lines | merging | claude-sonnet-b |
| `p10-docs-user-guide` | User guide: light fixtures, illuminance, night renders, Finalize Render, sun studies | merging | mai-flash |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Nov 01 03:43 | `p10-ui-pane-lights` | merged | 6,163 pass | claude-sonnet-d |
| Nov 01 03:32 | `harden-p10-r3d-lit-frame` | merged | 6,159 pass | tturbo-mac2 |
| Nov 01 03:13 | `harden-p10-io-glb-lights` | merged | 6,149 pass | tturbo-mac2 |
| Nov 01 02:57 | `harden-p10-io-light-proxies` | merged | 6,141 pass | tturbo-mac |
| Nov 01 02:16 | `upstream-sync-5caaf73e3` | merged | 6,136 pass | bonsai-ws2-a |
| Nov 01 02:00 | `p10-cmd-illuminance` | merged | 6,136 pass | mai-flash |
| Nov 01 01:39 | `p10-arch3d-fixture-meshes` | merged | 6,133 pass | claude-sonnet-a |
| Nov 01 01:00 | `p10-r3d-finalize` | merged | 6,128 pass | claude-sonnet-b |
| Nov 01 00:37 | `p10-arch3d-scene-lights` | merged | 6,123 pass | claude-sonnet-d |
| Nov 01 00:26 | `p10-cmd-lightfixture` | merged | 6,118 pass | mai-flash |
| Nov 01 00:10 | `p10-r3d-lit-frame` | merged | 6,111 pass | claude-sonnet-b |
| Oct 31 23:59 | `p10-mcp-render-final` | merged | 6,106 pass | kimi-k31 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 750 |
| bonsai-b | bonsai2-27b | merging | 697 |
| bonsai-ws2-a | bonsai2-27b | merging | 750 |
| bonsai-ws2-b | bonsai2-27b | idle | 716 |
| claude-sonnet-a | sonnet | merging | 61 |
| claude-sonnet-b | sonnet | merging | 60 |
| claude-sonnet-c | sonnet | idle | 10 |
| claude-sonnet-d | sonnet | idle | 4 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 599 |
| kimi-k31 | k3.1-pro-highspeed | idle | 110 |
| mai-flash | mai-code-1.1-flash | merging | 35 |
| tturbo-mac | tturbo-ultraheretic | merging | 520 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 337 |
