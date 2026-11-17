//! Cabinet option hook (phase 18, owner p18-cabinet-fronts): the `construction` and `mount` keys of
//! `cabinet` / `cabinet.edit`.

use crate::cmd::bad;
use crate::{Result, Session};
use cadcraft_arch::casework_face::row_heights;
use cadcraft_doc::arch::{Cabinet, Construction, FrontMount};
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    construction: Option<Construction>,
    mount: Option<FrontMount>,
}

/// Parse this group's keys of `p`; anything but a known name is refused.
pub(crate) fn parse(cmd: &str, _s: &Session, p: &Value) -> Result<Opts> {
    let construction = match p.get("construction") {
        None => None,
        Some(v) => Some(v.as_str().and_then(Construction::parse).ok_or_else(|| bad(cmd, "`construction` must be \"frameless\" or \"framed\""))?),
    };
    let mount = match p.get("mount") {
        None => None,
        Some(v) => Some(v.as_str().and_then(FrontMount::parse).ok_or_else(|| bad(cmd, "`mount` must be \"overlay\" or \"inset\""))?),
    };
    Ok(Opts { construction, mount })
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    pub(crate) fn apply(&self, cmd: &str, c: &mut Cabinet) -> Result<()> {
        if self.construction.is_none() && self.mount.is_none() {
            return Ok(());
        }
        if let Some(v) = self.construction {
            c.construction = v;
        }
        if let Some(v) = self.mount {
            c.mount = v;
        }
        // An explicit face layout must still fit the (smaller, for inset) face.
        if c.face.as_ref().is_some_and(|f| row_heights(c, f).is_none()) {
            return Err(bad(cmd, "the cabinet's face layout does not fit with this construction and mount"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn construction_and_mount_options() {
        let mut s = Session::new();
        let r = s.execute("cabinet", &json!({"kind": "base", "at": [1.0, 1.0], "construction": "framed", "mount": "inset"})).unwrap();
        let id = r["cabinets"][0].as_str().unwrap().to_string();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert_eq!(v["cabinets"][0]["construction"], "framed");
        assert_eq!(v["cabinets"][0]["mount"], "inset");
        s.execute("cabinet.edit", &json!({"id": id, "construction": "frameless", "mount": "overlay"})).unwrap();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert_eq!(v["cabinets"][0]["construction"], "frameless");
        assert_eq!(v["cabinets"][0]["mount"], "overlay");
        for bad in [json!({"construction": "Framed"}), json!({"mount": 3}), json!({"mount": null}), json!({"construction": "x"})] {
            let mut p = bad.clone();
            p["id"] = json!(id);
            assert!(s.execute("cabinet.edit", &p).is_err(), "{bad}");
        }
        assert!(s.execute("cabinet", &json!({"kind": "base", "at": [3.0, 1.0], "mount": "flush"})).is_err());
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert_eq!(v["cabinets"].as_array().unwrap().len(), 1);
        assert_eq!(v["cabinets"][0]["construction"], "frameless");
    }

    #[test]
    fn construction_and_mount_set_together() {
        let mut s = Session::new();
        let r = s.execute("cabinet", &json!({"kind": "wall", "at": [1.0, 1.0], "construction": "framed", "mount": "overlay"})).unwrap();
        let id = r["cabinets"][0].as_str().unwrap().to_string();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert_eq!(v["cabinets"][0]["construction"], "framed");
        assert_eq!(v["cabinets"][0]["mount"], "overlay");
        // Change both at once via edit.
        s.execute("cabinet.edit", &json!({"id": id, "construction": "frameless", "mount": "inset"})).unwrap();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert_eq!(v["cabinets"][0]["construction"], "frameless");
        assert_eq!(v["cabinets"][0]["mount"], "inset");
    }

    #[test]
    fn inset_mount_rejects_face_that_only_fits_overlay() {
        let mut s = Session::new();
        // Overlay base cabinet face height: 0.876 - 0.1 (toe) = 0.776.
        let r = s.execute("cabinet", &json!({"kind": "base", "at": [1.0, 1.0], "mount": "overlay"})).unwrap();
        let id = r["cabinets"][0].as_str().unwrap().to_string();
        // One fixed row filling the full overlay face height.
        s.execute("cabinet.face", &json!({"id": id, "rows": [{"kind": "door", "height": 0.776}]})).unwrap();
        // Inset frameless face height: 0.776 - 2*(0.019 + 0.003) = 0.732. The 0.776 row won't fit.
        assert!(
            s.execute("cabinet.edit", &json!({"id": id, "mount": "inset"})).is_err(),
            "inset mount should reject a face layout that only fits overlay"
        );
    }
}
