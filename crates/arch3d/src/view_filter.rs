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
use cadcraft_arch::furnishing::FurnishingHost;
use cadcraft_doc::arch::Look;
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

/// A filter that hides objects and clips/near-limits picks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewFilter {
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
    ViewFilter { hidden, clip: look.clip, near: look.near_clip, eye, forward }
}

/// The viewed level: the model's `plan_level` (the level walls are drawn at) or 0.
fn level(model: &ArchModel) -> u32 {
    model.plan_level.unwrap_or(0)
}

impl ViewFilter {
    /// Whether the filter is the no-op (nothing hidden, no clip, no near).
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.clip.is_none() && self.near.is_none()
    }

    /// A stable key for caching; `0` is the no-op filter.
    pub fn key(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        let mut h: u64 = 0;
        for (kind, id) in &self.hidden {
            h = combine(h, kind.len() as u64);
            h = combine(h, id.0);
        }
        if let Some(c) = self.clip {
            for &v in [c.min.x as i64, c.min.y as i64, c.min.z as i64, c.max.x as i64, c.max.y as i64, c.max.z as i64] {
                h = combine(h, (v.wrapping_mul(-1)) as u64);
            }
        }
        if let Some(n) = self.near {
            h = combine(h, (n as i64) as u64);
        }
        h
    }

    /// Drop the meshes (and furnishing/electrical boxes) of hidden objects, keep
    /// `scene.horizon` and everything else.
    pub fn apply(&self, scene: &Scene3d) -> Scene3d {
        let mut s: Scene3d = scene.clone();
        if self.hidden.is_empty() {
            return s;
        }
        let hidden: BTreeSet<ArchId> = self.hidden.iter().map(|(_, id)| *id).collect();
        s.meshes.retain(|m| !hidden.contains(&m.object));
        s.furnishing_boxes.retain(|b| !hidden.contains(&b.id));
        s.electrical_boxes.retain(|b| !hidden.contains(&b.id));
        s
    }

    /// Whether a world point is accepted: inside the clip box (inclusive, when set)
    /// and at least `near` along the camera forward from the eye.
    pub fn accepts(&self, p: Vec3) -> bool {
        if let Some(c) = self.clip {
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

    for w in model.walls.iter().filter(|w| (w.base_level == level || w.top_level == Some(level)) && facing_exterior(w, &rooms, &eye_p)) {
        push_hidden(&mut hidden, "wall", w.id);
    }

    for o in model.openings.iter() {
        if let Some(h) = o.host {
            if hidden.iter().any(|(_, id)| *id == h) {
                push_hidden(&mut hidden, "opening", o.id);
            }
        }
    }

    for d in model.electrical.iter() {
        if let Some(w) = d.wall {
            if hidden.iter().any(|(_, id)| *id == w) {
                push_hidden(&mut hidden, "electrical", d.id);
            }
        }
    }

    for f in model.furnishing.iter() {
        if let Some(FurnishingHost::Wall(w)) = f.host {
            if hidden.iter().any(|(_, id)| *id == w) {
                push_hidden(&mut hidden, "furnishing", f.id);
            }
        }
    }

    for lf in model.light.iter() {
        if let Some(w) = lf.wall {
            if hidden.iter().any(|(_, id)| *id == w) {
                push_hidden(&mut hidden, "light", lf.id);
            }
        }
    }

    for cb in model.casework.iter() {
        if on_wall(cb, &hidden) {
            push_hidden(&mut hidden, "cabinet", cb.id);
        }
    }

    hidden
}

/// Whether `w`'s outer face points toward `eye` (in plan).
fn facing_exterior(w: &cadcraft_doc::arch::Wall, rooms: &[&cadcraft_doc::arch::Room], eye: &Vec2) -> bool {
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
            let centroid = Vec2::new(mid.x, mid.y);
            outer_is_a = side_a.dist(centroid) >= side_b.dist(centroid);
        }
    }

    let outer_normal = if outer_is_a { normal } else { -normal };
    let to_eye = Vec2::new(eye.x - mid.x, eye.y - mid.y);
    outer_normal.dot(to_eye) > 0.0
}

/// A cabinet is "on" a hidden wall if its host wall is in the hidden set.
fn on_wall(cb: &cadcraft_doc::arch::Cabinet, hidden: &Hidden) -> bool {
    if let Some(w) = cb.wall { hidden.iter().any(|(_, id)| *id == w) } else { false }
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
