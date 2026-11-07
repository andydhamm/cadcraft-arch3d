# CADCraft arch3d: Chief Architect X18 Behavioral Reference

This document refines the `arch3d` implementation plan based on behavioral observations of Chief Architect X18. It is a **clean-room reference**: it describes *what* the software does (behaviors, defaults, workflows) to inform our own implementation, not *how* it is coded.

**How to use it (2026-10-22).** Written by the lead model (Darwin R3) from the Chief Architect X18 Reference Manual and
Tutorial Guide, which the challenge lists as fair clean-room sources, in its own words. When planning a phase, fold that
phase's items from sections 1 and 3 into the plan where they fit the phase's scope. Section 4 lists deferrals, not bans:
where it disagrees with sections 1–3 (mulled units, snaps, curved stairs), sections 1–3 win. Units: imperial defaults
are given with metric equivalents; CADCraft's own unit handling decides what the user sees.

## 1. What changes in the plan

### Phase 1: Walls and Joins
*   **Add `Main Layer` concept to `WallType`:** The plan currently treats walls as uniform thickness. Chief uses a "Main Layer" (usually the structural core, e.g., studs) as the reference for joins, dimensioning, and roof baselines.
    *   *Why:* Joins must "build through" to the main layer, not just the centerline. This prevents gaps in complex assemblies and ensures correct bearing points for roofs/floors.
    *   *Action:* Add `is_main: bool` to `WallLayer`. Update join solver to intersect at the main layer surface, not just the centerline.
*   **Add `Pony Wall` support:** Walls can have different types above and below a split height.
    *   *Why:* Common for half-walls (e.g., stone base, siding top).
    *   *Action:* Add `split_height: Option<f64>` and `upper_type: WallTypeId` to `Wall`.
*   **Add `Reversed` flag:** Walls need a flag to flip the interior/exterior orientation without changing the type.
    *   *Why:* Users often draw walls in the wrong direction; flipping is easier than redrawing.
    *   *Action:* Add `reversed: bool` to `Wall`.

### Phase 2: Openings
*   **Distinguish `Rough Opening` vs. `Finished Opening`:** The plan mentions cutting openings. Chief distinguishes between the structural cut (rough) and the frame size (finished).
    *   *Why:* The 3D boolean cut must use the *rough* dimensions. The 2D symbol and 3D frame use the *finished* dimensions.
    *   *Action:* Add `rough_width`, `rough_height` to `Opening`.
*   **Add `Recess Depth`:** Openings can be offset from the wall face.
    *   *Why:* Windows are often recessed into the wall assembly (e.g., behind siding).
    *   *Action:* Add `recess_depth: f64` to `Opening`.
*   **Add `Mulled Units`:** Adjacent openings can be grouped.
    *   *Why:* Common for bay windows or door/window combos. They share a single casing and label.
    *   *Action:* Add `mull_group_id: Option<ArchId>` to `Opening`.

### Phase 3: Rooms, Floors, Levels
*   **Add `Room Type` enum:** Rooms are not just "enclosed spaces." They have types (Living, Garage, Deck, Attic, Open Below).
    *   *Why:* Type drives behavior: Garages get slabs, Decks get no ceiling, Open Below suppresses floors.
    *   *Action:* Add `RoomType` enum to `Room`.
*   **Add `Invisible Walls` (Room Dividers):** Walls with 0 thickness used only for room detection.
    *   *Why:* To split large rooms for different finishes or ceiling heights without adding physical walls.
    *   *Action:* Add `is_invisible: bool` to `Wall`. Exclude from 3D mesh and 2D poché, but include in room detection.
*   **Add `Open Below` flag:** Rooms can be open to the floor below.
    *   *Why:* Stairwells, vaulted ceilings.
    *   *Action:* Add `open_below: bool` to `Room`. Suppress floor platform generation for this room.

### Phase 4: Automatic Roofs
*   **Add `Roof Directives` to Walls:** Walls need flags for `Full Gable`, `Knee Wall`, `Hip`, etc.
    *   *Why:* The roof generator needs to know which walls are gable ends vs. hip ends.
    *   *Action:* Add `roof_directive: RoofDirective` enum to `Wall`.
*   **Add `Attic Walls`:** Auto-generated triangular walls between the roof underside and the floor above.
    *   *Why:* To close the space under the roof for insulation/finishes.
    *   *Action:* Implement `generate_attic_walls()` in the roof builder.
*   **Add `Roof Returns`:** Small planes extending past gable ends.
    *   *Why:* Aesthetic and functional (protects wall from rain).
    *   *Action:* Implement `generate_roof_returns()` for gable ends.

### Phase 5: Stairs, Railings, Decks
*   **Add `Walkline` concept:** Tread depth is measured along a walkline, not the edge.
    *   *Why:* Critical for curved/winder stairs.
    *   *Action:* Add `walkline_offset: f64` to `Stair`.
*   **Add `Landing` object:** Landings are distinct objects, not just part of the stair.
    *   *Why:* They can be moved independently and have their own railing logic.
    *   *Action:* Add `Landing` struct.
*   **Add `Railing` types:** Baluster, Solid, Open, Panels.
    *   *Why:* Different visual and structural behaviors.
    *   *Action:* Add `RailingType` enum.

### Phase 6: The 3D View
*   **Add `Display Styles`:** Shaded, Hidden-Line, X-Ray, Clay.
    *   *Why:* Essential for different workflows (drafting vs. presentation).
    *   *Action:* Implement `DisplayStyle` enum in `render3d`.
*   **Add `Hide Camera-Facing Walls`:** Toggle to see interiors from outside.
    *   *Why:* Critical for interior design views.
    *   *Action:* Implement view-dependent culling in `render3d`.

### Phase 7: Generated Elevations and Sections
*   **Add `Auto Dimensions`:** Generate dimension strings for walls and openings.
    *   *Why:* Chief's "Auto Dimensions" is a key productivity feature.
    *   *Action:* Implement `generate_auto_dimensions()` in `arch` crate.
*   **Add `Schedules`:** Tables of rooms, doors, windows.
    *   *Why:* Essential for construction documents.
    *   *Action:* Add `Schedule` object type and rendering logic.

### Phase 8: Recognition (ARCHDETECT)
*   **Add `Layer Mapping`:** Map external DXF layers to internal arch types.
    *   *Why:* `A-WALL` should map to `Wall`, `A-DOOR` to `Door`.
    *   *Action:* Implement `LayerMap` in `ARCHDETECT`.

### Phase 9: Exports
*   **Add `Metadata`:** Objects need custom fields (Code, Manufacturer, Price).
    *   *Why:* For IFC export and schedules.
    *   *Action:* Add `metadata: HashMap<String, String>` to `ArchObject`.

### Phase 10: Polish
*   **Add `Material Regions`:** Localized material overrides on walls/floors.
    *   *Why:* To apply brick to only the bottom half of a wall.
    *   *Action:* Add `MaterialRegion` object.
*   **Add `Moldings`:** Baseboards, crown molding.
    *   *Why:* Essential for interior finishes.
    *   *Action:* Add `Molding` object type.

## 2. UI and workflow

### Drawing Tools
*   **Continuous Chain:** Right-click drag-release to continue drawing walls. Double-click or `Esc` to stop.
*   **Angle Snaps:** Default 15° increments. Angle snaps take priority over object snaps for walls.
*   **Extension Snaps:** Snap to imaginary extension lines of other walls.
*   **Numeric Input:** Support arithmetic (`+`, `-`, `*`, `/`) and mixed units (`1'6"`, `18"`, `1.5'`).

### Editing & Grips
*   **Perpendicular Drag:** Resizing a wall via edge handles moves the entire wall perpendicular to its axis.
*   **Same Wall Type Grip:** Dragging a "Same Wall Type" handle creates a *new* wall of the same type, rather than extending the existing one.
*   **Bumping/Pushing:** Moving objects can "bump" into others, stopping movement or pushing the obstructing object.
*   **Reverse Layers:** Command to flip the interior/exterior orientation of a wall.

### Openings
*   **Auto-Resize:** If space is insufficient, doors/windows automatically shrink to fit the wall segment.
*   **Preview:** An outline follows the mouse; it disappears if placement is invalid.
*   **Elevation Reference:** Place relative to `Absolute`, `From Floor`, `From Finished Floor`, `From Ceiling`, or `From Roof`.
*   **Swing Indicators:** Direction can be configured to point to the Hinge or the Handle.

### Rooms & Levels
*   **Room Boxes:** Draw rectangular "Room Boxes" with a `Function` and `Name`. A "Build House" tool converts these into walls, floors, and ceilings.
*   **Reference Display:** View objects from other floors (e.g., Floor 1 walls while on Floor 2). Reference objects can be snapped to but not selected/edited.
*   **Dynamic Defaults:** Changing a global default (e.g., ceiling height) updates all objects set to "Use Default" but leaves manually overridden objects unchanged.

### 3D View
*   **Display Styles:** Shaded, Hidden-Line, X-Ray, Clay.
*   **Pivot Point:** Configurable to center mouse movement on the camera location or the focal point.
*   **Overlays:** Crosshairs, X/Y/Z axis indicators, sun/moon direction indicators.
*   **Picking:** Option to select the *Room* defined by a wall instead of the wall itself.

### Generated Views
*   **Auto Dimensions:** Generate up to three rows of dimensions (innermost, middle, outermost).
*   **Callouts:** Construction line callouts can be auto-numbered based on position/angle.
*   **Schedules:** Live tables linked to plan objects. Update automatically on object changes.

## 3. Behaviour reference by phase

### Phase 1: Walls and Joins
*   **Wall Types:**
    *   **Main Layer:** Structural core determining join depth, platform bearing, and roof baselines.
    *   **Partition Walls:** Do not build through floor/ceiling platforms.
    *   **Pony Walls:** Split upper/lower types.
    *   **Foundation Walls:** Auto-generate footings.
*   **Join Logic:**
    *   **Build-Through:** Walls build through neighbors until meeting the *Main Layer*.
    *   **Acute Angles:** Intersections < 60° terminate the building wall at a right angle to its own direction.
    *   **Fix Connections:** "Caution" icon for disconnected walls; command to auto-connect nearby ends.
*   **Snapping:**
    *   **Auto-Snap:** Wall ends snap to intersections if center lines are within the larger wall’s width, even if Object Snaps are off.
    *   **Resize About:** Configurable reference line (Outer Surface, Main Layer Outside, Center, etc.) determines anchor points for resizing/snapping.

### Phase 2: Openings
*   **Placement:**
    *   **Minimum Widths:** Hinged/Sliding/Pocket: 12" (300mm). Garage: 6' (1800mm).
    *   **Auto-Paneling:** Hinged/Pocket doors become Double if width ≥ 4' (1200mm). Bifold doors become Double if width > 3' (900mm).
    *   **Minimum Separation:** A global default controls the minimum distance between openings; this value also dictates the width of shared casing in "Mulled" units.
    *   **Wall Segment Limits:** Standard windows require a 6" (150mm) wall segment. Bay/Box/Bow windows require 30" (725mm).
*   **Swing & Hinge Logic:**
    *   **Interaction:** "Hold-and-drag" determines hinge side (L/R) and swing direction (In/Out) from the *exterior* perspective.
    *   **Classification:** Doors are automatically classified as Exterior if they separate an interior room from an exterior room (or building exterior). Interior if between two interior rooms.
*   **Geometry & Components:**
    *   **Recessing:** Openings can be recessed into specific wall layers (e.g., sheathing vs. framing).
    *   **Jamb/Casing:** Separate profiles for Interior/Exterior. Jamb can "Fit to Wall" (auto-depth). Casing has `width`, `depth`, and `reveal` (gap).
    *   **Hardware:** Defaults: Interior hinged = 2 hinges; Exterior hinged = 3 hinges.
    *   **Display States:** Doors/Windows can be shown Open or Closed independently in 2D and 3D. `swing_angle` (0-180°) and `percent_open` control geometry.
    *   **Glass:** `glass_mode` can be Automatic, Opaque, or Transparent.
    *   **Mulled Units:** Windows/doors automatically group if casings touch (same sill elevation). Manual mulling groups objects within 24" (600mm). Components remain separate for scheduling but share a single label/casing.
    *   **Window Levels:** A parameter (0-N) controls plan view stacking and selection priority. Level 0 is selectable; others require "Select Next Object".
    *   **Bay/Box/Bow Defaults:**
        *   **Bay:** 2'-2" front, 4'-2" back, 1'-0" depth, 45° angle.
        *   **Box:** 4'-2" width, 1'-6" depth, 90° angle.
        *   **Bow:** 5 sections, radius centered 3'-9" inside wall, 11.5" depth.
        *   **Area:** Bay/Box/Bow areas are included in "Interior Area" but excluded from "Living Area."

### Phase 3: Rooms, Floors, Levels
*   **Room Detection:**
    *   **Measurement Basis:** Area calculation can be based on the wall's "Main Layer" (structural centerline) or "Surface" (exterior finish).
    *   **Room Boxes:** Users can draw rectangular "Room Boxes" with a `Function` and `Name`. A "Build House" tool converts these into walls, floors, and ceilings.
*   **Levels & Reference:**
    *   **Reference Display:** Allows viewing objects from other floors (e.g., Floor 1 walls while on Floor 2). Reference objects can be snapped to but not selected/edited.
    *   **Grade Level:** A specific elevation marker independent of terrain, used for story pole dimensions.
    *   **Dynamic Defaults:** Changing a global default (e.g., ceiling height) updates all objects set to "Use Default" but leaves manually overridden objects unchanged.
*   **Room Types & Defaults:**
    *   **Garage:** Concrete slab, no finish, stem walls/curbs.
    *   **Deck:** Framed platform, no ceiling/roof.
    *   **Porch:** Slab floor, ceiling, roof.
    *   **Attic:** Requires adjacent walls flagged as "Knee Walls".
    *   **Open Below:** Suppresses floor platform generation (used for stairwells).
*   **Area Metrics:**
    *   **Interior Area:** Measured from inner wall surfaces. Includes bay/box/bow areas.
    *   **Standard Area:** Measured from center of interior walls and outside surface (or Main Layer) of exterior walls. Excludes bay/box/bow areas.
    *   **Living Area:** Sum of Standard Areas for Interior room types. Excludes Exterior/Hybrid types (Garage, Deck, Porch), Open Below, and Attic. Rooms with Rough Ceiling height < 48" (1200mm) are excluded by default.

### Phase 4: Automatic Roofs
*   **Generation:**
    *   **Directives:** Roof generation relies on "roof directives" carried from lower floors to the top floor.
    *   **Auto-Rebuild:** Roofs automatically rebuild when wall geometry changes. Manual editing of roof planes disables auto-rebuild.
    *   **Pitch Display:** Supports "Rise/Run" (e.g., 6:12) and "Degrees" (e.g., 26.5°).
*   **Roof Shapes:** `Hip`, `Full Gable`, `Dutch Gable` (partial gable on hip), `High Shed/Gable`, `Knee Wall` (interior only), `Extend Slope Downward`.
*   **Parameters:** `Pitch`, `Upper Pitch` (for Dutch gable), `Height Upper Pitch Starts`, `Overhang Length`.
*   **Roof Returns:** Auto-generated on gable ends. Parameters: `Length`, `Extend` past overhang, `Gable/Hip/Full` type, `Sloping/Flat`, include `Shadow Boards`, `Ridge Caps`, `Frieze`, `Gutters`.
*   **Wall/Roof Interaction:** `Roof Cuts Wall at Bottom` prevents wall building below intersecting roof plane. `Include Frieze` generates molding at roof line.
*   **Bay/Box/Bow Roof Logic:** Default is Hip roof over bump-out. Options: `Use Existing Roof`, `Extend Existing Roof Over`, `Rectangular Roof Over`. Gable is not automatic.

### Phase 5: Stairs, Railings, Decks
*   **Stair Calculation:** "Best Fit" riser height closest to **6 3/4" (168.75 mm)** that reaches the next level precisely. Rise measured to floor *finish* surfaces; stringers locate to floor *structure*.
*   **Tread Modes:** `Automatic Treads`, `Lock Tread Depth` (count changes with length), or `Lock Number of Treads` (depth changes with length).
*   **Walkline:** Tread depth calculated along a "Walkline" offset from edge (default 12"/300 mm). Critical for curved/winder stairs.
*   **Winders:** Expand to fill gaps between stair side and nearby walls. Requires valid room definition.
*   **Wrapped Stairs:** Two perpendicular sections with identical attributes can "wrap" around a corner if close enough. Edits propagate.
*   **Stringers:** Options: `2 Closed Side`, `1 Center`, or `Custom`. Parameters: `Height Above/Below Tread`.
*   **Open Risers:** Checkbox to remove riser faces or add a skirt underneath.
*   **Landings:**
    *   *Auto Height:* Top Height = 1 riser + floor finish thickness (interior) or thickness (exterior).
    *   *Thickness:* Defaults to 6.75" (169mm) if free-standing, or matches attached stair riser.
    *   *Railing Logic:* Per-edge control: `Automatic` (suppress if connecting to stair/ramp/landing), `No Railing`, `Has Railing`.
    *   *Plan Display:* Option to display landing on floor above (`Automatic` if in "Open Below" room).
*   **Railings:**
    *   *Automatic Openings:* Creates doorway opening in railing when stairs snap to it.
    *   *Types:* `Balusters`, `Open`, `Open with Middle Rail`, `Panels`.
    *   *Heights:* Separate `Stair Rail Height` and `Landing Rail Height`.
    *   *Transitions:* Options for gooseneck and smooth transitions.
    *   *Extensions:* `Extend Top/Bottom End` values define handrail extension past first/last riser.
*   **Stairwells:** Auto button creates `Open Below` room on floor above, enclosed by railings.
*   **Ramps:** Default slope 1:12, max height 30" (760mm). Auto-length calculation.

### Phase 6: The 3D View
*   **Display Styles:**
    *   **Floor Overview:** A 3D mode that shows the current floor and floors below but **omits ceilings** on the current floor to reveal interior layout.
    *   **Vector View:** Hidden-line removal. Optional toggle to suppress curved edge lines for performance.
    *   **Glass House:** X-ray view.
*   **Navigation & UI:**
    *   **Pivot Point:** Configurable to center mouse movement on the camera location or the focal point.
    *   **Overlays:** Supports crosshairs, X/Y/Z axis indicators (floating/fixed), and sun/moon direction indicators.
    *   **Status Bar:** Displays X, Y, Z coordinates, layer, and drawing group.
    *   **Picking:** Option to select the *Room* defined by a wall instead of the wall itself.
    *   **Gamepad/3D Mouse:** Configurable sensitivity, acceleration curves, and axis inversion.

### Phase 7: Generated Elevations & Sections
*   **Construction Lines:** Guide lines drawn in plan view appear in sections/elevations if the line of sight is parallel. Vertical lines in sections appear in plan.
*   **Callouts:** Construction line callouts can be auto-numbered based on position/angle. Collinear lines share numbers.
*   **Auto Exterior Dimensions:** Generates up to three rows:
    1.  **Innermost:** Locates exterior/interior walls and openings.
    2.  **Middle:** Locates exterior/interior walls only.
    3.  **Outermost:** Overall exterior dimension.
    *   **Parameters:** `Line Separation`, `First Line Offset`, `Offset From Wall` (Center/Dimension Layer/Surface), `Reach`, `Minimum Area`.
    *   **State:** If manually edited, marked as "edited" and not overwritten by auto-generation.
*   **Auto Story Pole Dimensions:** Vertical dimension strings and Elevation Markers (e.g., Top of Plate, Ridge).
    *   **Parameters:** `Reach` (percentage across structure), `Line Separation`, `First Line Offset`, `Marks to Include`.
*   **Dimension Formatting:**
    *   **Rounding:** `Grid Rounding` (sum of parts equals whole) vs `Distance Rounding` (accurate segments, sum may vary).
    *   **Units:** Imperial (ft-in, fractional) or Metric. Fraction reduction: GCD or Closest.
    *   **Extension Lines:** `Gap From Marked Object`, `Fixed Proximity` (locks distance).
*   **Annotations:**
    *   **Callouts:** Linked to saved views/layout pages. Text macros insert view names/page numbers.
    *   **Markers:** Level Line, Point, Test Boring. Distinct geometry (center point, line, text).
    *   **Text Styles:** Saved sets of attributes (font, height, color). 3D text can "Face Camera" (billboard) or align with surface.

### Phase 8: Recognition (ARCHDETECT)
*   **Layer Handling:** Importing/detecting objects must dynamically create missing layers (e.g., `A-WALL`) in the target document.
*   **Clipboard:** Copying arch objects between files creates necessary custom layers/styles in the destination if missing.

### Phase 9: Exports
*   *(No specific behavioral notes from manual pages relevant to export formats beyond standard object data.)*

### Phase 10: Polish & Validation
*   **Plan Check:** Validates plans against building practices. Automatically assigns Room Types based on size/contents (e.g., small room + shelf = closet). Highlights errors with navigation.
*   **Materials List:** Supports custom formulas for cost estimation.
*   **UI Preferences:**
    *   **Contextual Toolbar:** Appears only when objects are selected, with buttons changing based on object type.
    *   **Command History:** `Shift+Esc` cycles through the last 100 used commands.
    *   **Line Weights:** Minimum display weight in pixels ensures visibility when zoomed out.

## 4. Not worth copying

*   **Framing Generation:** Full structural framing (studs, joists, trusses) is out of scope for Phases 0–10. Data models (`bearing`, `is_framing`, `header_type`) should be prepared, but generation logic is deferred.
*   **Terrain Mesh Generation:** From elevation points/contours. Terrain modifiers, hills, valleys, roads, driveways, sidewalks.
*   **Thermal Envelope:** Conditioned space detection and orientation logic for energy analysis.
*   **Ray Tracing:** Progressive path tracing for "final views".
*   **Electrical/Plumbing/HVAC:** MEP wiring/splines are **not** part of `arch3d` MVP.
*   **Layout Sheets:** Beyond the generated elevation sheets. Title blocks, revision tables, multi-page printing.
*   **External Links:** Linking layout views to external plan files (XREF style).
*   **Spatial Filtering for Exports:** `MATERIALSCOPE` object or command.
*   **Component Hierarchy:** Detailed component structure for material takeoffs.
*   **Path-Based Walkthrough Animation:** Not in Phase 6 scope.
*   **Artificial Light Sources:** Point, spot, area lights.
*   **Curved/Wrapped Stairs:** Deferred.
*   **Interactive Roof Plane Editing:** Joining/locating intersections.
*   **Dormers and Crickets:** Not in Phase 4.
*   **Detailed Eave/Fascia/Soffit Generation:** Deferred.
*   **Complex Window Types:** Bay/Bow and Mulled Units not in Phase 2 scope.
*   **Layer Mapping UI:** Not detailed.
*   **User-Defined Default Saving:** "Saved Defaults Dialog".
*   **Specific Snap Types:** Object Snaps, Extension Snaps, Wall Intersections.

## 5. Asset and interchange candidates (not from the manuals)

CADCraft arch3d is a personal, non-commercial project (Andy, 2026-10-22); the fork is public, so what is committed is
redistributed and must allow that.


* **Phase 10, fixtures library:** NVIDIA's SimReady assets (`huggingface.co/datasets/nvidia/simready-assets`, OpenUSD,
  2,575 assets) include appliances (dishwasher, refrigerator, toaster, range hood), 2 m and 4 m window blinds, file
  cabinets and beds. Licences are per asset (`<asset>/.metadata/LICENSE.txt`; the ones checked are CC BY 4.0): read
  each one before use, record source and licence in `ATTRIBUTION.md`, and convert the chosen few to glTF outside the
  codebase (CADCraft stays pure Rust, no USD reader). Most of the set is industrial and medical and does not fit.
* **Phase 9, optional OpenUSD export:** NVIDIA SimReady Foundation (`github.com/NVIDIA/simready-foundation`,
  Apache-2.0) specifies semantic labels, physics properties and packaging for simulation-ready USD. A USD export that
  follows it would let CADCraft houses load into Omniverse or Isaac. Only after glTF, OBJ and IFC4 are done.
* **Phase 10, home furniture and fixtures (best fit):** Sweet Home 3D's model libraries
  (`sweethome3d.com/import-models`, ZIP libraries of OBJ/DAE models). Use public-domain sets first (BlendSwap CC-0, 175
  models; Dodecagon), then CC BY sets with attribution (Scopia 500, KatorLegaz 90, BlendSwap CC-BY 135). Free Art
  License sets (Contributions 511, LucaPresidente 64) may go in too, as separate asset files that keep their own licence
  (share-alike covers those models and changes to them, not CADCraft). Check each model's licence, record it in
  `ATTRIBUTION.md`, convert to glTF.
  Sweet Home 3D itself is GPL: behaviour may inform the furniture catalogue UI, its code may not be used.
* **Local only, never in the repo:** sunvage SimHome (`huggingface.co/datasets/sunvage-lab/sunvage-vertens3d-simhome`,
  2,000 whole homes and 12,169 rooms in OpenUSD + glb) is "non-commercial research only", gated, no redistribution
  without written permission. This is a personal, non-commercial project, so it may be used locally: Andy was granted
  access on 2026-11-02. Use it as evaluation data (room detection and area rules against real homes) or as a local
  furniture source read from a folder outside the repository (phase 14's local catalog). The fork (`andydhamm/cadcraft`)
  is public, so none of it is ever committed, and no check depends on it.
* **Not an asset source:** D5 Render (free Community tier, Windows + RTX) and its D5 Works library (terms not
  published on its download page). At most a check that the phase 9 glTF export renders correctly in it.

## 6. The library: how content is reached and applied (from Chief's training videos, 2026-11-02)

Andy supplied 29 Chief Architect training videos as the description of the library. This section is a clean-room summary
of the **behaviour** they show, in our own words, cited as (video mm:ss); the transcripts stay outside the repository.
Chief's catalog content belongs to its vendor and manufacturers: arch3d ships its own (section 5 lists the sources).

### 6.1 The browser
- One side panel beside the plan, opened from the View menu or a toolbar button (librarybrowser 00:09–00:32). Panels, top
  to bottom: a filter bar, the folder tree with the filter results, a preview, details, an optional tags panel and a
  toolbar; panels can be closed, rearranged, locked and reset (librarybrowser 03:09–08:23).
- **Catalogs:** stock catalogs (generic core content installed at first run, plus optional extra and manufacturer
  catalogs) are read-only and can be updated; **Copy to User Catalog** makes an editable copy, which no longer receives
  updates. The **user catalog** holds what you make or import; items deleted from it go to a **trash** you can restore
  from or empty (librarybrowser 00:37–03:09). Online catalogs show in results with a badge and download whole, not item by
  item (library 01:00–04:05).
- **Search:** typing offers a plain text search (names, keywords, folder names) and attribute filters (manufacturer,
  style, symbol type, symbol placement, hardware, material maps, object type, part number). Each chosen filter becomes a
  chip; chips combine with AND, a chip's X removes it, and a leftover chip can leave zero results (library 01:13–03:22;
  librarybrowser 03:20–03:48). Back/forward through filter history; saved advanced filters (librarybrowser 03:48–04:37);
  tags with a built-in Favorites tag, usable as a filter (librarybrowser 06:19–06:50); a grid/list toggle; search into
  subfolders or not (librarybrowser 06:57–07:48); "show in folder" from a result (stairsandrailings 04:54).
- **Preview and details:** the preview rotates and zooms, switches rendering style and, for a material, the sample shape;
  details show name, folder, tags and, for user items, notes (librarybrowser 05:04–06:00).
- **Opened from dialogs:** a specification dialog's library button opens the browser already narrowed to what fits there:
  door styles, door hardware, appliances in a cabinet face, newels and balusters, rail and molding profiles, cladding,
  roof and ceiling finish layers, backdrops (residentialbootcamp 32:48; doorsandwindows 06:06; appliancesandfixtures
  07:16; stairsandrailings 02:12; kitchenandbathbootcamp 25:08; 3dcladding 02:07; suspendedceilings 01:45; backdrops
  01:22).

### 6.2 Applying an item
- **Click, move, click:** select an item in the browser, move into a plan or 3D view without holding the button, click to
  place; there is no drag and drop (library 01:41–01:54). The cursor tells what will happen: an outline to place, a
  swap cursor over a part it can replace, a spray can or roller for materials (kitchenandbathbootcamp 19:14–23:41).
- **Hosts:** what you click on decides the result.
  - Empty floor: the item is placed on its own, at the floor (library 01:35).
  - Over a matching part: the part is swapped, e.g. a cabinet door style or a pull (kitchenandbathbootcamp 19:23–20:39).
  - A sink over a base cabinet snaps into it: the counter gets its cut-out and the top drawer becomes a false front
    (appliancesandfixtures 02:53–03:09; residentialbootcamp 73:53–74:19); the sink stays a sub-object you can select and
    nudge (appliancesandfixtures 03:17–04:05).
  - An appliance dropped in the gap between two base cabinets gets the counter carried over it (appliancesandfixtures
    02:07–02:24); a microwave drawer or wall oven dropped on a cabinet becomes that cabinet's face item
    (appliancesandfixtures 05:51–09:06).
  - A faucet snaps toward the wall behind its sink and centres on it (appliancesandfixtures 04:24–05:02).
  - Ceiling lights hang from the room's ceiling and drop to a soffit placed under them (ceilingbasics 02:36–03:09).
- **After placing:** open its specification (size, elevation, materials), set it as the default for the next one, centre
  it on another object, move it point to point, copy and reflect, multiple-copy at intervals (appliancesandfixtures 04:50;
  kitchenandbathbootcamp 34:39–36:21, 80:21; residentialbootcamp 76:09). Tab cycles through stacked objects
  (appliancesandfixtures 03:23).
- **Material painter:** pick a material, click a surface. **Scope** decides how far it goes: one component; the whole
  object; every surface in the room that has the clicked material (other objects included); the floor; the plan
  (kitchenandbathbootcamp 22:12–23:14; materials 06:24–06:44). **Paint** replaces the material; **blend** tints it and
  keeps the texture (materials 06:55–07:19; library 05:34–05:50). An eyedropper picks a material up from a surface
  (kitchenandbathbootcamp 27:45).
- **Object painter (replace from library):** pick a replacement, click a target; scope component, object, room, floor or
  plan swaps every like item in that scope (hardware, doors, stools, balusters, stair panels); a toggle switches between
  replacing and placing (librarypainter 00:00–02:30).
- **Into the library:** selected objects become a user-catalog item with **Add to Library** (insertionpoints 02:31–02:49;
  kitchenandbathbootcamp 44:56–46:06); a block's insertion point can be set before adding it (insertionpoints 03:35–04:45);
  a material can be copied into the user catalog and edited there (kitchenandbathbootcamp 77:32–78:29).
- **Defaults:** Set as Default on a configured object makes the next one of its kind match; defaults belong to the plan
  (doorsandwindows 04:27; residentialbootcamp 41:25–42:52; kitchenandbathbootcamp 24:14).

### 6.3 Materials and finishes
- A material has a 2D pattern (for plan and elevation hatching) and a 3D texture with scale, offset, angle and a blend
  colour, plus a class (matte, plastic, polished, metal, glass, water…) and optional bump, normal and emissive maps
  (materials 00:15–05:58). Editing a material changes it wherever it is used, so copy first (decks 56:17).
- Tile and stone materials are generated from parameters (shape, layout, colours, grout) and saved to the user catalog
  (materials 08:23–10:42; tilematerials 06:55–08:22).
- Room types carry default floor finishes (residentialbootcamp 23:38); finish layers of ceilings and roofs take library
  materials (suspendedceilings 01:11; 3dcladding 05:59).

### 6.4 Electrical
- Outlets, switches and can lights come from **electrical tools**, not the library; the library adds more fixtures
  (kitchenandbathbootcamp 79:49–82:52). Work happens in an Electrical plan view (kitchenandbathbootcamp 79:25).
- **Connections:** drag from a switch to the lights it controls; each wire is an arc you can reshape
  (kitchenandbathbootcamp 81:36–82:14).
- **Auto-place outlets:** in a kitchen, GFCI outlets go along the counters above the base cabinets and under the wall
  cabinets (kitchenandbathbootcamp 82:14–82:35).
- Lights on one circuit dim together; light sets choose which lights a view uses (smallspacelightingandvisualizing
  22:27–23:30; interiorraytracing 08:08–10:10).

### 6.5 Views of library objects
- Every object has a plan symbol and a 3D shape; its category decides its layer and which pickers list it
  (stairsandrailings 06:44; kitchenandbathbootcamp 42:19–43:09). Saved plan views (e.g. Electrical, Kitchen & Bath) show
  and hide categories and labels (savedplanviews 00:10–05:38; kitchenandbathbootcamp 79:02–79:39).
- 3D rendering techniques: standard, vector (the line style elevations use), glass house, technical illustration,
  watercolor, hand-drawn lines, duotone, clay (a plain grey model) and physically based ray trace
  (kitchenandbathbootcamp 84:50–86:39). Watercolor and glass house are not required (Andy 2026-11-02); technical
  illustration is optional.

## 7. Beyond the library: what the training videos show (2026-11-07)

The same 29 videos (§6), read again on 2026-11-07 for everything other than the library and compared with the brief.
Behaviour only, in our own words, cited as (video mm:ss). Each subsection feeds one brief phase; where the brief puts
something out of scope, the brief wins.

### 7.1 Ceilings, foundations, stairs, decks and openings (brief phases 16 and 17)
- **Rough and finished ceilings.** A room has a rough ceiling (structure) and a finished one, apart by the ceiling
  finish (ceilingbasics 00:46–01:03; residentialbootcamp 14:11–15:43). A suspended ceiling is the room's ceiling raised,
  then a finish stack of drywall, an air gap and ceiling tile, so the finished surface drops (suspendedceilings
  00:51–02:15).
- **Cathedral and vaulted ceilings.** Turning a room's flat ceiling off makes its ceiling follow the roof underside, and
  re-pitching the roof re-pitches the vault (ceilingbasics 03:42–05:21). Interior walls in a vaulted room rise to the
  sloped ceiling (ceilingvaultstraysandbarrels 07:08–07:21). Lights on a vault hang plumb, and a pendant's mount follows
  the slope (smallspacelightingandvisualizing 51:28–55:02).
- **Trays.** An automatic tray takes a width in from the walls, a depth, and vertical or pitched sides. It can recess up
  into the attic instead of dropping the surrounding ceiling (ceilingvaultstraysandbarrels 08:02–08:55;
  kitchenandbathbootcamp 72:47–74:51). Trays nest, and a drawn outline can become one (saguarotrayceiling 00:50–03:02;
  ceilingvaultstraysandbarrels 10:09–11:43). The outline stays editable after placing (saguarotrayceiling 05:14).
- **Soffits.** A soffit is a box under the ceiling: above wall cabinets, as coffers, or as faux beams (interiorsoffits
  00:07–00:21). It snaps to a wall face or a room corner and has a direction (interiorsoffits 00:32–01:27). Its height is
  measured from the floor finish or the subfloor, and it follows later ceiling changes (interiorsoffits 03:05–03:59).
  Lights placed under a soffit drop to its underside, and moving a soffit over lights re-seats them (ceilingbasics
  02:38–03:09; slopedsoffits 01:04–01:26).
- **Foundations.** A foundation is derived from the first floor's exterior walls:
  - stem walls on footings, a grade beam on piers, or a monolithic slab, with a slab thickness;
  - stem height taken from a basement's ceiling;
  - footings drawn dashed in plan.
  (residentialbootcamp 44:53–49:03.)
- **Decks.** A deck room frames itself: joists, a ledger, a beam set back for a cantilever, posts and footings
  (decks 02:05–04:11, 14:25–16:18). Board direction is automatic and can be set per room (decks 09:45–10:21). Border
  boards and a centre transition board are options (decks 66:40–67:29).
- **Railings.**
  - Railings have newel posts at ends, corners and turns, spaced automatically up to a maximum, with top, middle and
    bottom rails (stairsandrailings 02:41–03:57, 11:40–12:39).
  - A railing can be set in from the edge, and a deck's posts drop onto the rim (stairsandrailings 03:47, 38:54–42:06;
    decks 05:02–05:43).
  - Stair railings are switched on or off per side and flight (stairsandrailings 01:49, 11:09–11:33).
- **Stairs.**
  - Tread thickness and nosing, open or closed risers, and stringers: two, centre, none, or a skirt against a wall
    (stairsandrailings 01:32–01:45).
  - A stair drawn over a wall cuts the wall off beneath it (stairsandrailings 14:14–14:23).
  - In plan, a stair shows its treads with nosings, a walk arrow and a break line (stairsandrailings 09:19–09:43).
- **Openings.**
  - Divided lites set by a count across and down, for the whole window or its top part only (residentialbootcamp
    34:50, 41:02–41:15; doorsandwindows 04:08, 07:39).
  - Round-top windows, reflected to make a circle (doorsandwindows 10:08–10:25), and windows whose top follows the roof
    pitch in a gable (doorsandwindows 09:16–09:27).
  - A fixed door panel serves as a sidelight (residentialbootcamp 34:26). Sill and casing switch off per side
    (residentialbootcamp 40:54; doorsandwindows 03:38–04:02).

### 7.2 Interior finish, kitchen and bath (brief phase 18)
- **Room-type finishes.** A room's type sets its floor build-up and finish, for example a garage slab or a tiled bath
  (residentialbootcamp 23:38–24:18, 30:41).
- **Moldings.**
  - Room moldings: base, crown and chair rail, set per room. Crown wraps soffits, and a soffit can ignore room moldings
    (kitchenandbathbootcamp 09:00; ceilingbasics 01:52–02:31; interiorsoffits 04:02–05:55).
  - Cabinet crown comes from a profile, with a size override and an offset for face items, and follows the box front
    when the door is removed (kitchenandbathbootcamp 25:01–27:40, 40:02).
  - A molding polyline sweeps a profile along a drawn path (ceilingvaultstraysandbarrels 13:26–16:53;
    kitchenandbathbootcamp 40:36–42:08). Tray moldings have horizontal and vertical offsets (saguarotrayceiling 03:05).
- **Countertops.** A counter detaches from its cabinets and becomes a custom shape. An edge moves by a typed amount for a
  seating overhang, turns into an arc with a radius handle, or drops to the floor as a waterfall
  (kitchenandbathbootcamp 65:25–69:17).
- **Backsplash.** Click a wall and the backsplash runs from the counter to the underside of the wall cabinets; drag its
  top to the ceiling, or draw a custom shape (kitchenandbathbootcamp 47:22–48:35). It is on or off per cabinet
  (kitchenandbathbootcamp 16:56).
- **Cabinet fronts.**
  - Boxes are framed or frameless, with overlay or inset fronts and a toe kick (kitchenandbathbootcamp 17:04–17:18).
  - A face layout editor changes an item's kind (door, drawer, opening, appliance), splits items across or down,
    equalizes them and sets their heights (kitchenandbathbootcamp 31:11–33:49; residentialbootcamp 69:45–71:13).
  - Fronts wider than about 27 in become door pairs (kitchenandbathbootcamp 31:38; residentialbootcamp 69:20–69:42).
- **Corners, fillers and shelves.**
  - A cabinet pushed into a corner becomes a corner cabinet: diagonal door or lazy susan, with the cut-out shown in plan
    (kitchenandbathbootcamp 29:17–30:05; residentialbootcamp 71:18–72:19).
  - Fillers close a run against a wall (kitchenandbathbootcamp 16:18, 57:30, 64:26).
  - Open shelving is a wall cabinet with no doors and an automatic or set shelf count (kitchenandbathbootcamp
    38:27–40:00).
- **Face appliances.** A microwave drawer or wall oven dropped on a cabinet becomes its face item, at the appliance's
  height. Two can stack in one tall cabinet (appliancesandfixtures 05:51–09:06).
- **Bath.** Glass walls make shower enclosures. A glass pony wall is a solid base about 42 in high with glass above, and
  a shower door is glass with hardware (kitchenandbathbootcamp 92:42–94:01). A wall niche is an opening type: a recess
  with a width, style and back material (kitchenandbathbootcamp 48:37–49:42; doorsandwindows 00:29).

### 7.3 Light, sky and cameras (brief phase 19)
- **Exposure and tone mapping.** Physically based views have automatic or manual exposure, a choice of tone curve,
  separate bounce counts for opaque and transmissive surfaces, and samples (interiorraytracing 04:07–05:24).
- **Sun.**
  - The sun has an intensity (about 100,000 suggested for daylight) and a cast-shadows switch (exteriorraytracing
    04:25–04:48, 13:59; backdrops 08:38).
  - Named sun angles (several per plan) are picked per camera (interiorraytracing 01:00, 10:45–11:53).
  - The backdrop alone can light the scene with the sun off (backdrops 06:45–07:24).
- **Sky.** A generated sky's colour follows the sun, through twilight to a night sky with stars and a moon that has its
  own brightness and position (backdrops 07:34–10:14).
- **HDRI.** An HDR panorama lights the scene as well as filling the background (backdrops 05:00–06:31). A 360° backdrop
  takes a horizon (eye-level) offset (backdrops 03:21–04:01), and backdrop intensity is set for day or night
  (interiorraytracing 05:44–06:03; exteriorraytracing 10:01–11:28).
- **Horizon.** Terrain can extend to the horizon so exteriors have no edge (exteriorraytracing 09:18–09:56).
- **Strip lights.** Rope or LED strip lights run along a path at a spacing: in trays, under cabinets and in stair risers
  (smallspacelightingandvisualizing 39:47–42:40; saguarotrayceiling 03:40–04:23; stairsandrailings 49:04–51:14;
  kitchenandbathbootcamp 82:41).
- **Material maps.** Bump and normal maps render and can be imported (materials 02:48–03:49).
- **Seeing in.**
  - Walls facing the camera hide while orbiting, together with what they host (smallspacelightingandvisualizing
    02:18–03:13, 37:50–39:07).
  - A 3D cross-section slider gives six live clip planes, and each camera has a near-clip distance
    (smallspacelightingandvisualizing 07:04–10:25).
- **Cameras.**
  - Each camera stores its rendering technique (interiorraytracing 02:51) and can be reset to its saved state
    (interiorraytracing 13:31–15:58; smallspacelightingandvisualizing 47:03).
  - Floor overview can show or hide lower floors (smallspacelightingandvisualizing 01:43–02:15).

### 7.4 Construction drawings (brief phases 20 and 21)
- **Plan views.**
  - Saved plan views are named layer sets bound to a floor or to any floor. Each remembers zoom and opens as a tab
    (savedplanviews 00:10–03:49).
  - Views carry their own annotation defaults (savedplanviews 04:14–07:48) and are created, duplicated and edited
    (developingsavedplanviews 01:10–10:48).
  - Templates ship about a dozen views, including a reflected ceiling plan (savedplanviews 01:52–02:02, 12:13–12:37).
- **Reference display.** A plan can show another floor beneath in its own colours, saved with the view (savedplanviews
  05:06–10:15; residentialbootcamp 48:31).
- **Dimensions.**
  - Dimensions locate walls by their framing and snap to wall layers, overhangs and ridges (residentialbootcamp
    16:13–28:43).
  - Selecting an object and typing a new value into a dimension moves that object (residentialbootcamp 16:19–17:42,
    22:29–23:11; kitchenandbathbootcamp 05:29–06:22).
  - Automatic exterior dimensions appear and update as walls and openings are added (residentialbootcamp 16:02, 20:03,
    33:46).
- **Labels and schedules.**
  - Openings carry labels and opening indicators in elevations (doorsandwindows 10:31–13:28).
  - Schedules choose categories and columns, number objects with callouts, find objects in the plan and update live
    (kitchenandbathbootcamp 69:26–72:43; newfeatures 09:02–09:56).
  - Room labels choose areas and dimensions per room (newfeatures 10:00–11:17).
- **Interior elevations.** An elevation taken inside a room shows only that wall and what is on it, clipped to the room
  or cropped with handles. It shows cabinet door-swing marks and takes annotations that stay with the saved view
  (kitchenandbathbootcamp 50:37–55:20; residentialbootcamp 89:44–92:41).
- **Material patterns.** Each material's 2D pattern appears in vector views and elevations (materials 00:15–01:11).
- **Layouts.**
  - Plan views go to a linked layout as live views at a scale, cropped. Camera views and schedules go too.
  - A title block on a master page appears on every page, with page macros.
  - All pages export to one PDF at sheet size (residentialbootcamp 96:11–110:46; kitchenandbathbootcamp 87:06–91:51).
