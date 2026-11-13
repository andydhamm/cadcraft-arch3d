//! Light fixtures (phase 12).

use super::Ctx;
use crate::step::P;
use cadcraft_doc::arch::LightKind;

/// One `IfcLightFixture` (key `light/<n>`, name `light-lf<n>`; `.POINTSOURCE.` for point and spot
/// lights, `.DIRECTIONSOURCE.` for area panels) per fixture, its body the housing meshes
/// (`ctx.pieces.get("light", id)`), in its level storey, with a `CADCraft_Light` property set
/// (`Type`, `Lumens`, `ColourTemperature` in kelvin, `On`).
pub(super) fn emit(ctx: &mut Ctx) {
    for fixture in &ctx.model.lights {
        let n = fixture.id.0;
        let slot = ctx.storey_for(fixture.level);
        let meshes = ctx.pieces.get("light", fixture.id).to_vec();
        let r = ctx.element(
            "IfcLightFixture",
            &format!("light/{n}"),
            &format!("light-lf{n}"),
            slot,
            &meshes,
            vec![
                P::Null,
                match fixture.kind {
                    LightKind::Point | LightKind::Spot => P::Enum("POINTSOURCE"),
                    LightKind::Area | LightKind::Strip => P::Enum("DIRECTIONSOURCE"),
                },
            ],
        );
        ctx.pset(
            "CADCraft_Light",
            &format!("light/{n}"),
            vec![
                ("Type", P::typed("IFCLABEL", P::str(fixture.fixture.clone()))),
                ("Lumens", P::typed("IFCLUMINOUSFLUXMEASURE", P::Real(fixture.lumens))),
                ("ColourTemperature", P::typed("IFCTHERMODYNAMICTEMPERATUREMEASURE", P::Real(fixture.kelvin))),
                ("On", P::typed("IFCBOOLEAN", P::Bool(fixture.on))),
            ],
            &[r],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchId, ArchModel, Level, LightFixture, LightKind};
    use cadcraft_geom::Vec2;

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, name: "Ground".into(), ..Level::default() }, Level { index: 2, elevation: 4.0, ..Level::default() }];
        m.lights = vec![
            LightFixture {
                id: ArchId(0),
                kind: LightKind::Point,
                level: 0,
                fixture: "Recessed pot light".into(),
                lumens: 800.0,
                kelvin: 3000.0,
                on: true,
                ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 2.0))
            },
            LightFixture {
                id: ArchId(3),
                kind: LightKind::Area,
                level: 2,
                fixture: "LED panel 600".into(),
                lumens: 1200.0,
                kelvin: 4000.0,
                on: false,
                width: 1.2,
                length: 0.6,
                ..LightFixture::new(ArchId(3), "LED panel 600", Vec2::new(3.0, 4.0))
            },
        ];
        m
    }

    #[test]
    fn each_fixture_is_an_ifclightfixture_in_its_storey() {
        let m = model();
        let text = write_model(&m, &Scene3d::default(), &IfcOptions::default());
        assert_eq!(text.matches("=IFCLIGHTFIXTURE(").count(), 2, "{text}");
        assert!(text.contains(",'light-lf0',$,$,#") && text.contains(",'light-lf3',$,$,#"), "{text}");
        assert!(text.contains(",.POINTSOURCE.);") && text.contains(",.DIRECTIONSOURCE.);"), "{text}");
        assert_eq!(text.matches("=IFCRELCONTAINEDINSPATIALSTRUCTURE(").count(), 2, "both fixtures belong to their storeys");
    }

    #[test]
    fn strip_dxf_proxy_and_ifc_light_fixture() {
        use cadcraft_doc::arch::{FixtureShape, StripFacing};
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, name: "Ground".into(), ..Level::default() }];
        m.lights = vec![LightFixture {
            kind: LightKind::Strip,
            shape: FixtureShape::Strip,
            path: vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)],
            lumens_per_metre: 500.0,
            lumens: 1000.0,
            facing: StripFacing::Down,
            height: Some(2.4),
            ..LightFixture::new(ArchId(0), "LED strip", Vec2::new(2.0, 2.0))
        }];

        // The DXF plan proxy is the path as an LWPOLYLINE on E-LITE through both ends.
        let proxies = crate::dxf_light::proxy_light_entities(&m);
        assert_eq!(proxies.len(), 1, "one polyline for the strip");
        let (kind, layer, tags) = &proxies[0];
        assert_eq!((kind.as_str(), layer.as_str()), ("LWPOLYLINE", "E-LITE"));
        let xs: Vec<f64> = tags.iter().filter(|t| t.code == 10).map(|t| t.f64()).collect();
        let ys: Vec<f64> = tags.iter().filter(|t| t.code == 20).map(|t| t.f64()).collect();
        assert_eq!((xs, ys), (vec![1.0, 3.0], vec![2.0, 2.0]));
        assert!(crate::dxf_light::is_light_proxy(tags));

        // The IFC element is an IfcLightFixture named light-lf0, direction source, with its lumens.
        let text = write_model(&m, &Scene3d::default(), &IfcOptions::default());
        assert_eq!(text.matches("=IFCLIGHTFIXTURE(").count(), 1, "{text}");
        assert!(text.contains(",'light-lf0',$,$,#") && text.contains(",.DIRECTIONSOURCE.);"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Lumens',$,IFCLUMINOUSFLUXMEASURE(1000.),$)"), "{text}");
    }

    #[test]
    fn a_fixture_carries_its_lumens_and_kelvin() {
        let mut m = model();
        m.lights.truncate(1);
        let text = write_model(&m, &Scene3d::default(), &IfcOptions::default());
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Type',$,IFCLABEL('Recessed pot light'),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('Lumens',$,IFCLUMINOUSFLUXMEASURE(800.),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('ColourTemperature',$,IFCTHERMODYNAMICTEMPERATUREMEASURE(3000.),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('On',$,IFCBOOLEAN(.T.),$)"), "{text}");
        assert!(text.contains("IFCPROPERTYSET('"));
        assert!(text.contains("'CADCraft_Light'"));
    }
}
