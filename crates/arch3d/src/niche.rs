//! Shower niches and shower doors.

use std::collections::BTreeMap;

use cadcraft_arch::opening::is_shower;
use cadcraft_doc::arch::paint::{MaterialRef, Side};
use cadcraft_doc::arch::{ArchModel, Finish, Hinge, ImageRgba, Opening, OpeningKind};
use cadcraft_doc::{ArchId, Wall};
use cadcraft_geom::Vec2;

use crate::Scene3d;
use crate::mesh::Mesh;
use crate::paint::ref_def;
use crate::texture::texture;

/// Thickness of a shower door's glass leaf, in metres.
const SHOWER_LEAF: f64 = 0.008;
/// Handle cross-section (along the wall and through it), in metres.
const HANDLE_SECTION: f64 = 0.02;
/// Handle length (vertical), in metres.
const HANDLE_LENGTH: f64 = 0.3;
/// How far the handle sits in from the leaf's free edge, in metres.
const HANDLE_INSET: f64 = 0.05;
/// Height of the handle's centre above the sill, in metres.
const HANDLE_HEIGHT: f64 = 1.0;
/// Thickness of a niche's back skin, in metres.
const SKIN: f64 = 0.001;

/// The wall's frame: unit direction and left normal.
fn frame(wall: &Wall) -> Option<(Vec2, Vec2)> {
    let d = wall.end - wall.start;
    if !(d.x.is_finite() && d.y.is_finite()) || d.len() < 1e-9 {
        return None;
    }
    let d = d.normalized();
    Some((d, Vec2::new(-d.y, d.x)))
}

/// Append the box `[u0, u1]` along the wall, `[n0, n1]` across it (from the centre line, positive to
/// the left) and `[z0, z1]` high to `mesh`.
fn push_box(mesh: &mut Mesh, wall: &Wall, dn: (Vec2, Vec2), u: (f64, f64), n: (f64, f64), z: (f64, f64)) {
    let (d, nrm) = dn;
    let p = |a: f64, b: f64| wall.start + d * a + nrm * b;
    mesh.push_prism(&[p(u.0, n.0), p(u.1, n.0), p(u.1, n.1), p(u.0, n.1)], z.0, z.1);
}

/// The recess's bottom across the wall (from the centre line, positive to the left) and the far
/// face, as `(recess_bottom, far_face)`.
fn recess_planes(wall: &Wall, o: &Opening) -> (f64, f64) {
    let t = wall.thickness / 2.0;
    match o.face {
        Some(Side::Right) => (-t + o.depth, t),
        _ => (t - o.depth, -t),
    }
}

/// The niches of `openings` that can be built: finite, positive recess inside the wall.
fn niches<'a>(wall: &Wall, openings: &'a [Opening]) -> impl Iterator<Item = &'a Opening> {
    let t = wall.thickness;
    openings.iter().filter(move |o| {
        o.kind == OpeningKind::Niche
            && o.depth.is_finite()
            && o.depth > 0.0
            && o.depth < t
            && o.offset.is_finite()
            && o.width.is_finite()
            && o.width > 0.0
            && o.height.is_finite()
            && o.height > 0.0
            && o.sill.is_finite()
    })
}

/// Add the back blocks of the niches in `openings` to the wall `mesh` (so finishes and paints split
/// them like the rest of the wall); `span` is the wall's (base, top).
pub fn add_back_blocks(mesh: &mut Mesh, wall: &Wall, openings: &[Opening], span: (f64, f64)) {
    let Some(dn) = frame(wall) else {
        return;
    };
    for o in niches(wall, openings) {
        let (near, far) = recess_planes(wall, o);
        let z0 = span.0 + o.sill;
        push_box(mesh, wall, dn, (o.offset - o.width / 2.0, o.offset + o.width / 2.0), (near.min(far), near.max(far)), (z0, z0 + o.height));
    }
}

/// Extra skins of the niches in `openings` (`niche-back*` materials): a 1 mm skin on the recess
/// bottom, owned by the wall.
pub fn back_skins(wall: &Wall, openings: &[Opening], span: (f64, f64)) -> Vec<Mesh> {
    let Some(dn) = frame(wall) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for o in niches(wall, openings) {
        let (near, _) = recess_planes(wall, o);
        // The skin lies inside the recess, on the open side of its bottom.
        let (n0, n1) = if o.face == Some(Side::Right) { (near - SKIN, near) } else { (near, near + SKIN) };
        let mut mesh = Mesh::new(&format!("niche-back:o{}", o.id.0), wall.id);
        let z0 = span.0 + o.sill;
        push_box(&mut mesh, wall, dn, (o.offset - o.width / 2.0, o.offset + o.width / 2.0), (n0, n1), (z0, z0 + o.height));
        if mesh.triangle_count() > 0 {
            out.push(mesh);
        }
    }
    out
}

/// The meshes of an opening that builds its own parts (a shower door or a niche), or `None` for
/// every other opening. `walls` are the walls by id, `span` the host wall's (base, top).
pub fn special_opening_meshes(walls: &BTreeMap<ArchId, &Wall>, o: &Opening, span: Option<(f64, f64)>) -> Option<Vec<Mesh>> {
    if o.kind == OpeningKind::Niche {
        return Some(Vec::new());
    }
    if !is_shower(o) {
        return None;
    }
    let Some(wall) = walls.get(&o.host) else {
        return Some(Vec::new());
    };
    let Some(dn) = frame(wall) else {
        return Some(Vec::new());
    };
    let (base, top) = span.unwrap_or((0.0, wall.height));
    let z0 = base + o.sill;
    let z1 = (z0 + o.height).min(top);
    if !(z0.is_finite() && z1.is_finite() && o.offset.is_finite() && o.width.is_finite()) || z1 <= z0 || o.width <= 0.0 {
        return Some(Vec::new());
    }
    let (u0, u1) = (o.offset - o.width / 2.0, o.offset + o.width / 2.0);
    let mut leaf = Mesh::new("glass-shower", o.id);
    push_box(&mut leaf, wall, dn, (u0, u1), (-SHOWER_LEAF / 2.0, SHOWER_LEAF / 2.0), (z0, z1));

    let free = if o.hinge == Hinge::Left { u1 } else { u0 };
    let sign = if o.hinge == Hinge::Left { -1.0 } else { 1.0 };
    let hu = free + sign * HANDLE_INSET;
    let hz = z0 + HANDLE_HEIGHT;
    let mut handle = Mesh::new("door-leaf-hardware", o.id);
    push_box(
        &mut handle,
        wall,
        dn,
        (hu - HANDLE_SECTION / 2.0, hu + HANDLE_SECTION / 2.0),
        (-HANDLE_SECTION / 2.0, HANDLE_SECTION / 2.0),
        (hz - HANDLE_LENGTH / 2.0, hz + HANDLE_LENGTH / 2.0),
    );
    let mut out = Vec::new();
    for m in [leaf, handle] {
        if m.triangle_count() > 0 {
            out.push(m);
        }
    }
    Some(out)
}

/// Fill the scene's materials for niche parts.
pub fn niche_materials(model: &ArchModel, scene: &mut Scene3d) {
    for o in model.openings.iter().filter(|o| o.kind == OpeningKind::Niche) {
        let Some(m) = o.material.as_deref().and_then(MaterialRef::parse) else {
            continue;
        };
        let def = ref_def(model, &m);
        if let Some(key) = def.texture.clone()
            && !scene.textures.contains_key(&key)
        {
            if let Some(f) = key.strip_prefix("finish:").and_then(Finish::parse) {
                let t = texture(f);
                let mut img = ImageRgba::default();
                img.width = t.width;
                img.height = t.height;
                img.rgba = t.rgba;
                scene.textures.insert(key, img);
            } else if let Some(img) = key.strip_prefix("item:").and_then(|id| model.images.get(id)) {
                scene.textures.insert(key, img.clone());
            }
        }
        scene.materials.insert(format!("niche-back:o{}", o.id.0), def);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_without_openings_has_no_niche_parts() {
        let wall = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, ..Default::default() };
        assert!(back_skins(&wall, &[], (0.0, 2.4)).is_empty());
        let mut m = Mesh::new("wall", wall.id);
        add_back_blocks(&mut m, &wall, &[], (0.0, 2.4));
        assert_eq!(m.triangle_count(), 0);
    }
}
