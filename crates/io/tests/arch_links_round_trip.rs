use cadcraft_doc::arch::sheet::{LINK_SCHEDULE, LINK_SHEET_VIEW, link_tags, read_link};
use cadcraft_doc::{Common, Drawing, EntityKind, Line, Space, Table};
use cadcraft_geom::Vec3;

#[test]
fn arch_links_survive_a_dxf_round_trip() {
    let mut drawing = Drawing::new_metric();

    // A LINE entity carrying a schedule link in its xdata.
    let line_handle = drawing
        .add(
            &Space::Model,
            Common { xdata: link_tags(LINK_SCHEDULE, "sc0", 0).into_iter().collect(), ..Common::default() },
            EntityKind::Line(Line { a: Vec3::new(0.0, 0.0, 0.0), b: Vec3::new(1.0, 0.0, 0.0) }),
        )
        .unwrap();

    // A TABLE entity carrying a sheet-view link in its xdata.
    let table_handle = drawing
        .add(
            &Space::Model,
            Common { xdata: link_tags(LINK_SHEET_VIEW, "Plans", 2).into_iter().collect(), ..Common::default() },
            EntityKind::Table(Table::default()),
        )
        .unwrap();

    // Write to DXF and read it back.
    let dxf: String = cadcraft_io::write_dxf(&drawing);
    let drawing2 = cadcraft_io::read_dxf(dxf.as_bytes()).unwrap();

    // The LINE's xdata link survives.
    let line = drawing2.entity(line_handle).unwrap();
    assert_eq!(read_link(&line.common.xdata), Some(("schedule".to_string(), "sc0".to_string(), 0)));

    // The TABLE's xdata link survives.
    let table = drawing2.entity(table_handle).unwrap();
    assert_eq!(read_link(&table.common.xdata), Some(("sheet-view".to_string(), "Plans".to_string(), 2)));
}
