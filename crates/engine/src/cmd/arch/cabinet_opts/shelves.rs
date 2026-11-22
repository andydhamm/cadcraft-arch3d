//! Cabinet option hook (phase 18, owner p18-fillers-shelves): parsing and applying this group of
//! `cabinet` / `cabinet.edit` keys.

use crate::{Result, Session};
use cadcraft_doc::arch::{Cabinet, MAX_SHELVES};
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    pub shelves: Option<Option<u32>>,
}

/// Parse this group's keys of `p`.
pub(crate) fn parse(cmd: &str, _s: &Session, p: &Value) -> Result<Opts> {
    let Some(v) = p.get("shelves") else {
        return Ok(Opts { shelves: None });
    };
    let shelves = match v {
        Value::Null => Some(None),
        Value::String(s) if s.trim().eq_ignore_ascii_case("auto") => Some(None),
        Value::Number(n) => {
            let n = n.as_u64().ok_or_else(|| crate::cmd::bad(cmd, "`shelves` must be a whole number from 0..=12 or \"auto\""))?;
            let n = u32::try_from(n).map_err(|_| crate::cmd::bad(cmd, "`shelves` must be a whole number from 0..=12 or \"auto\""))?;
            if n > MAX_SHELVES {
                return Err(crate::cmd::bad(cmd, format!("`shelves` must be from 0..={MAX_SHELVES} and not {n}")));
            }
            Some(Some(n))
        }
        Value::String(s) => {
            let n = s.trim().parse::<u32>().map_err(|_| crate::cmd::bad(cmd, "`shelves` must be a whole number from 0..=12 or \"auto\""))?;
            if n > MAX_SHELVES {
                return Err(crate::cmd::bad(cmd, format!("`shelves` must be from 0..={MAX_SHELVES} and not {n}")));
            }
            Some(Some(n))
        }
        _ => return Err(crate::cmd::bad(cmd, "`shelves` must be a whole number from 0..=12 or \"auto\"")),
    };
    Ok(Opts { shelves })
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    pub(crate) fn apply(&self, _cmd: &str, c: &mut Cabinet) -> Result<()> {
        if let Some(v) = self.shelves {
            c.shelves = v;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::Cabinet;
    use serde_json::json;

    #[test]
    fn shelves_boundary_values() {
        let s = Session::new();
        // 0 shelves is valid
        let opts = parse("cabinet", &s, &json!({"shelves": 0})).unwrap();
        assert_eq!(opts.shelves, Some(Some(0)));
        // 12 shelves is valid (MAX_SHELVES)
        let opts = parse("cabinet", &s, &json!({"shelves": 12})).unwrap();
        assert_eq!(opts.shelves, Some(Some(12)));
        // 13 shelves is refused
        assert!(parse("cabinet", &s, &json!({"shelves": 13})).is_err());
        // Negative shelves is refused
        assert!(parse("cabinet", &s, &json!({"shelves": -1})).is_err());
        // Float shelves is refused
        assert!(parse("cabinet", &s, &json!({"shelves": 2.5})).is_err());
        // Very large number is refused
        assert!(parse("cabinet", &s, &json!({"shelves": 1000000})).is_err());
        // String number "3" is valid
        let opts = parse("cabinet", &s, &json!({"shelves": "3"})).unwrap();
        assert_eq!(opts.shelves, Some(Some(3)));
        // String number with whitespace is valid
        let opts = parse("cabinet", &s, &json!({"shelves": " 4 "})).unwrap();
        assert_eq!(opts.shelves, Some(Some(4)));
        // "auto" case variations
        let opts = parse("cabinet", &s, &json!({"shelves": "AUTO"})).unwrap();
        assert_eq!(opts.shelves, Some(None));
        let opts = parse("cabinet", &s, &json!({"shelves": "Auto"})).unwrap();
        assert_eq!(opts.shelves, Some(None));
        let opts = parse("cabinet", &s, &json!({"shelves": " auto "})).unwrap();
        assert_eq!(opts.shelves, Some(None));
    }

    #[test]
    fn shelves_option() {
        let session = Session::new();
        let opts = parse("cabinet", &session, &json!({"shelves": 4})).unwrap();
        let mut c = Cabinet::default();
        opts.apply("cabinet", &mut c).unwrap();
        assert_eq!(c.shelves, Some(4));
        let opts = parse("cabinet", &session, &json!({"shelves": "auto"})).unwrap();
        let mut c = Cabinet::default();
        c.shelves = Some(2);
        opts.apply("cabinet", &mut c).unwrap();
        assert_eq!(c.shelves, None);
        let err = parse("cabinet", &session, &json!({"shelves": 13})).unwrap_err();
        assert!(err.to_string().contains("shelves"));
    }
}
