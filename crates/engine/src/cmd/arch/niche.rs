//! The `niche` command: a rectangular recess in a wall (a shower shelf, a display niche).
//!
//! A niche is an [`Opening`] of kind [`OpeningKind::Niche`] that is cut `depth` into the wall from
//! one face and backed by the rest of the wall. It is JSON-only, like `door` and `window`.

use serde_json::{Value, json};

use crate::EngineError;
use crate::Session;
use crate::cmd::{CommandSpec, bad, point_param, size_param, str_param};
use cadcraft_arch::opening::{place_opening, project_to_wall};
use cadcraft_arch::room::room_at;
use cadcraft_doc::arch::paint::{MaterialRef, Side};
use cadcraft_doc::arch::{ArchModel, MAX_OPENINGS, Opening, OpeningKind, handle_json, opening_handle, opening_label, wall_label};
use cadcraft_doc::{ArchId, Wall};
use cadcraft_geom::Vec2;

/// The smallest sill: a hole touching the wall's bottom would not be cut.
const MIN_SILL: f64 = 0.01;
/// How far off a wall face the room probe stands, in metres.
const ROOM_PROBE: f64 = 0.05;

/// The wall side that faces a room, else [`Side::Left`].
pub(crate) fn room_side(model: &ArchModel, wall: &Wall) -> Side {
    let d = wall.end - wall.start;
    let len = d.len();
    if !(len.is_finite() && len > 1e-12) {
        return Side::Left;
    }
    let dir = d * (1.0 / len);
    let left = Vec2::new(-dir.y, dir.x);
    let mid = wall.start.mid(wall.end);
    let faces = |n: Vec2| {
        let p = mid + n * (wall.thickness / 2.0 + ROOM_PROBE);
        room_at(model, wall.base_level, p).is_some()
    };
    if faces(left) || !faces(left * -1.0) { Side::Left } else { Side::Right }
}

fn run_niche(s: &mut Session, p: &Value) -> Result<Value, EngineError> {
    const CMD: &str = "niche";
    let d = s.doc_mut()?;

    let wall_s = str_param(p, "wall").ok_or_else(|| bad(CMD, "`wall` (string id) is required"))?;
    let host: ArchId = super::opening::parse_arch_id(wall_s, "wall").map_err(|e| bad(CMD, e))?;
    if d.arch.wall(host).is_none() {
        return Err(bad(CMD, format!("no wall {host}")));
    }
    let offset: f64 = if let Some(v) = p.get("offset").filter(|v| !v.is_null()) {
        v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad(CMD, "`offset` must be a finite number"))?
    } else if let Some(pt) = point_param(p, "point") {
        project_to_wall(&d.arch, host, pt).ok_or_else(|| bad(CMD, "point does not project onto the wall"))?
    } else {
        return Err(bad(CMD, "`offset` (number) or `point` ([x,y]) is required"));
    };

    let width = size_param(CMD, p, "width", false)?.unwrap_or(0.6);
    let height = size_param(CMD, p, "height", false)?.unwrap_or(0.9);
    let mut sill = size_param(CMD, p, "sill", true)?.unwrap_or(1.0);
    let depth = size_param(CMD, p, "depth", false)?.unwrap_or(0.1);

    let wall = d.arch.wall(host).ok_or_else(|| bad(CMD, format!("no wall {host}")))?;
    let face = match p.get("face").filter(|v| !v.is_null()) {
        None => room_side(&d.arch, wall),
        Some(v) => match v.as_str().map(|f| f.trim().to_ascii_lowercase()).as_deref() {
            Some("left") => Side::Left,
            Some("right") => Side::Right,
            _ => return Err(bad(CMD, "`face` must be \"left\" or \"right\"")),
        },
    };
    let material = match p.get("material").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => {
            let raw = v.as_str().ok_or_else(|| bad(CMD, "`material` must be a material key"))?;
            let m = MaterialRef::parse(raw).ok_or_else(|| bad(CMD, "`material` is not a material"))?;
            Some(m.key())
        }
    };

    if let Some(lv) = str_param(p, "level") {
        let level = d.arch.levels.iter().find(|l| l.name == lv).ok_or_else(|| bad(CMD, format!("unknown level `{lv}`")))?;
        if !wall.spans_level(level, &d.arch.levels) {
            return Err(bad(CMD, format!("wall {host} does not span level `{lv}`")));
        }
        let total = level.elevation - wall.bottom_z(&d.arch.levels) + sill;
        if !total.is_finite() || total < 0.0 {
            return Err(bad(CMD, format!("sill on level `{lv}` would be below the wall bottom")));
        }
        if wall.bottom_z(&d.arch.levels) + total + height > wall.top_z(&d.arch.levels) + 1e-9 {
            return Err(bad(CMD, format!("niche on level `{lv}` would rise above the top of wall {host}")));
        }
        sill = total;
    }
    if sill < MIN_SILL {
        return Err(bad(CMD, "the sill must be at least 0.01 m above the wall's bottom"));
    }
    if d.arch.openings.len() >= MAX_OPENINGS {
        return Err(bad(CMD, "max number of openings reached"));
    }

    let mut o = Opening::new(host, offset, width, height, OpeningKind::Niche);
    o.style = "niche".to_string();
    o.sill = sill;
    o.frame_width = 0.0;
    o.frame_depth = 0.0;
    o.casing_width = 0.0;
    o.depth = depth;
    o.face = Some(face);
    o.material = material;
    o.layer = "A-WALL".to_string();
    let mut placed = place_opening(&d.arch, o, false).map_err(|e| bad(CMD, e.to_string()))?;
    // The cut is exactly the size: place_opening wrote size + ROUGH_ALLOWANCE.
    (placed.rough_width, placed.rough_height) = (placed.width, placed.height);

    let placed_width = placed.width;
    let resized = (placed_width - width).abs() > 1e-9;
    d.ensure_layer("A-WALL");
    let id = d.arch.push_opening(placed);
    d.touch_arch();

    Ok(json!({
        "openings": [{
            "id": opening_label(id),
            "handle": handle_json(opening_handle(id)),
            "host": wall_label(host),
            "offset": offset,
            "width": placed_width,
            "resized": resized,
        }],
        "revision": d.arch_revision,
    }))
}

/// Add `depth`, `face` and `material` to the `openings` rows already in `out`. Read-only.
pub(crate) fn inspect_niche_rows(model: &ArchModel, out: &mut serde_json::Map<String, Value>) {
    let Some(Value::Array(rows)) = out.get_mut("openings") else {
        return;
    };
    for row in rows {
        let Some(o) = row
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix('o'))
            .and_then(|n| n.parse::<u32>().ok())
            .and_then(|n| model.opening(ArchId(n)))
        else {
            continue;
        };
        let face = o.face.map(|f| if f == Side::Left { "left" } else { "right" });
        if let Value::Object(m) = row {
            m.insert("depth".to_string(), json!(o.depth));
            m.insert("face".to_string(), json!(face));
            m.insert("material".to_string(), json!(o.material));
        }
    }
}

/// Command specs for the niche module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("niche", "Niche", run_niche)
            .menu(&["Architecture", "Niche"])
            .params("{wall:\"w2\", offset?|point?, width?: 0.6, height?: 0.9, sill?: 1.0, depth?: 0.1, face?: left|right, material?, level?}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walled() -> Session {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]]})).expect("wall");
        s
    }

    fn opening(s: &Session, id: u32) -> Opening {
        s.doc().expect("doc").arch.opening(ArchId(id)).cloned().expect("opening")
    }

    #[test]
    fn niche_command_defaults_and_reply() {
        let mut s = walled();
        let r = s.execute("niche", &json!({"wall": "w0", "offset": 5.0})).expect("niche");
        let one = &r["openings"][0];
        assert_eq!((one["id"].as_str(), one["host"].as_str()), (Some("o0"), Some("w0")));
        assert_eq!((one["offset"].as_f64(), one["width"].as_f64(), one["resized"].as_bool()), (Some(5.0), Some(0.6), Some(false)));
        assert!(one["handle"].is_string() && r["revision"].is_u64());
        let o = opening(&s, 0);
        assert_eq!((o.kind, o.style.as_str(), o.layer.as_str()), (OpeningKind::Niche, "niche", "A-WALL"));
        assert_eq!((o.width, o.height, o.sill, o.depth), (0.6, 0.9, 1.0, 0.1));
        assert_eq!((o.frame_width, o.casing_width, o.material), (0.0, 0.0, None));
        s.undo().expect("undo");
        assert!(s.doc().expect("doc").arch.openings.is_empty(), "one undo step");
    }

    #[test]
    fn niche_face_defaults_to_the_room_side() {
        let mut s = Session::new();
        // A clockwise loop: w0 runs up x = 0, so the room (x > 0) is on its right; w1 runs along
        // y = 5 towards +x, so the room (y < 5) is on its right as well.
        s.execute("wall", &json!({"points": [[0, 0], [0, 5], [6, 5], [6, 0], [0, 0]]})).expect("walls");
        s.execute("roomdetect", &json!({})).expect("rooms");
        assert!(!s.doc().expect("doc").arch.rooms.is_empty(), "a room is detected");
        for (w, want) in [("w0", Side::Right), ("w1", Side::Right)] {
            s.execute("niche", &json!({"wall": w, "offset": 3.0})).expect("niche");
            let n = s.doc().expect("doc").arch.openings.len();
            assert_eq!(opening(&s, (n - 1) as u32).face, Some(want), "{w}");
        }
        // An explicit face wins; a lone wall has no room, so left.
        let mut s = walled();
        s.execute("niche", &json!({"wall": "w0", "offset": 2.0})).expect("niche");
        s.execute("niche", &json!({"wall": "w0", "offset": 5.0, "face": "right"})).expect("niche");
        assert_eq!((opening(&s, 0).face, opening(&s, 1).face), (Some(Side::Left), Some(Side::Right)));
    }

    #[test]
    fn niche_refusals_leave_nothing() {
        let mut s = walled();
        let before = s.doc().expect("doc").arch_revision;
        for p in [
            json!({"wall": "w0", "offset": 5.0, "depth": 0.2}),
            json!({"wall": "w0", "offset": 5.0, "depth": 0.0}),
            json!({"wall": "w0", "offset": 5.0, "face": "up"}),
            json!({"wall": "w0", "offset": 5.0, "material": ""}),
            json!({"wall": "w0", "offset": 5.0, "width": -1.0}),
            json!({"wall": "w9", "offset": 5.0}),
            json!({"wall": "w0"}),
            json!({"wall": "w0", "offset": 50.0}),
        ] {
            assert!(s.execute("niche", &p).is_err(), "{p} should be refused");
        }
        let d = s.doc().expect("doc");
        assert!(d.arch.openings.is_empty());
        assert_eq!(d.arch_revision, before);
    }

    #[test]
    fn niche_rough_is_its_size() {
        let mut s = walled();
        s.execute("niche", &json!({"wall": "w0", "offset": 5.0, "width": 0.5, "height": 0.7, "material": "#c04020"})).expect("niche");
        let o = opening(&s, 0);
        assert_eq!((o.rough_width, o.rough_height), (0.5, 0.7));
        assert_eq!(o.material.as_deref(), Some("colour:#c04020"));
    }

    #[test]
    fn door_style_shower_defaults() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "type": "glass"})).expect("wall");
        s.execute("door", &json!({"wall": "w0", "offset": 3.0, "style": "Shower", "casing": 0.05})).expect("shower");
        let o = opening(&s, 0);
        assert_eq!((o.style.as_str(), o.width, o.height, o.sill), ("shower", 0.7, 1.9, 0.0));
        assert_eq!((o.frame_width, o.frame_depth, o.casing_width, o.rough_width, o.rough_height), (0.0, 0.0, 0.0, 0.7, 1.9));
        let mut s = walled();
        assert!(s.execute("door", &json!({"wall": "w0", "offset": 3.0, "style": "shower"})).is_err(), "not in a stud wall");
    }

    #[test]
    fn inspect_opening_rows_depth_face_material() {
        let mut s = walled();
        s.execute("niche", &json!({"wall": "w0", "offset": 2.0, "face": "right", "material": "colour:#c04020"})).expect("niche");
        s.execute("door", &json!({"wall": "w0", "offset": 6.0})).expect("door");
        let v = s.execute("arch.inspect", &json!({"openings": true})).expect("inspect");
        let rows = v["openings"].as_array().expect("rows");
        assert_eq!((rows[0]["kind"].as_str(), rows[0]["depth"].as_f64(), rows[0]["face"].as_str()), (Some("niche"), Some(0.1), Some("right")));
        assert_eq!(rows[0]["material"].as_str(), Some("colour:#c04020"));
        assert_eq!((rows[1]["depth"].as_f64(), rows[1]["face"].is_null(), rows[1]["material"].is_null()), (Some(0.0), true, true));
    }

    #[test]
    fn niche_sill_below_0_01_refused() {
        let mut s = walled();
        assert!(s.execute("niche", &json!({"wall": "w0", "offset": 5.0, "sill": 0.0})).is_err());
        assert!(s.execute("niche", &json!({"wall": "w0", "offset": 5.0, "sill": 0.005})).is_err());
        assert!(s.execute("niche", &json!({"wall": "w0", "offset": 5.0, "sill": 0.01})).is_ok());
    }
}
