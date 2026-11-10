//! The phase-17 rows of `arch.inspect`: stair, railing, room and opening detail fields.
//!
//! [`inspect_details`] walks the rows `run_inspect` already built, zipped with the model in model
//! order, and adds the phase-17 keys. Read-only.

use cadcraft_arch::railing::{built_path, post_points};
use cadcraft_arch3d::openings::glass_plane;
use cadcraft_doc::arch::{ArchModel, RailingHost, RoomType, room_label, stair_label};
use serde_json::{Map as JsonMap, Value, json};

/// Add the phase-17 fields to the `stairs`, `railings`, `rooms` and `openings` rows in `out`.
pub(crate) fn inspect_details(model: &ArchModel, out: &mut JsonMap<String, Value>) {
    if let Some(Value::Array(rows)) = out.get_mut("stairs") {
        for (row, s) in rows.iter_mut().zip(model.stairs.iter()) {
            let Value::Object(obj) = row else { continue };
            obj.insert("openRisers".to_string(), json!(s.open_risers));
            obj.insert("stringers".to_string(), json!(s.stringers.name()));
            obj.insert("underside".to_string(), json!(s.underside));
            obj.insert("railLeft".to_string(), json!(s.rail_left.name()));
            obj.insert("railRight".to_string(), json!(s.rail_right.name()));
            obj.insert("planCut".to_string(), json!(s.plan_cut));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("railings") {
        for (row, r) in rows.iter_mut().zip(model.railings.iter()) {
            let Value::Object(obj) = row else { continue };
            let posts: Vec<Value> = post_points(&built_path(&r.path, r.inset), r.post_spacing).iter().map(|p| json!([p.x, p.y])).collect();
            let follows = r.follows.map(|id| if r.host == RailingHost::Deck { room_label(id) } else { stair_label(id) });
            obj.insert("postSpacing".to_string(), json!(r.post_spacing));
            obj.insert("inset".to_string(), json!(r.inset));
            obj.insert("host".to_string(), json!(r.follows.map(|_| r.host.name())));
            obj.insert("auto".to_string(), json!(r.auto));
            obj.insert("posts".to_string(), Value::Array(posts));
            obj.insert("follows".to_string(), json!(follows));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("rooms") {
        for (row, r) in rows.iter_mut().zip(model.rooms.iter()) {
            let Value::Object(obj) = row else { continue };
            if r.room_type != RoomType::Deck {
                continue;
            }
            let rails: Vec<&str> = r.deck_rails.iter().map(|s| s.name()).collect();
            obj.insert("boards".to_string(), json!(r.deck_boards));
            obj.insert("rails".to_string(), json!(rails));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("openings") {
        for (row, o) in rows.iter_mut().zip(model.openings.iter()) {
            let Value::Object(obj) = row else { continue };
            let grille = o.grille.map(|g| json!({"across": g.across, "down": g.down, "part": if g.top_only { "top" } else { "whole" }}));
            let glass = glass_plane(model, o);
            obj.insert("top".to_string(), json!(o.top.name()));
            obj.insert("topRise".to_string(), json!(o.top_rise));
            obj.insert("roofOffset".to_string(), json!(o.roof_offset));
            obj.insert("grille".to_string(), json!(grille));
            obj.insert("sillSides".to_string(), json!(o.sill_sides.name()));
            obj.insert("casingSides".to_string(), json!(o.casing_sides.name()));
            obj.insert("glassCentre".to_string(), json!(glass.map(|g| [g.centre.x, g.centre.y, g.centre.z])));
            obj.insert("glassNormal".to_string(), json!(glass.map(|g| [g.normal.x, g.normal.y])));
        }
    }
}
