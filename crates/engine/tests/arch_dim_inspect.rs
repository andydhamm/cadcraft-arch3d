use serde_json::{Value, json};

use cadcraft_engine::Session;

fn house(s: &mut Session) {
    assert!(
        s.execute(
            "wall",
            &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2, "height": 2.4, "exterior": true})
        )
        .is_ok()
    );
    assert!(s.execute("wall", &json!({"points": [[0.0, 3.0], [10.0, 3.0]], "thickness": 0.1, "height": 2.4, "type": "interior"})).is_ok());
    assert!(s.execute("wall", &json!({"points": [[6.0, 3.0], [6.0, 8.0]], "thickness": 0.1, "height": 2.4, "type": "interior"})).is_ok());
    assert!(s.execute("roomdetect", &json!({})).is_ok());
}

fn add_window(s: &mut Session) {
    assert!(s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 0.9, "height": 1.2, "sill": 0.9})).is_ok());
}

fn inspect_dimensions(s: &mut Session) -> Value {
    s.execute("arch.inspect", &json!({"dimensions": true, "openings": true, "cabinets": true})).unwrap()
}

fn dimension_by_handle<'a>(reply: &'a Value, handle: &str) -> &'a Value {
    reply["dimensions"].as_array().unwrap().iter().find(|row| row["handle"].as_str() == Some(handle)).unwrap()
}

fn dimension_handle(reply: &Value) -> String {
    reply["handle"].as_str().unwrap().to_string()
}

#[test]
fn inspect_dimensions_rows_have_handle_points_and_attachments() {
    let mut s = Session::new();
    house(&mut s);
    add_window(&mut s);

    let handle = dimension_handle(&s.execute("dimlinear", &json!({"p1": [-0.1, 8.0], "p2": [4.0, 8.0], "at": [2.0, 9.5]})).unwrap());
    let reply = inspect_dimensions(&mut s);
    let row = dimension_by_handle(&reply, &handle);

    assert_eq!(row["kind"], "linear");
    assert_eq!(row["p1"], json!([-0.1, 8.0]));
    assert_eq!(row["p2"], json!([4.0, 8.0]));
    assert_eq!(row["at"], json!([2.0, 9.5]));
    assert_eq!(row["measurement"], 4.1);
    let attached = row["attached"].as_array().unwrap();
    assert_eq!(attached.len(), 2);
    assert_eq!(attached[0]["point"], "p13");
    assert_eq!(attached[0]["object"], "w3");
    assert_eq!(attached[0]["feature"], "rightFace");
    assert_eq!(attached[1]["point"], "p14");
    assert_eq!(attached[1]["object"], "o0");
    assert_eq!(attached[1]["feature"], "centre");
}

#[test]
fn window_locating_dimension_follows_the_window_4_1_to_5_1() {
    let mut s = Session::new();
    house(&mut s);
    add_window(&mut s);
    let h = dimension_handle(&s.execute("dimlinear", &json!({"p1": [-0.1, 8.0], "p2": [4.0, 8.0], "at": [2.0, 9.5]})).unwrap());

    let before = inspect_dimensions(&mut s);
    let before_row = dimension_by_handle(&before, &h);
    assert_eq!(before_row["measurement"], 4.1);
    assert_eq!(before_row["p2"], json!([4.0, 8.0]));

    assert!(s.execute("arch.opening.move", &json!({"opening": "o0", "offset": 5.0})).is_ok());
    let after = inspect_dimensions(&mut s);
    let after_row = dimension_by_handle(&after, &h);
    assert!((after_row["measurement"].as_f64().unwrap() - 5.1).abs() < 1e-6);
    assert_eq!(after_row["p2"], json!([5.0, 8.0]));
}

#[test]
fn wall_face_dimension_follows_w5_6_05_to_5_55() {
    let mut s = Session::new();
    house(&mut s);
    let h = dimension_handle(&s.execute("dimlinear", &json!({"p1": [-0.1, 5.5], "p2": [5.95, 5.5], "at": [3.0, 6.5]})).unwrap());

    let before = inspect_dimensions(&mut s);
    let before_row = dimension_by_handle(&before, &h);
    assert!((before_row["measurement"].as_f64().unwrap() - 6.05).abs() < 1e-6);

    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w5", "end": "start", "to": [5.5, 3.0]})).is_ok());
    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w5", "end": "end", "to": [5.5, 8.0]})).is_ok());
    let after = inspect_dimensions(&mut s);
    let after_row = dimension_by_handle(&after, &h);
    assert!((after_row["measurement"].as_f64().unwrap() - 5.55).abs() < 1e-6);
    assert!(after_row["attached"].as_array().unwrap().iter().any(|a| a["object"] == "w5" && a["feature"] == "leftFace"));
}

#[test]
fn centre_line_dimension_follows_w4_3_1_to_3_6() {
    let mut s = Session::new();
    house(&mut s);
    let h = dimension_handle(&s.execute("dimlinear", &json!({"p1": [5.0, -0.1], "p2": [5.0, 3.0], "at": [11.0, 1.5], "rotation": 90})).unwrap());

    let before = inspect_dimensions(&mut s);
    let before_row = dimension_by_handle(&before, &h);
    assert!((before_row["measurement"].as_f64().unwrap() - 3.1).abs() < 1e-6);

    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w4", "end": "start", "to": [0.0, 3.5]})).is_ok());
    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w4", "end": "end", "to": [10.0, 3.5]})).is_ok());
    let after = inspect_dimensions(&mut s);
    let after_row = dimension_by_handle(&after, &h);
    assert!((after_row["measurement"].as_f64().unwrap() - 3.6).abs() < 1e-6);
    assert!(after_row["attached"].as_array().unwrap().iter().any(|a| a["object"] == "w4" && a["feature"] == "centre"));
}

#[test]
fn cabinet_edge_dimension_follows_the_cabinet() {
    let mut s = Session::new();
    house(&mut s);
    assert!(s.execute("cabinet", &json!({"kind": "base", "wall": "w2", "offset": 8.0})).is_ok());
    let inspect = s.execute("arch.inspect", &json!({"cabinets": true, "dimensions": true})).unwrap();
    let cabinet = inspect["cabinets"].as_array().unwrap().first().unwrap();
    let cx = cabinet["at"][0].as_f64().unwrap();
    let width = cabinet["width"].as_f64().unwrap();
    let xw = cx - width / 2.0;
    let dim = s.execute("dimlinear", &json!({"p1": [0.1, 7.6], "p2": [xw, 7.6], "at": [xw / 2.0, 6.5], "rotation": 0})).unwrap();
    let handle = dimension_handle(&dim);

    let before = s.execute("arch.inspect", &json!({"dimensions": true, "cabinets": true})).unwrap();
    let before_row = dimension_by_handle(&before, &handle);
    assert!((before_row["measurement"].as_f64().unwrap() - (xw - 0.1)).abs() < 1e-6);
    assert!(before_row["attached"].as_array().unwrap().iter().any(|a| a["object"] == "w3" && a["feature"] == "leftFace"));
    assert!(before_row["attached"].as_array().unwrap().iter().any(|a| a["object"] == cabinet["id"].as_str().unwrap() && a["feature"] == "edge"));

    let cabinet_id = cabinet["id"].as_str().unwrap().to_string();
    assert!(s.execute("cabinet.edit", &json!({"id": cabinet_id, "wall": "w2", "offset": 7.5})).is_ok());
    let after = s.execute("arch.inspect", &json!({"dimensions": true, "cabinets": true})).unwrap();
    let after_row = dimension_by_handle(&after, &handle);
    assert!((after_row["measurement"].as_f64().unwrap() - (xw - 0.1 + 0.5)).abs() < 1e-6);
}

#[test]
fn free_dimension_has_no_attachments_and_stays() {
    let mut s = Session::new();
    house(&mut s);
    add_window(&mut s);
    let handle = dimension_handle(&s.execute("dimlinear", &json!({"p1": [20.0, 20.0], "p2": [23.0, 20.0], "at": [21.5, 21.0]})).unwrap());

    let before = inspect_dimensions(&mut s);
    let before_row = dimension_by_handle(&before, &handle);
    assert!(before_row["attached"].as_array().unwrap().is_empty());
    assert!((before_row["measurement"].as_f64().unwrap() - 3.0).abs() < 1e-6);

    assert!(s.execute("arch.opening.move", &json!({"opening": "o0", "offset": 5.0})).is_ok());
    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w5", "end": "start", "to": [5.5, 3.0]})).is_ok());
    let after = inspect_dimensions(&mut s);
    let after_row = dimension_by_handle(&after, &handle);
    assert!(after_row["attached"].as_array().unwrap().is_empty());
    assert!((after_row["measurement"].as_f64().unwrap() - 3.0).abs() < 1e-6);
}

#[test]
fn auto_dims_values_are_measured_live() {
    let mut s = Session::new();
    house(&mut s);
    assert!(s.execute("dimauto", &json!({"level": 0})).is_ok());
    let before = s.execute("arch.inspect", &json!({"autoDims": true, "dimensions": true})).unwrap();
    let north = before["autoDims"].as_array().unwrap().iter().find(|row| row["side"] == "north" && row["row"] == "walls").unwrap();
    let handles = north["handles"].as_array().unwrap();
    assert_eq!(handles.len(), 2);
    let values = north["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);

    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w5", "end": "start", "to": [5.5, 3.0]})).is_ok());
    assert!(s.execute("arch.wall.stretch", &json!({"wall": "w5", "end": "end", "to": [5.5, 8.0]})).is_ok());

    let after = s.execute("arch.inspect", &json!({"autoDims": true, "dimensions": true})).unwrap();
    let north_after = after["autoDims"].as_array().unwrap().iter().find(|row| row["side"] == "north" && row["row"] == "walls").unwrap();
    let values_after = north_after["values"].as_array().unwrap();
    assert!(values_after.len() >= 2);
    let dims = after["dimensions"].as_array().unwrap();
    for (handle, value) in handles.iter().zip(values_after.iter()) {
        let h = handle.as_str().unwrap();
        let live = dims.iter().find(|row| row["handle"].as_str() == Some(h)).unwrap()["measurement"].as_f64().unwrap();
        assert!((value.as_f64().unwrap() - live).abs() < 1e-6, "autoDims value must match live measurement");
    }
}

#[test]
fn inspect_dimensions_is_read_only() {
    let mut s = Session::new();
    house(&mut s);
    let before = s.doc().unwrap().arch_revision;
    let before_rooms = s.doc().unwrap().arch.rooms.len();
    let reply = s.execute("arch.inspect", &json!({"dimensions": true})).unwrap();
    assert!(reply.get("dimensions").is_some());
    assert_eq!(s.doc().unwrap().arch_revision, before);
    assert_eq!(s.doc().unwrap().arch.rooms.len(), before_rooms);
}
