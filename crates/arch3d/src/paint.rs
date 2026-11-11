//! Paint in 3D (phase 14): painted surfaces as their own `paint:<surface key>` meshes and the
//! material definitions they take.
//!
//! [`apply`] splits wall meshes by side when the wall carries a paint and renames every other
//! mesh whose surface has a paint in [`ArchModel::paints`] to `paint:<surface key>`.
//! [`materials`] fills the `MaterialDef` of every `paint:` mesh in the scene, inserting the `finish:`
//! and `item:` textures into `scene.textures` as it goes. Objects without paints are byte-identical
//! to the pre-paint build.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::paint::{CabinetPart, MaterialRef, Paint, Side, Surface};
use cadcraft_doc::arch::{Finish, ImageRgba};

use crate::furnishing::{pull_material, symbol_material};
use crate::texture::{mean_linear, texture};
use crate::uv::face_uvs;
use crate::{MaterialDef, Mesh, Scene3d, finish};

const TILE_DEFAULT: [f32; 2] = [1.0, 1.0];

/// sRGB bytes to linear light.
fn srgb_to_linear(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    })
}

/// The base colour (RGBA, as the GLB writer shows it) of an unpainted `part`: `wall`, `floor`,
/// `ceiling`, `roof`, `cabinet`, `counter`, `pull`, `fixture` or `furnishing`.
pub fn default_colour(part: &str) -> [f32; 4] {
    let rgb = match part {
        "wall" | "floor" | "ceiling" => [0.85, 0.85, 0.85],
        "roof" => [151.0 / 255.0, 84.0 / 255.0, 74.0 / 255.0],
        "cabinet" => [222.0 / 255.0, 184.0 / 255.0, 135.0 / 255.0],
        "counter" => [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0],
        "pull" => [192.0 / 255.0, 192.0 / 255.0, 192.0 / 255.0],
        "fixture" => [240.0 / 255.0, 240.0 / 255.0, 240.0 / 255.0],
        _ => [0.7, 0.7, 0.7],
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// The part name of an unpainted surface, for [`default_colour`].
fn part_name(surface: &Surface) -> &'static str {
    match surface {
        Surface::WallSide { .. } => "wall",
        Surface::RoomFloor(_) => "floor",
        Surface::RoomCeiling(_) => "ceiling",
        Surface::Roof(_) => "roof",
        Surface::Cabinet { part: CabinetPart::Counter, .. } => "counter",
        Surface::Cabinet { part: CabinetPart::Pull, .. } => "pull",
        Surface::Cabinet { .. } => "cabinet",
        Surface::Fixture(_) => "fixture",
        Surface::Furnishing { .. } => "furnishing",
    }
}

/// The definition of built-in finish `f`.
pub(crate) fn finish_def(f: Finish) -> MaterialDef {
    let t = texture(f);
    let m = mean_linear(&t);
    MaterialDef {
        base_color: [m[0], m[1], m[2], 1.0],
        texture: Some(format!("finish:{}", f.name())),
        tile: [t.tile[0] as f32, t.tile[1] as f32],
        normal_texture: Some(format!("finish:{}:normal", f.name())),
        normal_scale: 1.0,
        ..Default::default()
    }
}

/// The definition of a [`MaterialRef`].
pub(crate) fn ref_def(model: &ArchModel, m: &MaterialRef) -> MaterialDef {
    match m {
        MaterialRef::Finish(f) => finish_def(*f),
        MaterialRef::Item(id) => {
            let normal_key = format!("{id}:normal");
            match model.images.get(id).filter(|i| i.is_valid()) {
                Some(img) => {
                    let c = srgb_to_linear(img.mean_srgb());
                    MaterialDef {
                        base_color: [c[0], c[1], c[2], 1.0],
                        texture: Some(format!("item:{id}")),
                        normal_texture: model.images.get(&normal_key).filter(|i| i.is_valid()).map(|_| normal_key.clone()),
                        tile: TILE_DEFAULT,
                        ..Default::default()
                    }
                }
                None => MaterialDef { base_color: [0.5, 0.5, 0.5, 1.0], ..Default::default() },
            }
        }
        MaterialRef::Colour(c) => {
            let l = srgb_to_linear(*c);
            MaterialDef { base_color: [l[0], l[1], l[2], 1.0], ..Default::default() }
        }
    }
}

/// The material a surface shows unpainted. Symbol parts need `scene` (their definitions are
/// already in it); without it they fall back to the default colour.
fn unpainted_def(model: &ArchModel, scene: Option<&Scene3d>, surface: &Surface) -> MaterialDef {
    let finish_of = |f: Option<Finish>, ok: fn(Finish) -> bool| f.filter(|f| ok(*f)).map(finish_def);
    let from_scene = |name: String| scene.and_then(|s| s.materials.get(&name)).cloned();
    let found = match surface {
        Surface::WallSide { wall, side: Side::Right } => {
            model.walls.iter().find(|w| w.id == *wall).and_then(|w| finish_of(w.finish, Finish::for_walls))
        }
        Surface::RoomFloor(r) => model.rooms.iter().find(|x| x.id == *r).and_then(|x| finish_of(x.floor_finish, Finish::for_floors)),
        Surface::Roof(r) => model.roofs.iter().find(|x| x.id == *r).and_then(|x| finish_of(x.covering, Finish::for_roofs)),
        Surface::Furnishing { furnishing, part } => model.furnishings.iter().find(|f| f.id == *furnishing).and_then(|f| {
            let part = usize::try_from(*part).ok()?;
            from_scene(symbol_material(&f.symbol, part))
        }),
        Surface::Cabinet { cabinet, part: CabinetPart::Pull } => {
            model.cabinets.iter().find(|c| c.id == *cabinet).and_then(|c| c.pull_symbol.as_deref()).and_then(|key| from_scene(pull_material(key, 0)))
        }
        _ => None,
    };
    found.unwrap_or_else(|| MaterialDef { base_color: default_colour(part_name(surface)), ..Default::default() })
}

/// The tile (metres per repeat) that face-aligned UVs on `surface` take.
fn tile_of(model: &ArchModel, paint: &Paint, surface: &Surface) -> [f64; 2] {
    let def = match &paint.material {
        Some(m) => ref_def(model, m),
        None => unpainted_def(model, None, surface),
    };
    [f64::from(def.tile[0]), f64::from(def.tile[1])]
}

/// Split `side` of `wall` out of `mesh` when that side is painted, pushing the paint mesh to
/// `out`; returns the rest.
fn paint_side(model: &ArchModel, wall: &cadcraft_doc::arch::Wall, mesh: Mesh, side: Side, out: &mut Vec<Mesh>) -> Mesh {
    let surface = Surface::WallSide { wall: wall.id, side };
    let Some(paint) = model.paints.get(&surface.key()) else {
        return mesh;
    };
    let (part, rest) = finish::split_side(wall, &mesh, side);
    if part.indices.is_empty() || rest.indices.len() == mesh.indices.len() {
        return mesh;
    }
    let mut painted = face_uvs(&part, tile_of(model, paint, &surface));
    painted.material = format!("paint:{}", surface.key());
    out.push(painted);
    rest
}

/// The surface a non-wall mesh shows.
fn surface_of(mesh: &Mesh) -> Option<Surface> {
    let id = mesh.object;
    let m = mesh.material.as_str();
    let kind = |p: &str| m == p || m.strip_prefix(p).is_some_and(|r| r.starts_with('-'));
    if kind("floor") || m == "slab" {
        Some(Surface::RoomFloor(id))
    } else if m == "ceiling" {
        Some(Surface::RoomCeiling(id))
    } else if kind("roof") {
        Some(Surface::Roof(id))
    } else if m == "cabinet-box" {
        Some(Surface::Cabinet { cabinet: id, part: CabinetPart::Carcass })
    } else if m == "cabinet-front" {
        Some(Surface::Cabinet { cabinet: id, part: CabinetPart::Front })
    } else if m == "counter" {
        Some(Surface::Cabinet { cabinet: id, part: CabinetPart::Counter })
    } else if m.starts_with("pull:") {
        Some(Surface::Cabinet { cabinet: id, part: CabinetPart::Pull })
    } else if m.starts_with("fixture-") {
        Some(Surface::Fixture(id))
    } else if let Some(rest) = m.strip_prefix("sym:") {
        let part = rest.rsplit(':').next()?.parse::<u32>().ok()?;
        Some(Surface::Furnishing { furnishing: id, part })
    } else {
        None
    }
}

/// Split out or rename the painted surfaces among one part's `meshes` as `paint:<surface key>`
/// meshes. Meshes without a paint are untouched.
pub fn apply(model: &ArchModel, meshes: &mut Vec<Mesh>) {
    if model.paints.is_empty() {
        return;
    }
    let mut out: Vec<Mesh> = Vec::with_capacity(meshes.len());
    for mut mesh in std::mem::take(meshes) {
        let wall = model.walls.iter().find(|w| w.id == mesh.object);
        match wall {
            Some(wall) if mesh.material == "wall" => {
                let rest = paint_side(model, wall, mesh, Side::Left, &mut out);
                let rest = paint_side(model, wall, rest, Side::Right, &mut out);
                out.push(rest);
            }
            Some(wall) if mesh.material.starts_with("wall-") => {
                // The finish's exterior face (the right side): the paint replaces it.
                let key = Surface::WallSide { wall: wall.id, side: Side::Right }.key();
                if model.paints.contains_key(&key) {
                    mesh.material = format!("paint:{key}");
                }
                out.push(mesh);
            }
            _ => {
                if let Some(surface) = surface_of(&mesh) {
                    let key = surface.key();
                    if model.paints.contains_key(&key) {
                        mesh.material = format!("paint:{key}");
                    }
                }
                out.push(mesh);
            }
        }
    }
    *meshes = out;
}

/// Add the [`MaterialDef`] (and texture) of every `paint:` mesh in `scene`.
pub fn materials(model: &ArchModel, scene: &mut Scene3d) {
    let mut names: Vec<String> = scene.meshes.iter().filter(|m| m.material.starts_with("paint:")).map(|m| m.material.clone()).collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let Some(surface) = name.strip_prefix("paint:").and_then(Surface::parse) else {
            continue;
        };
        let Some(paint) = model.paints.get(&surface.key()) else {
            continue;
        };
        let mut def = match &paint.material {
            Some(m) => ref_def(model, m),
            None => unpainted_def(model, Some(scene), &surface),
        };
        if let Some(t) = paint.tint {
            let l = srgb_to_linear(t);
            def.base_color = [def.base_color[0] * l[0], def.base_color[1] * l[1], def.base_color[2] * l[2], def.base_color[3]];
            def.tint = Some(t);
        }
        if let Some(key) = def.texture.clone()
            && !scene.textures.contains_key(&key)
        {
            if let Some(f) = key.strip_prefix("finish:").and_then(Finish::parse) {
                let t = texture(f);
                let mut img = ImageRgba::default();
                img.width = t.width;
                img.height = t.height;
                img.rgba = t.rgba;
                scene.textures.insert(key, img);
            } else if let Some(img) = key.strip_prefix("item:").and_then(|id| model.images.get(id)) {
                scene.textures.insert(key, img.clone());
            }
        }
        scene.materials.insert(name, def);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Cabinet, Fixture, Furnishing, Roof, RoofPlane, Room, SymbolMesh, SymbolPart, Wall};
    use cadcraft_geom::Vec2;

    fn ring(finish: Option<Finish>) -> ArchModel {
        let pts = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
        let mut model = ArchModel::default();
        for i in 0..4usize {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            model.walls.push(Wall {
                id: ArchId(i as u32),
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                thickness: 0.2,
                height: 2.4,
                finish,
                ..Default::default()
            });
        }
        model
    }

    fn paint(model: &mut ArchModel, key: &str, material: Option<MaterialRef>, tint: Option<[u8; 3]>) {
        let mut p = Paint::default();
        p.material = material;
        p.tint = tint;
        model.paints.insert(key.to_string(), p);
    }

    #[test]
    fn unpainted_scene_is_unchanged() {
        let model = ring(None);
        let scene = Scene3d::build(&model);
        let mut meshes = scene.meshes.clone();
        apply(&model, &mut meshes);
        assert_eq!(meshes.len(), scene.meshes.len());
        for (a, b) in meshes.iter().zip(&scene.meshes) {
            assert_eq!((&a.material, &a.indices, &a.positions), (&b.material, &b.indices, &b.positions));
        }
        assert!(scene.meshes.iter().all(|m| m.material == "wall"));
        assert!(scene.materials.is_empty());
    }

    #[test]
    fn a_painted_wall_side_is_its_own_mesh() {
        let plain = Scene3d::build(&ring(None));
        let whole: usize = plain.meshes.iter().filter(|m| m.object == ArchId(0)).map(|m| m.indices.len()).sum();
        let mut model = ring(None);
        paint(&mut model, "wall/w0/right", Some(MaterialRef::Colour([200, 50, 50])), None);
        let scene = Scene3d::build(&model);
        let painted = scene.meshes.iter().find(|m| m.material == "paint:wall/w0/right").expect("paint mesh");
        assert!(!painted.indices.is_empty());
        assert_eq!(painted.uvs.len(), painted.positions.len());
        let total: usize = scene.meshes.iter().filter(|m| m.object == ArchId(0)).map(|m| m.indices.len()).sum();
        assert_eq!(total, whole);
        assert!(scene.meshes.iter().any(|m| m.object == ArchId(0) && m.material == "wall"));
        assert!(scene.meshes.iter().all(|m| crate::probe::part_of(m) == "wall" || m.object != ArchId(0)));
    }

    #[test]
    fn both_sides_of_a_partition_paint_apart() {
        let mut model = ring(None);
        paint(&mut model, "wall/w0/left", Some(MaterialRef::Colour([50, 200, 50])), None);
        paint(&mut model, "wall/w0/right", Some(MaterialRef::Colour([200, 50, 200])), None);
        let scene = Scene3d::build(&model);
        let left = scene.meshes.iter().find(|m| m.material == "paint:wall/w0/left").expect("left");
        let right = scene.meshes.iter().find(|m| m.material == "paint:wall/w0/right").expect("right");
        assert!(!left.indices.is_empty() && !right.indices.is_empty());
        // Opposite sides: the normals point opposite ways in y (the wall runs along +x).
        let ny = |m: &Mesh| m.normals.first().map_or(0.0, |n| n[1]);
        assert!(ny(left) * ny(right) < 0.0);
    }

    #[test]
    fn a_painted_exterior_replaces_the_finish_face() {
        let mut model = ring(Some(Finish::Brick));
        paint(&mut model, "wall/w0/right", None, None);
        let scene = Scene3d::build(&model);
        assert!(scene.meshes.iter().any(|m| m.object == ArchId(0) && m.material == "paint:wall/w0/right"));
        assert!(!scene.meshes.iter().any(|m| m.object == ArchId(0) && m.material == "wall-brick"));
        assert!(scene.meshes.iter().any(|m| m.object == ArchId(1) && m.material == "wall-brick"));
        // Unpainted material of a finished exterior is the finish's.
        let def = scene.materials.get("paint:wall/w0/right").expect("def");
        assert_eq!(def.texture.as_deref(), Some("finish:brick"));
        assert!(scene.textures.contains_key("finish:brick"));
    }

    #[test]
    fn painted_floor_ceiling_roof_cabinet_and_furnishing_parts_are_renamed() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(0),
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            ..Default::default()
        });
        let plane = RoofPlane {
            ring: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 3.0), Vec2::new(0.0, 3.0)],
            slope: 0.5,
            thickness: 0.2,
            eave_z: 2.4,
            fascia_depth: 0.1,
            soffit_depth: 0.1,
            base_edge: [0.0, 0.0, 6.0, 0.0],
            ..Default::default()
        };
        model.roofs.push(Roof { id: ArchId(0), planes: vec![plane], ..Default::default() });
        model.cabinets.push(Cabinet { id: ArchId(0), ..Default::default() });
        model.fixtures.push(Fixture { id: ArchId(0), ..Default::default() });
        let part = SymbolPart {
            name: "part".to_string(),
            positions: vec![[-0.25, -0.25, 0.0], [0.25, -0.25, 0.0], [0.25, 0.25, 0.5], [-0.25, 0.25, 0.5]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            base_color: [0.5, 0.5, 0.5, 1.0],
            metallic: 0.0,
            roughness: 0.8,
            ..Default::default()
        };
        let mut sym = SymbolMesh::default();
        sym.key = "0123456789abcdef".to_string();
        sym.parts = vec![part];
        model.symbols.insert("0123456789abcdef".to_string(), sym);
        model.furnishings.push(Furnishing {
            id: ArchId(0),
            item: "test".to_string(),
            symbol: "0123456789abcdef".to_string(),
            at: [2.0, 1.5],
            size: [0.5, 0.5, 0.5],
            ..Default::default()
        });
        let keys = ["room/r0/floor", "room/r0/ceiling", "roof/r0", "cabinet/cb0/carcass", "fixture/fx0", "furnishing/f0/0"];
        let before = Scene3d::build(&model);
        for k in keys {
            paint(&mut model, k, None, None);
        }
        let scene = Scene3d::build(&model);
        for k in keys {
            let name = format!("paint:{k}");
            let had = before.meshes.iter().any(|m| surface_of(m).is_some_and(|s| s.key() == k));
            assert_eq!(scene.meshes.iter().any(|m| m.material == name), had, "{name}");
        }
        assert!(before.meshes.iter().any(|m| surface_of(m).is_some_and(|s| s.key() == "furnishing/f0/0")));
        assert!(before.meshes.iter().any(|m| surface_of(m).is_some_and(|s| s.key() == "room/r0/floor")));
        assert!(before.meshes.iter().any(|m| surface_of(m).is_some_and(|s| s.key() == "roof/r0")));
        // Same meshes, only renamed.
        assert_eq!(scene.meshes.len(), before.meshes.len());
        // The furnishing part keeps its symbol material.
        let def = scene.materials.get("paint:furnishing/f0/0").expect("def");
        assert_eq!(def.base_color, [0.5, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn paint_materials_carry_texture_tile_and_tint() {
        let mut model = ring(None);
        paint(&mut model, "wall/w0/right", Some(MaterialRef::Finish(Finish::Siding)), Some([255, 200, 200]));
        let scene = Scene3d::build(&model);
        let def = scene.materials.get("paint:wall/w0/right").expect("def");
        assert_eq!(def.texture.as_deref(), Some("finish:siding"));
        assert_eq!(def.tint, Some([255, 200, 200]));
        let t = texture(Finish::Siding);
        assert_eq!(def.tile, [t.tile[0] as f32, t.tile[1] as f32]);
        let (m, l) = (mean_linear(&t), srgb_to_linear([255, 200, 200]));
        for c in 0..3 {
            assert!((def.base_color[c] - m[c] * l[c]).abs() < 1e-6);
        }
        assert!(scene.textures.contains_key("finish:siding"));
    }

    #[test]
    fn a_tint_only_paint_keeps_the_texture() {
        let mut model = ring(Some(Finish::Brick));
        paint(&mut model, "wall/w0/right", None, Some([100, 100, 200]));
        let scene = Scene3d::build(&model);
        let def = scene.materials.get("paint:wall/w0/right").expect("def");
        assert_eq!(def.texture.as_deref(), Some("finish:brick"));
        assert_eq!(def.tint, Some([100, 100, 200]));
        let m = mean_linear(&texture(Finish::Brick));
        assert!((def.base_color[0] - m[0] * srgb_to_linear([100, 100, 200])[0]).abs() < 1e-6);
        // Without a finish the tint multiplies the default wall colour.
        let mut bare = ring(None);
        paint(&mut bare, "wall/w0/left", None, Some([100, 100, 200]));
        let scene = Scene3d::build(&bare);
        let def = scene.materials.get("paint:wall/w0/left").expect("def");
        assert!(def.texture.is_none());
        assert!((def.base_color[0] - default_colour("wall")[0] * srgb_to_linear([100, 100, 200])[0]).abs() < 1e-6);
    }

    #[test]
    fn colour_paint_is_untextured() {
        let mut model = ring(Some(Finish::Brick));
        paint(&mut model, "wall/w0/right", Some(MaterialRef::Colour([255, 0, 0])), None);
        let scene = Scene3d::build(&model);
        let def = scene.materials.get("paint:wall/w0/right").expect("def");
        assert!(def.texture.is_none());
        assert_eq!(def.base_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(def.tile, [1.0, 1.0]);
    }
}
