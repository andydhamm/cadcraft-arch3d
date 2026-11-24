//! `denoiser.info` (phase 22): the learned denoiser's loaded weights and whether it can run on
//! this session's device. The engine has no GPU, so the answer comes from the render hooks the
//! app installs (`RenderHooks::denoiser_info`); without a renderer the reply says so.

use serde_json::{Value, json};

use crate::cmd::CommandSpec;
use crate::{EngineError, Session};

use super::render::render_hooks;

/// The `denoiser.info` command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("denoiser.info", "Denoiser Info", run).params("{}").noundo()]
}

fn run(_session: &mut Session, _params: &Value) -> Result<Value, EngineError> {
    let hook = render_hooks().and_then(|h| h.denoiser_info);
    Ok(reply(hook))
}

/// The command's reply for an optional hook: the hook's value with `ok: true`, or the "no
/// renderer" answer. A failing hook reports itself as unavailable with its message.
fn reply(hook: Option<fn() -> Result<Value, String>>) -> Value {
    let Some(f) = hook else {
        return json!({ "ok": true, "available": false, "reason": "no renderer is installed", "models": [] });
    };
    match f() {
        Ok(Value::Object(mut m)) => {
            m.insert("ok".into(), Value::Bool(true));
            Value::Object(m)
        }
        Ok(other) => json!({ "ok": true, "info": other }),
        Err(e) => json!({ "ok": true, "available": false, "reason": e, "models": [] }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denoiser_info_without_a_hook_says_unavailable() {
        let v = reply(None);
        assert_eq!(v["ok"], true);
        assert_eq!(v["available"], false);
        assert_eq!(v["reason"], "no renderer is installed");
        assert_eq!(v["models"], json!([]));
    }

    fn hook() -> Result<Value, String> {
        Ok(json!({ "models": [{ "name": "base" }], "available": true, "reason": "" }))
    }

    fn failing() -> Result<Value, String> {
        Err("boom".into())
    }

    #[test]
    fn denoiser_info_returns_the_hooks_models() {
        let v = reply(Some(hook));
        assert_eq!(v["ok"], true);
        assert_eq!(v["available"], true);
        assert_eq!(v["models"][0]["name"], "base");
        let v = reply(Some(failing));
        assert_eq!((v["ok"].clone(), v["available"].clone(), v["reason"].clone()), (json!(true), json!(false), json!("boom")));
    }

    #[test]
    fn denoiser_info_is_registered_and_noundo() {
        let spec = crate::cmd::find_command("denoiser.info");
        assert!(spec.is_some_and(|s| !s.undoable && s.label == "Denoiser Info"));
    }
}
