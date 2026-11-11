//! Opening trim widths for molding breaks (phase 18).

use cadcraft_doc::arch::{ArchModel, Opening, OpeningSides, Side};

/// Whether the opening's casing is built on the given face of its host wall.
///
/// The casing is built when `casing_width` is finite and positive, and on the face
/// matching `side` in `casing_sides` (default RIGHT: the wall's right face, the
/// exterior of a counter-clockwise ring).
pub fn casing_on_face(o: &Opening, side: Side) -> bool {
    let c = o.casing_width;
    if !c.is_finite() || c <= 0.0 {
        return false;
    }
    match side {
        Side::Left => o.casing_sides.left,
        Side::Right => o.casing_sides.right,
    }
}

/// The trim interval along the host wall from its start: [s0, s1].
///
/// The jambs are centred on ±width/2 and `frame_width` wide; the casing adds
/// `casing_width` on the matching face. Returns None for non-finite input.
pub fn trim_interval(model: &ArchModel, o: &Opening, side: Side) -> Option<(f64, f64)> {
    let wall = model.wall(o.host)?;
    let w = o.width;
    let fw = if o.frame_width.is_finite() { o.frame_width.max(0.0) } else { 0.0 };
    let c = if casing_on_face(o, side) { o.casing_width } else { 0.0 };
    let half = w / 2.0 + fw / 2.0 + c;
    let s0 = o.offset - half;
    let s1 = o.offset + half;
    if !s0.is_finite() || !s1.is_finite() {
        return None;
    }
    Some((s0, s1))
}

/// The top of the trim above the sill: height + casing width when built on that face.
pub fn trim_height(o: &Opening, side: Side) -> f64 {
    let c = if casing_on_face(o, side) { o.casing_width } else { 0.0 };
    o.height + c
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Opening, OpeningKind, Wall};
    use cadcraft_geom::Vec2;

    #[test]
    fn door_trim_is_width_plus_frame_plus_face_casings() {
        let mut model = ArchModel::default();
        let w = model.push_wall(Wall { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0), thickness: 0.2, ..Wall::default() });
        let o = Opening {
            host: w,
            offset: 5.0,
            width: 0.9,
            height: 2.1,
            kind: OpeningKind::Door,
            frame_width: 0.06,
            casing_width: 0.05,
            casing_sides: OpeningSides { left: false, right: true },
            ..Opening::default()
        };
        model.push_opening(o.clone());

        // Right face: casing present
        let (s0, s1) = trim_interval(&model, &o, Side::Right).unwrap();
        assert!((s0 - (5.0 - 0.9 / 2.0 - 0.06 / 2.0 - 0.05)).abs() < 1e-9);
        assert!((s1 - (5.0 + 0.9 / 2.0 + 0.06 / 2.0 + 0.05)).abs() < 1e-9);
        assert!((s1 - s0 - (0.9 + 0.06 + 0.1)).abs() < 1e-9);

        // Left face: no casing
        let (s0, s1) = trim_interval(&model, &o, Side::Left).unwrap();
        assert!((s0 - (5.0 - 0.9 / 2.0 - 0.06 / 2.0)).abs() < 1e-9);
        assert!((s1 - (5.0 + 0.9 / 2.0 + 0.06 / 2.0)).abs() < 1e-9);
        assert!((s1 - s0 - (0.9 + 0.06)).abs() < 1e-9);
    }

    #[test]
    fn hostile_openings_have_no_trim() {
        let mut model = ArchModel::default();
        let w = model.push_wall(Wall { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0), thickness: 0.2, ..Wall::default() });

        // NaN width
        let o = Opening { host: w, offset: 5.0, width: f64::NAN, height: 2.1, ..Opening::default() };
        assert!(trim_interval(&model, &o, Side::Right).is_none());

        // NaN offset
        let o = Opening { host: w, offset: f64::NAN, width: 0.9, height: 2.1, ..Opening::default() };
        assert!(trim_interval(&model, &o, Side::Right).is_none());

        // Negative frame width treated as 0
        let o = Opening { host: w, offset: 5.0, width: 0.9, height: 2.1, frame_width: -0.1, casing_width: 0.0, ..Opening::default() };
        let (s0, s1) = trim_interval(&model, &o, Side::Right).unwrap();
        assert!((s1 - s0 - 0.9).abs() < 1e-9);
    }
}
