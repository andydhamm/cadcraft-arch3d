//! Cabinet option hook (phase 18, owner p18-backsplash): parsing and applying this group of
//! `cabinet` / `cabinet.edit` keys: `backsplash` (bool), whether a backsplash stands behind the
//! cabinet. Backsplashes are recomputed from the model, so the edit needs nothing else.

use crate::cmd::bad;
use crate::{Result, Session};
use cadcraft_doc::arch::Cabinet;
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    /// `backsplash`: `Some(false)` suppresses the backsplash behind the cabinet.
    backsplash: Option<bool>,
}

/// Parse this group's keys of `p`; anything but a boolean `backsplash` is refused.
pub(crate) fn parse(cmd: &str, _s: &Session, p: &Value) -> Result<Opts> {
    match p.get("backsplash") {
        None => Ok(Opts::default()),
        Some(Value::Bool(b)) => Ok(Opts { backsplash: Some(*b) }),
        Some(_) => Err(bad(cmd, "`backsplash` must be true or false")),
    }
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    pub(crate) fn apply(&self, _cmd: &str, c: &mut Cabinet) -> Result<()> {
        if let Some(on) = self.backsplash {
            c.backsplash_off = !on;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::arch::Wall;
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn session() -> Session {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
            d.touch_arch();
        }
        s
    }

    fn off(s: &Session, i: usize) -> Option<bool> {
        s.doc().ok()?.arch.cabinets.get(i).map(|c| c.backsplash_off)
    }

    fn row_flag(s: &mut Session, i: usize) -> Option<bool> {
        let v = s.execute("arch.inspect", &json!({ "cabinets": true })).ok()?;
        v["cabinets"][i]["backsplash"].as_bool()
    }

    #[test]
    fn cabinet_backsplash_option_round_trips() {
        let mut s = session();
        s.execute("cabinet", &json!({ "kind": "base", "at": [1.0, 1.0], "widths": [0.6, 0.6] })).unwrap();
        assert_eq!((off(&s, 0), off(&s, 1)), (Some(false), Some(false)));
        assert_eq!(row_flag(&mut s, 0), Some(true));
        // Off on edit; the other cabinet and absent keys are untouched.
        s.execute("cabinet.edit", &json!({ "id": "cb0", "backsplash": false })).unwrap();
        assert_eq!((off(&s, 0), off(&s, 1)), (Some(true), Some(false)));
        assert_eq!((row_flag(&mut s, 0), row_flag(&mut s, 1)), (Some(false), Some(true)));
        s.execute("cabinet.edit", &json!({ "id": "cb0", "height": 0.9 })).unwrap();
        assert_eq!(off(&s, 0), Some(true));
        // And back on.
        s.execute("cabinet.edit", &json!({ "id": "cb0", "backsplash": true })).unwrap();
        assert_eq!(off(&s, 0), Some(false));
        // Off at creation.
        s.execute("cabinet", &json!({ "kind": "base", "at": [3.0, 1.0], "backsplash": false })).unwrap();
        assert_eq!(off(&s, 2), Some(true));
        // Anything else is refused and changes nothing.
        let before = s.doc().unwrap().arch.clone();
        for v in [json!("no"), json!(0), json!(null), json!([true])] {
            assert!(s.execute("cabinet.edit", &json!({ "id": "cb0", "backsplash": v })).is_err());
            assert!(s.execute("cabinet", &json!({ "kind": "base", "at": [3.5, 2.0], "backsplash": v })).is_err());
        }
        assert_eq!(s.doc().unwrap().arch, before);
        // The model without the flag writes the same payload as before.
        let json = serde_json::to_string(&before.cabinets[1]).unwrap();
        assert!(!json.contains("backsplash_off"));
    }
}
