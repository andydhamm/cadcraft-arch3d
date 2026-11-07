//! Ceiling finish stacks (phase 16): the layers under a room's rough ceiling, top to bottom.
//!
//! The finished ceiling is the rough ceiling minus [`thickness`]. A layer without a material is
//! an air gap. [`template`] knows the named stacks the `room {"ceilingStack": name}` parameter
//! accepts.

use cadcraft_doc::arch::CeilingLayer;

/// Most layers a ceiling stack may have.
pub const MAX_CEILING_LAYERS: usize = 8;
/// Thinnest accepted layer, in metres.
const MIN_LAYER: f64 = 0.001;
/// Thickest accepted layer, and the most a whole stack may add up to, in metres.
const MAX_STACK: f64 = 1.0;
/// Longest accepted material key, in characters.
const MAX_MATERIAL_CHARS: usize = 128;
/// Longest accepted layer name, in characters.
const MAX_NAME_CHARS: usize = 64;

/// The total thickness of a stack: the sum of its finite, positive layer thicknesses (a layer
/// with a non-finite or non-positive thickness in a loaded file adds nothing).
pub fn thickness(stack: &[CeilingLayer]) -> f64 {
    stack.iter().map(|l| l.thickness).filter(|t| t.is_finite() && *t > 0.0).sum()
}

/// The named stack `name` (case-insensitive), or `None` when there is no such template.
pub fn template(name: &str) -> Option<Vec<CeilingLayer>> {
    match name.trim().to_ascii_lowercase().as_str() {
        "none" => Some(Vec::new()),
        _ => None,
    }
}

/// The template names [`template`] knows, for error messages and menus.
pub fn template_names() -> &'static [&'static str] {
    &["none"]
}

/// Check a stack before it is stored: at most [`MAX_CEILING_LAYERS`] layers, each 0.001..1 m
/// thick, a material that is absent (an air gap) or a non-empty key of at most 128 characters,
/// a name of at most 64 characters, a material on the top and the bottom layer (an air gap goes
/// between layers), and at most 1 m in all. An empty stack is valid.
pub fn validate(stack: &[CeilingLayer]) -> Result<(), String> {
    if stack.len() > MAX_CEILING_LAYERS {
        return Err(format!("a ceiling stack has at most {MAX_CEILING_LAYERS} layers, got {}", stack.len()));
    }
    for (i, l) in stack.iter().enumerate() {
        let n = i + 1;
        if !(l.thickness.is_finite() && (MIN_LAYER..=MAX_STACK).contains(&l.thickness)) {
            return Err(format!("ceiling layer {n}: the thickness must be between {MIN_LAYER} and {MAX_STACK} m"));
        }
        if let Some(m) = &l.material {
            if m.trim().is_empty() || m.chars().count() > MAX_MATERIAL_CHARS {
                return Err(format!(
                    "ceiling layer {n}: the material must be a non-empty name of at most {MAX_MATERIAL_CHARS} characters, or null for an air gap"
                ));
            }
        }
        if l.name.chars().count() > MAX_NAME_CHARS {
            return Err(format!("ceiling layer {n}: the name has at most {MAX_NAME_CHARS} characters"));
        }
    }
    if stack.first().is_some_and(|l| l.material.is_none()) {
        return Err("the top ceiling layer needs a material (an air gap goes between layers)".into());
    }
    if stack.last().is_some_and(|l| l.material.is_none()) {
        return Err("the bottom ceiling layer needs a material (an air gap goes between layers)".into());
    }
    let total = thickness(stack);
    if total > MAX_STACK + 1e-12 {
        return Err(format!("a ceiling stack is at most {MAX_STACK} m thick, got {total:.4} m"));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn layer(material: Option<&str>, thickness: f64) -> CeilingLayer {
        CeilingLayer { material: material.map(str::to_string), thickness, ..Default::default() }
    }

    #[test]
    fn stack_template_none() {
        assert_eq!(template("none"), Some(Vec::new()));
        assert_eq!(template(" NONE "), Some(Vec::new()));
        assert_eq!(template("plaster"), None);
        assert!(template_names().contains(&"none"));
        for n in template_names() {
            assert!(template(n).is_some(), "{n}");
        }
    }

    #[test]
    fn stack_validate_rejects_bad_layers() {
        // Good stacks.
        assert!(validate(&[]).is_ok());
        let suspended = [layer(Some("drywall"), 0.0127), layer(None, 0.2), layer(Some("ceiling-tile"), 0.016)];
        assert!(validate(&suspended).is_ok());
        assert!((thickness(&suspended) - 0.2287).abs() < 1e-12);
        assert!(validate(&vec![layer(Some("drywall"), 0.01); 8]).is_ok());
        assert!(validate(&[layer(Some("a"), 0.5), layer(Some("b"), 0.5)]).is_ok(), "exactly 1 m is fine");
        // Bad thicknesses.
        for t in [-0.01, 0.0, 0.0009, 1.5, f64::NAN, f64::INFINITY] {
            assert!(validate(&[layer(Some("drywall"), t)]).is_err(), "{t}");
        }
        // Too many layers.
        let e = validate(&vec![layer(Some("drywall"), 0.01); 9]).unwrap_err();
        assert!(e.contains("at most 8"), "{e}");
        // Air gaps on the outside.
        assert!(validate(&[layer(None, 0.1)]).is_err());
        assert!(validate(&[layer(None, 0.1), layer(Some("tile"), 0.016)]).is_err());
        assert!(validate(&[layer(Some("tile"), 0.016), layer(None, 0.1)]).is_err());
        // Materials and names.
        assert!(validate(&[layer(Some(""), 0.01)]).is_err());
        assert!(validate(&[layer(Some("   "), 0.01)]).is_err());
        assert!(validate(&[layer(Some(&"m".repeat(129)), 0.01)]).is_err());
        assert!(validate(&[layer(Some(&"m".repeat(128)), 0.01)]).is_ok());
        let long_name = CeilingLayer { name: "n".repeat(65), ..layer(Some("drywall"), 0.01) };
        assert!(validate(&[long_name]).is_err());
        // Over 1 m in all.
        assert!(validate(&[layer(Some("a"), 0.6), layer(Some("b"), 0.6)]).is_err());
    }

    #[test]
    fn thickness_skips_hostile_layers() {
        let s = [layer(Some("a"), 0.1), layer(Some("b"), f64::NAN), layer(Some("c"), -1.0), layer(None, f64::INFINITY), layer(Some("d"), 0.2)];
        assert!((thickness(&s) - 0.3).abs() < 1e-12);
        assert_eq!(thickness(&[]), 0.0);
    }
}
