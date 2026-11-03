use cadcraft_doc::arch::{Furnishing, FurnishingHost, Surface};
use cadcraft_geom::{Vec2, Vec3, norm_angle};

use super::{Entity, Handle, Mat3, Result, Session, Value, bad, bool_or, erase_kept, f64_req, json, point_param, point_req, targets};

// ---------------- core operations ----------------

pub(crate) fn transform_entities(s: &mut Session, hs: &[Handle], m: &Mat3, copy: bool) -> Result<Vec<Handle>> {
    let mut ordinary = Vec::new();
    let mut furnishing = Vec::new();
    {
        let d = s.doc()?;
        for (idx, h) in hs.iter().copied().enumerate() {
            if cadcraft_doc::arch::is_furnishing_handle(h) {
                if let Some(id) = cadcraft_doc::arch::furnishing_id_of_handle(h) {
                    if d.arch.furnishing(id).is_some() {
                        furnishing.push((idx, id));
                        continue;
                    }
                }
            }
            ordinary.push((idx, h));
        }
    }

    let mut out = vec![None; hs.len()];
    if !ordinary.is_empty() {
        let ordinary_hs: Vec<Handle> = ordinary.iter().map(|(_, h)| *h).collect();
        let ordinary_out = edit_entities(s, &ordinary_hs, copy, |e| e.kind.transform(m))?;
        for ((idx, _), handle) in ordinary.iter().zip(ordinary_out) {
            out[*idx] = Some(handle);
        }
    }

    if !furnishing.is_empty() {
        let mut doc = s.doc_mut()?;
        for (idx, id) in furnishing {
            let mut f = doc.arch.furnishing_mut(id).cloned().unwrap();
            let new_at = m.apply(Vec2::new(f.at[0], f.at[1]));
            let old_rotation = f.rotation.to_radians();
            let mirror = m.det() < 0.0;
            let mut new_rotation = norm_angle(old_rotation + m.rotation());
            let mut mirrored = f.mirrored;
            if mirror {
                mirrored = !mirrored;
                let line_angle = 0.5 * m.rotation();
                new_rotation = norm_angle(2.0 * line_angle - old_rotation);
            }

            if copy {
                let mut fresh = f.clone();
                fresh.at = [new_at.x, new_at.y];
                fresh.rotation = new_rotation.to_degrees();
                fresh.mirrored = mirrored;
                fresh.host = FurnishingHost::None;
                let new_id = doc.arch.push_furnishing(fresh);
                let mut cloned = Vec::new();
                for (key, paint) in doc.arch.paints.iter() {
                    let Some(surface) = Surface::parse(key) else { continue };
                    if let Surface::Furnishing { furnishing: old, part } = surface {
                        if old == id {
                            cloned.push((Surface::Furnishing { furnishing: new_id, part }.key(), paint.clone()));
                        }
                    }
                }
                for (key, paint) in cloned {
                    doc.arch.paints.insert(key, paint);
                }
                out[idx] = Some(cadcraft_doc::arch::furnishing_handle(new_id));
            } else {
                let f = doc.arch.furnishing_mut(id).unwrap();
                f.at = [new_at.x, new_at.y];
                f.rotation = new_rotation.to_degrees();
                f.mirrored = mirrored;
                f.host = FurnishingHost::None;
                out[idx] = Some(cadcraft_doc::arch::furnishing_handle(id));
            }
        }
        doc.touch_arch();
    }

    Ok(out.into_iter().flatten().collect())
}

/// Scale `hs` by `f` about `base` in 3D, as AutoCAD 2024's SCALE does: about the base point's XY,
/// and every height of the objects about its Z, their thickness too.
pub(crate) fn scale_entities(s: &mut Session, hs: &[Handle], base: Vec3, f: f64, copy: bool) -> Result<Vec<Handle>> {
    if hs.iter().any(|h| cadcraft_doc::arch::is_furnishing_handle(*h)) {
        return Err(bad("scale", "scale a furnishing with furnishing.edit size"));
    }
    let m = Mat3::scale_about(base.xy(), f);
    edit_entities(s, hs, copy, |e| {
        e.kind.transform(&m);
        e.kind.scale_heights(base.z, f);
        e.common.thickness *= f;
    })
}

#[cfg(test)]
mod tests {
    use cadcraft_doc::Handle;
    use cadcraft_doc::arch::{ArchId, Furnishing, furnishing_handle};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    use super::*;

    fn add_furnishing(s: &mut Session, at: [f64; 2], rotation: f64) -> ArchId {
        s.doc_mut().unwrap().arch.push_furnishing(Furnishing {
            item: "armchair".into(),
            symbol: "sym".into(),
            name: "Armchair".into(),
            category: "Seating".into(),
            symbol_type: "chair".into(),
            at,
            rotation,
            size: [1.0, 1.0, 0.9],
            ..Default::default()
        })
    }

    #[test]
    fn move_moves_a_furnishing() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [1.0, 2.0], 0.0);
        let h = furnishing_handle(id);
        let moved = transform_entities(&mut s, &[h], &Mat3::translate(Vec2::new(5.0, 7.0)), false).unwrap();
        assert_eq!(moved, vec![h]);
        assert_eq!(s.doc().unwrap().arch.furnishing(id).unwrap().at, [6.0, 9.0]);
    }

    #[test]
    fn copy_count_makes_copies_at_the_interval() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [0.0, 0.0], 0.0);
        let h = furnishing_handle(id);
        let out = s.execute("copy", &json!({"handles": [h.hex()], "from": [0.0, 0.0], "to": [10.0, 0.0], "count": 3})).unwrap();
        let handles: Vec<Handle> = out["handles"].as_array().unwrap().iter().map(|v| Handle::parse_hex(v.as_str().unwrap()).unwrap()).collect();
        let positions: Vec<[f64; 2]> = handles
            .iter()
            .map(|h| {
                let id = cadcraft_doc::arch::furnishing_id_of_handle(*h).unwrap();
                s.doc().unwrap().arch.furnishing(id).unwrap().at
            })
            .collect();
        assert_eq!(positions, vec![[10.0, 0.0], [20.0, 0.0], [30.0, 0.0]]);
    }

    #[test]
    fn rotate_about_a_base_turns_a_furnishing() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [2.0, 0.0], 0.0);
        let h = furnishing_handle(id);
        transform_entities(&mut s, &[h], &Mat3::rotate_about(Vec2::new(0.0, 0.0), std::f64::consts::FRAC_PI_2), false).unwrap();
        let f = s.doc().unwrap().arch.furnishing(id).unwrap();
        assert!((f.at[0]).abs() < 1e-9);
        assert!((f.at[1] - 2.0).abs() < 1e-9);
        assert!((f.rotation - 90.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_reflects_and_marks_mirrored() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [3.0, 0.0], 0.0);
        let h = furnishing_handle(id);
        transform_entities(&mut s, &[h], &Mat3::mirror(Vec2::new(0.0, 0.0), Vec2::new(0.0, 3.0)), false).unwrap();
        let f = s.doc().unwrap().arch.furnishing(id).unwrap();
        assert_eq!(f.at, [-3.0, 0.0]);
        assert!(f.mirrored);
        assert!((f.rotation - 180.0).abs() < 1e-9);
    }

    #[test]
    fn scale_refuses_furnishings() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [1.0, 1.0], 0.0);
        let h = furnishing_handle(id);
        let err = s.execute("scale", &json!({"handles": [h.hex()], "base": [0.0, 0.0], "factor": 2.0})).unwrap_err();
        assert!(err.to_string().contains("scale a furnishing with furnishing.edit size"));
        assert_eq!(s.doc().unwrap().arch.furnishing(id).unwrap().at, [1.0, 1.0]);
    }

    #[test]
    fn transforms_are_one_undo_step() {
        let mut s = Session::new();
        let id = add_furnishing(&mut s, [2.0, 0.0], 0.0);
        let before = s.state().unwrap().undo.len();
        let h = furnishing_handle(id);
        s.execute("move", &json!({"handles": [h.hex()], "delta": [10.0, 0.0]})).unwrap();
        assert_eq!(s.state().unwrap().undo.len(), before + 1);
    }
}

/// `p` at the current elevation (ELEVATION), the Z AutoCAD gives a point entered without one.
pub(crate) fn at_elevation(s: &Session, p: Vec2) -> Vec3 {
    let z = s.doc().map(|d| d.header.f64("ELEVATION", 0.0)).ok().filter(|z| z.is_finite());
    Vec3::new(p.x, p.y, z.unwrap_or(0.0))
}

/// Apply `edit` to each of `hs` still in the drawing, or to a copy of each when `copy`; an object
/// on a locked layer is left alone unless it is copied. The handles edited or made.
fn edit_entities(s: &mut Session, hs: &[Handle], copy: bool, edit: impl Fn(&mut Entity)) -> Result<Vec<Handle>> {
    let d = s.doc_mut()?;
    let mut out = Vec::new();
    for h in hs {
        let Some(sp) = d.space_of(*h) else { continue };
        if copy {
            let Some(e) = d.entity(*h).map(|e| (**e).clone()) else { continue };
            let nh = d.new_handle();
            let mut ne = Entity { handle: nh, ..e };
            edit(&mut ne);
            if let Some(st) = d.space_mut(&sp) {
                st.push(ne);
                out.push(nh);
            }
        } else {
            let locked = d.entity(*h).and_then(|e| d.layer(&e.common.layer)).is_some_and(|l| l.locked);
            if locked {
                continue;
            }
            d.modify_entity(*h, &edit)?;
            out.push(*h);
        }
    }
    Ok(out)
}

pub(super) fn erase(s: &mut Session, hs: &[Handle]) -> Result<usize> {
    let d = s.doc()?;
    let free: Vec<Handle> = hs.iter().copied().filter(|h| !d.entity(*h).and_then(|e| d.layer(&e.common.layer)).is_some_and(|l| l.locked)).collect();
    let n = erase_kept(s, &free)?;
    s.set_selection(Vec::new());
    Ok(n)
}

pub(super) fn run_erase(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let n = erase(s, &hs)?;
    Ok(json!({ "erased": n }))
}

pub(super) fn delta_of(cmd: &str, p: &Value) -> Result<Vec2> {
    if let Some(d) = point_param(p, "delta") {
        return Ok(d);
    }
    let a = point_req(cmd, p, "from")?;
    let b = point_req(cmd, p, "to")?;
    Ok(b - a)
}

pub(super) fn run_move(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let d = delta_of("move", p)?;
    let moved = transform_entities(s, &hs, &Mat3::translate(d), false)?;
    Ok(json!({ "moved": moved.len() }))
}

pub(super) fn run_copy(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let d = delta_of("copy", p)?;
    let n = match p.get("count") {
        None => 1,
        Some(v) => v.as_u64().filter(|n| (1..=10_000).contains(n)).ok_or_else(|| bad("copy", "`count` must be an integer from 1 to 10000"))?,
    };
    let mut out = Vec::new();
    for k in 1..=n {
        out.extend(transform_entities(s, &hs, &Mat3::translate(d * k as f64), true)?.into_iter().map(|h| h.hex()));
    }
    Ok(json!({ "handles": out }))
}

pub(super) fn run_rotate(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let base = point_req("rotate", p, "base")?;
    let a = f64_req("rotate", p, "angle")?.to_radians();
    let r = transform_entities(s, &hs, &Mat3::rotate_about(base, a), bool_or(p, "copy", false))?;
    Ok(json!({ "handles": r.iter().map(|h| h.hex()).collect::<Vec<_>>() }))
}

pub(super) fn run_scale(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let base = point_req("scale", p, "base")?;
    let f = f64_req("scale", p, "factor")?;
    if f <= 0.0 {
        return Err(bad("scale", "factor must be positive"));
    }
    // A Z given with the base point, as AutoCAD takes one; without one, the current elevation.
    let base = match p.get("base").and_then(|b| b.get(2)) {
        Some(z) => z.as_f64().map(|z| Vec3::new(base.x, base.y, z)).ok_or_else(|| bad("scale", "`base` Z must be a number"))?,
        None => at_elevation(s, base),
    };
    let r = scale_entities(s, &hs, base, f, bool_or(p, "copy", false))?;
    Ok(json!({ "handles": r.iter().map(|h| h.hex()).collect::<Vec<_>>() }))
}

pub(super) fn run_mirror(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let a = point_req("mirror", p, "p1")?;
    let b = point_req("mirror", p, "p2")?;
    if a.near(b, cadcraft_geom::EPS) {
        return Err(bad("mirror", "mirror line points coincide"));
    }
    let erase_src = bool_or(p, "erase", false);
    let r = transform_entities(s, &hs, &Mat3::mirror(a, b), !erase_src)?;
    Ok(json!({ "handles": r.iter().map(|h| h.hex()).collect::<Vec<_>>() }))
}
