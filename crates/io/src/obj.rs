//! Wavefront OBJ + MTL writer for CADCraft arch3d scenes.
//!
//! Objects carry the same names as the GLB's nodes ([`crate::glb::node_names`]) and vertices are in
//! glTF's Y-up axes, `(x, y, z)` -> `(x, z, -y)`, as the GLB root rotation shows them. Meshes
//! without vertices or triangles are left out of both files.

use std::collections::BTreeMap;
use std::fmt::Write;

use cadcraft_arch3d::{MaterialDef, Mesh, Scene3d, texture};

use crate::glb;

/// A number with six decimals, trailing zeros trimmed (`-0` and non-finite values become `0`).
fn num(v: f32) -> String {
    let mut s = if v.is_finite() { format!("{v:.6}") } else { "0.000000".to_string() };
    if s.contains('.') {
        s.truncate(s.trim_end_matches('0').trim_end_matches('.').len());
    }
    if s == "-0" { "0".to_string() } else { s }
}

/// The meshes both files write, with their object names.
fn written(scene: &Scene3d) -> Vec<(String, &Mesh)> {
    let names = glb::node_names(scene);
    scene.meshes.iter().filter(|m| !m.positions.is_empty()).zip(names).filter(|(m, _)| m.indices.len() >= 3).map(|(m, n)| (n, m)).collect()
}

/// The OBJ text for `scene`, referencing the material library `mtl_name`. Each mesh is its own
/// `o` group, except that every mesh of one furnishing (`furnishing-f<n>`) or of one cabinet's pulls
/// (`cabinet-cb<n>-pull`), the objects the GLB writes as one node of several primitives, is one
/// group at its first mesh's place, with a `usemtl` per part.
pub fn write_obj(scene: &Scene3d, mtl_name: &str) -> String {
    let mut out = String::from("# CADCraft arch3d\n");
    let _ = writeln!(out, "mtllib {mtl_name}");
    let mut base: usize = 0;
    for (name, meshes) in groups(written(scene)) {
        let _ = writeln!(out, "o {name}");
        for m in meshes {
            write_mesh(&mut out, m, base);
            base = base.saturating_add(m.positions.len());
        }
    }
    out
}

/// Whether the meshes named `name` are one object node of several primitives in the GLB.
fn one_node(name: &str) -> bool {
    name.starts_with("furnishing-") || (name.starts_with("cabinet-") && name.ends_with("-pull"))
}

/// The written meshes as OBJ groups, in scene order: one per mesh, or one per [`one_node`] name
/// holding all its meshes.
fn groups(written: Vec<(String, &Mesh)>) -> Vec<(String, Vec<&Mesh>)> {
    let mut out: Vec<(String, Vec<&Mesh>)> = Vec::new();
    let mut slot: BTreeMap<String, usize> = BTreeMap::new();
    for (name, m) in written {
        if one_node(&name) {
            if let Some(group) = slot.get(&name).and_then(|&i| out.get_mut(i)) {
                group.1.push(m);
                continue;
            }
            slot.insert(name.clone(), out.len());
        }
        out.push((name, vec![m]));
    }
    out
}

/// One mesh's `usemtl`, vertices, normals and faces; its vertices are numbered after `base`.
fn write_mesh(out: &mut String, m: &Mesh, base: usize) {
    let _ = writeln!(out, "usemtl {}", m.material);
    for p in &m.positions {
        let _ = writeln!(out, "v {} {} {}", num(p[0]), num(p[2]), num(-p[1]));
    }
    for n in m.positions.iter().enumerate().map(|(i, _)| m.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])) {
        let _ = writeln!(out, "vn {} {} {}", num(n[0]), num(n[2]), num(-n[1]));
    }
    for t in m.indices.chunks_exact(3) {
        let ix: Vec<usize> =
            t.iter().map(|&i| (i as usize).min(m.positions.len().saturating_sub(1)).saturating_add(base).saturating_add(1)).collect();
        if let [a, b, c] = ix[..] {
            let _ = writeln!(out, "f {a}//{a} {b}//{b} {c}//{c}");
        }
    }
}

/// The MTL text for `scene`: one material per distinct mesh material, in first-use order. A
/// material defined in `scene.materials` (phase 14: symbol parts, paints) shows its base colour as
/// sRGB, or, when textured, its texture's mean colour times its tint; a finish its texture's mean
/// colour; anything else its phase-12 colour. No texture maps.
pub fn write_mtl(scene: &Scene3d) -> String {
    let mut out = String::from("# CADCraft arch3d\n");
    let mut seen: Vec<&str> = Vec::new();
    for (_, m) in written(scene) {
        if seen.contains(&m.material.as_str()) {
            continue;
        }
        seen.push(m.material.as_str());
        let c = match (scene.materials.get(&m.material), glb::finish_of(&m.material)) {
            (Some(def), _) => defined_rgba(scene, def),
            (None, Some(f)) => {
                let [r, g, b] = texture::mean_srgb(&texture::texture(f));
                [f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0, 1.0]
            }
            (None, None) => glb::material_rgba(&m.material),
        };
        let _ = writeln!(out, "newmtl {}", m.material);
        let _ = writeln!(out, "Kd {} {} {}", num(c[0] as f32), num(c[1] as f32), num(c[2] as f32));
        let _ = writeln!(out, "d {}", num(c[3] as f32));
        let _ = writeln!(out, "illum 2");
    }
    out
}

/// The sRGB colour (0..=1) and alpha of a [`MaterialDef`]: its texture's mean colour times its
/// tint when its texture is in the scene, else its linear base colour (the tint already in it) as
/// sRGB.
fn defined_rgba(scene: &Scene3d, def: &MaterialDef) -> [f64; 4] {
    let alpha = unit(f64::from(def.base_color[3]));
    if let Some(image) = def.texture.as_ref().and_then(|key| scene.textures.get(key)) {
        let mean = image.mean_srgb();
        let tint = def.tint.unwrap_or([255; 3]);
        let c = |k: usize| f64::from(mean[k]) / 255.0 * (f64::from(tint[k]) / 255.0);
        return [c(0), c(1), c(2), alpha];
    }
    let c = |k: usize| linear_to_srgb(f64::from(def.base_color[k]));
    [c(0), c(1), c(2), alpha]
}

/// `v` clamped to 0..=1 (0 when it is not a number).
fn unit(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// A linear colour component (0..=1) as sRGB (IEC 61966-2-1).
fn linear_to_srgb(v: f64) -> f64 {
    let v = unit(v);
    if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;

    fn mesh(material: &str, id: u32, tris: bool) -> Mesh {
        Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 3.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: if tris { vec![0, 1, 2] } else { Vec::new() },
            material: material.into(),
            object: ArchId(id),
            level_elevation: 0.0,
            ..Mesh::default()
        }
    }

    fn scene() -> Scene3d {
        let mut s = Scene3d::default();
        s.meshes = vec![
            mesh("wall", 1, true),
            mesh("roof", 0, true),
            mesh("roof", 0, true),
            mesh("window-glass", 2, true),
            mesh("wall", 3, false),
            mesh("wall", 4, true),
        ];
        s
    }

    #[test]
    fn obj_groups_are_the_glb_node_names() {
        let s = scene();
        let obj = write_obj(&s, "x.mtl");
        let groups: Vec<&str> = obj.lines().filter_map(|l| l.strip_prefix("o ")).collect();
        assert_eq!(groups, ["wall-w1", "roof-r0p0", "roof-r0p1", "window-glass-o2", "wall-w4"]);
        assert!(obj.starts_with("# CADCraft arch3d\nmtllib x.mtl\n"));
    }

    #[test]
    fn every_usemtl_is_defined_in_the_mtl() {
        let s = scene();
        let obj = write_obj(&s, "x.mtl");
        let mtl = write_mtl(&s);
        let defined: Vec<&str> = mtl.lines().filter_map(|l| l.strip_prefix("newmtl ")).collect();
        assert_eq!(defined, ["wall", "roof", "window-glass"]);
        for u in obj.lines().filter_map(|l| l.strip_prefix("usemtl ")) {
            assert!(defined.contains(&u), "{u} undefined");
        }
        assert!(mtl.contains("d 0.35\n"));
    }

    #[test]
    fn obj_faces_index_existing_vertices() {
        let s = scene();
        let obj = write_obj(&s, "x.mtl");
        let v = obj.lines().filter(|l| l.starts_with("v ")).count();
        let vn = obj.lines().filter(|l| l.starts_with("vn ")).count();
        assert_eq!(v, vn);
        assert_eq!(v, 15);
        let mut faces = 0;
        for l in obj.lines().filter_map(|l| l.strip_prefix("f ")) {
            for p in l.split(' ') {
                let (a, b) = p.split_once("//").expect("v//vn");
                let a: usize = a.parse().expect("index");
                assert_eq!(a.to_string(), b);
                assert!((1..=v).contains(&a));
            }
            faces += 1;
        }
        assert_eq!(faces, 5);
        assert!(obj.contains("v 0 3 -2\n"), "Y-up axes");
    }

    #[test]
    fn obj_mtl_gives_a_finished_material_its_mean_colour() {
        let mut s = Scene3d::default();
        s.meshes = vec![mesh("wall-brick", 1, true), mesh("wall", 1, true)];
        let obj = write_obj(&s, "x.mtl");
        assert!(obj.contains("o wall-w1-exterior\nusemtl wall-brick\n"));
        let mtl = write_mtl(&s);
        assert!(!mtl.contains("map_Kd"));
        let [r, g, b] = texture::mean_srgb(&texture::texture(cadcraft_doc::arch::Finish::Brick));
        let want = format!("newmtl wall-brick\nKd {} {} {}\n", num(f32::from(r) / 255.0), num(f32::from(g) / 255.0), num(f32::from(b) / 255.0));
        assert!(mtl.contains(&want), "{mtl}");
    }

    #[test]
    fn obj_export_writes_its_mtl_beside() {
        let d = cadcraft_doc::Drawing::default();
        let obj = crate::write_framed(&d, "house.obj", None).expect("obj");
        let mtl = crate::write_framed(&d, "house.mtl", None).expect("mtl");
        assert!(String::from_utf8(obj).expect("utf8").contains("mtllib house.mtl\n"));
        assert!(String::from_utf8(mtl).expect("utf8").starts_with("# CADCraft arch3d"));
    }

    #[test]
    fn obj_finished_face_uses_exterior_node_name() {
        let mut s = Scene3d::default();
        s.meshes = vec![mesh("wall-brick", 1, true), mesh("wall", 1, true)];
        let obj = write_obj(&s, "x.mtl");
        let groups: Vec<&str> = obj.lines().filter_map(|l| l.strip_prefix("o ")).collect();
        assert_eq!(groups, ["wall-w1-exterior", "wall-w1"]);
    }

    #[test]
    fn obj_empty_scene_is_valid() {
        let s = Scene3d::default();
        let obj = write_obj(&s, "x.mtl");
        assert!(obj.starts_with("# CADCraft arch3d\nmtllib x.mtl\n"));
        assert!(!obj.contains("o "));
        assert!(!obj.contains("v "));
        assert!(!obj.contains("f "));
    }

    const KEY: &str = "00112233aabbccdd";

    /// A model with a two-part symbol (part 0 a plain colour, part 1 textured) and two furnishings
    /// of it, f0 and f1.
    fn furnished() -> cadcraft_doc::arch::ArchModel {
        use cadcraft_doc::arch::{ArchModel, Furnishing, ImageRgba, SymbolMesh, SymbolPart};
        let tri = |z: f32| SymbolPart {
            positions: vec![[-0.5, -0.5, z], [0.5, -0.5, z], [0.0, 0.5, z + 0.5]],
            normals: vec![[0.0, -1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        let plain = SymbolPart { base_color: [0.5, 0.25, 0.125, 1.0], ..tri(0.0) };
        let textured = SymbolPart { uvs: vec![[0.0, 0.0]; 3], texture: Some(0), base_color: [1.0, 1.0, 1.0, 0.5], ..tri(0.5) };
        // Mean colour (150, 75, 100).
        let mut image = ImageRgba { width: 2, height: 1, ..Default::default() };
        image.rgba = vec![200, 100, 50, 255, 100, 50, 150, 255];
        let mut m = ArchModel::default();
        m.symbols.insert(KEY.into(), SymbolMesh { key: KEY.into(), parts: vec![plain, textured], textures: vec![image], ..Default::default() });
        for k in 0..2u32 {
            let at = [f64::from(k) * 2.0, 1.0];
            m.furnishings.push(Furnishing { id: ArchId(k), symbol: KEY.into(), at, size: [1.0, 1.0, 1.0], ..Default::default() });
        }
        m
    }

    /// The OBJ's groups in order, each with its `usemtl` lines.
    fn groups_of(obj: &str) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        for l in obj.lines() {
            if let Some(g) = l.strip_prefix("o ") {
                out.push((g.to_string(), Vec::new()));
            } else if let (Some(u), Some(last)) = (l.strip_prefix("usemtl "), out.last_mut()) {
                last.1.push(u.to_string());
            }
        }
        out
    }

    #[test]
    fn obj_groups_furnishings_by_node_name() {
        let model = furnished();
        let mut s = Scene3d::build(&model);
        // A wall between f0's two parts: a furnishing is still one group, whatever the mesh order.
        s.meshes.insert(1, mesh("wall", 7, true));
        let obj = write_obj(&s, "x.mtl");
        let (p0, p1) = (format!("sym:{KEY}:0"), format!("sym:{KEY}:1"));
        let want = [
            ("furnishing-f0".to_string(), vec![p0.clone(), p1.clone()]),
            ("wall-w7".to_string(), vec!["wall".to_string()]),
            ("furnishing-f1".to_string(), vec![p0, p1]),
        ];
        assert_eq!(groups_of(&obj), want, "{obj}");
        // The groups are the GLB's node names, and every face indexes a vertex written before it.
        let names: std::collections::BTreeSet<String> = glb::node_names(&s).into_iter().collect();
        assert_eq!(groups_of(&obj).into_iter().map(|g| g.0).collect::<std::collections::BTreeSet<_>>(), names);
        let mut seen = 0usize;
        for l in obj.lines() {
            if l.starts_with("v ") {
                seen += 1;
            } else if let Some(f) = l.strip_prefix("f ") {
                for p in f.split(' ') {
                    let a: usize = p.split_once("//").and_then(|(a, _)| a.parse().ok()).unwrap_or(0);
                    assert!((1..=seen).contains(&a), "{l} after {seen} vertices");
                }
            }
        }
        assert_eq!(seen, 15, "four furnishing parts and the wall");
        // Each part's vertices follow its own usemtl: f0's second part is the triangle at z 0.5..1
        // (plan (-0.5, 0.5, 0.5) is OBJ (-0.5, 0.5, -0.5) in Y-up axes).
        let f0 = obj.split("o furnishing-f0\n").nth(1).unwrap_or_default();
        let second = f0.split(&format!("usemtl sym:{KEY}:1\n")).nth(1).unwrap_or_default();
        assert!(second.starts_with("v -0.5 0.5 -0.5\n"), "{second}");
    }

    #[test]
    fn obj_mtl_has_part_colours() {
        let model = furnished();
        let mut s = Scene3d::build(&model);
        // A paint's tint (the textured part as p14-arch3d-paint will give it).
        if let Some(def) = s.materials.get_mut(&format!("sym:{KEY}:1")) {
            def.tint = Some([255, 128, 0]);
        }
        let obj = write_obj(&s, "x.mtl");
        let mtl = write_mtl(&s);
        let defined: Vec<&str> = mtl.lines().filter_map(|l| l.strip_prefix("newmtl ")).collect();
        let (p0, p1) = (format!("sym:{KEY}:0"), format!("sym:{KEY}:1"));
        assert_eq!(defined, [p0.as_str(), p1.as_str()], "one material per part, however many furnishings use it");
        for u in obj.lines().filter_map(|l| l.strip_prefix("usemtl ")) {
            assert!(defined.contains(&u), "{u} undefined");
        }
        // The plain part: its linear base colour as sRGB.
        let srgb = |c: f64| if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        let kd = |c: [f64; 3]| format!("Kd {} {} {}", num(c[0] as f32), num(c[1] as f32), num(c[2] as f32));
        let plain = format!("newmtl {p0}\n{}\nd 1\n", kd([srgb(0.5), srgb(0.25), srgb(0.125)]));
        assert!(mtl.contains(&plain), "{mtl}");
        assert!(mtl.contains(&format!("newmtl {p0}\nKd 0.735357 0.537099 0.388573\n")), "{mtl}");
        // The textured part: its texture's mean colour (150, 75, 100) times the tint, its alpha as d.
        let mean_tint = |m: f64, t: f64| m / 255.0 * (t / 255.0);
        let textured = format!("newmtl {p1}\n{}\nd 0.5\n", kd([mean_tint(150.0, 255.0), mean_tint(75.0, 128.0), mean_tint(100.0, 0.0)]));
        assert!(mtl.contains(&textured), "{mtl}");
        assert!(!mtl.contains("map_Kd"), "no texture maps, as in phase 13");
    }
}
