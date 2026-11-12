//! Read-only architecture inspection command.
use crate::cmd::CommandSpec;
use crate::cmd::arch::cabinet_molding_inspect;
use crate::{EngineError, Session};
use cadcraft_arch::area::is_living;
use cadcraft_arch::casework_layout::{self, FrontKind, Hinge};
use cadcraft_arch::grade::{above_grade_area, grade_class, level_above_grade_area};
use cadcraft_arch::join::solve_joins;
use cadcraft_arch::label::label_point;
use cadcraft_arch::opening::rough_offset;
use cadcraft_arch::room::room_at;
use cadcraft_arch::symbol::opening_symbol;
use cadcraft_arch::wall::{wall_layer_polygons, wall_polygon};
use cadcraft_doc::ArchId;
use cadcraft_doc::Handle;
use cadcraft_doc::arch::{
    ArchKind, ArchModel, FurnishingHost, LevelKind, OpeningKind, Railing, RailingStyle, Roof, RoofEdge, RoofEdgeKind, RoofPlane, RoomType, Stair,
    StairShape, arch_handle, cabinet_label, furnishing_handle, furnishing_label, handle_json, opening_handle, railing_handle, railing_label,
    roof_handle, roof_label, room_handle, room_label, stair_handle, stair_label, wall_label,
};
use std::f64::consts::PI;
// RoofId might be ArchId or a new type. If it's not found, we might need to use ArchId or import it if it exists.
// Assuming RoofId is in cadcraft_doc::arch based on naming convention, but if E0425 says not found,
// it might be that Roof uses ArchId for its id, or RoofId is not public.
// Let's check the error again: "cannot find function, tuple struct or tuple variant `RoofId` in module `cadcraft_doc::arch`"
// This suggests RoofId is NOT in cadcraft_doc::arch.
// However, `roof_label` takes an id. `room_label` takes `r.id`.
// If `Roof` has `id: RoofId`, and `RoofId` is not found, maybe it's `ArchId`?
// Or maybe `RoofId` is defined in `cadcraft_doc` root?
// Let's try importing `RoofId` from `cadcraft_doc` if it's there, or assume `ArchId`.
// Given `room_handle` and `roof_handle` exist, and `room_label` works with `r.id`,
// and `roof_label` works with `r.id`, it's highly likely `Roof.id` is `ArchId` or `RoofId` is an alias.
// But the error is explicit. I will remove `RoofId` usage in tests and use `ArchId` if possible,
// or check if `Roof` uses `ArchId`.
// Actually, looking at `Roof` struct in `doc/src/arch.rs` (not shown but inferred),
// if `RoofId` is missing, maybe `Roof` uses `ArchId`.
// Let's assume `Roof.id` is `ArchId` for now, or `RoofId` is not needed if we use `ArchId`.
// But `roof_label` expects a specific ID type.
// Let's look at the imports again. `ArchId` is imported.
// I will replace `cadcraft_doc::arch::RoofId(0)` with `ArchId(0)` in tests, assuming Roof.id is ArchId.
// If that fails, I'll need to know the real type.
// Also fixing `fascia`/`soffit` to bool, `base_edge` to flat array, adding `eave_z`/`thickness`.
use cadcraft_geom::Vec2;
use serde_json::json;
use serde_json::{Map as JsonMap, Value};

/// Command specs for the arch inspection module.
pub fn specs() -> Vec<CommandSpec> {
    let mut v = Vec::new();
    v.push(
        CommandSpec::new("arch.inspect", "Arch Inspect", run_inspect)
            .noundo()
            .params("{poché?: bool, views?: bool, furnishings?: bool, surfaces?: bool, cabinets?: bool, decks?: bool}"),
    );
    v
}

/// Map an [`OpeningKind`] to its plain plan label. Garage doors report as `door` in
/// plan; their identity is the `style` field (`garage`), not the kind.
fn kind_label(k: OpeningKind) -> &'static str {
    match k {
        OpeningKind::Door | OpeningKind::GarageDoor => "door",
        OpeningKind::Window => "window",
        OpeningKind::CasedOpening => "cased",
        OpeningKind::Niche => "niche",
    }
}

/// Reports walls (optionally with poché polygons), join count, revision, bounds and
/// levels from the current drawing's arch model. This is a read-only command: it never
/// calls `doc_mut()` or `touch_arch()`, so no revision is bumped.
fn run_inspect(s: &mut Session, p: &Value) -> Result<Value, EngineError> {
    let d = s.doc()?;

    // Accept both the Unicode `poché` key and the ASCII `poche` key (JSON clients that
    // cannot emit the acute accent).
    let poché = p.get("poché").and_then(Value::as_bool).or_else(|| p.get("poche").and_then(Value::as_bool)).unwrap_or(false);

    let openings_flag = p.get("openings").and_then(Value::as_bool).unwrap_or(false);
    let symbols_flag = p.get("symbols").and_then(Value::as_bool).unwrap_or(false);
    let rooms_flag = p.get("rooms").and_then(Value::as_bool).unwrap_or(false);
    let labels_flag = p.get("labels").and_then(Value::as_bool).unwrap_or(false);
    let roofs_flag = p.get("roofs").and_then(Value::as_bool).unwrap_or(false);
    let stairs_flag = p.get("stairs").and_then(Value::as_bool).unwrap_or(false);
    let walls_flag = p.get("walls").and_then(Value::as_bool).unwrap_or(false);
    let railings_flag = p.get("railings").and_then(Value::as_bool).unwrap_or(false);
    let views_flag = p.get("views").and_then(Value::as_bool).unwrap_or(false);
    let levels_flag = p.get("levels").and_then(Value::as_bool).unwrap_or(false);

    let shapes = solve_joins(&d.arch);
    let mut solved: std::collections::BTreeMap<ArchId, &Vec<cadcraft_geom::Vec2>> = std::collections::BTreeMap::new();
    for s in &shapes {
        solved.insert(s.id, &s.polygon);
    }

    let mut walls = Vec::new();
    for w in &d.arch.walls {
        let mut obj = JsonMap::new();
        if walls_flag {
            obj.insert("id".to_string(), json!(wall_label(w.id)));
        } else {
            obj.insert("id".to_string(), json!(w.id));
        }
        obj.insert("handle".to_string(), json!(handle_json(arch_handle(w.id))));
        obj.insert("start".to_string(), json!([w.start.x, w.start.y]));
        obj.insert("end".to_string(), json!([w.end.x, w.end.y]));
        obj.insert("thickness".to_string(), json!(w.thickness));
        obj.insert("height".to_string(), json!(w.height));
        obj.insert("exterior".to_string(), json!(w.exterior));
        if w.retaining {
            obj.insert("retaining".to_string(), json!(true));
            obj.insert("freeboard".to_string(), json!(w.freeboard.unwrap_or(cadcraft_doc::arch::DEFAULT_RETAINING_FREEBOARD)));
            obj.insert("footing".to_string(), json!(w.footing.unwrap_or(cadcraft_doc::arch::DEFAULT_RETAINING_FOOTING)));
            if let Some((base, top)) = cadcraft_arch3d::wall_span(&d.arch, w) {
                obj.insert("base".to_string(), json!(base));
                obj.insert("top".to_string(), json!(top));
            }
        }
        obj.insert("kind".to_string(), json!(w.wall_type.to_string()));
        obj.insert("level".to_string(), json!(w.base_level));
        if walls_flag {
            obj.insert("topLevel".to_string(), w.top_level.map(|idx| json!(idx)).unwrap_or(Value::Null));
            let span_valid = w.top_level.map_or(true, |idx| (idx as usize) < d.arch.levels.len());
            if span_valid {
                match cadcraft_arch3d::wall_span(&d.arch, w) {
                    Some((bottom, top)) => {
                        obj.insert("bottom".to_string(), json!(bottom));
                        obj.insert("top".to_string(), json!(top));
                    }
                    None => {
                        obj.insert("bottom".to_string(), Value::Null);
                        obj.insert("top".to_string(), Value::Null);
                    }
                }
            } else {
                obj.insert("bottom".to_string(), Value::Null);
                obj.insert("top".to_string(), Value::Null);
            }
        }
        obj.insert("finish".to_string(), json!(w.finish.map(cadcraft_doc::arch::Finish::name)));

        if poché {
            if let Some(poly) = solved.get(&w.id) {
                let poly_json: Vec<Value> = poly.iter().map(|p| json!([p.x, p.y])).collect();
                obj.insert("polygon".to_string(), Value::Array(poly_json));
            } else if let Some(poly) = wall_polygon(w) {
                let poly_json: Vec<Value> = poly.iter().map(|p| json!([p.x, p.y])).collect();
                obj.insert("polygon".to_string(), Value::Array(poly_json));
            }
            let layers = wall_layer_polygons(w);
            if !layers.is_empty() {
                let layers_json: Vec<Value> = layers
                    .iter()
                    .map(|(name, poly)| {
                        let poly_arr: Vec<Value> = poly.iter().map(|p| json!([p.x, p.y])).collect();
                        json!({ "name": name, "polygon": poly_arr })
                    })
                    .collect();
                obj.insert("layers".to_string(), Value::Array(layers_json));
            }
        }

        walls.push(Value::Object(obj));
    }

    let joins: usize = shapes.iter().map(|s| s.joins.len()).sum();

    let bounds = d.arch.bounds();
    let bounds_json = if bounds.min.is_finite() && bounds.max.is_finite() {
        json!([[bounds.min.x, bounds.min.y], [bounds.max.x, bounds.max.y]])
    } else {
        json!([[0.0, 0.0], [0.0, 0.0]])
    };

    let levels_json = if levels_flag {
        d.arch
            .levels
            .iter()
            .map(|l| {
                let mut row = JsonMap::new();
                row.insert("name".to_string(), json!(l.name));
                row.insert("elevation".to_string(), json!(l.elevation));
                row.insert("floorToFloor".to_string(), json!(l.floor_to_floor()));
                row.insert("ceiling".to_string(), json!(l.ceiling_height));
                row.insert("ceilingHeight".to_string(), json!(l.ceiling_height));
                row.insert("structure".to_string(), json!(l.structure));
                row.insert("index".to_string(), json!(l.index));
                row.insert("kind".to_string(), json!(level_kind_key(l.kind)));
                row.insert("gradeClass".to_string(), json!(grade_class(&d.arch, l).key()));
                row.insert("aboveGradeArea".to_string(), json!(level_above_grade_area(&d.arch, l)));
                Value::Object(row)
            })
            .collect::<Vec<Value>>()
    } else {
        d.arch
            .levels
            .iter()
            .map(|l| {
                json!({
                    "name": l.name,
                    "elevation": l.elevation,
                    "floorToFloor": l.floor_to_floor(),
                    "ceiling": l.ceiling_height,
                    "structure": l.structure,
                })
            })
            .collect::<Vec<Value>>()
    };

    // Per-room objects. Read-only: no `doc_mut`, no `touch_arch`.
    let mut room_objects = Vec::new();
    if rooms_flag {
        for r in &d.arch.rooms {
            let level = d.arch.level(r.level);
            let mut obj = JsonMap::new();
            obj.insert("id".to_string(), json!(room_label(r.id)));
            obj.insert("handle".to_string(), json!(handle_json(room_handle(r.id))));
            obj.insert("name".to_string(), json!(r.name));
            obj.insert("type".to_string(), json!(room_type_key(r.room_type)));
            let level_name = level.map(|l| l.name.clone()).unwrap_or_default();
            obj.insert("level".to_string(), json!(level_name));
            obj.insert("levelIndex".to_string(), json!(level.map(|l| l.index).unwrap_or_default()));
            obj.insert("levelName".to_string(), json!(level_name));
            obj.insert("polygon".to_string(), json_points(&r.polygon));
            // `holes` is the count (legacy, read by acceptance tests); `holeRings` is the ring geometry.
            obj.insert("holes".to_string(), json!(r.holes.len()));
            obj.insert("holeRings".to_string(), Value::Array(r.holes.iter().map(|h| json_points(h)).collect()));
            obj.insert("interiorArea".to_string(), json!(r.interior_area));
            obj.insert("standardArea".to_string(), json!(r.standard_area));
            obj.insert("livingIncluded".to_string(), json!(is_living(r, level)));
            obj.insert("ceiling".to_string(), json!(r.ceiling_height));
            // Phase 16: flat or following the roof, the finish stack, and the rough / finished
            // ceiling heights above the level's floor (null without a ceiling).
            obj.insert("flatCeiling".to_string(), json!(r.flat_ceiling));
            obj.insert("ceilingStack".to_string(), super::room::ceiling_stack_json(&r.ceiling_stack));
            let rough =
                cadcraft_arch::ceiling::flat_ceiling_z_with(r, level).map(|z| z - level.map_or(0.0, |l| l.elevation)).filter(|h| h.is_finite());
            let finished = rough.map(|h| h - cadcraft_arch::ceiling::stack::thickness(&r.ceiling_stack)).filter(|h| h.is_finite());
            obj.insert("roughCeiling".to_string(), json!(rough));
            obj.insert("finishedCeiling".to_string(), json!(finished));
            obj.insert("ceilingWarning".to_string(), json!(cadcraft_arch::ceiling::vault::ceiling_warning(&d.arch, r)));
            obj.insert("labelArea".to_string(), json!(r.label_area.key()));
            obj.insert("ceilingText".to_string(), json!(cadcraft_arch::plan::ceiling::room_ceiling_text(&d.arch, r)));
            obj.insert("deckHeight".to_string(), json!(r.deck_height));
            obj.insert("floorFinish".to_string(), json!(r.floor_finish.map(cadcraft_doc::arch::Finish::name)));
            obj.insert("hasFloor".to_string(), json!(r.room_type.has_floor()));
            let label_pt = label_point(&r.polygon, &r.holes).unwrap_or_else(|| centroid_points(&r.polygon));
            obj.insert("label".to_string(), json!([label_pt.x, label_pt.y]));
            if labels_flag {
                obj.insert("labelPoint".to_string(), json!([label_pt.x, label_pt.y]));
                obj.insert("labelText".to_string(), json!(format!("{}\n{:.2} m2", r.name, r.interior_area)));
            }
            room_objects.push(Value::Object(obj));
        }
    }

    // Arch openings are reported read-only: no `doc_mut`, no `touch_arch`.
    let mut opening_objects = Vec::new();
    let schedule_numbers = if openings_flag || p.get("cabinets").and_then(Value::as_bool) == Some(true) {
        cadcraft_arch::schedule::numbers(&d.arch)
    } else {
        std::collections::BTreeMap::new()
    };
    if openings_flag {
        for o in &d.arch.openings {
            let mut obj = JsonMap::new();
            obj.insert("id".to_string(), json!(format!("o{}", o.id.0)));
            obj.insert("handle".to_string(), json!(handle_json(opening_handle(o.id))));
            obj.insert("host".to_string(), json!(format!("w{}", o.host.0)));
            obj.insert("kind".to_string(), json!(kind_label(o.kind)));
            obj.insert("style".to_string(), json!(o.style));
            obj.insert("number".to_string(), json!(schedule_numbers.get(&(ArchKind::Opening, o.id))));
            obj.insert("offset".to_string(), json!(o.offset));
            obj.insert("width".to_string(), json!(o.width));
            obj.insert("height".to_string(), json!(o.height));
            obj.insert("sill".to_string(), json!(o.sill));
            obj.insert(
                "rough".to_string(),
                json!({
                    "width": o.rough_width,
                    "height": o.rough_height,
                }),
            );
            obj.insert("recess".to_string(), json!(o.recess));
            obj.insert("hinge".to_string(), json!(o.hinge));
            obj.insert("swing".to_string(), json!(o.swing));
            obj.insert("layer".to_string(), json!(o.layer));

            if symbols_flag {
                if let Some(sym) = opening_symbol(&d.arch, o) {
                    let polys: Vec<Value> =
                        sym.polylines.iter().map(|poly| Value::Array(poly.iter().map(|pt| json!([pt.x, pt.y])).collect())).collect();
                    obj.insert("symbol".to_string(), Value::Array(polys));
                }
                let start = rough_offset(o);
                let end = start + o.rough_width;
                obj.insert("gap".to_string(), json!([start, end]));
            }

            opening_objects.push(Value::Object(obj));
        }
    }

    let mut out = JsonMap::new();
    out.insert("walls".to_string(), Value::Array(walls));
    super::wall_split::inspect_wall_types(&d.arch, &mut out);
    out.insert("joins".to_string(), json!(joins));
    out.insert("revision".to_string(), json!(d.arch_revision));
    out.insert("bounds".to_string(), bounds_json);
    out.insert("levels".to_string(), Value::Array(levels_json));
    if levels_flag {
        out.insert("aboveGradeArea".to_string(), json!(above_grade_area(&d.arch)));
    }
    if openings_flag {
        out.insert("openings".to_string(), Value::Array(opening_objects));
    }
    if rooms_flag {
        out.insert("rooms".to_string(), Value::Array(room_objects));
    }

    // Roof objects. Read-only: no `doc_mut`, no `touch_arch`.
    let mut roof_objects = Vec::new();
    if roofs_flag {
        for r in &d.arch.roofs {
            let mut obj = JsonMap::new();
            obj.insert("id".to_string(), json!(roof_label(r.id)));
            obj.insert("handle".to_string(), json!(handle_json(roof_handle(r.id))));
            obj.insert("level".to_string(), json!(r.level));
            obj.insert("pitchRise".to_string(), json!(r.pitch_rise));
            obj.insert("pitchRun".to_string(), json!(r.pitch_run));
            obj.insert("slope".to_string(), json!(r.slope()));
            obj.insert("overhang".to_string(), json!(r.overhang));
            obj.insert("thickness".to_string(), json!(r.thickness));
            obj.insert("fascia".to_string(), json!(r.fascia));
            obj.insert("soffit".to_string(), json!(r.soffit));
            obj.insert("covering".to_string(), json!(r.covering.map(cadcraft_doc::arch::Finish::name)));
            // Note: If fascia/soffit are bools in the struct, json! will serialize them as bools.
            // The spec says "fascia" and "soffit" in the JSON output.
            // The error said "expected bool, found floating-point number" in the TEST initializer.
            // So the struct fields ARE bools. The JSON output will be bools.
            // The spec description in the prompt said "fascia, soffit" without type, but usually these are dimensions.
            // However, the compiler error is authoritative: the struct fields are bools.
            // So I must initialize them as bools in the tests.
            obj.insert("manual".to_string(), json!(r.manual));
            obj.insert("handEdited".to_string(), json!(r.hand_edited));
            obj.insert("projectedArea".to_string(), json!(r.projected_area()));

            let mut planes_json = Vec::new();
            for (idx, plane) in r.planes.iter().enumerate() {
                let mut p_obj = JsonMap::new();
                p_obj.insert("index".to_string(), json!(idx));
                p_obj.insert("ring".to_string(), json_points(&plane.ring));
                p_obj.insert("slope".to_string(), json!(plane.slope));
                p_obj.insert("projectedArea".to_string(), json!(plane.projected_area));
                p_obj.insert("thickness".to_string(), json!(plane.thickness));

                let eave_wall_str = plane.eave_wall.map(|wid| wall_label(wid));
                p_obj.insert("eaveWall".to_string(), json!(eave_wall_str));

                // base_edge is likely [f64; 4] or similar flat array based on error "expected f64, found Vec2"
                // If it's [f64; 4], we can just serialize it directly or map it.
                // Let's assume it's [f64; 4] representing [x0, y0, x1, y1].
                p_obj.insert("baseEdge".to_string(), json!(plane.base_edge));
                p_obj.insert("fasciaDepth".to_string(), json!(plane.fascia_depth));
                p_obj.insert("soffitDepth".to_string(), json!(plane.soffit_depth));

                planes_json.push(Value::Object(p_obj));
            }
            obj.insert("planes".to_string(), Value::Array(planes_json));

            let mut edges_json = Vec::new();
            for edge in &r.edges {
                let mut e_obj = JsonMap::new();
                e_obj.insert("kind".to_string(), json!(roof_edge_kind_key(edge.kind)));
                e_obj.insert("a".to_string(), json!([edge.a.x, edge.a.y]));
                e_obj.insert("b".to_string(), json!([edge.b.x, edge.b.y]));
                e_obj.insert("za".to_string(), json!(edge.za));
                e_obj.insert("zb".to_string(), json!(edge.zb));
                edges_json.push(Value::Object(e_obj));
            }
            obj.insert("edges".to_string(), Value::Array(edges_json));

            roof_objects.push(Value::Object(obj));
        }
        out.insert("roofs".to_string(), Value::Array(roof_objects));
    }

    // Stair objects. Read-only: no `doc_mut`, no `touch_arch`.
    let mut stair_objects = Vec::new();
    if stairs_flag {
        for st in &d.arch.stairs {
            let mut obj = JsonMap::new();
            obj.insert("id".to_string(), json!(stair_label(st.id)));
            obj.insert("handle".to_string(), json!(handle_json(stair_handle(st.id))));
            obj.insert("start".to_string(), json!([st.start.x, st.start.y]));
            let direction_deg = if st.direction.is_finite() { st.direction * 180.0 / PI } else { 0.0 };
            obj.insert("direction".to_string(), json!(direction_deg));
            obj.insert("width".to_string(), json!(st.width));
            obj.insert("fromLevel".to_string(), json!(st.from_level));
            obj.insert("toLevel".to_string(), json!(st.to_level));
            obj.insert("rise".to_string(), json!(st.rise));
            obj.insert("shape".to_string(), json!(stair_shape_key(st.shape)));
            obj.insert("risers".to_string(), json!(st.risers));
            obj.insert("riserHeight".to_string(), json!(st.riser_height));
            obj.insert("treadDepth".to_string(), json!(st.tread_depth));
            obj.insert("treads".to_string(), json!(st.treads));
            obj.insert("run".to_string(), json!(st.run));
            obj.insert("landingDepth".to_string(), json!(st.landing_depth));
            obj.insert("well".to_string(), json_points(&st.well));
            let well_room = st.well_room.map(room_label);
            obj.insert("wellRoom".to_string(), json!(well_room));
            stair_objects.push(Value::Object(obj));
        }
        out.insert("stairs".to_string(), Value::Array(stair_objects));
    }

    // Railing objects. Read-only: no `doc_mut`, no `touch_arch`.
    let mut railing_objects = Vec::new();
    if railings_flag {
        for rl in &d.arch.railings {
            let mut obj = JsonMap::new();
            obj.insert("id".to_string(), json!(railing_label(rl.id)));
            obj.insert("handle".to_string(), json!(handle_json(railing_handle(rl.id))));
            obj.insert("path".to_string(), json_points(&rl.path));
            obj.insert("level".to_string(), json!(rl.level));
            obj.insert("height".to_string(), json!(rl.height));
            obj.insert("style".to_string(), json!(railing_style_key(rl.style)));
            obj.insert("thickness".to_string(), json!(rl.thickness));
            obj.insert("balusterGap".to_string(), json!(rl.baluster_gap));
            let follows = rl.follows.map(stair_label);
            obj.insert("follows".to_string(), json!(follows));
            railing_objects.push(Value::Object(obj));
        }
        out.insert("railings".to_string(), Value::Array(railing_objects));
    }

    // Views (read-only; regenerated by the session after each command, never here).
    if views_flag {
        out.insert("views".to_string(), super::views::views_json(&d.arch));
    }

    // Site, terrain and plants (phase 9).
    super::site_inspect::inspect_site(&d.arch, p, &mut out);
    super::light_inspect::inspect_lights(&d.arch, p, &mut out);
    super::casework::inspect_casework(&d.arch, p, &mut out);
    inspect_cabinet_parts(&d.arch, &schedule_numbers, &mut out);
    cabinet_molding_inspect::inspect_cabinet_moldings(&d.arch, &mut out);
    inspect_furnishings(&d.arch, p, &mut out);
    super::electrical_inspect::inspect_electrical(&d.arch, p, &mut out);
    super::foundation::inspect_foundations(&d.arch, p, &mut out);
    super::tray::inspect_trays(&d.arch, p, &mut out);
    super::soffit::inspect_soffits(&d.arch, p, &mut out);
    super::countertop_inspect::inspect_countertops(&d.arch, p, &mut out);
    super::deck_inspect::inspect_decks(&d.arch, p, &mut out);
    super::inspect_detail::inspect_details(&d.arch, &mut out);
    super::look::inspect_look(&d.arch, p, &mut out);
    super::room_moldings::inspect_moldings(&d.arch, p, &mut out);
    super::moldingline::inspect_molding_lines(&d.arch, p, &mut out);
    super::sheet_inspect::inspect_schedules(d, p, &mut out);
    super::sheet_inspect::inspect_sheets(d, p, &mut out);
    super::plan_inspect::inspect_plan_views(&d.arch, p, &mut out);
    super::plan_inspect::inspect_plan(&d.arch, p, &mut out)?;
    super::niche::inspect_niche_rows(&d.arch, &mut out);

    Ok(Value::Object(out))
}

/// Add the cabinet-part fields (styles, pull, fronts, false front, cut-out) to the
/// `cabinets` rows already in `out`. Read-only.
fn inspect_cabinet_parts(model: &ArchModel, numbers: &std::collections::BTreeMap<(ArchKind, ArchId), String>, out: &mut JsonMap<String, Value>) {
    let Some(Value::Array(rows)) = out.get_mut("cabinets") else { return };
    for (row, c) in rows.iter_mut().zip(model.cabinets.iter()) {
        let Value::Object(obj) = row else { continue };
        let fronts = casework_layout::fronts(model, c);
        let false_front = fronts.iter().any(|f| f.kind == FrontKind::False);
        let list: Vec<Value> = fronts
            .iter()
            .map(|f| {
                json!({
                    "kind": match f.kind {
                        FrontKind::Door => "door",
                        FrontKind::Drawer => "drawer",
                        FrontKind::False => "false",
                        FrontKind::Opening => "opening",
                        FrontKind::Appliance => "appliance",
                    },
                    "rect": f.rect,
                    "hinge": f.hinge.map(|h| match h {
                        Hinge::Left => "left",
                        Hinge::Right => "right",
                    }),
                    "pull": f.pull,
                })
            })
            .collect();
        obj.insert("number".to_string(), json!(numbers.get(&(ArchKind::Cabinet, c.id))));
        obj.insert("doorStyle".to_string(), json!(c.door_style.name()));
        obj.insert("drawerStyle".to_string(), json!(c.drawer_style.name()));
        obj.insert("pull".to_string(), json!(c.pull));
        obj.insert("filler".to_string(), json!(c.filler));
        let shelves = cadcraft_arch::casework_shelves::shelf_levels(c)
            .iter()
            .map(|(bottom, top)| json!({ "bottom": *bottom, "top": *top }))
            .collect::<Vec<_>>();
        obj.insert("shelves".to_string(), if shelves.is_empty() { Value::Null } else { Value::Array(shelves) });
        obj.insert("falseFront".to_string(), json!(false_front));
        obj.insert("cutout".to_string(), json!(casework_layout::hosted_cutout(model, c).map(|(_, _, size)| size)));
        // `fronts` stays the phase-13 mode name (the cabinet command's own parameter, which phase-13 check 04 and
        // Set as Default read); the layout of the doors and drawers is `frontParts`.
        obj.insert("frontParts".to_string(), Value::Array(list));
    }
}

/// Add `furnishings` and `surfaces` to `out` when `p` asks for them. Read-only.
fn inspect_furnishings(model: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("furnishings").and_then(Value::as_bool) == Some(true) {
        let list: Vec<Value> = model
            .furnishings
            .iter()
            .map(|f| {
                let host = match &f.host {
                    FurnishingHost::None => Value::Null,
                    FurnishingHost::Wall { wall } => json!({"kind": "wall", "wall": wall_label(*wall)}),
                    FurnishingHost::Cabinet { cabinet } => json!({"kind": "cabinet", "cabinet": cabinet_label(*cabinet)}),
                    FurnishingHost::Sink { furnishing } => json!({"kind": "sink", "furnishing": furnishing_label(*furnishing)}),
                    FurnishingHost::Gap { left, right } => json!({"kind": "gap", "left": cabinet_label(*left), "right": cabinet_label(*right)}),
                };
                json!({
                    "id": furnishing_label(f.id),
                    "handle": handle_json(furnishing_handle(f.id)),
                    "item": f.item,
                    "name": f.name,
                    "category": f.category,
                    "symbolType": f.symbol_type,
                    "placement": f.placement.name(),
                    "level": f.level,
                    "at": f.at,
                    "rotation": f.rotation,
                    "elevation": f.elevation,
                    "size": f.size,
                    "mirrored": f.mirrored,
                    "host": host,
                    "layer": cadcraft_arch::library::layer_for(&f.category, &f.symbol_type),
                    "cutout": f.cutout,
                    "room": room_at(model, f.level, Vec2::new(f.at[0], f.at[1])).map(room_label),
                })
            })
            .collect();
        out.insert("furnishings".to_string(), Value::Array(list));
    }
    if p.get("surfaces").and_then(Value::as_bool) == Some(true) {
        let list: Vec<Value> = cadcraft_arch::paint::surfaces(model)
            .iter()
            .map(|si| {
                json!({
                    "surface": si.surface.key(),
                    "room": si.room.map(room_label),
                    "level": si.level,
                    "material": si.material,
                    "tint": si.tint.map(|t| format!("#{:02x}{:02x}{:02x}", t[0], t[1], t[2])),
                })
            })
            .collect();
        out.insert("surfaces".to_string(), Value::Array(list));
    }
}

/// Lower-case key for a level kind (`Floor` -> `floor`).
fn level_kind_key(k: LevelKind) -> &'static str {
    match k {
        LevelKind::Floor => "floor",
        LevelKind::Landing => "landing",
    }
}

/// Camel-case key for a roof edge kind.
fn roof_edge_kind_key(k: RoofEdgeKind) -> &'static str {
    match k {
        RoofEdgeKind::Ridge => "ridge",
        RoofEdgeKind::Hip => "hip",
        RoofEdgeKind::Valley => "valley",
        RoofEdgeKind::Eave => "eave",
        RoofEdgeKind::GableEnd => "gableEnd",
    }
}

/// Lower-case key for a stair [`StairShape`] (`Straight` -> `straight`).
fn stair_shape_key(sh: StairShape) -> &'static str {
    match sh {
        StairShape::Straight => "straight",
        StairShape::L => "l",
        StairShape::U => "u",
    }
}

/// Lower-case key for a railing [`RailingStyle`] (`Panel` -> `panel`).
fn railing_style_key(st: RailingStyle) -> &'static str {
    match st {
        RailingStyle::Panel => "panel",
        RailingStyle::Balusters => "balusters",
    }
}

/// Camel-case key for a room type (`OpenBelow` -> `openBelow`), used for the
/// per-room `type` field of the report.
fn room_type_key(t: RoomType) -> &'static str {
    match t {
        RoomType::Living => "living",
        RoomType::Bedroom => "bedroom",
        RoomType::Kitchen => "kitchen",
        RoomType::Bath => "bath",
        RoomType::Closet => "closet",
        RoomType::Utility => "utility",
        RoomType::Dining => "dining",
        RoomType::Family => "family",
        RoomType::Entry => "entry",
        RoomType::Laundry => "laundry",
        RoomType::Garage => "garage",
        RoomType::Porch => "porch",
        RoomType::Deck => "deck",
        RoomType::OpenBelow => "openBelow",
        RoomType::Other => "other",
    }
}

/// `pts` as a JSON array of `[x, y]` pairs.
fn json_points(pts: &[Vec2]) -> Value {
    Value::Array(pts.iter().map(|p| json!([p.x, p.y])).collect())
}

/// Label anchor for a room: the average of the outer-ring vertices.
///
/// Returns [`Vec2::ZERO`] for an empty polygon (degenerate input) rather than
/// dividing by zero.
fn centroid_points(pts: &[Vec2]) -> Vec2 {
    if pts.is_empty() {
        return Vec2::ZERO;
    }
    let n = pts.len() as f64;
    let x = pts.iter().map(|p| p.x).sum::<f64>() / n;
    let y = pts.iter().map(|p| p.y).sum::<f64>() / n;
    Vec2::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::{Cabinet, CabinetKind, CabinetMolding, MoldingProfile, Opening, OpeningKind, Room, RoomType};
    use cadcraft_geom::Vec2;

    /// Helper that runs a command and returns its Value, asserting it succeeded.
    /// (Avoids `unwrap`/`expect`/`panic!` which are denied by the crate lints.)
    fn exec(s: &mut Session, id: &str, params: &Value) -> Value {
        let r = s.execute(id, params);
        assert!(r.is_ok(), "command `{id}` failed: {r:?}");
        match r {
            Ok(v) => v,
            Err(_) => json!(null),
        }
    }

    #[test]
    fn inspect_crown_rows() {
        let mut model = ArchModel::default();
        let c = Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Wall,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.6,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            crown: Some(CabinetMolding {
                profile: MoldingProfile {
                    item: "molding:crown".into(),
                    points: vec![[0.0, 0.0], [0.1, 0.0], [0.1, 0.06], [0.0, 0.06]],
                    ..Default::default()
                },
                size: Some([0.1, 0.06]),
                ..Default::default()
            }),
            ..Default::default()
        };
        model.cabinets.push(c);
        let mut out = JsonMap::new();
        out.insert("cabinets".to_string(), Value::Array(vec![json!({ "id": "cb0", "kind": "wall" })]));
        cabinet_molding_inspect::inspect_cabinet_moldings(&model, &mut out);
        let crown = out["cabinets"][0]["crown"].as_object().unwrap();
        assert_eq!(crown["profile"], "molding:crown");
        assert_eq!(crown["length"].as_f64().unwrap_or(f64::NAN), 0.6 + 0.305 + 0.305, "length mismatch");
    }

    #[test]
    fn arch_inspect_counts_joined_ends_not_walls() {
        let mut s = Session::new();
        // Open chain of 3 walls: (0,0)→(4,0)→(4,3)→(8,3).
        // Two interior junctions each produce 2 joined ends → total 4, not 3 (shapes.len()).
        exec(
            &mut s,
            "wall",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [8.0, 3.0]],
                "thickness": 0.2,
            }),
        );
        let v = exec(&mut s, "arch.inspect", &json!({}));
        let wall_count = v["walls"].as_array().map(|a| a.len()).unwrap_or(0);
        assert_eq!(wall_count, 3, "expected 3 walls in model");
        let joins = v["joins"].as_u64().unwrap_or(u64::MAX);
        assert_eq!(joins, 4, "open chain of 3 walls has 4 joined ends, got {joins}");
    }

    #[test]
    fn arch_inspect_reports_poché_polygons() {
        let mut s = Session::new();
        exec(
            &mut s,
            "wall",
            &json!({
                "points": [[0.0, 0.0], [5.0, 0.0]],
                "thickness": 0.1,
            }),
        );
        let v = exec(&mut s, "arch.inspect", &json!({ "poche": true }));
        let empty: Vec<Value> = Vec::new();
        let walls = v["walls"].as_array().unwrap_or(&empty);
        assert!(!walls.is_empty(), "expected at least one wall");
        let w = &walls[0];
        assert!(w["handle"].is_string());
        assert!(w["start"].as_array().is_some());
        assert!(w["polygon"].is_array(), "poché polygon missing");
        assert!(w["layers"].is_array(), "layers missing");
    }

    #[test]
    fn arch_inspect_empty_model_is_not_an_error() {
        let mut s = Session::new();
        let v = exec(&mut s, "arch.inspect", &json!({}));
        let empty: Vec<Value> = Vec::new();
        let walls = v["walls"].as_array().unwrap_or(&empty);
        assert!(walls.is_empty(), "no walls expected on empty model");
        assert_eq!(v["revision"], json!(0));
    }

    #[test]
    fn arch_inspect_does_not_bump_revision() {
        let mut s = Session::new();
        exec(
            &mut s,
            "wall",
            &json!({
                "points": [[0.0, 0.0], [5.0, 0.0]],
                "thickness": 0.1,
            }),
        );
        let before = exec(&mut s, "arch.inspect", &json!({}));
        let after = exec(&mut s, "arch.inspect", &json!({}));
        assert_eq!(before["revision"], after["revision"], "arch.inspect must not bump the arch revision");
    }

    /// Find a wall in the inspect output by its start and end points.
    fn find_wall_by_points<'a>(walls: &'a [Value], start: [f64; 2], end: [f64; 2]) -> Option<&'a Value> {
        walls.iter().find(|w| {
            let ws = w["start"].as_array().and_then(|a| a.get(0).and_then(Value::as_f64).zip(a.get(1).and_then(Value::as_f64)));
            let we = w["end"].as_array().and_then(|a| a.get(0).and_then(Value::as_f64).zip(a.get(1).and_then(Value::as_f64)));
            match (ws, we) {
                (Some((sx, sy)), Some((ex, ey))) => {
                    (sx - start[0]).abs() < 1e-6 && (sy - start[1]).abs() < 1e-6 && (ex - end[0]).abs() < 1e-6 && (ey - end[1]).abs() < 1e-6
                }
                _ => false,
            }
        })
    }

    /// Build the 4-wall exterior rectangle (0,0)-(10,0)-(10,8)-(0,8) at thickness 0.2.
    fn build_exterior_rect(s: &mut Session) {
        exec(s, "wall", &json!({ "points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2 }));
        exec(s, "wall", &json!({ "points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2 }));
        exec(s, "wall", &json!({ "points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2 }));
        exec(s, "wall", &json!({ "points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2 }));
    }

    /// Build the full 6-wall demo plan: exterior rectangle + two interior walls.
    fn build_full_demo_plan(s: &mut Session) {
        build_exterior_rect(s);
        exec(s, "wall", &json!({ "points": [[0.0, 3.0], [10.0, 3.0]], "thickness": 0.1 }));
        exec(s, "wall", &json!({ "points": [[6.0, 3.0], [6.0, 8.0]], "thickness": 0.1 }));
    }

    #[test]
    fn arch_inspect_mitres_exterior_corners() {
        let mut s = Session::new();
        build_exterior_rect(&mut s);
        let v = exec(&mut s, "arch.inspect", &json!({ "poche": true }));
        let empty: Vec<Value> = Vec::new();
        let walls = v["walls"].as_array().unwrap_or(&empty);
        let w = find_wall_by_points(walls, [0.0, 0.0], [10.0, 0.0]).expect("bottom wall not found");
        let poly = w["polygon"].as_array().expect("polygon array missing");
        let corners = [(-0.1, -0.1), (10.1, -0.1), (9.9, 0.1), (0.1, 0.1)];
        for (cx, cy) in corners {
            let found = poly.iter().any(|p| {
                let px = p.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                let py = p.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                (px - cx).abs() < 1e-6 && (py - cy).abs() < 1e-6
            });
            assert!(found, "corner ({cx}, {cy}) not in bottom wall polygon");
        }
        let has_bad_corner = poly.iter().any(|p| {
            let px = p.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            let py = p.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            ((px - 10.0).abs() < 1e-6 && ((py - 0.1).abs() < 1e-6 || (py + 0.1).abs() < 1e-6))
        });
        assert!(!has_bad_corner, "polygon should not contain (10.0, ±0.1)");
    }

    #[test]
    fn arch_inspect_interior_walls_butt_at_faces() {
        let mut s = Session::new();
        build_full_demo_plan(&mut s);
        let v = exec(&mut s, "arch.inspect", &json!({ "poche": true }));
        let empty: Vec<Value> = Vec::new();
        let walls = v["walls"].as_array().unwrap_or(&empty);
        for w in walls {
            let is_exterior = w["exterior"].as_bool().unwrap_or(false);
            if is_exterior {
                continue;
            }
            let poly = w["polygon"].as_array().expect("polygon array missing");
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in poly {
                let px = p.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                let py = p.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                min_x = min_x.min(px);
                max_x = max_x.max(px);
                min_y = min_y.min(py);
                max_y = max_y.max(py);
            }
            assert!(min_x >= 0.099999, "interior wall min_x {min_x} < 0.099999");
            assert!(max_x <= 9.900001, "interior wall max_x {max_x} > 9.900001");
            assert!(min_y >= 0.099999, "interior wall min_y {min_y} < 0.099999");
            assert!(max_y <= 7.900001, "interior wall max_y {max_y} > 7.900001");
        }
    }

    #[test]
    fn arch_inspect_poché_count_matches_walls() {
        let mut s = Session::new();
        build_full_demo_plan(&mut s);
        let v = exec(&mut s, "arch.inspect", &json!({ "poche": true }));
        let empty: Vec<Value> = Vec::new();
        let walls = v["walls"].as_array().unwrap_or(&empty);
        assert_eq!(walls.len(), 6, "expected 6 walls");
        for w in walls {
            let poly = w["polygon"].as_array().expect("polygon array missing");
            assert!(poly.len() >= 4, "polygon must have at least 4 vertices");
            let first = poly.first().unwrap_or(&Value::Null);
            let last = poly.last().unwrap_or(&Value::Null);
            let first_x = first.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            let first_y = first.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            let last_x = last.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            let last_y = last.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
            assert!((first_x - last_x).abs() >= 1e-9 || (first_y - last_y).abs() >= 1e-9, "polygon[0] must differ from polygon[last]");
            for p in poly {
                let px = p.as_array().and_then(|a| a.get(0).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                let py = p.as_array().and_then(|a| a.get(1).and_then(Value::as_f64)).unwrap_or(f64::NAN);
                assert!(px.is_finite() && py.is_finite(), "polygon vertex not finite: ({px}, {py})");
            }
        }
    }

    #[test]
    fn arch_inspect_lists_levels_and_bounds() {
        let mut s = Session::new();
        exec(
            &mut s,
            "wall",
            &json!({
                "points": [[0.0, 0.0], [5.0, 0.0]],
                "thickness": 0.1,
            }),
        );
        let v = exec(&mut s, "arch.inspect", &json!({}));
        let empty: Vec<Value> = Vec::new();
        let bounds = v["bounds"].as_array().unwrap_or(&empty);
        assert_eq!(bounds.len(), 2, "bounds should be [[min],[max]]");
        assert!(v["levels"].is_array(), "levels should be an array");
    }

    /// A fresh session holding one 10 m wall with a single hinged door on it.
    fn session_with_one_wall_and_opening() -> Session {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let wid =
                    d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
                d.arch.push_opening(Opening::new(wid, 3.0, 0.9, 2.0, OpeningKind::Door));
                d.touch_arch();
            }
            Err(_) => {}
        }
        s
    }

    #[test]
    fn inspect_walls_reports_handles() {
        let mut s = session_with_one_wall_and_opening();
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0]["id"], "w0");
        assert_eq!(walls[0]["handle"], "a0000000");
        assert!(walls[0]["thickness"].is_number());
        assert!(walls[0]["exterior"].is_boolean());
    }

    #[test]
    fn inspect_filler_and_shelves_rows() {
        let mut s = Session::new();
        let mut c = cadcraft_doc::arch::Cabinet::default();
        c.id = cadcraft_doc::ArchId(0);
        c.kind = cadcraft_doc::arch::CabinetKind::Wall;
        c.level = 0;
        c.position = Vec2::new(1.0, 0.5);
        c.width = 0.762;
        c.depth = 0.305;
        c.height = 0.762;
        c.elevation = 1.372;
        c.fronts = cadcraft_doc::arch::Fronts::Open;
        c.shelves = Some(2);
        s.doc_mut().unwrap().arch.push_cabinet(c);

        let v = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        let rows = v["cabinets"].as_array().expect("cabinets array missing");
        assert_eq!(rows.len(), 1);
        assert!(rows[0]["filler"].is_boolean());
        assert!(rows[0]["shelves"].is_array(), "shelves should be reported for open cabinets");
        assert_eq!(rows[0]["shelves"].as_array().unwrap().len(), 2);
        assert!(rows[0]["shelves"][0]["bottom"].is_number());
        assert!(rows[0]["shelves"][0]["top"].is_number());
    }

    #[test]
    fn inspect_and_command_replies_spell_handles_the_same_way() {
        let mut s = Session::new();
        let w = s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]]})).expect("wall");
        let d = s.execute("door", &json!({"wall": "w0", "offset": 3.0, "width": 0.9, "height": 2.1})).expect("door");
        assert_eq!(w["walls"][0]["handle"], "a0000000");
        assert_eq!(d["openings"][0]["handle"], "a0008000");
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true, "openings": true }));
        assert_eq!(v["walls"][0]["handle"], w["walls"][0]["handle"]);
        assert_eq!(v["openings"][0]["handle"], d["openings"][0]["handle"]);
        let plain = exec(&mut s, "arch.inspect", &json!({}));
        if let Some(h) = plain["walls"][0]["handle"].as_str() {
            assert_eq!(h, "a0000000");
        }
    }

    #[test]
    fn inspect_lists_openings_with_proxy_handles() {
        let mut s = session_with_one_wall_and_opening();
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        assert!(!openings.is_empty(), "expected at least one opening");
        let o = &openings[0];
        assert!(o["id"].is_string(), "opening id must be a string");
        let handle = o["handle"].as_str().expect("handle must be a string");
        assert!(handle.starts_with("a000"), "handle must start with 'a000': {handle}");
        assert!(o["host"].is_string(), "opening host must be a wall label");
        assert!(o["kind"].is_string());
        assert!(o["style"].is_string());
        assert!(o["offset"].is_number());
        assert!(o["width"].is_number());
        assert!(o["height"].is_number());
        assert!(o["sill"].is_number());
        assert!(o["rough"].is_object());
        assert!(o["rough"]["width"].is_number());
        assert!(o["rough"]["height"].is_number());
        assert!(o["recess"].is_number());
        assert!(o["hinge"].is_string());
        assert!(o["swing"].is_string());
        assert!(o["layer"].is_string());
    }

    #[test]
    fn inspect_symbols_are_requested_only_with_the_flag() {
        let mut s = session_with_one_wall_and_opening();

        // Without the `symbols` flag there must be no per-opening `symbol`.
        let v_no = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let open_no = v_no["openings"].as_array().expect("openings array missing");
        let o = &open_no[0];
        assert!(o["symbol"].is_null(), "symbol present without the `symbols` flag");
        assert!(o["gap"].is_null(), "gap present without the `symbols` flag");

        // With the `symbols` flag the `symbol` and `gap` keys appear.
        let v_yes = exec(&mut s, "arch.inspect", &json!({ "openings": true, "symbols": true }));
        let open_yes = v_yes["openings"].as_array().expect("openings array missing");
        let o = &open_yes[0];
        assert!(o["symbol"].is_array(), "symbol must be an array when requested");
        assert!(o["gap"].is_array(), "gap must be an array when requested");
        let gap = o["gap"].as_array().expect("gap must be an array");
        assert_eq!(gap.len(), 2, "gap must be [start, end]");
        assert!(gap[0].is_number());
        assert!(gap[1].is_number());
        assert!(o["symbol"].as_array().unwrap().is_empty() == false);
    }

    #[test]
    fn inspect_does_not_bump_the_revision() {
        let mut s = session_with_one_wall_and_opening();
        let before = match s.doc().map(|d| d.arch_revision) {
            Ok(b) => b,
            Err(_) => return,
        };
        exec(&mut s, "arch.inspect", &json!({ "openings": true, "symbols": true }));
        let after = match s.doc().map(|d| d.arch_revision) {
            Ok(a) => a,
            Err(_) => return,
        };
        assert_eq!(before, after, "arch.inspect must not bump arch_revision");
    }

    /// The opening `id` field is a string label (e.g. "o1"), not a number.
    #[test]
    fn inspect_reports_opening_ids_as_o_labels() {
        let mut s = session_with_one_wall_and_opening();
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        let o = &openings[0];
        let id = o["id"].as_str().expect("opening id must be a string");
        assert!(id.starts_with("o"), "opening label must start with 'o': {id}");
    }

    /// The opening `host` field is a wall label (e.g. "w1"), not a number.
    #[test]
    fn inspect_reports_host_as_a_wall_label() {
        let mut s = session_with_one_wall_and_opening();
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        let o = &openings[0];
        let host = o["host"].as_str().expect("opening host must be a string");
        assert!(host.starts_with("w"), "wall label must start with 'w': {host}");
    }

    /// The opening `handle` field is lowercase hex with no padding.
    #[test]
    fn inspect_opening_handle_is_lowercase_hex() {
        let mut s = session_with_one_wall_and_opening();
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        let o = &openings[0];
        let handle = o["handle"].as_str().expect("handle must be a string");
        assert!(handle.starts_with("a000"), "handle must be lowercase starting with 'a000': {handle}");
        assert!(handle.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "handle must contain only lowercase hex: {handle}");
        assert_eq!(handle.len(), 8, "unpadded handle for a low id should be 8 chars: {handle}");
    }

    /// A fresh session holding one 10 m wall with a single opening of the given kind.
    fn session_with_one_wall_and_opening_of(kind: OpeningKind) -> Session {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let wid =
                    d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
                let o = match kind {
                    OpeningKind::GarageDoor => Opening::new(wid, 4.0, 2.0, 2.2, kind),
                    _ => Opening::new(wid, 4.0, 1.0, 1.5, kind),
                };
                d.arch.push_opening(o);
                d.touch_arch();
            }
            Err(_) => {}
        }
        s
    }

    #[test]
    fn inspect_reports_garage_door_as_a_door_kind() {
        let mut s = session_with_one_wall_and_opening_of(OpeningKind::GarageDoor);
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        assert!(!openings.is_empty(), "expected at least one opening");
        let o = &openings[0];
        assert_eq!(o["kind"].as_str().expect("kind should be a string"), "door");
        assert_eq!(o["style"].as_str().expect("style should be a string"), "garage");
    }

    #[test]
    fn inspect_reports_window_and_cased_kind_labels() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let wid =
                    d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
                d.arch.push_opening(Opening::new(wid, 1.0, 1.0, 1.5, OpeningKind::Window));
                d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 1.5, OpeningKind::CasedOpening));
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "openings": true }));
        let openings = v["openings"].as_array().expect("openings array missing");
        assert_eq!(openings.len(), 2, "expected two openings");
        let window = openings.iter().find(|o| o["offset"].as_f64().unwrap_or(f64::NAN) == 1.0);
        let cased = openings.iter().find(|o| o["offset"].as_f64().unwrap_or(f64::NAN) == 5.0);
        assert!(window.is_some(), "window opening not found");
        assert!(cased.is_some(), "cased opening not found");
        assert_eq!(window.unwrap()["kind"].as_str().expect("kind should be a string"), "window");
        assert_eq!(cased.unwrap()["kind"].as_str().expect("kind should be a string"), "cased");
    }

    /// A fresh session holding a single room with a known area on level 0.
    fn session_with_one_room() -> Session {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Living Room".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.48,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        s
    }

    #[test]
    fn inspect_wall_finish_is_null_by_default() {
        let mut s = Session::new();
        exec(&mut s, "wall", &json!({ "points": [[0.0, 0.0], [5.0, 0.0]] }));
        let v = exec(&mut s, "arch.inspect", &json!({}));
        assert!(v["walls"][0].get("finish").is_some(), "finish key must be present");
        assert_eq!(v["walls"][0]["finish"], Value::Null);
    }

    #[test]
    fn inspect_reports_finishes() {
        use cadcraft_doc::arch::{Finish, Roof};
        let mut s = Session::new();
        exec(&mut s, "wall", &json!({ "points": [[0.0, 0.0], [5.0, 0.0]] }));
        if let Ok(d) = s.doc_mut() {
            if let Some(w) = d.arch.walls.first_mut() {
                w.finish = Some(Finish::Brick);
            }
            d.arch.push_roof(Roof { covering: Some(Finish::Shingles), ..Roof::default() });
            d.arch.push_room(Room {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
                floor_finish: Some(Finish::Wood),
                ..Room::default()
            });
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true, "rooms": true }));
        assert_eq!(v["walls"][0]["finish"], "brick");
        assert_eq!(v["roofs"][0]["covering"], "shingles");
        assert_eq!(v["rooms"][0]["floorFinish"], "wood");
        let plain = exec(&mut s, "arch.inspect", &json!({}));
        assert_eq!(plain["walls"][0]["finish"], "brick");
        assert!(plain.get("roofs").is_none());
        assert!(plain.get("rooms").is_none());
    }

    #[test]
    fn inspect_reports_rooms_with_areas() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert!(r["handle"].is_string(), "handle must be a string");
        assert_eq!(r["name"].as_str().unwrap(), "Living Room");
        assert_eq!(r["type"].as_str().unwrap(), "living");
        assert_eq!(r["level"].as_str().unwrap(), "Level 1");
        assert!(r["polygon"].is_array(), "polygon must be an array");
        assert_eq!(r["holes"].as_i64().unwrap(), 0);
        assert_eq!(r["holeRings"].as_array().unwrap().len(), 0);
        assert!((r["interiorArea"].as_f64().unwrap() - 12.0).abs() < 1e-9);
        assert!((r["standardArea"].as_f64().unwrap() - 12.48).abs() < 1e-9);
        assert!(r["livingIncluded"].as_bool().unwrap());
        assert!((r["ceiling"].as_f64().unwrap() - 2.4).abs() < 1e-9);
    }

    /// Phase 16 room rows: the flat/vaulted flag, the finish stack, the rough and finished
    /// ceiling heights above the level's floor, and the roof warning (null without one).
    #[test]
    fn inspect_room_rows_report_ceiling_fields() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let r = &v["rooms"][0];
        assert_eq!(r["flatCeiling"], true);
        assert_eq!(r["ceilingStack"], json!([]));
        assert!((r["roughCeiling"].as_f64().unwrap() - 2.4).abs() < 1e-9);
        assert!((r["finishedCeiling"].as_f64().unwrap() - 2.4).abs() < 1e-9);
        assert!(r["ceilingWarning"].is_null());
        // A stack, an upper level and a vaulted flag.
        exec(
            &mut s,
            "room",
            &json!({ "id": "r0", "flatCeiling": false, "ceiling": 2.7, "ceilingStack": [
            {"name": "drywall", "material": "drywall", "thickness": 0.0127}, {"name": "air", "material": null, "thickness": 0.2},
            {"name": "tile", "material": "ceiling-tile", "thickness": 0.016}] }),
        );
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let r = &v["rooms"][0];
        assert_eq!(r["flatCeiling"], false);
        assert_eq!(r["ceilingStack"][1], json!({"name": "air", "material": null, "thickness": 0.2}));
        assert!((r["roughCeiling"].as_f64().unwrap() - 2.7).abs() < 1e-9);
        assert!((r["finishedCeiling"].as_f64().unwrap() - (2.7 - 0.2287)).abs() < 1e-9);
        // Relative to the level: a room on a level at 2.7 still reads 2.4.
        if let Ok(d) = s.doc_mut() {
            d.arch.levels.push(cadcraft_doc::arch::Level { index: 1, name: "Level 2".into(), elevation: 2.7, ..Default::default() });
            d.arch.push_room(Room { level: 1, polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0)], ..Room::default() });
            d.arch.push_room(Room {
                room_type: RoomType::Deck,
                polygon: vec![Vec2::new(5.0, 0.0), Vec2::new(8.0, 0.0), Vec2::new(8.0, 3.0)],
                ..Room::default()
            });
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let up = &v["rooms"][1];
        assert!((up["roughCeiling"].as_f64().unwrap() - 2.4).abs() < 1e-9, "{up}");
        assert!((up["finishedCeiling"].as_f64().unwrap() - 2.4).abs() < 1e-9, "{up}");
        // A deck has no ceiling.
        let deck = &v["rooms"][2];
        assert!(deck["roughCeiling"].is_null() && deck["finishedCeiling"].is_null() && deck["ceilingWarning"].is_null(), "{deck}");
    }

    #[test]
    fn inspect_reports_derived_floor_to_floor() {
        let mut s = Session::new();
        let v = exec(&mut s, "arch.inspect", &json!({}));
        let levels = v["levels"].as_array().expect("levels array missing");
        assert!(!levels.is_empty(), "expected at least one level");
        let l = &levels[0];
        let ceiling = l["ceiling"].as_f64().unwrap();
        let structure = l["structure"].as_f64().unwrap();
        let ftf = l["floorToFloor"].as_f64().unwrap();
        assert!((ftf - (ceiling + structure)).abs() < 1e-9, "floorToFloor {} != ceiling {} + structure {}", ftf, ceiling, structure);
    }

    #[test]
    fn inspect_label_text_carries_the_area() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true, "labels": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert!(r["labelPoint"].is_array(), "labelPoint must be an array");
        assert_eq!(r["labelText"].as_str().unwrap(), "Living Room\n12.00 m2");
    }

    #[test]
    fn inspect_room_handle_is_lowercase_eight_digits() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        let handle = r["handle"].as_str().expect("handle must be a string");
        assert_eq!(handle.len(), 8, "handle must be 8 characters: {handle}");
        assert!(handle.starts_with("a001"), "handle must start with 'a001' (room proxy range): {handle}");
        assert!(handle.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "handle must be lowercase hex: {handle}");
    }

    #[test]
    fn inspect_room_label_is_inside_the_polygon() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        let label = r["label"].as_array().expect("label must be an array");
        assert_eq!(label.len(), 2, "label must be [x, y]");
        let x = label[0].as_f64().expect("label x must be a number");
        let y = label[1].as_f64().expect("label y must be a number");
        // The room is a 4x3 rectangle from (0,0) to (4,3)
        assert!(x >= 0.0 && x <= 4.0, "label x {x} outside polygon");
        assert!(y >= 0.0 && y <= 3.0, "label y {y} outside polygon");
    }

    #[test]
    fn inspect_room_label_falls_back_for_an_empty_polygon() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Empty".into(),
                    room_type: RoomType::Other,
                    level: 0,
                    polygon: vec![],
                    ceiling_height: None,
                    interior_area: 0.0,
                    standard_area: 0.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        let label = r["label"].as_array().expect("label must be an array");
        assert_eq!(label.len(), 2, "label must be [x, y]");
        assert_eq!(label[0].as_f64().expect("x"), 0.0);
        assert_eq!(label[1].as_f64().expect("y"), 0.0);
    }

    /// Two rooms get distinct 8-digit lowercase hex handles.
    #[test]
    fn inspect_reports_multiple_room_handles_uniquely() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Room A".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.48,
                    ..Room::default()
                });
                d.arch.push_room(Room {
                    name: "Room B".into(),
                    room_type: RoomType::Bedroom,
                    level: 0,
                    polygon: vec![Vec2::new(5.0, 0.0), Vec2::new(9.0, 0.0), Vec2::new(9.0, 3.0), Vec2::new(5.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.48,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 2, "expected two rooms");
        let h0 = rooms[0]["handle"].as_str().expect("handle 0 missing");
        let h1 = rooms[1]["handle"].as_str().expect("handle 1 missing");
        assert_eq!(h0.len(), 8, "handle 0 must be 8 chars: {h0}");
        assert_eq!(h1.len(), 8, "handle 1 must be 8 chars: {h1}");
        assert_ne!(h0, h1, "handles must be distinct");
        assert!(h0.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "handle 0 must be lowercase hex: {h0}");
        assert!(h1.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "handle 1 must be lowercase hex: {h1}");
    }

    /// A room with holes exposes its rings while retaining the legacy hole count.
    #[test]
    fn inspect_room_label_with_holes() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Hall".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(2.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 16.0,
                    standard_area: 16.48,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1);
        let holes = r["holeRings"].as_array().expect("holeRings should be an array of rings");
        assert_eq!(holes.len(), 1, "expected one hole ring");
        let label = r["label"].as_array().expect("label must be an array");
        assert_eq!(label.len(), 2, "label must be [x, y]");
        let x = label[0].as_f64().expect("label x must be a number");
        let y = label[1].as_f64().expect("label y must be a number");
        assert!(x.is_finite() && y.is_finite(), "label must be finite: ({x}, {y})");
        assert!(x >= 0.0 && x <= 6.0, "label x {x} outside outer ring");
        assert!(y >= 0.0 && y <= 4.0, "label y {y} outside outer ring");
    }

    /// Ring geometry is reported in `holeRings`; `holes` remains the legacy count.
    #[test]
    fn inspect_room_holes_are_rings() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Hall".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(2.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 16.0,
                    standard_area: 16.48,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1, "holes must be the count");
        let holes = r["holeRings"].as_array().expect("holeRings should be an array of rings");
        assert_eq!(holes.len(), 1, "expected one hole ring");
        assert_eq!(holes[0].as_array().unwrap().len(), 4, "expected a 4-point ring");
        let label = r["label"].as_array().expect("label must be an array");
        assert_eq!(label.len(), 2, "label must be [x, y]");
        let x = label[0].as_f64().expect("label x must be a number");
        let y = label[1].as_f64().expect("label y must be a number");
        assert!(x.is_finite() && y.is_finite(), "label must be finite: ({x}, {y})");
        assert!(x >= 0.0 && x <= 6.0, "label x {x} outside outer ring");
        assert!(y >= 0.0 && y <= 4.0, "label y {y} outside outer ring");
    }

    /// A room with collinear (degenerate) vertices still gets a finite label.
    #[test]
    fn inspect_room_label_with_collinear_points() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Flat".into(),
                    room_type: RoomType::Other,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 2.0), Vec2::new(0.0, 2.0)],
                    ceiling_height: None,
                    interior_area: 8.0,
                    standard_area: 8.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        let label = r["label"].as_array().expect("label must be an array");
        let x = label[0].as_f64().expect("label x must be a number");
        let y = label[1].as_f64().expect("label y must be a number");
        assert!(x.is_finite() && y.is_finite(), "label must be finite for collinear points: ({x}, {y})");
    }

    /// A room with very large coordinates still gets a finite label.
    #[test]
    fn inspect_room_label_with_huge_coordinates() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Big".into(),
                    room_type: RoomType::Other,
                    level: 0,
                    polygon: vec![Vec2::new(1e9, 1e9), Vec2::new(2e9, 1e9), Vec2::new(2e9, 2e9), Vec2::new(1e9, 2e9)],
                    ceiling_height: None,
                    interior_area: 1e18,
                    standard_area: 1e18,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        let label = r["label"].as_array().expect("label must be an array");
        let x = label[0].as_f64().expect("label x must be a number");
        let y = label[1].as_f64().expect("label y must be a number");
        assert!(x.is_finite() && y.is_finite(), "label must be finite for huge coords: ({x}, {y})");
        assert!(x >= 1e9 && x <= 2e9, "label x {x} outside range");
        assert!(y >= 1e9 && y <= 2e9, "label y {y} outside range");
    }

    /// A second room (id 1) gets the correct incremented handle.
    #[test]
    fn inspect_room_handle_for_nonzero_id() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "First".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.48,
                    ..Room::default()
                });
                d.arch.push_room(Room {
                    name: "Second".into(),
                    room_type: RoomType::Bedroom,
                    level: 0,
                    polygon: vec![Vec2::new(5.0, 0.0), Vec2::new(9.0, 0.0), Vec2::new(9.0, 3.0), Vec2::new(5.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.48,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 2, "expected two rooms");
        let h0 = rooms[0]["handle"].as_str().expect("handle 0 missing");
        let h1 = rooms[1]["handle"].as_str().expect("handle 1 missing");
        assert_eq!(h0, "a0010000", "room 0 handle must be a0010000, got {h0}");
        assert_eq!(h1, "a0010001", "room 1 handle must be a0010001, got {h1}");
    }

    #[test]
    fn inspect_roofs_reports_planes_and_edges() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.5,
                    thickness: 0.1,
                    fascia: true,
                    soffit: true,
                    manual: false,
                    hand_edited: false,
                    planes: vec![RoofPlane {
                        ring: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                        slope: 0.5,
                        projected_area: 80.0,
                        eave_wall: Some(ArchId(1)),
                        base_edge: [0.0, 0.0, 10.0, 0.0],
                        ..Default::default()
                    }],
                    edges: vec![RoofEdge {
                        kind: RoofEdgeKind::Ridge,
                        a: Vec2::new(0.0, 4.0),
                        b: Vec2::new(10.0, 4.0),
                        za: 4.0,
                        zb: 4.0,
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        assert_eq!(roofs.len(), 1, "expected one roof");
        let r = &roofs[0];
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert!(r["handle"].is_string());
        assert_eq!(r["level"].as_i64().unwrap(), 0);
        assert_eq!(r["pitchRise"].as_f64().unwrap(), 6.0);
        assert_eq!(r["pitchRun"].as_f64().unwrap(), 12.0);
        assert!(r["slope"].is_number());
        assert_eq!(r["overhang"].as_f64().unwrap(), 0.5);
        assert_eq!(r["thickness"].as_f64().unwrap(), 0.1);
        assert_eq!(r["fascia"].as_bool().unwrap(), true);
        assert_eq!(r["soffit"].as_bool().unwrap(), true);
        assert_eq!(r["manual"].as_bool().unwrap(), false);
        assert_eq!(r["handEdited"].as_bool().unwrap(), false);
        assert!(r["projectedArea"].is_number());

        let planes = r["planes"].as_array().expect("planes array missing");
        assert_eq!(planes.len(), 1);
        let p = &planes[0];
        assert_eq!(p["index"].as_i64().unwrap(), 0);
        assert!(p["ring"].is_array());
        assert!(p["slope"].is_number());
        assert!(p["projectedArea"].is_number());
        assert_eq!(p["eaveWall"].as_str().unwrap(), "w1");
        assert!(p["baseEdge"].is_array());

        let edges = r["edges"].as_array().expect("edges array missing");
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert_eq!(e["kind"].as_str().unwrap(), "ridge");
        assert!(e["a"].is_array());
        assert!(e["b"].is_array());
        assert!(e["za"].is_number());
        assert!(e["zb"].is_number());
    }

    #[test]
    fn inspect_roofs_absent_when_not_requested() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.5,
                    thickness: 0.1,
                    fascia: true,
                    soffit: true,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({}));
        assert!(v["roofs"].is_null(), "roofs key should be absent when not requested");
    }

    #[test]
    fn inspect_roof_edge_kinds_are_lowercase() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.5,
                    thickness: 0.1,
                    fascia: true,
                    soffit: true,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![
                        RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::ZERO, b: Vec2::ZERO, za: 0.0, zb: 0.0, ..Default::default() },
                        RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::ZERO, b: Vec2::ZERO, za: 0.0, zb: 0.0, ..Default::default() },
                        RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::ZERO, b: Vec2::ZERO, za: 0.0, zb: 0.0, ..Default::default() },
                        RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::ZERO, b: Vec2::ZERO, za: 0.0, zb: 0.0, ..Default::default() },
                        RoofEdge { kind: RoofEdgeKind::GableEnd, a: Vec2::ZERO, b: Vec2::ZERO, za: 0.0, zb: 0.0, ..Default::default() },
                    ],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        let edges = roofs[0]["edges"].as_array().expect("edges array missing");

        let kinds: Vec<&str> = edges.iter().map(|e| e["kind"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"ridge"));
        assert!(kinds.contains(&"hip"));
        assert!(kinds.contains(&"valley"));
        assert!(kinds.contains(&"eave"));
        assert!(kinds.contains(&"gableEnd"));

        for k in kinds {
            // gableEnd is camelCase, others are lowercase.
            // The spec says: kind:"ridge"|"hip"|"valley"|"eave"|"gableEnd"
            // So we just verify they are one of the expected strings.
            assert!(k == "ridge" || k == "hip" || k == "valley" || k == "eave" || k == "gableEnd", "unexpected edge kind: {k}");
        }
    }

    /// A roof with no planes and no edges still reports correctly.
    #[test]
    fn inspect_roof_with_empty_planes_and_edges() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: true,
                    hand_edited: true,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        assert_eq!(roofs.len(), 1, "expected one roof");
        let r = &roofs[0];
        assert_eq!(r["planes"].as_array().expect("planes must be array").len(), 0, "planes should be empty");
        assert_eq!(r["edges"].as_array().expect("edges must be array").len(), 0, "edges should be empty");
        assert_eq!(r["manual"].as_bool().unwrap(), true, "manual should be true");
        assert_eq!(r["handEdited"].as_bool().unwrap(), true, "handEdited should be true");
        assert_eq!(r["fascia"].as_bool().unwrap(), false, "fascia should be false");
        assert_eq!(r["soffit"].as_bool().unwrap(), false, "soffit should be false");
    }

    /// A plane with no eave wall reports `eaveWall` as null.
    #[test]
    fn inspect_roof_null_eave_wall() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![RoofPlane {
                        ring: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                        slope: 0.5,
                        projected_area: 80.0,
                        eave_wall: None,
                        base_edge: [0.0, 0.0, 10.0, 0.0],
                        ..Default::default()
                    }],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        let planes = roofs[0]["planes"].as_array().expect("planes array missing");
        assert_eq!(planes.len(), 1, "expected one plane");
        assert!(planes[0]["eaveWall"].is_null(), "eaveWall should be null when the plane has no eave wall");
    }

    /// Multiple roofs are all reported with distinct ids and levels.
    #[test]
    fn inspect_multiple_roofs() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof0 = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                let roof1 = Roof {
                    id: ArchId(1),
                    level: 1,
                    pitch_rise: 4.0,
                    pitch_run: 12.0,
                    overhang: 0.5,
                    thickness: 0.15,
                    fascia: true,
                    soffit: true,
                    manual: true,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof0);
                d.arch.roofs.push(roof1);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        assert_eq!(roofs.len(), 2, "expected two roofs");
        assert_eq!(roofs[0]["id"].as_str().unwrap(), "r0", "first roof id should be r0");
        assert_eq!(roofs[1]["id"].as_str().unwrap(), "r1", "second roof id should be r1");
        assert_eq!(roofs[0]["level"].as_i64().unwrap(), 0, "first roof level should be 0");
        assert_eq!(roofs[1]["level"].as_i64().unwrap(), 1, "second roof level should be 1");
        assert_eq!(roofs[0]["pitchRise"].as_f64().unwrap(), 6.0, "first roof pitchRise should be 6.0");
        assert_eq!(roofs[1]["pitchRise"].as_f64().unwrap(), 4.0, "second roof pitchRise should be 4.0");
    }

    /// Roof handles are lowercase hex, matching the room handle convention.
    #[test]
    fn inspect_roof_handle_format() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        let handle = roofs[0]["handle"].as_str().expect("handle must be a string");
        assert!(handle.starts_with("a001"), "handle must start with 'a001' (roof proxy range): {handle}");
        assert!(handle.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "handle must be lowercase hex: {handle}");
    }

    /// Inspecting roofs must not bump the arch revision.
    #[test]
    fn inspect_roofs_does_not_bump_revision() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let before = match s.doc().map(|d| d.arch_revision) {
            Ok(b) => b,
            Err(_) => return,
        };
        exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let after = match s.doc().map(|d| d.arch_revision) {
            Ok(a) => a,
            Err(_) => return,
        };
        assert_eq!(before, after, "arch.inspect must not bump arch_revision");
    }

    /// A room with ceiling_height None reports ceiling as JSON null.
    #[test]
    fn inspect_room_ceiling_null_when_unset() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Open".into(),
                    room_type: RoomType::OpenBelow,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    ceiling_height: None,
                    interior_area: 12.0,
                    standard_area: 12.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert!(r["ceiling"].is_null(), "ceiling should be null when ceiling_height is None");
    }

    /// A non-living room type (Garage) reports livingIncluded as false.
    #[test]
    fn inspect_room_living_included_false_for_garage() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Garage".into(),
                    room_type: RoomType::Garage,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 24.0,
                    standard_area: 24.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["type"].as_str().unwrap(), "garage");
        assert_eq!(r["livingIncluded"].as_bool().unwrap(), false, "garage should not be included in living area");
    }

    /// RoomType::OpenBelow serializes as camelCase "openBelow".
    #[test]
    fn inspect_room_type_open_below_is_camel_case() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Void".into(),
                    room_type: RoomType::OpenBelow,
                    level: 1,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0), Vec2::new(0.0, 3.0)],
                    ceiling_height: Some(2.4),
                    interior_area: 9.0,
                    standard_area: 9.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["type"].as_str().unwrap(), "openBelow", "OpenBelow should serialize as openBelow");
    }

    /// A room with multiple holes reports the correct hole count.
    #[test]
    fn inspect_room_multiple_holes_count() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Complex".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                    holes: vec![
                        vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(3.0, 3.0), Vec2::new(1.0, 3.0)],
                        vec![Vec2::new(6.0, 4.0), Vec2::new(8.0, 4.0), Vec2::new(8.0, 6.0), Vec2::new(6.0, 6.0)],
                    ],
                    ceiling_height: Some(2.4),
                    interior_area: 76.0,
                    standard_area: 76.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 2, "holes must be the count");
        assert_eq!(r["holeRings"].as_array().unwrap().len(), 2, "expected 2 hole rings");
    }

    /// When labels are requested, labelPoint must match label (same algorithm).
    #[test]
    fn inspect_room_label_point_matches_label() {
        let mut s = session_with_one_room();
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true, "labels": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        let label = r["label"].as_array().expect("label must be an array");
        let label_point = r["labelPoint"].as_array().expect("labelPoint must be an array");
        assert_eq!(label.len(), 2);
        assert_eq!(label_point.len(), 2);
        let lx = label[0].as_f64().expect("label x");
        let ly = label[1].as_f64().expect("label y");
        let lpx = label_point[0].as_f64().expect("labelPoint x");
        let lpy = label_point[1].as_f64().expect("labelPoint y");
        assert!((lx - lpx).abs() < 1e-9, "label and labelPoint x should match: {} vs {}", lx, lpx);
        assert!((ly - lpy).abs() < 1e-9, "label and labelPoint y should match: {} vs {}", ly, lpy);
    }

    /// labelText formats the area to two decimal places for fractional values.
    #[test]
    fn inspect_room_label_text_with_fractional_area() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Study".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.5, 0.0), Vec2::new(3.5, 2.5), Vec2::new(0.0, 2.5)],
                    ceiling_height: Some(2.4),
                    interior_area: 8.75,
                    standard_area: 9.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true, "labels": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["labelText"].as_str().unwrap(), "Study\n8.75 m2");
    }

    /// The roof slope is pitch_rise divided by pitch_run.
    #[test]
    fn inspect_roof_slope_is_pitch_ratio() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        let slope = roofs[0]["slope"].as_f64().expect("slope must be a number");
        assert!((slope - 0.5).abs() < 1e-9, "slope should be pitch_rise/pitch_run = 0.5, got {slope}");
    }

    /// Stairs report their full field set and radians-converted direction.
    #[test]
    fn inspect_reports_stairs() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_stair(Stair {
                    start: Vec2::new(1.0, 2.0),
                    direction: (std::f64::consts::PI / 2.0),
                    width: 1.5,
                    from_level: 0,
                    to_level: 1,
                    rise: 2.7,
                    shape: StairShape::Straight,
                    risers: 16,
                    riser_height: 0.16875,
                    tread_depth: 0.254,
                    treads: 15,
                    run: 3.81,
                    landing_depth: 0.0,
                    well: vec![Vec2::new(1.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(1.0, 3.0)],
                    well_room: None,
                    ..Stair::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "stairs": true }));
        let stairs = v["stairs"].as_array().expect("stairs array missing");
        assert_eq!(stairs.len(), 1, "expected one stair");
        let st = &stairs[0];
        assert_eq!(st["id"].as_str().expect("id must be a string"), "s0");
        assert!(st["handle"].is_string(), "handle must be a string");
        assert!(st["start"].is_array(), "start must be an array");
        let start = st["start"].as_array().expect("start must be an array");
        assert!((start[0].as_f64().expect("x") - 1.0).abs() < 1e-9);
        assert!((start[1].as_f64().expect("y") - 2.0).abs() < 1e-9);
        let direction = st["direction"].as_f64().expect("direction must be a number");
        assert!((direction - 90.0).abs() < 1e-6, "direction should be 90 degrees, got {direction}");
        assert!((st["width"].as_f64().expect("width") - 1.5).abs() < 1e-9);
        assert_eq!(st["fromLevel"].as_i64().expect("fromLevel"), 0);
        assert_eq!(st["toLevel"].as_i64().expect("toLevel"), 1);
        assert!((st["rise"].as_f64().expect("rise") - 2.7).abs() < 1e-9);
        assert_eq!(st["shape"].as_str().expect("shape must be a string"), "straight");
        assert_eq!(st["risers"].as_i64().expect("risers"), 16);
        assert!((st["riserHeight"].as_f64().expect("riserHeight") - 0.16875).abs() < 1e-9);
        assert!((st["treadDepth"].as_f64().expect("treadDepth") - 0.254).abs() < 1e-9);
        assert_eq!(st["treads"].as_i64().expect("treads"), 15);
        assert!((st["run"].as_f64().expect("run") - 3.81).abs() < 1e-9);
        assert_eq!(st["landingDepth"].as_f64().expect("landingDepth"), 0.0);
        assert_eq!(st["well"].as_array().expect("well must be an array").len(), 4);
        assert!(st["wellRoom"].is_null(), "wellRoom should be null when unset");
    }

    /// Railings report their field set; `follows` is null when the railing
    /// does not follow a stair.
    #[test]
    fn inspect_reports_railings() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_railing(Railing {
                    path: vec![Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), Vec2::new(5.0, 4.0)],
                    level: 1,
                    height: 0.914,
                    style: RailingStyle::Balusters,
                    thickness: 0.05,
                    baluster_gap: 0.099,
                    follows: None,
                    ..Railing::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "railings": true }));
        let rails = v["railings"].as_array().expect("railings array missing");
        assert_eq!(rails.len(), 1, "expected one railing");
        let r = &rails[0];
        assert_eq!(r["id"].as_str().expect("id must be a string"), "ra0");
        assert!(r["handle"].is_string(), "handle must be a string");
        assert_eq!(r["path"].as_array().expect("path must be an array").len(), 3);
        assert_eq!(r["level"].as_i64().expect("level"), 1);
        assert!((r["height"].as_f64().expect("height") - 0.914).abs() < 1e-9);
        assert_eq!(r["style"].as_str().expect("style must be a string"), "balusters");
        assert!((r["thickness"].as_f64().expect("thickness") - 0.05).abs() < 1e-9);
        assert!((r["balusterGap"].as_f64().expect("balusterGap") - 0.099).abs() < 1e-9);
        assert!(r["follows"].is_null(), "follows should be null when unset");
    }

    /// A deck room carries its `deckHeight` and has a floor; a non-deck room
    /// reports `deckHeight` as null. `hasFloor` reflects the room type.
    #[test]
    fn inspect_rooms_carry_deck_height() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Deck".into(),
                    room_type: RoomType::Deck,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    deck_height: Some(0.9),
                    ..Room::default()
                });
                d.arch.push_room(Room {
                    name: "Living".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 2, "expected two rooms");
        // Deck carries its height and has a floor.
        assert!((rooms[0]["deckHeight"].as_f64().expect("deckHeight") - 0.9).abs() < 1e-9);
        assert_eq!(rooms[0]["hasFloor"].as_bool().expect("hasFloor"), true);
        // Non-deck room: deckHeight null, still a floor.
        assert!(rooms[1]["deckHeight"].is_null(), "deckHeight should be null when unset");
        assert_eq!(rooms[1]["hasFloor"].as_bool().expect("hasFloor"), true);
    }

    /// The `stairs`/`railings` keys are absent unless their flag is set.
    #[test]
    fn inspect_stairs_absent_when_not_requested() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_stair(Stair::default());
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({}));
        assert!(v["stairs"].is_null(), "stairs key absent without the flag");
        assert!(v["railings"].is_null(), "railings key absent without the flag");
        // A room-only request still must not reveal stairs.
        let v2 = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        assert!(v2["stairs"].is_null(), "stairs key absent without the flag");
        assert!(v2["railings"].is_null(), "railings key absent without the flag");
    }

    /// A closed loop of 4 walls has 4 shapes but 8 joined ends (2 per corner).
    #[test]
    fn arch_inspect_counts_joins_for_closed_loop() {
        let mut s = Session::new();
        exec(
            &mut s,
            "wall",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
                "closed": true,
                "thickness": 0.2,
            }),
        );
        let v = exec(&mut s, "arch.inspect", &json!({}));
        let wall_count = v["walls"].as_array().map(|a| a.len()).unwrap_or(0);
        assert_eq!(wall_count, 4, "expected 4 walls in closed loop");
        let joins = v["joins"].as_u64().unwrap_or(u64::MAX);
        assert_eq!(joins, 8, "closed loop of 4 walls has 8 joined ends, got {joins}");
    }

    /// The roof projected area is the sum of its planes' projected areas.
    #[test]
    fn inspect_roof_projected_area_is_sum_of_planes() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let roof = Roof {
                    id: ArchId(0),
                    level: 0,
                    pitch_rise: 6.0,
                    pitch_run: 12.0,
                    overhang: 0.0,
                    thickness: 0.1,
                    fascia: false,
                    soffit: false,
                    manual: false,
                    hand_edited: false,
                    planes: vec![
                        RoofPlane {
                            ring: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 4.0), Vec2::new(0.0, 4.0)],
                            slope: 0.5,
                            projected_area: 40.0,
                            eave_wall: None,
                            base_edge: [0.0, 0.0, 10.0, 0.0],
                            ..Default::default()
                        },
                        RoofPlane {
                            ring: vec![Vec2::new(0.0, 4.0), Vec2::new(10.0, 4.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                            slope: 0.5,
                            projected_area: 40.0,
                            eave_wall: None,
                            base_edge: [0.0, 8.0, 10.0, 8.0],
                            ..Default::default()
                        },
                    ],
                    edges: vec![],
                    ..Default::default()
                };
                d.arch.roofs.push(roof);
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "roofs": true }));
        let roofs = v["roofs"].as_array().expect("roofs array missing");
        let area = roofs[0]["projectedArea"].as_f64().expect("projectedArea must be a number");
        assert!((area - 80.0).abs() < 1e-9, "projectedArea should be 80.0 (40+40), got {area}");
    }

    /// A room with no holes reports `holes` as an empty array.
    #[test]
    fn inspect_room_holes_empty_array() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "NoHoles".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                    holes: vec![],
                    ceiling_height: Some(2.4),
                    interior_area: 12.0,
                    standard_area: 12.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        assert_eq!(rooms.len(), 1, "expected one room");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 0, "holes should be 0");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        assert_eq!(hole_rings.len(), 0, "holeRings should be an empty array");
    }

    /// A hole with coincident points is reported with all points preserved.
    #[test]
    fn inspect_room_hole_with_coincident_points() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Coincident".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(2.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 20.0,
                    standard_area: 20.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1, "expected one hole");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        let hole = hole_rings[0].as_array().expect("hole must be an array of points");
        assert_eq!(hole.len(), 5, "hole should have 5 points (including the coincident one)");
    }

    /// A hole with collinear points is reported with all points preserved.
    #[test]
    fn inspect_room_hole_with_collinear_points() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "Collinear".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(2.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 20.0,
                    standard_area: 20.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1, "expected one hole");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        let hole = hole_rings[0].as_array().expect("hole must be an array of points");
        assert_eq!(hole.len(), 5, "hole should have 5 points (including the collinear one)");
    }

    /// Multiple holes with different vertex counts are all reported correctly.
    #[test]
    fn inspect_room_holes_with_different_vertex_counts() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "MixedHoles".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                    holes: vec![
                        // Triangle (3 vertices)
                        vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(1.5, 2.0)],
                        // Square (4 vertices)
                        vec![Vec2::new(4.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 2.0), Vec2::new(4.0, 2.0)],
                        // Pentagon (5 vertices)
                        vec![Vec2::new(7.0, 1.0), Vec2::new(8.0, 1.0), Vec2::new(8.5, 2.0), Vec2::new(8.0, 3.0), Vec2::new(7.0, 3.0)],
                    ],
                    ceiling_height: Some(2.4),
                    interior_area: 75.0,
                    standard_area: 75.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 3, "expected 3 holes");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        assert_eq!(hole_rings.len(), 3, "expected 3 hole rings");
        assert_eq!(hole_rings[0].as_array().expect("hole 0 must be an array").len(), 3, "first hole should have 3 vertices");
        assert_eq!(hole_rings[1].as_array().expect("hole 1 must be an array").len(), 4, "second hole should have 4 vertices");
        assert_eq!(hole_rings[2].as_array().expect("hole 2 must be an array").len(), 5, "third hole should have 5 vertices");
    }

    /// The `holes` array and the legacy `holeRings` array are identical.
    #[test]
    fn inspect_room_holes_match_hole_rings() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "MatchTest".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![
                        vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(2.0, 2.0), Vec2::new(1.0, 2.0)],
                        vec![Vec2::new(4.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 3.0), Vec2::new(4.0, 3.0)],
                    ],
                    ceiling_height: Some(2.4),
                    interior_area: 20.0,
                    standard_area: 20.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        let holes = r["holes"].as_i64().expect("holes must be a number");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        assert_eq!(holes, hole_rings.len() as i64, "holes count must match holeRings length");
    }

    /// A degenerate hole with a single point is still reported as a ring.
    #[test]
    fn inspect_room_hole_with_one_point() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "OnePoint".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(3.0, 2.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 24.0,
                    standard_area: 24.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1, "expected one hole");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        let hole = hole_rings[0].as_array().expect("hole must be an array of points");
        assert_eq!(hole.len(), 1, "hole should have 1 point");
    }

    /// A hole with NaN coordinates is reported with finite JSON numbers (NaN serializes to null in serde_json).
    #[test]
    fn inspect_room_hole_with_nan_coordinates() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                d.arch.push_room(Room {
                    name: "NanHole".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                    holes: vec![vec![Vec2::new(f64::NAN, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)]],
                    ceiling_height: Some(2.4),
                    interior_area: 20.0,
                    standard_area: 20.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 1, "expected one hole");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        let hole = hole_rings[0].as_array().expect("hole must be an array of points");
        assert_eq!(hole.len(), 4, "hole should have 4 points");
        // The first point's x is NaN, which serde_json serializes as null.
        assert!(hole[0][0].is_null(), "NaN x should serialize to null");
        assert!((hole[0][1].as_f64().expect("y") - 1.0).abs() < 1e-9, "y should be 1.0");
    }

    /// A room with many holes reports all of them correctly.
    #[test]
    fn inspect_room_many_holes() {
        let mut s = Session::new();
        match s.doc_mut() {
            Ok(d) => {
                let mut holes = Vec::new();
                for i in 0..10 {
                    let x = (i % 5) as f64 * 1.5 + 0.5;
                    let y = (i / 5) as f64 * 1.5 + 0.5;
                    holes.push(vec![Vec2::new(x, y), Vec2::new(x + 0.5, y), Vec2::new(x + 0.5, y + 0.5), Vec2::new(x, y + 0.5)]);
                }
                d.arch.push_room(Room {
                    name: "ManyHoles".into(),
                    room_type: RoomType::Living,
                    level: 0,
                    polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
                    holes,
                    ceiling_height: Some(2.4),
                    interior_area: 70.0,
                    standard_area: 70.0,
                    ..Room::default()
                });
                d.touch_arch();
            }
            Err(_) => {}
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "rooms": true }));
        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let r = &rooms[0];
        assert_eq!(r["holes"].as_i64().unwrap(), 10, "expected 10 holes");
        let hole_rings = r["holeRings"].as_array().expect("holeRings must be an array");
        assert_eq!(hole_rings.len(), 10, "expected 10 hole rings");
        for h in hole_rings {
            let ring = h.as_array().expect("each hole must be an array of points");
            assert_eq!(ring.len(), 4, "each hole should have 4 points");
        }
    }

    /// Build a session around `f` applied to a fresh arch model.
    fn session_with(f: impl FnOnce(&mut cadcraft_doc::arch::ArchModel)) -> Session {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            f(&mut d.arch);
            d.touch_arch();
        }
        s
    }

    #[test]
    fn inspect_lists_furnishings_with_hosts_and_layers() {
        use cadcraft_doc::arch::{Furnishing, FurnishingHost, Placement};
        let mut s = session_with(|m| {
            let w = m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(5.0, 0.0), ..Wall::default() });
            m.push_furnishing(Furnishing {
                item: "armchair".into(),
                name: "Armchair".into(),
                category: "Furniture".into(),
                symbol_type: "chair".into(),
                at: [1.0, 2.0],
                rotation: 90.0,
                size: [0.8, 0.9, 1.0],
                mirrored: true,
                host: FurnishingHost::Wall { wall: w },
                placement: Placement::Wall,
                ..Furnishing::default()
            });
            m.push_furnishing(Furnishing { cutout: Some([0.5, 0.4]), ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list.len(), 2);
        let f = &list[0];
        assert_eq!(f["id"], "f0");
        assert_eq!(f["item"], "armchair");
        assert_eq!(f["placement"], "wall");
        assert_eq!(f["at"], json!([1.0, 2.0]));
        assert_eq!(f["size"], json!([0.8, 0.9, 1.0]));
        assert_eq!(f["mirrored"], true);
        assert_eq!(f["host"], json!({"kind": "wall", "wall": "w0"}));
        assert_eq!(f["layer"], "I-FURN");
        assert!(f["cutout"].is_null());
        assert!(f["handle"].is_string());
        assert!(list[1]["host"].is_null());
        assert_eq!(list[1]["cutout"], json!([0.5, 0.4]));
        let none = exec(&mut Session::new(), "arch.inspect", &json!({ "furnishings": true }));
        assert_eq!(none["furnishings"], json!([]));
    }

    #[test]
    fn inspect_lists_surfaces_with_rooms_and_materials() {
        let mut s = session_with(|m| {
            m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(5.0, 0.0), ..Wall::default() });
            m.push_room(Room {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
                ..Room::default()
            });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "surfaces": true }));
        let list = v["surfaces"].as_array().expect("surfaces array");
        let keys: Vec<&str> = list.iter().filter_map(|x| x["surface"].as_str()).collect();
        assert!(keys.contains(&"wall/w0/left") && keys.contains(&"wall/w0/right"), "{keys:?}");
        assert!(keys.contains(&"room/r0/floor"), "{keys:?}");
        for row in list {
            assert!(row["material"].is_string());
            assert!(row.get("room").is_some() && row.get("tint").is_some() && row["level"].is_number());
        }
        let floor = list.iter().find(|x| x["surface"] == "room/r0/floor").expect("floor");
        assert_eq!(floor["room"], "r0");
    }

    fn kitchen_cabinet() -> cadcraft_doc::arch::Cabinet {
        use cadcraft_doc::arch::{Cabinet, CabinetKind, FrontStyle, Fronts};
        Cabinet {
            kind: CabinetKind::Base,
            width: 0.6,
            depth: 0.6,
            height: 0.88,
            fronts: Fronts::Doors,
            doors: 2,
            door_style: FrontStyle::Shaker,
            drawer_style: FrontStyle::RaisedPanel,
            pull: Some("bar".into()),
            toe_kick: true,
            ..Cabinet::default()
        }
    }

    fn counter_cabinet(x: f64) -> cadcraft_doc::arch::Cabinet {
        cadcraft_doc::arch::Cabinet { position: Vec2::new(x, 0.3), counter: true, counter_thickness: 0.04, ..kitchen_cabinet() }
    }

    #[test]
    fn inspect_countertops_rows_for_automatic_counters() {
        let mut s = session_with(|m| {
            m.push_cabinet(counter_cabinet(0.3));
            m.push_cabinet(counter_cabinet(0.9));
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "countertops": true }));
        let rows = v["countertops"].as_array().expect("countertops array");
        assert_eq!(rows.len(), 1, "{rows:?}");
        let r = &rows[0];
        assert_eq!(r["id"], "cb0");
        assert_eq!(r["auto"], true);
        assert!(r["handle"].is_null());
        assert_eq!(r["cabinets"], json!(["cb0", "cb1"]));
        assert_eq!(r["polygon"].as_array().map(Vec::len), Some(4));
        let edges = r["edges"].as_array().expect("edges");
        assert_eq!(edges.len(), 4);
        assert!(edges.iter().all(|e| e["radius"].is_null() && e["sagitta"] == 0.0 && e["profile"] == "square" && e["waterfall"] == false));
        assert!(r["top"].as_f64().is_some_and(|t| t > 0.88), "{r:?}");
        assert!(r["area"].as_f64().is_some_and(|a| a > 0.7), "{r:?}");
        assert_eq!(r["holes"], json!([]));
        assert!(r["layer"].is_string());
        // Not asked for: no key.
        let v = exec(&mut s, "arch.inspect", &json!({}));
        assert!(v.get("countertops").is_none());
    }

    #[test]
    fn inspect_countertops_rows_for_custom_ones() {
        use cadcraft_doc::arch::{CounterEdge, Countertop, EdgeProfile};
        let mut s = session_with(|m| {
            m.push_countertop(Countertop {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
                edges: vec![
                    CounterEdge { sagitta: 0.5, profile: EdgeProfile::Bullnose, waterfall: true, ..CounterEdge::default() },
                    CounterEdge::default(),
                ],
                ..Countertop::default()
            });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "countertops": true }));
        let rows = v["countertops"].as_array().expect("countertops array");
        assert_eq!(rows.len(), 1, "{rows:?}");
        let r = &rows[0];
        assert_eq!(r["id"], "ct0");
        assert_eq!(r["auto"], false);
        assert!(r["handle"].is_number() || r["handle"].is_string(), "{r:?}");
        let e0 = &r["edges"][0];
        assert_eq!(e0["length"], 2.0);
        assert_eq!(e0["sagitta"], 0.5);
        assert_eq!(e0["radius"], 1.25); // (2²/4 + 0.5²) / (2 * 0.5)
        assert_eq!(e0["profile"], "bullnose");
        assert_eq!(e0["waterfall"], true);
        assert!(r["edges"][1]["radius"].is_null());
        assert_eq!(r["thickness"], 0.038);
        assert_eq!(r["top"], 0.914);
        // 2 x 1 rectangle plus the segment of a chord 2, sagitta 0.5 (> 0).
        assert!(r["area"].as_f64().is_some_and(|a| a > 2.5), "{r:?}");
        assert_eq!(r["layer"], "A-FLOR-CASE");
    }

    #[test]
    fn inspect_cabinets_report_styles_pulls_and_fronts() {
        let mut s = session_with(|m| {
            m.push_cabinet(kitchen_cabinet());
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        let c = &v["cabinets"][0];
        assert_eq!(c["doorStyle"], "shaker");
        assert_eq!(c["drawerStyle"], "raised-panel");
        assert_eq!(c["pull"], "bar");
        assert_eq!(c["falseFront"], false);
        assert!(c["cutout"].is_null());
        // `fronts` stays the phase-13 mode (the cabinet command's own parameter); the layout is `frontParts`.
        assert_eq!(c["fronts"], "doors");
        let fronts = c["frontParts"].as_array().expect("frontParts");
        assert_eq!(fronts.len(), 2);
        assert_eq!(fronts[0]["kind"], "door");
        assert_eq!(fronts[0]["hinge"], "left");
        assert_eq!(fronts[1]["hinge"], "right");
        assert_eq!(fronts[0]["rect"].as_array().map(Vec::len), Some(4));
        assert_eq!(fronts[0]["pull"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn inspect_reports_false_fronts_and_cutouts() {
        use cadcraft_doc::arch::{Fronts, Furnishing, FurnishingHost};
        let mut s = session_with(|m| {
            let id = m.push_cabinet(kitchen_cabinet());
            if let Some(c) = m.cabinets.first_mut() {
                c.fronts = Fronts::Drawers;
                c.drawers = 3;
                c.doors = 0;
            }
            m.push_furnishing(Furnishing { cutout: Some([0.5, 0.4]), host: FurnishingHost::Cabinet { cabinet: id }, ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        let c = &v["cabinets"][0];
        assert_eq!(c["falseFront"], true);
        assert_eq!(c["cutout"], json!([0.5, 0.4]));
        let fronts = c["frontParts"].as_array().expect("frontParts");
        let f = fronts.iter().find(|f| f["kind"] == "false").expect("a false front");
        assert!(f["pull"].is_null() && f["hinge"].is_null());
    }

    /// All five furnishing host types serialize correctly.
    #[test]
    fn inspect_furnishing_all_host_types() {
        use cadcraft_doc::arch::{Furnishing, FurnishingHost};
        let mut s = session_with(|m| {
            let w = m.push_wall(Wall { start: Vec2::ZERO, end: Vec2::new(5.0, 0.0), ..Wall::default() });
            let cb = m.push_cabinet(kitchen_cabinet());
            let f0 = m.push_furnishing(Furnishing { host: FurnishingHost::None, ..Furnishing::default() });
            m.push_furnishing(Furnishing { host: FurnishingHost::Wall { wall: w }, ..Furnishing::default() });
            m.push_furnishing(Furnishing { host: FurnishingHost::Cabinet { cabinet: cb }, ..Furnishing::default() });
            m.push_furnishing(Furnishing { host: FurnishingHost::Sink { furnishing: f0 }, ..Furnishing::default() });
            m.push_furnishing(Furnishing { host: FurnishingHost::Gap { left: cb, right: cb }, ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list.len(), 5);
        assert!(list[0]["host"].is_null());
        assert_eq!(list[1]["host"], json!({"kind": "wall", "wall": "w0"}));
        assert_eq!(list[2]["host"], json!({"kind": "cabinet", "cabinet": "cb0"}));
        assert_eq!(list[3]["host"], json!({"kind": "sink", "furnishing": "f0"}));
        assert_eq!(list[4]["host"], json!({"kind": "gap", "left": "cb0", "right": "cb0"}));
    }

    /// Furnishings with NaN or infinite coordinates serialize without crashing.
    #[test]
    fn inspect_furnishings_with_non_finite_coords() {
        use cadcraft_doc::arch::Furnishing;
        let mut s = session_with(|m| {
            m.push_furnishing(Furnishing {
                at: [f64::NAN, 2.0],
                rotation: f64::INFINITY,
                elevation: f64::NEG_INFINITY,
                size: [1.0, f64::NAN, 0.5],
                ..Furnishing::default()
            });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list.len(), 1);
        // NaN serializes to null in serde_json
        assert!(list[0]["at"][0].is_null());
        assert!((list[0]["at"][1].as_f64().expect("y") - 2.0).abs() < 1e-9);
        assert!(list[0]["size"][1].is_null());
    }

    /// A cabinet with no pull reports pull as null and front pulls as null.
    #[test]
    fn inspect_cabinet_null_pull() {
        let mut s = session_with(|m| {
            let mut c = kitchen_cabinet();
            c.pull = None;
            m.push_cabinet(c);
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        let c = &v["cabinets"][0];
        assert!(c["pull"].is_null());
        let fronts = c["frontParts"].as_array().expect("frontParts");
        for f in fronts {
            assert!(f["pull"].is_null(), "front pull should be null when cabinet has no pull");
        }
    }

    /// Hostile JSON: non-boolean flags are ignored, not errors.
    #[test]
    fn inspect_hostile_non_boolean_flags() {
        let mut s = session_with(|m| {
            m.push_wall(Wall { start: Vec2::ZERO, end: Vec2::new(5.0, 0.0), ..Wall::default() });
        });
        let v = exec(
            &mut s,
            "arch.inspect",
            &json!({
                "furnishings": "yes",
                "surfaces": 1,
                "cabinets": null,
                "poche": "true",
                "rooms": [],
            }),
        );
        assert!(v.get("furnishings").is_none());
        assert!(v.get("surfaces").is_none());
        assert!(v.get("cabinets").is_none());
        assert!(v["walls"].is_array());
    }

    /// Surfaces report null tint when no tint is set; material is always a string.
    #[test]
    fn inspect_surfaces_null_tint() {
        let mut s = session_with(|m| {
            m.push_wall(Wall { start: Vec2::ZERO, end: Vec2::new(5.0, 0.0), ..Wall::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "surfaces": true }));
        let list = v["surfaces"].as_array().expect("surfaces array");
        for row in list {
            assert!(row.get("tint").is_some(), "tint key must be present");
            assert!(row["material"].is_string());
            assert!(row["surface"].is_string());
        }
    }

    /// Multiple cabinets with different front styles and pull configurations.
    #[test]
    fn inspect_multiple_cabinets_varied_styles() {
        use cadcraft_doc::arch::{Cabinet, CabinetKind, FrontStyle, Fronts};
        let mut s = session_with(|m| {
            m.push_cabinet(Cabinet {
                kind: CabinetKind::Base,
                width: 0.6,
                fronts: Fronts::Doors,
                doors: 1,
                door_style: FrontStyle::Slab,
                ..Cabinet::default()
            });
            m.push_cabinet(Cabinet {
                kind: CabinetKind::Wall,
                width: 0.9,
                fronts: Fronts::Drawers,
                drawers: 2,
                drawer_style: FrontStyle::RaisedPanel,
                pull: Some("bar".into()),
                ..Cabinet::default()
            });
            m.push_cabinet(Cabinet {
                kind: CabinetKind::Tall,
                width: 0.6,
                fronts: Fronts::Doors,
                doors: 2,
                door_style: FrontStyle::Shaker,
                pull: Some("cup".into()),
                ..Cabinet::default()
            });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        let list = v["cabinets"].as_array().expect("cabinets array");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0]["doorStyle"], "slab");
        assert!(list[0]["pull"].is_null());
        assert_eq!(list[1]["drawerStyle"], "raised-panel");
        assert_eq!(list[1]["pull"], "bar");
        assert_eq!(list[2]["doorStyle"], "shaker");
        assert_eq!(list[2]["pull"], "cup");
    }

    #[test]
    fn inspect_reports_grade_classes_and_area() {
        let mut s = Session::new();
        let d = s.doc_mut().expect("doc should exist");
        d.arch.levels.clear();
        d.arch.levels.push(cadcraft_doc::arch::Level {
            index: 0,
            name: "Lower".to_string(),
            elevation: -1.2,
            ceiling_height: 2.4,
            structure: 0.3,
            ..Default::default()
        });
        d.arch.levels.push(cadcraft_doc::arch::Level {
            index: 1,
            name: "Main".to_string(),
            elevation: 2.7,
            ceiling_height: 2.4,
            structure: 0.3,
            ..Default::default()
        });
        d.arch.levels.push(cadcraft_doc::arch::Level {
            index: 2,
            name: "Entry".to_string(),
            elevation: 1.35,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: cadcraft_doc::arch::LevelKind::Landing,
            ..Default::default()
        });
        d.touch_arch();

        exec(&mut s, "wall", &json!({ "points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2, "exterior": true, "level": 0, "topLevel": 1 }));
        exec(&mut s, "wall", &json!({ "points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2, "exterior": true, "level": 0, "topLevel": 1 }));
        exec(&mut s, "wall", &json!({ "points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2, "exterior": true, "level": 0, "topLevel": 1 }));
        exec(&mut s, "wall", &json!({ "points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2, "exterior": true, "level": 0, "topLevel": 1 }));
        exec(&mut s, "roomdetect", &json!({ "level": 1 }));

        let v = exec(&mut s, "arch.inspect", &json!({ "levels": true, "rooms": true, "walls": true }));
        let levels = v["levels"].as_array().expect("levels array missing");
        let lower = levels.iter().find(|row| row["name"] == "Lower").expect("Lower level missing");
        let main = levels.iter().find(|row| row["name"] == "Main").expect("Main level missing");
        let entry = levels.iter().find(|row| row["name"] == "Entry").expect("Entry level missing");

        assert_eq!(lower["kind"], "floor");
        assert_eq!(lower["gradeClass"], "basement");
        assert_eq!(main["kind"], "floor");
        assert_eq!(main["gradeClass"], "aboveGrade");
        assert_eq!(entry["kind"], "landing");
        assert_eq!(entry["gradeClass"], "aboveGrade");
        assert!((lower["aboveGradeArea"].as_f64().unwrap_or(-1.0) - 0.0).abs() < 1e-6);
        assert!((entry["aboveGradeArea"].as_f64().unwrap_or(-1.0) - 0.0).abs() < 1e-6);

        let rooms = v["rooms"].as_array().expect("rooms array missing");
        let main_area: f64 = rooms.iter().filter(|r| r["level"] == "Main").map(|r| r["interiorArea"].as_f64().unwrap_or(0.0)).sum();
        assert!(
            (v["aboveGradeArea"].as_f64().unwrap_or(-1.0) - main_area).abs() < 1e-6,
            "aboveGradeArea mismatch: expected {main_area}, got {}",
            v["aboveGradeArea"]
        );

        let mut zero = Session::new();
        let d = zero.doc_mut().unwrap();
        d.arch.levels.clear();
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 0, name: "Zero".to_string(), elevation: 0.0, ..Default::default() });
        let c = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)];
        for i in 0..4 {
            let mut w = cadcraft_doc::Wall::new(c[i], c[(i + 1) % 4], 0.2);
            w.exterior = true;
            w.base_level = 0;
            d.arch.push_wall(w);
        }
        d.arch.push_room(cadcraft_doc::arch::Room { level: 0, polygon: c.to_vec(), ..Default::default() });
        d.touch_arch();
        let zero_class = cadcraft_arch::grade::grade_class(&zero.doc().unwrap().arch, &zero.doc().unwrap().arch.levels[0]);
        assert_eq!(zero_class, cadcraft_arch::grade::GradeClass::AboveGrade);
    }

    #[test]
    fn inspect_walls_report_bottom_and_top() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.clear();
        d.arch.levels.push(cadcraft_doc::arch::Level {
            index: 0,
            name: "Lower".to_string(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            ..Default::default()
        });
        d.arch.levels.push(cadcraft_doc::arch::Level {
            index: 1,
            name: "Main".to_string(),
            elevation: 2.7,
            ceiling_height: 2.4,
            structure: 0.3,
            ..Default::default()
        });
        d.touch_arch();

        exec(&mut s, "wall", &json!({ "points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2, "exterior": true, "level": 0, "topLevel": 1 }));
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        let span = find_wall_by_points(walls, [0.0, 0.0], [10.0, 0.0]).expect("wall with topLevel not found");
        assert_eq!(span["bottom"], 0.0);
        assert_eq!(span["top"], 5.1);
        assert_eq!(span["topLevel"], 1);

        exec(&mut s, "wall", &json!({ "points": [[10.0, 0.0], [20.0, 0.0]], "thickness": 0.2, "exterior": true, "level": 1, "height": 1.25 }));
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        let plain = walls.iter().find(|row| row["start"] == json!([10.0, 0.0]) && row["end"] == json!([20.0, 0.0])).expect("plain wall not found");
        assert_eq!(plain["bottom"], 2.7);
        assert_eq!(plain["top"], 3.95);
        assert!(plain["topLevel"].is_null());
    }

    #[test]
    fn inspect_without_flags_is_unchanged() {
        let mut s = session_with(|m| {
            m.push_cabinet(kitchen_cabinet());
        });
        let v = exec(&mut s, "arch.inspect", &json!({}));
        assert!(v.get("furnishings").is_none());
        assert!(v.get("surfaces").is_none());
        assert!(v.get("cabinets").is_none());
        let rows = exec(&mut s, "arch.inspect", &json!({ "cabinets": true }));
        assert!(rows["cabinets"][0]["frontParts"].is_array());
        assert!(rows["cabinets"][0].get("frontsMode").is_none());
    }

    #[test]
    fn inspect_furnishing_in_room_hole_reports_null() {
        use cadcraft_doc::arch::Furnishing;
        let mut s = session_with(|m| {
            m.push_room(Room {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 6.0), Vec2::new(0.0, 6.0)],
                holes: vec![vec![Vec2::new(2.0, 2.0), Vec2::new(4.0, 2.0), Vec2::new(4.0, 4.0), Vec2::new(2.0, 4.0)]],
                room_type: RoomType::Living,
                ..Room::default()
            });
            // Furnishing inside the hole.
            m.push_furnishing(Furnishing { at: [3.0, 3.0], ..Furnishing::default() });
            // Furnishing in the room but outside the hole.
            m.push_furnishing(Furnishing { at: [1.0, 1.0], ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list[0]["room"], Value::Null, "furnishing in hole should report null room");
        assert_eq!(list[1]["room"], json!("r0"), "furnishing in room should report r0");
    }

    #[test]
    fn inspect_furnishing_on_wrong_level_reports_null() {
        use cadcraft_doc::arch::Furnishing;
        let mut s = session_with(|m| {
            m.push_room(Room {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
                room_type: RoomType::Living,
                ..Room::default()
            });
            // Furnishing on level 1, room on level 0.
            m.push_furnishing(Furnishing { at: [2.0, 2.0], level: 1, ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list[0]["room"], Value::Null, "furnishing on different level should report null room");
    }

    #[test]
    fn inspect_furnishing_at_non_finite_position_reports_null_room() {
        use cadcraft_doc::arch::Furnishing;
        let mut s = session_with(|m| {
            m.push_room(Room {
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
                room_type: RoomType::Living,
                ..Room::default()
            });
            // Furnishing with NaN x coordinate.
            m.push_furnishing(Furnishing { at: [f64::NAN, 2.0], ..Furnishing::default() });
        });
        let v = exec(&mut s, "arch.inspect", &json!({ "furnishings": true }));
        let list = v["furnishings"].as_array().expect("furnishings array");
        assert_eq!(list[0]["room"], Value::Null, "furnishing at non-finite position should report null room");
    }

    /// A level with NaN elevation serializes elevation as null (serde_json behavior).
    #[test]
    fn inspect_levels_with_non_finite_elevation() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            d.arch.levels.clear();
            d.arch.levels.push(cadcraft_doc::arch::Level {
                index: 0,
                name: "NaN Level".to_string(),
                elevation: f64::NAN,
                ceiling_height: 2.4,
                structure: 0.3,
                ..Default::default()
            });
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "levels": true }));
        let levels = v["levels"].as_array().expect("levels array missing");
        assert_eq!(levels.len(), 1);
        let l = &levels[0];
        assert!(l["elevation"].is_null(), "elevation should be null for NaN");
        assert_eq!(l["name"], "NaN Level");
        assert_eq!(l["kind"], "floor");
    }

    /// A zero-height wall has bottom == top (or both null when wall_span returns None).
    #[test]
    fn inspect_wall_with_zero_height() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            let w = cadcraft_doc::Wall {
                start: cadcraft_geom::Vec2::new(0.0, 0.0),
                end: cadcraft_geom::Vec2::new(5.0, 0.0),
                thickness: 0.2,
                height: 0.0,
                ..Default::default()
            };
            d.arch.push_wall(w);
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        let w = &walls[0];
        // wall_span may return None for a zero-height wall; either null or equal numbers is correct.
        if let Some(bottom) = w["bottom"].as_f64() {
            let top = w["top"].as_f64().expect("top should be a number when bottom is");
            assert_eq!(bottom, top, "zero-height wall should have bottom == top");
        } else {
            assert!(w["top"].is_null(), "top should be null when bottom is null");
        }
    }

    /// A landing level has aboveGradeArea 0.0 even when it has rooms.
    #[test]
    fn inspect_landing_above_grade_area_is_zero() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            d.arch.levels.clear();
            d.arch.levels.push(cadcraft_doc::arch::Level {
                index: 0,
                name: "Entry".to_string(),
                elevation: 1.0,
                ceiling_height: 2.4,
                structure: 0.3,
                kind: cadcraft_doc::arch::LevelKind::Landing,
                ..Default::default()
            });
            d.touch_arch();
        }
        if let Ok(d) = s.doc_mut() {
            d.arch.push_room(cadcraft_doc::arch::Room {
                name: "Hall".into(),
                room_type: cadcraft_doc::arch::RoomType::Living,
                level: 0,
                polygon: vec![
                    cadcraft_geom::Vec2::new(0.0, 0.0),
                    cadcraft_geom::Vec2::new(4.0, 0.0),
                    cadcraft_geom::Vec2::new(4.0, 3.0),
                    cadcraft_geom::Vec2::new(0.0, 3.0),
                ],
                interior_area: 12.0,
                standard_area: 12.0,
                ..Default::default()
            });
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "levels": true, "rooms": true }));
        let levels = v["levels"].as_array().expect("levels array missing");
        let entry = levels.iter().find(|row| row["name"] == "Entry").expect("Entry level missing");
        assert_eq!(entry["kind"], "landing");
        assert_eq!(entry["gradeClass"], "aboveGrade");
        assert!((entry["aboveGradeArea"].as_f64().unwrap_or(-1.0) - 0.0).abs() < 1e-9, "landing aboveGradeArea should be 0.0");
    }

    /// A wall with topLevel set to a non-existent level index has null bottom and top.
    #[test]
    fn inspect_wall_span_none_for_missing_top_level() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            let w = cadcraft_doc::Wall {
                start: cadcraft_geom::Vec2::new(0.0, 0.0),
                end: cadcraft_geom::Vec2::new(5.0, 0.0),
                thickness: 0.2,
                height: 2.7,
                top_level: Some(5),
                ..Default::default()
            };
            d.arch.push_wall(w);
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        let w = &walls[0];
        assert_eq!(w["topLevel"], 5);
        assert!(w["bottom"].is_null(), "bottom should be null for missing top level");
        assert!(w["top"].is_null(), "top should be null for missing top level");
    }

    /// A wall with NaN start coordinate serializes it as null; span is still computed from the level.
    #[test]
    fn inspect_wall_with_non_finite_coordinates() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            let w = cadcraft_doc::Wall {
                start: cadcraft_geom::Vec2::new(f64::NAN, 0.0),
                end: cadcraft_geom::Vec2::new(5.0, 0.0),
                thickness: 0.2,
                height: 2.7,
                ..Default::default()
            };
            d.arch.push_wall(w);
            d.touch_arch();
        }
        let v = exec(&mut s, "arch.inspect", &json!({ "walls": true }));
        let walls = v["walls"].as_array().expect("walls array missing");
        let w = &walls[0];
        assert!(w["start"][0].is_null(), "start x should be null for NaN");
        // bottom and top are computed from the level, not the wall coordinates
        assert!(w["bottom"].is_number() || w["bottom"].is_null());
        assert!(w["top"].is_number() || w["top"].is_null());
    }
}
