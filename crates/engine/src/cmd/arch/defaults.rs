//! Set as Default (p14-cmd-defaults fills this in): the parameters a command uses when a call
//! leaves them out, kept in the drawing's `defaults` by kind ("cabinet.base", "door",
//! "furnishing.<item id>", ...).

use serde_json::Value;

use crate::Session;

/// `p` with the drawing's defaults for `kind` filled in where `p` leaves them out.
pub(crate) fn merged(_s: &Session, _kind: &str, p: &Value) -> Value {
    p.clone()
}
