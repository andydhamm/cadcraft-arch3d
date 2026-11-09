# Bilevel showcase house

A split-entry bi-level house built from a single JSON command script.

## The house

- **Lower** — a basement level at elevation **-1.2 m** (below finished grade).
- **Entry** — a landing level at elevation **0.15 m** holding the front door (wall `w0`, offset 4.5, y = 0) and a small utility room.
- **Main** — the above-grade level at elevation **1.5 m**, with **at least 371.6 m²** of above-grade room area.

The site is a sloping lot that rises about **1.5 m** from the front (y = -5) to the back (y = 35), so the Main level stays above grade all round while the Lower level sits below it. A driveway, two sidewalks and two mulch planting beds are draped on the terrain, and ten plants (four trees, six shrubs) sit outside the house footprint.

## Rooms

| Level  | Rooms |
|--------|-------|
| Lower  | Family, Lower Bath, Kitchen, Living, Bedroom 1 |
| Entry  | Entry, Utility |
| Main   | Kitchen, Living, Bedroom 1, Main Bath |

The Main level also carries the casework (base and wall cabinets with bar pulls), the kitchen appliances (range, refrigerator, sink, dishwasher), the baths (toilet, lavatory, bathtub) and the furnishing.

## Views and cameras

The script saves four cameras (Front, Back, Kitchen, Living), creates four elevations (South, North, East, West) and two sections (Section A across the house, Section B across x = 13).

## Replay

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json
```

Inspect the result:

```sh
cadcraft-cli run --metric --commands docs/arch3d/showcase/bilevel.json --cmd 'arch.inspect {"levels":true,"site":true}'
```

`tools/showcase/render_all.py` renders every view style, the exports and a contact sheet with a manifest.
