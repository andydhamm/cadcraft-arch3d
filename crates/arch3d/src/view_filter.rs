//! Seeing in: cutaway hiding, per-camera clip boxes and near clips, and their
//! filtering of `pick3d`.
//!
//! `cutaway` hides the camera-facing exterior walls of the viewed level and
//! everything hosted in or on them; `clip` (a six-plane box) and `near` (a near
//! plane along the camera forward) are honoured by [`Scene3d::pick_filtered`].
//! The cutaway and clip/near filters are independent and both are applied.

use std::collections::BTreeSet;

use crate::Scene3d;
use crate::camera::Camera;
use crate::incremental::ObjectKind;
use crate::mesh::Mesh;
use cadcraft_doc::arch::{FurnishingHost, Look};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

/// A set of hidden objects, keyed by a part-class string and the object id, so
/// `hidden` is a total order even when the id set is empty.
pub type Hidden = BTreeSet<(&'static str, ArchId)>;

/// The part class used as the label prefix for a hidden object.
const PART_OF: &[(&'static str, ObjectKind)] = &[
    ("wall", ObjectKind::Wall),
    ("opening", ObjectKind::Opening),
    ("cabinet", ObjectKind::Cabinet),
    ("electrical", ObjectKind::Electrical),
    ("furnishing", ObjectKind::Furnishing),
    ("light", ObjectKind::Light),
];

/// Mixes a 64-bit value into a running hash (FNV-1a style); deterministic and
/// independent of insertion order.
fn combine(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x5BC96907F1C0439Cu64)
}

/// A six-plane clip box: three min and three max planes, inclusive of both.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl ClipBox {
    /// Whether `p` lies inside the box, boundaries included.
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y && p.z >= self.min.z && p.z <= self.max.z
    }
}

/// The floor overview's level limit: what lies above `elevation` is dropped, and with
/// `lower_levels` false what lies below it too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelFilter {
    /// The overview level's elevation, in metres.
    pub elevation: f64,
    /// Whether the levels below are kept.
    pub lower_levels: bool,
}

/// A filter that hides objects and clips/near-limits picks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewFilter {
    /// The floor overview's level limit (None: every level).
    pub levels: Option<LevelFilter>,
    /// Objects removed by the cutaway.
    pub hidden: Hidden,
    /// Inclusive six-plane clip box (None: no clipping).
    pub clip: Option<ClipBox>,
    /// Near clip along the camera forward (None: no near clip).
    pub near: Option<f64>,
    /// The camera eye.
    pub eye: Vec3,
    /// The camera forward (unit).
    pub forward: Vec3,
}

/// Build a filter from a saved `Look`, the model, and a live camera.
pub fn from_look(look: &Look, model: &ArchModel, camera: &Camera) -> ViewFilter {
    let eye = camera.eye();
    let forward = crate::camera::forward(camera);
    let hidden = if look.cutaway { cutaway_hidden(model, eye, level(model)) } else { Hidden::default() };
    let clip = look.clip.as_ref().map(|c| ClipBox { min: Vec3::new(c.min[0], c.min[1], c.min[2]), max: Vec3::new(c.max[0], c.max[1], c.max[2]) });
    ViewFilter { hidden, clip, near: look.near_clip, eye, forward, levels: None }
}

/// The viewed level: the model's `plan_level` (the level walls are drawn at) or 0.
fn level(model: &ArchModel) -> u32 {
    model.plan_level.unwrap_or(0)
}

impl ViewFilter {
    /// Build a filter from a saved `Look`, the model, and a live camera.
    pub fn from_look(look: &Look, model: &ArchModel, camera: &Camera) -> Self {
        let eye = camera.eye();
        let forward = crate::camera::forward(camera);
        let hidden = if look.cutaway { cutaway_hidden(model, eye, level(model)) } else { Hidden::default() };
        let clip = look.clip.as_ref().map(|c| ClipBox { min: Vec3::new(c.min[0], c.min[1], c.min[2]), max: Vec3::new(c.max[0], c.max[1], c.max[2]) });
        Self { hidden, clip, near: look.near_clip, eye, forward, levels: None }
    }

    /// The floor overview of `level`: higher levels, every ceiling and every roof leave the
    /// scene, and with `lower_levels` false the lower levels too. A cutaway (hidden set) is
    /// recomputed for the overview level.
    pub fn with_overview(mut self, model: &ArchModel, level: u32, lower_levels: bool) -> Self {
        let elevation = model.levels.iter().rev().find(|l| l.index == level).map_or(0.0, |l| l.elevation);
        if !self.hidden.is_empty() {
            self.hidden = cutaway_hidden(model, self.eye, level);
        }
        self.levels = Some(LevelFilter { elevation, lower_levels });
        self
    }

    /// Whether the filter is the no-op (nothing hidden, no clip, no near, no level limit).
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.clip.is_none() && self.near.is_none() && self.levels.is_none()
    }

    /// A stable key for caching; `0` is the no-op filter.
    pub fn key(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        let mut h: u64 = 0;
        if let Some(l) = &self.levels {
            h = combine(h, l.elevation.to_bits());
            h = combine(h, u64::from(l.lower_levels) + 1);
        }
        for (kind, id) in &self.hidden {
            h = combine(h, kind.len() as u64);
            h = combine(h, id.0 as u64);
        }
        if let Some(c) = &self.clip {
            let vals = [c.min.x, c.min.y, c.min.z, c.max.x, c.max.y, c.max.z];
            for &v in &vals {
                h = combine(h, v.to_bits());
            }
        }
        if let Some(n) = self.near {
            h = combine(h, n.to_bits());
        }
        h
    }

    /// Whether `mesh` is hidden by the cutaway filter.
    ///
    /// A mesh is hidden only if its `ObjectKind` (derived from its material/part) and its `ArchId`
    /// are present in the `hidden` set. This prevents hiding a room's floor/ceiling (kind `Room`)
    /// when a wall with the same `ArchId` is hidden.
    fn hides(&self, mesh: &Mesh) -> bool {
        let kind = crate::incremental::object_kind_of(mesh);
        if let Some((label, _)) = PART_OF.iter().find(|(_, k)| *k == kind) { self.hidden.contains(&(*label, mesh.object)) } else { false }
    }

    /// Drop the meshes (and furnishing/electrical boxes) of hidden objects and, for an overview,
    /// of the levels it leaves out; keep `scene.horizon`, the site and everything else.
    pub fn apply(&self, scene: &Scene3d) -> Scene3d {
        let mut s: Scene3d = scene.clone();
        if let Some(l) = self.levels {
            s.meshes.retain(|m| {
                if matches!(crate::probe::part_of(m), "roof" | "ceiling") {
                    return false;
                }
                // A mesh with no level (NaN) is kept.
                let e = m.level_elevation;
                !(e.is_finite() && (e > l.elevation + 1e-6 || (!l.lower_levels && e < l.elevation - 1e-6)))
            });
        }
        if self.hidden.is_empty() {
            return s;
        }
        s.meshes.retain(|m| !self.hides(m));
        s.furnishing_boxes.retain(|b| !self.hidden.contains(&("furnishing", b.id)));
        s.electrical_boxes.retain(|b| !self.hidden.contains(&("electrical", b.id)));
        s
    }

    /// Whether a world point is accepted: inside the clip box (inclusive, when set)
    /// and at least `near` along the camera forward from the eye.
    pub fn accepts(&self, p: Vec3) -> bool {
        if let Some(ref c) = self.clip {
            if !c.contains(p) {
                return false;
            }
        }
        if let Some(n) = self.near {
            let d = (p - self.eye).dot(self.forward);
            if d < n {
                return false;
            }
        }
        true
    }
}

/// The hidden set for a cutaway: exterior walls on `level` whose outer face
/// points at `eye`, plus everything hosted in or on them.
pub fn cutaway_hidden(model: &ArchModel, eye: Vec3, level: u32) -> Hidden {
    let mut hidden: Hidden = Hidden::default();
    let rooms: Vec<&cadcraft_doc::arch::Room> = model.rooms.iter().filter(|r| r.level == level).collect();
    let eye_p = Vec2::new(eye.x, eye.y);
    let walls_on_level: Vec<&cadcraft_doc::arch::Wall> = model.walls.iter().filter(|w| w.base_level == level || w.top_level == Some(level)).collect();
    let outer_centroid: Vec2 = if walls_on_level.is_empty() {
        Vec2::new(eye_p.x, eye_p.y)
    } else {
        let n = walls_on_level.len() as f64;
        let cx: f64 = walls_on_level.iter().map(|w| (w.start.x + w.end.x) / 2.0).sum();
        let cy: f64 = walls_on_level.iter().map(|w| (w.start.y + w.end.y) / 2.0).sum();
        Vec2::new(cx / n, cy / n)
    };

    for w in
        model.walls.iter().filter(|w| (w.base_level == level || w.top_level == Some(level)) && facing_exterior(w, &rooms, &outer_centroid, &eye_p))
    {
        push_hidden(&mut hidden, "wall", w.id);
    }

    for o in model.openings.iter() {
        if hidden.iter().any(|(_, id)| *id == o.host) {
            push_hidden(&mut hidden, "opening", o.id);
        }
    }

    for d in model.electrical.iter() {
        if hidden.iter().any(|(_, id)| *id == d.wall) {
            push_hidden(&mut hidden, "electrical", d.id);
        }
    }

    for f in model.furnishings.iter() {
        if let FurnishingHost::Wall { wall } = f.host {
            if hidden.iter().any(|(_, id)| *id == wall) {
                push_hidden(&mut hidden, "furnishing", f.id);
            }
        }
    }

    for lf in model.lights.iter() {
        let pos = Vec2::new(lf.position.x, lf.position.y);
        for (_, w_id) in hidden.iter() {
            if let Some(w) = model.walls.iter().find(|w| w.id == *w_id) {
                if point_to_segment_dist(pos, w.start, w.end) < w.thickness / 2.0 + 0.05 {
                    push_hidden(&mut hidden, "light", lf.id);
                    break;
                }
            }
        }
    }

    for cb in model.cabinets.iter() {
        if on_wall(&cb, model, &hidden) {
            push_hidden(&mut hidden, "cabinet", cb.id);
        }
    }

    hidden
}

/// Whether `w`'s outer face points toward `eye` (in plan).
///
/// `outer_centroid` is the average centre-line midpoint of the level's exterior walls; it
/// decides which side of a wall is "outer" when no room is on either side.
fn facing_exterior(w: &cadcraft_doc::arch::Wall, rooms: &[&cadcraft_doc::arch::Room], outer_centroid: &Vec2, eye: &Vec2) -> bool {
    let dx = w.end.x - w.start.x;
    let dy = w.end.y - w.start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return false;
    }
    let mid = Vec2::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0);
    let dir = Vec2::new(dx / len, dy / len);
    let normal = Vec2::new(-dir.y, dir.x);
    let half = w.thickness / 2.0;
    let side_a = Vec2::new(mid.x + normal.x * (half + 0.05), mid.y + normal.y * (half + 0.05));
    let side_b = Vec2::new(mid.x - normal.x * (half + 0.05), mid.y - normal.y * (half + 0.05));

    let in_a = rooms.iter().any(|r| point_in_ring(side_a, &r.polygon));
    let in_b = rooms.iter().any(|r| point_in_ring(side_b, &r.polygon));

    let outer_is_a;
    match (in_a, in_b) {
        (true, true) => return false,
        (true, false) => outer_is_a = false,
        (false, true) => outer_is_a = true,
        (false, false) => {
            let dist_a = side_a.dist(*outer_centroid);
            let dist_b = side_b.dist(*outer_centroid);
            if (dist_a - dist_b).abs() < 1e-9 {
                return true;
            }
            outer_is_a = dist_a >= dist_b;
        }
    }

    let outer_normal = if outer_is_a { normal } else { -normal };
    let to_eye = Vec2::new(eye.x - mid.x, eye.y - mid.y);
    outer_normal.dot(to_eye) > 0.0
}

/// A cabinet is "on" a hidden wall if its position is within 0.05 m of the
/// wall's centre line (back edge within 0.05 m of the inner face).
fn on_wall(cb: &cadcraft_doc::arch::Cabinet, model: &ArchModel, hidden: &Hidden) -> bool {
    let pos = Vec2::new(cb.position.x, cb.position.y);
    let mut near_any: bool = false;
    for (_, w_id) in hidden.iter() {
        if let Some(w) = model.walls.iter().find(|w| w.id == *w_id) {
            if point_to_segment_dist(pos, w.start, w.end) < w.thickness / 2.0 + 0.05 {
                near_any = true;
                break;
            }
        }
    }
    near_any
}

/// Distance from point `p` to line segment `a-b`.
fn point_to_segment_dist(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let ab_len_sq = ab.x * ab.x + ab.y * ab.y;
    if ab_len_sq < 1e-12 {
        (p - a).len()
    } else {
        let t = ((p - a).x * ab.x + (p - a).y * ab.y) / ab_len_sq;
        let t = if t < 0.0 {
            0.0
        } else if t > 1.0 {
            1.0
        } else {
            t
        };
        ((p - (a + ab * t)).len())
    }
}

/// Raycasting point-in-polygon test.
fn point_in_ring(p: Vec2, ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let a = ring[i];
        let b = ring[j];
        if (a.y > p.y) != (b.y > p.y) {
            let x_intersect = (a.x - b.x) * (p.y - a.y) / (a.y - b.y) + a.x;
            if p.x < x_intersect {
                inside = !inside;
            }
        }
    }
    inside
}

/// Insert a hidden entry.
fn push_hidden(hidden: &mut Hidden, kind: &'static str, id: ArchId) {
    hidden.insert((kind, id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Room, Wall};
    use cadcraft_geom::Vec2;

    fn room(rect: [[f64; 2]; 4]) -> Room {
        Room {
            id: ArchId(7),
            name: "A".to_string(),
            level: 0,
            polygon: rect.iter().map(|[x, y]| Vec2::new(*x, *y)).collect(),
            holes: Vec::new(),
            ..Room::default()
        }
    }

    #[test]
    fn cutaway_hides_facing_exterior_walls_and_hosted() {
        let wall = Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.2,
            base_level: 0,
            top_level: None,
            ..Wall::default()
        };
        let room = room([[0.1, 0.1], [3.9, 0.1], [3.9, 3.9], [0.1, 3.9]]);
        let model = ArchModel {
            walls: vec![wall.clone()],
            rooms: vec![room],
            openings: vec![cadcraft_doc::arch::Opening::new(wall.id, 2.0, 1.0, 1.8, cadcraft_doc::arch::OpeningKind::Window)],
            ..ArchModel::default()
        };
        let hidden = cutaway_hidden(&model, Vec3::new(2.0, -5.0, 1.2), 0);
        assert!(hidden.contains(&("wall", wall.id)));
        assert!(hidden.contains(&("opening", model.openings[0].id)));
    }

    #[test]
    fn from_inside_hides_nothing_and_level_only() {
        let wall = Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.2,
            base_level: 0,
            top_level: None,
            ..Wall::default()
        };
        let room = room([[0.1, 0.1], [3.9, 0.1], [3.9, 3.9], [0.1, 3.9]]);
        let model = ArchModel { walls: vec![wall], rooms: vec![room], ..ArchModel::default() };
        assert!(cutaway_hidden(&model, Vec3::new(1.5, 1.5, 1.0), 0).is_empty());
        assert!(cutaway_hidden(&model, Vec3::new(1.5, 1.5, 1.0), 1).is_empty());
    }

    #[test]
    fn outer_side_without_rooms_uses_centroid() {
        let wall = Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.2,
            base_level: 0,
            top_level: None,
            ..Wall::default()
        };
        let model = ArchModel { walls: vec![wall], ..ArchModel::default() };
        let hidden = cutaway_hidden(&model, Vec3::new(2.0, 5.0, 1.0), 0);
        assert!(hidden.contains(&("wall", ArchId(1))));
    }

    #[test]
    fn accepts_clip_and_near() {
        let f = ViewFilter {
            hidden: Hidden::default(),
            clip: Some(ClipBox { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 3.0) }),
            near: Some(1.0),
            eye: Vec3::new(0.0, 0.0, 0.0),
            forward: Vec3::new(0.0, 0.0, 1.0),
            levels: None,
        };
        assert!(f.accepts(Vec3::new(0.0, 0.0, 2.0)));
        assert!(!f.accepts(Vec3::new(0.0, 0.0, 0.5)));
        assert!(!f.accepts(Vec3::new(2.0, 0.0, 2.0)));
    }

    #[test]
    fn overview_drops_higher_levels_ceilings_roofs_lower_optional() {
        use crate::mesh::Mesh;
        use cadcraft_doc::arch::Level;
        let model = ArchModel {
            levels: vec![
                Level { index: 0, name: "Ground".into(), elevation: 0.0, ..Level::default() },
                Level { index: 1, name: "Upper".into(), elevation: 3.0, ..Level::default() },
            ],
            ..ArchModel::default()
        };
        let mesh = |material: &str, id: u32, e: f64| {
            let mut m = Mesh::new(material, ArchId(id));
            m.level_elevation = e;
            m
        };
        let scene = Scene3d {
            meshes: vec![
                mesh("floor", 1, 0.0),
                mesh("ceiling", 2, 0.0),
                mesh("roof", 3, f64::NAN),
                mesh("floor", 4, 3.0),
                mesh("terrain", 5, f64::NAN),
            ],
            ..Scene3d::default()
        };
        let ids = |f: &ViewFilter| f.apply(&scene).meshes.iter().map(|m| m.object.0).collect::<Vec<_>>();
        let base = ViewFilter::default();
        assert_eq!(ids(&base.clone().with_overview(&model, 0, true)), vec![1, 5], "level 0: no upper floor, ceiling or roof");
        assert_eq!(ids(&base.clone().with_overview(&model, 1, true)), vec![1, 4, 5], "level 1 with the lower level");
        assert_eq!(ids(&base.clone().with_overview(&model, 1, false)), vec![4, 5], "level 1 alone");
        assert_ne!(base.clone().with_overview(&model, 1, true).key(), base.clone().with_overview(&model, 1, false).key());
        assert_eq!(base.key(), 0);
    }

    #[test]
    fn hidden_wall_keeps_the_room_with_the_same_id() {
        use crate::mesh::Mesh;

        let mut wall0 = Mesh::new("wall", ArchId(0));
        wall0.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        wall0.indices = vec![0, 1, 2];

        let mut ceiling0 = Mesh::new("ceiling", ArchId(0));
        ceiling0.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        ceiling0.indices = vec![0, 1, 2];

        let mut floor0 = Mesh::new("floor", ArchId(0));
        floor0.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        floor0.indices = vec![0, 1, 2];

        let mut wall1 = Mesh::new("wall", ArchId(1));
        wall1.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        wall1.indices = vec![0, 1, 2];

        let scene = Scene3d { meshes: vec![wall0, ceiling0, floor0, wall1], ..Scene3d::default() };

        let mut hidden = Hidden::default();
        hidden.insert(("wall", ArchId(0)));

        let filter = ViewFilter { hidden, ..ViewFilter::default() };

        let filtered = filter.apply(&scene);

        assert_eq!(filtered.meshes.len(), 3);
        assert!(!filtered.meshes.iter().any(|m| m.material == "wall" && m.object == ArchId(0)));
        assert!(filtered.meshes.iter().any(|m| m.material == "ceiling" && m.object == ArchId(0)));
        assert!(filtered.meshes.iter().any(|m| m.material == "floor" && m.object == ArchId(0)));
        assert!(filtered.meshes.iter().any(|m| m.material == "wall" && m.object == ArchId(1)));
    }

    #[test]
    fn key_near_precision() {
        let base = ViewFilter::default();
        let f1 = ViewFilter { near: Some(1.1), ..base.clone() };
        let f2 = ViewFilter { near: Some(1.9), ..base.clone() };
        assert_ne!(f1.key(), f2.key(), "different near values must produce different keys");
    }

    #[test]
    fn key_clip_precision() {
        let base = ViewFilter::default();
        let f1 = ViewFilter { clip: Some(ClipBox { min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(1.1, 1.0, 1.0) }), ..base.clone() };
        let f2 = ViewFilter { clip: Some(ClipBox { min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(1.9, 1.0, 1.0) }), ..base.clone() };
        assert_ne!(f1.key(), f2.key(), "different clip values must produce different keys");
    }

    #[test]
    fn cutaway_spanning_wall_hidden_at_both_levels() {
        let wall = Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.2,
            base_level: 0,
            top_level: Some(1),
            ..Wall::default()
        };
        let room = room([[0.1, 0.1], [3.9, 0.1], [3.9, 3.9], [0.1, 3.9]]);
        let model = ArchModel {
            walls: vec![wall],
            rooms: vec![room],
            levels: vec![
                cadcraft_doc::arch::Level { index: 0, name: "Ground".into(), elevation: 0.0, ..cadcraft_doc::arch::Level::default() },
                cadcraft_doc::arch::Level { index: 1, name: "Upper".into(), elevation: 3.0, ..cadcraft_doc::arch::Level::default() },
            ],
            ..ArchModel::default()
        };
        let hidden0 = cutaway_hidden(&model, Vec3::new(2.0, -5.0, 1.0), 0);
        let hidden1 = cutaway_hidden(&model, Vec3::new(2.0, -5.0, 4.0), 1);
        assert!(hidden0.contains(&("wall", ArchId(1))), "wall should be hidden at level 0");
        assert!(hidden1.contains(&("wall", ArchId(1))), "wall should be hidden at level 1");
    }

    #[test]
    fn cutaway_zero_thickness_wall() {
        let wall = Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.0,
            base_level: 0,
            top_level: None,
            ..Wall::default()
        };
        let model = ArchModel { walls: vec![wall], ..ArchModel::default() };
        let hidden = cutaway_hidden(&model, Vec3::new(2.0, -5.0, 1.0), 0);
        assert!(hidden.contains(&("wall", ArchId(1))), "zero-thickness wall should be hidden when facing");
    }

    #[test]
    fn apply_levels_exact_elevation_kept() {
        use crate::mesh::Mesh;
        use cadcraft_doc::arch::Level;
        let model = ArchModel {
            levels: vec![
                Level { index: 0, name: "Ground".into(), elevation: 0.0, ..Level::default() },
                Level { index: 1, name: "Upper".into(), elevation: 3.0, ..Level::default() },
            ],
            ..ArchModel::default()
        };
        let mesh = |material: &str, id: u32, e: f64| {
            let mut m = Mesh::new(material, ArchId(id));
            m.level_elevation = e;
            m
        };
        let scene = Scene3d { meshes: vec![mesh("floor", 1, 0.0), mesh("wall", 2, 0.0)], ..Scene3d::default() };
        let f0 = ViewFilter::default().with_overview(&model, 0, true);
        let filtered0 = f0.apply(&scene);
        assert_eq!(filtered0.meshes.len(), 2, "level 0 overview should keep meshes at elevation 0");

        let f1 = ViewFilter::default().with_overview(&model, 1, false);
        let filtered1 = f1.apply(&scene);
        assert_eq!(filtered1.meshes.len(), 0, "level 1 overview without lower levels should drop elevation 0 meshes");
    }

    #[test]
    fn accepts_clip_boundary() {
        let f = ViewFilter {
            hidden: Hidden::default(),
            clip: Some(ClipBox { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) }),
            near: None,
            eye: Vec3::new(0.0, 0.0, 0.0),
            forward: Vec3::new(0.0, 0.0, 1.0),
            levels: None,
        };
        assert!(f.accepts(Vec3::new(1.0, 0.0, 0.0)), "point on max-x boundary should be accepted");
        assert!(f.accepts(Vec3::new(-1.0, 0.0, 0.0)), "point on min-x boundary should be accepted");
        assert!(f.accepts(Vec3::new(0.0, 1.0, 0.0)), "point on max-y boundary should be accepted");
        assert!(f.accepts(Vec3::new(0.0, 0.0, 1.0)), "point on max-z boundary should be accepted");
        assert!(!f.accepts(Vec3::new(1.0001, 0.0, 0.0)), "point just outside max-x should be rejected");
        assert!(!f.accepts(Vec3::new(-1.0001, 0.0, 0.0)), "point just outside min-x should be rejected");
    }

    #[test]
    fn hidden_opening_keeps_the_wall_with_the_same_id() {
        use crate::incremental::ObjectKind;
        use crate::incremental::object_kind_of;
        use crate::mesh::Mesh;

        let mut wall2 = Mesh::new("wall", ArchId(2));
        wall2.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        wall2.indices = vec![0, 1, 2];

        let mut opening2 = Mesh::new("frame", ArchId(2));
        opening2.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        opening2.indices = vec![0, 1, 2];

        assert_eq!(object_kind_of(&opening2), ObjectKind::Opening);

        let scene = Scene3d { meshes: vec![wall2, opening2], ..Scene3d::default() };

        let mut hidden = Hidden::default();
        hidden.insert(("opening", ArchId(2)));

        let filter = ViewFilter { hidden, ..ViewFilter::default() };

        let filtered = filter.apply(&scene);

        assert_eq!(filtered.meshes.len(), 1);
        assert_eq!(filtered.meshes[0].material, "wall");
        assert_eq!(filtered.meshes[0].object, ArchId(2));
    }
}
