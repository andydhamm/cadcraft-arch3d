//! The optional PBR fields of a material (phase 22) and the table that gives library materials
//! their values.
//!
//! [`MaterialExt`] carries the lobes the base colour / metallic / roughness triple cannot say:
//! clear coat, anisotropy, sheen, transmission with volume, an edge bevel and a per-feature colour
//! variation. [`ext_for`] is the one table from a mesh material name to those values;
//! [`resolve`] prefers an explicit [`crate::MaterialDef`] in the scene.

use crate::Scene3d;

/// The optional PBR fields of a material. `Default` is everything off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialExt {
    /// Clear-coat strength, 0 = off.
    pub clearcoat: f32,
    /// Clear-coat roughness.
    pub clearcoat_roughness: f32,
    /// Anisotropy, 0..1, 0 = off.
    pub anisotropy: f32,
    /// Anisotropy direction, radians.
    pub anisotropy_rotation: f32,
    /// Sheen colour, 0 = off.
    pub sheen: [f32; 3],
    /// Sheen roughness.
    pub sheen_roughness: f32,
    /// Transmission, 0..1, 0 = opaque.
    pub transmission: f32,
    /// A thin surface (no volume) rather than a solid one.
    pub thin: bool,
    /// Index of refraction.
    pub ior: f32,
    /// Volume thickness in metres.
    pub thickness: f32,
    /// Edge bevel radius in metres: `None` = the default 2 mm, `Some(0.0)` = hard.
    pub edge_radius: Option<f32>,
    /// Metres per colour-variation feature, 0 = none.
    pub variation_scale: f32,
    /// Strength of the colour variation, 0..1.
    pub variation_amount: f32,
    /// Metallic override used only by the path tracer under materials: full; never exported.
    pub metallic: Option<f32>,
    /// Roughness override used only by the path tracer under materials: full; never exported.
    pub roughness: Option<f32>,
}

impl Default for MaterialExt {
    /// Everything off: ior 1.5, clear-coat roughness 0.03, sheen roughness 0.5.
    fn default() -> Self {
        MaterialExt {
            clearcoat: 0.0,
            clearcoat_roughness: 0.03,
            anisotropy: 0.0,
            anisotropy_rotation: 0.0,
            sheen: [0.0; 3],
            sheen_roughness: 0.5,
            transmission: 0.0,
            thin: false,
            ior: 1.5,
            thickness: 0.0,
            edge_radius: None,
            variation_scale: 0.0,
            variation_amount: 0.0,
            metallic: None,
            roughness: None,
        }
    }
}

impl MaterialExt {
    /// True when every field is its default.
    pub fn is_default(&self) -> bool {
        *self == MaterialExt::default()
    }

    /// True when a coat, anisotropy, sheen or transmission is above 0.
    pub fn has_lobes(&self) -> bool {
        self.clearcoat > 0.0 || self.anisotropy > 0.0 || self.sheen.iter().any(|c| *c > 0.0) || self.transmission > 0.0
    }
}

/// A matcher over a lower-cased material name with its opening-part prefix stripped.
type Matcher = fn(&str) -> bool;

fn has(name: &str, words: &[&str]) -> bool {
    words.iter().any(|w| name.contains(w))
}

fn starts(name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| name.starts_with(p))
}

const OFF: MaterialExt = MaterialExt {
    clearcoat: 0.0,
    clearcoat_roughness: 0.03,
    anisotropy: 0.0,
    anisotropy_rotation: 0.0,
    sheen: [0.0; 3],
    sheen_roughness: 0.5,
    transmission: 0.0,
    thin: false,
    ior: 1.5,
    thickness: 0.0,
    edge_radius: None,
    variation_scale: 0.0,
    variation_amount: 0.0,
    metallic: None,
    roughness: None,
};

/// The table, first match wins: (matcher, values).
const TABLE: &[(Matcher, MaterialExt)] = &[
    // Hard-edged, otherwise plain.
    (|n| starts(n, &["light-", "plant-", "path-"]) || matches!(n, "terrain" | "ground" | "horizon"), MaterialExt { edge_radius: Some(0.0), ..OFF }),
    // Glazing: today's glass alpha is 0.35, so transmission 0.65.
    (|n| n == "glass-wall", MaterialExt { transmission: 0.65, thickness: 0.01, edge_radius: Some(0.0), ..OFF }),
    (|n| n.starts_with("glass"), MaterialExt { transmission: 0.65, thickness: 0.006, edge_radius: Some(0.0), ..OFF }),
    // Stainless steel appliances: brushed-steel anisotropy plus a soft sheen.
    (
        |n| matches!(n, "fixture-range" | "fixture-refrigerator" | "fixture-dishwasher") || n.contains("stainless"),
        MaterialExt {
            anisotropy: 0.6,
            sheen: [0.3; 3],
            sheen_roughness: 0.35,
            metallic: Some(1.0),
            roughness: Some(0.35),
            variation_scale: 0.4,
            variation_amount: 0.15,
            ..OFF
        },
    ),
    // Quartz and tile.
    (
        |n| starts(n, &["counter", "countertop", "backsplash"]) || n.ends_with("tile"),
        MaterialExt { clearcoat: 0.6, clearcoat_roughness: 0.08, variation_scale: 0.5, variation_amount: 0.1, ..OFF },
    ),
    // Fabric.
    (
        |n| has(n, &["carpet", "fabric", "drap", "curtain", "rug", "sofa", "bed"]),
        MaterialExt { sheen: [0.5; 3], sheen_roughness: 0.6, variation_scale: 0.05, variation_amount: 0.2, ..OFF },
    ),
    // Wood, siding, masonry and plain building surfaces: only variation.
    (
        |n| has(n, &["wood", "siding", "brick", "shingle", "concrete", "drywall", "plaster"]) || matches!(n, "wall" | "floor" | "ceiling" | "roof"),
        MaterialExt { variation_scale: 0.6, variation_amount: 0.12, ..OFF },
    ),
];

/// The extra PBR values of a mesh material NAME (case-insensitive). An opening part
/// `window-<part>` / `door-<part>` is its part. Unknown names are all off.
pub fn ext_for(material: &str) -> MaterialExt {
    let name = material.to_ascii_lowercase();
    let part = name.strip_prefix("window-").or_else(|| name.strip_prefix("door-")).filter(|p| !p.is_empty()).unwrap_or(&name);
    TABLE.iter().find(|(matches, _)| matches(part)).map(|(_, ext)| *ext).unwrap_or_default()
}

/// The values for `material` in `scene`: the `ext` of a [`Scene3d::materials`] entry when it is
/// not default, else [`ext_for`].
pub fn resolve(scene: &Scene3d, material: &str) -> MaterialExt {
    match scene.materials.get(material) {
        Some(def) if !def.ext.is_default() => def.ext,
        _ => ext_for(material),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MaterialDef;

    #[test]
    fn ext_default_is_all_off() {
        let e = MaterialExt::default();
        assert!(e.is_default());
        assert!(!e.has_lobes());
        assert_eq!(e.ior, 1.5);
        assert_eq!(e.clearcoat_roughness, 0.03);
        assert_eq!(e.sheen_roughness, 0.5);
        assert_eq!(MaterialDef::default().ext, e);
    }

    #[test]
    fn stainless_gets_anisotropy_quartz_clearcoat_fabric_sheen() {
        let s = ext_for("fixture-range");
        assert_eq!((s.anisotropy, s.metallic, s.roughness), (0.6, Some(1.0), Some(0.35)));
        assert_eq!(ext_for("Brushed-Stainless"), s);
        let q = ext_for("countertop-quartz");
        assert_eq!((q.clearcoat, q.clearcoat_roughness, q.variation_scale), (0.6, 0.08, 0.5));
        assert_eq!(ext_for("ceramic-tile").clearcoat, 0.6);
        let f = ext_for("carpet");
        assert_eq!((f.sheen, f.sheen_roughness, f.variation_amount), ([0.5; 3], 0.6, 0.2));
        assert!(f.has_lobes());
    }

    #[test]
    fn glazing_gets_transmission_and_volume() {
        let g = ext_for("window-glass");
        assert_eq!((g.transmission, g.ior, g.thickness, g.edge_radius), (0.65, 1.5, 0.006, Some(0.0)));
        assert_eq!(ext_for("door-glass"), g);
        let w = ext_for("glass-wall");
        assert_eq!((w.transmission, w.thickness), (0.65, 0.01));
    }

    #[test]
    fn plain_wall_has_no_lobes() {
        for name in ["wall", "floor", "ceiling", "roof", "brick", "wood-oak"] {
            let e = ext_for(name);
            assert!(!e.has_lobes(), "{name}");
            assert_eq!((e.variation_scale, e.variation_amount), (0.6, 0.12), "{name}");
        }
        assert_eq!(ext_for("terrain").edge_radius, Some(0.0));
        assert!(!ext_for("terrain").has_lobes());
        assert!(ext_for("no-such-material").is_default());
    }

    #[test]
    fn resolve_prefers_a_material_def() {
        let mut scene = Scene3d::default();
        assert_eq!(resolve(&scene, "wall"), ext_for("wall"));
        let custom = MaterialExt { clearcoat: 0.9, ..MaterialExt::default() };
        scene.materials.insert("wall".to_string(), MaterialDef { ext: custom, ..Default::default() });
        assert_eq!(resolve(&scene, "wall"), custom);
        scene.materials.insert("carpet".to_string(), MaterialDef::default());
        assert_eq!(resolve(&scene, "carpet"), ext_for("carpet"));
    }
}
