//! Electrical devices (phase 15): outlets and switches.

use super::Ctx;
use crate::step::P;
use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind, electrical_label};
use std::collections::BTreeSet;

/// One `IfcOutlet` (`.POWEROUTLET.`) per outlet and one `IfcSwitchingDevice` (`.DIMMERSWITCH.` for
/// dimmers, `.TOGGLESWITCH.` for single-pole and three-way switches) per switch, key
/// `electrical/<n>`, name `electrical-el<n>`, its body the plate meshes
/// (`ctx.pieces.get("electrical", id)`), in its level storey, with a `CADCraft_Electrical` property
/// set (`Kind`, `Height`, `Wall`). A repeated id is written once.
pub(super) fn emit(ctx: &mut Ctx) {
    let mut seen = BTreeSet::new();
    let devices = ctx.model.electrical.clone();
    for d in &devices {
        if seen.insert(d.id.0) {
            emit_one(ctx, d);
        }
    }
}

fn emit_one(ctx: &mut Ctx, d: &ElectricalDevice) {
    let key = format!("electrical/{}", d.id.0);
    let name = format!("electrical-{}", electrical_label(d.id));
    let meshes = ctx.pieces.get("electrical", d.id).to_vec();
    let slot = ctx.storey_for(d.level);
    let (entity, predefined) = if d.kind.is_switch() {
        let p = if d.kind == ElectricalKind::Dimmer { "DIMMERSWITCH" } else { "TOGGLESWITCH" };
        ("IfcSwitchingDevice", p)
    } else {
        ("IfcOutlet", "POWEROUTLET")
    };
    let element = ctx.element(entity, &key, &name, slot, &meshes, vec![P::Null, P::Enum(predefined)]);
    let height = if d.height.is_finite() { P::typed("IFCLENGTHMEASURE", P::Real(d.height)) } else { P::Null };
    ctx.pset(
        "CADCraft_Electrical",
        &key,
        vec![
            ("Kind", P::typed("IFCLABEL", P::str(d.kind.key()))),
            ("Height", height),
            ("Wall", P::typed("IFCIDENTIFIER", P::str(format!("w{}", d.wall.0)))),
        ],
        &[element],
    );
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchId, ArchModel, ElectricalDevice, ElectricalKind, Level};

    fn device(kind: ElectricalKind) -> ElectricalDevice {
        ElectricalDevice { kind, wall: ArchId(0), offset: 1.0, ..Default::default() }
    }

    fn base() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, ..Level::default() }];
        m
    }

    #[test]
    fn ifc_has_outlets_and_switching_devices() {
        let mut m = base();
        m.push_electrical(device(ElectricalKind::Duplex));
        m.push_electrical(device(ElectricalKind::Dimmer));
        m.push_electrical(device(ElectricalKind::SinglePole));
        let text = write_model(&m, &Scene3d::default(), &IfcOptions::default());
        assert_eq!(text.matches("=IFCOUTLET(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCSWITCHINGDEVICE(").count(), 2, "{text}");
        for n in 0..3 {
            assert!(text.contains(&format!(",'electrical-el{n}',$,$,#")), "{text}");
        }
        assert_eq!(text.matches(",.POWEROUTLET.);").count(), 1, "{text}");
        assert_eq!(text.matches(",.DIMMERSWITCH.);").count(), 1, "{text}");
        assert_eq!(text.matches(",.TOGGLESWITCH.);").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELCONTAINEDINSPATIALSTRUCTURE(").count(), 1, "{text}");
        let rel = text.lines().find(|l| l.contains("=IFCRELCONTAINEDINSPATIALSTRUCTURE(")).unwrap_or_default();
        let mut contained = 0;
        for entity in ["=IFCOUTLET(", "=IFCSWITCHINGDEVICE("] {
            for line in text.lines().filter(|l| l.contains(entity)) {
                let id = line.split('=').next().unwrap_or_default();
                assert!(rel.contains(&format!("{id},")) || rel.contains(&format!("{id})")), "{id} in {rel}");
                contained += 1;
            }
        }
        assert_eq!(contained, 3, "all three elements are contained: {rel}");
    }

    #[test]
    fn ifc_electrical_property_set_carries_kind_height_and_wall() {
        let mut m = base();
        m.push_electrical(ElectricalDevice { height: 0.3, ..device(ElectricalKind::Gfci) });
        let text = write_model(&m, &Scene3d::default(), &IfcOptions::default());
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Kind',$,IFCLABEL('gfci'),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Height',$,IFCLENGTHMEASURE(0.3),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Wall',$,IFCIDENTIFIER('w0'),$)"), "{text}");
        assert!(text.contains("'CADCraft_Electrical'"), "{text}");
    }

    #[test]
    fn an_empty_model_writes_no_electrical_entities() {
        let text = write_model(&ArchModel::default(), &Scene3d::default(), &IfcOptions::default());
        assert!(!text.contains("=IFCOUTLET("));
        assert!(!text.contains("=IFCSWITCHINGDEVICE("));
        assert!(!text.contains("CADCraft_Electrical"));
    }
}
