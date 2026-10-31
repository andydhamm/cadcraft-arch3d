# arch3d build status

Updated 2026-10-31 23:45 UTC. This page and the card in the README are regenerated every 15 minutes from the swarm's own dashboard.

**Phase 10 of 13: Lighting, light fixtures and final renders — 9 of 22 segments merged · 0 of 11 acceptance checks**

6,100 workspace tests passing at the last merge · 596 segments merged in all · 0 of 13 seats working

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
| 10 | Lighting, light fixtures and final renders | building | 0 / 11 | 7.2 h so far · expected done Nov 01 11:09–Nov 02 03:13 UTC |
| 11 | Ray-traced Finalize Render | planned | — | — |
| 12 | Exports and interoperability | planned | — | — |
| 13 | Polish | planned | — | — |

## Phase 10 segments

9 merged, 7 queued, 6 merging

| Segment | What | State | Seat |
|---|---|---|---|
| `p10-arch3d-scene-lights` | Scene lights and direct illuminance with shadow rays | merging | claude-sonnet-d |
| `p10-arch3d-fixture-meshes` | Fixture housings in 3D | merging | claude-sonnet-a |
| `p10-r3d-lit-pipeline` | The lit shader: fixture lights with spot cones and shadows, emissive lenses, tone mapping | merging | kimi-k31 |
| `p10-cmd-lightfixture` | LIGHTFIXTURE: place, grid in a room, edit, delete, list types; arch.inspect lights | merging | mai-flash |
| `p10-mcp-render-final` | MCP render tool: 3D stills and Finalize Render through the RENDER 3D command | merging | kimi-k31 |
| `upstream-sync-5caaf73e3` | merge upstream upstream/main up to 5caaf73e3 (1 commits) | merging | bonsai-ws2-a |

## Recent merges

| When (UTC) | Segment | Result | Workspace tests | Seat |
|---|---|---|---|---|
| Oct 31 23:31 | `harden-p10-cmd-sunstudy` | merged | 6,100 pass | tturbo-mac2 |
| Oct 31 23:22 | `harden-p10-arch-light-symbol` | merged | 6,095 pass | tturbo-mac |
| Oct 31 23:06 | `harden-p10-r3d-shadow-atlas` | merged | 6,090 pass | tturbo-mac |
| Oct 31 22:44 | `p10-io-light-proxies` | merged | 6,084 pass | bonsai-a |
| Oct 31 22:17 | `p10-arch-light-placement` | merged | 6,081 pass | kimi-k31 |
| Oct 31 21:50 | `harden-p9-arch-site` | merged | 6,071 pass | tturbo-mac2 |
| Oct 31 21:36 | `p10-arch-light-symbol` | merged | 6,066 pass | bonsai-ws2-a |
| Oct 31 21:23 | `p10-cmd-sunstudy` | merged | 6,062 pass | bonsai-b |
| Oct 31 21:14 | `harden-p9-cmd-wall-retaining` | merged | 6,058 pass | tturbo-mac |
| Oct 31 21:03 | `p10-r3d-shadow-atlas` | merged | 6,047 pass | claude-sonnet-c |
| Oct 31 20:55 | `p10-cmd-render` | merged | 6,041 pass | bonsai-a |
| Oct 31 20:33 | `p10-arch-light-placement` | refused | 6,045 pass, 3 fail | darwin-ws3 |

## Seats active in the last hour

| Seat | Model | State | Rounds |
|---|---|---|---|
| bonsai-a | bonsai2-27b | idle | 742 |
| bonsai-b | bonsai2-27b | idle | 688 |
| bonsai-ws2-a | bonsai2-27b | merging | 745 |
| bonsai-ws2-b | bonsai2-27b | idle | 710 |
| claude-sonnet-a | sonnet | idle | 59 |
| claude-sonnet-b | sonnet | idle | 55 |
| claude-sonnet-c | sonnet | idle | 10 |
| claude-sonnet-d | sonnet | idle | 3 |
| darwin-ws3 | darwin-r3-exl3-3.05bpw-sc | idle | 598 |
| kimi-k31 | k3.1-pro-highspeed | merging | 110 |
| mai-flash | mai-code-1.1-flash | idle | 31 |
| tturbo-mac | tturbo-ultraheretic | gate | 516 |
| tturbo-mac2 | tturbo-ultraheretic | agent | 335 |
