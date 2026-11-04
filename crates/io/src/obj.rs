//! Wavefront OBJ + MTL writer for CADCraft arch3d scenes.
//!
//! Objects carry the same names as the GLB's nodes ([`crate::glb::node_names`]) and vertices are in
//! glTF's Y-up axes, `(x, y, z)` -> `(x, z, -y)`, as the GLB root rotation shows them. Meshes
//! without vertices or triangles are left out of both files.

use std::fmt::Write;

use cadcraft_arch3d::{Mesh, Scene3d, texture};

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

/// The OBJ text for `scene`, referencing the material library `mtl_name`.
pub fn write_obj(scene: &Scene3d, mtl_name: &str) -> String {
    let mut out = String::from("# CADCraft arch3d\n");
    let _ = writeln!(out, "mtllib {mtl_name}");
    let mut base: usize = 0;
    for (name, m) in written(scene) {
        let _ = writeln!(out, "o {name}");
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
        base = base.saturating_add(m.positions.len());
    }
    out
}

/// The MTL text for `scene`: one material per distinct mesh material, in first-use order.
pub fn write_mtl(scene: &Scene3d) -> String {
    let mut out = String::from("# CADCraft arch3d\n");
    let mut seen: Vec<&str> = Vec::new();
    for (_, m) in written(scene) {
        if seen.contains(&m.material.as_str()) {
            continue;
        }
        seen.push(m.material.as_str());

        let c = if let Some(def) = scene.materials.get(&m.material) {
            // Use MaterialDef
            let mut r = def.base_color[0];
            let mut g = def.base_color[1];
            let mut b = def.base_color[2];
            let a = def.base_color[3];

            // Apply tint if present
            if let Some([tr, tg, tb]) = def.tint {
                r *= f32::from(tr) / 255.0;
                g *= f32::from(tg) / 255.0;
                b *= f32::from(tb) / 255.0;
            }

            // If textured, use mean color of texture
            if let Some(tex_key) = &def.texture {
                if let Some(img) = scene.textures.get(tex_key) {
                    let [mr, mg, mb] = img.mean_srgb();
                    r = f32::from(mr) / 255.0;
                    g = f32::from(mg) / 255.0;
                    b = f32::from(mb) / 255.0;
                    // Apply tint to texture mean
                    if let Some([tr, tg, tb]) = def.tint {
                        r *= f32::from(tr) / 255.0;
                        g *= f32::from(tg) / 255.0;
                        b *= f32::from(tb) / 255.0;
                    }
                }
            }

            [r as f64, g as f64, b as f64, a as f64]
        } else {
            match glb::finish_of(&m.material) {
                Some(f) => {
                    let [r, g, b] = texture::mean_srgb(&texture::texture(f));
                    [f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0, 1.0]
                }
                None => glb::material_rgba(&m.material),
            }
        };

        let _ = writeln!(out, "newmtl {}", m.material);
        let _ = writeln!(out, "Kd {} {} {}", num(c[0] as f32), num(c[1] as f32), num(c[2] as f32));
        let _ = writeln!(out, "d {}", num(c[3] as f32));
        let _ = writeln!(out, "illum 2");
    }
    out
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

    #[test]
    fn obj_groups_furnishings_by_node_name() {
        let mut s = Scene3d::default();
        // Add a furnishing mesh
        let m = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            material: "sym:key1:0".into(),
            object: ArchId(5),
            level_elevation: 0.0,
            ..Mesh::default()
        };
        s.meshes.push(m);

        let obj = write_obj(&s, "x.mtl");
        let groups: Vec<&str> = obj.lines().filter_map(|l| l.strip_prefix("o ")).collect();
        // glb::node_names should produce "furnishing-f5"
        assert_eq!(groups, ["furnishing-f5"]);
    }

    #[test]
    fn obj_mtl_has_part_colours() {
        let mut s = Scene3d::default();
        let m = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            material: "sym:key1:0".into(),
            object: ArchId(5),
            level_elevation: 0.0,
            ..Mesh::default()
        };
        s.meshes.push(m);

        // Add a material definition
        let mut def = cadcraft_arch3d::MaterialDef::default();
        def.base_color = [1.0, 0.0, 0.0, 1.0]; // Red
        s.materials.insert("sym:key1:0".into(), def);

        let mtl = write_mtl(&s);
        assert!(mtl.contains("newmtl sym:key1:0"));
        assert!(mtl.contains("Kd 1 0 0"));
    }
}
