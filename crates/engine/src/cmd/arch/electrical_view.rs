use serde_json::{Value, json};

use cadcraft_doc::LayerState;

use crate::{
    EngineError, Session,
    cmd::{CommandSpec, bad},
};

/// Show the electrical layer state: keep all E-* layers on, dim every other layer.
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("view.electrical", "Electrical View", run_view_electrical).menu(&["Architecture", "Electrical View"]).params("{}")]
}

fn run_view_electrical(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let obj = params.as_object().ok_or_else(|| bad("view.electrical", "`view.electrical` takes no parameters"))?;
    if !obj.is_empty() {
        return Err(bad("view.electrical", "`view.electrical` takes no parameters"));
    }

    let d = s.doc_mut()?;
    let name = "Electrical";
    let saved = d.layers.clone();
    d.layer_states.retain(|st| st.name != name);
    d.layer_states.push(LayerState { name: name.to_string(), layers: saved, rules: Default::default() });

    let mut shown = Vec::new();
    let mut dimmed = Vec::new();
    for layer in &mut d.layers {
        if layer.name.starts_with("E-") {
            layer.on = true;
            layer.frozen = false;
            layer.transparency = 0;
            shown.push(layer.name.clone());
        } else {
            layer.transparency = 70;
            dimmed.push(layer.name.clone());
        }
    }

    shown.sort();
    dimmed.sort();

    Ok(json!({
        "layerState": name,
        "shown": shown,
        "dimmed": dimmed,
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
        assert_eq!(result["shown"].as_array().unwrap().len(), 2);
        assert_eq!(result["dimmed"].as_array().unwrap().len(), 2);

        let doc = s.doc().unwrap();
        let state = doc.layer_states.iter().find(|st| st.name == "Electrical").unwrap();
        assert_eq!(state.layers.len(), 4);
        let e = doc.layers.iter().find(|l| l.name == "E-POWR").unwrap();
        assert!(e.on);
        assert!(!e.frozen);
        assert_eq!(e.transparency, 0);
        let a = doc.layers.iter().find(|l| l.name == "A-WALL").unwrap();
        assert!(a.on);
        assert!(a.frozen);
        assert_eq!(a.transparency, 70);
    }
}
