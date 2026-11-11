//! Seeing in: cutaway hiding, per-camera clip boxes and near clips, and their
//! filtering of `pick3d`.
//!
//! `cutaway` hides the camera-facing exterior walls of the viewed level and
//! everything hosted in or on them; `clip` (a six-plane box) and `near` (a near
//! plane along the camera forward) are honoured by [`Scene3d::pick_filtered`].
//! The cutaway and clip/near filters are independent and both are applied.

use std::collections::BTreeSet;

use crate::{Camera, Scene3d};
use crate::mesh::Mesh;
use cadcraft_arch::{FurnishingHost, ObjectKind};
use cadraft_doc::arch::Look;
use cadcraft_doc::{ArchId, ArchModel, Vec3};

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

/// A six-plane clip box: three min and three max planes, inclusive of both.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl ClipBox {
    /// Whether `p` lies inside the box, boundaries included.
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
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
    let hidden = if look.cutaway {
        cutaway_hidden(model, eye, level(model))
    } else {
        Hidden::default()
    };
    ViewFilter {
        hidden,
        clip: look.clip,
        near: look.near_clip,
        eye,
        forward,
    }
}

/// The viewed level: the model's `plan_level` (the level walls are drawn at) or 0.
fn level(model: &ArchModel) -> u32 {
    model.plan_level
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
        let mut h = self.clip.map_or(0, |c| h64((c.min.x as i64, c.min.y as i64, c.min.z as i64, c.max.x as i64, c.max.y as i64, c.max.z as i64)));
        if let Some(n) = self.near {
            h = h64((h, n as i64));
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
        let mut hidden: BTreeSet<ArchId> = BTreeSet::new();
        for (_, id) in &self.hidden {
            hidden.insert(id);
        }
        s.meshes.retain(|m| !hidden.contains(&m.object));
        s.furnishing_boxes.retain(|b| !hidden.contains(&b.object));
        s.electrical_boxes.retain(|b| !hidden.contains(&b.object));
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
    let rooms: Vec<&cadraft_doc::arch::Room> = model.rooms.iter().filter(|r| r.level == level).collect();

    for w in model.walls.iter().filter(|w| {
        // Only exterior walls on the level.
        (w.base_level == level || w.top_level == Some(level))
            && w.level_elevation.is_some()
            && is_exterior(&w, model, rooms, level)
    }) {
        push_hidden(&mut hidden, "wall", w.id);
    }

    for o in model.openings.iter() {
        if let Some(h) = o.host {
            if hidden.iter().any(|(_, id)| id == h && id.0 == h.0) {
                push_hidden(&mut hidden, "opening", o.id);
            }
        }
    }

    for d in model.electrical.iter() {
        if let Some(w) = d.wall {
            if hidden.iter().any(|(_, id)| id == w) {
                push_hidden(&mut hidden, "electrical", d.id);
            }
        }
    }

    for f in model.furnishing.iter() {
        if let Some(FurnishingHost::Wall(w)) = f.host {
            if hidden.iter().any(|(_, id)| id == w) {
                push_hidden(&mut hidden, "furnishing", f.id);
            }
        }
    }

    for lf in model.light.iter() {
        if is_wall_mounted(lf, model, level) && hidden.iter().any(|(_, id)| id == lf.wall) {
            push_hidden(&mut hidden, "light", lf.id);
        }
    }

    for cb in model.casework.iter() {
        if on_wall(&cb, model, level, &hidden) {
            push_hidden(&mut hidden, "cabinet", cb.id);
        }
    }

    hidden
}

impl ViewFilter {
    fn push_hidden(&mut hidden, ...)
}

#[inline]
fn h64(v: (u64, i64)) -> u64 {
    (v.0.wrapping_mul(0x9e3779b97f4a7c15)) ^ (v.1 as u64)
}
