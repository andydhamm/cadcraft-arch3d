use serde_json::Value;

use crate::Session;

pub(crate) fn merged(_s: &Session, _kind: &str, p: &Value) -> Value {
    p.clone()
}
