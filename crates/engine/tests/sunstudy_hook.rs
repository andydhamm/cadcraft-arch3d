//! `sunstudy` passes a failed render back instead of listing a still that was never written. A test
//! binary of its own: the render hook is process-wide and the first one installed stays.

use cadcraft_engine::{RenderHooks, RenderRequest, RenderStats, Session, set_render_hooks};
use serde_json::{Value, json};

fn failing(_: &RenderRequest) -> Result<Value, String> {
    Err("the adapter is gone".to_string())
}

fn stats() -> RenderStats {
    RenderStats::default()
}

#[test]
fn a_still_that_fails_to_render_fails_the_study() {
    set_render_hooks(RenderHooks::new(failing, stats));
    let mut session = Session::new();
    session.new_drawing(true);
    if let Err(error) = session.execute("site", &json!({"latitude": 51.05, "trueNorth": 0})) {
        panic!("site: {error:?}");
    }
    let reply = session.execute("sunstudy", &json!({"date": "2026-06-21", "from": "12:00", "to": "12:00", "step": "1h", "out": "study/house"}));
    match reply {
        Ok(value) => panic!("a study whose only still failed reported success: {value}"),
        Err(error) => assert!(error.to_string().contains("the adapter is gone"), "{error}"),
    }
}
