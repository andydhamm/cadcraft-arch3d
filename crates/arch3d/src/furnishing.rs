//! Furnishings in 3D (phase 14): a [`Furnishing`] places a library symbol, and each part of the
//! symbol becomes one mesh `sym:<key>:<i>` owned by the furnishing. The symbols a model uses (by
//! its furnishings and its cabinets' pulls) also give the scene one [`MaterialDef`] per part and
//! their textures.
//!
//! A symbol vertex goes through [`Furnishing::local_to_plan`] (scale to the size, mirror x, rotate,
//! move) and up by the level's floor. Normals take the inverse of the scale, then the mirror and the
//! rotation; a mirrored furnishing reverses its triangles so they stay wound outward.
//!
//! A furnishing whose symbol is missing (a drawing opened without it) is a box of its size named
//! [`MISSING_SYMBOL_MATERIAL`], so the object still shows. Every furnishing probes by its placed
//! box ([`FurnishingBox`]), since symbol meshes need not be closed.

use std::collections::BTreeSet;

use cadcraft_doc::arch::{Furnishing, MAX_SYMBOL_TRIANGLES, SymbolMesh, SymbolPart};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

use crate::{MaterialDef, Mesh, Scene3d, level_for};

/// The material of the box that stands in for a missing symbol.
pub const MISSING_SYMBOL_MATERIAL: &str = "sym:missing:0";

/// How far outside its box a point may be and still hit a furnishing.
const BOX_EPS: f64 = 1e-9;

/// The mesh material of part `part` of symbol `key` on a furnishing: `sym:<key>:<part>`.
pub fn symbol_material(key: &str, part: usize) -> String {
    format!("sym:{key}:{part}")
}

/// The mesh material of part `part` of symbol `key` on a cabinet's pulls: `pull:<key>:<part>`.
pub fn pull_material(key: &str, part: usize) -> String {
    format!("pull:{key}:{part}")
}

/// The [`Scene3d::textures`] key of texture `texture` of symbol `key`: `sym:<key>:t<texture>`.
pub fn symbol_texture(key: &str, texture: usize) -> String {
    format!("sym:{key}:t{texture}")
}

/// Symbol `key` of `model`, when it is there and within [`MAX_SYMBOL_TRIANGLES`].
pub fn symbol_for<'a>(model: &'a ArchModel, key: &str) -> Option<&'a SymbolMesh> {
    model.symbols.get(key).filter(|s| s.triangles() <= MAX_SYMBOL_TRIANGLES)
}

/// Whether `f` can be built: finite placement and a positive, finite size.
fn placeable(f: &Furnishing) -> bool {
    f.at.iter().chain([&f.rotation, &f.elevation]).all(|v| v.is_finite()) && f.size.iter().all(|s| s.is_finite() && *s > 0.0)
}

/// The meshes of furnishing `f`: one `sym:<key>:<i>` mesh per part of its symbol (object `f.id`,
/// at its level's floor elevation), or one box of `f.size` named [`MISSING_SYMBOL_MATERIAL`] when
/// the symbol is missing or gives no mesh. Nothing for a non-finite placement or a size that is not
/// positive.
pub fn furnishing_meshes(model: &ArchModel, f: &Furnishing) -> Vec<Mesh> {
    if !placeable(f) {
        return Vec::new();
    }
    let floor = level_for(model, f.level).elevation;
    if !floor.is_finite() {
        return Vec::new();
    }
    let meshes = match symbol_for(model, &f.symbol) {
        Some(symbol) => {
            let frame = Frame::new(f, symbol.extents(), floor);
            symbol.parts.iter().enumerate().filter_map(|(i, part)| frame.part_mesh(part, symbol_material(&f.symbol, i), f.id)).collect()
        }
        None => Vec::new(),
    };
    if meshes.is_empty() { missing_box(f, floor).into_iter().collect() } else { meshes }
}

/// Whether a symbol part can be copied as given: parallel arrays and triangles that index them.
fn well_formed(part: &SymbolPart) -> bool {
    let n = part.positions.len();
    n > 0
        && u32::try_from(n).is_ok()
        && part.normals.len() == n
        && (part.uvs.is_empty() || part.uvs.len() == n)
        && !part.indices.is_empty()
        && part.indices.len().is_multiple_of(3)
        && part.indices.iter().all(|&i| usize::try_from(i).is_ok_and(|i| i < n))
}

/// The symbol-to-plan map of one furnishing, computed once for all its vertices; the same
/// arithmetic as [`Furnishing::local_to_plan`].
struct Frame {
    scale: [f64; 3],
    mirrored: bool,
    sin: f64,
    cos: f64,
    at: [f64; 2],
    elevation: f64,
    floor: f64,
}

impl Frame {
    fn new(f: &Furnishing, extents: [f64; 3], floor: f64) -> Frame {
        let k = |i: usize| {
            let e = extents[i];
            if e.abs() < 1e-9 { 1.0 } else { f.size[i] / e }
        };
        let (sin, cos) = f.rotation.to_radians().sin_cos();
        Frame { scale: [k(0), k(1), k(2)], mirrored: f.mirrored, sin, cos, at: f.at, elevation: f.elevation, floor }
    }

    /// A symbol vertex in plan, its z above zero (the level's floor added).
    fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let mut x = f64::from(p[0]) * self.scale[0];
        let y = f64::from(p[1]) * self.scale[1];
        let z = f64::from(p[2]) * self.scale[2];
        if self.mirrored {
            x = -x;
        }
        [
            (x * self.cos - y * self.sin + self.at[0]) as f32,
            (x * self.sin + y * self.cos + self.at[1]) as f32,
            (z + self.elevation + self.floor) as f32,
        ]
    }

    /// A symbol normal in plan: through the inverse of the scale (renormalised), then the mirror
    /// and the rotation.
    fn normal(&self, n: [f32; 3]) -> [f32; 3] {
        let v = [f64::from(n[0]) / self.scale[0], f64::from(n[1]) / self.scale[1], f64::from(n[2]) / self.scale[2]];
        let [mut x, y, z] = unit(v).unwrap_or([0.0, 0.0, 1.0]);
        if self.mirrored {
            x = -x;
        }
        [(x * self.cos - y * self.sin) as f32, (x * self.sin + y * self.cos) as f32, z as f32]
    }

    /// One part's mesh, or `None` for a part that is empty or malformed.
    fn part_mesh(&self, part: &SymbolPart, material: String, object: ArchId) -> Option<Mesh> {
        if !well_formed(part) {
            return None;
        }
        let mut indices = part.indices.clone();
        if self.mirrored {
            for tri in indices.as_chunks_mut::<3>().0 {
                tri.swap(1, 2);
            }
        }
        // An untextured part has no UVs; zeros keep the mesh's arrays parallel, as every scene
        // mesh's are.
        let uvs = if part.uvs.is_empty() { vec![[0.0, 0.0]; part.positions.len()] } else { part.uvs.clone() };
        let mut mesh = Mesh::new(&material, object);
        mesh.positions = part.positions.iter().map(|&p| self.point(p)).collect();
        mesh.normals = part.normals.iter().map(|&n| self.normal(n)).collect();
        mesh.uvs = uvs;
        mesh.indices = indices;
        mesh.level_elevation = self.floor;
        Some(mesh)
    }
}

/// `v` scaled to unit length, or `None` when it has no direction.
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let m = v[0].abs().max(v[1].abs()).max(v[2].abs());
    if !(m.is_finite() && m > 0.0) {
        return None;
    }
    let w = [v[0] / m, v[1] / m, v[2] / m];
    let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    Some([w[0] / l, w[1] / l, w[2] / l])
}

/// The box of `f.size` at `f`'s place: the stand-in for a missing symbol.
fn missing_box(f: &Furnishing, floor: f64) -> Option<Mesh> {
    let unit_size = [1.0; 3];
    let ring: Vec<Vec2> = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]]
        .iter()
        .map(|c| {
            // Symbol space of a unit symbol scaled to the size: the corners of the footprint.
            let p = f.local_to_plan([c[0], c[1], 0.0], unit_size);
            Vec2::new(p[0], p[1])
        })
        .collect();
    let z0 = f.elevation + floor;
    let z1 = z0 + f.size[2];
    if !(z0.is_finite() && z1.is_finite()) {
        return None;
    }
    let mut mesh = Mesh::new(MISSING_SYMBOL_MATERIAL, f.id);
    mesh.push_prism(&ring, z0, z1);
    mesh.level_elevation = floor;
    (!mesh.indices.is_empty()).then_some(mesh)
}

/// A furnishing's placed box: its size at its `at`, rotation and elevation. [`crate::probe::probe`]
/// tests a point against these boxes before any mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct FurnishingBox {
    /// The furnishing.
    pub id: ArchId,
    /// Footprint centre in plan.
    pub at: [f64; 2],
    /// Degrees counter-clockwise.
    pub rotation: f64,
    /// Half the width (local x) and half the depth (local y).
    pub half: [f64; 2],
    /// Bottom and top (the level's floor plus the furnishing's elevation, and its height above).
    pub z: [f64; 2],
    /// The level's floor elevation.
    pub level: f64,
}

impl FurnishingBox {
    /// Whether `p` is in the box: its sides and bottom count as inside, its top face does not (as
    /// [`crate::probe::probe_mesh`] treats a closed mesh's top face).
    pub fn contains(&self, p: Vec3) -> bool {
        if !p.is_finite() {
            return false;
        }
        let (s, c) = self.rotation.to_radians().sin_cos();
        let (dx, dy) = (p.x - self.at[0], p.y - self.at[1]);
        let u = dx * c + dy * s;
        let v = dy * c - dx * s;
        u.abs() <= self.half[0] + BOX_EPS && v.abs() <= self.half[1] + BOX_EPS && p.z >= self.z[0] - BOX_EPS && p.z < self.z[1]
    }
}

/// The placed box of `f` on a level whose floor is at `floor`; `None` when `f` cannot be built.
pub fn placed_box(f: &Furnishing, floor: f64) -> Option<FurnishingBox> {
    let z0 = f.elevation + floor;
    let z1 = z0 + f.size[2];
    if !placeable(f) || !z0.is_finite() || !z1.is_finite() {
        return None;
    }
    Some(FurnishingBox { id: f.id, at: f.at, rotation: f.rotation, half: [f.size[0] / 2.0, f.size[1] / 2.0], z: [z0, z1], level: floor })
}

/// The placed box of `f` in `model` (on its level's floor).
pub fn furnishing_box(model: &ArchModel, f: &Furnishing) -> Option<FurnishingBox> {
    placed_box(f, level_for(model, f.level).elevation)
}

/// Add to `scene` one [`MaterialDef`] per part of every symbol a furnishing uses (`sym:<key>:<i>`)
/// and of every cabinet's pull symbol (`pull:<key>:<i>`): the part's base colour, metallic and
/// roughness, tile [1, 1], and its texture `sym:<key>:t<t>` (inserted into `scene.textures`) when it
/// has a valid one.
pub fn symbol_materials(model: &ArchModel, scene: &mut Scene3d) {
    let furnishing_keys: BTreeSet<&str> = model.furnishings.iter().map(|f| f.symbol.as_str()).collect();
    let pull_keys: BTreeSet<&str> = model.cabinets.iter().filter_map(|c| c.pull_symbol.as_deref()).collect();
    for key in furnishing_keys {
        if let Some(symbol) = symbol_for(model, key) {
            add_part_materials(scene, key, symbol, symbol_material);
        }
    }
    for key in pull_keys {
        if let Some(symbol) = symbol_for(model, key) {
            add_part_materials(scene, key, symbol, pull_material);
        }
    }
}

/// One [`MaterialDef`] per part of `symbol`, named by `name(key, part)`, and the textures they use.
fn add_part_materials(scene: &mut Scene3d, key: &str, symbol: &SymbolMesh, name: fn(&str, usize) -> String) {
    for (i, part) in symbol.parts.iter().enumerate() {
        let texture = part.texture.and_then(|t| {
            let t = usize::try_from(t).ok()?;
            let image = symbol.textures.get(t).filter(|image| image.is_valid())?;
            let texture_key = symbol_texture(key, t);
            scene.textures.insert(texture_key.clone(), image.clone());
            Some(texture_key)
        });
        let defaults = MaterialDef::default();
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let base_color = [0, 1, 2, 3].map(|c| finite(part.base_color[c], defaults.base_color[c]));
        let material = MaterialDef {
            base_color,
            metallic: finite(part.metallic, defaults.metallic),
            roughness: finite(part.roughness, defaults.roughness),
            texture,
            tile: [1.0, 1.0],
            ..defaults
        };
        scene.materials.insert(name(key, i), material);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::ImageRgba;

    fn triangle_part() -> SymbolPart {
        SymbolPart {
            positions: vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 1.0]],
            normals: vec![[0.0, -1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            base_color: [0.5, 0.25, 0.125, 1.0],
            ..Default::default()
        }
    }

    fn model_with(part: SymbolPart) -> (ArchModel, Furnishing) {
        let mut model = ArchModel::default();
        model.symbols.insert("k".into(), SymbolMesh { key: "k".into(), parts: vec![part], ..Default::default() });
        let f = Furnishing { symbol: "k".into(), size: [1.0, 1.0, 1.0], ..Default::default() };
        (model, f)
    }

    #[test]
    fn malformed_parts_are_skipped_and_fall_back_to_the_box() {
        let mut bad = triangle_part();
        bad.indices = vec![0, 1, 7];
        let (model, f) = model_with(bad);
        let meshes = furnishing_meshes(&model, &f);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].material, MISSING_SYMBOL_MATERIAL);
        let mut short = triangle_part();
        short.normals.pop();
        let (model, f) = model_with(short);
        assert_eq!(furnishing_meshes(&model, &f)[0].material, MISSING_SYMBOL_MATERIAL);
        let (model, f) = model_with(triangle_part());
        let meshes = furnishing_meshes(&model, &f);
        assert_eq!(meshes[0].material, "sym:k:0");
        assert_eq!(meshes[0].uvs.len(), meshes[0].positions.len(), "an untextured part gets zero UVs");
    }

    #[test]
    fn hostile_placements_build_nothing() {
        let (model, f) = model_with(triangle_part());
        for bad in [
            Furnishing { size: [0.0, 1.0, 1.0], ..f.clone() },
            Furnishing { size: [1.0, -1.0, 1.0], ..f.clone() },
            Furnishing { size: [1.0, 1.0, f64::INFINITY], ..f.clone() },
            Furnishing { at: [f64::NAN, 0.0], ..f.clone() },
            Furnishing { rotation: f64::INFINITY, ..f.clone() },
            Furnishing { elevation: f64::NAN, ..f.clone() },
        ] {
            assert!(furnishing_meshes(&model, &bad).is_empty(), "{bad:?}");
            assert!(placed_box(&bad, 0.0).is_none());
        }
        // A huge scale stays finite in its normals.
        let mut model = model;
        if let Some(s) = model.symbols.get_mut("k") {
            s.parts[0].positions = vec![[-1e-30, -1e-30, 0.0], [1e-30, -1e-30, 0.0], [0.0, 1e-30, 1e-30]];
        }
        for m in furnishing_meshes(&model, &f) {
            assert!(m.normals.iter().flatten().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn over_the_triangle_cap_is_a_box() {
        let mut big = triangle_part();
        big.indices = vec![0; (MAX_SYMBOL_TRIANGLES + 1) * 3];
        let (model, f) = model_with(big);
        assert_eq!(furnishing_meshes(&model, &f)[0].material, MISSING_SYMBOL_MATERIAL);
        let mut scene = Scene3d::default();
        symbol_materials(&model, &mut scene);
        assert!(scene.materials.is_empty());
    }

    #[test]
    fn invalid_textures_are_left_out() {
        let mut part = triangle_part();
        part.texture = Some(0);
        part.uvs = vec![[0.0, 0.0]; 3];
        let (mut model, f) = model_with(part);
        model.furnishings.push(f);
        if let Some(s) = model.symbols.get_mut("k") {
            // Two by two pixels need 16 bytes.
            let mut image = ImageRgba { width: 2, height: 2, ..Default::default() };
            image.rgba = vec![0; 3];
            s.textures.push(image);
        }
        let mut scene = Scene3d::default();
        symbol_materials(&model, &mut scene);
        assert_eq!(scene.materials.get("sym:k:0").and_then(|m| m.texture.clone()), None);
        assert!(scene.textures.is_empty());
    }
}
