//! Opening indicators in elevations: the swing triangles of hinged and double doors, casement, awning and hopper
//! windows, and cabinet doors. They are placed on the face toward the viewer, then pass through hidden-line removal
//! so an object hidden by another loses its indicator.

use crate::Scene3d;
use crate::hlr::{OrthoView, Seg3, visible_segments};
use cadcraft_arch::casework_layout::{self, FrontKind, Hinge as FrontHinge};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchKind, ArchView, Cabinet, Hinge, Opening, OpeningKind, ViewLine, Wall, level_by_index};

/// Most indicator lines in one view.
const MAX_INDICATOR_LINES: usize = 20_000;
/// Most lines one object contributes.
const MAX_PER_ITEM: usize = 4;
/// How far the indicators are lifted toward the viewer, metres.
const LIFT: f64 = 1e-4;
/// A face counts as facing the viewer when its normal is at least this much along the view.
const MIN_FACING: f64 = 0.2;

/// Where an opening sits in 3D: a point on the wall's viewer-side face and the unit along the wall.
pub(crate) struct OpeningBox {
    /// The viewer-side face point at the wall's start (z = 0).
    origin: [f64; 3],
    /// Unit along the wall.
    along: [f64; 2],
    /// Along-wall extent `s0..s1` and height `z0..z1`.
    pub s: [f64; 2],
    pub z: [f64; 2],
    /// The wall-centre start (for centres on the plan line).
    start: [f64; 2],
}

impl OpeningBox {
    /// The 3D point at `s` along the wall and height `z`, on the viewer-side face, lifted `lift` toward the viewer.
    fn point(&self, s: f64, z: f64) -> [f64; 3] {
        [self.origin[0] + self.along[0] * s, self.origin[1] + self.along[1] * s, z]
    }

    /// The centre of the box in the wall's centre plane.
    pub(crate) fn centre(&self) -> [f64; 3] {
        let s = (self.s[0] + self.s[1]) / 2.0;
        [self.start[0] + self.along[0] * s, self.start[1] + self.along[1] * s, (self.z[0] + self.z[1]) / 2.0]
    }
}

fn finite2(v: [f64; 2]) -> bool {
    v[0].is_finite() && v[1].is_finite()
}

/// The box of `o` on its host wall, `None` when the wall is missing or degenerate, or (when `ov` is given) seen
/// edge-on or from behind.
pub(crate) fn opening_box(model: &ArchModel, o: &Opening, ov: Option<&OrthoView>) -> Option<OpeningBox> {
    let wall: &Wall = model.walls.iter().find(|w| w.id == o.host)?;
    let (sx, sy, ex, ey) = (wall.start.x, wall.start.y, wall.end.x, wall.end.y);
    if !finite2([sx, sy]) || !finite2([ex, ey]) || !wall.thickness.is_finite() {
        return None;
    }
    let len = (ex - sx).hypot(ey - sy);
    if !(len > 1e-9) {
        return None;
    }
    let along = [(ex - sx) / len, (ey - sy) / len];
    let normal = [-along[1], along[0]];
    let (mut face, mut lift) = (normal, 0.0);
    if let Some(ov) = ov {
        let d = ov.dir();
        let k = normal[0] * d[0] + normal[1] * d[1];
        if k.abs() < MIN_FACING {
            return None;
        }
        if k > 0.0 {
            face = [-normal[0], -normal[1]];
        }
        lift = LIFT;
    }
    if !(o.offset.is_finite() && o.width.is_finite() && o.height.is_finite() && o.sill.is_finite()) || o.width <= 0.0 || o.height <= 0.0 {
        return None;
    }
    let off = wall.thickness / 2.0 + lift;
    let bottom = wall.bottom_z(&model.levels) + o.sill;
    Some(OpeningBox {
        origin: [sx + face[0] * off, sy + face[1] * off, 0.0],
        along,
        s: [o.offset - o.width / 2.0, o.offset + o.width / 2.0],
        z: [bottom, bottom + o.height],
        start: [sx, sy],
    })
}

/// The swing lines in `(s, z)` of a box `s0..s1 × z0..z1`: hinge jamb at `s0` when `hinge_start`.
fn triangle(apex: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [([f64; 2], [f64; 2]); 2] {
    [(apex, a), (apex, b)]
}

/// Swing lines of a style in a rectangle, empty for styles with no indicator.
fn swing_lines(kind_is_door: bool, style: &str, hinge_start: bool, s: [f64; 2], z: [f64; 2]) -> Vec<([f64; 2], [f64; 2])> {
    let style = style.to_lowercase();
    let [s0, s1] = s;
    let [z0, z1] = z;
    let (zm, sm) = ((z0 + z1) / 2.0, (s0 + s1) / 2.0);
    let one = |from_start: bool| {
        let (hs, ls) = if from_start { (s0, s1) } else { (s1, s0) };
        triangle([hs, zm], [ls, z0], [ls, z1])
    };
    if style.contains("double") {
        let mut v = triangle([s0, zm], [sm, z0], [sm, z1]).to_vec();
        v.extend(triangle([s1, zm], [sm, z0], [sm, z1]));
        v
    } else if style.contains("awning") {
        triangle([sm, z1], [s0, z0], [s1, z0]).to_vec()
    } else if style.contains("hopper") {
        triangle([sm, z0], [s0, z1], [s1, z1]).to_vec()
    } else if style.contains("casement") || (kind_is_door && (style == "hinged" || style.contains("hinge"))) {
        one(hinge_start).to_vec()
    } else {
        Vec::new()
    }
}

/// The cabinet's front frame: the point at `u` across (from the centre of the back edge) and height `z` on the front
/// plane, lifted toward the viewer when `lift`.
struct FrontFrame {
    centre: [f64; 2],
    width_dir: [f64; 2],
    front: [f64; 2],
    depth: f64,
    floor: f64,
}

impl FrontFrame {
    fn new(model: &ArchModel, c: &Cabinet) -> Option<Self> {
        if !(c.position.x.is_finite() && c.position.y.is_finite() && c.rotation.is_finite() && c.depth.is_finite() && c.width.is_finite()) {
            return None;
        }
        let r = c.rotation.to_radians();
        let width_dir = [r.cos(), r.sin()];
        let front = [-r.sin(), r.cos()];
        let floor = level_by_index(&model.levels, c.level).map_or(0.0, |l| l.elevation);
        Some(FrontFrame { centre: [c.position.x, c.position.y], width_dir, front, depth: c.depth, floor })
    }

    fn point(&self, u: f64, z: f64, lift: f64) -> [f64; 3] {
        let d = self.depth + lift;
        [self.centre[0] + self.width_dir[0] * u + self.front[0] * d, self.centre[1] + self.width_dir[1] * u + self.front[1] * d, self.floor + z]
    }
}

/// The centre of the cabinet's front (in the face plane), `None` unless its front faces the viewer. The cabinet's
/// vertical extent runs from its bottom to its top.
pub(crate) fn cabinet_front_centre(model: &ArchModel, c: &Cabinet, ov: Option<&OrthoView>) -> Option<[f64; 3]> {
    let f = FrontFrame::new(model, c)?;
    if let Some(ov) = ov {
        let d = ov.dir();
        if f.front[0] * d[0] + f.front[1] * d[1] >= -0.7 {
            return None;
        }
    }
    if !c.height.is_finite() || !c.elevation.is_finite() {
        return None;
    }
    Some(f.point(0.0, c.elevation + c.height / 2.0, 0.0))
}

/// Indicator lines for `view`: the 3D indicators after hidden-line removal in `sc`.
pub(crate) fn indicator_lines(model: &ArchModel, sc: &Scene3d, _view: &ArchView, ov: &OrthoView) -> Vec<ViewLine> {
    let mut segs: Vec<Seg3> = Vec::new();
    for o in &model.openings {
        if segs.len() >= MAX_INDICATOR_LINES {
            break;
        }
        let door = matches!(o.kind, OpeningKind::Door);
        if !(door || o.kind == OpeningKind::Window) {
            continue;
        }
        let Some(b) = opening_box(model, o, Some(ov)) else { continue };
        for (p, q) in swing_lines(door, &o.style, o.hinge == Hinge::Left, b.s, b.z).into_iter().take(MAX_PER_ITEM) {
            segs.push(Seg3 { a: b.point(p[0], p[1]), b: b.point(q[0], q[1]), kind: ArchKind::Opening, object: o.id, ..Seg3::default() });
        }
    }
    for c in &model.cabinets {
        if segs.len() >= MAX_INDICATOR_LINES {
            break;
        }
        let Some(f) = FrontFrame::new(model, c) else { continue };
        let d = ov.dir();
        if f.front[0] * d[0] + f.front[1] * d[1] >= -0.7 {
            continue;
        }
        let mut n = 0;
        for fr in casework_layout::fronts(model, c) {
            if fr.kind != FrontKind::Door || n >= MAX_PER_ITEM {
                continue;
            }
            let Some(h) = fr.hinge else { continue };
            let [x0, z0, x1, z1] = fr.rect;
            if !(x0.is_finite() && x1.is_finite() && z0.is_finite() && z1.is_finite()) {
                continue;
            }
            let (zm, hs, ls) = ((z0 + z1) / 2.0, if h == FrontHinge::Left { x0 } else { x1 }, if h == FrontHinge::Left { x1 } else { x0 });
            for (p, q) in triangle([hs, zm], [ls, z0], [ls, z1]) {
                if n >= MAX_PER_ITEM {
                    break;
                }
                n += 1;
                segs.push(Seg3 {
                    a: f.point(p[0], p[1], LIFT),
                    b: f.point(q[0], q[1], LIFT),
                    kind: ArchKind::Cabinet,
                    object: c.id,
                    ..Seg3::default()
                });
            }
        }
    }
    segs.truncate(MAX_INDICATOR_LINES);
    let mut out: Vec<ViewLine> = visible_segments(sc, ov, &segs)
        .into_iter()
        .map(|l| ViewLine { a: l.a, b: l.b, kind: l.kind, object: l.object, ..ViewLine::default() })
        .collect();
    out.truncate(MAX_INDICATOR_LINES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::ArchId;
    use cadcraft_geom::Vec2;

    fn base() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m
    }

    fn south() -> (ArchView, OrthoView) {
        (ArchView::default(), OrthoView::elevation([0.0, 1.0]).expect("view"))
    }

    fn lines_of(m: &ArchModel, ov: &OrthoView) -> Vec<ViewLine> {
        indicator_lines(m, &crate::build(m), &ArchView::default(), ov)
    }

    /// Each line as `(apex, far end)` where the apex is the end `apex` accepts (lines come back in either order).
    fn oriented(l: &[ViewLine], apex: impl Fn([f64; 2]) -> bool) -> Vec<([f64; 2], [f64; 2])> {
        l.iter().map(|x| if apex(x.a) { (x.a, x.b) } else { (x.b, x.a) }).collect()
    }

    fn near(p: [f64; 2], x: f64, z: f64) -> bool {
        (p[0] - x).abs() < 1e-6 && (p[1] - z).abs() < 1e-6
    }

    fn door(m: &mut ArchModel, style: &str, hinge: Hinge) -> ArchId {
        let mut o = Opening::new(ArchId(0), 3.0, 0.9, 2.1, OpeningKind::Door);
        o.style = style.into();
        o.hinge = hinge;
        m.push_opening(o)
    }

    fn window(m: &mut ArchModel, style: &str) -> ArchId {
        let mut o = Opening::new(ArchId(0), 7.0, 0.9, 1.2, OpeningKind::Window);
        o.style = style.into();
        o.sill = 0.9;
        m.push_opening(o)
    }

    #[test]
    fn hinged_door_triangle_apex_on_the_hinge_jamb() {
        let (_, ov) = south();
        for (h, jamb, far) in [(Hinge::Left, 2.55, 3.45), (Hinge::Right, 3.45, 2.55)] {
            let mut m = base();
            door(&mut m, "hinged", h);
            let l = lines_of(&m, &ov);
            assert_eq!(l.len(), 2, "{l:?}");
            assert!(l.iter().all(|x| x.kind == ArchKind::Opening));
            let o = oriented(&l, |p| near(p, jamb, 1.05));
            for (a, b) in &o {
                assert!(near(*a, jamb, 1.05), "apex {a:?}");
                assert!((b[0] - far).abs() < 1e-6);
            }
            let mut ends: Vec<f64> = o.iter().map(|x| x.1[1]).collect();
            ends.sort_by(f64::total_cmp);
            assert!(ends[0].abs() < 1e-6 && (ends[1] - 2.1).abs() < 1e-6);
        }
    }

    #[test]
    fn double_door_two_triangles() {
        let (_, ov) = south();
        let mut m = base();
        door(&mut m, "double", Hinge::Left);
        let l = lines_of(&m, &ov);
        assert_eq!(l.len(), 4, "{l:?}");
        let mid = |p: [f64; 2]| (p[0] - 3.0).abs() < 1e-6;
        assert!(l.iter().all(|x| mid(x.a) != mid(x.b)), "{l:?}");
        let o = oriented(&l, |p| !mid(p));
        assert_eq!(o.iter().filter(|x| near(x.0, 2.55, 1.05)).count(), 2);
        assert_eq!(o.iter().filter(|x| near(x.0, 3.45, 1.05)).count(), 2);
    }

    #[test]
    fn casement_and_awning_windows() {
        let (_, ov) = south();
        let mut m = base();
        window(&mut m, "casement");
        let l = lines_of(&m, &ov);
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(oriented(&l, |p| near(p, 6.55, 1.5)).iter().all(|x| near(x.0, 6.55, 1.5) && (x.1[0] - 7.45).abs() < 1e-6));
        let mut m = base();
        window(&mut m, "awning");
        let l = lines_of(&m, &ov);
        assert_eq!(l.len(), 2);
        assert!(oriented(&l, |p| near(p, 7.0, 2.1)).iter().all(|x| near(x.0, 7.0, 2.1) && (x.1[1] - 0.9).abs() < 1e-6));
    }

    #[test]
    fn sliders_and_fixed_have_none() {
        let (_, ov) = south();
        for s in ["slider", "fixed", "single-hung"] {
            let mut m = base();
            window(&mut m, s);
            assert!(lines_of(&m, &ov).is_empty(), "{s}");
        }
        for s in ["sliding", "pocket", "bifold", "fixed"] {
            let mut m = base();
            door(&mut m, s, Hinge::Left);
            assert!(lines_of(&m, &ov).is_empty(), "{s}");
        }
    }

    #[test]
    fn cabinet_doors_swing_from_their_hinge() {
        let mut m = ArchModel::default();
        let c = Cabinet { position: Vec2::new(5.0, 2.0), width: 1.2, depth: 0.6, height: 0.9, doors: 2, ..Cabinet::default() };
        m.push_cabinet(c);
        let ov = OrthoView::elevation([0.0, -1.0]).expect("view");
        let l = lines_of(&m, &ov);
        assert!(l.len() >= 2 && l.len() <= MAX_PER_ITEM, "{l:?}");
        assert!(l.iter().all(|x| x.kind == ArchKind::Cabinet));
        // Seen from behind there are none.
        assert!(lines_of(&m, &OrthoView::elevation([0.0, 1.0]).expect("view")).is_empty());
    }

    #[test]
    fn walls_seen_edge_on_have_none() {
        let mut m = base();
        door(&mut m, "hinged", Hinge::Left);
        let ov = OrthoView::elevation([1.0, 0.0]).expect("view");
        assert!(lines_of(&m, &ov).is_empty());
    }

    #[test]
    fn hidden_indicators_are_removed() {
        let (_, ov) = south();
        let mut m = base();
        door(&mut m, "hinged", Hinge::Left);
        let mut w = Wall::new(Vec2::new(-5.0, -2.0), Vec2::new(15.0, -2.0), 0.2);
        w.height = 3.0;
        m.push_wall(w);
        assert!(lines_of(&m, &ov).is_empty());
    }
}
