//! Area lights and IES fixtures through the DXF arch record (phase 11): the LED panel's size and
//! the pot light's whole photometric profile survive a save and a reopen.

use cadcraft_arch::light::{find_type, fixture_from_type};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{ArchId, FixtureShape, IesProfile, LightKind};
use cadcraft_geom::Vec2;
use cadcraft_io::{read_dxf, write_dxf};

/// A 3 x 4 profile with values that are not round decimals, so a lossy float format shows.
fn profile() -> IesProfile {
    let vertical = vec![0.0, 22.5, 45.0, 90.0];
    let horizontal = vec![0.0, 90.0, 180.0];
    let candela = (0..horizontal.len() * vertical.len()).map(|i| 1234.567_891_234_5 / (1.0 + i as f64 * 0.37) + 0.1 + 0.2).collect();
    IesProfile { name: "Downlight \"A\" 3000K".into(), vertical, horizontal, candela, lumens: 1520.5, ..Default::default() }
}

#[test]
fn arch_record_round_trips_area_and_ies() {
    let mut d = Drawing::default();

    let panel_type = find_type("LED panel 600").expect("the catalogue's LED panel");
    let mut panel = fixture_from_type(ArchId(0), &panel_type, Vec2::new(2.0, 1.5));
    panel.width = 0.6;
    panel.length = 0.45;
    panel.rotation = 30.0;
    assert_eq!((panel.shape, panel.kind), (FixtureShape::Panel, LightKind::Area));

    let pot_type = find_type("Recessed pot light").expect("the catalogue's pot light");
    let mut pot = fixture_from_type(ArchId(1), &pot_type, Vec2::new(4.0, 1.5));
    pot.ies = Some(profile());

    // A plain pot light beside them: no IES, no size, and neither is written for it.
    let plain = fixture_from_type(ArchId(2), &pot_type, Vec2::new(6.0, 1.5));

    d.arch.lights = vec![panel.clone(), pot.clone(), plain.clone()];

    let text = write_dxf(&d);
    let back = read_dxf(text.as_bytes()).expect("reopen");
    assert_eq!(back.arch.lights.len(), 3);
    assert_eq!(back.arch.lights, d.arch.lights, "the lights come back equal");

    let panel_back = &back.arch.lights[0];
    assert_eq!((panel_back.width, panel_back.length), (0.6, 0.45));
    assert_eq!((panel_back.shape, panel_back.kind), (FixtureShape::Panel, LightKind::Area));

    assert_eq!(back.arch.lights[1].ies.as_ref(), Some(&profile()), "the whole IesProfile");
    assert!(back.arch.lights[2].ies.is_none());
    assert_eq!((back.arch.lights[2].width, back.arch.lights[2].length), (0.0, 0.0));

    // Saving what was read gives the same record.
    assert_eq!(read_dxf(write_dxf(&back).as_bytes()).expect("reopen again").arch.lights, d.arch.lights);
}

#[test]
fn round_trip_non_square_panel() {
    let mut d = Drawing::default();

    let panel_type = find_type("LED panel 600").expect("the catalogue's LED panel");
    let mut panel = fixture_from_type(ArchId(0), &panel_type, Vec2::new(2.0, 1.5));
    panel.width = 1.2;
    panel.length = 0.3;
    d.arch.lights = vec![panel.clone()];

    let text = write_dxf(&d);
    let back = read_dxf(text.as_bytes()).expect("reopen");
    assert_eq!(back.arch.lights.len(), 1);
    assert_eq!(back.arch.lights[0].width, 1.2);
    assert_eq!(back.arch.lights[0].length, 0.3);
}

#[test]
fn round_trip_ies_with_many_values() {
    let mut d = Drawing::default();

    let pot_type = find_type("Recessed pot light").expect("the catalogue's pot light");
    let mut pot = fixture_from_type(ArchId(0), &pot_type, Vec2::new(2.0, 1.5));

    // Build a larger IES profile: 10 vertical angles, 8 horizontal angles = 80 candela values
    let vertical: Vec<f64> = (0..10).map(|i| i as f64 * 18.0).collect();
    let horizontal: Vec<f64> = (0..8).map(|i| i as f64 * 45.0).collect();
    let candela: Vec<f64> = (0..80).map(|i| 1000.0 / (1.0 + i as f64 * 0.1)).collect();
    pot.ies = Some(IesProfile { name: "Large profile".into(), vertical, horizontal, candela, lumens: 1500.0, ..Default::default() });

    d.arch.lights = vec![pot.clone()];

    let text = write_dxf(&d);
    let back = read_dxf(text.as_bytes()).expect("reopen");
    assert_eq!(back.arch.lights.len(), 1);
    let ies = back.arch.lights[0].ies.as_ref().expect("IES should round-trip");
    assert_eq!(ies.name, "Large profile");
    assert_eq!(ies.vertical.len(), 10);
    assert_eq!(ies.horizontal.len(), 8);
    assert_eq!(ies.candela.len(), 80);
    assert_eq!(ies.candela, pot.ies.as_ref().unwrap().candela);
}
