16. **Ceilings and foundations.** (Andy 2026-11-07: what the training videos show beyond the library, checked against
    this brief; the behaviour is CHIEF-REFERENCE §7.1.)
    - **Observables first:** `build3d {"ceiling": [[x,y], …], "level": n}` returns, for each point, the finished ceiling
      elevation above it on that level (`null` outside every room), and `build3d` reports a count of parts by kind
      (`riser`, `tread`, `post`, `joist`, `beam`, `footing`, `grille`, `soffit`, …). Phase 17 uses both.
    - **Ceiling build-up:** a room's ceiling height is its **rough** ceiling (the underside of the structure above). Its
      ceiling gets a finish stack (layers, each a material and a thickness; an air-gap layer has no material), and the
      **finished** ceiling is the rough one minus the stack. The default stack is empty, so phase 3's numbers hold (and
      today's 20 mm ceiling solid stays as it is under an empty stack: phase-3 check 05 probes it). The template offers
      "Drywall" (12.7 mm) and "Suspended" (drywall, an air gap, 16 mm ceiling tile). This is the "ceiling finish layer"
      phase 14's `library {"for": …}` already points at.
    - **Ceilings that follow the roof:** a room's `flatCeiling` (default on). Off, on a room under a roof, its ceiling is
      the underside of the roof planes over it: a cathedral or vaulted ceiling, re-pitched when the roof is. Every wall
      bounding a vaulted room, exterior or interior, rises to meet that ceiling along its thickness (today they stop at
      their height and leave a gap); an interior wall between a vaulted and a flat room rises to the roof underside, closing
      the attic side. Ceiling items hang from the vaulted ceiling at their point, plumb. A room or ceiling edit that would
      put a flat ceiling above the roof underside is refused with an error naming the room; roof edits are never refused,
      and where one leaves a flat ceiling above the underside, that ceiling follows the underside there and
      `arch.inspect` reports a warning for the room.
    - **Trays:** `tray {"room" | "polygon", "inset", "depth", "sides"?: "vertical" | pitch}` recesses the ceiling up
      into the attic: inside an outline `inset` in from the room's walls (or a drawn polygon), the ceiling is `depth`
      higher. Trays nest (a tray inside a tray), stay editable (outline, inset, depth), and are refused, with nothing
      built, if any part would rise above the roof underside, or if the room has a level above it instead of an attic.
    - **Soffits:** `SOFFIT` is a box hung from the ceiling, from a plan rectangle or polygon, given a bottom elevation or
      a depth below the finished ceiling, with its own material; drawn in plan as a dashed outline on `A-CLNG`. Placing
      against a wall or into a room corner snaps it to the wall faces; `soffit {"over": [cabinets]}` fills from the tops
      of wall cabinets to the ceiling. Phase 14's rule now applies: ceiling items placed over a soffit hang from its
      underside, and moving a soffit over placed lights re-seats them. (Interior soffits are not roof soffits: the probe
      and materials keep them apart.)
    - **Foundations:** `FOUNDATION {"type": "stem" | "slab", "footingDepth"?: 1.2}` builds under every exterior wall with
      no level below it: the lowest level's, and any on grade higher up (an entry landing, a garage, a bump-out):
      - **stem:** a 0.2 m concrete wall whose outer face is flush with the wall above, from the underside of the level's
        floor structure down to a footing 0.6 m wide and 0.25 m deep, centred under it, its bottom `footingDepth` below
        the level's floor elevation;
      - **slab:** the level's floor structure becomes a 0.1 m concrete slab, with a thickened edge 0.3 m wide (inward from
        the outer face) reaching 0.3 m below the slab's top.
      A basement's exterior walls (drawn as usual; `FOUNDATION` creates no walls) take the "Concrete 0.2" wall type and
      get the same footing directly under the basement's floor structure. Footings draw dashed in the plan of the level
      they support and show filled in sections. With phase 9 terrain, footing depth is still measured from the floor
      elevation; the pad already leaves room for it.
    - **Rebuild and exports:** each new object or edit rebuilds only what it touches, and `scene_cache_equals_full`
      gains ceiling stacks, vaults, trays, soffits and foundations. glTF nodes per object; IFC: `IfcFooting`, foundation
      walls as `IfcWall`, soffits as `IfcCovering` (CEILING); DXF proxies on their layers.
    - **Numbers for the checks** (phase-1 house, one level, walls 2.4 m high, level ceiling 2.4 m, a 6/12 gable roof with
      gables on the x = 0 and x = 10 walls as in phase 4, so the roof underside is z = 2.4 + 0.5·(y + 0.1) for y ≤ 4):
      - **vault:** with `flatCeiling` off in the south room, `ceiling` at `(5,2)` is 3.45 and at `(5,0.5)` 2.70 (± 1e-6);
        the north-west room stays 2.4 at `(3,5)`. Probes find the interior wall at `(5,3.0,3.9)` (it stopped at 2.4
        before) and the south wall at `(5,0.05,2.45)` (it rises to meet the ceiling, 2.475 there). A library ceiling item
        placed at `(5,2)` has its top at 3.45;
      - **tray:** in the north-west room (interior 0.1…5.95 × 3.05…7.9), `tray {"inset":0.6,"depth":0.3}` gives `ceiling`
        2.7 at the room centre `(3.025,5.475)` and 2.4 at `(0.4,5.0)`; a nested tray inset 0.6 from it with depth 0.15
        gives 2.85 at the centre. A first tray with depth 0.5 is refused and builds nothing: its top (2.9) would cross the
        roof underside at y = 7.3 (2.8);
      - **suspended ceiling:** in the north-east room (rough ceiling 2.4) with the stack drywall 0.0127 + air 0.2 + tile
        0.016 (top to bottom), `ceiling` at `(8,5)` is 2.1713; probes at `(8,5,2.18)` hit `ceiling` (the tile) and at
        `(8,5,2.3)` nothing (the air gap);
      - **soffit:** in the north-east room with no stack, a soffit over x 7.0…9.0, y 7.3…7.9 with its bottom at 2.1:
        probes hit it at `(8.0,7.6,2.2)` and nothing at `(8.0,7.2,2.2)`; a ceiling item placed at `(8.0,7.6)` has its top
        at 2.1;
      - **stem foundation** (level 1 at 0, structure 0.3): under the south wall, probes at x = 5 hit the foundation wall
        at `(5,0.0,−0.5)`, the footing at `(5,0.25,−1.0)`, and nothing at `(5,0.25,−0.5)` or `(5,0.35,−1.0)`. With a
        basement level at −2.7 (structure 0.3) and its walls drawn on the same centre lines, its footing is hit at
        `(5,0.25,−3.1)`. **Slab:** the slab is hit at `(5,1.0,−0.05)`, the thickened edge at `(5,0.15,−0.2)`, and
        nothing at `(5,0.25,−0.2)`.
    - **Demo:** the showcase house with a vaulted living room, a tray ceiling in the dining room, soffits over the kitchen
      wall cabinets, a suspended ceiling in the utility room, footings under every exterior wall on grade and its
      concrete basement; its views re-rendered onto the contact sheet.
    - **Out of scope:** manual ceiling planes with their own pitch, barrel and shelf ceilings, wall, floor and roof
      framing and trusses, garage curbs, piers and grade beams.
17. **Stairs, decks, railings and openings in detail.** (Andy 2026-11-07, as phase 16; CHIEF-REFERENCE §7.1.)
    - **Carry-over first: one stair convention.** `start` is the centre of the bottom nosing line, as the `STAIR`
      command, the stair well and phase-5 check 04 have it. The 3D flight and the plan linework still extend from `start`
      to one side, so every stair today sits half its width off its well. Move both to the centre convention, and rewrite
      phase-5 check 03 to probe the new part kinds (`tread`, `riser`, `stringer`) at points inside them; its numbers
      otherwise hold. The showcase script rebuilds its stairs.
    - **Decks you can stand under:** a `DECK` gets its structure, rebuilt with it:
      - a ledger on the house wall it touches (on the longer contact when it touches walls on two sides);
      - joists 38 × 184 mm running away from the ledger, in equal bays at most 0.406 m apart, plus rim joists on its
        free edges;
      - a drop beam 76 × 235 mm under the joists, set back 0.3 m from the outer edge (the edge opposite the ledger);
      - posts 0.14 m square under the beam, from grade to its bottom, with end posts 0.3 m in from the deck's sides and
        bays at most 2.4 m, each on a 0.3 m round pier footing;
      - boards 25 mm thick on top, whose direction (default parallel to the ledger) can be set per deck. The direction
        turns the board texture; it never re-frames the joists.
      A deck touching no wall has no ledger: its joists span its shorter side, with a beam set back 0.3 m from each end
      of the joists. Grade is the terrain under each post when there is one, else z = 0.
    - **Railings built as built:** posts (90 mm square) at every end, corner and turn and at most `post_spacing` apart
      (default 1.8 m, in equal bays; the field exists, unused, with 1.2 today), a top rail and a bottom rail 0.05 m above
      the walking surface, balusters or a panel between, and an optional horizontal inset from the path. New stairs and
      decks get their railings automatically, as `Railing` objects that follow their host (the existing `follows`): a
      deck's on its free edges with the posts on the rim joists; a stair's on each side that is not against a wall, with
      a newel at each end of each flight. Per side: automatic (default), on or off. Drawings saved before this phase keep
      the railings they had.
    - **Stairs in detail:** treads 25 mm thick with a 25 mm nosing; risers open or closed (open was out of scope in
      phase 5); stringers two closed (default), one centre, or none with a skirt board on a wall side. A wall that runs
      under a flight stops at the flight's underside, 0.3 m (settable) under the line through the tread nosings, measured
      vertically, built as phase 4 clips gables: a wall profile polygon, no 3D CSG.
    - **Stair plan symbol** (replacing today's outline and riser lines): on the lower level, nosing lines solid up to the
      plan cut height (default 1.2 m above the floor), a diagonal break line, dashed beyond, and a walk arrow labelled
      `UP`; on the upper level, the part seen through the stair well, solid, labelled `DN`. On the stair's layer, in the
      plan, in exports and in the DXF proxy linework.
    - **Openings in detail:**
      - grilles (divided lites): a count across and down, for the whole sash or its top part only, as 20 × 20 mm bars in the
        glass plane, evenly dividing the glass;
      - window tops: square (default), round (a half circle across the width) or segment (an arc of a given rise). The
        rough opening is cut to that shape;
      - a window can take `top: "roof"`: in a gable wall, its top follows the roof underside, less an offset (default
        0.15 m);
      - a fixed door style (a glazed leaf that does not open, used as a sidelight);
      - the sill and the casing can be switched off on either side.
    - **Rebuild and exports:** incremental as before; `scene_cache_equals_full` gains deck structure, railings, stair
      parts and grilles. IFC: deck joists and beams as `IfcMember`, posts as `IfcColumn`, railings as `IfcRailing`.
    - **Numbers for the checks** (phase-1 house, walls 2.4 m high; a level 2 at 2.7 for the stair checks, as in phase 5;
      the roof-top window uses phase 16's vaulted south room under the gable roof):
      - **deck** `(2,8.1) (6,8.1) (6,11.1) (2,11.1)` against the north wall, 0.9 m above grade: 9 joists at x = 2.4,
        2.8 … 5.6 plus rim joists; 3 posts at x = 2.3, 4.0 and 5.7 on the beam line y = 10.8. Probes: a post at
        `(4.0,10.8,0.2)`, nothing at `(3.15,10.8,0.2)`, the beam at `(3.15,10.8,0.6)`, a joist at `(2.8,9.5,0.8)`,
        nothing at `(3.0,9.5,0.8)`, boards at `(3.0,9.5,0.89)`;
      - **railing** (in an empty drawing) path `(0,0) (0,3) (4,3) (4,0)` with the default spacing: 8 posts, at the four
        path points and at `(0,1.5)`, `(1.3333,3)`, `(2.6667,3)` and `(4,1.5)` (± 1e-3); a probe at `(1.3333,3,0.5)` hits a
        post;
      - **stair plan:** the phase-5 stair (16 risers of 0.16875 m) drawn on level 1 has 7 solid nosing lines (treads
        whose top is under the 1.2 m cut: 7 × 0.16875 = 1.18125), one break line and 8 dashed nosing lines, counted in the
        level-1 plan's DXF export on the stair's layer; the level-2 plan has a `DN` label and no dashed nosing lines;
      - **wall under a flight:** a stair starting at `(1,1)` heading +x, 0.9 m wide (so y 0.55…1.45), and a 0.1 m wall
        `(3,0)–(3,3)` under it. At x = 3.0 (2.0 m from the start, under tread 8, whose top is 1.35 and bottom 1.325) the
        line through the nosings is 0.16875 + 2.025 × 0.16875 / 0.254 = 1.5141, so the wall stops at 1.2141: it is hit at
        `(3.0,1.0,1.15)` and nothing is at `(3.0,1.0,1.27)`; outside the flight it keeps its full height (hit at
        `(3.0,2.5,2.0)`). Open risers: `build3d` reports 0 `riser` parts, closed 16;
      - **grilles:** the phase-4 window (0.9 × 1.2, sill 0.9, centred at x = 4 on the north wall) with 2 × 2 lites:
        probes on its glass plane (`arch.inspect` reports the plane and the glass centre) hit `grille` at the glass centre
        and 0.15 m beside it, and `glass` 0.15 m beside and 0.15 m above it;
      - **round top:** a 0.9 × 1.5 window, sill 0.6, `top: "round"` (radius 0.45, spring line at 1.65) at x = 4 on the
        north wall: a probe inside the wall's thickness at `(4.0,8.0,2.0)` finds no wall and at `(3.6,8.0,2.05)` finds it;
      - **roof top:** a 1.0 m window centred at y = 1.5 in the x = 0 gable wall, sill 1.2, `top: "roof"`: its top is
        3.05 at y = 1.5 and 2.825 at y = 1.05, so `(0,1.5,3.0)` finds no wall and `(0,1.05,2.9)` finds it.
    - **Demo:** the showcase house with its stairs on their wells and drawn with full symbols in every plan, the rear
      deck framed and railed, and gridded windows with one round-top; its views re-rendered onto the contact sheet.
    - **Out of scope:** winders, curved, spiral and wrapped stairs (still deferred), flights of different widths, starter
      and flared treads, horizontal and cable rails, deck border and transition boards, dormers, shed roofs.
18. **Interior finish, kitchen and bath.** (Andy 2026-11-07, as phase 16; CHIEF-REFERENCE §7.2.)
    - **Room-type finishes:** each room type has default floor, wall and ceiling materials in the template (Garage:
      concrete floor; Bath and Laundry: tile floor; Kitchen: tile or wood; bedrooms: carpet); a room takes them when it is
      detected or its type changes, and a painter edit overrides them. Phase 14's "no surface keeps the default material"
      then holds by construction.
    - **Moldings:** profiles are library items (a new **Moldings** folder in Cabinet parts) generated in Rust: base
      (default 90 × 15 mm), shoe, crown (default 100 × 100 mm), chair rail, casing, plus any closed 2D polyline drawn in
      the plan and added with `library.add`.
      - **Room moldings:** each room has a list of moldings (profile, kind `base` | `crown` | `chair`, height or offset).
        Base runs along the wall faces at the floor and stops at door openings' casings; crown runs round the finished
        ceiling and wraps soffits and trays (a soffit can opt out); chair rail sits at its height and stops at openings.
      - **Cabinet moldings:** crown and light rail on wall cabinets, following the box front (or the face where there's no
        door), with a size override.
      - **Molding polylines:** a profile swept along a drawn 2D path at an elevation (a tray's edge, a wainscot cap), with
        mitred corners.
    - **Countertops you can shape:** `countertop.detach {"run"}` turns an automatic counter into a custom one that keeps
      its outline; its edges then move (a typed distance: a seating overhang), bulge into arcs (a sagitta, with a radius
      grip), and take an edge profile; an end can turn down to the floor (a **waterfall**, same material and thickness).
      `countertop {"polygon", "elevation", "thickness"}` draws one from scratch. Sink cut-outs (phase 14) carry over.
    - **Backsplash:** `backsplash {"run" | "wall"}` covers the wall face behind a counter, from the counter top to the
      underside of the wall cabinets above (else to a set height, default 0.45 m), thickness 10 mm, its own material; its
      top can be dragged to the ceiling. Each base cabinet has a backsplash on or off.
    - **Cabinet construction and fronts:**
      - boxes framed or frameless; fronts overlay or inset;
      - **face layout:** each front item has a kind (door, drawer, false front, opening, appliance) and a height, and can be
        split across or down into equal items; `cabinet.face {"id", …}` edits it; door pairs are automatic on fronts
        wider than 0.686 m (27 in), replacing today's pair above 0.6 m, with single or double forced on demand;
      - **corners:** a base or wall cabinet pushed into an inside corner where two runs meet becomes a corner cabinet
        (blind, diagonal door or lazy susan, its plan symbol showing the cut-out), and the counter runs round the corner
        in one piece;
      - **fillers:** `cabinet.fill {"run"}` closes the gap between a run's end and a wall with a filler up to 0.15 m wide;
        larger gaps are reported, never stretched;
      - **open shelves:** a wall cabinet with no doors and a shelf count (automatic or given), evenly spaced;
      - **face appliances:** a microwave drawer, wall oven or warming drawer placed on a cabinet face replaces that front
        item at the appliance's height; a tall cabinet can stack two.
    - **Bath:**
      - **glass walls:** a wall type "Glass" (10 mm clear glass, no poché fill, its own material, default height 2.0 m);
      - **pony walls:** any wall can be split at a height into a lower and an upper type (a solid base with glass above
        is the glass pony wall);
      - **shower doors:** an opening style "Shower" (a frameless glass door that swings both ways) for glass walls;
      - **wall niches:** an opening kind `niche`, a recess `depth` deep into the wall from one face, with a width, height
        and sill, and its own back material; refused when the wall is not at least 12 mm thicker than the recess.
    - **Rebuild and exports:** incremental as before; IFC `IfcCovering` (MOLDING, SKIRTINGBOARD) for moldings, glass
      walls as `IfcWall` with a glass material, countertops and backsplashes as part of their cabinets' `IfcFurniture`.
    - **Numbers for the checks:**
      - **moldings:** a room with walls on centre lines `(0,0) (4.1,0) (4.1,3.1) (0,3.1)`, 0.1 m thick (interior 4.0 ×
        3.0), with one 0.9 m door: base length 14.0 minus the door's trim width on the room's face (its frame plus casings, as
        `arch.inspect` reports it), crown length 14.0, chair rail as base (± 1e-6, measured along the wall faces); a probe 0.01 m off a wall face at z 0.05 hits `molding`;
      - **countertops:** detaching an automatic counter keeps its area (± 1e-9); moving one edge of length L outward by
        0.3 adds 0.3 · L; setting that edge to an arc with sagitta s adds the circular segment's area (chord L,
        R = (L²/4 + s²) / (2s), area R²·acos((R − s)/R) − (R − s)·√(2Rs − s²)) ± 1e-6. A waterfall end is hit by a probe
        0.3 m above the floor inside its thickness;
      - **backsplash:** over a counter run with wall cabinets above part of it, the backsplash's top equals the cabinets'
        bottom over that part and the counter top + 0.45 elsewhere (± 1e-6);
      - **fronts:** a front split into 3 equal drawers gives three heights equal to 1e-9 that, with the reveals, sum to
        the face height; a 0.762 m wall cabinet gets two doors and a 0.610 m one gets one; a wall oven of height h placed on
        a tall cabinet's upper door makes a face item of height h ± 1e-6;
      - **corner:** two base runs meeting at an inside corner give one corner cabinet and one counter polygon;
      - **glass walls:** a "Glass" wall is hit as `glass` at mid-height and nothing above 2.0; split at 1.07 into a
        solid lower part and glass above, a probe at 0.5 hits `wall`, at 1.5 `glass`, at 2.2 nothing;
      - **niche:** a niche 0.6 wide, 0.9 high, sill 1.0, depth 0.1 from the inside face of the phase-1 north wall at
        x = 4: nothing at `(4.0,7.95,1.45)`, the wall at `(4.0,8.05,1.45)`.
    - **Demo:** the showcase kitchen with an island whose counter has a seating overhang and a waterfall end, a full
      backsplash, a corner cabinet, fillers against the walls, crown on the wall cabinets and open shelves; both full
      baths with glass showers, a glass pony wall and a niche; base and crown moldings in every finished room; Finalized.
    - **Out of scope:** material builders and the interactive texture editor (still), drawer accessories (pull-outs) and
      showing fronts open, apron sinks, 3D cladding relief, corner boards, wall caps, valley flashing, cabinet schedules
      (phase 21).
19. **Light, sky and cameras.** (Andy 2026-11-07, as phase 16; CHIEF-REFERENCE §7.3.)
    - **Exposure:** automatic (from the frame's log-average luminance toward middle grey) or manual as EV100, where a
      pixel's linear value is the scene luminance × 1 / (1.2 · 2^EV100); tone mapping ACES (default), Hable or none.
      Applied the same in the pane, the raster Finalize and the path tracer, replacing today's fixed exposure constant.
    - **The sun:** intensity in lux on a surface facing it (default 100,000), cast shadows on or off, and the sun off with
      the backdrop lighting the scene alone. **Named sun angles** (a date and time, or an azimuth and elevation) stored in
      the drawing, several per plan, chosen per camera.
    - **A sky that follows the sun:** an analytic clear-sky model (Preetham's or an equivalent closed form; no data tables
      we cannot generate) gives the sky's colour and brightness from the sun's position, in the pane, raster Finalize and
      path tracer, lighting the scene as well as filling the background. From sun elevation 0° to −6° it fades through
      twilight to night; at night a moon (a dim directional light, default 0.25 lx, with its own position) and optional
      stars.
    - **HDRI:** Radiance `.hdr` (RGBE, run-length encoded) read in Rust, used as a 360° backdrop that **lights** the
      scene: importance-sampled in the path tracer, and in the raster views as nine spherical-harmonic coefficients for
      diffuse light. Intensity and rotation per backdrop; a horizon offset for any 360° backdrop. A CC-0 HDRI joins the
      sample set as groundwork, before this phase is planned.
    - **Ground to the horizon:** outside the site skirt, a flat ground plane at the site boundary's mean edge elevation,
      in the site's ground material, reaching the far plane, so no exterior view shows the edge of the world.
    - **Strip lights:** `LightFixture` kind `strip` along a 2D path at a height (or under a cabinet run, or round a
      tray), with lumens per metre and a colour temperature, emitting downward (Lambertian) or outward. In the raster
      view it counts as one light against the budget (sampled as points 0.15 m apart); in the path tracer it is an
      emissive strip.
    - **Normal maps:** a material can carry a tangent-space normal map; phase 13's procedural brick, siding, shingle and
      wood textures gain matching ones. Used by the raster views and the path tracer, and exported as glTF
      `normalTexture`.
    - **Seeing in:** a **cutaway** toggle hides the walls whose outer face points at the camera on the viewed level,
      together with everything hosted in or on them (openings, cabinets, devices, wall items); a per-camera **clip box**
      (six planes, dragged in the pane) and **near clip**, honoured by the raster views, the path tracer and `pick3d`.
    - **Cameras keep their look:** each saved camera stores its technique (standard, path-traced, clay, technical), sun
      angle, backdrop, exposure, light set, cutaway and clip box; `camera.reset` returns a camera to its saved state.
      Floor overview (the current level and the ones below, no ceilings) is a camera kind, with lower levels optional.
    - **Numbers for the checks:**
      - **exposure:** in phase 11's white furnace box (radiance 2.0) with tone mapping off, EV100 1 gives pixels of linear
        value 0.8333 (sRGB 8-bit 235) and EV100 2 gives 0.4167 (173), ± 2 %;
      - **sun:** at phase 6's solar noon (elevation 62.39°), sky off, `illuminance` on open level ground (direct light
        only) is 100,000 · sin 62.39° = 88,612 lx ± 1 %; with cast shadows off, the phase-10 column no longer darkens
        the floor behind it in sunlight;
      - **sky:** at solar noon sky pixels have blue > red; at sun elevation 2° the sky pixels toward the sun have red >
        blue; at −10° the brightest sky pixel is under 1 % of the noon sky's;
      - **HDRI:** a uniform environment of radiance 1.0 (a small RGBE file the check writes) over an upward-facing white
        diffuse plane of albedo 0.5, EV100 −1, tone mapping off: plane pixels 0.8333 ± 2 % path-traced and ± 5 % raster;
      - **strip:** a 2 m strip of 500 lm/m facing down at 2.4 m in a dark room: `illuminance` on the floor under its
        centre is 49.72 lx ± 2 %;
      - **horizon:** an exterior camera 1.6 m above the site looking level: the pixel row 5 px under the horizon has no
        sky or clear-colour pixels across its full width;
      - **cutaway and clip:** with cutaway on, `pick3d` at the pixel of a window in a camera-facing wall returns an object
        behind it or `null`, never that wall or window; with a clip box ending at z = 2.0, a pick from above at a room's
        centre returns its floor;
      - **cameras:** a saved camera re-rendered after changing the global exposure, sun and light set gives the same
        image as before (fixed seed), and `camera.reset` after an orbit gives the saved image again.
    - **Demo:** the showcase house at dusk (twilight sky, windows and strip lights lit, ground to the horizon), at noon
      under an HDRI, and a floor overview with the cutaway, each from its own saved camera with its own look.
    - **Out of scope:** depth of field, colour grading, upscaling and DLSS, animated water and pools, shadow animation,
      clear coat, anisotropy and translucency, volumetrics, physical-camera exposure triangles (aperture, shutter, ISO).
20. **Plan views and dimensions.** (Andy 2026-11-07, as phase 16; CHIEF-REFERENCE §7.4.) Built on CADCraft's own
    layer states, named views and associative dimensions; extend them, don't duplicate.
    - **Plan views:** a saved plan view is a named object in the drawing: a level (or any level), a layer state with
      transparency, an annotation set (the default layer, text and dimension style for new annotation), and zoom. The
      template ships Working, Floor Plan, Electrical (phase 15's `view.electrical` becomes this view), Reflected Ceiling,
      Roof Plan, Kitchen & Bath and Site. Views open as tabs; create, duplicate, rename, delete. **Reference display:**
      a view can draw another level beneath in a muted colour.
    - **Reflected ceiling plan:** ceiling items, soffits, trays and vault lines as seen from below (drawn as looking
      down), with each room's finished ceiling height labelled (vaults labelled with their pitch).
    - **Roof plan:** ridges, hips and valleys as lines, eaves and gable edges, and each plane's pitch as a label with a
      slope arrow.
    - **Dimensions that know the building:** dimensions attach to wall faces, wall centre lines, opening centres and
      object edges, and follow them when the model changes. **Driving dimensions:** select an object, edit a dimension
      that locates it, and the object moves (the far end stays). **Auto exterior dimensions:** per side, up to three rows
      (openings; walls, with interior walls at their centre lines; overall), regenerated with the model unless a row was
      edited by hand (then marked edited and left alone).
    - **Room labels:** name, area (interior or standard, chosen per room), dimensions and finished ceiling height, each
      shown or hidden per plan view.
    - **Numbers for the checks:**
      - **auto dimensions** (phase-1 house): the overall row is 10.2 on the south and north sides and 8.2 on the east and
        west; the north wall row is 6.1 + 4.1, split at the interior wall's centre line x = 6;
      - **driving:** with the interior wall `(6,3)–(6,8)` selected, setting the 6.1 dimension to 5.6 moves it to x = 5.5:
        north-west room interior 5.35 × 4.85 = 25.9475 m², north-east 4.35 × 4.85 = 21.0975 m², and the row reads
        5.6 + 4.6;
      - **associative:** moving the phase-4 window 1 m along its wall changes its locating dimension by 1.0 after
        `arch.inspect`, with no other command;
      - **roof plan** (phase 4 hip at 6/12): one ridge line 2.0 m long, four hip lines of plan length 5.798 (± 1e-3)
        and four `6/12` labels;
      - **reflected ceiling plan:** phase 16's tray room shows two tray outlines and the label 2.70;
      - **plan views:** the Reflected Ceiling view's SVG export has no entity on `I-FURN`, and the Electrical view
        matches phase 15's results.
    - **Demo:** the showcase house's plan views for every level (floor plan, electrical, reflected ceiling), its roof
      plan and site plan, dimensioned automatically.
    - **Out of scope:** framing plans, story poles, construction lines, shadow plans, interior auto-dimensions other than
      phase 21's cabinet runs.
21. **Schedules, elevations and sheets.** (Andy 2026-11-07, as phase 16; CHIEF-REFERENCE §7.4.) Built on CADCraft's
    own layouts, viewports (`MVIEW`), `TABLE` and PDF plotting; extend them, don't duplicate.
    - **Labels and schedules:** each door, window and cabinet gets a schedule number (`D01`, `W01`, `C01`) shown as a
      label in plan and in elevations on its own layer. **Schedules** (door, window, cabinet, room finish) are `TABLE`
      entities linked to the model: chosen columns, identical items grouped in one row with a count, updated live.
      Opening indicators (the swing triangles) show in elevations on their own layer.
    - **Interior elevations:** `ELEVATION {"room", "wall"}` shows only that wall and what stands in front of it within the
      room, with cabinet door swings and automatic dimensions for the cabinet runs and overall height.
    - **Annotations survive regeneration:** text, dimensions and markers placed on a linked view are stored with the
      view, in view coordinates or attached to view features, and kept when it regenerates.
    - **Material patterns:** elevations draw each surface's 2D material pattern (siding courses, brick, shingles, tile)
      clipped to the face, so walls are not blank outlines.
    - **Sheets:** `sheet {"name", "size": "ARCH D", "views": [{"view", "scale", "at"}]}` places plan views, elevations,
      sections, interior elevations, schedules and a 3D image (a saved camera's render) as viewports on a CADCraft
      layout. A master title block on every sheet with macros (sheet number and title, project, date, scale); callout
      markers in plan name the sheet and view they open. `plot {"sheets": "all"}` writes one multi-page PDF in sheet
      order.
    - **Numbers for the checks:**
      - **schedules:** four 0.9 × 1.2 windows and one 1.2 × 1.2 give a window schedule of two rows with counts 4 and 1;
        resizing one of the four to 1.2 × 1.2 makes the counts 3 and 2;
      - **interior elevation:** in the phase-1 house, the north-west room's elevation of its east wall (the interior wall
        x = 6) has every segment within y 3.05…7.9 (± 1e-3), and a 0.6 m base cabinet against that wall shows as its front
        rectangle;
      - **annotations:** a text placed on that view keeps its view coordinates after a wall moves and the view
        regenerates;
      - **material patterns:** a wall in phase 13's brick shows horizontal pattern lines across its face in the front
        elevation, and the same wall in a plain paint shows none;
      - **sheets:** the showcase set's PDF has as many pages as sheets, each 914.4 × 609.6 mm (± 0.5); in a sheet's SVG
        export, the phase-1 house's 10.2 m south face at 1:50 is 204.0 mm ± 0.5.
    - **Demo:** the showcase house's full drawing set as one PDF: cover with a 3D render, site plan, a floor plan,
      electrical plan and reflected ceiling plan per level, roof plan, four elevations, two sections, kitchen and bath
      interior elevations, and the door, window, cabinet and room schedules.
    - **Out of scope:** framing plans, revision tables, CAD detail libraries, multi-file projects, LiDAR room capture.
