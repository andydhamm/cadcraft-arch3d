//! Cabinet option hook (phase 18, owner p18-cabinet-moldings): parsing and applying this group of
//! `cabinet` / `cabinet.edit` keys.

use crate::cmd::{bad, size_param};
use crate::{Result, Session};
use cadcraft_doc::arch::{Cabinet, CabinetKind, CabinetMolding};
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    pub crown: Option<Option<CabinetMolding>>,
    pub crown_size: Option<Option<[f64; 2]>>,
    pub light_rail: Option<Option<CabinetMolding>>,
    pub light_rail_size: Option<Option<[f64; 2]>>,
}

fn item(cmd: &str, s: &Session, p: &Value, key: &str) -> Result<Option<CabinetMolding>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v.as_str().ok_or_else(|| bad(cmd, format!("`{key}` must be a molding item id or null")))?;
            let profile =
                crate::library::molding::profile_of(&s.library, raw).map_err(|e| bad(cmd, format!("`{key}` is not a valid molding profile: {e}")))?;
            Ok(Some(CabinetMolding { profile, size: None, ..Default::default() }))
        }
    }
}

fn size_of(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<[f64; 2]>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => {
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, format!("`{key}` must be [w, h] or null")))?;
            let w = a[0]
                .as_f64()
                .filter(|n| n.is_finite() && (0.002..=0.5).contains(n))
                .ok_or_else(|| bad(cmd, format!("`{key}` width must be a finite number in [0.002, 0.5]")))?;
            let h = a[1]
                .as_f64()
                .filter(|n| n.is_finite() && (0.002..=0.5).contains(n))
                .ok_or_else(|| bad(cmd, format!("`{key}` height must be a finite number in [0.002, 0.5]")))?;
            Ok(Some(Some([w, h])))
        }
    }
}

/// Parse this group's keys of `p`.
pub(crate) fn parse(cmd: &str, s: &Session, p: &Value) -> Result<Opts> {
    Ok(Opts {
        crown: match p.get("crown") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(item(cmd, s, p, "crown")?),
        },
        crown_size: size_of(cmd, p, "crownSize")?,
        light_rail: match p.get("lightRail") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(item(cmd, s, p, "lightRail")?),
        },
        light_rail_size: size_of(cmd, p, "lightRailSize")?,
    })
}

fn apply_field(
    cmd: &str,
    c: &mut Cabinet,
    kind: &str,
    key: &str,
    size_key: &str,
    value: &Option<Option<CabinetMolding>>,
    size: &Option<Option<[f64; 2]>>,
) -> Result<()> {
    if let Some(opt) = value {
        if !matches!(c.kind, CabinetKind::Wall | CabinetKind::Tall) {
            if opt.is_some() {
                return Err(bad(cmd, format!("`{key}` is for wall and tall cabinets only")));
            }
            return Ok(());
        }
        if let Some(molding) = opt {
            let mut new = molding.clone();
            if let Some(Some(size_value)) = size {
                new.size = Some(*size_value);
            }
            if let Some([_, _]) = new.size {
                // keep the explicit override once resolved.
            }
            match key {
                "crown" => c.crown = Some(new),
                "lightRail" => c.light_rail = Some(new),
                _ => {}
            }
        } else {
            match key {
                "crown" => c.crown = None,
                "lightRail" => c.light_rail = None,
                _ => {}
            }
        }
    }
    if let Some(size_value) = size {
        if !matches!(c.kind, CabinetKind::Wall | CabinetKind::Tall) {
            return Err(bad(cmd, format!("`{size_key}` is for wall and tall cabinets only")));
        }
        let slot = match key {
            "crown" => &mut c.crown,
            "lightRail" => &mut c.light_rail,
            _ => return Ok(()),
        };
        match size_value {
            Some(size) => {
                let Some(m) = slot.as_mut() else {
                    return Err(bad(cmd, format!("`{size_key}` requires a {kind} molding on this cabinet")));
                };
                m.size = Some(*size);
            }
            None => {
                if let Some(m) = slot.as_mut() {
                    m.size = None;
                }
            }
        }
    }
    Ok(())
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    pub(crate) fn apply(&self, cmd: &str, c: &mut Cabinet) -> Result<()> {
        apply_field(cmd, c, "crown", "crown", "crownSize", &self.crown, &self.crown_size)?;
        apply_field(cmd, c, "light rail", "lightRail", "lightRailSize", &self.light_rail, &self.light_rail_size)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, CabinetKind};
    use cadcraft_geom::Vec2;

    #[test]
    fn crown_and_light_rail_params() {
        let mut s = crate::Session::new();
        let mut c = Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Wall,
            position: Vec2::new(0.0, 0.0),
            width: 0.6,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            ..Default::default()
        };
        let opts = Opts {
            crown: Some(Some(CabinetMolding {
                profile: crate::library::molding::profile_of(&s.library, "molding:crown").unwrap(),
                size: Some([0.05, 0.05]),
                ..Default::default()
            })),
            light_rail: Some(Some(CabinetMolding {
                profile: crate::library::molding::profile_of(&s.library, "molding:casing").unwrap(),
                size: Some([0.05, 0.064]),
                ..Default::default()
            })),
            ..Default::default()
        };
        opts.apply("cabinet", &mut c).unwrap();
        assert_eq!(c.crown.as_ref().unwrap().size, Some([0.05, 0.05]));
        assert_eq!(c.light_rail.as_ref().unwrap().size, Some([0.05, 0.064]));
    }

    #[test]
    fn crown_on_a_base_cabinet_is_refused_on_create_and_edit() {
        let mut s = crate::Session::new();
        let mut c = Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 0.0),
            width: 0.6,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            ..Default::default()
        };
        let bad_item = Opts {
            crown: Some(Some(CabinetMolding {
                profile: crate::library::molding::profile_of(&s.library, "molding:crown").unwrap(),
                size: None,
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(bad_item.apply("cabinet", &mut c).is_err());
        let bad_size = Opts { crown_size: Some(Some([0.05, 0.05])), ..Default::default() };
        assert!(bad_size.apply("cabinet", &mut c).is_err());
        c.kind = CabinetKind::Wall;
        c.crown = Some(CabinetMolding {
            profile: crate::library::molding::profile_of(&s.library, "molding:crown").unwrap(),
            size: None,
            ..Default::default()
        });
        let clear = Opts { crown: Some(None), ..Default::default() };
        assert!(clear.apply("cabinet", &mut c).is_ok());
    }
}
