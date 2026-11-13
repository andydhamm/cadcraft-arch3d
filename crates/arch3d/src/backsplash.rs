//! Backsplash meshes.

use cadcraft_arch::backsplash::BacksplashSegment;
use cadcraft_doc::arch::Backsplash;

use crate::mesh::Mesh;

/// The meshes of one backsplash (`backsplash` material, object = the backsplash) over its resolved
/// `segments`, `level` being its level's elevation.
///
/// One mesh with a box per segment: the plan quad runs along the wall face `a -> b` and out into
/// the room (to the left of `a -> b`, the counter outline's side) by the thickness, from `bottom`
/// to `top`. Nothing is built for a bad thickness or when no segment survives.
pub fn backsplash_meshes(b: &Backsplash, segments: &[BacksplashSegment], level: f64) -> Vec<Mesh> {
    if !b.thickness.is_finite() || b.thickness <= 0.0 {
        return Vec::new();
    }
    let mut mesh = Mesh::new("backsplash", b.id);
    mesh.level_elevation = level;
    for s in segments {
        let along = s.b - s.a;
        if !(s.a.is_finite() && s.b.is_finite() && s.bottom.is_finite() && s.top.is_finite()) || along.len() < 1e-9 || s.top <= s.bottom {
            continue;
        }
        let out = along.normalized().perp() * b.thickness;
        mesh.push_prism(&[s.a, s.b, s.b + out, s.a + out], s.bottom, s.top);
    }
    if mesh.indices.is_empty() { Vec::new() } else { vec![mesh] }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_geom::Vec2;

    fn seg(x0: f64, x1: f64, top: f64) -> BacksplashSegment {
        BacksplashSegment { a: Vec2::new(x0, 0.0), b: Vec2::new(x1, 0.0), bottom: 0.9, top, ..Default::default() }
    }

    #[test]
    fn the_stub_builds_nothing() {
        assert!(backsplash_meshes(&Backsplash::default(), &[], 0.0).is_empty());
    }

    #[test]
    fn one_mesh_with_a_box_per_segment_into_the_room() {
        let b = Backsplash { id: ArchId(3), ..Default::default() };
        let m = backsplash_meshes(&b, &[seg(0.0, 1.0, 1.3), seg(1.0, 2.0, 1.4)], 0.0);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].material, "backsplash");
        assert_eq!(m[0].object, ArchId(3));
        assert_eq!(m[0].triangle_count(), 24);
        let bb = m[0].bounds();
        assert!((bb.min[1] - 0.0).abs() < 1e-6 && (bb.max[1] - 0.01).abs() < 1e-6, "{bb:?}");
        assert!((bb.max[2] - 1.4).abs() < 1e-6 && (bb.min[2] - 0.9).abs() < 1e-6);
    }

    #[test]
    fn nothing_for_no_segments_or_a_bad_thickness() {
        assert!(backsplash_meshes(&Backsplash::default(), &[], 0.0).is_empty());
        let bad = Backsplash { thickness: f64::NAN, ..Default::default() };
        assert!(backsplash_meshes(&bad, &[seg(0.0, 1.0, 1.3)], 0.0).is_empty());
        let flat = BacksplashSegment { top: 0.5, ..seg(0.0, 1.0, 1.3) };
        assert!(backsplash_meshes(&Backsplash::default(), &[flat], 0.0).is_empty());
    }
}
