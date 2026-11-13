use serde_json::{Value, json};

use cadcraft_doc::LayerState;
use cadcraft_doc::arch::{MAX_PLAN_VIEWS, template_plan_views};

use crate::{
    EngineError, Session,
    cmd::{
        CommandSpec,
        arch::planview::{ensure_plan_layers, open_plan_view},
        bad,
    },
};

/// Open the Electrical plan view (E-* layers on, every other layer dimmed); the layers from
/// before are saved as the layer state "Electrical".
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("view.electrical", "Electrical View", run_view_electrical).menu(&["Architecture", "Electrical View"]).params("{}")]
}

fn run_view_electrical(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let obj = params.as_object().ok_or_else(|| bad("view.electrical", "`view.electrical` takes no parameters"))?;
    if !obj.is_empty() {
        return Err(bad("view.electrical", "`view.electrical` takes no parameters"));
    }

    let name = "Electrical";
    let d = s.doc()?;
    let missing = d.arch.plan_view(name).is_none();
    if missing && d.arch.plan_views.len() >= MAX_PLAN_VIEWS {
        return Err(bad("view.electrical", format!("a drawing holds at most {MAX_PLAN_VIEWS} plan views; delete one first")));
    }
    let active = d.arch.active_plan_view.as_deref().is_some_and(|a| a.trim().eq_ignore_ascii_case(name));
    {
        let d = s.doc_mut()?;
        if missing && let Some(t) = template_plan_views().into_iter().find(|v| v.name.eq_ignore_ascii_case(name)) {
            d.arch.plan_views.push(t);
        }
        let Some(v) = d.arch.plan_view(name).cloned() else {
            return Err(bad("view.electrical", "the Electrical plan view is not available"));
        };
        ensure_plan_layers(d, &v);
        if !active || !d.layer_states.iter().any(|st| st.name == name) {
            // The layers from before the view's rules (plus those just created, at their defaults).
            let saved = d.layers.clone();
            d.layer_states.retain(|st| st.name != name);
            d.layer_states.push(LayerState { name: name.to_string(), layers: saved, ..Default::default() });
        }
    }
    let opened = open_plan_view(s, name).map_err(|e| bad("view.electrical", e.to_string()))?;
    let d = s.doc()?;
    let mut shown: Vec<String> = d.layers.iter().filter(|l| l.on && l.transparency == 0).map(|l| l.name.clone()).collect();
    let mut dimmed: Vec<String> = d.layers.iter().filter(|l| l.transparency > 0).map(|l| l.name.clone()).collect();
    shown.sort();
    dimmed.sort();
    let plan_view = opened.get("planView").cloned().unwrap_or_else(|| json!(name));

    Ok(json!({
        "layerState": name,
        "shown": shown,
        "dimmed": dimmed,
        "planView": plan_view,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::Layer;
    use serde_json::json;

    #[test]
    fn electrical_view_dims_everything_but_e_layers() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("A-WALL"));
            d.layers.push(Layer::new("E-POWR"));
            d.layers.push(Layer::new("E-LITE"));
            d.layers.push(Layer::new("A-DOOR"));
            d.layers[0].on = true;
            d.layers[0].frozen = true;
            d.layers[1].on = false;
            d.layers[1].frozen = false;
            d.layers[2].on = true;
            d.layers[2].frozen = false;
            d.layers[3].on = false;
            d.layers[3].frozen = true;
        }

        let result = s.execute("view.electrical", &json!({})).unwrap();
        assert_eq!(result["layerState"], "Electrical");
        assert_eq!(result["shown"].as_array().unwrap().len(), 3);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 2);

        let doc = s.doc().unwrap();
        let state = doc.layer_states.iter().find(|st| st.name == "Electrical").unwrap();
        assert_eq!(state.layers.len(), 5);
        let e = doc.layers.iter().find(|l| l.name == "E-POWR").unwrap();
        assert!(e.on);
        assert!(!e.frozen);
        assert_eq!(e.transparency, 0);
        let a = doc.layers.iter().find(|l| l.name == "A-WALL").unwrap();
        assert!(a.on);
        assert!(a.frozen);
        assert_eq!(a.transparency, 70);
    }

    fn rooms_session() -> Session {
        let mut s = Session::new();
        for pts in [[[0.0, 0.0], [6.0, 0.0]], [[6.0, 0.0], [6.0, 4.0]], [[6.0, 4.0], [0.0, 4.0]], [[0.0, 4.0], [0.0, 0.0]]] {
            s.execute("wall", &json!({"points": pts, "thickness": 0.2})).unwrap();
        }
        s.execute("roomdetect", &json!({})).unwrap();
        s
    }

    #[test]
    fn view_electrical_saves_the_layers_from_before_the_view() {
        let mut s = rooms_session();
        let before = s.doc().unwrap().layers.clone();
        assert!(before.iter().all(|l| l.name != "A-ROOM"));
        s.execute("view.electrical", &json!({})).unwrap();
        s.execute("layerstate.restore", &json!({"name": "Electrical"})).unwrap();
        let after = &s.doc().unwrap().layers;
        let room = after.iter().find(|l| l.name == "A-ROOM").expect("A-ROOM");
        assert_eq!(room.transparency, 0);
        for l in &before {
            let now = after.iter().find(|x| x.name == l.name).unwrap();
            assert_eq!((now.on, now.frozen, now.transparency), (l.on, l.frozen, l.transparency), "{}", l.name);
        }
    }

    #[test]
    fn view_electrical_twice_keeps_the_saved_state() {
        let mut s = rooms_session();
        s.execute("view.electrical", &json!({})).unwrap();
        let first = s.doc().unwrap().layer_states.iter().find(|st| st.name == "Electrical").unwrap().layers.clone();
        s.execute("view.electrical", &json!({})).unwrap();
        let second = &s.doc().unwrap().layer_states.iter().find(|st| st.name == "Electrical").unwrap().layers;
        assert_eq!(first.len(), second.len());
        assert!(second.iter().all(|l| l.transparency == 0));
    }

    #[test]
    fn view_electrical_opens_the_electrical_plan_view() {
        let mut s = rooms_session();
        let r = s.execute("view.electrical", &json!({})).unwrap();
        assert_eq!(r["planView"], "Electrical");
        let d = s.doc().unwrap();
        assert_eq!(d.arch.active_plan_view.as_deref(), Some("Electrical"));
        assert!(d.arch.open_plan_views.iter().any(|n| n == "Electrical"));
    }

    #[test]
    fn view_electrical_re_adds_a_deleted_template() {
        let mut s = rooms_session();
        s.execute("planview.delete", &json!({"name": "Electrical"})).unwrap();
        assert!(s.doc().unwrap().arch.plan_view("Electrical").is_none());
        s.execute("view.electrical", &json!({})).unwrap();
        assert!(s.doc().unwrap().arch.plan_view("Electrical").is_some());
    }

    #[test]
    fn non_empty_params_still_refused() {
        let mut s = rooms_session();
        let depth = s.state().unwrap().undo.len();
        assert!(s.execute("view.electrical", &json!({"x": 1})).is_err());
        assert!(s.execute("view.electrical", &json!([])).is_err());
        assert_eq!(s.state().unwrap().undo.len(), depth);
    }

    #[test]
    fn view_electrical_refused_at_max_plan_views() {
        let mut s = rooms_session();
        s.execute("planview.delete", &json!({"name": "Electrical"})).unwrap();
        let d = s.doc_mut().unwrap();
        let mut i = 0;
        while d.arch.plan_views.len() < MAX_PLAN_VIEWS {
            let name = format!("Test{}", i);
            d.arch.plan_views.push(cadcraft_doc::arch::PlanView { name, ..Default::default() });
            i += 1;
        }
        assert!(s.doc().unwrap().arch.plan_view("Electrical").is_none());
        let e = s.execute("view.electrical", &json!({})).unwrap_err().to_string();
        assert!(e.contains(&MAX_PLAN_VIEWS.to_string()), "{e}");
    }

    #[test]
    fn view_electrical_case_insensitive_active_view() {
        let mut s = rooms_session();
        let d = s.doc_mut().unwrap();
        d.arch.active_plan_view = Some("electrical".to_string());
        s.execute("view.electrical", &json!({})).unwrap();
        let state = s.doc().unwrap().layer_states.iter().find(|st| st.name == "Electrical").unwrap();
        assert!(state.layers.iter().all(|l| l.transparency == 0), "saved state must not be dimmed");
    }

    #[test]
    fn view_electrical_hostile_json_types() {
        let mut s = rooms_session();
        let depth = s.state().unwrap().undo.len();
        for p in [json!(null), json!(1), json!("string"), json!(true)] {
            assert!(s.execute("view.electrical", &p).is_err(), "{p}");
        }
        assert_eq!(s.state().unwrap().undo.len(), depth);
    }

    #[test]
    fn view_electrical_undo_restores_layers() {
        let mut s = rooms_session();
        let before = s.doc().unwrap().layers.clone();
        s.execute("view.electrical", &json!({})).unwrap();
        let wall = s.doc().unwrap().layer("A-WALL").unwrap();
        assert_eq!(wall.transparency, 70);
        s.execute("undo", &json!({})).unwrap();
        let after = &s.doc().unwrap().layers;
        for l in &before {
            let now = after.iter().find(|x| x.name == l.name).unwrap();
            assert_eq!((now.on, now.frozen, now.transparency), (l.on, l.frozen, l.transparency), "{}", l.name);
        }
    }

    #[test]
    fn electrical_view_with_no_layers() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        assert_eq!(result["shown"].as_array().unwrap().len(), 0);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn electrical_view_all_e_layers() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("E-POWR"));
            d.layers.push(Layer::new("E-LITE"));
            d.layers.push(Layer::new("E-DATA"));
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        assert_eq!(result["shown"].as_array().unwrap().len(), 3);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn electrical_view_no_e_layers() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("A-WALL"));
            d.layers.push(Layer::new("A-DOOR"));
            d.layers.push(Layer::new("A-WINDOW"));
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        assert_eq!(result["shown"].as_array().unwrap().len(), 0);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn electrical_view_case_sensitive_prefix() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("E-POWR"));
            d.layers.push(Layer::new("e-powr"));
            d.layers.push(Layer::new("EL"));
            d.layers.push(Layer::new("EPOWR"));
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        // Only "E-POWR" matches (case-sensitive, requires dash)
        assert_eq!(result["shown"].as_array().unwrap().len(), 1);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn electrical_view_output_is_sorted() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("E-ZONE"));
            d.layers.push(Layer::new("E-ALPHA"));
            d.layers.push(Layer::new("A-MIDDLE"));
            d.layers.push(Layer::new("A-FIRST"));
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        let shown: Vec<String> = result["shown"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let dimmed: Vec<String> = result["dimmed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let mut shown_sorted = shown.clone();
        shown_sorted.sort();
        let mut dimmed_sorted = dimmed.clone();
        dimmed_sorted.sort();
        assert_eq!(shown, shown_sorted);
        assert_eq!(dimmed, dimmed_sorted);
    }

    #[test]
    fn electrical_view_shows_exactly_e_layers() {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.layers.clear();
            d.layers.push(Layer::new("E-POWR"));
            d.layers.push(Layer::new("A-WALL"));
            d.layers.push(Layer::new("E-LITE"));
            d.layers.push(Layer::new("A-DOOR"));
        }
        let result = s.execute("view.electrical", &json!({})).unwrap();
        let shown: Vec<String> = result["shown"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let dimmed: Vec<String> = result["dimmed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();

        // All shown layers should start with "E-"
        for name in &shown {
            assert!(name.starts_with("E-"), "{} should start with E-", name);
        }
        // All dimmed layers should not start with "E-"
        for name in &dimmed {
            assert!(!name.starts_with("E-"), "{} should not start with E-", name);
        }
    }
}
