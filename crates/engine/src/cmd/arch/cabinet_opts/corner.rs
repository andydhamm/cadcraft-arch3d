//! Cabinet option hook (phase 18, owner p18-corners): parsing and applying this group of
//! `cabinet` / `cabinet.edit` keys: `corner` = "auto" | "none" | "blind" | "diagonal" | "lazySusan".

use crate::cmd::bad;
use crate::{Result, Session};
use cadcraft_doc::arch::{Cabinet, CornerPref};
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    /// The corner treatment asked for; `None` leaves the cabinet's as it is.
    corner: Option<CornerPref>,
}

/// Parse this group's keys of `p`; a `corner` that is not one of the five names is refused.
pub(crate) fn parse(cmd: &str, _s: &Session, p: &Value) -> Result<Opts> {
    let Some(v) = p.get("corner").filter(|v| !v.is_null()) else { return Ok(Opts::default()) };
    let err = || bad(cmd, "`corner` must be \"auto\", \"none\", \"blind\", \"diagonal\" or \"lazySusan\"");
    let pref = v.as_str().and_then(CornerPref::parse).ok_or_else(err)?;
    Ok(Opts { corner: Some(pref) })
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    pub(crate) fn apply(&self, _cmd: &str, c: &mut Cabinet) -> Result<()> {
        if let Some(p) = self.corner {
            c.corner = p;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Wall;
    use cadcraft_geom::Vec2;
    use serde_json::json;

    #[test]
    fn corner_option_round_trips() {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
            d.touch_arch();
        }
        let r = s.execute("cabinet", &json!({ "kind": "base", "at": [1.0, 1.0], "corner": "lazySusan" }));
        assert!(r.is_ok(), "{r:?}");
        let corner = |s: &Session| s.doc().map(|d| d.arch.cabinets.first().map(|c| c.corner)).ok().flatten();
        assert_eq!(corner(&s), Some(CornerPref::LazySusan));
        for (name, want) in [("none", CornerPref::None), ("blind", CornerPref::Blind), ("diagonal", CornerPref::Diagonal), ("auto", CornerPref::Auto)]
        {
            let r = s.execute("cabinet.edit", &json!({ "id": "cb0", "corner": name }));
            assert!(r.is_ok(), "{name}: {r:?}");
            assert_eq!(corner(&s), Some(want));
        }
        // An edit that leaves `corner` out keeps it.
        s.execute("cabinet.edit", &json!({ "id": "cb0", "corner": "blind" })).ok();
        s.execute("cabinet.edit", &json!({ "id": "cb0", "height": 0.9 })).ok();
        assert_eq!(corner(&s), Some(CornerPref::Blind));
        // Anything else is refused and changes nothing.
        for bad in [json!("spiral"), json!("Blind"), json!(3), json!(true), json!([])] {
            let r = s.execute("cabinet.edit", &json!({ "id": "cb0", "corner": bad }));
            assert!(r.is_err(), "{bad}");
            assert!(s.execute("cabinet", &json!({ "kind": "base", "at": [2.0, 1.0], "corner": bad })).is_err());
        }
        assert_eq!(corner(&s), Some(CornerPref::Blind));
        assert_eq!(s.doc().map(|d| d.arch.cabinets.len()).ok(), Some(1));
    }
}
