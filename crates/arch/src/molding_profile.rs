//! Molding profiles generated in Rust: the five built-in cross-sections, validation, scaling and
//! the conversion of a drawn closed polyline into a profile.
//!
//! A profile is a closed counter-clockwise outline in metres: x runs out from the surface the
//! molding is fixed to, y runs up, and the bounding box starts at (0, 0).

use crate::poly::is_simple;
use cadcraft_doc::arch::{MAX_PROFILE_POINTS, MoldingKind, MoldingProfile};
use cadcraft_geom::Vec2;

/// The built-in profile ids and their display names.
pub const BUILTIN: [(&str, &str); 5] = [
    ("molding:base", "Base molding"),
    ("molding:shoe", "Shoe molding"),
    ("molding:crown", "Crown molding"),
    ("molding:chair-rail", "Chair rail"),
    ("molding:casing", "Casing"),
];

/// Smallest width or height of a profile, metres.
const MIN_SIZE: f64 = 0.002;
/// Largest width or height of a profile, metres.
const MAX_SIZE: f64 = 0.5;

/// The outline of the built-in profile `id`, or `None` for any other id.
pub fn builtin_profile(id: &str) -> Option<MoldingProfile> {
    let points: Vec<[f64; 2]> = match id {
        "molding:base" => vec![[0.0, 0.0], [0.015, 0.0], [0.015, 0.075], [0.0125, 0.083], [0.0085, 0.088], [0.004, 0.09], [0.0, 0.09]],
        "molding:shoe" => {
            let mut v = vec![[0.0, 0.0], [0.013, 0.0]];
            for deg in [15.0f64, 30.0, 45.0, 60.0, 75.0] {
                let t = deg.to_radians();
                v.push([0.013 * t.cos(), 0.019 * t.sin()]);
            }
            v.push([0.0, 0.019]);
            v
        }
        "molding:crown" => vec![
            [0.0, 0.0],
            [0.012, 0.0],
            [0.02, 0.012],
            [0.035, 0.03],
            [0.06, 0.05],
            [0.08, 0.07],
            [0.088, 0.085],
            [0.1, 0.088],
            [0.1, 0.1],
            [0.0, 0.1],
        ],
        "molding:chair-rail" => vec![[0.0, 0.0], [0.012, 0.0], [0.019, 0.012], [0.019, 0.052], [0.012, 0.064], [0.0, 0.064]],
        "molding:casing" => vec![[0.0, 0.0], [0.012, 0.0], [0.016, 0.008], [0.016, 0.064], [0.0, 0.064]],
        _ => return None,
    };
    Some(MoldingProfile { item: id.to_string(), points })
}

/// The default profile of a molding kind (base, crown or chair rail).
pub fn default_profile(kind: MoldingKind) -> MoldingProfile {
    let id = match kind {
        MoldingKind::Base => "molding:base",
        MoldingKind::Crown => "molding:crown",
        MoldingKind::Chair => "molding:chair-rail",
    };
    builtin_profile(id).unwrap_or_default()
}

/// Check a profile outline: 3..=`MAX_PROFILE_POINTS` finite points, a simple counter-clockwise ring
/// of area above 1e-8 whose bounding box starts at (0, 0) and measures 2 mm to 0.5 m each way.
pub fn validate_profile(points: &[[f64; 2]]) -> Result<(), String> {
    if points.len() < 3 || points.len() > MAX_PROFILE_POINTS {
        return Err(format!("a molding profile has 3 to {MAX_PROFILE_POINTS} points, not {}", points.len()));
    }
    if points.iter().any(|p| !(p[0].is_finite() && p[1].is_finite())) {
        return Err("a molding profile point is not a finite number".to_string());
    }
    let ring: Vec<Vec2> = points.iter().map(|p| Vec2::new(p[0], p[1])).collect();
    if !is_simple(&ring) {
        return Err("a molding profile must not cross itself".to_string());
    }
    let mut twice_area = 0.0;
    for (i, a) in points.iter().enumerate() {
        let b = points.get((i + 1) % points.len()).unwrap_or(a);
        twice_area += a[0] * b[1] - b[0] * a[1];
    }
    let area = twice_area / 2.0;
    if area.abs() <= 1e-8 {
        return Err("a molding profile has no area".to_string());
    }
    if area < 0.0 {
        return Err("a molding profile must run counter-clockwise".to_string());
    }
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    if min[0].abs() > 1e-9 || min[1].abs() > 1e-9 {
        return Err("a molding profile's bounding box must start at (0, 0)".to_string());
    }
    for (name, v) in [("width", max[0] - min[0]), ("height", max[1] - min[1])] {
        if !(MIN_SIZE..=MAX_SIZE).contains(&v) {
            return Err(format!("a molding profile's {name} must be between {MIN_SIZE} and {MAX_SIZE} m, not {v}"));
        }
    }
    Ok(())
}

/// The profile stretched to `size` = [width, height] (metres, each in 0.002 to 0.5); same item id.
pub fn scaled(p: &MoldingProfile, size: [f64; 2]) -> Result<MoldingProfile, String> {
    for v in size {
        if !v.is_finite() || !(MIN_SIZE..=MAX_SIZE).contains(&v) {
            return Err(format!("a molding size must be between {MIN_SIZE} and {MAX_SIZE} m, not {v}"));
        }
    }
    let [w, h] = p.size();
    if w <= 0.0 || h <= 0.0 {
        return Err("the molding profile has no size".to_string());
    }
    let points = p.points.iter().map(|q| [q[0] * size[0] / w, q[1] * size[1] / h]).collect();
    Ok(MoldingProfile { item: p.item.clone(), points })
}

/// A profile outline from a closed polyline's points: drop a closing point equal to the first,
/// move the minimum to (0, 0), reverse when clockwise, then validate.
pub fn profile_from_polyline(points: &[Vec2]) -> Result<Vec<[f64; 2]>, String> {
    if points.iter().any(|p| !p.is_finite()) {
        return Err("a molding profile point is not a finite number".to_string());
    }
    let mut v: Vec<Vec2> = points.to_vec();
    if v.len() > 1 {
        if let (Some(f), Some(l)) = (v.first().copied(), v.last().copied()) {
            if (f.x - l.x).abs() <= 1e-9 && (f.y - l.y).abs() <= 1e-9 {
                v.pop();
            }
        }
    }
    let min_x = v.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let min_y = v.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    let mut out: Vec<[f64; 2]> = v.iter().map(|p| [p.x - min_x, p.y - min_y]).collect();
    let mut twice_area = 0.0;
    for (i, a) in out.iter().enumerate() {
        let b = out.get((i + 1) % out.len()).unwrap_or(a);
        twice_area += a[0] * b[1] - b[0] * a[1];
    }
    if twice_area < 0.0 {
        out.reverse();
    }
    validate_profile(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_validate_and_have_the_brief_sizes() {
        let expect = [
            ("molding:base", [0.015, 0.09]),
            ("molding:shoe", [0.013, 0.019]),
            ("molding:crown", [0.1, 0.1]),
            ("molding:chair-rail", [0.019, 0.064]),
            ("molding:casing", [0.016, 0.064]),
        ];
        for (id, size) in expect {
            let p = builtin_profile(id).unwrap();
            assert_eq!(p.item, id);
            assert_eq!(validate_profile(&p.points), Ok(()), "{id}");
            let s = p.size();
            assert!((s[0] - size[0]).abs() < 1e-9 && (s[1] - size[1]).abs() < 1e-9, "{id}: {s:?}");
        }
        assert_eq!(BUILTIN.len(), 5);
        for (id, _) in BUILTIN {
            assert!(builtin_profile(id).is_some());
        }
        assert!(builtin_profile("molding:nope").is_none());
    }

    #[test]
    fn scaled_profile_has_the_new_bounding_box() {
        let p = builtin_profile("molding:crown").unwrap();
        let q = scaled(&p, [0.05, 0.2]).unwrap();
        assert_eq!(q.item, p.item);
        let s = q.size();
        assert!((s[0] - 0.05).abs() < 1e-9 && (s[1] - 0.2).abs() < 1e-9);
        assert_eq!(validate_profile(&q.points), Ok(()));
        assert!(scaled(&p, [0.0, 0.1]).is_err());
        assert!(scaled(&p, [0.1, 0.6]).is_err());
        assert!(scaled(&p, [f64::NAN, 0.1]).is_err());
    }

    #[test]
    fn validate_refuses_bad_profiles() {
        let sq = [[0.0, 0.0], [0.02, 0.0], [0.02, 0.02], [0.0, 0.02]];
        assert_eq!(validate_profile(&sq), Ok(()));
        assert!(validate_profile(&sq[..2]).is_err());
        let cw = [[0.0, 0.0], [0.0, 0.02], [0.02, 0.02], [0.02, 0.0]];
        assert!(validate_profile(&cw).is_err());
        let bow = [[0.0, 0.0], [0.02, 0.02], [0.02, 0.0], [0.0, 0.02]];
        assert!(validate_profile(&bow).is_err());
        let nan = [[0.0, 0.0], [f64::NAN, 0.0], [0.02, 0.02], [0.0, 0.02]];
        assert!(validate_profile(&nan).is_err());
        let off = [[0.01, 0.0], [0.03, 0.0], [0.03, 0.02], [0.01, 0.02]];
        assert!(validate_profile(&off).is_err());
        let thin = [[0.0, 0.0], [0.001, 0.0], [0.001, 0.02], [0.0, 0.02]];
        assert!(validate_profile(&thin).is_err());
        let big = [[0.0, 0.0], [0.6, 0.0], [0.6, 0.02], [0.0, 0.02]];
        assert!(validate_profile(&big).is_err());
        let many: Vec<[f64; 2]> = (0..=MAX_PROFILE_POINTS).map(|i| [i as f64 * 1e-4, 0.0]).collect();
        assert!(validate_profile(&many).is_err());
    }

    #[test]
    fn profile_from_polyline_normalises() {
        let v = |x: f64, y: f64| Vec2::new(x, y);
        // Clockwise, offset, with a closing point.
        let pts = [v(5.0, 5.0), v(5.0, 5.03), v(5.04, 5.03), v(5.04, 5.0), v(5.0, 5.0)];
        let out = profile_from_polyline(&pts).unwrap();
        assert_eq!(out.len(), 4);
        assert_eq!(validate_profile(&out), Ok(()));
        let w = out.iter().map(|p| p[0]).fold(0.0, f64::max);
        assert!((w - 0.04).abs() < 1e-9);
        let bow = [v(0.0, 0.0), v(0.02, 0.02), v(0.02, 0.0), v(0.0, 0.02)];
        assert!(profile_from_polyline(&bow).is_err());
    }

    #[test]
    fn default_profile_per_kind() {
        assert_eq!(default_profile(MoldingKind::Base).item, "molding:base");
        assert_eq!(default_profile(MoldingKind::Crown).item, "molding:crown");
        assert_eq!(default_profile(MoldingKind::Chair).item, "molding:chair-rail");
    }
}
