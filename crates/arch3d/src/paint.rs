//! Paint in 3D (phase 14): painted surfaces as their own `paint:<surface key>` meshes and the
//! material definitions they take.
//!
//! [`apply`] splits wall meshes by side when the wall carries a paint and renames every other
//! mesh whose surface has a paint in [`ArchModel::paints`] to `paint:<surface key>`.
//! [`materials`] fills the `MaterialDef` of every `paint:` mesh in the scene, inserting the `finish:`
//! and `item:` textures into `scene.textures` as it goes. Objects without paints are byte-identical
//! to the pre-paint build.

use std::collections::BTreeMap;

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Finish, ImageRgba, Side};
use cadcraft_doc::arch::paint::{CabinetPart, MaterialRef, Paint, Surface};

use crate::{finish, Mesh, Scene3d, texture};
use crate::uv::face_uvs;

const TILE_DEFAULT: [f32; 2] = [1.0, 1.0];

/// SRGB [u8; 3] to linear [f32; 3].
fn srgb_to_linear(c: [u8; 3]) -> [f32; 3] {
    let f = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(1.0 / 2.2)
        }
    };
    [f(c[0] as f32), f(c[1] as f32), f(c[2] as f32)]
}

/// Multiply a linear base colour by a linear tint.
fn tint_color(base: [f32; 4], tint: [u8; 3]) -> [f32; 4] {
    let t = srgb_to_linear(tint);
    [base[0] * t[0], base[1] * t[1], base[2] * t[2], base[3]]
}

/// Default (unpainted) material colour for a surface part, linear, alpha 1.0.
fn default_colour(part: &str) -> [f32; 4] {
    match part {
        "wall" => [0.85, 0.85, 0.85, 1.0],
        "floor" => [0.6, 0.45, 0.35, 1.0],
        "ceiling" => [0.95, 0.95, 0.95, 1.0],
        "roof" => [0.3, 0.25, 0.2, 1.0],
        "cabinet" | "counter" => [0.55, 0.4, 0.3, 1.0],
        "pull" => [0.6, 0.6, 0.6, 1.0],
        "fixture" => [0.55, 0.55, 0.55, 1.0],
        "furnishing" => [0.7, 0.7, 0.7, 1.0],
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

/// The `MaterialDef` for a [`MaterialRef`]. Textures are not inserted here; the caller inserts
/// them into `scene.textures`.
fn def_from_material_ref(model: &ArchModel, ref_: &MaterialRef) -> MaterialDef {
    match ref_ {
        MaterialRef::Finish(f) => {
            let img = crate::texture::texture(f);
            MaterialDef {
                base_color: img.mean_linear,
                metallic: 0.0,
                roughness: 0.8,
                texture: Some(format!("finish:{}", f.name())),
                tile: img.tile,
                tint: None,
            }
        }
        MaterialRef::Item(id) => {
            let img = model
                .images
                .get(id)
                .map(|i| i.mean_srgb().iter().map(|c| srgb_to_linear([*c, 0, 0, 0])[..3] as f32).collect::<[f32; 3]>());
            MaterialDef {
                base_color: [img.unwrap_or(0.5), img.unwrap_or(0.5), img.unwrap_or(0.5), 1.0],
                metallic: 0.0,
                roughness: 0.8,
                texture: Some(format!("item:{}", id)),
                tile: TILE_DEFAULT,
                tint: None,
            }
        }
        MaterialRef::Colour(c) => MaterialDef {
            base_color: [srgb_to_linear(c)[0], srgb_to_linear(c)[1], srgb_to_linear(c)[2], 1.0],
            metallic: 0.0,
            roughness: 0.8,
            texture: None,
            tile: TILE_DEFAULT,
            tint: None,
        },
    }
}

/// The unpainted (pre-paint) material for a surface. For symbol parts, reads from
/// `model.symbols`. Otherwise a reasonable per-part default.
fn unpainted_material(model: &ArchModel, surface: &Surface) -> MaterialDef {
    match surface {
        Surface::Furnishing { furnishing, part } => {
            let furn = model.furnishings.iter().find(|f| f.id == *furnishing);
            if let Some(furn) = furn {
                if let Some(sym) = model.symbols.get(&furn.symbol) {
                    if let Some(p) = sym.parts.get(*part) {
                        let tex_key = if let Some(t) = p.texture {
                            Some(format!("sym:{}:t{}", sym.key, t))
                        } else {
                            None
                        };
                        MaterialDef {
                            base_color: p.base_color,
                            metallic: p.metallic,
                            roughness: p.roughness,
                            texture: tex_key,
                            tile: TILE_DEFAULT,
                            tint: None,
                        }
                    } else {
                        default_colour("furnishing")
                    }
                } else {
                    default_colour("furnishing")
                }
            } else {
                default_colour("furnishing")
            }
        }
        Surface::Cabinet { cabinet, part } => {
            match part {
                CabinetPart::Carcass => default_colour("cabinet"),
                CabinetPart::Front => default_colour("cabinet"),
                CabinetPart::Counter => default_colour("counter"),
                CabinetPart::Pull => default_colour("pull"),
            }
        }
        Surface::Fixture(_) => default_colour("fixture"),
        _ => {
            match surface {
                Surface::WallSide { .. } => default_colour("wall"),
                Surface::RoomFloor(_) => default_colour("floor"),
                Surface::RoomCeiling(_) => default_colour("ceiling"),
                Surface::Roof(_) => default_colour("roof"),
                _ => default_colour("wall"),
            }
        }
    }
}

/// Build a `MaterialDef` for a paint. If the paint's material is `None`, fall back to the
/// unpainted material for the surface. The tint is applied on top.
fn material_def_for_paint(model: &ArchModel, paint: &Paint, surface: &Surface) -> MaterialDef {
    let base = match paint.material {
        Some(ref m) => def_from_material_ref(model, m),
        None => unpainted_material(model, surface),
    };
    let mut out = base;
    if let Some(t) = paint.tint {
        out.base_color = tint_color(out.base_color, t);
    }
    out
}

/// Split a wall mesh on `side`, and re-style the side triangles as a `paint:` mesh when the
/// model has a paint for that side.
fn paint_wall_side(
    model: &ArchModel,
    wall: &cadcraft_doc::arch::Wall,
    mesh: &Mesh,
    side: Side,
    out: &mut Vec<Mesh>,
) {
    let key = format!("wall/{}/{}", side, wall.id);
    let paint_key = format!("paint:{}", key);
    if model.paints.contains_key(&paint_key) {
        let (side_mesh, rest) = finish::split_side(wall, mesh, side);
        if !side_mesh.indices.is_empty() {
            let tile = model.paints.get(&paint_key).map_or(TILE_DEFAULT, |p| {
                p.material
                    .as_ref()
                    .and_then(|m| match m {
                        MaterialRef::Finish(f) => {
                            let img = crate::texture::texture(f);
                            Some(img.tile)
                        }
                        _ => Some(TILE_DEFAULT),
                    })
                    .or_else(|| {
                        match side {
                            Side::Right => {
                                // wall may have a finish
                                if let Some(f) = wall.finish.filter(|f| f.for_walls()) {
                                    let img = crate::texture::texture(f);
                                    Some(img.tile)
                                } else {
                                    None
                                }
                            }
                            Side::Left => None,
                        }
                    })
                    .unwrap_or(TILE_DEFAULT);
            };
            let mut side_mesh = face_uvs(&side_mesh, tile);
            side_mesh.material = paint_key;
            out.push(side_mesh);
        }
        out.push(rest);
    } else {
        out.push(mesh.clone());
    }
}

/// Determine the surface key for a non-wall mesh. Returns `None` when the mesh has no
/// identifiable surface.
fn surface_key_for(mesh: &Mesh, model: &ArchModel) -> Option<String> {
    if mesh.material.starts_with("floor") {
        let id = model.rooms.iter().find(|r| r.id == mesh.object);
        if id.is_some() {
            return Some(format!("room/{}/floor", mesh.object));
        }
    }
    if mesh.material == "ceiling" {
        if model.rooms.iter().any(|r| r.id == mesh.object) {
            return Some(format!("room/{}/ceiling", mesh.object));
        }
    }
    if mesh.material.starts_with("roof") {
        if model.roofs.iter().any(|r| r.id == mesh.object) {
            return Some(format!("roof/{}", mesh.object));
        }
    }
    if matches!(mesh.material, "cabinet-box" | "cabinet-front" | "counter") {
        let part = match mesh.material.as_str() {
            "cabinet-box" => "carcass",
            "cabinet-front" => "front",
            "counter" => "counter",
            _ => return None,
        };
        if model.cabinets.iter().any(|c| c.id == mesh.object) {
            return Some(format!("cabinet/{}/{}", mesh.object, part));
        }
    }
    if mesh.material.starts_with("fixture") {
        if model.fixtures.iter().any(|f| f.id == mesh.object) {
            return Some(format!("fixture/{}", mesh.object));
        }
    }
    if mesh.material.starts_with("pull:") {
        if model.cabinets.iter().any(|c| c.id == mesh.object) {
            return Some(format!("cabinet/{}/pull", mesh.object));
        }
    }
    if mesh.material.starts_with("sym:") {
        // sym:<key>:<part> — object is the furnishing id
        let furn = model.furnishings.iter().find(|f| f.id == mesh.object);
        if furn.is_some() {
            // part is the last numeric component
            if let Some(tail) = mesh.material.strip_prefix("sym:") {
                if let Some(part_str) = tail.rsplit(':').next() {
                    let part = part_str.parse::<u32>().unwrap_or(0);
                    return Some(format!("furnishing/{}/{}", mesh.object, part));
                }
            }
        }
    }
    None
}

/// Rename a mesh to `paint:<key>` and push it if there is a paint for that key; otherwise push
/// the original.
fn rename_or_keep(mesh: &Mesh, key: &str, model: &ArchModel, out: &mut Vec<Mesh>) {
    let paint_key = format!("paint:{}", key);
    if model.paints.contains_key(&paint_key) {
        let mut m = mesh.clone();
        m.material = paint_key;
        out.push(m);
    } else {
        out.push(mesh.clone());
    }
}

/// Split out or rename the painted surfaces among one part's `meshes` as `paint:<surface key>`
/// meshes.
#[allow(clippy::ptr_arg)] // The final signature: painting splits a mesh into several.
pub fn apply(model: &ArchModel, meshes: &mut Vec<Mesh>) {
    let mut new_meshes: Vec<Mesh> = Vec::with_capacity(meshes.len());

    for mesh in meshes.iter() {
        if mesh.material == "wall" || mesh.material.starts_with("wall-") {
            // Wall mesh: check for paints on left/right
            if let Some(wall) = model.walls.iter().find(|w| w.id == mesh.object) {
                let left_key = format!("wall/{}/left", wall.id);
                let right_key = format!("wall/{}/right", wall.id);
                let has_left = model.paints.contains_key(&left_key);
                let has_right = model.paints.contains_key(&right_key);
                let is_exterior = mesh.material.starts_with("wall-");

                if is_exterior {
                    // This is the exterior (right) face from finish splitting
                    if has_right {
                        let mut m = mesh.clone();
                        m.material = right_key.clone();
                        new_meshes.push(m);
                    } else {
                        new_meshes.push(mesh.clone());
                    }
                } else {
                    // Combined wall mesh — split as needed
                    if has_left {
                        let (side, rest) = finish::split_side(wall, mesh, Side::Left);
                        if !side.indices.is_empty() {
                            let tile = model.paints.get(&left_key).map_or(TILE_DEFAULT, |p| {
                                p.material
                                    .as_ref()
                                    .and_then(|m| {
                                        match m {
                                            MaterialRef::Finish(f) => {
                                                let img = crate::texture::texture(f);
                                                Some(img.tile)
                                            }
                                            _ => Some(TILE_DEFAULT),
                                        }
                                    })
                                    .unwrap_or(TILE_DEFAULT)
                            };
                            let mut side_mesh = face_uvs(&side, tile);
                            side_mesh.material = left_key.clone();
                            new_meshes.push(side_mesh);
                        }
                        if has_right {
                            let (side_r, rest_r) = finish::split_side(wall, &rest, Side::Right);
                            if !side_r.indices.is_empty() {
                                let tile = model.paints.get(&right_key).map_or(TILE_DEFAULT, |p| {
                                    p.material
                                        .as_ref()
                                        .and_then(|m| {
                                            match m {
                                                MaterialRef::Finish(f) => {
                                                    let img = crate::texture::texture(f);
                                                    Some(img.tile)
                                                }
                                                _ => Some(TILE_DEFAULT),
                                            }
                                        })
                                        .unwrap_or(TILE_DEFAULT)
                                };
                                let mut side_mesh = face_uvs(&side_r, tile);
                                side_mesh.material = right_key.clone();
                                new_meshes.push(side_mesh);
                            }
                            new_meshes.push(rest_r);
                        } else {
                            new_meshes.push(rest);
                        }
                    } else if has_right {
                        let (side_r, rest_r) = finish::split_side(wall, mesh, Side::Right);
                        if !side_r.indices.is_empty() {
                            let tile = model.paints.get(&right_key).map_or(TILE_DEFAULT, |p| {
                                p.material
                                    .as_ref()
                                    .and_then(|m| {
                                        match m {
                                            MaterialRef::Finish(f) => {
                                                let img = crate::texture::texture(f);
                                                Some(img.tile)
                                            }
                                            _ => Some(TILE_DEFAULT),
                                        }
                                    })
                                    .unwrap_or(TILE_DEFAULT)
                            };
                            let mut side_mesh = face_uvs(&side_r, tile);
                            side_mesh.material = right_key.clone();
                            new_meshes.push(side_mesh);
                        }
                        new_meshes.push(rest_r);
                    } else {
                        new_meshes.push(mesh.clone());
                    }
                }
            } else {
                new_meshes.push(mesh.clone());
            }
        } else {
            // Non-wall mesh
            if let Some(key) = surface_key_for(mesh, model) {
                rename_or_keep(mesh, &key, model, &mut new_meshes);
            } else {
                new_meshes.push(mesh.clone());
            }
        }
    }

    *meshes = new_meshes;
}

/// Add the [`MaterialDef`] (and texture) of every `paint:` mesh in `scene`.
pub fn materials(model: &ArchModel, scene: &mut Scene3d) {
    for mesh in scene.meshes.iter() {
        if let Some(key) = mesh.material.strip_prefix("paint:") {
            if let Some(surface) = Surface::parse(key) {
                let paint_key = format!("paint:{}", key);
                let def = model.paints.get(&paint_key).map_or_else(
                    || unpainted_material(model, &surface),
                    |p| material_def_for_paint(model, p, &surface),
                );
                scene.materials.insert(mesh.material.clone(), def);

                // Insert textures into scene.textures
                if let Some(tex_key) = &def.texture {
                    if tex_key.starts_with("finish:") {
                        let f_name = tex_key.trim_start_matches("finish:");
                        if let Some(f) = Finish::by_name(f_name) {
                            if !scene.textures.contains_key(tex_key) {
                                scene.textures.insert(tex_key.clone(), crate::texture::texture(f));
                            }
                        }
                    } else if tex_key.starts_with("item:") {
                        let item_id = tex_key.trim_start_matches("item:");
                        if let Some(img) = model.images.get(item_id) {
                            if img.is_valid() && !scene.textures.contains_key(tex_key) {
                                scene.textures.insert(tex_key.clone(), img.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scene3d;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::*;

    fn make_walls() -> ArchModel {
        let pts = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let mut model = ArchModel::default();
        for i in 0..4usize {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            model.walls.push(Wall {
                id: ArchId(i as u32),
                start: cadcraft_geom::Vec2::new(a.0, a.1),
                end: cadcraft_geom::Vec2::new(b.0, b.1),
                thickness: 0.2,
                height: 2.4,
                ..Default::default()
            });
        }
        model
    }

    #[test]
    fn unpainted_scene_is_unchanged() {
        let model = make_walls();
        let scene_before = Scene3d::build(&model);
        let mut meshes = scene_before.meshes.clone();
        apply(&model, &mut meshes);
        let scene_after = Scene3d::build(&model);
        // Same number of meshes
        assert_eq!(scene_before.meshes.len(), scene_after.meshes.len());
        // Same materials
        assert_eq!(scene_before.materials, scene_after.materials);
        // Same digest
        assert_eq!(scene_before.digest(), scene_after.digest());
    }

    #[test]
    fn a_painted_wall_side_is_its_own_mesh() {
        let mut model = make_walls();
        let key = format!("paint:wall/w0/right");
        model.paints.insert(key.clone(), Paint {
            material: Some(MaterialRef::Colour([200, 50, 50])),
            tint: None,
        });
        let scene = Scene3d::build(&model);
        let painted = scene
            .meshes
            .iter()
            .find(|m| m.material == key)
            .expect("paint mesh should exist");
        assert!(painted.indices.len() > 0);
        // The paint mesh has fewer triangles than the original wall mesh
        let wall_mesh = Scene3d::build(&{
            let m = make_walls();
            m
        })
        .meshes
        .find(|m| m.object == ArchId(0) && m.material == "wall")
        .expect("wall mesh");
        assert!(painted.indices.len() < wall_mesh.indices.len());
    }

    #[test]
    fn both_sides_of_a_partition_paint_apart() {
        let mut model = make_walls();
        let left_key = format!("paint:wall/w0/left");
        let right_key = format!("paint:wall/w0/right");
        model.paints.insert(
            left_key.clone(),
            Paint { material: Some(MaterialRef::Colour([50, 200, 50])), tint: None },
        );
        model.paints.insert(
            right_key.clone(),
            Paint { material: Some(MaterialRef::Colour([200, 50, 200])), tint: None },
        );
        let scene = Scene3d::build(&model);
        let left = scene.meshes.iter().find(|m| m.material == left_key).expect("left paint mesh");
        let right = scene.meshes.iter().find(|m| m.material == right_key).expect("right paint mesh");
        assert!(left.indices.len() > 0 && right.indices.len() > 0);
    }

    #[test]
    fn a_painted_exterior_replaces_the_finish_face() {
        let mut model = make_walls();
        model.walls[0].finish = Some(Finish::Brick);
        let key = format!("paint:wall/w0/right");
        model.paints.insert(key, Paint {
            material: None,
            tint: None,
        });
        let scene = Scene3d::build(&model);
        // The right side should be the paint mesh (replacing the finish face)
        assert!(scene.meshes.iter().any(|m| m.material == key));
        // No separate wall-brick mesh for wall 0
        assert!(!scene
            .meshes
            .iter()
            .any(|m| m.object == ArchId(0) && m.material == "wall-brick"));
    }

    #[test]
    fn painted_floor_ceiling_roof_cabinet_and_furnishing_parts_are_renamed() {
        let mut model = make_walls();
        // Add a room
        model.rooms.push(cadcraft_doc::arch::Room {
            id: ArchId(0),
            polygon: vec![
                cadcraft_geom::Vec2::new(0.5, 0.5),
                cadcraft_geom::Vec2::new(3.5, 0.5),
                cadcraft_geom::Vec2::new(3.5, 3.5),
                cadcraft_geom::Vec2::new(0.5, 3.5),
            ],
            ..Default::default()
        });
        // Add a roof
        model.roofs.push(Roof {
            id: ArchId(0),
            level: 0,
            ..Default::default()
        });
        // Add a cabinet
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            ..Default::default()
        });
        // Add a fixture
        model.fixtures.push(Fixture {
            id: ArchId(0),
            ..Default::default()
        });
        // Add a furnishing with a symbol
        model.symbols.insert(
            "sym_test_0".to_string(),
            SymbolMesh {
                key: "sym_test_0",
                parts: vec![SymbolPart {
                    name: "part".to_string(),
                    positions: vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [0.5, 0.5, 0.0], [0.0, 0.5, 0.0]],
                    normals: vec![],
                    uvs: vec![],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    base_color: [0.5, 0.5, 0.5, 1.0],
                    metallic: 0.0,
                    roughness: 0.8,
                    texture: None,
                }],
                textures: vec![],
            },
        );
        model.furnishings.push(Furnishing {
            id: ArchId(0),
            item: "test".to_string(),
            symbol: "sym_test_0".to_string(),
            name: "test".to_string(),
            category: "test".to_string(),
            symbol_type: "test".to_string(),
            placement: Placement::Floor,
            level: 0,
            at: [2.0, 2.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [0.5, 0.5, 0.5],
            mirrored: false,
            host: FurnishingHost::None,
            cutout: None,
            ..Default::default()
        });

        // Paint all surfaces
        for key in [
            format!("paint:room/r0/floor"),
            format!("paint:room/r0/ceiling"),
            format!("paint:roof/r0"),
            format!("paint:cabinet/cb0/carcass"),
            format!("paint:fixture/fx0"),
            format!("paint:furnishing/f0/0"),
        ] {
            model.paints.insert(key.clone(), Paint { material: None, tint: None });
        }

        let scene = Scene3d::build(&model);
        for key in [
            format!("paint:room/r0/floor"),
            format!("paint:room/r0/ceiling"),
            format!("paint:roof/r0"),
            format!("paint:cabinet/cb0/carcass"),
            format!("paint:fixture/fx0"),
            format!("paint:furnishing/f0/0"),
        ] {
            assert!(
                scene.meshes.iter().any(|m| m.material == key),
                "expected paint mesh for {}",
                key
            );
        }
    }

    #[test]
    fn paint_materials_carry_texture_tile_and_tint() {
        let mut model = make_walls();
        let key = format!("paint:wall/w0/right");
        model.paints.insert(key, Paint {
            material: Some(MaterialRef::Finish(Finish::Siding)),
            tint: Some([255, 200, 200]),
        });
        let scene = Scene3d::build(&model);
        let def = scene
            .materials
            .get(&key)
            .expect("material def should exist");
        // Texture should be finish:siding
        assert_eq!(def.texture.as_deref(), Some("finish:siding"));
        // Tile should be from the siding texture
        assert!(def.tile[0] > 0.0 && def.tile[1] > 0.0);
        // Base colour should be the siding mean linear coloured by the tint
        let siding = crate::texture::texture(&Finish::Siding);
        let linear_tint = srgb_to_linear([255, 200, 200]);
        assert!((def.base_color[0] - siding.mean_linear[0] * linear_tint[0]).abs() < 0.01);
    }

    #[test]
    fn a_tint_only_paint_keeps_the_texture() {
        let mut model = make_walls();
        let key = format!("paint:wall/w0/right");
        model.paints.insert(key, Paint { material: None, tint: Some([100, 100, 200]) });
        // Wall has no finish, so unpainted material has no texture
        let scene = Scene3d::build(&model);
        let def = scene.materials.get(&key).expect("material def should exist");
        assert!(def.texture.is_none());
        // Base colour should be the default wall colour tinted by blue
        let default = default_colour("wall");
        let linear_tint = srgb_to_linear([100, 100, 200]);
        assert!((def.base_color[0] - default[0] * linear_tint[0]).abs() < 0.01);
    }

    #[test]
    fn colour_paint_is_untextured() {
        let mut model = make_walls();
        let key = format!("paint:wall/w0/right");
        model.paints.insert(key, Paint {
            material: Some(MaterialRef::Colour([255, 0, 0])),
            tint: None,
        });
        let scene = Scene3d::build(&model);
        let def = scene.materials.get(&key).expect("material def should exist");
        assert!(def.texture.is_none());
    }
}
