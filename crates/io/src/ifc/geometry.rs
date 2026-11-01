//! Shapes for the IFC export: a product's body is one `IfcTriangulatedFaceSet` made of the
//! [`Scene3d`] meshes that belong to it, in its storey's coordinates (z less the storey's
//! elevation), so the IFC geometry is the 3D view's geometry.

use crate::step::{P, Ref, StepWriter};
use cadcraft_arch3d::{Mesh, Scene3d};
use cadcraft_doc::arch::ArchId;
use std::collections::BTreeMap;

/// The kind of object a mesh's material belongs to: `wall`, `opening` (every `window-…` and
/// `door-…` part), `floor`, `ceiling`, `roof` (roof planes), `fascia`, `soffit`, `stair`,
/// `railing`, `deck`, `terrain`, `plant` (crown and trunk), `path` (`path-…`), `light` (`light-…`),
/// else `other`.
pub(crate) fn kind_of(material: &str) -> &'static str {
    match material {
        "wall" => "wall",
        "floor" => "floor",
        "ceiling" => "ceiling",
        "roof" => "roof",
        "fascia" => "fascia",
        "soffit" => "soffit",
        "stair" => "stair",
        "railing" => "railing",
        "deck" => "deck",
        "terrain" => "terrain",
        m if m.starts_with("window-") || m.starts_with("door-") => "opening",
        m if m.starts_with("plant-") => "plant",
        m if m.starts_with("path-") => "path",
        m if m.starts_with("light-") => "light",
        _ => "other",
    }
}

/// The scene's meshes by (kind, object), in scene order.
pub(crate) struct Pieces<'s> {
    by: BTreeMap<(&'static str, u32), Vec<&'s Mesh>>,
}

impl<'s> Pieces<'s> {
    pub(crate) fn new(scene: &'s Scene3d) -> Pieces<'s> {
        let mut by: BTreeMap<(&'static str, u32), Vec<&'s Mesh>> = BTreeMap::new();
        for m in &scene.meshes {
            by.entry((kind_of(&m.material), m.object.0)).or_default().push(m);
        }
        Pieces { by }
    }

    /// The meshes of object `id` of `kind` (empty when it built none).
    pub(crate) fn get(&self, kind: &str, id: ArchId) -> &[&'s Mesh] {
        self.by.iter().find(|((k, n), _)| *k == kind && *n == id.0).map_or(&[], |(_, v)| v.as_slice())
    }

    /// The objects of `kind` that built meshes, in id order.
    pub(crate) fn ids(&self, kind: &str) -> Vec<ArchId> {
        self.by.keys().filter(|(k, _)| *k == kind).map(|&(_, n)| ArchId(n)).collect()
    }
}

/// An `IfcTriangulatedFaceSet` over `points` (x, y, z in metres) and `triangles` (0-based indices
/// into `points`; written 1-based as IFC wants). `None` without a triangle, or when an index is out
/// of range or a coordinate is not finite (nothing invalid is ever written).
pub(crate) fn faceset(w: &mut StepWriter, points: &[[f64; 3]], triangles: &[[u32; 3]]) -> Option<Ref> {
    if triangles.is_empty() || points.iter().flatten().any(|c| !c.is_finite()) || triangles.iter().flatten().any(|&i| i as usize >= points.len()) {
        return None;
    }
    let list = w.add("IfcCartesianPointList3D", vec![P::List(points.iter().map(|p| P::reals(p)).collect())]);
    let coord_index = P::List(triangles.iter().map(|t| P::List(t.iter().map(|&i| P::Int(i64::from(i) + 1)).collect())).collect());
    // Closed is left unknown ($): not every mesh of the 3D build is a closed shell.
    Some(w.add("IfcTriangulatedFaceSet", vec![P::Ref(list), P::Null, P::Null, coord_index, P::Null]))
}

/// The `IfcProductDefinitionShape` of a product whose body is `meshes` (one face set for all of
/// them), with z measured from `z0` (its storey's elevation), in the `Body` context `body_context`.
/// `None` when the meshes hold no triangle.
pub(crate) fn body(w: &mut StepWriter, body_context: Ref, meshes: &[&Mesh], z0: f64) -> Option<Ref> {
    let mut points = Vec::new();
    let mut triangles = Vec::new();
    for m in meshes {
        let base = u32::try_from(points.len()).ok()?;
        points.extend(m.positions.iter().map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]) - z0]));
        for t in m.indices.chunks_exact(3) {
            triangles.push([base + t[0], base + t[1], base + t[2]]);
        }
    }
    let set = faceset(w, &points, &triangles)?;
    let shape = w.add("IfcShapeRepresentation", vec![P::Ref(body_context), P::str("Body"), P::str("Tessellation"), P::refs(&[set])]);
    Some(w.add("IfcProductDefinitionShape", vec![P::Null, P::Null, P::refs(&[shape])]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::StepHeader;

    fn square(material: &str, object: u32, z: f32) -> Mesh {
        Mesh {
            positions: vec![[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: material.into(),
            object: ArchId(object),
            ..Mesh::default()
        }
    }

    #[test]
    fn materials_map_to_their_objects_kind() {
        assert_eq!(kind_of("wall"), "wall");
        assert_eq!(kind_of("window-glass"), "opening");
        assert_eq!(kind_of("door-leaf"), "opening");
        assert_eq!(kind_of("plant-trunk"), "plant");
        assert_eq!(kind_of("path-asphalt-driveway"), "path");
        assert_eq!(kind_of("light-lens-lit"), "light");
        assert_eq!(kind_of("roof"), "roof");
        assert_eq!(kind_of("fascia"), "fascia");
        assert_eq!(kind_of("terrain"), "terrain");
        assert_eq!(kind_of("something"), "other");
    }

    #[test]
    fn pieces_group_a_scene_by_object() {
        let scene = Scene3d {
            meshes: vec![square("wall", 1, 0.0), square("window-glass", 0, 1.0), square("window-frame", 0, 1.0), square("wall", 0, 0.0)],
            ..Scene3d::default()
        };
        let pieces = Pieces::new(&scene);
        assert_eq!(pieces.ids("wall"), vec![ArchId(0), ArchId(1)]);
        assert_eq!(pieces.get("opening", ArchId(0)).len(), 2);
        assert!(pieces.get("opening", ArchId(1)).is_empty());
    }

    #[test]
    fn a_body_is_one_face_set_in_storey_coordinates() {
        let (a, b) = (square("wall", 0, 3.0), square("wall", 0, 5.5));
        let mut w = StepWriter::new();
        let ctx = w.add("IfcGeometricRepresentationSubContext", vec![]);
        assert!(body(&mut w, ctx, &[&a, &b], 3.0).is_some());
        let text = w.finish(&StepHeader::default());
        assert!(text.contains("#2=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(1.,0.,0.),(1.,1.,0.),(0.,1.,0.),(0.,0.,2.5),"), "{text}");
        assert!(text.contains("#3=IFCTRIANGULATEDFACESET(#2,$,$,((1,2,3),(1,3,4),(5,6,7),(5,7,8)),$);"), "{text}");
        assert!(text.contains("#4=IFCSHAPEREPRESENTATION(#1,'Body','Tessellation',(#3));"), "{text}");
        assert!(text.contains("#5=IFCPRODUCTDEFINITIONSHAPE($,$,(#4));"), "{text}");
    }

    #[test]
    fn nothing_invalid_is_written() {
        let mut w = StepWriter::new();
        assert!(faceset(&mut w, &[[0.0; 3]], &[]).is_none(), "no triangle");
        assert!(faceset(&mut w, &[[0.0; 3]], &[[0, 0, 1]]).is_none(), "an index past the points");
        assert!(faceset(&mut w, &[[f64::NAN, 0.0, 0.0], [0.0; 3], [0.0; 3]], &[[0, 1, 2]]).is_none(), "a NaN");
        assert!(w.is_empty());
    }
}
