//! Phase-15 cover plates for outlets and switches.

use std::collections::BTreeSet;

use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

use crate::{MaterialDef, Mesh, Scene3d};

/// The plate's standard size: width along the wall, depth proud of the face, height.
pub const PLATE: [f64; 3] = [0.07, 0.006, 0.115];

const BOX_EPS: f64 = 1e-9;

/// The plate's placed box used for probing before a mesh is built.
#[derive(Clone, Debug, PartialEq)]
pub struct PlateBox {
    pub id: ArchId,
    pub at: [f64; 2],
    pub along: [f64; 2],
    pub normal: [f64; 2],
    pub half: [f64; 2],
    pub depth: f64,
    pub z: [f64; 2],
    pub level: f64,
}

impl PlateBox {
    /// Whether `p` is inside the plate box: its sides and wall face count as inside, its outer
    /// face does not.
    pub fn contains(&self, p: Vec3) -> bool {
        if !p.is_finite() {
            return false;
        }
        let u = (p.x - self.at[0]) * self.along[0] + (p.y - self.at[1]) * self.along[1];
        let v = (p.x - self.at[0]) * self.normal[0] + (p.y - self.at[1]) * self.normal[1];
        u.abs() <= self.half[0] + BOX_EPS && v >= -BOX_EPS && v <= self.depth + BOX_EPS && p.z >= self.z[0] - BOX_EPS && p.z < self.z[1]
    }
}

/// The material key a plate uses in the scene.
pub fn plate_material(kind: ElectricalKind) -> String {
    format!("electrical:{}", kind.key())
}

/// The placed plate box on a level, exact and symbol-free for probing.
pub fn plate_box(d: &ElectricalDevice, place: &cadcraft_arch::electrical::DevicePlace, level: f64) -> PlateBox {
    PlateBox {
        id: d.id,
        at: [place.at.x, place.at.y],
        along: [place.along.x, place.along.y],
        normal: [place.normal.x, place.normal.y],
        half: [PLATE[0] / 2.0, PLATE[2] / 2.0],
        depth: PLATE[1],
        z: [place.z - PLATE[2] / 2.0, place.z + PLATE[2] / 2.0],
        level,
    }
}

/// Build the plate mesh for `d` at `place`; an absent place or an invalid location returns no mesh.
pub fn plate_meshes(d: &ElectricalDevice, place: Option<&cadcraft_arch::electrical::DevicePlace>) -> Vec<Mesh> {
    let Some(place) = place else {
        return Vec::new();
    };
    let half = PLATE[0] / 2.0;
    let depth = PLATE[1];
    let base = Vec2::new(place.at.x, place.at.y);
    let ring = [
        base + place.along * -half + place.normal * 0.0,
        base + place.along * half + place.normal * 0.0,
        base + place.along * half + place.normal * depth,
        base + place.along * -half + place.normal * depth,
    ];
    let mut mesh = Mesh::new(&plate_material(d.kind), d.id);
    let z0 = place.z - PLATE[2] / 2.0;
    let z1 = place.z + PLATE[2] / 2.0;
    mesh.push_prism(&ring, z0, z1);
    if mesh.indices.is_empty() { Vec::new() } else { vec![mesh] }
}

/// The placed box of `d` in `model`, on the level's elevation.
pub fn plate_box_of(model: &ArchModel, d: &ElectricalDevice) -> Option<PlateBox> {
    let place = cadcraft_arch::electrical::place_device(model, d).ok()?;
    Some(plate_box(d, &place, crate::level_for(model, d.level).elevation))
}

/// Add a material definition for each distinct plate kind in the scene.
pub fn plate_materials(model: &ArchModel, scene: &mut Scene3d) {
    let keys: BTreeSet<String> = model.electrical.iter().map(|d| plate_material(d.kind)).collect();
    let base = srgb_to_linear([0xf2, 0xf0, 0xea]);
    for key in keys {
        scene.materials.insert(key.clone(), MaterialDef { base_color: [base[0], base[1], base[2], 1.0], roughness: 0.6, ..Default::default() });
    }
}

fn srgb_to_linear(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    })
}

#[cfg(test)]
mod tests {
    use cadcraft_arch::electrical::place_device;
    use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind, WallSide};
    use cadcraft_doc::{ArchId, ArchModel, Wall};
    use cadcraft_geom::{Vec2, Vec3};

    use super::*;

    fn wall(model: &mut ArchModel, id: u32, start: Vec2, end: Vec2) -> ArchId {
        model.push_wall(Wall { id: ArchId(id), start, end, thickness: 0.2, height: 2.4, ..Wall::default() })
    }

    #[test]
    fn a_plate_is_a_closed_box_on_the_wall_face() {
        let mut model = ArchModel::default();
        model.levels.push(cadcraft_doc::Level { index: 0, elevation: 0.0, ..Default::default() });
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        let d = ElectricalDevice {
            id: ArchId(0),
            kind: ElectricalKind::Duplex,
            wall: w,
            side: WallSide::Left,
            offset: 1.5,
            height: 0.3,
            ..Default::default()
        };

        let place = place_device(&model, &d).unwrap();
        let meshes = plate_meshes(&d, Some(&place));
        assert_eq!(meshes.len(), 1);
        let mesh = &meshes[0];
        assert!(mesh.is_manifold());
        assert_eq!(mesh.material, "electrical:duplex");
        let bounds = mesh.bounds();
        assert!((bounds.max[0] - bounds.min[0] - 0.07).abs() < 1e-6);
        assert!((bounds.max[1] - bounds.min[1] - 0.006).abs() < 1e-6);
        assert!((bounds.max[2] - bounds.min[2] - 0.115).abs() < 1e-6);
        assert!((bounds.min[1] - place.at.y).abs() < 1e-6, "back face should sit on the wall face plane");
        let box_ = plate_box(&d, &place, 0.0);
        let center = Vec3::new(place.at.x, place.at.y, place.z);
        assert!(box_.contains(center));
        assert!(box_.contains(Vec3::new(place.at.x, place.at.y + 0.003, place.z)));
        assert!(!box_.contains(Vec3::new(place.at.x, place.at.y + 0.007, place.z)));
        assert!(!box_.contains(Vec3::new(place.at.x + 0.04, place.at.y, place.z)));
        assert!(!box_.contains(Vec3::new(place.at.x, place.at.y, place.z + 0.06)));
    }

    #[test]
    fn a_plate_that_cannot_be_placed_builds_nothing() {
        let mut model = ArchModel::default();
        model.levels.push(cadcraft_doc::Level { index: 0, elevation: 0.0, ..Default::default() });
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        let bad = ElectricalDevice { id: ArchId(0), wall: w, side: WallSide::Left, offset: 4.1, ..Default::default() };
        assert!(place_device(&model, &bad).is_err());
        assert!(plate_meshes(&bad, None).is_empty());
        assert!(plate_box_of(&model, &bad).is_none());

        let missing = ElectricalDevice { id: ArchId(1), wall: ArchId(123), side: WallSide::Right, offset: 1.5, ..Default::default() };
        assert!(place_device(&model, &missing).is_err());
        assert!(plate_box_of(&model, &missing).is_none());
    }

    #[test]
    fn plate_materials_are_off_white_and_rough() {
        let mut model = ArchModel::default();
        let mut scene = Scene3d::default();
        plate_materials(&model, &mut scene);
        assert!(scene.materials.is_empty());

        let mut model = ArchModel::default();
        model.levels.push(cadcraft_doc::Level { index: 0, elevation: 0.0, ..Default::default() });
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        model.push_electrical(ElectricalDevice {
            id: ArchId(0),
            kind: ElectricalKind::Duplex,
            wall: w,
            side: WallSide::Left,
            offset: 1.5,
            height: 0.3,
            ..Default::default()
        });

        scene = Scene3d::default();
        plate_materials(&model, &mut scene);
        let def = scene.materials.get("electrical:duplex").unwrap();
        assert!((def.roughness - 0.6).abs() < 1e-9);
        assert_eq!(def.metallic, 0.0);
        assert_eq!(def.base_color[3], 1.0);
        let expected = srgb_to_linear([0xf2, 0xf0, 0xea]);
        for i in 0..3 {
            assert!((def.base_color[i] - expected[i]).abs() < 1e-3, "offset at {i}: {} vs {}", def.base_color[i], expected[i]);
        }
    }
}
