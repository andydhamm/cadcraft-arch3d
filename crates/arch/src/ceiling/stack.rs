//! Ceiling finish stacks (phase 16): the layers under a room's rough ceiling, top to bottom.
//!
//! The finished ceiling is the rough ceiling minus [`thickness`]. A layer without a material is
//! an air gap. [`template`] knows the named stacks the `room {"ceilingStack": name}` parameter
//! accepts: "none", "drywall" (12.7 mm) and "suspended" (drywall, a 0.2 m air gap, 16 mm ceiling
//! tile). A layer's material is a plain name or a material key the library offers for a ceiling
//! (`library {"for": "room/<id>/ceiling"}`), anything [`MaterialRef::parse`] reads.

use cadcraft_doc::arch::{CeilingLayer, MaterialRef};

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

/// Drywall: 12.7 mm (1/2 in) gypsum board.
const DRYWALL: f64 = 0.0127;
/// A suspended ceiling's air gap (the plenum between the drywall and the grid).
const PLENUM: f64 = 0.2;
/// A suspended ceiling's tile.
const CEILING_TILE: f64 = 0.016;

/// The layers of a stack that are built and counted, top to bottom: of the first
/// [`MAX_CEILING_LAYERS`], those with a finite, positive thickness. A stored stack always passed
/// [`validate`]; a loaded file's may not have, and its extra or hostile layers are skipped.
pub fn layers(stack: &[CeilingLayer]) -> impl Iterator<Item = &CeilingLayer> {
    stack.iter().take(MAX_CEILING_LAYERS).filter(|l| l.thickness.is_finite() && l.thickness > 0.0)
}

/// The total thickness of a stack: the sum of its [`layers`] (a layer with a non-finite or
/// non-positive thickness in a loaded file adds nothing).
pub fn thickness(stack: &[CeilingLayer]) -> f64 {
    layers(stack).map(|l| l.thickness).sum()
}

/// The named stack `name` (case-insensitive, surrounding spaces ignored), or `None` when there is
/// no such template. The layers are listed top to bottom.
pub fn template(name: &str) -> Option<Vec<CeilingLayer>> {
    let drywall = || CeilingLayer::new("drywall", Some("drywall"), DRYWALL);
    match name.trim().to_ascii_lowercase().as_str() {
        "none" => Some(Vec::new()),
        "drywall" => Some(vec![drywall()]),
        "suspended" => {
            Some(vec![drywall(), CeilingLayer::new("air", None, PLENUM), CeilingLayer::new("ceiling tile", Some("ceiling-tile"), CEILING_TILE)])
        }
        _ => None,
    }
}

/// The template names [`template`] knows, for error messages and menus.
pub fn template_names() -> &'static [&'static str] {
    &["none", "drywall", "suspended"]
}

/// Check a stack before it is stored: at most [`MAX_CEILING_LAYERS`] layers, each 0.001..1 m
/// thick, a material that is absent (an air gap) or a non-empty name or key of at most 128
/// characters that [`MaterialRef::parse`] reads,
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
        if let Some(m) = &l.material
            && (m.trim().is_empty() || m.chars().count() > MAX_MATERIAL_CHARS)
        {
            return Err(format!(
                "ceiling layer {n}: the material must be a non-empty name of at most {MAX_MATERIAL_CHARS} characters, or null for an air gap"
            ));
        }
        if let Some(m) = &l.material
            && MaterialRef::parse(m).is_none()
        {
            return Err(format!("ceiling layer {n}: `{m}` is not a material name or key"));
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
    fn templates_have_the_brief_numbers() {
        let drywall = template("drywall").unwrap();
        assert_eq!(drywall, vec![CeilingLayer::new("drywall", Some("drywall"), 0.0127)]);
        let suspended = template("suspended").unwrap();
        assert_eq!(
            suspended,
            vec![
                CeilingLayer::new("drywall", Some("drywall"), 0.0127),
                CeilingLayer::new("air", None, 0.2),
                CeilingLayer::new("ceiling tile", Some("ceiling-tile"), 0.016),
            ],
            "top to bottom"
        );
        assert!((thickness(&drywall) - 0.0127).abs() < 1e-12);
        assert!((thickness(&suspended) - 0.2287).abs() < 1e-12);
        assert_eq!(template("none"), Some(Vec::new()));
        // Every template is a stack the room command accepts.
        for n in template_names() {
            assert!(validate(&template(n).unwrap()).is_ok(), "{n}");
        }
    }

    #[test]
    fn template_names_case_insensitive() {
        assert_eq!(template_names(), &["none", "drywall", "suspended"]);
        for n in template_names() {
            let want = template(n).unwrap();
            for spelled in [n.to_ascii_uppercase(), format!(" {n} "), n[..1].to_ascii_uppercase() + &n[1..]] {
                assert_eq!(template(&spelled), Some(want.clone()), "{spelled}");
            }
        }
        assert_eq!(template("Suspended"), template("SUSPENDED"));
        for unknown in ["plaster", "", "drywall2", "suspended ceiling"] {
            assert_eq!(template(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn finished_ceiling_with_suspended_is_2_1713() {
        use cadcraft_doc::arch::{ArchModel, Room};
        use cadcraft_geom::Vec2;
        let mut m = ArchModel::default();
        m.ensure_ground_level();
        let ring = vec![Vec2::new(6.05, 3.05), Vec2::new(9.9, 3.05), Vec2::new(9.9, 7.9), Vec2::new(6.05, 7.9)];
        m.push_room(Room { polygon: ring, ceiling_stack: template("Suspended").unwrap(), ..Room::default() });
        let p = Vec2::new(8.0, 5.0);
        let room = m.rooms[0].clone();
        assert!((crate::ceiling::rough_ceiling_in(&m, &room, p).unwrap() - 2.4).abs() < 1e-12);
        assert!((crate::ceiling::finished_ceiling_in(&m, &room, p).unwrap() - 2.1713).abs() < 1e-9);
        m.rooms[0].ceiling_stack = template("drywall").unwrap();
        assert!((crate::ceiling::finished_ceiling_at(&m, 0, p).unwrap() - 2.3873).abs() < 1e-9);
    }

    #[test]
    fn layer_materials_are_library_keys_or_names() {
        // A Materials item id from `library {"for": "room/<id>/ceiling"}`, its key form, a finish
        // and a plain name are all accepted; a key that does not parse is not.
        for m in ["material-oak", "item:material-oak", "finish:wood", "wood", "drywall", "ceiling-tile", "#c0c0c0", "colour:#c0c0c0"] {
            assert!(validate(&[layer(Some(m), 0.0127)]).is_ok(), "{m}");
        }
        for m in ["item:", "colour:#zz0000", "#12", "finish:", "finish:drywall"] {
            assert!(validate(&[layer(Some(m), 0.0127)]).is_err(), "{m}");
        }
    }

    #[test]
    fn effective_layers_are_bounded_and_skip_hostile_ones() {
        let s = [layer(Some("a"), 0.1), layer(Some("b"), f64::NAN), layer(None, 0.0), layer(Some("d"), 0.2)];
        let kept: Vec<f64> = layers(&s).map(|l| l.thickness).collect();
        assert_eq!(kept, vec![0.1, 0.2]);
        // A loaded file with far more layers than a stack may have: only the first
        // MAX_CEILING_LAYERS count, in 3D and in the finished ceiling alike.
        let many = vec![layer(Some("a"), 0.01); 10_000];
        assert_eq!(layers(&many).count(), MAX_CEILING_LAYERS);
        assert!((thickness(&many) - 0.08).abs() < 1e-12);
    }

    #[test]
    fn thickness_skips_hostile_layers() {
        let s = [layer(Some("a"), 0.1), layer(Some("b"), f64::NAN), layer(Some("c"), -1.0), layer(None, f64::INFINITY), layer(Some("d"), 0.2)];
        assert!((thickness(&s) - 0.3).abs() < 1e-12);
        assert_eq!(thickness(&[]), 0.0);
    }
}
