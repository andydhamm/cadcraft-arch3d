//! Free molding lines: validation, resolved path runs and the run's offset geometry.

use crate::molding_runs::MoldingRun;
use cadcraft_doc::arch::{ArchModel, MoldingKind, MoldingLine};
use cadcraft_geom::Vec2;

const MAX_PATH_POINTS: usize = 1024;
const DUP_TOL: f64 = 1e-9;
const MAX_COORD: f64 = 1e6;
const MITRE_LIMIT_RADS: f64 = 174.0 * std::f64::consts::PI / 180.0;

/// Validate a free molding line: point count, finite coordinates, valid profile, legal inset and no
/// turns sharper than 174°.
pub fn validate(l: &MoldingLine) -> Result<(), String> {
    let mut pts = l.points.clone();
    pts = drop_consecutive_duplicates(pts, l.closed);
    if l.closed {
        if pts.len() < 3 || pts.len() > MAX_PATH_POINTS {
            return Err(format!("a molding line has 3 to {MAX_PATH_POINTS} points when closed, not {}", pts.len()));
        }
    } else if pts.len() < 2 || pts.len() > MAX_PATH_POINTS {
        return Err(format!("a molding line has 2 to {MAX_PATH_POINTS} points when open, not {}", pts.len()));
    }
    if pts.iter().any(|p| !p.is_finite()) {
        return Err("a molding line point is not finite".to_string());
    }
    if pts.iter().any(|p| p.x.abs() > MAX_COORD || p.y.abs() > MAX_COORD) {
        return Err(format!("a molding line point is outside ±{MAX_COORD}"));
    }
    if !l.elevation.is_finite() || !(-1.0..=10.0).contains(&l.elevation) {
        return Err("a molding line elevation must be finite between -1 and 10 m".to_string());
    }
    if !l.inset.is_finite() || l.inset.abs() > 1.0 {
        return Err("a molding line inset must be finite and within ±1 m".to_string());
    }
    if crate::molding_profile::validate_profile(&l.profile.points).is_err() {
        return Err("a molding line profile is not valid".to_string());
    }
    let n = pts.len();
    if n < 2 {
        return Err("a molding line needs at least two points".to_string());
    }

    for i in 0..n {
        if l.closed || (i > 0 && i < n - 1) {
            let a = if l.closed { pts[(i + n - 1) % n] } else { pts[i - 1] };
            let b = pts[i];
            let c = if l.closed { pts[(i + 1) % n] } else { pts[i + 1] };
            let before = segment_dir(a, b);
            let after = segment_dir(b, c);
            if !(before.is_finite() && after.is_finite()) {
                return Err("a molding line turn is not finite".to_string());
            }
            let dot = before.dot(after).clamp(-1.0, 1.0);
            let turn = dot.acos();
            if turn > MITRE_LIMIT_RADS {
                return Err("a molding line turn is sharper than 174 degrees".to_string());
            }
        }
    }
    Ok(())
}

/// Resolved run of a molding line at world `level_elevation`.
pub fn line_run_at(l: &MoldingLine, level_elevation: f64) -> Option<MoldingRun> {
    if validate(l).is_err() {
        return None;
    }
    let path = offset_path(&l.points, l.closed, l.inset);
    if path.len() < if l.closed { 3 } else { 2 } {
        return None;
    }
    let z = vec![level_elevation + l.elevation; path.len()];
    Some(MoldingRun {
        kind: MoldingKind::Chair,
        index: 0,
        profile: l.profile.clone(),
        path,
        z,
        closed: l.closed,
        side: if l.flip { -1.0 } else { 1.0 },
        ..Default::default()
    })
}

/// Resolved run of `l` on `model`'s `l.level`.
pub fn line_run(model: &ArchModel, l: &MoldingLine) -> Option<MoldingRun> {
    let level_elevation = model.level(l.level).map_or(0.0, |level| level.elevation);
    line_run_at(l, level_elevation)
}

fn drop_consecutive_duplicates(mut pts: Vec<Vec2>, closed: bool) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts.drain(..) {
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() <= DUP_TOL && (last.y - p.y).abs() <= DUP_TOL {
                continue;
            }
        }
        out.push(p);
    }
    if closed && out.len() > 1 {
        if let (Some(first), Some(last)) = (out.first().copied(), out.last().copied()) {
            if (first.x - last.x).abs() <= DUP_TOL && (first.y - last.y).abs() <= DUP_TOL {
                out.pop();
            }
        }
    }
    out
}

fn segment_dir(a: Vec2, b: Vec2) -> Vec2 {
    let d = b - a;
    let len = d.len();
    if len <= DUP_TOL || !len.is_finite() { Vec2::ZERO } else { d / len }
}

fn offset_path(points: &[Vec2], closed: bool, inset: f64) -> Vec<Vec2> {
    let pts = drop_consecutive_duplicates(points.to_vec(), closed);
    if pts.len() < (if closed { 3 } else { 2 }) {
        return Vec::new();
    }
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let curr = pts[i];
        let (prev, next) = if closed {
            (pts[(i + n - 1) % n], pts[(i + 1) % n])
        } else {
            if i == 0 {
                (pts[0], pts[1])
            } else if i == n - 1 {
                (pts[n - 2], pts[n - 1])
            } else {
                (pts[i - 1], pts[i + 1])
            }
        };
        let offset = if closed || i > 0 && i < n - 1 {
            let d0 = segment_dir(prev, curr);
            let d1 = segment_dir(curr, next);
            let left0 = Vec2::new(-d0.y, d0.x);
            let left1 = Vec2::new(-d1.y, d1.x);
            let sum = left0 + left1;
            if sum.len() <= DUP_TOL {
                curr + left0 * inset
            } else {
                let m = sum / sum.len();
                let scale = 1.0 / m.dot(left0).max(1.0e-12);
                curr + m * inset * scale
            }
        } else {
            let d = segment_dir(prev, next);
            let left = Vec2::new(-d.y, d.x);
            curr + left * inset
        };
        out.push(offset);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Level, MoldingLine};
    use cadcraft_geom::Vec2;

    fn profile() -> cadcraft_doc::arch::MoldingProfile {
        crate::molding_profile::builtin_profile("molding:chair-rail").unwrap()
    }

    #[test]
    fn line_run_of_an_l() {
        let line = MoldingLine {
            points: vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(3.0, 2.0)],
            elevation: 0.2,
            inset: 0.1,
            profile: profile(),
            ..Default::default()
        };
        let run = line_run_at(&line, 1.5).unwrap();
        assert_eq!(run.path.len(), 3);
        assert!((run.path[0].x - 1.0).abs() < 1e-9);
        assert!((run.path[0].y - 1.1).abs() < 1e-9);
        assert!((run.path[1].x - 2.9).abs() < 1e-9);
        assert!((run.path[1].y - 1.1).abs() < 1e-9);
        assert!((run.path[2].x - 2.9).abs() < 1e-9);
        assert!((run.path[2].y - 2.0).abs() < 1e-9);
        assert!((crate::molding_runs::run_length(&run) - 2.8).abs() < 1e-9);
        assert_eq!(run.z, vec![1.7, 1.7, 1.7]);
    }

    #[test]
    fn flip_sets_side_minus_one() {
        let line = MoldingLine { flip: true, profile: profile(), points: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)], ..Default::default() };
        let run = line_run_at(&line, 0.0).unwrap();
        assert_eq!(run.side, -1.0);
    }

    #[test]
    fn inset_shifts_left_with_mitres() {
        let line = MoldingLine {
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0)],
            inset: 0.25,
            profile: profile(),
            ..Default::default()
        };
        let run = line_run_at(&line, 0.0).unwrap();
        assert_eq!(run.path[0], Vec2::new(0.0, 0.25));
        assert!((run.path[1].x - 2.75).abs() < 1e-9);
        assert!((run.path[1].y - 0.25).abs() < 1e-9);
        assert!((run.path[2].x - 2.75).abs() < 1e-9);
        assert!((run.path[2].y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn closed_square() {
        let line = MoldingLine {
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0)],
            closed: true,
            inset: 0.1,
            profile: profile(),
            ..Default::default()
        };
        let run = line_run_at(&line, 0.0).unwrap();
        assert!(run.closed);
        assert_eq!(run.path.len(), 4);
        assert!((crate::molding_runs::run_length(&run) - 7.2).abs() < 1e-9);
    }

    #[test]
    fn validate_refuses() {
        let ok = MoldingLine {
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)],
            inset: 0.1,
            elevation: 0.0,
            profile: profile(),
            ..Default::default()
        };
        assert!(validate(&ok).is_ok());
        let bad_elev = MoldingLine { elevation: 11.0, ..ok.clone() };
        assert!(validate(&bad_elev).is_err());
        let bad_inset = MoldingLine { inset: 2.0, ..ok.clone() };
        assert!(validate(&bad_inset).is_err());
        let bad_turn = MoldingLine { points: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(0.0, 0.0)], ..ok.clone() };
        assert!(validate(&bad_turn).is_err());
    }

    #[test]
    fn level_elevation_is_the_level() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 2.5, ..Default::default() });
        let line = MoldingLine {
            level: 0,
            elevation: 0.2,
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)],
            profile: profile(),
            ..Default::default()
        };
        let run = line_run(&model, &line).unwrap();
        assert!((run.z[0] - 2.7).abs() < 1e-9);
    }
}
