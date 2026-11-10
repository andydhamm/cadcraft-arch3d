# Bilevel showcase house

A split-entry bi-level house built from a single JSON command script.

## The house

- **Lower** — a basement level at elevation **-1.2 m** (below finished grade).
- **Entry** — a landing level at elevation **0.15 m** holding the front door (wall `w0`, offset 4.5, y = 0) and a small utility room.
- **Main** — the above-grade level at elevation **1.5 m**, with **at least 371.6 m²** of above-grade room area.

The site is a sloping lot that rises about **1.5 m** from the front (y = -5) to the back (y = 35), so the Main level stays above grade all round while the Lower level sits below it. A driveway, two sidewalks and two mulch planting beds are draped on the terrain, and ten plants (four trees, six shrubs) sit outside the house footprint.

## Rooms

The replayed model's room set is the split-entry bi-level. The `arch.inspect {"rooms": true}` view records the level, room name, type and interior area for each room polygon, including the landing's Open Below foyer and the split stairs that connect the lower, entry and main levels.

| Level | Name | Type | Interior area |
|---|---|---:|---:|
| Lower | Family | family | 34.52 m² |
| Lower | Lower Bath | bath | 6.21 m² |
| Lower | Kitchen | kitchen | 14.88 m² |
| Lower | Living | living | 22.92 m² |
| Lower | Bedroom 1 | bedroom | 13.70 m² |
| Entry | Entry | entry | 7.33 m² |
| Entry | Utility | utility | 4.15 m² |
| Main | Kitchen | kitchen | 18.36 m² |
| Main | Living | living | 34.74 m² |
| Main | Bedroom 1 | bedroom | 12.95 m² |
| Main | Main Bath | bath | 7.61 m² |
| Main | Dining | dining | 18.61 m² |
| Main | Bedroom 2 | bedroom | 10.92 m² |
| Main | Bedroom 3 | bedroom | 8.83 m² |

The split entry is the front landing between the basement and main levels. The Lower→Entry stair ends at the landing and the Entry→Main stair starts from the landing's edge; the landing is a level of its own, so the front door sits on a small utility landing. The Open Below foyer is a room on the Main level cut out over the landing; its ceiling lifts the landing to full headroom instead of clamping the floor slab. The `renders` contact sheet and each image below show the plan, electrical views, elevations, sections and rendered views for the house.

The Main level also carries the casework (base and wall cabinets with bar pulls), the kitchen appliances (range, refrigerator, sink, dishwasher), the baths (toilet, lavatory, bathtub) and the furnishing.

## Views and cameras

The script saves four cameras (Front, Back, Kitchen, Living), creates four elevations (South, North, East, West) and two sections (Section A across the house, Section B across x = 13).

## Render gallery

- [Contact sheet](./renders/contact-sheet.png)
- [Plan — Lower](./renders/plan-lower.png)
- [Plan — Entry](./renders/plan-entry.png)
- [Plan — Main](./renders/plan-main.png)
- [Electrical — Lower](./renders/electrical-lower.png)
- [Electrical — Main](./renders/electrical-main.png)
- [Elevation — South](./renders/elevation-south.png)
- [Elevation — North](./renders/elevation-north.png)
- [Elevation — East](./renders/elevation-east.png)
- [Elevation — West](./renders/elevation-west.png)
- [Section — A](./renders/section-a.png)
- [Section — B](./renders/section-b.png)
- [Camera — Front](./renders/camera-front.png)
- [Camera — Back](./renders/camera-back.png)
- [Camera — Kitchen](./renders/camera-kitchen.png)
- [Camera — Living](./renders/camera-living.png)
- [Night render](./renders/night.png)
- [Path-traced final](./renders/path.png)
- [Clay render](./renders/clay.png)
- [Technical illustration](./renders/technical.png)

## Replay

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json
```

Inspect the result:

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json --cmd 'arch.inspect {"levels":true,"site":true}'
```

`tools/showcase/render_all.py` renders every view style, the exports and a contact sheet with a manifest. Use the fallback renderer explicitly for headless runs and write into `docs/arch3d/showcase/renders`:

```sh
CADCRAFT_RENDER_ADAPTER=fallback python3 tools/showcase/render_all.py --out docs/arch3d/showcase/renders
```
