use cadcraft_doc::arch::sheet::{LINK_SCHEDULE, LINK_SHEET_VIEW, link_tags, read_link};
use cadcraft_doc::{Common, Drawing, Entity, EntityKind, Handle, RawTag};
use craft_dxf::Tag;

#[test]
fn arch_links_survive_a_dxf_round_trip() {
    let mut drawing = Drawing::new_metric();

    // LINE entity with a schedule link
    let line_handle = drawing.push_entity(Entity {
        kind: EntityKind::Line {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            p1: cadcraft_geom::Vec3::new(0.0, 0.0, 0.0),
            p2: cadcraft_geom::Vec3::new(1.0, 0.0, 0.0),
            normal: cadcraft_geom::Vec3::new(0.0, 0.0, 1.0),
            ..EntityKind::Line::default()
        },
        common: Common { xdata: link_tags(LINK_SCHEDULE, "sc0", 0).into_iter().collect(), ..Common::default() },
        ..Entity::default()
    });

    // TABLE entity with a sheet-view link
    let table_handle = drawing.push_entity(Entity {
        kind: EntityKind::Table { ..EntityKind::Table::default() },
        common: Common { xdata: link_tags(LINK_SHEET_VIEW, "Plans", 2).into_iter().collect(), ..Common::default() },
        ..Entity::default()
    });

    // Write to DXF
    let tags: Vec<Tag> = cadcraft_io::dxf::write(&drawing);

    // Read back
    let drawing2 = cadcraft_io::dxf::read(&tags);

    // Verify the LINE's xdata
    let line = drawing2.model.get(line_handle);
    let link = read_link(&line.common.xdata);
    assert_eq!(link, Some(("schedule".to_string(), "sc0".to_string(), 0)));

    // Verify the TABLE's xdata
    let table = drawing2.model.get(table_handle);
    let link = read_link(&table.common.xdata);
    assert_eq!(link, Some(("sheet-view".to_string(), "Plans".to_string(), 2)));
}
