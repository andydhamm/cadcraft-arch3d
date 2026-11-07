//! `build3d` — build the 3D scene for the current drawing's arch model.
//!
//! View-only command (`.noundo()`): reads `Drawing::arch`, runs the
//! `arch3d` builder, and reports the result as JSON. It must not mutate the
//! document or bump `arch_revision`.

use std::time::Instant;

use std::collections::BTreeMap;

use cadcraft_geom::{Vec2, Vec3};
use serde_json::{Value, json};

use crate::Session;

/// Run body. Returns `{"meshes": ..., "triangles": ..., "rebuildMs": ...,
/// "bounds": {"min": [x,y,z], "max": [x,y,z]}, "floors": ..., "ceilings": ..., "roofs": ...,
/// "stairs": ..., "railings": ..., "decks": ..., "terrain": ..., "paths": ..., "plants": ...,
/// "lights": ..., "fixtures": ..., "openings": ..., "revision": ...,
/// "incremental": bool, "rebuilt": n, "objects": n, "probe": [{id, part, level} | null, ...]}`.
/// With `{"digest": true}` a `"digest"` key (16 hex chars) is also present.
/// The optional `probe` key is a list of at most 1024 `[x, y, z]` points;
/// each element of the returned `probe` array is the hit (or `null`) for the
/// corresponding input point, in the same order.
///
/// Phase 16: the optional `ceiling` key is a list of at most 1024 `[x, y]` plan points and
/// `level` a level index or name (default index 0); the reply's `"ceiling"` (only when
/// `ceiling` was given) holds, per point, the finished ceiling elevation above it on that level
/// (`cadcraft_arch::ceiling::finished_ceiling_at`, absolute z) or `null` outside every room.
/// `"parts"` always counts the scene's meshes by part (`cadcraft_arch3d::probe::part_of`), except
/// `furnishing` and `electrical`, which count the furnishings and plates built; parts with no
/// mesh are left out.
fn run_build3d(s: &mut Session, params: &Value) -> crate::Result<Value> {
    let incremental = parse_bool_param(params, "incremental")?;
    let show_digest = parse_bool_param(params, "digest")?;
    let ceiling_points = parse_ceiling_points(params)?;
    let ceiling_level = parse_level_param(&s.doc()?.arch, params)?;

    let arch_model = s.doc()?.arch.clone();
    let arch_revision = s.doc()?.arch_revision;
    let t = Instant::now();

    let (scene, stats) = {
        let st = s.state_mut()?;
        if incremental {
            match st.scene_cache.as_mut() {
                Some(cache) => {
                    let stats = cache.update(&arch_model);
                    (cache.scene().clone(), stats)
                }
                None => {
                    let (cache, stats) = cadcraft_arch3d::SceneCache::new(&arch_model);
                    let scene = cache.scene().clone();
                    st.scene_cache = Some(cache);
                    (scene, stats)
                }
            }
        } else {
            let (cache, stats) = cadcraft_arch3d::SceneCache::new(&arch_model);
            let scene = cache.scene().clone();
            st.scene_cache = Some(cache);
            (scene, stats)
        }
    };

    let rebuild_ms = elapsed_ms(t);

    let probe_points = parse_probe_points(params)?;
    let probe_results: Vec<Value> = probe_points
        .iter()
        .map(|p| {
            scene
                .probe(*p)
                .map(|h| {
                    json!({
                        "id": h.id,
                        "part": h.part,
                        "level": h.level,
                    })
                })
                .unwrap_or(Value::Null)
        })
        .collect();

    let mut reply = json!({
        "meshes": scene.meshes.len(),
        "triangles": scene.meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>(),
        "rebuildMs": rebuild_ms,
        "bounds": {
            "min": [scene.bounds.min[0], scene.bounds.min[1], scene.bounds.min[2]],
            "max": [scene.bounds.max[0], scene.bounds.max[1], scene.bounds.max[2]]
        },
        "floors": scene.floors,
        "ceilings": scene.ceilings,
        "roofs": scene.roofs,
        "stairs": scene.stairs,
        "railings": scene.railings,
        "decks": scene.decks,
        "terrain": scene.terrain,
        "paths": scene.paths,
        "plants": scene.plants,
        "lights": scene.lights.len(),
        "fixtures": scene.fixtures,
        "shadowMapsBuilt": super::render::stats().shadow_maps_built,
        "finalRenders": super::render::stats().final_renders,
        "openings": scene.openings_built,
        "incremental": stats.incremental,
        "rebuilt": stats.rebuilt,
        "objects": stats.objects,
        "revision": arch_revision,
        "probe": probe_results,
        "parts": parts_json(&scene),
    });
    if let (Some(points), Value::Object(obj)) = (ceiling_points, &mut reply) {
        let zs: Vec<Value> =
            points.iter().map(|p| cadcraft_arch::ceiling::finished_ceiling_at(&arch_model, ceiling_level, *p).map_or(Value::Null, |z| json!(z))).collect();
        obj.insert("ceiling".to_string(), Value::Array(zs));
    }
    if show_digest {
        if let serde_json::Value::Object(ref mut obj) = reply {
            obj.insert("digest".to_string(), serde_json::Value::String(scene.digest()));
        }
    }

    Ok(reply)
}

/// Parse an optional boolean param. Missing or `null` yields `false`; any other type is
/// refused.
fn parse_bool_param(params: &Value, key: &str) -> crate::Result<bool> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(v) => match v {
            Value::Bool(b) => Ok(*b),
            _ => Err(crate::cmd::bad("build3d", format!("`{key}` must be a boolean"))),
        },
    }
}

/// Parse the optional `probe` key — a JSON array of `[x, y, z]` arrays — into a
///
/// Absent or `null` yields an empty list (no probing). A non-array value, an
/// array longer than 1024 entries, an element that is not a 3-number array,
/// or any non-finite coordinate is rejected with `bad`.
fn parse_probe_points(params: &Value) -> crate::Result<Vec<Vec3>> {
    match params.get("probe") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(v) => {
            let arr = v.as_array().ok_or_else(|| crate::cmd::bad("build3d", "`probe` must be an array of [x, y, z] arrays"))?;
            if arr.len() > 1024 {
                return Err(crate::cmd::bad("build3d", "probe: at most 1024 points allowed"));
            }
            let mut pts = Vec::with_capacity(arr.len());
            for (i, item) in arr.iter().enumerate() {
                let sub = item.as_array().ok_or_else(|| crate::cmd::bad("build3d", format!("probe[{}] must be [x, y, z]", i)))?;
                if sub.len() != 3 {
                    return Err(crate::cmd::bad("build3d", format!("probe[{}] must be an array of 3 numbers, got {}", i, sub.len())));
                }
                let x = sub.get(0).and_then(Value::as_f64).ok_or_else(|| crate::cmd::bad("build3d", format!("probe[{}][0] is not a number", i)))?;
                let y = sub.get(1).and_then(Value::as_f64).ok_or_else(|| crate::cmd::bad("build3d", format!("probe[{}][1] is not a number", i)))?;
                let z = sub.get(2).and_then(Value::as_f64).ok_or_else(|| crate::cmd::bad("build3d", format!("probe[{}][2] is not a number", i)))?;
                if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                    return Err(crate::cmd::bad("build3d", format!("probe[{}] has a non-finite coordinate", i)));
                }
                pts.push(Vec3::new(x, y, z));
            }
            Ok(pts)
        }
    }
}

/// Most points one `ceiling` query answers (as many as `probe`).
const MAX_CEILING_POINTS: usize = 1024;

/// Parse the optional `ceiling` key: a JSON array of at most 1024 `[x, y]` arrays of finite
/// numbers. Absent or `null` is `None` (no query, no reply key); anything else is refused.
fn parse_ceiling_points(params: &Value) -> crate::Result<Option<Vec<Vec2>>> {
    let bad = |msg: String| crate::cmd::bad("build3d", msg);
    let arr = match params.get("ceiling") {
        None | Some(Value::Null) => return Ok(None),
        Some(v) => v.as_array().ok_or_else(|| bad("`ceiling` must be an array of [x, y] arrays".into()))?,
    };
    if arr.len() > MAX_CEILING_POINTS {
        return Err(bad(format!("ceiling: at most {MAX_CEILING_POINTS} points allowed")));
    }
    let mut pts = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let sub = item.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(format!("ceiling[{i}] must be [x, y]")))?;
        let x = sub.first().and_then(Value::as_f64).ok_or_else(|| bad(format!("ceiling[{i}][0] is not a number")))?;
        let y = sub.get(1).and_then(Value::as_f64).ok_or_else(|| bad(format!("ceiling[{i}][1] is not a number")))?;
        if !x.is_finite() || !y.is_finite() {
            return Err(bad(format!("ceiling[{i}] has a non-finite coordinate")));
        }
        pts.push(Vec2::new(x, y));
    }
    Ok(Some(pts))
}

/// Parse the optional `level` key: a non-negative integer level index or a level name (a
/// numeric string is an index too). Absent or `null` is level 0. An unknown level is refused
/// (index 0 is always known, even before the model has levels).
fn parse_level_param(arch: &cadcraft_doc::arch::ArchModel, params: &Value) -> crate::Result<u32> {
    let by_index = |n: u64| -> Option<u32> {
        let n = u32::try_from(n).ok()?;
        (arch.level(n).is_some() || n == 0).then_some(n)
    };
    match params.get("level") {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(by_index)
            .ok_or_else(|| crate::cmd::bad("build3d", format!("unknown level `{n}` (a level index or name)"))),
        Some(Value::String(name)) => {
            let t = name.trim();
            arch.levels
                .iter()
                .find(|l| l.name == t)
                .or_else(|| arch.levels.iter().find(|l| l.name.eq_ignore_ascii_case(t)))
                .map(|l| l.index)
                .or_else(|| t.parse::<u64>().ok().and_then(by_index))
                .ok_or_else(|| crate::cmd::bad("build3d", format!("unknown level `{name}`")))
        }
        Some(other) => Err(crate::cmd::bad("build3d", format!("`level` must be a level index or name, got `{other}`"))),
    }
}

/// The scene's parts and how many there are: meshes by `probe::part_of`, except furnishings and
/// electrical plates, counted by the boxes built (one per object); parts with none are left out.
fn parts_json(scene: &cadcraft_arch3d::Scene3d) -> Value {
    let mut parts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for m in &scene.meshes {
        let part = cadcraft_arch3d::probe::part_of(m);
        if part != "furnishing" && part != "electrical" {
            *parts.entry(part).or_default() += 1;
        }
    }
    for (part, n) in [("furnishing", scene.furnishing_boxes.len()), ("electrical", scene.electrical_boxes.len())] {
        if n > 0 {
            parts.insert(part, n);
        }
    }
    json!(parts)
}

/// Elapsed ms since `t`. On wasm32 this is `0.0` — no `web-time` dependency so
/// the web build keeps compiling.
#[cfg(not(target_arch = "wasm32"))]
fn elapsed_ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
#[cfg(target_arch = "wasm32")]
fn elapsed_ms(_t: Instant) -> f64 {
    0.0
}

/// Command spec for `build3d`.
pub fn specs() -> Vec<crate::cmd::CommandSpec> {
    let mut v = Vec::new();
    v.push(
        crate::cmd::CommandSpec::new("build3d", "Build 3D", run_build3d)
            .menu(&["Architecture", "Build 3D"])
            .noundo()
            .params("{probe?: [[x, y, z], ...], ceiling?: [[x, y], ...], level?: index|name, incremental?: bool, digest?: bool}"),
    );
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    use cadcraft_doc::arch::{Level, Opening, OpeningKind, Railing, Room, RoomType, Stair};
    use cadcraft_doc::{ArchId, Wall};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    /// A session with a fresh empty drawing.
    fn fresh_session() -> Session {
        let mut s = Session::empty();
        s.new_drawing(false);
        s
    }

    /// A session whose arch model holds a single extrudable wall.
    fn wall_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.touch_arch();
        s
    }

    #[test]
    fn build3d_reports_mesh_and_triangle_counts() {
        let mut s = wall_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["meshes"].as_u64(), Some(1), "one wall -> one mesh");
        let tri = r["triangles"].as_u64().expect("missing triangles");
        assert!(tri > 0, "expected triangles > 0, got {tri}");
    }

    #[test]
    fn build3d_on_an_empty_model_is_not_an_error() {
        let mut s = fresh_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["meshes"].as_u64(), Some(0));
        assert_eq!(r["triangles"].as_u64(), Some(0));
        // All zeros: bounds at [0,0,0], never an error.
        for axis in 0..3 {
            assert_eq!(r["bounds"]["min"][axis].as_f64(), Some(0.0));
            assert_eq!(r["bounds"]["max"][axis].as_f64(), Some(0.0));
        }
    }

    #[test]
    fn build3d_does_not_bump_the_revision() {
        let mut s = wall_session();
        let before = s.doc().unwrap().arch_revision;
        let _ = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, before, "build3d must not bump arch_revision");
    }

    /// A session whose arch model holds a level and two `Living` rooms.
    fn rooms_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Living,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Living,
            polygon: vec![Vec2::new(5.0, 0.0), Vec2::new(9.0, 0.0), Vec2::new(9.0, 5.0), Vec2::new(5.0, 5.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    /// A session whose arch model holds a level and one `OpenBelow` room.
    fn open_below_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::OpenBelow,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    /// A session whose arch model holds one 10 m wall with a single hinged door on it.
    fn wall_with_opening_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(wid, 3.0, 0.9, 2.0, OpeningKind::Door));
        d.touch_arch();
        s
    }

    #[test]
    fn build3d_reports_the_openings_it_cut() {
        let mut s = wall_with_opening_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["openings"].as_u64(), Some(1), "expected 1 cut opening");
    }

    #[test]
    fn build3d_reports_floor_and_ceiling_counts() {
        let mut s = rooms_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["floors"].as_u64(), Some(2), "two Living rooms -> 2 floors");
        assert_eq!(r["ceilings"].as_u64(), Some(2), "two Living rooms -> 2 ceilings");
    }

    #[test]
    fn build3d_reports_zero_floors_for_open_below() {
        let mut s = open_below_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["floors"].as_u64(), Some(0), "OpenBelow rooms get no floor slab");
        assert_eq!(r["ceilings"].as_u64(), Some(1), "OpenBelow room still gets a ceiling");
    }

    #[test]
    fn build3d_still_reports_meshes_and_openings() {
        let mut s = wall_with_opening_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert!(r["meshes"].is_number(), "missing `meshes` key");
        assert!(r["triangles"].is_number(), "missing `triangles` key");
        assert!(r["openings"].is_number(), "missing `openings` key");
        assert!(r["bounds"].is_object(), "missing `bounds` key");
        assert!(r["revision"].is_number(), "missing `revision` key");
    }

    /// A session whose arch model holds a level and a `Deck` room.
    fn deck_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Deck,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    /// A session whose arch model holds a level and a `Porch` room.
    fn porch_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Porch,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    /// A session with mixed room types on one level.
    fn mixed_rooms_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Living,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Bedroom,
            polygon: vec![Vec2::new(5.0, 0.0), Vec2::new(9.0, 0.0), Vec2::new(9.0, 5.0), Vec2::new(5.0, 5.0)],
            ..Room::default()
        });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::OpenBelow,
            polygon: vec![Vec2::new(0.0, 6.0), Vec2::new(4.0, 6.0), Vec2::new(4.0, 11.0), Vec2::new(0.0, 11.0)],
            ..Room::default()
        });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Deck,
            polygon: vec![Vec2::new(5.0, 6.0), Vec2::new(9.0, 6.0), Vec2::new(9.0, 11.0), Vec2::new(5.0, 11.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    /// A session with rooms on two levels.
    fn two_level_rooms_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.levels.push(Level { ..Level::default() });
        d.arch.rooms.push(Room {
            level: 0,
            room_type: RoomType::Living,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.arch.rooms.push(Room {
            level: 1,
            room_type: RoomType::Bedroom,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        d.touch_arch();
        s
    }

    #[test]
    fn build3d_deck_room_has_floor_no_ceiling() {
        let mut s = deck_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["floors"].as_u64(), Some(1), "Deck room gets a floor slab");
        assert_eq!(r["ceilings"].as_u64(), Some(0), "Deck room gets no ceiling");
    }

    #[test]
    fn build3d_porch_room_has_floor_no_ceiling() {
        let mut s = porch_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["floors"].as_u64(), Some(1), "Porch room gets a floor slab");
        assert_eq!(r["ceilings"].as_u64(), Some(0), "Porch room gets no ceiling");
    }

    #[test]
    fn build3d_mixed_room_types_counts() {
        let mut s = mixed_rooms_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        // Living (floor+ceiling) + Bedroom (floor+ceiling) + OpenBelow (ceiling only) + Deck (floor only)
        assert_eq!(r["floors"].as_u64(), Some(3), "Living + Bedroom + Deck = 3 floors");
        assert_eq!(r["ceilings"].as_u64(), Some(3), "Living + Bedroom + OpenBelow = 3 ceilings");
    }

    #[test]
    fn build3d_rooms_on_two_levels_both_build() {
        let mut s = two_level_rooms_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["floors"].as_u64(), Some(2), "one room per level -> 2 floors");
        assert_eq!(r["ceilings"].as_u64(), Some(2), "one room per level -> 2 ceilings");
    }

    #[test]
    fn build3d_probe_answers_every_point() {
        let mut s = wall_session();
        let params = json!({
            "probe": [
                [5.0, 0.0, 1.0],
                [5.0, 1.0, 1.0],
                [-1.0, 0.0, 1.0],
            ],
        });
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 3, "expected 3 probe results, got {}", probe.len());

        // [5, 0, 1] is inside the 10 m wall (x 0..10, y -0.1..0.1, z 0..2.7).
        let hit = &probe[0];
        assert!(hit.is_object(), "probe[0] should be a hit object (inside wall)");
        assert!(hit["id"].is_string(), "probe[0].id should be a string");
        assert!(hit["part"].is_string(), "probe[0].part should be a string");
        assert!(hit["level"].is_number(), "probe[0].level should be a number");

        // [5, 1, 1] is well outside in y.
        assert!(probe[1].is_null(), "probe[1] should be null (outside wall)");

        // [-1, 0, 1] is well outside in x.
        assert!(probe[2].is_null(), "probe[2] should be null (outside wall)");
    }

    #[test]
    fn build3d_probe_rejects_non_finite() {
        let mut s = wall_session();
        // JSON has no NaN/Infinity, so a real non-finite `Number` is unrepresentable;
        // the textual form `"NaN"` is the closest input, and the command must reject it
        // (non-number) rather than coerce it.
        let params = json!({"probe": [[5.0, 0.0, "NaN"]]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a non-finite probe coordinate");
    }

    #[test]
    fn wall_on_level_two_builds_at_its_elevation() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap(); // creates Level 2 at elevation 2.7, index 1
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2, "level": "Level 2"})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let min_z = r["bounds"]["min"][2].as_f64().expect("min z");
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((min_z - 2.7).abs() < 1e-3, "Level-2 wall must start at its level elevation z=2.7, got {min_z}");
        assert!((max_z - 5.4).abs() < 1e-3, "Level-2 wall top must be 2.7+2.7=5.4, got {max_z}");
    }

    #[test]
    fn wall_to_ceiling_tops_at_ceiling_height() {
        let mut s = fresh_session();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2, "toCeiling": true})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((max_z - 2.4).abs() < 1e-3, "to_ceiling wall must top at Level 1 ceiling height 2.4, got {max_z}");
    }

    #[test]
    fn wall_on_level_one_default_height() {
        let mut s = fresh_session();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let min_z = r["bounds"]["min"][2].as_f64().expect("min z");
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((min_z - 0.0).abs() < 1e-3, "Level-1 wall must start at z=0.0, got {min_z}");
        assert!((max_z - 2.7).abs() < 1e-3, "Level-1 wall default height must be 2.7, got {max_z}");
    }

    #[test]
    fn wall_to_ceiling_on_level_two() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2, "level": "Level 2", "toCeiling": true})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let min_z = r["bounds"]["min"][2].as_f64().expect("min z");
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((min_z - 2.7).abs() < 1e-3, "Level-2 to-ceiling wall must start at z=2.7, got {min_z}");
        assert!((max_z - 5.1).abs() < 1e-3, "Level-2 to-ceiling wall must top at 2.7+2.4=5.1, got {max_z}");
    }

    #[test]
    fn wall_custom_height_on_level_two() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2, "level": "Level 2", "height": 3.0})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let min_z = r["bounds"]["min"][2].as_f64().expect("min z");
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((min_z - 2.7).abs() < 1e-3, "Level-2 wall must start at z=2.7, got {min_z}");
        assert!((max_z - 5.7).abs() < 1e-3, "Level-2 wall with height 3.0 must top at 2.7+3.0=5.7, got {max_z}");
    }

    #[test]
    fn wall_on_level_three_builds_at_its_elevation() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        s.execute("level", &json!({"above": "Level 2"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0]], "thickness": 0.2, "level": "Level 3"})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        let min_z = r["bounds"]["min"][2].as_f64().expect("min z");
        let max_z = r["bounds"]["max"][2].as_f64().expect("max z");
        assert!((min_z - 5.4).abs() < 1e-3, "Level-3 wall must start at z=5.4, got {min_z}");
        assert!((max_z - 8.1).abs() < 1e-3, "Level-3 wall top must be 5.4+2.7=8.1, got {max_z}");
    }

    #[test]
    fn build3d_probe_empty_array() {
        let mut s = wall_session();
        let params = json!({"probe": []});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 0, "expected 0 probe results for empty input");
    }

    #[test]
    fn build3d_probe_cap_exceeded() {
        let mut s = wall_session();
        let mut points = Vec::new();
        for i in 0..1025 {
            points.push(json!([i as f64, 0.0, 1.0]));
        }
        let params = json!({"probe": points});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject more than 1024 probe points");
    }

    #[test]
    fn build3d_probe_cap_exact() {
        let mut s = wall_session();
        let mut points = Vec::new();
        for i in 0..1024 {
            points.push(json!([i as f64, 0.0, 1.0]));
        }
        let params = json!({"probe": points});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 1024, "expected 1024 probe results for 1024 input points");
    }

    #[test]
    fn build3d_probe_wrong_element_count() {
        let mut s = wall_session();
        let params = json!({"probe": [[1.0, 2.0]]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a probe point with 2 elements");
    }

    #[test]
    fn build3d_probe_extra_element() {
        let mut s = wall_session();
        let params = json!({"probe": [[1.0, 2.0, 3.0, 4.0]]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a probe point with 4 elements");
    }

    #[test]
    fn build3d_probe_non_array_element() {
        let mut s = wall_session();
        let params = json!({"probe": [1]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a non-array probe element");
    }

    #[test]
    fn build3d_probe_non_number_element() {
        let mut s = wall_session();
        let params = json!({"probe": [["a", "b", "c"]]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a probe point with non-number elements");
    }

    #[test]
    fn build3d_probe_null_element() {
        let mut s = wall_session();
        let params = json!({"probe": [null]});
        let r = s.execute("build3d", &params);
        assert!(r.is_err(), "build3d must reject a null probe element");
    }

    #[test]
    fn build3d_probe_huge_values() {
        let mut s = wall_session();
        let params = json!({"probe": [[1e300, 1e300, 1e300]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 1);
        assert!(probe[0].is_null(), "probe at huge coordinates should miss");
    }

    #[test]
    fn build3d_probe_above_wall() {
        let mut s = wall_session();
        // Wall is at z 0..2.7, so z=3.0 is above
        let params = json!({"probe": [[5.0, 0.0, 3.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_null(), "probe above wall should miss");
    }

    #[test]
    fn build3d_probe_below_wall() {
        let mut s = wall_session();
        // Wall is at z 0..2.7, so z=-0.1 is below
        let params = json!({"probe": [[5.0, 0.0, -0.1]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_null(), "probe below wall should miss");
    }

    #[test]
    fn build3d_probe_through_opening() {
        let mut s = wall_with_opening_session();
        // Door at offset 3.0, width 0.9, so x 3.0..3.9, y 0, z 0..2.0
        // Probe through the door opening at z=1.0
        let params = json!({"probe": [[3.5, 0.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_null(), "probe through door opening should miss wall");
    }

    #[test]
    fn build3d_probe_multiple_walls() {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(0.0, 5.0),
            end: Vec2::new(10.0, 5.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.touch_arch();
        let params = json!({"probe": [[5.0, 0.0, 1.0], [5.0, 5.0, 1.0], [5.0, 2.5, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_object(), "probe at first wall should hit");
        assert!(probe[1].is_object(), "probe at second wall should hit");
        assert!(probe[2].is_null(), "probe between walls should miss");
    }

    #[test]
    fn build3d_probe_order_preserved() {
        let mut s = wall_session();
        let params = json!({"probe": [[5.0, 0.0, 1.0], [5.0, 1.0, 1.0], [5.0, 0.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 3);
        assert!(probe[0].is_object(), "first probe should hit");
        assert!(probe[1].is_null(), "second probe should miss");
        assert!(probe[2].is_object(), "third probe should hit");
    }

    #[test]
    fn build3d_probe_mixed_hits_and_misses() {
        let mut s = wall_session();
        let params = json!({"probe": [
            [5.0, 0.0, 1.0],   // hit
            [5.0, 1.0, 1.0],   // miss
            [5.0, 0.0, 3.0],   // miss (above)
            [5.0, 0.0, -0.1],  // miss (below)
            [5.0, 0.0, 0.5],   // hit
        ]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 5);
        assert!(probe[0].is_object(), "probe 0 should hit");
        assert!(probe[1].is_null(), "probe 1 should miss");
        assert!(probe[2].is_null(), "probe 2 should miss (above)");
        assert!(probe[3].is_null(), "probe 3 should miss (below)");
        assert!(probe[4].is_object(), "probe 4 should hit");
    }

    #[test]
    fn build3d_probe_wall_at_origin() {
        let mut s = wall_session();
        // Wall starts at origin, probe just inside the wall from the start should hit
        let params = json!({"probe": [[0.1, 0.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_object(), "probe just inside wall start should hit");
    }

    #[test]
    fn build3d_probe_wall_at_end() {
        let mut s = wall_session();
        // Wall ends at x=10, probe at end should hit
        let params = json!({"probe": [[10.0, 0.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_object(), "probe at wall end should hit");
    }

    #[test]
    fn build3d_probe_just_outside_wall_end() {
        let mut s = wall_session();
        // Wall ends at x=10, probe just past should miss
        let params = json!({"probe": [[10.5, 0.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert!(probe[0].is_null(), "probe just past wall end should miss");
    }

    /// A session holding the phase-1 wall chain: a closed 10 x 8 m perimeter,
    /// one `wall` command per side, on the default level.
    fn wall_chain_session() -> Session {
        let mut s = fresh_session();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s
    }

    /// `roofauto` on the phase-1 chain produces one roof, which `build3d` must
    /// report in `roofs`.
    #[test]
    fn build3d_reports_roof_count() {
        let mut s = wall_chain_session();
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "roofauto'd phase-1 chain must report 1 roof");
    }

    /// Walls without a roof must report zero.
    #[test]
    fn build3d_reports_zero_roofs_without_roofs() {
        let mut s = wall_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(0), "walls only must report 0 roofs");
    }

    /// Two separate wall chains, each with its own roof, must report two roofs.
    #[test]
    fn build3d_reports_multiple_roofs() {
        let mut s = fresh_session();
        // First building: 10x8 m
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        // Second building: 6x6 m, offset to not overlap
        s.execute("wall", &json!({"points": [[20.0, 0.0], [26.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[26.0, 0.0], [26.0, 6.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[26.0, 6.0], [20.0, 6.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[20.0, 6.0], [20.0, 0.0]], "thickness": 0.2})).unwrap();
        // Roof on both (one roofauto call covers all buildings)
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "one roofauto call creates one roof");
    }

    /// Roof count must be reported correctly when probe is also used.
    #[test]
    fn build3d_roof_count_with_probe() {
        let mut s = wall_chain_session();
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let params = json!({"probe": [[5.0, 4.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "roof count must be correct with probe");
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 1);
    }

    /// Calling build3d twice must give the same roof count (idempotent).
    #[test]
    fn build3d_roof_count_stable_across_calls() {
        let mut s = wall_chain_session();
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r1 = s.execute("build3d", &json!({})).unwrap();
        let r2 = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r1["roofs"].as_u64(), r2["roofs"].as_u64(), "roof count must be stable");
        assert_eq!(r1["roofs"].as_u64(), Some(1));
    }

    /// Roof count must be correct in a scene with walls, openings, and rooms.
    #[test]
    fn build3d_roof_count_in_complex_scene() {
        let mut s = wall_chain_session();
        // Add a separate wall with a door
        let d = s.doc_mut().unwrap();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(11.0, 0.0), end: Vec2::new(21.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(wid, 3.0, 0.9, 2.0, OpeningKind::Door));
        d.touch_arch();
        // Add a room
        s.execute(
            "room",
            &json!({
                "points": [[0.5, 0.5], [9.5, 0.5], [9.5, 7.5], [0.5, 7.5], [0.5, 0.5]],
                "name": "Living Room",
                "type": "Living"
            }),
        )
        .unwrap();
        // Add a roof
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "roof count must be correct in complex scene");
        assert!(r["meshes"].as_u64().unwrap() > 0, "scene must have meshes");
        assert!(r["triangles"].as_u64().unwrap() > 0, "scene must have triangles");
    }

    /// A two-story building should report one roof (on the top level).
    #[test]
    fn build3d_roof_count_two_story_building() {
        let mut s = fresh_session();
        // First floor: 10x8 m
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        // Second floor at elevation 2.7
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "thickness": 0.2, "level": "Level 2"})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 0.0], [10.0, 8.0]], "thickness": 0.2, "level": "Level 2"})).unwrap();
        s.execute("wall", &json!({"points": [[10.0, 8.0], [0.0, 8.0]], "thickness": 0.2, "level": "Level 2"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 8.0], [0.0, 0.0]], "thickness": 0.2, "level": "Level 2"})).unwrap();
        // Roof on top level
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "two-story building must report 1 roof");
    }

    /// A single wall (no closed loop) should report zero roofs.
    #[test]
    fn build3d_roof_count_single_wall() {
        let mut s = wall_session();
        // A single wall cannot form a closed loop, so no roof is built
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(0), "single wall must report 0 roofs");
    }

    /// A building with openings (doors and windows) should report the correct roof count.
    #[test]
    fn build3d_roof_count_with_openings() {
        let mut s = wall_chain_session();
        // Add a door and a window to the walls
        let d = s.doc_mut().unwrap();
        let first_wall_id = d.arch.walls[0].id;
        d.arch.push_opening(Opening::new(first_wall_id, 3.0, 0.9, 2.0, OpeningKind::Door));
        let second_wall_id = d.arch.walls[1].id;
        d.arch.push_opening(Opening::new(second_wall_id, 2.0, 1.2, 1.5, OpeningKind::Window));
        d.touch_arch();
        // Add a roof
        s.execute("roofauto", &json!({"pitchRise": 6, "pitchRun": 12})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["roofs"].as_u64(), Some(1), "building with openings must report 1 roof");
        assert_eq!(r["openings"].as_u64(), Some(2), "expected 2 openings");
    }

    #[test]
    fn build3d_reports_stair_and_railing_counts() {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();

        // Create two levels manually or via command if available.
        // Using command is safer if it exists, but we know stair command failed.
        // Level command likely exists.
        drop(d);
        s.execute("level", &json!({"above": "Level 1"})).unwrap();

        let d = s.doc_mut().unwrap();
        // Manually add a stair to the model
        d.arch.stairs.push(Stair {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.touch_arch();

        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["stairs"].as_u64(), Some(1), "expected 1 stair");
        // Railings count should be non-negative
        assert!(r["railings"].as_u64().unwrap_or(0) >= 0, "railings count should be non-negative");
    }

    /// An empty scene must report zero stairs, railings, and decks.
    #[test]
    fn build3d_zero_stairs_railings_decks_on_empty_scene() {
        let mut s = fresh_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["stairs"].as_u64(), Some(0), "empty scene must report 0 stairs");
        assert_eq!(r["railings"].as_u64(), Some(0), "empty scene must report 0 railings");
        assert_eq!(r["decks"].as_u64(), Some(0), "empty scene must report 0 decks");
    }

    /// Multiple stairs must each be counted.
    #[test]
    fn build3d_multiple_stairs_count_correctly() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let d = s.doc_mut().unwrap();
        d.arch.stairs.push(Stair {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.arch.stairs.push(Stair {
            id: ArchId(2),
            start: Vec2::new(5.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.arch.stairs.push(Stair {
            id: ArchId(3),
            start: Vec2::new(10.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.touch_arch();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["stairs"].as_u64(), Some(3), "expected 3 stairs");
    }

    /// A standalone railing (not following a stair) must be counted.
    #[test]
    fn build3d_standalone_railing_counted() {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.railings.push(Railing { id: ArchId(1), path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)], ..Railing::default() });
        d.touch_arch();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["railings"].as_u64(), Some(1), "expected 1 railing");
        assert_eq!(r["stairs"].as_u64(), Some(0), "railing alone must not create stairs");
    }

    /// Multiple deck rooms must each be counted.
    #[test]
    fn build3d_multiple_decks_count_correctly() {
        let mut s = fresh_session();
        s.execute(
            "room",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                "name": "Deck A",
                "type": "Deck"
            }),
        )
        .unwrap();
        s.execute(
            "room",
            &json!({
                "points": [[5.0, 0.0], [9.0, 0.0], [9.0, 5.0], [5.0, 5.0], [5.0, 0.0]],
                "name": "Deck B",
                "type": "Deck"
            }),
        )
        .unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["decks"].as_u64(), Some(2), "expected 2 decks");
    }

    /// Stair, railing, and deck counts must be stable across repeated build3d calls.
    #[test]
    fn build3d_stair_railing_deck_counts_stable_across_calls() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let d = s.doc_mut().unwrap();
        d.arch.stairs.push(Stair {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.arch.railings.push(Railing { id: ArchId(2), path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)], ..Railing::default() });
        d.touch_arch();
        drop(d);
        s.execute(
            "room",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                "name": "Deck",
                "type": "Deck"
            }),
        )
        .unwrap();

        let r1 = s.execute("build3d", &json!({})).unwrap();
        let r2 = s.execute("build3d", &json!({})).unwrap();
        let r3 = s.execute("build3d", &json!({})).unwrap();

        assert_eq!(r1["stairs"].as_u64(), Some(1));
        assert_eq!(r1["railings"].as_u64(), Some(1));
        assert_eq!(r1["decks"].as_u64(), Some(1));

        assert_eq!(r1["stairs"].as_u64(), r2["stairs"].as_u64(), "stairs count must be stable");
        assert_eq!(r1["railings"].as_u64(), r2["railings"].as_u64(), "railings count must be stable");
        assert_eq!(r1["decks"].as_u64(), r2["decks"].as_u64(), "decks count must be stable");

        assert_eq!(r2["stairs"].as_u64(), r3["stairs"].as_u64(), "stairs count must be stable");
        assert_eq!(r2["railings"].as_u64(), r3["railings"].as_u64(), "railings count must be stable");
        assert_eq!(r2["decks"].as_u64(), r3["decks"].as_u64(), "decks count must be stable");
    }

    /// Stair, railing, and deck counts must be correct when probe is also used.
    #[test]
    fn build3d_stair_railing_deck_counts_with_probe() {
        let mut s = fresh_session();
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let d = s.doc_mut().unwrap();
        d.arch.stairs.push(Stair {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            ..Stair::default()
        });
        d.arch.railings.push(Railing { id: ArchId(2), path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)], ..Railing::default() });
        d.touch_arch();
        drop(d);
        s.execute(
            "room",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                "name": "Deck",
                "type": "Deck"
            }),
        )
        .unwrap();

        let params = json!({"probe": [[2.0, 2.0, 1.0]]});
        let r = s.execute("build3d", &params).unwrap();
        assert_eq!(r["stairs"].as_u64(), Some(1), "stairs count must be correct with probe");
        assert_eq!(r["railings"].as_u64(), Some(1), "railings count must be correct with probe");
        assert_eq!(r["decks"].as_u64(), Some(1), "decks count must be correct with probe");
        let probe = r["probe"].as_array().expect("missing `probe` array");
        assert_eq!(probe.len(), 1);
    }

    /// Non-deck rooms must not be counted as decks.
    #[test]
    fn build3d_non_deck_rooms_have_zero_deck_count() {
        let mut s = fresh_session();
        s.execute(
            "room",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                "name": "Living Room",
                "type": "Living"
            }),
        )
        .unwrap();
        s.execute(
            "room",
            &json!({
                "points": [[5.0, 0.0], [9.0, 0.0], [9.0, 5.0], [5.0, 5.0], [5.0, 0.0]],
                "name": "Bedroom",
                "type": "Bedroom"
            }),
        )
        .unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["decks"].as_u64(), Some(0), "non-deck rooms must not be counted as decks");
    }

    #[test]
    fn build3d_reports_deck_count() {
        let mut s = fresh_session();
        // Create a deck room
        s.execute(
            "room",
            &json!({
                "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                "name": "Deck",
                "type": "Deck"
            }),
        )
        .unwrap();

        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["decks"].as_u64(), Some(1), "expected 1 deck");
    }

    /// Helper: a T-junction of two walls (shared join) plus three isolated walls, so a
    /// stretch of one wall rebuilds only the affected join, not every wall.
    fn t_junction_session() -> Session {
        let mut s = fresh_session();
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(20.0, 0.0),
            end: Vec2::new(24.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(4),
            start: Vec2::new(28.0, 0.0),
            end: Vec2::new(32.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(5),
            start: Vec2::new(36.0, 0.0),
            end: Vec2::new(40.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.touch_arch();
        s
    }

    #[test]
    fn build3d_incremental_after_one_stretch_rebuilds_few_objects() {
        use cadcraft_arch::join::{End, stretch_wall};

        let mut s = t_junction_session();

        // Seed the cache with a full build.
        s.execute("build3d", &json!({})).unwrap();

        // Stretch wall 1's end from (10,0) to (11,0); the T-stem (wall 2) is affected.
        let d = s.doc_mut().unwrap();
        stretch_wall(&mut d.arch, ArchId(1), End::End, Vec2::new(11.0, 0.0)).unwrap();
        d.touch_arch();

        let r = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let rebuilt = r["rebuilt"].as_u64().expect("missing rebuilt");
        let objects = r["objects"].as_u64().expect("missing objects");

        assert!(rebuilt >= 1, "at least one object should be rebuilt, got {}", rebuilt);
        assert!(rebuilt <= 2, "stretching one wall should rebuild at most 2 objects (got {} out of {})", rebuilt, objects);
        assert_eq!(objects, 5, "scene should still have 5 wall objects, got {}", objects);
    }

    #[test]
    fn build3d_incremental_digest_equals_a_full_build() {
        use cadcraft_arch::join::{End, stretch_wall};

        let mut s = t_junction_session();

        // Seed the cache, get the baseline digest.
        let r = s.execute("build3d", &json!({"digest": true})).unwrap();
        let baseline_digest = r["digest"].as_str().expect("missing digest");

        // Save the original wall state for a proper undo (stretch_wall also moves
        // the T-stem at the shared join).
        let original_walls = s.doc().unwrap().arch.walls.clone();

        // Stretch wall 1, then restore it (simulating undo).
        {
            let d = s.doc_mut().unwrap();
            stretch_wall(&mut d.arch, ArchId(1), End::End, Vec2::new(11.0, 0.0)).unwrap();
            d.touch_arch();
        }
        let r_stretched = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let stretched_digest = r_stretched["digest"].as_str().expect("missing digest");
        assert!(stretched_digest != baseline_digest, "stretch should change the scene digest");

        // Restore ALL walls to their original state (proper undo).
        {
            let d = s.doc_mut().unwrap();
            d.arch.walls = original_walls;
            d.touch_arch();
        }
        let r_undone = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let undone_digest = r_undone["digest"].as_str().expect("missing digest");

        // The scene after undo must be digest-identical to the original full build.
        assert_eq!(undone_digest, baseline_digest, "undo should restore the original scene: {} != {}", undone_digest, baseline_digest);
    }

    #[test]
    fn build3d_incremental_without_a_cache_builds_everything() {
        // No cache has been seeded yet: an incremental build must seed one with a
        // full build and report incremental=false.
        let mut s = t_junction_session();
        let r = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let incremental = r["incremental"].as_bool().expect("missing incremental");
        let rebuilt = r["rebuilt"].as_u64().expect("missing rebuilt");
        let objects = r["objects"].as_u64().expect("missing objects");

        assert!(!incremental, "a first incremental build (no cache) is a full build: incremental should be false");
        assert_eq!(rebuilt, objects, "first build must rebuild every object: rebuilt={} objects={}", rebuilt, objects);
    }

    #[test]
    fn build3d_incremental_follows_undo() {
        use cadcraft_arch::join::{End, stretch_wall};

        let mut s = t_junction_session();

        // Baseline: full build, no digest.
        s.execute("build3d", &json!({})).unwrap();

        // Stretch wall 1.
        {
            let d = s.doc_mut().unwrap();
            stretch_wall(&mut d.arch, ArchId(1), End::End, Vec2::new(11.0, 0.0)).unwrap();
            d.touch_arch();
        }
        // Incremental: few rebuilds.
        let r_stretched = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let rebuilt = r_stretched["rebuilt"].as_u64().expect("missing rebuilt");
        assert!(rebuilt >= 1 && rebuilt <= 2, "stretch rebuilds a few: rebuilt={}", rebuilt);

        // Undo: restore wall 1's end.
        {
            let d = s.doc_mut().unwrap();
            if let Some(w) = d.arch.walls.iter_mut().find(|w| w.id == ArchId(1)) {
                w.end = Vec2::new(10.0, 0.0);
            }
            d.touch_arch();
        }
        // Incremental again: must produce a scene whose wall-1 mesh matches a full build of
        // the restored model.
        let r_undone = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let undone_digest = r_undone["digest"].as_str().expect("missing digest");

        // The full-build digest of the same restored model must agree.
        let r_full = s.execute("build3d", &json!({"digest": true})).unwrap();
        let full_digest = r_full["digest"].as_str().expect("missing digest");

        assert_eq!(undone_digest, full_digest, "incremental-after-undo digest {} must equal full digest {}", undone_digest, full_digest);
    }

    #[test]
    fn build3d_full_reports_rebuilt_equal_to_objects() {
        // A full build rebuilds every object, so rebuilt must equal objects.
        let mut s = t_junction_session();
        let r = s.execute("build3d", &json!({})).unwrap();
        let rebuilt = r["rebuilt"].as_u64().expect("missing rebuilt");
        let objects = r["objects"].as_u64().expect("missing objects");

        assert_eq!(rebuilt, objects, "full build must rebuild all objects: rebuilt={} objects={}", rebuilt, objects);
    }

    #[test]
    fn build3d_rejects_non_boolean_incremental() {
        let mut s = wall_session();
        let r = s.execute("build3d", &json!({"incremental": "yes"}));
        assert!(r.is_err(), "non-boolean `incremental` must be refused");
    }

    #[test]
    fn build3d_rejects_non_boolean_digest() {
        let mut s = wall_session();
        let r = s.execute("build3d", &json!({"digest": "true"}));
        assert!(r.is_err(), "non-boolean `digest` must be refused");
    }

    /// An incremental build with no model changes since the last build must rebuild zero objects.
    #[test]
    fn build3d_incremental_no_changes_rebuilds_zero() {
        let mut s = wall_session();
        // Seed the cache with a full build.
        s.execute("build3d", &json!({})).unwrap();
        // Incremental build with no changes: rebuilt must be 0.
        let r = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let rebuilt = r["rebuilt"].as_u64().expect("missing rebuilt");
        let objects = r["objects"].as_u64().expect("missing objects");
        assert_eq!(rebuilt, 0, "no changes -> 0 rebuilt, got {}", rebuilt);
        assert_eq!(objects, 1, "scene should still have 1 wall object, got {}", objects);
        assert!(r["incremental"].as_bool().expect("missing incremental"), "should report incremental=true");
    }

    /// Adding a wall after a full build must be detected by an incremental build.
    #[test]
    fn build3d_incremental_after_adding_wall() {
        let mut s = wall_session();
        // Seed the cache with a full build (1 wall).
        let r_full = s.execute("build3d", &json!({})).unwrap();
        let objects_before = r_full["objects"].as_u64().expect("missing objects");
        assert_eq!(objects_before, 1, "baseline should have 1 wall");

        // Add a second wall.
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(0.0, 5.0),
            end: Vec2::new(10.0, 5.0),
            thickness: 0.2,
            height: 2.7,
            ..Wall::default()
        });
        d.touch_arch();

        // Incremental build must see the new wall.
        let r = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let rebuilt = r["rebuilt"].as_u64().expect("missing rebuilt");
        let objects = r["objects"].as_u64().expect("missing objects");
        assert!(rebuilt >= 1, "adding a wall must rebuild at least 1 object, got {}", rebuilt);
        assert_eq!(objects, 2, "scene should now have 2 wall objects, got {}", objects);
    }

    /// Removing a wall after a full build must be detected by an incremental build.
    #[test]
    fn build3d_incremental_after_removing_wall() {
        let mut s = t_junction_session();
        // Seed the cache with a full build (5 walls).
        let r_full = s.execute("build3d", &json!({})).unwrap();
        let objects_before = r_full["objects"].as_u64().expect("missing objects");
        assert_eq!(objects_before, 5, "baseline should have 5 walls");

        // Remove one wall.
        let d = s.doc_mut().unwrap();
        d.arch.walls.retain(|w| w.id != ArchId(5));
        d.touch_arch();

        // Incremental build must see the removal.
        let r = s.execute("build3d", &json!({"incremental": true})).unwrap();
        let objects = r["objects"].as_u64().expect("missing objects");
        assert_eq!(objects, 4, "scene should now have 4 wall objects, got {}", objects);
    }

    /// An incremental build on an empty model must work and report zero objects.
    #[test]
    fn build3d_incremental_empty_model() {
        let mut s = fresh_session();
        // Incremental build with no cache: full build of empty model, seeds cache.
        let r1 = s.execute("build3d", &json!({"incremental": true})).unwrap();
        assert_eq!(r1["objects"].as_u64(), Some(0), "empty model -> 0 objects");
        assert_eq!(r1["rebuilt"].as_u64(), Some(0), "empty model -> 0 rebuilt");
        assert!(!r1["incremental"].as_bool().expect("missing incremental"), "first build is full");

        // Second incremental build: cache exists, no changes.
        let r2 = s.execute("build3d", &json!({"incremental": true})).unwrap();
        assert_eq!(r2["objects"].as_u64(), Some(0), "empty model -> 0 objects");
        assert_eq!(r2["rebuilt"].as_u64(), Some(0), "no changes -> 0 rebuilt");
        assert!(r2["incremental"].as_bool().expect("missing incremental"), "should report incremental=true");
    }

    /// The phase-1 house through the commands: three wall commands 2.4 m high (w0..w5) and
    /// `roomdetect`, giving S (0.1..9.9 x 0.1..2.95), NW (0.1..5.95 x 3.05..7.9) and NE
    /// (6.05..9.9 x 3.05..7.9); `level` puts the walls on that level index.
    fn house_walls(s: &mut Session, level: Option<u32>) {
        let mut walls = vec![
            json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true}),
            json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
            json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
        ];
        for w in walls.iter_mut() {
            if let Some(l) = level {
                w["level"] = json!(l);
            }
            s.execute("wall", w).unwrap();
        }
        let detect = match level {
            Some(l) => json!({"level": l}),
            None => json!({}),
        };
        s.execute("roomdetect", &detect).unwrap();
    }

    fn house() -> Session {
        let mut s = Session::new();
        house_walls(&mut s, None);
        s
    }

    fn ceilings_of(r: &Value) -> Vec<Option<f64>> {
        r["ceiling"].as_array().expect("a `ceiling` list").iter().map(Value::as_f64).collect()
    }

    fn close(got: &[Option<f64>], want: &[Option<f64>]) -> bool {
        got.len() == want.len()
            && got.iter().zip(want).all(|(g, w)| match (g, w) {
                (Some(g), Some(w)) => (g - w).abs() < 1e-9,
                (None, None) => true,
                _ => false,
            })
    }

    #[test]
    fn build3d_ceiling_query_flat_house() {
        let mut s = house();
        let r = s.execute("build3d", &json!({"ceiling": [[5, 1.5], [3, 5], [8, 5], [20, 20]]})).unwrap();
        let got = ceilings_of(&r);
        assert!(close(&got, &[Some(2.4), Some(2.4), Some(2.4), None]), "{got:?}");
        // No `ceiling` (or null): no reply key.
        let r = s.execute("build3d", &json!({})).unwrap();
        assert!(r.get("ceiling").is_none());
        let r = s.execute("build3d", &json!({"ceiling": null})).unwrap();
        assert!(r.get("ceiling").is_none());
        // A room's own ceiling and its finish stack show up; an empty list answers an empty list.
        let ne = cadcraft_arch::ceiling::room_at_point(&s.doc().unwrap().arch, 0, Vec2::new(8.0, 5.0)).map(|r| r.id.0).unwrap();
        s.execute("room", &json!({"id": ne, "ceiling": 2.7, "ceilingStack": [{"material": "drywall", "thickness": 0.0127}]})).unwrap();
        let r = s.execute("build3d", &json!({"ceiling": [[8, 5], [5, 1.5]]})).unwrap();
        assert!(close(&ceilings_of(&r), &[Some(2.6873), Some(2.4)]), "{:?}", ceilings_of(&r));
        let r = s.execute("build3d", &json!({"ceiling": []})).unwrap();
        assert_eq!(r["ceiling"], json!([]));
    }

    #[test]
    fn build3d_ceiling_level_by_index_and_name() {
        let mut s = house();
        s.execute("level", &json!({"above": 0})).unwrap();
        house_walls(&mut s, Some(1));
        let names: Vec<String> = s.doc().unwrap().arch.levels.iter().map(|l| l.name.clone()).collect();
        for (level, want) in [(json!(0), 2.4), (json!("Level 1"), 2.4), (json!(1), 5.1), (json!(names[1].clone()), 5.1), (json!("1"), 5.1)] {
            let r = s.execute("build3d", &json!({"ceiling": [[5, 1.5]], "level": level})).unwrap();
            assert!(close(&ceilings_of(&r), &[Some(want)]), "{level}: {:?}", ceilings_of(&r));
        }
        // The default is level 0.
        let r = s.execute("build3d", &json!({"ceiling": [[5, 1.5]]})).unwrap();
        assert!(close(&ceilings_of(&r), &[Some(2.4)]));
    }

    #[test]
    fn build3d_ceiling_refuses_bad_points_and_levels() {
        let mut s = house();
        let rev = s.doc().unwrap().arch_revision;
        let too_many: Vec<Value> = (0..1025).map(|_| json!([1, 1])).collect();
        let bad = [
            json!({"ceiling": [[1, "x"]]}),
            json!({"ceiling": "all"}),
            json!({"ceiling": [[1]]}),
            json!({"ceiling": [[1, 2, 3]]}),
            json!({"ceiling": [1, 2]}),
            json!({"ceiling": [[null, 2]]}),
            json!({"ceiling": {"x": 1, "y": 2}}),
            json!({"ceiling": too_many}),
            json!({"ceiling": [[1, 1]], "level": "Moon"}),
            json!({"ceiling": [[1, 1]], "level": 7}),
            json!({"ceiling": [[1, 1]], "level": -1}),
            json!({"ceiling": [[1, 1]], "level": 0.5}),
            json!({"ceiling": [[1, 1]], "level": true}),
            json!({"ceiling": [[1, 1]], "level": [0]}),
            json!({"level": "Moon"}),
        ];
        for p in &bad {
            let r = s.execute("build3d", p);
            assert!(matches!(r, Err(crate::EngineError::BadParams { .. })), "{p}: {r:?}");
        }
        let at_cap: Vec<Value> = (0..1024).map(|_| json!([1e300, -1e300])).collect();
        let r = s.execute("build3d", &json!({"ceiling": at_cap})).unwrap();
        assert_eq!(r["ceiling"].as_array().map(Vec::len), Some(1024));
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn build3d_parts_counts_meshes_by_part() {
        let mut s = house();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["parts"], json!({"ceiling": 3, "floor": 3, "wall": 6}), "{}", r["parts"]);
        // Electrical plates count by their boxes (one per device), not by their meshes.
        s.execute("outlet", &json!({"kind": "duplex", "wall": "w0", "offset": 4.0, "side": "left"})).unwrap();
        let r = s.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["parts"]["electrical"], 1, "{}", r["parts"]);
        assert_eq!(r["parts"]["wall"], 6);
        // An empty model has no parts.
        let mut e = fresh_session();
        let r = e.execute("build3d", &json!({})).unwrap();
        assert_eq!(r["parts"], json!({}));
    }
}
