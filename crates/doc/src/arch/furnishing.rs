//! Furnishings: library items placed in the plan, and the symbol meshes they draw.

use super::ArchId;
use crate::Handle;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for furnishing proxy handles.
pub const FURNISHING_HANDLE_OFFSET: u64 = 0x60000;

/// Maximum number of furnishings in a drawing.
pub const MAX_FURNISHINGS: usize = 4096;

/// Maximum triangles in one imported symbol.
pub const MAX_SYMBOL_TRIANGLES: usize = 400_000;

/// The handle of furnishing `id`.
pub fn furnishing_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(FURNISHING_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the furnishing handle range.
pub fn is_furnishing_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(FURNISHING_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// The furnishing id of handle `h`, if it is a furnishing handle.
pub fn furnishing_id_of_handle(h: Handle) -> Option<ArchId> {
    if !is_furnishing_handle(h) {
        return None;
    }
    let lo = super::ARCH_HANDLE_BASE.saturating_add(FURNISHING_HANDLE_OFFSET);
    u32::try_from(h.0.checked_sub(lo)?).ok().map(ArchId)
}

/// `f{n}`.
pub fn furnishing_label(id: ArchId) -> String {
    format!("f{}", id.0)
}

/// Where a furnishing sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// On the floor.
    #[default]
    Floor,
    /// Hung on a wall.
    Wall,
    /// Hung from the ceiling.
    Ceiling,
    /// On a counter top.
    Counter,
    /// Inside a cabinet.
    InCabinet,
}

impl Placement {
    /// Parse the kebab-case name.
    pub fn parse(s: &str) -> Option<Placement> {
        Some(match s {
            "floor" => Placement::Floor,
            "wall" => Placement::Wall,
            "ceiling" => Placement::Ceiling,
            "counter" => Placement::Counter,
            "in-cabinet" => Placement::InCabinet,
            _ => return None,
        })
    }

    /// The kebab-case name.
    pub fn name(self) -> &'static str {
        match self {
            Placement::Floor => "floor",
            Placement::Wall => "wall",
            Placement::Ceiling => "ceiling",
            Placement::Counter => "counter",
            Placement::InCabinet => "in-cabinet",
        }
    }
}

/// What a furnishing is attached to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FurnishingHost {
    /// Free standing.
    #[default]
    None,
    /// Attached to a wall.
    Wall {
        /// The wall.
        wall: ArchId,
    },
    /// Attached to a cabinet.
    Cabinet {
        /// The cabinet.
        cabinet: ArchId,
    },
    /// Attached to a sink (a faucet).
    Sink {
        /// The sink furnishing.
        furnishing: ArchId,
    },
    /// Sits in the gap between two furnishings.
    Gap {
        /// Left neighbour.
        left: ArchId,
        /// Right neighbour.
        right: ArchId,
    },
}

/// A library item placed in the plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Furnishing {
    /// Identifier.
    pub id: ArchId,
    /// Library item id, e.g. `armchair` or `own:range-30`.
    pub item: String,
    /// Key into `ArchModel.symbols`.
    pub symbol: String,
    /// Display name.
    pub name: String,
    /// Library category.
    pub category: String,
    /// Plan symbol type.
    pub symbol_type: String,
    /// Where it sits.
    pub placement: Placement,
    /// Level index.
    pub level: u32,
    /// Footprint centre.
    pub at: [f64; 2],
    /// Degrees counter-clockwise.
    pub rotation: f64,
    /// Bottom above the level's floor.
    pub elevation: f64,
    /// Width x, depth y, height z (local).
    pub size: [f64; 3],
    /// Mirrored left-right.
    #[serde(default)]
    pub mirrored: bool,
    /// What it is attached to.
    #[serde(default)]
    pub host: FurnishingHost,
    /// Counter cut-out width x depth (sinks, cooktops, basins).
    #[serde(default)]
    pub cutout: Option<[f64; 2]>,
}

impl Furnishing {
    /// Map a symbol vertex `p` (symbol space: Z up, bottom-centred, `extents` = native size)
    /// to plan x, y and z above the level's floor. The symbol's front is local -y.
    pub fn local_to_plan(&self, p: [f64; 3], extents: [f64; 3]) -> [f64; 3] {
        let k = |i: usize| {
            let e = extents[i];
            if e.abs() < 1e-9 { 1.0 } else { self.size[i] / e }
        };
        let mut x = p[0] * k(0);
        let y = p[1] * k(1);
        let z = p[2] * k(2);
        if self.mirrored {
            x = -x;
        }
        let (s, c) = self.rotation.to_radians().sin_cos();
        [x * c - y * s + self.at[0], x * s + y * c + self.at[1], z + self.elevation]
    }
}

/// An sRGB image, 4 bytes per pixel, row 0 at the top.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA bytes.
    pub rgba: Vec<u8>,
}

impl ImageRgba {
    /// Mean colour (sRGB bytes, rounded); black when empty or invalid.
    pub fn mean_srgb(&self) -> [u8; 3] {
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for px in self.rgba.chunks_exact(4) {
            for (s, v) in sum.iter_mut().zip(px) {
                *s += u64::from(*v);
            }
            n += 1;
        }
        if n == 0 {
            return [0; 3];
        }
        let m = |i: usize| ((sum[i] + n / 2) / n).min(255) as u8;
        [m(0), m(1), m(2)]
    }

    /// Whether the buffer length matches and the size is 1..=4096 each way.
    pub fn is_valid(&self) -> bool {
        let ok = |d: u32| (1..=4096).contains(&d);
        ok(self.width)
            && ok(self.height)
            && (self.width as usize).checked_mul(self.height as usize).and_then(|n| n.checked_mul(4)) == Some(self.rgba.len())
    }
}

/// One material-uniform part of a symbol.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SymbolPart {
    /// Part name.
    pub name: String,
    /// Positions, Z up.
    pub positions: Vec<[f32; 3]>,
    /// Normals.
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates; empty when untextured.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle indices.
    pub indices: Vec<u32>,
    /// Linear RGBA.
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Index into `SymbolMesh.textures`.
    pub texture: Option<u32>,
}

/// An imported symbol mesh, Z up, bottom-centred.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SymbolMesh {
    /// 16 hex digits: FNV-1a 64 of the source file's bytes.
    pub key: String,
    /// Parts.
    pub parts: Vec<SymbolPart>,
    /// Textures the parts refer to.
    pub textures: Vec<ImageRgba>,
}

impl SymbolMesh {
    /// Max minus min over all parts; zeros when empty.
    pub fn extents(&self) -> [f64; 3] {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut any = false;
        for p in self.parts.iter().flat_map(|part| part.positions.iter()) {
            any = true;
            for i in 0..3 {
                let v = f64::from(p[i]);
                lo[i] = lo[i].min(v);
                hi[i] = hi[i].max(v);
            }
        }
        if !any {
            return [0.0; 3];
        }
        [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
    }

    /// Total triangle count.
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.indices.len() / 3).sum()
    }
}

/// FNV-1a 64 of `bytes` as 16 lower-case hex digits.
pub fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::super::ArchModel;
    use super::*;

    #[test]
    fn furnishing_handles_round_trip() {
        let h = furnishing_handle(ArchId(7));
        assert!(is_furnishing_handle(h));
        assert_eq!(furnishing_id_of_handle(h), Some(ArchId(7)));
        assert_eq!(furnishing_label(ArchId(7)), "f7");
        assert!(!is_furnishing_handle(super::super::cabinet_handle(ArchId(7))));
        assert_eq!(furnishing_id_of_handle(super::super::cabinet_handle(ArchId(7))), None);
        assert_eq!(furnishing_id_of_handle(Handle(1)), None);
        assert!(h.0 < super::super::ARCH_HANDLE_BASE + super::super::ARCH_HANDLE_SPAN);
        for p in [Placement::Floor, Placement::Wall, Placement::Ceiling, Placement::Counter, Placement::InCabinet] {
            assert_eq!(Placement::parse(p.name()), Some(p));
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{}\"", p.name()));
        }
        assert_eq!(Placement::parse("floating"), None);
    }

    #[test]
    fn local_to_plan_scales_mirrors_rotates_and_moves() {
        let mut f = Furnishing { size: [2.0, 4.0, 1.0], at: [10.0, 20.0], elevation: 0.5, ..Default::default() };
        let ext = [1.0, 2.0, 0.5];
        let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(near(f.local_to_plan([0.5, 1.0, 0.5], ext), [11.0, 22.0, 1.5]));
        f.mirrored = true;
        assert!(near(f.local_to_plan([0.5, 1.0, 0.5], ext), [9.0, 22.0, 1.5]));
        f.mirrored = false;
        f.rotation = 90.0;
        assert!(near(f.local_to_plan([0.5, 0.0, 0.0], ext), [10.0, 21.0, 0.5]));
        // Degenerate extent scales by 1.
        assert!(near(f.local_to_plan([0.0, 0.0, 3.0], [1.0, 1.0, 0.0]), [10.0, 20.0, 3.5]));
    }

    #[test]
    fn symbol_extents_and_triangles() {
        let empty = SymbolMesh::default();
        assert_eq!(empty.extents(), [0.0; 3]);
        assert_eq!(empty.triangles(), 0);
        let part = SymbolPart { positions: vec![[-1.0, -0.5, 0.0], [1.0, 0.5, 2.0], [0.0, 0.0, 1.0]], indices: vec![0, 1, 2], ..Default::default() };
        let m = SymbolMesh { key: "k".into(), parts: vec![part.clone(), part], textures: vec![] };
        assert_eq!(m.extents(), [2.0, 1.0, 2.0]);
        assert_eq!(m.triangles(), 2);
        let img = ImageRgba { width: 2, height: 1, rgba: vec![0, 0, 0, 255, 255, 100, 50, 255] };
        assert!(img.is_valid());
        assert_eq!(img.mean_srgb(), [128, 50, 25]);
        assert!(!ImageRgba { width: 2, height: 1, rgba: vec![0; 7] }.is_valid());
        assert!(!ImageRgba { width: 0, height: 1, rgba: vec![] }.is_valid());
        assert!(!ImageRgba { width: 4097, height: 1, rgba: vec![0; 4097 * 4] }.is_valid());
    }

    #[test]
    fn fnv1a64_hex_is_the_standard_hash() {
        assert_eq!(fnv1a64_hex(b""), "cbf29ce484222325");
        assert_eq!(fnv1a64_hex(b"a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a64_hex(b"foobar"), "85944171f73967e8");
    }

    #[test]
    fn arch_model_without_furnishings_serializes_as_before() {
        let v = serde_json::to_value(ArchModel::default()).unwrap();
        let obj = v.as_object().unwrap();
        for k in ["furnishings", "next_furnishing_id", "symbols", "images", "paints", "backdrop", "defaults"] {
            assert!(!obj.contains_key(k), "{k} must be skipped when empty");
        }
        let back: ArchModel = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), v);
    }

    #[test]
    fn furnishings_round_trip_through_serde() {
        let mut m = ArchModel::default();
        let id = m.push_furnishing(Furnishing {
            item: "sink".into(),
            symbol: "0123456789abcdef".into(),
            size: [1.0, 0.5, 0.3],
            host: FurnishingHost::Gap { left: ArchId(1), right: ArchId(2) },
            cutout: Some([0.8, 0.4]),
            ..Default::default()
        });
        assert_eq!(id, ArchId(0));
        assert!(m.furnishing(id).is_some());
        if let Some(f) = m.furnishing_mut(id) {
            f.mirrored = true;
        }
        m.symbols.insert("0123456789abcdef".into(), SymbolMesh { key: "0123456789abcdef".into(), ..Default::default() });
        m.images.insert("material-oak".into(), ImageRgba { width: 1, height: 1, rgba: vec![1, 2, 3, 255] });
        m.paints.insert("wall/w3/left".into(), super::super::Paint { material: super::super::MaterialRef::parse("siding"), tint: Some([1, 2, 3]) });
        m.backdrop = Some(super::super::Backdrop::Gradient { top: [1, 2, 3], bottom: [4, 5, 6] });
        m.defaults.insert("door".into(), serde_json::json!({"width": 0.9}));
        let json = serde_json::to_string(&m).unwrap();
        let back: ArchModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert!(json.contains("\"kind\":\"gap\""));
    }
}
