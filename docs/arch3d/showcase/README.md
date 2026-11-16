# Bilevel showcase house

A split-entry bi-level house built from a single JSON command script, `bilevel.json`, plus two scripts that add the drawing set on top of it.

## The house

- **Lower** — a basement level at elevation **-1.2 m** (below finished grade).
- **Entry** — a landing level at elevation **0.15 m** holding the front door (in the entry wall `w1`, offset 1.5) and the Entry room.
- **Main** — the above-grade level at elevation **1.5 m**, **390.3 m²** of above-grade room area.
- **Windows** — twelve, some with grilles; the round-topped window closes the north-facing façade with a softer head.
- **Finish** — the kitchen has a corner base run, a detached island counter with a seating overhang and waterfall edge, crown on the wall cabinets, a backsplash, a tall wall-oven cabinet, fillers and open shelves; both baths have frameless glass shower doors, a split pony wall and a niche; room moldings run on the finished rooms.

The site is a sloping lot that rises about **1.5 m** from the front (y = -5) to the back (y = 35), so the Main level stays above grade all round while the Lower level sits below it. A driveway, two sidewalks and two mulch planting beds are draped on the terrain, and ten plants (four trees, six shrubs) sit outside the house footprint.

## Rooms

The rooms are drawn inside the walls. `arch.inspect {"rooms": true, "levels": true}` on the replayed model gives each room's level, name, type and interior area:

| Level | Name | Type | Interior area |
|---|---|---|---:|
| Lower | Utility | utility | 124.03 m² |
| Lower | Family | family | 140.12 m² |
| Lower | Rec Room | other | 93.41 m² |
| Lower | Lower Bath | bath | 45.92 m² |
| Lower | Rear deck | deck | 18.00 m² |
| Entry | Entry | entry | 6.49 m² |
| Main | Hall | other | 42.65 m² |
| Main | Main Bath | bath | 18.91 m² |
| Main | Living | living | 85.17 m² |
| Main | Kitchen | kitchen | 105.02 m² |
| Main | Bedroom 1 | bedroom | 50.73 m² |
| Main | Bedroom 2 | bedroom | 24.73 m² |
| Main | Bedroom 3 | bedroom | 24.99 m² |
| Main | Dining | dining | 38.07 m² |
| Main | Foyer | openBelow | 8.27 m² |

Above-grade area: 390.3 m².

The split entry is the front landing between the basement and main levels. The Lower→Entry stair ends at the landing and the Entry→Main stair starts from the landing's edge; the landing is a level of its own. The Foyer is an Open Below room on the Main level cut out over the landing; its ceiling lifts the landing to full headroom instead of clamping the floor slab. Both stairs carry auto side railings.

The rear deck sits against the north exterior wall on the Lower level, with a closed perimeter ring, ledger and joists, a wood surface and a pair of outdoor dining pieces.

The Main level carries the casework (base and wall cabinets with bar pulls in the kitchen), the kitchen appliances (range, refrigerator, sink, dishwasher), the baths (toilet, lavatory, bathtub) and the furnishing. The bedrooms hold furniture, not cabinets.

## Ceilings and foundation

- **Living** — a vaulted ceiling (its flat ceiling is switched off).
- **Dining** — a tray ceiling, inset 0.8 and depth 0.3, with an LED strip around it.
- **Kitchen** — a soffit over the wall cabinets, with an LED strip under them.
- **Utility** (Lower) — a suspended ceiling.
- **Foundation** — a stem foundation under the exterior walls.

## Light, sky and cameras

The script saves seven cameras: Front, Back, Kitchen and Living for the plan-style views, and **dusk**, **noon-hdri** and **overview**, each with its own look:

- **dusk** — the `dusk` sun angle (azimuth 300°, elevation -3°), a sky backdrop and the light set Evening: the windows and the LED strips are lit.
- **noon-hdri** — the `noon` sun angle (21 June, solar noon), the `polyhaven:kloofendal-puresky` HDRI backdrop and the light set Day; its technique path-traces by itself.
- **overview** — a cutaway overview of the Main level under the noon sun.

[Phase 19 demo](./phase-19-demo.json) is the small demo of the same features on a tiny model. The script also creates four elevations (South, North, East, West) and two sections (Section A across the house, Section B across x = 13).

## The drawing set

`phase-20-showcase.json` adds the plan views (floor plan, electrical and reflected ceiling for every level, the roof plan and the site plan). `phase-21-showcase.json` adds the schedules, two interior elevations and **14 sheets**: A-000 Cover, A-001 Site Plan, the floor plans A-101, A-111 and A-121 (Entry Plans), the electrical sheets E-101 and E-111, the reflected ceilings A-102 and A-112, and A-201, A-301, A-302, A-401 and A-501. `plot` writes the whole set as one PDF (`showcase-set.pdf` in the replay below), one page per sheet.

## Render gallery

`tools/showcase/render_all.py` writes every image below. The committed images are refreshed by the operator with `python3 tools/showcase/render_all.py --full --out docs/arch3d/showcase/renders` (the tool sets `CADCRAFT_RENDER_ADAPTER=fallback` unless the environment already chooses a renderer; the operator selects the GPU through that variable) and may be older than the script; the manifest's `scriptSha256` says which script they show.

- [Contact sheet](./renders/contact-sheet.png)

Plans

- [Plan — Lower](./renders/plan-lower.png)
- [Plan — Main](./renders/plan-main.png)
- [Plan — Entry](./renders/plan-entry.png)

Plan views

- [Lower Floor Plan](./renders/planview-lower-floor.png)
- [Lower Reflected Ceiling](./renders/planview-lower-rcp.png)
- [Main Floor Plan](./renders/planview-main-floor.png)
- [Main Reflected Ceiling](./renders/planview-main-rcp.png)
- [Entry Floor Plan](./renders/planview-entry-floor.png)
- [Entry Reflected Ceiling](./renders/planview-entry-rcp.png)
- [Roof Plan](./renders/planview-roof.png)
- [Site](./renders/planview-site.png)

Electrical

- [Electrical — Lower](./renders/electrical-lower.png)
- [Electrical — Main](./renders/electrical-main.png)

Elevations

- [Elevation — South](./renders/elevation-south.png)
- [Elevation — North](./renders/elevation-north.png)
- [Elevation — East](./renders/elevation-east.png)
- [Elevation — West](./renders/elevation-west.png)

Sections

- [Section — A](./renders/section-a.png)
- [Section — B](./renders/section-b.png)

Interior elevations

- [Kitchen West Interior](./renders/interior-kitchen-west-interior.png)
- [Main Bath East Interior](./renders/interior-main-bath-east-interior.png)

Cameras

- [Camera — Front](./renders/camera-front.png)
- [Camera — Back](./renders/camera-back.png)
- [Camera — Kitchen](./renders/camera-kitchen.png)
- [Camera — Living](./renders/camera-living.png)

Look cameras

- [Camera — dusk](./renders/camera-dusk.png)
- [Camera — noon-hdri](./renders/camera-noon-hdri.png)
- [Camera — overview](./renders/camera-overview.png)

Night, path, clay, technical

- [Night render](./renders/night.png)
- [Path-traced final](./renders/path.png)
- [Clay render](./renders/clay.png)
- [Technical illustration](./renders/technical.png)

Sheets

- [A-000](./renders/sheet-a-000.png)
- [A-001](./renders/sheet-a-001.png)
- [A-101](./renders/sheet-a-101.png)
- [E-101](./renders/sheet-e-101.png)
- [A-102](./renders/sheet-a-102.png)
- [A-111](./renders/sheet-a-111.png)
- [E-111](./renders/sheet-e-111.png)
- [A-112](./renders/sheet-a-112.png)
- [A-121](./renders/sheet-a-121.png)
- [A-201](./renders/sheet-a-201.png)
- [A-301](./renders/sheet-a-301.png)
- [A-302](./renders/sheet-a-302.png)
- [A-401](./renders/sheet-a-401.png)
- [A-501](./renders/sheet-a-501.png)

Drawing set

- [The drawing set as one PDF](./renders/drawing-set.pdf)

## Replay

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json
```

Inspect the result:

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json --cmd 'arch.inspect {"rooms": true, "levels": true}'
```

`phase-21-showcase.json` adds the full drawing set on top of `bilevel.json` and `phase-20-showcase.json` (schedules, interior elevations and the sheet set); replay the three in order and `plot` the whole set as one PDF:

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json --commands docs/arch3d/showcase/phase-20-showcase.json --commands docs/arch3d/showcase/phase-21-showcase.json --cmd 'plot {"sheets": "all", "path": "showcase-set.pdf"}'
```
