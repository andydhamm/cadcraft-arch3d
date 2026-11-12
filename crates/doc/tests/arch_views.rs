use cadcraft_doc::Drawing;
use cadcraft_doc::arch::*;

#[test]
fn arch_kind_maps_parts_keys_handles_and_labels() {
    assert_eq!(ArchKind::from_part("leaf"), ArchKind::Opening);
    assert_eq!(ArchKind::from_part("frame"), ArchKind::Opening);
    assert_eq!(ArchKind::from_part("glass"), ArchKind::Opening);
    assert_eq!(ArchKind::from_part("floor"), ArchKind::Room);
    assert_eq!(ArchKind::from_part("roof"), ArchKind::Roof);
    assert_eq!(ArchKind::from_part("stair"), ArchKind::Stair);
    assert_eq!(ArchKind::from_part("railing"), ArchKind::Railing);
    assert_eq!(ArchKind::from_part("wall"), ArchKind::Wall);
    assert_eq!(ArchKind::from_part("unknown"), ArchKind::Wall);

    assert_eq!(ArchKind::Wall.key(), "wall");
    assert_eq!(ArchKind::Opening.key(), "opening");
    assert_eq!(ArchKind::View.key(), "view");

    let id = ArchId(1);
    assert_eq!(ArchKind::Wall.handle(id), arch_handle(id));
    assert_eq!(ArchKind::Opening.handle(id), opening_handle(id));
    assert_eq!(ArchKind::View.handle(id), view_handle(id));

    assert_eq!(ArchKind::Wall.label(id), "w1");
    assert_eq!(ArchKind::View.label(id), "v1");
}

#[test]
fn views_default_empty_and_old_models_deserialize() {
    let m = ArchModel::default();
    assert!(m.views.is_empty());
    assert_eq!(m.next_view_id, 0);

    // Simulate an old model JSON without views
    let json = r#"{"walls":[],"openings":[],"rooms":[],"next_id":0,"next_opening_id":0,"next_room_id":0,"levels":[],"types":[],"roofs":[],"next_roof_id":0,"stairs":[],"next_stair_id":0,"railings":[],"next_railing_id":0}"#;
    let back: ArchModel = serde_json::from_str(json).unwrap();
    assert!(back.views.is_empty());
    assert_eq!(back.next_view_id, 0);
}

#[test]
fn push_view_assigns_ids_and_refuses_past_the_cap() {
    let mut m = ArchModel::default();

    let v1 = ArchView { name: "Front".into(), look: [0.0, 1.0], ..ArchView::default() };
    let id1 = m.push_view(v1).unwrap();
    assert_eq!(id1, ArchId(0));
    assert_eq!(m.views.len(), 1);
    assert_eq!(m.next_view_id, 1);

    let v2 = ArchView { name: "Back".into(), look: [0.0, -1.0], ..ArchView::default() };
    let id2 = m.push_view(v2).unwrap();
    assert_eq!(id2, ArchId(1));

    // Fill up to cap
    for i in 2..MAX_VIEWS {
        let v = ArchView { name: format!("View{}", i), look: [1.0, 0.0], ..ArchView::default() };
        m.push_view(v).unwrap();
    }
    assert_eq!(m.views.len(), MAX_VIEWS);

    // One more should fail
    let v_extra = ArchView { name: "Extra".into(), look: [1.0, 0.0], ..ArchView::default() };
    assert_eq!(m.push_view(v_extra), Err(ViewError::TooMany));
    assert_eq!(m.views.len(), MAX_VIEWS);
}

#[test]
fn invalid_views_are_refused_and_dropped_on_read() {
    let mut m = ArchModel::default();

    // Empty name
    let v_bad = ArchView { name: String::new(), look: [0.0, 1.0], ..ArchView::default() };
    assert_eq!(m.push_view(v_bad), Err(ViewError::EmptyName));

    // Bad look length
    let v_bad2 = ArchView { name: "Bad".into(), look: [0.0, 0.0], ..ArchView::default() };
    assert_eq!(m.push_view(v_bad2), Err(ViewError::BadLookLength));

    // Section missing cut
    let v_bad3 = ArchView { name: "Sec".into(), kind: ViewKind::Section, look: [1.0, 0.0], cut: None, ..ArchView::default() };
    assert_eq!(m.push_view(v_bad3), Err(ViewError::MissingCut));

    // Valid view
    let v_good = ArchView { name: "Good".into(), look: [0.0, 1.0], ..ArchView::default() };
    m.push_view(v_good).unwrap();

    // Serialize and deserialize to test read-time validation dropping invalids
    // We manually inject an invalid view into the JSON to simulate a corrupt file
    let mut json = serde_json::to_string(&m).unwrap();
    // Inject an invalid view into the views array
    json = json.replace(
        "\"views\":[",
        "\"views\":[{\"name\":\"\",\"look\":[0,1],\"kind\":\"Elevation\",\"scale\":50,\"insert\":[0,0],\"dimensions\":false,\"layer\":\"A-ELEV\"},",
    );

    let back: ArchModel = serde_json::from_str(&json).unwrap();
    assert_eq!(back.views.len(), 1);
    assert_eq!(back.views[0].name, "Good");

    // Programmatically inserted invalid views are also dropped by validate().
    let mut m2 = ArchModel::default();
    m2.views.push(ArchView { name: "".into(), look: [0.0, 1.0], ..ArchView::default() });
    m2.views.push(ArchView { name: "Good".into(), look: [0.0, 1.0], ..ArchView::default() });

    let msgs = m2.validate();
    assert!(msgs.iter().any(|m| m.contains("invalid definition")));
    assert_eq!(m2.views.len(), 1);
    assert_eq!(m2.views[0].name, "Good");
}

#[test]
fn generated_geometry_is_not_serialized() {
    let mut v = ArchView { name: "Test".into(), look: [0.0, 1.0], ..ArchView::default() };
    v.generated = Some(ViewGeometry {
        revision: 1,
        extent: [0.0, 0.0, 1.0, 1.0],
        lines: vec![ViewLine { a: [0.0, 0.0], b: [1.0, 1.0], kind: ArchKind::Wall, object: ArchId(0), ..ViewLine::default() }],
        ..ViewGeometry::default()
    });

    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("generated"));
    assert!(!json.contains("revision"));

    let back: ArchView = serde_json::from_str(&json).unwrap();
    assert!(back.generated.is_none());
}

#[test]
fn view_handle_is_its_own_range() {
    let id = ArchId(5);
    let h = view_handle(id);
    assert!(is_view_handle(h));
    assert!(is_arch_handle(h));
    assert!(!is_opening_handle(h));
    assert!(!is_room_handle(h));
    assert!(!is_roof_handle(h));
    assert!(!is_stair_handle(h));
    assert!(!is_railing_handle(h));

    // Verify disjointness
    assert_ne!(h, arch_handle(id));
    assert_ne!(h, opening_handle(id));
}

/// A model without views writes the payload it wrote before views existed: `views` is left out
/// when empty and `next_view_id` when zero (io's hostile-record test splices that payload, and a
/// longer one turned its truncated splice into valid JSON). A deleted view's id is still not reused.
#[test]
fn a_model_without_views_writes_no_view_fields() {
    let json = serde_json::to_string(&ArchModel::default()).unwrap();
    assert!(!json.contains("\"views\""), "{json}");
    assert!(!json.contains("next_view_id"), "{json}");

    let mut m = ArchModel::default();
    let front = ArchView { name: "Front".into(), look: [0.0, 1.0], scale: 50.0, layer: "A-ELEV".into(), ..Default::default() };
    let id = m.push_view(front).unwrap();
    assert!(serde_json::to_string(&m).unwrap().contains("\"views\""));
    m.remove_view(id);
    let back: ArchModel = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert!(back.views.is_empty());
    assert_eq!(back.next_view_id, 1);
}

#[test]
fn arch_kind_maps_casework_parts() {
    for part in ["cabinet", "counter", "pull"] {
        assert_eq!(ArchKind::from_part(part), ArchKind::Cabinet, "{part}");
    }
    assert_eq!(ArchKind::from_part("fixture"), ArchKind::Fixture);
    let id = ArchId(3);
    assert_eq!(ArchKind::Cabinet.key(), "cabinet");
    assert_eq!(ArchKind::Fixture.key(), "fixture");
    assert_eq!(ArchKind::Cabinet.handle(id), cabinet_handle(id));
    assert_eq!(ArchKind::Fixture.handle(id), fixture_handle(id));
    assert_eq!(ArchKind::Cabinet.label(id), "cb3");
    assert_eq!(ArchKind::Fixture.label(id), "fx3");
    assert_eq!(ArchKind::from_part("wall"), ArchKind::Wall);
}

#[test]
fn interior_view_validation() {
    let base = ArchView { name: "Kitchen".into(), kind: ViewKind::Interior, look: [0.0, 1.0], ..ArchView::default() };
    assert_eq!(base.validate(), Err(ViewError::MissingRoom));
    let with_room = ArchView { room: Some(ArchId(0)), room_point: Some([1.0, 1.0]), ..base.clone() };
    assert_eq!(with_room.validate(), Err(ViewError::MissingWall));
    let no_point = ArchView { room_point: None, wall: Some(ArchId(0)), ..with_room.clone() };
    assert_eq!(no_point.validate(), Err(ViewError::MissingRoom));
    let nan_point = ArchView { room_point: Some([f64::NAN, 0.0]), wall: Some(ArchId(0)), ..with_room.clone() };
    assert_eq!(nan_point.validate(), Err(ViewError::MissingRoom));
    let ok = ArchView { wall: Some(ArchId(0)), ..with_room.clone() };
    assert_eq!(ok.validate(), Ok(()));
    let cut = ArchView { cut: Some([[0.0, 0.0], [1.0, 0.0]]), ..ok.clone() };
    assert_eq!(cut.validate(), Err(ViewError::UnexpectedCut));
    assert!(!ViewError::MissingRoom.to_string().is_empty());
    assert_eq!(ViewKind::Interior.key(), "interior");
    assert_eq!(ViewKind::Elevation.key(), "elevation");
    assert_eq!(ViewKind::Section.key(), "section");
    // New fields stay out of an old payload.
    let plain = serde_json::to_value(ArchView::default()).unwrap();
    for k in ["room", "room_point", "wall", "level", "notes", "next_note_id"] {
        assert!(plain.get(k).is_none(), "{k}");
    }
}

#[test]
fn view_notes_validate() {
    let note = ViewNote { id: 0, text: "Note".into(), ..ViewNote::default() };
    let view = |n: ViewNote| ArchView { name: "E".into(), notes: vec![n], ..ArchView::default() };
    assert_eq!(view(note.clone()).validate(), Ok(()));
    let bad = |n: ViewNote| matches!(view(n).validate(), Err(ViewError::BadNote(_)));
    assert!(bad(ViewNote { text: String::new(), ..note.clone() }));
    assert!(bad(ViewNote { text: "x".repeat(300), ..note.clone() }));
    assert!(bad(ViewNote { text: "a\u{7}b".into(), ..note.clone() }));
    assert!(bad(ViewNote { at: [f64::NAN, 0.0], ..note.clone() }));
    assert!(bad(ViewNote { b: Some([2e6, 0.0]), ..note.clone() }));
    assert!(bad(ViewNote { offset: [f64::INFINITY, 0.0], ..note.clone() }));
    assert!(bad(ViewNote { height: 0.0, ..note.clone() }));
    assert!(bad(ViewNote { height: 11.0, ..note.clone() }));
    assert!(bad(ViewNote { layer: String::new(), ..note.clone() }));
    let many = ArchView { name: "E".into(), notes: vec![note.clone(); MAX_VIEW_NOTES + 1], ..ArchView::default() };
    assert!(matches!(many.validate(), Err(ViewError::BadNote(_))));
    // ArchModel::validate drops bad notes and keeps the view.
    let mut m = ArchModel::default();
    m.views.push(ArchView { name: "E".into(), notes: vec![note, ViewNote { text: String::new(), ..ViewNote::default() }], ..ArchView::default() });
    let msgs = m.validate();
    assert_eq!(m.views.len(), 1);
    assert_eq!(m.views[0].notes.len(), 1);
    assert_eq!(msgs.iter().filter(|s| s.contains("note")).count(), 1);
}

#[test]
fn room_label_show_numbers_defaults_off() {
    assert!(!RoomLabelShow::default().numbers);
    let old: RoomLabelShow = serde_json::from_str(r#"{"name":true,"area":false,"dimensions":false,"ceiling":false}"#).unwrap();
    assert!(!old.numbers);
    let j = serde_json::to_value(RoomLabelShow::default()).unwrap();
    assert!(j.get("numbers").is_none());
    let on = RoomLabelShow { numbers: true, ..RoomLabelShow::default() };
    let back: RoomLabelShow = serde_json::from_value(serde_json::to_value(on).unwrap()).unwrap();
    assert!(back.numbers);
}
