//! Placement limits and auto-resize for architectural openings.
//!
//! Pure 2D logic (no rendering). Encodes Chief's placement rules as data:
//! minimum widths per opening kind/style, minimum wall segments, separation
//! between openings, and the rough allowance that the 3D cut uses. All checks
//! are finite and panic-free, so hostile DXF/command input cannot crash the
//! crate.

use super::{ArchId, ArchModel, Wall};
use cadcraft_doc::arch::{Opening, OpeningKind, OpeningTop, is_glass_type};
use cadcraft_geom::Vec2;

/// Minimum width of a hinged/pocket/sliding opening, in metres.
pub const MIN_HINGED_WIDTH: f64 = 0.3;

/// Minimum width of a garage opening, in metres.
pub const MIN_GARAGE_WIDTH: f64 = 1.8;

/// Minimum wall segment on either side of a standard opening, in metres.
pub const MIN_WINDOW_SEGMENT: f64 = 0.15;

/// Minimum wall segment on either side of a bay/box/bow opening, in metres.
pub const MIN_BAY_SEGMENT: f64 = 0.725;

/// Minimum clear separation between two unrelated openings on a host wall, in metres.
pub const MIN_SEPARATION: f64 = 0.05;

/// Allowance added to finished dimensions to obtain the rough (cut) dimensions.
pub const ROUGH_ALLOWANCE: f64 = 0.12;

/// Wall material that must remain behind a niche's recess, in metres.
pub const NICHE_BACKING: f64 = 0.012;

/// The tolerance used when comparing spans for touching/overlap.
const EPS: f64 = 1e-9;

/// Whether `o` is a shower door: a frameless glass door (style "shower" in any case).
pub fn is_shower(o: &Opening) -> bool {
    o.kind == OpeningKind::Door && o.style.trim().eq_ignore_ascii_case("shower")
}

/// The allowance the 3D cut adds to the finished size: none for niches (the recess is exactly
/// its size) and shower doors (frameless), [`ROUGH_ALLOWANCE`] for everything else.
pub fn cut_allowance(o: &Opening) -> f64 {
    if o.kind == OpeningKind::Niche || is_shower(o) { 0.0 } else { ROUGH_ALLOWANCE }
}

/// Refuse a niche whose recess leaves less than [`NICHE_BACKING`] of the wall behind it.
pub fn validate_niche_depth(model: &ArchModel, host: ArchId, depth: f64) -> Result<(), PlacementError> {
    let wall = model.wall(host).ok_or_else(|| PlacementError(format!("host wall {host} not found")))?;
    if !depth.is_finite() || depth <= 0.0 {
        return Err(PlacementError("the niche depth must be positive".into()));
    }
    if !wall.thickness.is_finite() || wall.thickness < depth + NICHE_BACKING - EPS {
        return Err(PlacementError("the wall is not 12 mm thicker than the niche".into()));
    }
    Ok(())
}

/// A placement validation failure.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementError(pub String);

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The left edge of the rough (cut) span, i.e. the finished offset shifted
/// so the finished opening is centred within the rough opening.
pub fn rough_offset(o: &Opening) -> f64 {
    o.offset - o.rough_width / 2.0
}

/// Project a world point `p` onto the centre line of the host wall and return
/// the offset (metres) from the wall start, clamped to `[0, len]`.
///
/// Returns `None` when the host is missing or has a zero/non-finite length.
pub fn project_to_wall(model: &ArchModel, host: ArchId, p: Vec2) -> Option<f64> {
    let wall = model.wall(host)?;
    if !wall.start.is_finite() || !wall.end.is_finite() {
        return None;
    }
    let len = wall.start.dist(wall.end);
    if !len.is_finite() || len < 1e-12 {
        return None;
    }
    let dir = (wall.end - wall.start).normalized();
    let t = (p - wall.start).dot(dir);
    Some(t.clamp(0.0, len))
}

/// Minimum opening width in metres for a given kind and style.
pub fn min_width(kind: OpeningKind, style: &str) -> f64 {
    if kind == OpeningKind::GarageDoor {
        return MIN_GARAGE_WIDTH;
    }
    let s = style.to_ascii_lowercase();
    if s.contains("bifold") {
        0.45
    } else if s.contains("hinged") || s.contains("pocket") || s.contains("sliding") {
        MIN_HINGED_WIDTH
    } else {
        0.1
    }
}

/// Minimum wall segment required on each side of an opening for a given style.
pub fn min_segment(style: &str) -> f64 {
    let s = style.to_ascii_lowercase();
    if s.contains("bay") || s.contains("box") || s.contains("bow") { MIN_BAY_SEGMENT } else { MIN_WINDOW_SEGMENT }
}

/// Whether the two closed-ish spans `[a0..a1]` and `[b0..b1]` overlap.
/// Touching endpoints are *not* an overlap.
pub fn spans_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> bool {
    a0 < b1 && b0 < a1
}

/// Validate that an opening of the given size can be placed on `host` at
/// `offset`. On success returns the clamped (on-wall) offset.
pub fn validate_placement(
    model: &ArchModel,
    host: ArchId,
    offset: f64,
    width: f64,
    sill: f64,
    height: f64,
    kind: OpeningKind,
    style: &str,
    ignore: Option<ArchId>,
) -> Result<f64, PlacementError> {
    let wall = model.wall(host).ok_or_else(|| PlacementError(format!("host wall {host} not found"))).and_then(|w| {
        if w.start.is_finite() && w.end.is_finite() { Ok(w) } else { Err(PlacementError("host wall has non-finite endpoints".into())) }
    })?;
    let len = wall.start.dist(wall.end);
    if !len.is_finite() || len < 1e-12 {
        return Err(PlacementError("host wall has zero or non-finite length".into()));
    }

    if !offset.is_finite() || !width.is_finite() || !sill.is_finite() || !height.is_finite() {
        return Err(PlacementError("non-finite numeric input".into()));
    }
    if width <= 0.0 || height <= 0.0 || sill < 0.0 {
        return Err(PlacementError("width, height and sill must be positive".into()));
    }

    let min_w = min_width(kind, style);
    if width < min_w {
        return Err(PlacementError(format!("width {width} below minimum {min_w} for {kind:?}")));
    }
    if kind == OpeningKind::Door && style.trim().eq_ignore_ascii_case("shower") {
        let glass = match wall.split.as_ref() {
            None => is_glass_type(&wall.wall_type),
            Some(sp) => is_glass_type(&sp.upper) && sill >= sp.height - EPS,
        };
        if !glass {
            return Err(PlacementError("a shower door goes in a glass wall".into()));
        }
    }

    if !wall.height.is_finite() || sill + height > wall.height {
        return Err(PlacementError(format!("sill+height {} exceeds wall height {}", sill + height, wall.height)));
    }

    let offset = offset.clamp(0.0, len);
    let rough_width = width + ROUGH_ALLOWANCE;
    let rough_start = offset - rough_width / 2.0;
    let rough_end = rough_start + rough_width;
    let min_seg = min_segment(style);
    if rough_start < min_seg {
        return Err(PlacementError("rough span extends past the start of the wall".into()));
    }
    if rough_end > len - min_seg {
        return Err(PlacementError("rough span extends past the end of the wall".into()));
    }

    let c0 = rough_start;
    let c1 = rough_end;
    for other in model.openings_on(host) {
        if ignore == Some(other.id) {
            continue;
        }
        if !other.offset.is_finite() || !other.width.is_finite() || !other.rough_width.is_finite() {
            continue;
        }
        let b0 = other.offset - other.rough_width / 2.0;
        let b1 = b0 + other.rough_width;
        let gap = (b0 - c1).max(c0 - b1);
        let need = if other.mull_group.is_some() { 0.0 } else { MIN_SEPARATION };
        if gap < need - EPS {
            let other_id = other.id;
            return Err(PlacementError(format!("opening overlaps or is too close to opening {other_id}")));
        }
    }

    Ok(offset)
}

/// The height `validate_placement` checks against the wall height: for a `top: roof` window whose
/// host wall is taller than its sill, `min(height, wall.height - sill)` (the head follows the rake
/// above the wall's nominal height); otherwise `o.height`, so every other opening is checked as
/// before and a roof window whose sill is above the wall height is still refused.
pub fn check_height(model: &ArchModel, o: &Opening) -> f64 {
    if o.top == OpeningTop::Roof {
        if let Some(w) = model.wall(o.host) {
            let room = w.height - o.sill;
            if room.is_finite() && room > 0.0 {
                return o.height.min(room);
            }
        }
    }
    o.height
}

/// Place an opening on its host wall. When `fit` is true and the opening does
/// not fit, the width is shrunk (bisection) to the largest value that still
/// fits and is at least `min_width`; if even `min_width` does not fit at the
/// current offset, the opening is slid into the valid window. The offset is
/// kept clamped to the wall.
pub fn place_opening(model: &ArchModel, mut o: Opening, fit: bool) -> Result<Opening, PlacementError> {
    let min_w = min_width(o.kind, &o.style);
    let ch = check_height(model, &o);
    if o.kind == OpeningKind::Niche {
        validate_niche_depth(model, o.host, o.depth)?;
    }

    match validate_placement(model, o.host, o.offset, o.width, o.sill, ch, o.kind, &o.style, None) {
        Ok(offset) => {
            o.offset = offset;
            o.rough_width = o.width + ROUGH_ALLOWANCE;
            o.rough_height = o.height + ROUGH_ALLOWANCE;
            auto_panel(&mut o);
            clamp_recess(model, &mut o);
            Ok(o)
        }
        Err(e) if fit => {
            let wall = model.wall(o.host).ok_or_else(|| PlacementError("host wall not found".into()))?;
            let len = wall.start.dist(wall.end);
            if !len.is_finite() || len < 1e-12 {
                return Err(PlacementError("host wall has zero or non-finite length".into()));
            }
            let min_seg = min_segment(&o.style);
            let min_w = min_width(o.kind, &o.style);

            // (2) Try the requested width: clamp into the feasible offset window.
            {
                let rough_w = o.width + ROUGH_ALLOWANCE;
                let min_off = min_seg + rough_w / 2.0;
                let max_off = len - min_seg - rough_w / 2.0;
                if min_off <= max_off {
                    let clamped = o.offset.clamp(min_off, max_off);
                    if let Ok(offset) = validate_placement(model, o.host, clamped, o.width, o.sill, ch, o.kind, &o.style, None) {
                        o.offset = offset;
                        o.rough_width = o.width + ROUGH_ALLOWANCE;
                        o.rough_height = o.height + ROUGH_ALLOWANCE;
                        auto_panel(&mut o);
                        clamp_recess(model, &mut o);
                        return Ok(o);
                    }
                }
            }

            // (3) Bisection: shrink width down to min_width, clamping offset each iteration.
            let mut lo = min_w;
            let mut hi = o.width;
            for _ in 0..64 {
                let mid = (lo + hi) / 2.0;
                if mid == lo || mid == hi {
                    break;
                }
                let mid_rough = mid + ROUGH_ALLOWANCE;
                let mid_min_off = min_seg + mid_rough / 2.0;
                let mid_max_off = len - min_seg - mid_rough / 2.0;
                if mid_min_off <= mid_max_off {
                    let clamped = o.offset.clamp(mid_min_off, mid_max_off);
                    if validate_placement(model, o.host, clamped, mid, o.sill, ch, o.kind, &o.style, None).is_ok() {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                } else {
                    hi = mid;
                }
            }

            // (4) Final: use lo, clamp offset, validate.
            {
                let final_rough = lo + ROUGH_ALLOWANCE;
                let final_min_off = min_seg + final_rough / 2.0;
                let final_max_off = len - min_seg - final_rough / 2.0;
                if final_min_off <= final_max_off {
                    let final_clamped = o.offset.clamp(final_min_off, final_max_off);
                    if let Ok(offset) = validate_placement(model, o.host, final_clamped, lo, o.sill, ch, o.kind, &o.style, None) {
                        o.offset = offset;
                        o.width = lo;
                        o.rough_width = lo + ROUGH_ALLOWANCE;
                        o.rough_height = o.height + ROUGH_ALLOWANCE;
                        auto_panel(&mut o);
                        clamp_recess(model, &mut o);
                        return Ok(o);
                    }
                }
            }

            // (5) Relaxed: the wall is too short for min_seg clearance even at min_width.
            // Place at min_width using only physical bounds (rough span inside the wall).
            {
                let rough_w = lo + ROUGH_ALLOWANCE;
                let phys_min = rough_w / 2.0;
                let phys_max = len - rough_w / 2.0;
                if phys_min <= phys_max {
                    let clamped = o.offset.clamp(phys_min, phys_max);
                    let ok_height = wall.height.is_finite() && o.sill + ch <= wall.height;
                    let c0 = clamped - rough_w / 2.0;
                    let c1 = clamped + rough_w / 2.0;
                    // Check no overlap with existing openings on this wall.
                    let mut overlap_ok = true;
                    for other in model.openings_on(o.host) {
                        if other.offset.is_finite() && other.width.is_finite() && other.rough_width.is_finite() {
                            let b0 = other.offset - other.rough_width / 2.0;
                            let b1 = b0 + other.rough_width;
                            let gap = (b0 - c1).max(c0 - b1);
                            let need = if other.mull_group.is_some() { 0.0 } else { MIN_SEPARATION };
                            if gap < need - EPS {
                                overlap_ok = false;
                                break;
                            }
                        }
                    }
                    if ok_height && overlap_ok {
                        o.offset = clamped;
                        o.width = lo;
                        o.rough_width = lo + ROUGH_ALLOWANCE;
                        o.rough_height = o.height + ROUGH_ALLOWANCE;
                        auto_panel(&mut o);
                        clamp_recess(model, &mut o);
                        return Ok(o);
                    }
                }
            }
            Err(e)
        }
        Err(e) => Err(e),
    }
}

/// Whether an opening is wide enough to be a double (two-leaf) unit.
pub fn is_double(o: &Opening) -> bool {
    let s = o.style.to_ascii_lowercase();
    let hinged_or_pocket = s.contains("hinged") || s.contains("pocket");
    if hinged_or_pocket {
        o.width >= 1.2
    } else if s.contains("bifold") {
        o.width > 0.9
    } else {
        false
    }
}

/// Append " double" to the style when the opening qualifies as a double unit.
pub fn auto_panel(o: &mut Opening) {
    if is_double(o) && !o.style.contains("double") {
        o.style.push_str(" double");
    }
}

/// Clamp the opening recess into the host wall thickness.
pub fn clamp_recess(model: &ArchModel, o: &mut Opening) -> f64 {
    let max = match model.wall(o.host) {
        Some(w) if w.thickness.is_finite() => w.thickness,
        _ => 0.0,
    };
    o.recess = o.recess.clamp(0.0, max);
    o.recess
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_geom::Vec2;

    fn wall(model: &mut ArchModel, start: Vec2, end: Vec2, thickness: f64) -> ArchId {
        model.push_wall(Wall { start, end, thickness, ..Wall::default() })
    }

    fn opening_on(model: &mut ArchModel, host: ArchId, offset: f64, width: f64) -> ArchId {
        let o = Opening::new(host, offset, width, 2.0, OpeningKind::Door);
        model.push_opening(o)
    }

    fn roof_window(host: ArchId, sill: f64, height: f64) -> Opening {
        Opening { host, offset: 2.5, width: 1.0, sill, height, kind: OpeningKind::Window, top: OpeningTop::Roof, ..Opening::default() }
    }

    #[test]
    fn check_height_clamps_only_roof_tops() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let h = model.wall(host).map_or(0.0, |w| w.height);
        let roof = roof_window(host, 1.0, 5.0);
        assert!((check_height(&model, &roof) - (h - 1.0)).abs() < 1e-12);
        let low = roof_window(host, 1.0, 0.5);
        assert_eq!(check_height(&model, &low), 0.5);
        let above = roof_window(host, h + 0.5, 1.0);
        assert_eq!(check_height(&model, &above), 1.0);
        let square = Opening { top: OpeningTop::Square, ..roof_window(host, 1.0, 5.0) };
        assert_eq!(check_height(&model, &square), 5.0);
        let round = Opening { top: OpeningTop::Round, ..roof_window(host, 1.0, 5.0) };
        assert_eq!(check_height(&model, &round), 5.0);
    }

    #[test]
    fn place_opening_skips_the_wall_height_check_for_roof_tops() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let h = model.wall(host).map_or(0.0, |w| w.height);
        assert!(place_opening(&model, roof_window(host, 1.0, h + 1.0), false).is_ok());
        let square = Opening { top: OpeningTop::Square, ..roof_window(host, 1.0, h + 1.0) };
        assert!(place_opening(&model, square, false).is_err());
        assert!(place_opening(&model, roof_window(host, h + 0.5, 1.0), false).is_err());
    }

    #[test]
    fn validate_rejects_an_opening_wider_than_the_wall() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(1.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 0.0, 2.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
    }

    #[test]
    fn validate_enforces_the_minimum_garage_width() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 1.0, 1.0, 0.0, 2.0, OpeningKind::GarageDoor, "garage", None);
        assert!(res.is_err());
        let ok = validate_placement(&model, host, 2.0, 2.0, 0.0, 2.0, OpeningKind::GarageDoor, "garage", None);
        assert!(ok.is_ok());
    }

    #[test]
    fn validate_requires_a_minimum_wall_segment_for_windows() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(1.0, 0.0), 0.2);
        // Window at offset 0 leaves no segment at the start.
        let res = validate_placement(&model, host, 0.0, 0.6, 0.0, 1.5, OpeningKind::Window, "slider", None);
        assert!(res.is_err());
    }

    #[test]
    fn validate_rejects_overlapping_openings_on_the_same_host() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        opening_on(&mut model, host, 0.5, 1.0);
        let res = validate_placement(&model, host, 1.0, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
    }

    #[test]
    fn an_overlap_error_names_the_other_opening_by_id() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        model.walls.iter_mut().find(|w| w.id == host).map(|w| w.height = 2.5);
        let other = opening_on(&mut model, host, 0.5, 1.0);
        let res = validate_placement(&model, host, 1.0, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        let err = res.unwrap_err();
        assert_eq!(err.0, format!("opening overlaps or is too close to opening {other}"));
    }

    #[test]
    fn validate_rejects_a_sill_above_the_header() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 1.0, 0.6, 2.0, 2.0, OpeningKind::Window, "slider", None);
        assert!(res.is_err());
    }

    #[test]
    fn fit_true_shrinks_the_opening_to_the_available_span() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(1.5, 0.0), 0.2);
        let o = Opening::new(host, 0.5, 2.0, 2.0, OpeningKind::Door);
        let placed = place_opening(&mut model, o, true).unwrap();
        assert!(placed.width >= min_width(OpeningKind::Door, "hinged"));
        assert!(placed.width < 2.0);
        assert!(
            validate_placement(&model, host, placed.offset, placed.width, placed.sill, placed.height, OpeningKind::Door, &placed.style, None,)
                .is_ok()
        );
    }

    #[test]
    fn project_to_wall_maps_a_point_to_an_offset() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(10.0, 0.0), 0.2);
        assert_eq!(project_to_wall(&model, host, Vec2::new(3.0, 5.0)), Some(3.0));
        assert_eq!(project_to_wall(&model, host, Vec2::new(-1.0, 0.0)), Some(0.0));
        assert_eq!(project_to_wall(&model, host, Vec2::new(12.0, 0.0)), Some(10.0));
    }

    #[test]
    fn auto_panel_makes_a_wide_hinged_door_double() {
        let mut o = Opening::new(ArchId(1), 0.0, 1.5, 2.0, OpeningKind::Door);
        auto_panel(&mut o);
        assert!(o.style.contains("double"));
        let mut narrow = Opening::new(ArchId(2), 0.0, 0.9, 2.0, OpeningKind::Door);
        auto_panel(&mut narrow);
        assert!(!narrow.style.contains("double"));
    }

    #[test]
    fn validate_survives_nan_and_infinite_input() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let nan = validate_placement(&model, host, f64::NAN, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(nan.is_err());
        let inf = validate_placement(&model, host, f64::INFINITY, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(inf.is_err());
    }

    #[test]
    fn validate_rejects_sill_above_header() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        // Wall height is 0.2 (default), sill 2.0 + height 2.0 = 4.0 > 0.2
        let res = validate_placement(&model, host, 1.0, 0.6, 2.0, 2.0, OpeningKind::Window, "slider", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("exceeds wall height"));
    }

    #[test]
    fn fit_slides_an_opening_away_from_the_wall_end_before_shrinking() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(10.0, 0.0), 0.2);
        // Offset 0.05 is too close to the wall start (min_seg 0.15 + rough/2 0.56 = 0.71 min_off)
        let o = Opening::new(host, 0.05, 1.0, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        // Slid, not shrunk: width unchanged and offset pushed to the feasible window
        assert_eq!(placed.width, 1.0);
        assert!(placed.offset >= 0.71);
    }

    #[test]
    fn fit_never_shrinks_below_the_kind_minimum_width() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(0.5, 0.0), 0.2);
        // Very short wall: even a 0.5 m opening won't fit fully
        let o = Opening::new(host, 0.0, 0.5, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        assert!(placed.width >= min_width(OpeningKind::Door, "hinged"));
    }

    #[test]
    fn fit_keeps_the_rough_span_inside_the_wall() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(3.0, 0.0), 0.2);
        let o = Opening::new(host, 0.1, 2.5, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        // The rough span must not extend past either wall end.
        let rough_w = placed.rough_width;
        let min_seg = min_segment(&placed.style);
        assert!(placed.offset >= min_seg + rough_w / 2.0 - 1e-9);
        assert!(placed.offset + (rough_w / 2.0) <= 3.0 - min_seg + 1e-9);
    }

    #[test]
    fn fit_shrinks_a_too_wide_opening() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(1.5, 0.0), 0.2);
        // Opening width 2.0 is too wide for a 1.5m wall with min segments
        let o = Opening::new(host, 0.5, 2.0, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        // Should be shrunk to fit but still >= min_width
        assert!(placed.width >= min_width(OpeningKind::Door, "hinged"));
        assert!(placed.width < 2.0);
    }

    #[test]
    fn fit_slides_an_opening_away_from_the_wall_end() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(3.0, 0.0), 0.2);
        // Place at offset 0 with a width that fits but is too close to the start
        let o = Opening::new(host, 0.0, 1.0, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        // Should be slid away from the wall start
        assert!(placed.offset > 0.0);
    }

    #[test]
    fn rough_span_is_centred_on_the_offset() {
        let o = Opening::new(ArchId(1), 1.0, 1.0, 2.0, OpeningKind::Door);
        let rough_w = o.width + ROUGH_ALLOWANCE;
        let o_rough = Opening { rough_width: rough_w, ..o };
        let off = rough_offset(&o_rough);
        // Rough span [off, off + rough_w] should be centred on offset 1.0
        let centre = (off + off + rough_w) / 2.0;
        assert!((centre - 1.0).abs() < 1e-9);
    }

    #[test]
    fn validate_placement_with_zero_width_opening() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 1.0, 0.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("positive"));
    }

    #[test]
    fn validate_placement_with_nan_offset() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, f64::NAN, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("non-finite"));
    }

    #[test]
    fn validate_placement_with_openings_too_close() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        // First opening at offset 0.5, width 1.0, rough width 1.12
        // c0 = 0.5 - 0.56 = -0.06, c1 = -0.06 + 1.12 = 1.06
        opening_on(&mut model, host, 0.5, 1.0);
        // Second opening at offset 1.1, width 1.0, rough width 1.12
        // b0 = 1.1 - 0.56 = 0.54, b1 = 0.54 + 1.12 = 1.66
        // gap = (0.54 - 1.06).max(-0.06 - 1.66) = -0.52 (overlap)
        let res = validate_placement(&model, host, 1.1, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
    }

    #[test]
    fn validate_placement_with_openings_far_enough_apart() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        model.walls.iter_mut().find(|w| w.id == host).map(|w| w.height = 2.5);
        // First opening at offset 0.5, width 1.0, rough width 1.12
        // c0 = 0.5 - 0.56 = -0.06, c1 = -0.06 + 1.12 = 1.06
        opening_on(&mut model, host, 0.5, 1.0);
        // Second opening at offset 1.67, width 1.0, rough width 1.12
        // b0 = 1.67 - 0.56 = 1.11, b1 = 1.11 + 1.12 = 2.23
        // gap = (1.11 - 1.06).max(-0.06 - 2.23) = 0.05 = MIN_SEPARATION
        let res = validate_placement(&model, host, 1.67, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_ok());
    }

    #[test]
    fn validate_rejects_zero_length_wall() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::ZERO, 0.2);
        let res = validate_placement(&model, host, 0.0, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("zero or non-finite length"));
    }

    #[test]
    fn validate_rejects_negative_sill() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 1.0, 0.9, -0.1, 2.0, OpeningKind::Window, "slider", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("positive"));
    }

    #[test]
    fn validate_survives_huge_values() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        // Huge offset: clamped to wall end, then rough span check fails
        let res = validate_placement(&model, host, 1e308, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        // Huge width: fails rough span check
        let res2 = validate_placement(&model, host, 2.5, 1e308, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res2.is_err());
        // Huge sill: fails sill+height > wall.height
        let res3 = validate_placement(&model, host, 2.5, 0.9, 1e308, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res3.is_err());
    }

    #[test]
    fn fit_false_returns_error_when_opening_does_not_fit() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(1.0, 0.0), 0.2);
        let o = Opening::new(host, 0.5, 2.0, 2.0, OpeningKind::Door);
        let res = place_opening(&model, o, false);
        assert!(res.is_err());
    }

    #[test]
    fn project_to_wall_returns_none_for_zero_length_wall() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::ZERO, 0.2);
        assert_eq!(project_to_wall(&model, host, Vec2::new(3.0, 5.0)), None);
    }

    #[test]
    fn fit_on_short_wall_uses_relaxed_physical_bounds() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(0.5, 0.0), 0.2);
        model.walls.iter_mut().find(|w| w.id == host).map(|w| w.height = 2.5);
        let o = Opening::new(host, 0.0, 0.3, 2.0, OpeningKind::Door);
        let placed = place_opening(&model, o, true).unwrap();
        assert!(placed.offset >= 0.21 - 1e-9);
        assert!(placed.offset <= 0.29 + 1e-9);
        assert_eq!(placed.width, 0.3);
    }

    #[test]
    fn validate_rejects_wall_with_non_finite_endpoints() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::new(f64::NAN, 0.0), Vec2::new(5.0, 0.0), 0.2);
        let res = validate_placement(&model, host, 1.0, 0.9, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.0.contains("non-finite"));
    }

    #[test]
    fn project_to_wall_returns_none_for_non_finite_wall() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::new(f64::NAN, 0.0), Vec2::new(5.0, 0.0), 0.2);
        assert_eq!(project_to_wall(&model, host, Vec2::new(3.0, 5.0)), None);
    }

    #[test]
    fn spans_overlap_detects_overlap_and_touching() {
        assert!(spans_overlap(0.0, 1.0, 0.5, 1.5));
        assert!(spans_overlap(0.5, 1.5, 0.0, 1.0));
        assert!(spans_overlap(0.0, 2.0, 0.5, 1.5));
        // Touching endpoints are not an overlap.
        assert!(!spans_overlap(0.0, 1.0, 1.0, 2.0));
        assert!(!spans_overlap(1.0, 2.0, 0.0, 1.0));
        // Disjoint spans.
        assert!(!spans_overlap(0.0, 1.0, 2.0, 3.0));
        assert!(!spans_overlap(2.0, 3.0, 0.0, 1.0));
    }

    #[test]
    fn min_width_for_bifold_and_other_styles() {
        assert_eq!(min_width(OpeningKind::Door, "bifold"), 0.45);
        assert_eq!(min_width(OpeningKind::Door, "hinged"), MIN_HINGED_WIDTH);
        assert_eq!(min_width(OpeningKind::Door, "pocket"), MIN_HINGED_WIDTH);
        assert_eq!(min_width(OpeningKind::Door, "sliding"), MIN_HINGED_WIDTH);
        assert_eq!(min_width(OpeningKind::Window, "slider"), 0.1);
        assert_eq!(min_width(OpeningKind::GarageDoor, "garage"), MIN_GARAGE_WIDTH);
        // Case-insensitive.
        assert_eq!(min_width(OpeningKind::Door, "Bifold"), 0.45);
        assert_eq!(min_width(OpeningKind::Door, "HINGED"), MIN_HINGED_WIDTH);
    }

    #[test]
    fn clamp_recess_clamps_to_host_thickness() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        let mut o = Opening::new(host, 1.0, 1.0, 2.0, OpeningKind::Door);
        o.recess = 0.5;
        let clamped = clamp_recess(&model, &mut o);
        assert_eq!(clamped, 0.2);
        assert_eq!(o.recess, 0.2);
        // Negative recess clamps to 0.
        o.recess = -0.1;
        let clamped2 = clamp_recess(&model, &mut o);
        assert_eq!(clamped2, 0.0);
        assert_eq!(o.recess, 0.0);
    }

    #[test]
    fn validate_placement_with_ignore_parameter() {
        let mut model = ArchModel::default();
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(5.0, 0.0), 0.2);
        model.walls.iter_mut().find(|w| w.id == host).map(|w| w.height = 2.5);
        let other = opening_on(&mut model, host, 1.0, 1.0);
        // Without ignore, overlaps.
        let res = validate_placement(&model, host, 1.0, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", None);
        assert!(res.is_err());
        // With ignore, passes.
        let res2 = validate_placement(&model, host, 1.0, 1.0, 0.0, 2.0, OpeningKind::Door, "hinged", Some(other));
        assert!(res2.is_ok());
    }

    #[test]
    fn project_to_wall_for_diagonal_wall() {
        let mut model = ArchModel::default();
        // Diagonal wall from (0,0) to (10,10), length 10*sqrt(2)
        let host = wall(&mut model, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.2);
        // Point (5,5) is at the midpoint, offset should be half the wall length.
        let offset = project_to_wall(&model, host, Vec2::new(5.0, 5.0)).unwrap();
        let expected = 10.0 * 2.0_f64.sqrt() / 2.0;
        assert!((offset - expected).abs() < 1e-9);
        // Point (0,10) projects to the wall at the same midpoint.
        let offset2 = project_to_wall(&model, host, Vec2::new(0.0, 10.0)).unwrap();
        assert!((offset2 - expected).abs() < 1e-9);
        // Point before the start clamps to 0.
        let offset3 = project_to_wall(&model, host, Vec2::new(-5.0, -5.0)).unwrap();
        assert_eq!(offset3, 0.0);
        // Point past the end clamps to wall length.
        let offset4 = project_to_wall(&model, host, Vec2::new(15.0, 15.0)).unwrap();
        let wall_len = 10.0 * 2.0_f64.sqrt();
        assert!((offset4 - wall_len).abs() < 1e-9);
    }

    fn glass_wall(model: &mut ArchModel, thickness: f64) -> ArchId {
        model.push_wall(Wall {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness,
            wall_type: "Glass".into(),
            height: 2.0,
            ..Wall::default()
        })
    }

    fn niche(host: ArchId, offset: f64, depth: f64) -> Opening {
        Opening { host, offset, width: 0.6, height: 0.9, sill: 1.0, kind: OpeningKind::Niche, style: "niche".into(), depth, ..Opening::default() }
    }

    #[test]
    fn niche_needs_12_mm_more_wall() {
        let mut m = ArchModel::default();
        let h = wall(&mut m, Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2);
        assert!(place_opening(&m, niche(h, 4.0, 0.1), false).is_ok());
        assert!(place_opening(&m, niche(h, 4.0, 0.188), false).is_ok());
        let e = place_opening(&m, niche(h, 4.0, 0.19), false).unwrap_err();
        assert_eq!(e.0, "the wall is not 12 mm thicker than the niche");
        assert!(place_opening(&m, niche(h, 4.0, 0.0), false).is_err());
        assert!(place_opening(&m, niche(h, 4.0, f64::NAN), false).is_err());
    }

    #[test]
    fn niche_overlaps_like_openings() {
        let mut m = ArchModel::default();
        let h = wall(&mut m, Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2);
        let a = place_opening(&m, niche(h, 4.0, 0.1), false).unwrap();
        m.push_opening(a);
        assert!(place_opening(&m, niche(h, 4.5, 0.1), false).is_err());
        assert!(place_opening(&m, niche(h, 6.0, 0.1), false).is_ok());
        let mut tall = niche(h, 6.0, 0.1);
        tall.sill = 2.0;
        assert!(place_opening(&m, tall, false).is_err());
    }

    #[test]
    fn shower_style_only_in_glass() {
        let mut m = ArchModel::default();
        let plain = wall(&mut m, Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2);
        let glass = glass_wall(&mut m, 0.01);
        let door = |host| Opening { host, offset: 5.0, width: 0.7, height: 1.9, style: "Shower".into(), ..Opening::default() };
        let e = place_opening(&m, door(plain), false).unwrap_err();
        assert_eq!(e.0, "a shower door goes in a glass wall");
        assert!(place_opening(&m, door(glass), false).is_ok());
        let split = m.push_wall(Wall {
            start: Vec2::new(0.0, 5.0),
            end: Vec2::new(10.0, 5.0),
            thickness: 0.1,
            height: 2.4,
            split: Some(cadcraft_doc::arch::WallSplit { height: 1.0, upper: "Glass".into(), upper_thickness: 0.01 }),
            ..Wall::default()
        });
        let mut low = door(split);
        low.height = 0.8;
        assert!(place_opening(&m, low, false).is_err());
        let mut high = door(split);
        high.sill = 1.0;
        high.height = 1.4;
        assert!(place_opening(&m, high, false).is_ok());
    }

    #[test]
    fn cut_allowance_is_zero_for_niches_and_shower_doors_only() {
        let base = Opening::default();
        assert_eq!(cut_allowance(&base), ROUGH_ALLOWANCE);
        assert_eq!(cut_allowance(&Opening { kind: OpeningKind::Niche, ..base.clone() }), 0.0);
        let shower = Opening { style: "SHOWER".into(), ..base.clone() };
        assert!(is_shower(&shower));
        assert_eq!(cut_allowance(&shower), 0.0);
        let win = Opening { kind: OpeningKind::Window, style: "shower".into(), ..base.clone() };
        assert!(!is_shower(&win));
        assert_eq!(cut_allowance(&win), ROUGH_ALLOWANCE);
        for kind in [OpeningKind::Window, OpeningKind::GarageDoor, OpeningKind::CasedOpening] {
            assert_eq!(cut_allowance(&Opening { kind, ..base.clone() }), ROUGH_ALLOWANCE);
        }
    }
}
