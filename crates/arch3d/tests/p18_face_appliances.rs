//! Phase 18 face appliances in 3D: the proud front box and its mesh owner metadata.

use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Cabinet, CabinetKind, FaceKind, FaceLayout, FaceRow, Fronts, Level};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn model_of(c: Cabinet) -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.push_cabinet(c);
    m
}

fn appliance_face() -> FaceLayout {
    FaceLayout {
        rows: vec![
            FaceRow { kind: FaceKind::Appliance, height: Some(0.737), across: 1, appliance: Some("appliance:wall-oven".to_string()) },
            FaceRow { kind: FaceKind::Door, height: None, across: 0, ..FaceRow::default() },
        ],
        ..FaceLayout::default()
    }
}

#[test]
fn appliance_box_stands_5_mm_proud() {
    let mut c = Cabinet {
        id: ArchId(0),
        kind: CabinetKind::Tall,
        position: Vec2::new(0.0, 0.0),
        width: 0.762,
        depth: 0.6,
        height: 2.034,
        fronts: Fronts::Doors,
        doors: 2,
        toe_kick: true,
        ..Default::default()
    };
    c.face = Some(appliance_face());
    let model = model_of(c);
    let cabinet_id = model.cabinets[0].id;
    let s = Scene3d::build(&model);
    let hit = s.probe(Vec3::new(0.0, 0.603, 1.6)).expect("appliance front should be hit by a probe 5 mm in front of the plane");
    assert_eq!(hit.part, "cabinet");
    assert!(hit.id.starts_with("cb"), "probe hit id should belong to the cabinet: {:?}", hit.id);
    assert!(s.probe(Vec3::new(0.0, 0.603, 0.55)).is_none(), "the door beneath the oven should stay flush");
    assert!(s.meshes.iter().any(|m| m.material == "cabinet-appliance" && m.object == cabinet_id));
}

#[test]
fn appliance_mesh_part_and_owner() {
    let mut c = Cabinet {
        id: ArchId(0),
        kind: CabinetKind::Tall,
        position: Vec2::new(0.5, 0.0),
        width: 0.762,
        depth: 0.6,
        height: 2.034,
        fronts: Fronts::Doors,
        doors: 2,
        toe_kick: true,
        ..Default::default()
    };
    c.face = Some(appliance_face());
    let model = model_of(c);
    let cabinet_id = model.cabinets[0].id;
    let s = Scene3d::build(&model);
    let mesh = s.meshes.iter().find(|m| m.material == "cabinet-appliance" && m.object == cabinet_id).expect("appliance mesh");
    assert!(!mesh.positions.is_empty());
    let hit = s.probe(Vec3::new(0.5, 0.603, 1.6)).expect("appliance should be probeable");
    assert_eq!(hit.part, "cabinet");
    assert_eq!(hit.id, format!("cb{}", cabinet_id.0));
}
