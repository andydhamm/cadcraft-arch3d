//! Own glTF 2.0 GLB writer for CADCraft arch3d scenes.
//!
//! Pure Rust, no new dependency: a single binary buffer, one bufferView and one
//! accessor per attribute (`componentType` 5126/5125, `type` VEC3/VEC2/SCALAR,
//! `min`/`max` on POSITION), one mesh + node per [`Mesh`] named by material
//! (`wall-w{n}`, `floor-r{n}`/`ceiling-r{n}`, `deck-r{n}`, roof:
//! `roof-r{n}p{m}`, `roof-r{n}-fascia`, `roof-r{n}-soffit`; other `<material>-o{n}`),
//! one material per distinct `Mesh::material` (flat pbrMetallicRoughness base
//! colour via [`cadcraft_color`]), and one scene. The GLB header magic is
//! `0x46546C67`, little-endian, 4-byte aligned.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use cadcraft_arch3d::{Mesh, Scene3d, texture};
use cadcraft_color::{Color, Rgb};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::Finish;
use serde_json::{Value, json};

/// Build a GLB from the 3D scene extracted from the drawing's arch model.
pub fn write_drawing(drawing: &Drawing) -> Vec<u8> {
    write(&Scene3d::build(&drawing.arch))
}

/// Write a full GLB file (header + JSON chunk + BIN chunk) for `scene`.
///
/// The file is the 12-byte header, a space-padded JSON chunk and a
/// zero-padded binary chunk, all little-endian and 4-byte aligned.
pub fn write(scene: &Scene3d) -> Vec<u8> {
    let (json_text, bin) = build_bin(scene);

    let json_pad = (4 - json_text.len() % 4) % 4;
    let bin_pad = (4 - bin.len() % 4) % 4;
    let total = 12 + (8 + json_text.len() + json_pad) + (8 + bin.len() + bin_pad);

    let mut out = Vec::with_capacity(total);
    // Magic "glTF" = 0x46546C67 (little-endian).
    out.extend_from_slice(&[0x67, 0x6c, 0x54, 0x46]);
    // Version 2.
    out.extend_from_slice(&[0x02, 0x00, 0x00, 0x00]);
    push_u32(&mut out, total as u32);

    // JSON chunk (0x4E4F534A), padded to 4-byte alignment with 0x20.
    // chunkLength is the padded length per glTF 2.0 §4.4.
    push_u32(&mut out, (json_text.len() + json_pad) as u32);
    out.extend_from_slice(&[0x4a, 0x53, 0x4f, 0x4e]);
    out.extend_from_slice(json_text.as_bytes());
    out.resize(out.len() + json_pad, 0x20);

    // Binary chunk (0x004E4942), padded to 4-byte alignment with 0x00.
    // chunkLength is the padded length per glTF 2.0 §4.4.
    push_u32(&mut out, (bin.len() + bin_pad) as u32);
    out.extend_from_slice(&[0x42, 0x49, 0x4e, 0x00]);
    out.extend_from_slice(&bin);
    out.resize(out.len() + bin_pad, 0);

    out
}

/// Append a 32-bit value in little-endian byte order.
fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.push((v & 0xFF) as u8);
    buf.push(((v >> 8) & 0xFF) as u8);
    buf.push(((v >> 16) & 0xFF) as u8);
    buf.push(((v >> 24) & 0xFF) as u8);
}

/// Append a float in little-endian byte order (platform independent).
fn push_f32(buf: &mut Vec<u8>, v: f32) {
    push_u32(buf, v.to_bits());
}

/// Ensure `buf` length is a multiple of 4.
fn align4(buf: &mut Vec<u8>) {
    let r = buf.len() % 4;
    if r != 0 {
        buf.resize(buf.len() + (4 - r), 0);
    }
}

/// The finish of a finished-face material `wall-<f>`, `roof-<f>` or `floor-<f>`, or of the deck
/// boards' `deck` (Wood).
pub(crate) fn finish_of(material: &str) -> Option<Finish> {
    if let Some(f) = cadcraft_arch3d::texture::board_finish(material) {
        return Some(f);
    }
    let rest = material.strip_prefix("wall-").or_else(|| material.strip_prefix("roof-")).or_else(|| material.strip_prefix("floor-"))?;
    Finish::parse(rest)
}

/// Roughness of a finish's material.
fn finish_roughness(f: Finish) -> f64 {
    match f {
        Finish::Siding => 0.6,
        Finish::Shingles => 0.95,
        Finish::Brick => 0.9,
        Finish::Wood => 0.5,
    }
}

/// One node name per written mesh (those with vertices), in scene order, shared with the OBJ
/// writer. Roof planes are numbered in scene order: `roof-r{n}p0`, `roof-r{n}p1`, ...
pub(crate) fn node_names(scene: &Scene3d) -> Vec<String> {
    let mut roof_planes: HashMap<u32, u32> = HashMap::new();
    let mut foundation_stems: HashMap<u32, u32> = HashMap::new();
    let mut foundation_footings: HashMap<u32, u32> = HashMap::new();
    let mut wall_bodies: HashMap<u32, u32> = HashMap::new();
    let mut names = Vec::new();
    for m in scene.meshes.iter().filter(|m| !m.positions.is_empty()) {
        let name = match m.material.as_str() {
            mat if mat.starts_with("sym:") => format!("furnishing-f{}", m.object.0),
            mat if mat.starts_with("pull:") => format!("cabinet-cb{}-pull", m.object.0),
            mat if mat.starts_with("paint:") => paint_node_name(mat, m.object),
            "roof" => {
                let plane = match roof_planes.entry(m.object.0) {
                    Entry::Occupied(mut e) => {
                        let i = *e.get();
                        *e.get_mut() += 1;
                        i
                    }
                    Entry::Vacant(_) => {
                        roof_planes.insert(m.object.0, 1);
                        0
                    }
                };
                format!("roof-r{}p{}", m.object.0, plane)
            }
            "fascia" => format!("roof-r{}-fascia", m.object.0),
            "soffit" => format!("roof-r{}-soffit", m.object.0),
            "wall" => {
                // A vault's rise is a second plain wall mesh of the same wall.
                let i = next_index(&mut wall_bodies, m.object.0);
                if i == 0 { format!("wall-{}", m.object) } else { format!("wall-{}-rise", m.object) }
            }
            mat if mat == "soffit-box" || mat.starts_with("soffit-box:") => format!("soffit-sf{}", m.object.0),
            "foundation" => format!("foundation-fd{}-{}", m.object.0, next_index(&mut foundation_stems, m.object.0)),
            "footing" => format!("footing-fd{}-{}", m.object.0, next_index(&mut foundation_footings, m.object.0)),
            "slab" => format!("slab-r{}", m.object.0),
            "ceiling-layer" => format!("ceiling-r{}-layer", m.object.0),
            mat if mat.starts_with("wall-") && finish_of(mat).is_some() => format!("wall-{}-exterior", m.object),
            mat if mat.starts_with("roof-") && finish_of(mat).is_some() => {
                let plane = roof_planes.entry(m.object.0).or_insert(0);
                let i = *plane;
                *plane += 1;
                format!("roof-r{}p{}", m.object.0, i)
            }
            mat if mat.starts_with("floor-") && finish_of(mat).is_some() => format!("floor-r{}", m.object.0),
            "stair" => format!("stair-s{}", m.object.0),
            "railing" => format!("railing-ra{}", m.object.0),
            mat if mat.starts_with("stair-") => format!("stair-s{}", m.object.0),
            mat if mat.starts_with("railing-") => format!("railing-ra{}", m.object.0),
            "deck" => format!("deck-r{}", m.object.0),
            "deck-ledger" => format!("deck-r{}-ledger", m.object.0),
            "deck-joist" => format!("deck-r{}-joists", m.object.0),
            "deck-rim" => format!("deck-r{}-rims", m.object.0),
            "deck-beam" => format!("deck-r{}-beams", m.object.0),
            "deck-post" => format!("deck-r{}-posts", m.object.0),
            "deck-footing" => format!("deck-r{}-piers", m.object.0),
            "floor" => format!("floor-r{}", m.object.0),
            "ceiling" => format!("ceiling-r{}", m.object.0),
            "terrain" => "terrain".to_string(),
            "plant-crown" => format!("plant-pl{}", m.object.0),
            "plant-trunk" => format!("plant-pl{}-trunk", m.object.0),
            "light-trim" => format!("light-lf{}", m.object.0),
            mat if mat.starts_with("light-lens") => format!("light-lf{}-lens", m.object.0),
            mat if mat.starts_with("light-") => format!("light-lf{}-{}", m.object.0, mat.trim_start_matches("light-")),
            mat if mat.starts_with("path-") => {
                let kind = mat.trim_start_matches("path-").split('-').next().unwrap_or("path");
                format!("{kind}-tm{}", m.object.0)
            }
            "cabinet-box" => format!("cabinet-cb{}", m.object.0),
            "cabinet-front" => format!("cabinet-cb{}-front", m.object.0),
            "counter" => format!("cabinet-cb{}-counter", m.object.0),
            mat if mat.starts_with("fixture-") => format!("fixture-fx{}", m.object.0),
            mat if mat.starts_with("electrical:") => format!("electrical-el{}", m.object.0),
            _ => format!("{}-o{}", m.material, m.object.0),
        };
        names.push(name);
    }
    names
}

/// The running 0-based index of `key`'s next mesh in `counts`.
fn next_index(counts: &mut HashMap<u32, u32>, key: u32) -> u32 {
    let n = counts.entry(key).or_insert(0);
    let i = *n;
    *n = n.saturating_add(1);
    i
}

/// Separates a node name from a mesh index in `build_bin`'s node keys; never part of a node name.
const NODE_KEY_SEP: char = '\u{1}';

/// Whether a mesh of `material` joins the other meshes of its node: a furnishing's parts (`sym:`, its paints) and a
/// cabinet's pulls (`pull:`, `paint:cabinet/cb<n>/pull`), and (phase 17) the parts of a stair (`stair-…`), of a railing
/// (`railing-…`) and of one deck-structure kind (`deck-ledger`, `-joist`, `-rim`, `-beam`, `-post`, `-footing`). The
/// bare legacy `stair` / `railing` and the boards `deck` keep a node per mesh.
pub(crate) fn shares_a_node(material: &str) -> bool {
    material.starts_with("stair-")
        || material.starts_with("railing-")
        || matches!(material, "deck-ledger" | "deck-joist" | "deck-rim" | "deck-beam" | "deck-post" | "deck-footing")
        || material.starts_with("sym:")
        || material.starts_with("pull:")
        || material.starts_with("paint:furnishing/")
        || (material.starts_with("paint:cabinet/") && material.ends_with("/pull"))
}

fn paint_node_name(material: &str, object: cadcraft_doc::ArchId) -> String {
    let key = material.strip_prefix("paint:").unwrap_or(material);
    let Some(surface) = cadcraft_doc::arch::Surface::parse(key) else {
        return format!("{}-o{}", material, object.0);
    };
    match surface {
        cadcraft_doc::arch::Surface::WallSide { wall, side } => {
            format!("wall-w{}-{}", wall.0, if side == cadcraft_doc::arch::Side::Left { "left" } else { "right" })
        }
        cadcraft_doc::arch::Surface::RoomFloor(r) => format!("floor-r{}", r.0),
        cadcraft_doc::arch::Surface::RoomCeiling(r) => format!("ceiling-r{}", r.0),
        cadcraft_doc::arch::Surface::Roof(r) => format!("roof-r{}p0", r.0),
        cadcraft_doc::arch::Surface::Cabinet { cabinet, part } => {
            let base = format!("cabinet-cb{}", cabinet.0);
            match part {
                cadcraft_doc::arch::CabinetPart::Carcass => base,
                cadcraft_doc::arch::CabinetPart::Front => format!("{base}-front"),
                cadcraft_doc::arch::CabinetPart::Counter => format!("{base}-counter"),
                cadcraft_doc::arch::CabinetPart::Pull => format!("{base}-pull"),
            }
        }
        cadcraft_doc::arch::Surface::Fixture(fx) => format!("fixture-fx{}", fx.0),
        cadcraft_doc::arch::Surface::Furnishing { furnishing, .. } => format!("furnishing-f{}", furnishing.0),
        cadcraft_doc::arch::Surface::Countertop(c) => format!("countertop-ct{}", c.0),
        cadcraft_doc::arch::Surface::Backsplash(b) => format!("backsplash-bs{}", b.0),
    }
}

fn texture_image_name(key: &str) -> String {
    if let Some(rest) = key.strip_suffix(":normal") {
        format!("{}-normal", texture_image_name(rest))
    } else if let Some(rest) = key.strip_prefix("sym:") {
        format!("sym-{}", rest.replace(':', "-"))
    } else if let Some(rest) = key.strip_prefix("item:") {
        rest.to_string()
    } else if let Some(rest) = key.strip_prefix("finish:") {
        rest.to_string()
    } else {
        key.replace(':', "-")
    }
}

fn uv_for_mesh(m: &Mesh, scene: &Scene3d) -> Vec<[f32; 2]> {
    let Some(def) = scene.materials.get(&m.material) else { return m.uvs.clone() };
    let [u, v] = def.tile;
    if !u.is_finite() || !v.is_finite() || u <= 0.0 || v <= 0.0 {
        return m.uvs.clone();
    }
    if u == 1.0 && v == 1.0 {
        return m.uvs.clone();
    }
    let factor_u = 1.0 / u;
    let factor_v = 1.0 / v;
    m.uvs.iter().map(|uv| [uv[0] * factor_u, uv[1] * factor_v]).collect()
}

/// Build the JSON text plus the single binary buffer (with all views/accessors) for `scene`.
fn build_bin(scene: &Scene3d) -> (String, Vec<u8>) {
    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();

    let mut materials: Vec<Value> = Vec::new();
    let mut material_index: HashMap<String, u32> = HashMap::new();
    let names = node_names(scene);

    let written_meshes: Vec<&Mesh> = scene.meshes.iter().filter(|m| !m.positions.is_empty()).collect();
    let mut pngs: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut png_index: HashMap<String, u32> = HashMap::new();
    let mut texture_order: Vec<String> = Vec::new();
    let mut texture_seen: HashMap<String, ()> = HashMap::new();

    let add_png = |pngs: &mut Vec<(String, Vec<u8>, String)>,
                   png_index: &mut HashMap<String, u32>,
                   texture_order: &mut Vec<String>,
                   texture_seen: &mut HashMap<String, ()>,
                   key: String,
                   bytes: Vec<u8>,
                   name: String| {
        if texture_seen.contains_key(&key) {
            return;
        }
        texture_seen.insert(key.clone(), ());
        texture_order.push(key.clone());
        let idx = pngs.len() as u32;
        png_index.insert(key.clone(), idx);
        pngs.push((key.clone(), bytes, name));
    };

    for f in Finish::ALL {
        if written_meshes.iter().any(|m| finish_of(&m.material) == Some(f)) {
            let t = texture::texture(f);
            if let Ok(bytes) = crate::png::encode_rgba8(t.width, t.height, &t.rgba) {
                add_png(
                    &mut pngs,
                    &mut png_index,
                    &mut texture_order,
                    &mut texture_seen,
                    format!("finish:{}", f.name()),
                    bytes,
                    f.name().to_string(),
                );
            }
        }
    }

    for m in &written_meshes {
        if let Some(def) = scene.materials.get(&m.material) {
            if let Some(tex) = &def.texture {
                if let Some(image) = scene.textures.get(tex) {
                    if image.is_valid() {
                        let key = tex.clone();
                        if !texture_seen.contains_key(&key) {
                            if let Ok(bytes) = crate::png::encode_rgba8(image.width, image.height, &image.rgba) {
                                add_png(
                                    &mut pngs,
                                    &mut png_index,
                                    &mut texture_order,
                                    &mut texture_seen,
                                    key.clone(),
                                    bytes,
                                    texture_image_name(&key),
                                );
                            }
                        }
                    }
                }
            }
            if let Some(tex) = &def.normal_texture {
                if let Some(image) = scene.textures.get(tex) {
                    if image.is_valid() {
                        let key = tex.clone();
                        if !texture_seen.contains_key(&key) {
                            if let Ok(bytes) = crate::png::encode_rgba8(image.width, image.height, &image.rgba) {
                                add_png(
                                    &mut pngs,
                                    &mut png_index,
                                    &mut texture_order,
                                    &mut texture_seen,
                                    key.clone(),
                                    bytes,
                                    texture_image_name(&key),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let mut group_order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, m) in written_meshes.iter().enumerate() {
        let name = names.get(i).cloned().unwrap_or_default();
        if !grouped.contains_key(&name) {
            group_order.push(name.clone());
        }
        grouped.entry(name.clone()).or_default().push(i);
        let mat_idx = if let Some(idx) = material_index.get(&m.material) {
            *idx
        } else {
            let def = scene.materials.get(&m.material);
            let is_roof = m.material == "roof";
            let rgb = base_color_for(&m.material);
            let is_glass = m.material.contains("glass");
            let mut mat = json!({
                "name": m.material,
                "pbrMetallicRoughness": {
                    "baseColorFactor": if is_roof {
                        [ROOF_ALBEDO[0], ROOF_ALBEDO[1], ROOF_ALBEDO[2], 1.0]
                    } else if let Some(d) = def {
                        [d.base_color[0] as f64, d.base_color[1] as f64, d.base_color[2] as f64, d.base_color[3] as f64]
                    } else {
                        [
                            rgb.0 as f64 / 255.0,
                            rgb.1 as f64 / 255.0,
                            rgb.2 as f64 / 255.0,
                            if is_glass { 0.35 } else { 1.0 }
                        ]
                    },
                    "metallicFactor": if let Some(d) = def { d.metallic as f64 } else { 0.0 },
                    "roughnessFactor": if let Some(d) = def { d.roughness as f64 } else if is_roof { 0.9 } else if is_glass { 0.05 } else { 1.0 },
                },
            });
            if let Some(d) = def {
                if let Some(tex) = &d.texture {
                    if let Some(image) = scene.textures.get(tex) {
                        if image.is_valid() {
                            if let Some(index) = png_index.get(tex) {
                                mat["pbrMetallicRoughness"] = json!({
                                    "baseColorTexture": { "index": *index },
                                    "baseColorFactor": [d.base_color[0] as f64, d.base_color[1] as f64, d.base_color[2] as f64, d.base_color[3] as f64],
                                    "metallicFactor": d.metallic as f64,
                                    "roughnessFactor": d.roughness as f64,
                                });
                            }
                        }
                    }
                }
                if let Some(tex) = &d.normal_texture {
                    if let Some(image) = scene.textures.get(tex) {
                        if image.is_valid() {
                            if let Some(index) = png_index.get(tex) {
                                let mut normal = json!({ "index": *index });
                                if d.normal_scale != 1.0 {
                                    normal["scale"] = json!(d.normal_scale as f64);
                                }
                                mat["normalTexture"] = normal;
                            }
                        }
                    }
                }
            } else {
                let textured =
                    finish_of(&m.material).and_then(|f| pngs.iter().position(|(k, _, _)| k == &format!("finish:{}", f.name())).map(|t| (f, t)));
                if let Some((f, t)) = textured {
                    mat["pbrMetallicRoughness"] = json!({
                        "baseColorTexture": { "index": t },
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": 0.0,
                        "roughnessFactor": finish_roughness(f),
                    });
                }
            }
            if is_glass {
                mat["alphaMode"] = json!("BLEND");
            }
            materials.push(mat);
            let idx = (materials.len() - 1) as u32;
            material_index.insert(m.material.clone(), idx);
            idx
        };
        let _ = mat_idx;
    }

    let mut primitive_meshes: Vec<Value> = Vec::new();
    let mut mesh_index: HashMap<String, u32> = HashMap::new();
    let mut node_order: Vec<String> = Vec::new();
    let mut node_meshes: HashMap<String, Vec<Value>> = HashMap::new();

    for (i, m) in written_meshes.iter().enumerate() {
        let name = names.get(i).cloned().unwrap_or_default();
        let mat_idx = *material_index.get(&m.material).unwrap_or(&0);
        let uv_data = uv_for_mesh(m, scene);

        let pos_start = bin.len();
        for p in &m.positions {
            push_f32(&mut bin, p[0]);
            push_f32(&mut bin, p[1]);
            push_f32(&mut bin, p[2]);
        }
        align4(&mut bin);
        let pos_len = (m.positions.len() * 12) as u32;
        let bv_pos = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": pos_start,
            "byteLength": pos_len,
            "target": 34962,
        }));
        let (pmin, pmax) = extents3(&m.positions);
        accessors.push(json!({
            "bufferView": bv_pos,
            "byteOffset": 0,
            "count": m.positions.len() as u32,
            "type": "VEC3",
            "componentType": 5126,
            "min": [pmin[0] as f64, pmin[1] as f64, pmin[2] as f64],
            "max": [pmax[0] as f64, pmax[1] as f64, pmax[2] as f64],
        }));
        let acc_pos = accessors.len() - 1;

        let n_start = bin.len();
        for n in &m.normals {
            push_f32(&mut bin, n[0]);
            push_f32(&mut bin, n[1]);
            push_f32(&mut bin, n[2]);
        }
        align4(&mut bin);
        let n_len = (m.normals.len() * 12) as u32;
        let bv_norm = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": n_start,
            "byteLength": n_len,
            "target": 34962,
        }));
        accessors.push(json!({
            "bufferView": bv_norm,
            "byteOffset": 0,
            "count": m.normals.len() as u32,
            "type": "VEC3",
            "componentType": 5126,
        }));
        let acc_norm = accessors.len() - 1;

        let uv_start = bin.len();
        for uv in &uv_data {
            push_f32(&mut bin, uv[0]);
            push_f32(&mut bin, uv[1]);
        }
        align4(&mut bin);
        let uv_len = (uv_data.len() * 8) as u32;
        let bv_uv = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": uv_start,
            "byteLength": uv_len,
            "target": 34962,
        }));
        let mut uv_acc = json!({
            "bufferView": bv_uv,
            "byteOffset": 0,
            "count": uv_data.len() as u32,
            "type": "VEC2",
            "componentType": 5126,
        });
        if let Some(first) = uv_data.first() {
            let (mut lo, mut hi) = (*first, *first);
            for uv in &uv_data {
                for k in 0..2 {
                    lo[k] = lo[k].min(uv[k]);
                    hi[k] = hi[k].max(uv[k]);
                }
            }
            uv_acc["min"] = json!([lo[0] as f64, lo[1] as f64]);
            uv_acc["max"] = json!([hi[0] as f64, hi[1] as f64]);
        }
        accessors.push(uv_acc);
        let acc_uv = accessors.len() - 1;

        let idx_start = bin.len();
        for idx in &m.indices {
            push_u32(&mut bin, *idx);
        }
        align4(&mut bin);
        let idx_len = (m.indices.len() * 4) as u32;
        let bv_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": idx_start,
            "byteLength": idx_len,
            "target": 34963,
        }));
        accessors.push(json!({
            "bufferView": bv_idx,
            "byteOffset": 0,
            "count": m.indices.len() as u32,
            "type": "SCALAR",
            "componentType": 5125,
        }));
        let acc_idx = accessors.len() - 1;

        let primitive = json!({
            "attributes": {
                "POSITION": acc_pos,
                "NORMAL": acc_norm,
                "TEXCOORD_0": acc_uv
            },
            "indices": acc_idx,
            "material": mat_idx
        });
        // Only a furnishing's parts and a cabinet's pulls share one node (a primitive each); every other mesh keeps a
        // node of its own, as before phase 14, even when it shares its name (a fixture of two materials).
        let key = if shares_a_node(&m.material) { name.clone() } else { format!("{name}{NODE_KEY_SEP}{i}") };
        if !node_meshes.contains_key(&key) {
            node_order.push(key.clone());
        }
        node_meshes.entry(key).or_default().push(primitive);
    }

    let shown = |key: &str| key.split(NODE_KEY_SEP).next().unwrap_or(key).to_string();
    for key in &node_order {
        let mesh = json!({
            "name": shown(key),
            "primitives": node_meshes.get(key).cloned().unwrap_or_default()
        });
        let mesh_index_for_name = primitive_meshes.len() as u32;
        mesh_index.insert(key.clone(), mesh_index_for_name);
        primitive_meshes.push(mesh);
    }

    let mut gltf_nodes: Vec<Value> = Vec::new();
    for key in &node_order {
        gltf_nodes.push(json!({
            "name": shown(key),
            "mesh": mesh_index.get(key).cloned().unwrap_or_default(),
        }));
    }

    let mut images: Vec<Value> = Vec::new();
    let mut textures: Vec<Value> = Vec::new();
    for (key, (name, png)) in pngs.iter().enumerate().map(|(i, (_, bytes, name))| (i, (name, bytes))) {
        let _ = key;
        let start = bin.len();
        bin.extend_from_slice(png);
        align4(&mut bin);
        buffer_views.push(json!({ "buffer": 0, "byteOffset": start, "byteLength": png.len() as u32 }));
        images.push(json!({ "bufferView": buffer_views.len() - 1, "mimeType": "image/png", "name": name }));
        textures.push(json!({ "sampler": 0, "source": images.len() - 1, "name": name }));
    }

    let buffers: Vec<Value> = if bin.is_empty() { Vec::new() } else { vec![json!({ "byteLength": bin.len() as u32 })] };
    let (light_defs, light_nodes) = crate::glb_lights::punctual_lights(scene);
    let root = if written_meshes.is_empty() {
        Vec::new()
    } else {
        let mut children: Vec<u32> = (1..=gltf_nodes.len() as u32).collect();
        children.extend((0..light_nodes.len() as u32).map(|k| gltf_nodes.len() as u32 + 1 + k));
        vec![json!({
            "name": "arch3d-root",
            "rotation": [ROOT_ROTATION[0], ROOT_ROTATION[1], ROOT_ROTATION[2], ROOT_ROTATION[3]],
            "children": children,
        })]
    };
    let has_root = !root.is_empty();
    let all_nodes: Vec<Value> = root.into_iter().chain(gltf_nodes).chain(if has_root { light_nodes } else { Vec::new() }).collect();
    let scene_nodes: Vec<Value> = if written_meshes.is_empty() { Vec::new() } else { vec![json!(0)] };
    let mut value = json!({
        "asset": { "version": "2.0", "generator": "CADCraft arch3d" },
        "scene": 0,
        "scenes": [ { "name": "arch3d", "nodes": scene_nodes } ],
        "nodes": all_nodes,
        "meshes": primitive_meshes,
        "materials": materials,
        "buffers": buffers,
        "bufferViews": buffer_views,
        "accessors": accessors,
    });
    if !pngs.is_empty() {
        value["samplers"] = json!([{ "magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497 }]);
        value["images"] = json!(images);
        value["textures"] = json!(textures);
    }
    if has_root && !light_defs.is_empty() {
        value["extensionsUsed"] = json!(["KHR_lights_punctual"]);
        value["extensions"] = json!({ "KHR_lights_punctual": { "lights": light_defs } });
    }

    (json_string(value), bin)
}

/// Root node rotation: -90 degrees about X, the quaternion that turns the Z-up
/// arch3d scene into glTF's Y-up convention, mapping (x, y, z) to (x, z, -y).
/// Stored as (x, y, z, w); the angle is -π/2, so the axis (1,0,0) scaled by
/// sin(-π/4) ≈ -0.7071068 with w = cos(-π/4) ≈ 0.7071068.
const ROOT_ROTATION: [f64; 4] = [-0.7071068, 0.0, 0.0, 0.7071068];

/// Matte reddish roof albedo, as a 0..=1 `f64` triple.
const ROOF_ALBEDO: [f64; 3] = [151.0 / 255.0, 84.0 / 255.0, 74.0 / 255.0];

/// Resolve a material name to a flat base colour via [`cadcraft_color`].
fn base_color_for(name: &str) -> Rgb {
    match name {
        "cabinet-box" => Rgb(222, 184, 135),   // Light wood tone
        "cabinet-front" => Rgb(205, 164, 115), // Slightly darker wood
        "counter" => Rgb(160, 160, 160),       // Stone grey
        "fixture-sink" | "fixture-lavatory" | "fixture-toilet" | "fixture-bathtub" | "fixture-shower" => Rgb(240, 240, 240), // White
        "fixture-range" | "fixture-refrigerator" | "fixture-dishwasher" => Rgb(192, 192, 192), // Stainless grey
        _ => {
            let c = Color::parse(name).unwrap_or(Color::ByLayer);
            c.resolve(Color::ByLayer, Color::ByLayer)
        }
    }
}

/// Linear 0..=1 RGBA of material `name`, as the GLB's base colour factor shows it (roof albedo,
/// glass at 0.35 alpha).
pub(crate) fn material_rgba(name: &str) -> [f64; 4] {
    if name == "roof" {
        return [ROOF_ALBEDO[0], ROOF_ALBEDO[1], ROOF_ALBEDO[2], 1.0];
    }
    let rgb = base_color_for(name);
    let alpha = if name.contains("glass") { 0.35 } else { 1.0 };
    [rgb.0 as f64 / 255.0, rgb.1 as f64 / 255.0, rgb.2 as f64 / 255.0, alpha]
}

/// Axis-aligned extents (min/max) of a set of `VEC3` vertices.
fn extents3(v: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    if v.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in v {
        for i in 0..3 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    (min, max)
}

/// Serialize a `Value` to a `String`. `serde_json::to_string` never fails for a
/// `Value`, so the fallback is defensive only.
fn json_string(v: Value) -> String {
    match serde_json::to_string(&v) {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

/// The parsed JSON chunk of the GLB for `scene` (shared by other modules' tests).
#[cfg(test)]
pub(crate) fn tests_json(scene: &Scene3d) -> Value {
    let bytes = write(scene);
    let len = bytes.get(12..16).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    bytes.get(20..20 + len).and_then(|j| serde_json::from_slice(j).ok()).unwrap_or_default()
}

/// A one-wall model with a duplex (el0) and a dimmer (el1) on it (shared by tests).
#[cfg(test)]
pub(crate) fn wired_model() -> cadcraft_doc::arch::ArchModel {
    use cadcraft_doc::arch::{ArchModel, ElectricalDevice, ElectricalKind, Level, Wall, WallSide};
    use cadcraft_geom::Vec2;
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Level::default() });
    let wall = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), 0.2));
    m.push_electrical(ElectricalDevice { kind: ElectricalKind::Duplex, wall, side: WallSide::Left, offset: 1.5, ..Default::default() });
    m.push_electrical(ElectricalDevice { kind: ElectricalKind::Dimmer, wall, side: WallSide::Left, offset: 3.0, ..Default::default() });
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    use cadcraft_arch3d::{MaterialDef, Mesh, Scene3d};
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::ImageRgba;
    use serde_json::Value;

    /// One triangle, one triangle's indices, and a known material/id.
    fn wall_mesh() -> Mesh {
        Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            uvs: vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5]],
            indices: vec![0, 1, 2],
            ..Mesh::new("wall", ArchId(1))
        }
    }

    #[test]
    fn paint_node_names_for_countertop_and_backsplash() {
        assert_eq!(paint_node_name("paint:countertop/ct3", ArchId(3)), "countertop-ct3");
        assert_eq!(paint_node_name("paint:backsplash/bs2", ArchId(2)), "backsplash-bs2");
    }

    fn scene_with(walls: usize) -> Scene3d {
        Scene3d {
            meshes: (0..walls)
                .map(|i| {
                    let mut m = wall_mesh();
                    m.object = ArchId((i + 1) as u32);
                    m
                })
                .collect(),
            ..Default::default()
        }
    }

    /// A roof scene: two roof planes, a fascia and a soffit, all on roof 0.
    fn scene_with_roof() -> Scene3d {
        let mut scene = scene_with(0);
        scene.meshes.extend(["roof", "roof", "fascia", "soffit"].iter().map(|material| {
            let mut m = wall_mesh();
            m.material = material.to_string();
            m.object = ArchId(0);
            m
        }));
        scene
    }

    /// Parse the JSON chunk of a GLB byte slice.
    fn glb_json(bytes: &[u8], offset: usize, len: usize) -> Value {
        serde_json::from_slice(&bytes[offset..offset + len]).unwrap_or_default()
    }

    /// The root node of the scene, if any (present only when there is a mesh).
    fn root_node(json: &Value) -> Option<&Value> {
        json["nodes"].as_array()?.first()
    }

    /// Read the JSON chunk of a GLB byte slice and parse it.
    fn glb_json_chunk(bytes: &[u8]) -> Value {
        let json_len = le32(&bytes, 12) as usize;
        glb_json(bytes, 20, json_len)
    }

    /// A little-endian `u32` stored at byte offset `off` (4 bytes, LE).
    fn le32(bytes: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
    }

    #[test]
    fn glb_header_and_chunk_layout_are_valid() {
        let bytes = write(&scene_with(1));
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(&bytes[4..8], &[0x02, 0x00, 0x00, 0x00]);
        let total = le32(&bytes, 8);
        assert_eq!(total as usize, bytes.len());

        let json_len = le32(&bytes, 12) as usize;
        assert_eq!(le32(&bytes, 16), 0x4E4F534A);
        let json_pad = (4 - json_len % 4) % 4;
        let bin_start = 12 + 8 + json_len + json_pad;
        assert_eq!(bin_start % 4, 0);
        let bin_len = le32(&bytes, bin_start) as usize;
        assert_eq!(le32(&bytes, bin_start + 4), 0x004E4942);
        let bin_pad = (4 - bin_len % 4) % 4;
        assert_eq!(bytes.len(), bin_start + 8 + bin_len + bin_pad);
    }

    #[test]
    fn glb_json_chunk_parses_and_has_one_mesh_per_wall() {
        let bytes = write(&scene_with(3));
        let json = glb_json_chunk(&bytes);
        match &json["meshes"] {
            Value::Array(m) => {
                assert_eq!(m.len(), 3);
                for i in 0..3 {
                    assert_eq!(m[i]["name"], format!("wall-w{}", i + 1));
                }
            }
            _ => assert!(false, "json['meshes'] is not an array"),
        }
    }

    #[test]
    fn glb_accessor_min_max_match_the_geometry() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        let pos = &json["accessors"][0];
        match &pos["min"] {
            Value::Array(a) => {
                assert_eq!(a.len(), 3);
                assert_eq!(a[0].as_f64().unwrap_or(f64::NAN), 0.0);
                assert_eq!(a[1].as_f64().unwrap_or(f64::NAN), 0.0);
                assert_eq!(a[2].as_f64().unwrap_or(f64::NAN), 0.0);
            }
            _ => assert!(false, "min is not an array"),
        }
        match &pos["max"] {
            Value::Array(a) => {
                assert_eq!(a.len(), 3);
                assert_eq!(a[0].as_f64().unwrap_or(f64::NAN), 1.0);
                assert_eq!(a[1].as_f64().unwrap_or(f64::NAN), 0.0);
                assert_eq!(a[2].as_f64().unwrap_or(f64::NAN), 1.0);
            }
            _ => assert!(false, "max is not an array"),
        }
    }

    #[test]
    fn glb_indices_count_is_a_multiple_of_three() {
        let bytes = write(&scene_with(2));
        let json = glb_json_chunk(&bytes);
        match &json["accessors"] {
            Value::Array(accs) => {
                for acc in accs {
                    if let Some(t) = acc["type"].as_str() {
                        if t == "SCALAR" {
                            match acc["count"].as_u64() {
                                Some(count) => assert!(count % 3 == 0, "indices count {count} is not a multiple of 3"),
                                _ => assert!(false, "no count on SCALAR accessor"),
                            }
                        }
                    }
                }
            }
            _ => assert!(false, "accessors is not an array"),
        }
    }

    fn finished(material: &str, id: u32) -> Mesh {
        let mut m = wall_mesh();
        m.material = material.into();
        m.object = ArchId(id);
        m
    }

    #[test]
    fn glb_embeds_one_png_per_finish_used() {
        let mut scene = scene_with(0);
        scene.meshes = vec![finished("wall-brick", 1), finished("wall-brick", 2), finished("roof-shingles", 0), finished("floor-wood", 3)];
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let images = json["images"].as_array().unwrap();
        let names: Vec<&str> = images.iter().map(|i| i["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["shingles", "brick", "wood"]);
        let json_len = le32(&bytes, 12) as usize;
        let bin = 20 + json_len + 8;
        for (i, img) in images.iter().enumerate() {
            assert_eq!(img["mimeType"], "image/png");
            let bv = &json["bufferViews"][img["bufferView"].as_u64().unwrap() as usize];
            assert!(bv.get("target").is_none() && bv.get("byteStride").is_none());
            let off = bv["byteOffset"].as_u64().unwrap() as usize;
            assert_eq!(off % 4, 0);
            assert_eq!(&bytes[bin + off..bin + off + 8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
            assert_eq!(json["textures"][i]["source"], i as u64);
            assert_eq!(json["textures"][i]["sampler"], 0);
        }
        assert_eq!(json["samplers"][0]["minFilter"], 9987);
        assert_eq!(json["samplers"][0]["wrapS"], 10497);
        assert_eq!(write(&scene), bytes);
    }

    #[test]
    fn finished_material_uses_its_texture() {
        let mut scene = scene_with(0);
        scene.meshes = vec![finished("wall-brick", 1), finished("roof-shingles", 0)];
        let json = glb_json_chunk(&write(&scene));
        let mats = json["materials"].as_array().unwrap();
        let brick = mats.iter().find(|m| m["name"] == "wall-brick").unwrap();
        let pbr = &brick["pbrMetallicRoughness"];
        assert_eq!(pbr["baseColorTexture"]["index"], 1);
        assert_eq!(pbr["baseColorFactor"], json!([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(pbr["metallicFactor"].as_f64().unwrap(), 0.0);
        assert_eq!(pbr["roughnessFactor"].as_f64().unwrap(), 0.9);
        let roof = mats.iter().find(|m| m["name"] == "roof-shingles").unwrap();
        assert_eq!(roof["pbrMetallicRoughness"]["baseColorTexture"]["index"], 0);
        assert_eq!(roof["pbrMetallicRoughness"]["roughnessFactor"].as_f64().unwrap(), 0.95);
        let uv = json["accessors"].as_array().unwrap().iter().find(|a| a["type"] == "VEC2").unwrap();
        assert_eq!(uv["max"], json!([0.5, 0.5]));
    }

    #[test]
    fn deck_boards_carry_the_wood_texture() {
        let mut scene = scene_with(0);
        scene.meshes = vec![finished("deck", 4), finished("deck-joist", 4)];
        let json = glb_json_chunk(&write(&scene));
        let names: Vec<&str> = json["images"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["wood"]);
        let mats = json["materials"].as_array().unwrap();
        let deck = &mats.iter().find(|m| m["name"] == "deck").unwrap()["pbrMetallicRoughness"];
        assert_eq!(deck["baseColorTexture"]["index"], 0);
        assert_eq!(deck["roughnessFactor"].as_f64().unwrap(), 0.5);
        let joist = &mats.iter().find(|m| m["name"] == "deck-joist").unwrap()["pbrMetallicRoughness"];
        assert!(joist.get("baseColorTexture").is_none());
        assert_eq!(node_names(&scene), ["deck-r4", "deck-r4-joists"]);
    }

    #[test]
    fn normal_texture_for_brick_and_item_materials() {
        let mut scene = Scene3d::default();
        let mut brick = wall_mesh();
        brick.material = "wall-brick".to_string();
        brick.object = ArchId(1);
        scene.materials.insert(
            "wall-brick".to_string(),
            MaterialDef {
                base_color: [1.0; 4],
                texture: Some("finish:brick".to_string()),
                normal_texture: Some("finish:brick:normal".to_string()),
                normal_scale: 1.5,
                ..Default::default()
            },
        );
        scene.textures.insert("finish:brick".to_string(), ImageRgba { width: 8, height: 8, rgba: vec![255; 8 * 8 * 4], ..Default::default() });
        scene.textures.insert("finish:brick:normal".to_string(), ImageRgba { width: 8, height: 8, rgba: vec![128; 8 * 8 * 4], ..Default::default() });

        let mut item = wall_mesh();
        item.material = "paint:room/r0/floor".to_string();
        item.object = ArchId(2);
        scene.materials.insert(
            "paint:room/r0/floor".to_string(),
            MaterialDef {
                base_color: [1.0; 4],
                texture: Some("item:test-mat".to_string()),
                normal_texture: Some("test-mat:normal".to_string()),
                normal_scale: 1.0,
                ..Default::default()
            },
        );
        scene.textures.insert("item:test-mat".to_string(), ImageRgba { width: 8, height: 8, rgba: vec![200; 8 * 8 * 4], ..Default::default() });
        scene.textures.insert("test-mat:normal".to_string(), ImageRgba { width: 8, height: 8, rgba: vec![64; 8 * 8 * 4], ..Default::default() });
        scene.meshes = vec![brick, item];

        let json = glb_json_chunk(&write(&scene));
        let names: Vec<&str> = json["images"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert!(names.starts_with(&["brick", "brick-normal"]));
        assert!(names.iter().any(|&name| name == "test-mat"));
        assert!(names.iter().any(|&name| name == "test-mat-normal"));
        let brick_mat = json["materials"].as_array().unwrap().iter().find(|m| m["name"] == "wall-brick").unwrap();
        assert_eq!(brick_mat["normalTexture"]["index"], 1);
        assert_eq!(brick_mat["normalTexture"]["scale"], 1.5);
        let item_mat = json["materials"].as_array().unwrap().iter().find(|m| m["name"] == "paint:room/r0/floor").unwrap();
        assert_eq!(item_mat["normalTexture"]["index"], 3);
        assert_eq!(item_mat["normalTexture"].get("scale"), None);
    }

    #[test]
    fn glb_without_finishes_has_no_images() {
        let json = glb_json_chunk(&write(&scene_with_roof()));
        assert!(json.get("images").is_none() && json.get("textures").is_none() && json.get("samplers").is_none());
    }

    #[test]
    fn exterior_face_node_is_named_wall_exterior() {
        let mut scene = scene_with(0);
        scene.meshes = vec![finished("wall-siding", 4), finished("roof-wood", 2), finished("roof-wood", 2), finished("floor-wood", 5)];
        assert_eq!(node_names(&scene), ["wall-w4-exterior", "roof-r2p0", "roof-r2p1", "floor-r5"]);
    }

    #[test]
    fn electrical_plate_materials_reach_the_glb() {
        let scene = Scene3d::build(&wired_model());
        let json = glb_json_chunk(&write(&scene));
        let materials = json["materials"].as_array().cloned().unwrap_or_default();
        let is_plate = |m: &Value| m["name"].as_str().is_some_and(|n| n.starts_with("electrical:"));
        let plates: Vec<&str> = materials.iter().filter(|m| is_plate(m)).filter_map(|m| m["name"].as_str()).collect();
        assert_eq!(plates, ["electrical:duplex", "electrical:dimmer"]);
        for m in materials.iter().filter(|m| is_plate(m)) {
            let f = m["pbrMetallicRoughness"]["baseColorFactor"].as_array().cloned().unwrap_or_default();
            assert_eq!(f.len(), 4);
            assert!(f.iter().all(|v| v.as_f64().is_some_and(f64::is_finite)));
        }
        let meshes = json["meshes"].as_array().cloned().unwrap_or_default();
        for (node, mat) in [("electrical-el0", "electrical:duplex"), ("electrical-el1", "electrical:dimmer")] {
            let want = materials.iter().position(|m| m["name"] == mat);
            assert!(want.is_some(), "{mat} missing");
            let referenced = json["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|n| n["name"] == node)
                .filter_map(|n| n["mesh"].as_u64())
                .filter_map(|i| meshes.get(i as usize))
                .flat_map(|m| m["primitives"].as_array().cloned().unwrap_or_default())
                .any(|p| p["material"].as_u64().map(|i| i as usize) == want);
            assert!(referenced, "{node} does not reference {mat}");
        }
    }

    #[test]
    fn node_names_for_soffits_foundations_footings_slabs_stack_layers_and_wall_rises() {
        let mesh = |material: &str, object: u32| {
            let mut m = wall_mesh();
            m.material = material.to_string();
            m.object = ArchId(object);
            m
        };
        let scene = Scene3d {
            meshes: vec![
                mesh("wall", 4),
                mesh("wall", 4),
                mesh("wall", 0),
                mesh("soffit-box", 0),
                mesh("soffit-box:paint", 1),
                mesh("foundation", 0),
                mesh("foundation", 0),
                mesh("footing", 0),
                mesh("footing", 0),
                mesh("foundation", 1),
                mesh("slab", 3),
                mesh("ceiling-layer", 2),
            ],
            ..Scene3d::default()
        };
        assert_eq!(
            node_names(&scene),
            [
                "wall-w4",
                "wall-w4-rise",
                "wall-w0",
                "soffit-sf0",
                "soffit-sf1",
                "foundation-fd0-0",
                "foundation-fd0-1",
                "footing-fd0-0",
                "footing-fd0-1",
                "foundation-fd1-0",
                "slab-r3",
                "ceiling-r2-layer"
            ]
        );
    }

    fn p17_scene(materials: &[(&str, u32)]) -> Scene3d {
        let meshes = materials
            .iter()
            .map(|&(material, object)| {
                let mut m = wall_mesh();
                m.material = material.to_string();
                m.object = ArchId(object);
                m
            })
            .collect();
        Scene3d { meshes, ..Scene3d::default() }
    }

    #[test]
    fn node_names_for_phase17_materials() {
        let scene = p17_scene(&[
            ("stair-tread", 2),
            ("stair-riser", 2),
            ("stair-landing", 2),
            ("stair", 2),
            ("railing-post", 3),
            ("railing-rail", 3),
            ("railing", 3),
            ("deck", 4),
            ("deck-ledger", 4),
            ("deck-joist", 4),
            ("deck-rim", 4),
            ("deck-beam", 4),
            ("deck-post", 4),
            ("deck-footing", 4),
            ("window-grille", 5),
            ("window-sill-left", 5),
            ("door-casing-left", 6),
        ]);
        assert_eq!(
            node_names(&scene),
            [
                "stair-s2",
                "stair-s2",
                "stair-s2",
                "stair-s2",
                "railing-ra3",
                "railing-ra3",
                "railing-ra3",
                "deck-r4",
                "deck-r4-ledger",
                "deck-r4-joists",
                "deck-r4-rims",
                "deck-r4-beams",
                "deck-r4-posts",
                "deck-r4-piers",
                "window-grille-o5",
                "window-sill-left-o5",
                "door-casing-left-o6"
            ]
        );
    }

    #[test]
    fn phase17_parts_share_one_node() {
        let scene = p17_scene(&[
            ("stair-tread", 0),
            ("stair-riser", 0),
            ("railing-post", 1),
            ("deck-joist", 2),
            ("deck-joist", 2),
            ("stair", 0),
            ("stair", 0),
            ("deck", 2),
        ]);
        let json = tests_json(&scene);
        let nodes: Vec<&str> = json["nodes"].as_array().map(|a| a.iter().filter_map(|n| n["name"].as_str()).collect()).unwrap_or_default();
        let count = |n: &str| nodes.iter().filter(|&&x| x == n).count();
        assert_eq!(count("stair-s0"), 3, "{nodes:?}"); // one shared node + two legacy nodes
        assert_eq!(count("railing-ra1"), 1);
        assert_eq!(count("deck-r2-joists"), 1);
        assert_eq!(count("deck-r2"), 1);
        for m in ["stair-tread", "railing-rail", "deck-footing", "deck-post"] {
            assert!(shares_a_node(m), "{m}");
        }
        for m in ["stair", "railing", "deck", "wall", "window-grille"] {
            assert!(!shares_a_node(m), "{m}");
        }
    }

    #[test]
    fn glb_nodes_are_named_electrical_el() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "electrical:duplex".to_string();
                m.object = ArchId(0);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:gfci".to_string();
                m.object = ArchId(3);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:dimmer".to_string();
                m.object = ArchId(7);
                m
            },
        ];
        let names = node_names(&scene);
        assert_eq!(names, ["electrical-el0", "electrical-el3", "electrical-el7"]);
        assert!(names.iter().all(|n| !n.contains(':')));
    }

    #[test]
    fn glb_electrical_all_kinds_named() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "electrical:duplex".to_string();
                m.object = ArchId(0);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:gfci".to_string();
                m.object = ArchId(1);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:dimmer".to_string();
                m.object = ArchId(2);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:single-pole".to_string();
                m.object = ArchId(3);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:three-way".to_string();
                m.object = ArchId(4);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "electrical:240v".to_string();
                m.object = ArchId(5);
                m
            },
        ];
        let names = node_names(&scene);
        assert_eq!(names, ["electrical-el0", "electrical-el1", "electrical-el2", "electrical-el3", "electrical-el4", "electrical-el5",]);
        assert!(names.iter().all(|n| n.starts_with("electrical-el")));
        assert!(names.iter().all(|n| !n.contains(':')));
    }

    #[test]
    fn glb_electrical_large_id() {
        let mut scene = Scene3d::default();
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "electrical:duplex".to_string();
            m.object = ArchId(65535);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "electrical:dimmer".to_string();
            m.object = ArchId(1048576);
            m
        });
        let names = node_names(&scene);
        assert_eq!(names, ["electrical-el65535", "electrical-el1048576"]);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert!(node_names.contains(&"electrical-el65535".to_string()));
        assert!(node_names.contains(&"electrical-el1048576".to_string()));
    }

    #[test]
    fn glb_electrical_deterministic() {
        let scene = Scene3d::build(&wired_model());
        let a = write(&scene);
        let b = write(&scene);
        assert_eq!(a, b);
    }

    #[test]
    fn glb_is_byte_deterministic() {
        let a = write(&scene_with(2));
        let b = write(&scene_with(2));
        assert_eq!(a, b);
    }

    #[test]
    fn glb_chunk_header_is_length_then_type() {
        let bytes = write(&scene_with(1));
        let json_len = le32(&bytes, 12);
        assert_eq!(le32(&bytes, 16), 0x4E4F534A);
        let json_pad = (4 - json_len as usize % 4) % 4;
        let bin_start = 12 + 8 + json_len as usize + json_pad;
        let _bin_len = le32(&bytes, bin_start);
        assert_eq!(le32(&bytes, bin_start + 4), 0x004E4942);
    }

    #[test]
    fn glb_buffers_use_byte_length() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        assert!(json["buffers"][0].get("byteLength").is_some());
        assert!(json["buffers"][0].get("offset").is_none());
        assert!(json["buffers"][0].get("length").is_none());
    }

    #[test]
    fn glb_buffer_views_use_byte_offset_and_length() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        for bv in json["bufferViews"].as_array().unwrap() {
            assert!(bv.get("byteOffset").is_some());
            assert!(bv.get("byteLength").is_some());
            assert!(bv.get("target").is_some());
            assert!(bv.get("offset").is_none());
            assert!(bv.get("length").is_none());
            assert!(bv.get("viewType").is_none());
        }
    }

    #[test]
    fn glb_accessors_reference_buffer_views() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        for acc in json["accessors"].as_array().unwrap() {
            assert!(acc.get("bufferView").is_some());
            assert!(acc.get("byteOffset").is_some());
            assert!(acc.get("buffer").is_none());
            assert!(acc.get("offset").is_none());
        }
    }

    #[test]
    fn glb_asset_declares_version_2_0() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        assert_eq!(json["asset"]["version"], "2.0");
        assert_eq!(json["asset"]["generator"], "CADCraft arch3d");
    }

    #[test]
    fn glb_json_chunk_is_space_padded() {
        let bytes = write(&scene_with(1));
        let json_len = le32(&bytes, 12) as usize;
        let json_pad = (4 - json_len % 4) % 4;
        for i in 0..json_pad {
            assert_eq!(bytes[20 + json_len + i], 0x20);
        }
    }

    #[test]
    fn empty_scene_writes_a_valid_empty_glb() {
        let scene = Scene3d::default();
        let bytes = write(&scene);
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(&bytes[4..8], &[0x02, 0x00, 0x00, 0x00]);
        let total = le32(&bytes, 8);
        assert_eq!(total as usize, bytes.len());
        let json = glb_json_chunk(&bytes);
        assert_eq!(json["meshes"].as_array().and_then(|a| Some(a.len())), Some(0));
        assert_eq!(json["scenes"][0]["nodes"].as_array().and_then(|a| Some(a.len())), Some(0));
        // No root node when the scene is empty.
        assert_eq!(json["nodes"].as_array().and_then(|a| Some(a.len())), Some(0));
        let json_len = le32(&bytes, 12) as usize;
        let json_pad = (4 - json_len % 4) % 4;
        let bin_start = 12 + 8 + json_len + json_pad;
        assert_eq!(le32(&bytes, bin_start + 4), 0x004E4942);
        assert_eq!(bytes.len(), bin_start + 8);
    }

    #[test]
    fn glb_wall_names_are_unchanged() {
        let scene = scene_with(3);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(meshes[0]["name"], "wall-w1");
        assert_eq!(meshes[1]["name"], "wall-w2");
        assert_eq!(meshes[2]["name"], "wall-w3");
    }

    #[test]
    fn one_node_per_furnishing_with_a_primitive_per_part() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "sym:thing:0".to_string();
                m.object = ArchId(1);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "sym:thing:1".to_string();
                m.object = ArchId(1);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "sym:other:0".to_string();
                m.object = ArchId(2);
                m
            },
        ];
        let json = glb_json_chunk(&write(&scene));
        let mesh_names: Vec<&str> = json["meshes"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
        assert!(mesh_names.contains(&"furnishing-f1"));
        assert!(mesh_names.contains(&"furnishing-f2"));
        let f1 = json["meshes"].as_array().unwrap().iter().find(|m| m["name"] == "furnishing-f1").unwrap();
        assert_eq!(f1["primitives"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn only_furnishings_and_pulls_share_a_node() {
        // A fixture of two materials was two nodes before phase 14 (the OBJ groups match them); it stays two.
        let mut scene = Scene3d::default();
        scene.meshes = ["fixture-chrome", "fixture-ceramic", "pull:bar", "pull:bar"]
            .iter()
            .map(|mat| {
                let mut m = wall_mesh();
                m.material = (*mat).to_string();
                m.object = ArchId(3);
                m
            })
            .collect();
        let json = glb_json_chunk(&write(&scene));
        let names: Vec<&str> = json["nodes"].as_array().unwrap().iter().filter_map(|n| n["name"].as_str()).collect();
        assert_eq!(names.iter().filter(|n| **n == "fixture-fx3").count(), 2, "{names:?}");
        assert_eq!(names.iter().filter(|n| **n == "cabinet-cb3-pull").count(), 1, "{names:?}");
    }

    #[test]
    fn cabinet_pulls_are_one_node() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "pull:bar:0".to_string();
                m.object = ArchId(7);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "paint:cabinet/cb7/pull".to_string();
                m.object = ArchId(7);
                m
            },
        ];
        let json = glb_json_chunk(&write(&scene));
        let names: Vec<&str> = json["meshes"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"cabinet-cb7-pull"));
        assert_eq!(names.iter().filter(|n| **n == "cabinet-cb7-pull").count(), 1);
    }

    #[test]
    fn painted_wall_sides_are_named_nodes() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "paint:wall/w3/left".to_string();
                m.object = ArchId(3);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "paint:wall/w3/right".to_string();
                m.object = ArchId(3);
                m
            },
        ];
        let json = glb_json_chunk(&write(&scene));
        let names: Vec<&str> = json["meshes"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"wall-w3-left"));
        assert!(names.contains(&"wall-w3-right"));
    }

    #[test]
    fn symbol_textures_embed_once_as_png() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![
            {
                let mut m = wall_mesh();
                m.material = "sym:demo:0".to_string();
                m.object = ArchId(9);
                m
            },
            {
                let mut m = wall_mesh();
                m.material = "sym:demo:1".to_string();
                m.object = ArchId(9);
                m
            },
        ];
        scene.materials.insert("sym:demo:0".to_string(), MaterialDef { texture: Some("sym:demo:t0".to_string()), ..Default::default() });
        scene.materials.insert("sym:demo:1".to_string(), MaterialDef { texture: Some("sym:demo:t0".to_string()), ..Default::default() });
        let mut rgba = vec![0_u8; 2 * 2 * 4];
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
        scene.textures.insert("sym:demo:t0".to_string(), ImageRgba { width: 2, height: 2, rgba, ..Default::default() });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let image_names: Vec<&str> = json["images"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
        assert_eq!(image_names, ["sym-demo-t0"]);
    }

    #[test]
    fn paint_tints_reach_base_color_factor() {
        let mut scene = Scene3d::default();
        scene.meshes = vec![{
            let mut m = wall_mesh();
            m.material = "paint:wall/w4/left".to_string();
            m.object = ArchId(4);
            m
        }];
        let tint = [0x20, 0x40, 0x80];
        scene
            .materials
            .insert("paint:wall/w4/left".to_string(), MaterialDef { base_color: [0.38, 0.46, 0.62, 1.0], tint: Some(tint), ..Default::default() });
        let json = glb_json_chunk(&write(&scene));
        let factor = &json["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"];
        let values = factor.as_array().unwrap();
        assert!((values[0].as_f64().unwrap() - 0.38).abs() < 1e-6);
        assert!((values[1].as_f64().unwrap() - 0.46).abs() < 1e-6);
        assert!((values[2].as_f64().unwrap() - 0.62).abs() < 1e-6);
        assert!((values[3].as_f64().unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn unpainted_scenes_write_the_same_glb() {
        let scene = scene_with(2);
        let a = write(&scene);
        let b = write(&scene);
        assert_eq!(a, b);
    }

    #[test]
    fn glb_names_opening_meshes_by_component() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "frame".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "leaf".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "casing".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "sill".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "header".into();
            m.object = ArchId(0);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(meshes[0]["name"], "glass-o2");
        assert_eq!(meshes[1]["name"], "frame-o0");
        assert_eq!(meshes[2]["name"], "leaf-o1");
        assert_eq!(meshes[3]["name"], "casing-o2");
        assert_eq!(meshes[4]["name"], "sill-o2");
        assert_eq!(meshes[5]["name"], "header-o0");
    }

    #[test]
    fn glb_glass_material_blends() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(2);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mat = &json["materials"][0];
        let pbr = &mat["pbrMetallicRoughness"];
        let base = &pbr["baseColorFactor"];
        assert_eq!(base[3].as_f64().unwrap(), 0.35);
        assert_eq!(pbr["metallicFactor"].as_f64().unwrap(), 0.0);
        assert_eq!(pbr["roughnessFactor"].as_f64().unwrap(), 0.05);
        assert_eq!(mat["alphaMode"], "BLEND");
    }

    #[test]
    fn glb_is_byte_deterministic_with_openings() {
        let mut scene = scene_with(2);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "frame".into();
            m.object = ArchId(3);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "leaf".into();
            m.object = ArchId(4);
            m
        });
        let a = write(&scene);
        let b = write(&scene);
        assert_eq!(a, b);
    }

    #[test]
    fn glb_nodes_include_door_window_and_glass() {
        let mut scene = scene_with(0);
        // Door frame mesh (o0).
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "door-frame".into();
            m.object = ArchId(0);
            m
        });
        // Window glass mesh (o1) — verifies {material}-o{number} naming.
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "window-glass".into();
            m.object = ArchId(1);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let nodes = json["nodes"].as_array().unwrap();
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap().to_string()).collect();
        assert!(mesh_names.contains(&"door-frame-o0".to_string()), "missing door-frame-o0: {:?}", mesh_names);
        assert!(mesh_names.contains(&"window-glass-o1".to_string()), "missing window-glass-o1: {:?}", mesh_names);
        // Nodes mirror mesh names (the first node is the root, the rest are meshes).
        let node_names: Vec<String> = nodes.iter().skip(1).map(|n| n["name"].as_str().unwrap().to_string()).collect();
        assert!(node_names.contains(&"door-frame-o0".to_string()), "node missing door-frame-o0: {:?}", node_names);
        assert!(node_names.contains(&"window-glass-o1".to_string()), "node missing window-glass-o1: {:?}", node_names);
        // Glass material: alphaMode BLEND and baseColorFactor[3] < 0.6.
        let materials = json["materials"].as_array().unwrap();
        assert!(
            materials.iter().any(|m| m["name"] == "window-glass"
                && m["alphaMode"] == "BLEND"
                && m["pbrMetallicRoughness"]["baseColorFactor"][3].as_f64().unwrap() < 0.6),
            "window-glass material missing BLEND/alpha"
        );
    }

    #[test]
    fn glb_names_roof_nodes() {
        let bytes = write(&scene_with_roof());
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["roof-r0p0", "roof-r0p1", "roof-r0-fascia", "roof-r0-soffit"]);
        // The first node is the root; the rest are the per-mesh nodes.
        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert_eq!(node_names, vec!["roof-r0p0", "roof-r0p1", "roof-r0-fascia", "roof-r0-soffit"]);
    }

    #[test]
    fn glb_roof_material_is_distinct() {
        let scene = Scene3d {
            meshes: vec![wall_mesh(), {
                let mut m = wall_mesh();
                m.material = "roof".into();
                m.object = ArchId(0);
                m
            }],
            ..Default::default()
        };
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mats = json["materials"].as_array().unwrap();
        assert_eq!(mats.len(), 2);
        let roof = mats.iter().find(|m| m["name"] == "roof").unwrap();
        let pbr = &roof["pbrMetallicRoughness"];
        let base = &pbr["baseColorFactor"];
        assert!(base[0].as_f64().unwrap() > 0.5 && base[1].as_f64().unwrap() < 0.5 && base[2].as_f64().unwrap() < 0.5, "roof albedo not reddish");
        assert_eq!(base[3].as_f64().unwrap(), 1.0);
        assert_eq!(pbr["metallicFactor"].as_f64().unwrap(), 0.0);
        assert_eq!(pbr["roughnessFactor"].as_f64().unwrap(), 0.9);
        // Distinct from the wall entry.
        let wall = mats.iter().find(|m| m["name"] == "wall").unwrap();
        assert_eq!(wall["pbrMetallicRoughness"]["roughnessFactor"].as_f64().unwrap(), 1.0);
        assert_ne!(wall["pbrMetallicRoughness"]["baseColorFactor"].as_array().unwrap(), base.as_array().unwrap());
    }

    #[test]
    fn glb_is_deterministic_with_a_roof() {
        let scene = scene_with_roof();
        let a = write(&scene);
        let b = write(&scene);
        assert_eq!(a, b);
    }

    #[test]
    fn glb_names_multiple_roofs_with_interleaved_planes() {
        let mut scene = scene_with(0);
        // Roof 0, plane 0
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m
        });
        // Roof 1, plane 0
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(1);
            m
        });
        // Roof 0, plane 1
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m
        });
        // Roof 1, plane 1
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(1);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["roof-r0p0", "roof-r1p0", "roof-r0p1", "roof-r1p1"]);
    }

    #[test]
    fn glb_roof_with_nan_positions_is_valid() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m.positions[0][0] = f32::NAN;
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn glb_roof_with_huge_positions_is_valid() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m.positions[0][0] = 1e10;
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn glb_roof_with_zero_planes_creates_no_meshes() {
        let scene = scene_with(0);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.is_empty());
    }

    #[test]
    fn glb_roof_material_only_created_when_roof_mesh_exists() {
        let scene = scene_with(0);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials: Vec<String> = json["materials"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(!materials.contains(&"roof".to_string()));
    }

    /// Verify that both chunk length fields carry the PADDED length (a multiple
    /// of 4) and that the BIN chunk header sits exactly at 12 + 8 + jsonChunkLen.
    /// Four successive model sizes are tested so every alignment residue (0‥3)
    /// is exercised, guaranteeing that the padding code path is hit.
    #[test]
    fn glb_chunk_lengths_are_padded_multiples_of_four() {
        for n in 0..=3 {
            let bytes = write(&scene_with(n));
            let json_chunk_len = le32(&bytes, 12) as usize;
            assert_eq!(json_chunk_len % 4, 0, "n={n}: JSON chunkLength {json_chunk_len} is not 4-byte aligned");
            // BIN chunk must start at 12 (header) + 8 (JSON chunk header) + json_chunk_len.
            let bin_offset = 12 + 8 + json_chunk_len;
            assert!(bin_offset + 8 <= bytes.len(), "n={n}: bin_offset {bin_offset} is out of bounds (file len {})", bytes.len());
            assert_eq!(le32(&bytes, bin_offset + 4), 0x004E4942, "n={n}: chunk at byte {bin_offset} is not a BIN chunk");
            let bin_chunk_len = le32(&bytes, bin_offset) as usize;
            assert_eq!(bin_chunk_len % 4, 0, "n={n}: BIN chunkLength {bin_chunk_len} is not 4-byte aligned");
        }
    }

    /// The 12-byte GLB header's `length` field must equal the total byte count.
    #[test]
    fn glb_header_length_is_the_file_length() {
        for n in 0..=3 {
            let bytes = write(&scene_with(n));
            let header_len = le32(&bytes, 8) as usize;
            assert_eq!(header_len, bytes.len(), "n={n}: header length {header_len} != actual file length {}", bytes.len());
        }
    }

    /// Walk the GLB chunk-by-chunk from the 12-byte header to EOF, verifying
    /// each chunk length is a multiple of 4 and that exactly 2 chunks
    /// (JSON + BIN) are found.
    #[test]
    fn glb_chunks_can_be_walked_from_header() {
        let bytes = write(&scene_with(2));
        let total = le32(&bytes, 8) as usize;
        assert_eq!(total, bytes.len());

        let mut offset = 12;
        let mut chunk_count = 0;
        while offset < bytes.len() {
            let chunk_len = le32(&bytes, offset) as usize;
            let _chunk_type = le32(&bytes, offset + 4);
            assert_eq!(chunk_len % 4, 0, "chunk {chunk_count} length {chunk_len} not multiple of 4");
            offset += 8 + chunk_len;
            chunk_count += 1;
        }
        assert_eq!(chunk_count, 2, "expected exactly 2 chunks (JSON + BIN)");
        assert_eq!(offset, bytes.len(), "chunk walking did not reach end of file");
    }

    /// Extract the JSON chunk using only the declared chunk length from the
    /// header and verify it parses as valid JSON.
    #[test]
    fn glb_json_chunk_content_is_valid_when_extracted_by_declared_length() {
        let bytes = write(&scene_with(3));
        let json_len = le32(&bytes, 12) as usize;
        let json_data = &bytes[20..20 + json_len];
        let json_str = std::str::from_utf8(json_data).expect("JSON chunk is not valid UTF-8");
        let value: Value = serde_json::from_str(json_str).expect("JSON chunk is not valid JSON");
        assert_eq!(value["asset"]["version"], "2.0");
        assert!(value.get("meshes").is_some());
    }

    /// Verify chunk structure holds for a larger scene (100 meshes).
    #[test]
    fn glb_large_scene_chunk_structure_is_valid() {
        let bytes = write(&scene_with(100));
        let total = le32(&bytes, 8) as usize;
        assert_eq!(total, bytes.len());

        let json_len = le32(&bytes, 12) as usize;
        assert_eq!(json_len % 4, 0);

        let bin_start = 12 + 8 + json_len;
        let bin_len = le32(&bytes, bin_start) as usize;
        assert_eq!(bin_len % 4, 0);
        assert_eq!(bin_start + 8 + bin_len, bytes.len());
    }

    /// Verify the declared BIN chunk length matches the actual binary data
    /// written (computed from mesh attribute sizes).
    #[test]
    fn glb_bin_chunk_length_matches_actual_bin_data() {
        let n = 5;
        let bytes = write(&scene_with(n));
        let json_len = le32(&bytes, 12) as usize;
        let bin_start = 12 + 8 + json_len;
        let bin_len = le32(&bytes, bin_start) as usize;

        // Each wall mesh contributes: 3*12 (pos) + 3*12 (norm) + 3*8 (uv) + 3*4 (idx) = 108 bytes
        let expected_bin = n * 108;
        assert_eq!(bin_len, expected_bin, "BIN chunk length {bin_len} != expected {expected_bin}");
    }

    /// Every primitive attribute key is a valid glTF 2.0 semantic: one of the
    /// built-in names or an application-specific name starting with "_".
    #[test]
    fn glb_attribute_names_are_gltf_semantics() {
        let bytes = write(&scene_with(2));
        let json = glb_json_chunk(&bytes);
        let attrs = json["meshes"].as_array().unwrap();
        for prim in attrs {
            let attributes = prim["primitives"][0]["attributes"].as_object().unwrap();
            for key in attributes.keys() {
                let builtin =
                    matches!(key.as_str(), "POSITION" | "NORMAL" | "TANGENT" | "TEXCOORD_0" | "TEXCOORD_1" | "COLOR_0" | "JOINTS_0" | "WEIGHTS_0");
                assert!(builtin || key.starts_with('_'), "non-gltf attribute {key}");
            }
        }
    }

    /// The scene is Y-up: a single root node carries the -90°-about-X rotation
    /// and the per-mesh nodes are its children.
    #[test]
    fn glb_scene_is_y_up() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        let root = root_node(&json).expect("root node");
        assert_eq!(root["name"], "arch3d-root");
        let rot = root["rotation"].as_array().unwrap();
        assert_eq!(rot[0].as_f64().unwrap(), -0.7071068);
        assert_eq!(rot[1].as_f64().unwrap(), 0.0);
        assert_eq!(rot[2].as_f64().unwrap(), 0.0);
        assert_eq!(rot[3].as_f64().unwrap(), 0.7071068);
        let children = root["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].as_u64().unwrap(), 1);
        // scenes[0].nodes references only the root.
        assert_eq!(json["scenes"][0]["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(json["scenes"][0]["nodes"][0].as_u64().unwrap(), 0);
    }

    /// The top-level JSON carries no stray "version" key; the version lives in
    /// asset.version per glTF 2.0.
    #[test]
    fn glb_json_has_no_stray_top_level_version() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        assert!(json.get("version").is_none(), "stray top-level version");
        assert_eq!(json["asset"]["version"], "2.0");
    }

    #[test]
    fn glb_roof_material_shared_across_roofs() {
        let mut scene = scene_with(0);
        // Roof 0
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m
        });
        // Roof 1
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(1);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials: Vec<String> = json["materials"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(materials, vec!["roof"]);
    }

    /// An empty scene has no root node and no scene node references.
    #[test]
    fn glb_empty_scene_has_empty_node_list() {
        let scene = Scene3d::default();
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 0, "empty scene should have no nodes");
        let scene_nodes = json["scenes"][0]["nodes"].as_array().unwrap();
        assert_eq!(scene_nodes.len(), 0, "empty scene should have no scene node references");
    }

    /// The root node carries only name, rotation and children — no mesh or
    /// translation fields (identity translations were dropped per spec).
    #[test]
    fn glb_root_node_has_no_mesh_or_translation() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        let root = root_node(&json).expect("root node");
        assert!(root.get("mesh").is_none(), "root node should not have a mesh field");
        assert!(root.get("translation").is_none(), "root node should not have a translation field");
        assert!(root.get("name").is_some(), "root node should have a name");
        assert!(root.get("rotation").is_some(), "root node should have a rotation");
        assert!(root.get("children").is_some(), "root node should have children");
    }

    /// No node in the scene carries an identity translation field; the spec
    /// requires all identity translations to be dropped.
    #[test]
    fn glb_all_nodes_lack_identity_translation() {
        let bytes = write(&scene_with(3));
        let json = glb_json_chunk(&bytes);
        let nodes = json["nodes"].as_array().unwrap();
        for node in nodes {
            let name = node["name"].as_str().unwrap_or("<unnamed>");
            assert!(node.get("translation").is_none(), "node {name} should not have a translation field");
        }
    }

    /// Every primitive of every mesh type (wall, roof, glass, frame, etc.) uses
    /// only valid glTF 2.0 attribute semantics.
    #[test]
    fn glb_attribute_names_valid_across_mesh_types() {
        let mut scene = scene_with(1);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "frame".into();
            m.object = ArchId(6);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let meshes = json["meshes"].as_array().unwrap();
        for mesh in meshes {
            let mesh_name = mesh["name"].as_str().unwrap_or("<unnamed>");
            let prims = mesh["primitives"].as_array().unwrap();
            for prim in prims {
                let attrs = prim["attributes"].as_object().unwrap();
                for key in attrs.keys() {
                    let builtin = matches!(
                        key.as_str(),
                        "POSITION" | "NORMAL" | "TANGENT" | "TEXCOORD_0" | "TEXCOORD_1" | "COLOR_0" | "JOINTS_0" | "WEIGHTS_0"
                    );
                    assert!(builtin || key.starts_with('_'), "non-gltf attribute {key} on mesh {mesh_name}");
                }
            }
        }
    }

    /// For any non-empty scene, scenes[0].nodes references only the root node
    /// at index 0; per-mesh nodes are children of the root, not scene-level.
    #[test]
    fn glb_scenes_nodes_always_references_root_only() {
        let bytes = write(&scene_with(5));
        let json = glb_json_chunk(&bytes);
        let scene_nodes = json["scenes"][0]["nodes"].as_array().unwrap();
        assert_eq!(scene_nodes.len(), 1, "scene should reference only the root node");
        assert_eq!(scene_nodes[0].as_u64().unwrap(), 0, "scene should reference root node at index 0");
    }

    /// Buffer views use ARRAY_BUFFER (34962) for attributes and
    /// ELEMENT_ARRAY_BUFFER (34963) for indices.
    #[test]
    fn glb_buffer_view_targets_are_correct() {
        let bytes = write(&scene_with(2));
        let json = glb_json_chunk(&bytes);
        let views = json["bufferViews"].as_array().unwrap();
        assert_eq!(views.len(), 8);
        for i in 0..8 {
            let target = views[i]["target"].as_u64().unwrap();
            if i % 4 == 3 {
                assert_eq!(target, 34963, "indices buffer view target wrong");
            } else {
                assert_eq!(target, 34962, "attribute buffer view target wrong");
            }
        }
    }

    /// The material list contains one entry per distinct material name.
    #[test]
    fn glb_material_count_matches_distinct_materials() {
        let mut scene = scene_with(2);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "glass".into();
            m.object = ArchId(6);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "frame".into();
            m.object = ArchId(7);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials = json["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 3);
    }

    /// A non-empty scene has one root node plus one node per mesh.
    #[test]
    fn glb_node_count_is_one_plus_mesh_count() {
        let bytes = write(&scene_with(4));
        let json = glb_json_chunk(&bytes);
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 5);
    }

    /// Glass detection works for material names containing "glass" at any position.
    #[test]
    fn glb_glass_detection_for_various_names() {
        let mut scene = scene_with(0);
        for name in ["glass", "window-glass", "glass-panel", "tinted-glass"] {
            scene.meshes.push({
                let mut m = wall_mesh();
                m.material = name.into();
                m.object = ArchId(0);
                m
            });
        }
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials = json["materials"].as_array().unwrap();
        for mat in materials {
            assert_eq!(mat["alphaMode"], "BLEND", "material {} should be BLEND", mat["name"]);
            let alpha = mat["pbrMetallicRoughness"]["baseColorFactor"][3].as_f64().unwrap();
            assert_eq!(alpha, 0.35, "material {} alpha should be 0.35", mat["name"]);
        }
    }

    /// Non-glass materials remain opaque (no alphaMode, alpha 1.0).
    #[test]
    fn glb_non_glass_materials_are_opaque() {
        let mut scene = scene_with(0);
        for name in ["wall", "frame", "leaf", "casing", "sill", "header", "roof"] {
            scene.meshes.push({
                let mut m = wall_mesh();
                m.material = name.into();
                m.object = ArchId(0);
                m
            });
        }
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials = json["materials"].as_array().unwrap();
        for mat in materials {
            assert!(mat.get("alphaMode").is_none(), "material {} should not have alphaMode", mat["name"]);
            let alpha = mat["pbrMetallicRoughness"]["baseColorFactor"][3].as_f64().unwrap();
            assert_eq!(alpha, 1.0, "material {} alpha should be 1.0", mat["name"]);
        }
    }

    /// Every buffer view's byteOffset is a multiple of 4 (4-byte alignment).
    #[test]
    fn glb_buffer_views_are_4_byte_aligned() {
        let bytes = write(&scene_with(3));
        let json = glb_json_chunk(&bytes);
        for bv in json["bufferViews"].as_array().unwrap() {
            let offset = bv["byteOffset"].as_u64().unwrap();
            assert_eq!(offset % 4, 0, "buffer view byteOffset {} not 4-byte aligned", offset);
        }
    }

    /// Accessor type and componentType match the glTF 2.0 spec for each attribute.
    #[test]
    fn glb_accessor_types_and_component_types() {
        let bytes = write(&scene_with(1));
        let json = glb_json_chunk(&bytes);
        let accessors = json["accessors"].as_array().unwrap();
        // 4 accessors per mesh: POSITION(VEC3/5126), NORMAL(VEC3/5126), TEXCOORD_0(VEC2/5126), INDICES(SCALAR/5125)
        assert_eq!(accessors.len(), 4);
        assert_eq!(accessors[0]["type"], "VEC3");
        assert_eq!(accessors[0]["componentType"], 5126);
        assert_eq!(accessors[1]["type"], "VEC3");
        assert_eq!(accessors[1]["componentType"], 5126);
        assert_eq!(accessors[2]["type"], "VEC2");
        assert_eq!(accessors[2]["componentType"], 5126);
        assert_eq!(accessors[3]["type"], "SCALAR");
        assert_eq!(accessors[3]["componentType"], 5125);
    }

    /// The buffer's byteLength matches the BIN chunk's declared length.
    #[test]
    fn glb_buffer_byte_length_matches_bin_chunk() {
        let bytes = write(&scene_with(2));
        let json = glb_json_chunk(&bytes);
        let buffer_len = json["buffers"][0]["byteLength"].as_u64().unwrap();
        let json_len = le32(&bytes, 12) as usize;
        let bin_start = 12 + 8 + json_len;
        let bin_len = le32(&bytes, bin_start) as u64;
        assert_eq!(buffer_len, bin_len, "buffer byteLength {} != BIN chunk length {}", buffer_len, bin_len);
    }

    /// Every node's mesh index is within the range of the meshes array.
    #[test]
    fn glb_node_mesh_indices_are_valid() {
        let bytes = write(&scene_with(3));
        let json = glb_json_chunk(&bytes);
        let mesh_count = json["meshes"].as_array().unwrap().len();
        for node in json["nodes"].as_array().unwrap().iter().skip(1) {
            let mesh_idx = node["mesh"].as_u64().unwrap();
            assert!(mesh_idx < mesh_count as u64, "node mesh index {} >= mesh count {}", mesh_idx, mesh_count);
        }
    }

    /// Every buffer view references buffer 0.
    #[test]
    fn glb_all_buffer_views_reference_buffer_zero() {
        let bytes = write(&scene_with(2));
        let json = glb_json_chunk(&bytes);
        for bv in json["bufferViews"].as_array().unwrap() {
            assert_eq!(bv["buffer"].as_u64().unwrap(), 0, "buffer view does not reference buffer 0");
        }
    }

    #[test]
    fn glb_names_stair_nodes() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(42);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"stair-s42".to_string()), "missing stair-s42: {:?}", mesh_names);

        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert!(node_names.contains(&"stair-s42".to_string()), "node missing stair-s42: {:?}", node_names);
    }

    #[test]
    fn glb_names_railing_and_deck_nodes() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(7);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(9);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"railing-ra7".to_string()), "missing railing-ra7: {:?}", mesh_names);
        assert!(mesh_names.contains(&"deck-r9".to_string()), "missing deck-r9: {:?}", mesh_names);

        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert!(node_names.contains(&"railing-ra7".to_string()), "node missing railing-ra7: {:?}", node_names);
        assert!(node_names.contains(&"deck-r9".to_string()), "node missing deck-r9: {:?}", node_names);
    }

    #[test]
    fn glb_multiple_stairs_railings_decks_interleaved() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(2);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["stair-s1", "railing-ra1", "deck-r1", "stair-s2", "railing-ra2", "deck-r2"]);

        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert_eq!(node_names, vec!["stair-s1", "railing-ra1", "deck-r1", "stair-s2", "railing-ra2", "deck-r2"]);
    }

    #[test]
    fn glb_stair_railing_deck_zero_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(0);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["stair-s0", "railing-ra0", "deck-r0"]);

        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert_eq!(node_names, vec!["stair-s0", "railing-ra0", "deck-r0"]);
    }

    #[test]
    fn glb_stair_railing_deck_large_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(65535);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(65536);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(1048576);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["stair-s65535", "railing-ra65536", "deck-r1048576"]);

        let node_names: Vec<String> = json["nodes"].as_array().unwrap().iter().skip(1).filter_map(|n| n["name"].as_str()).map(String::from).collect();
        assert_eq!(node_names, vec!["stair-s65535", "railing-ra65536", "deck-r1048576"]);
    }

    #[test]
    fn glb_mixed_scene_all_types_node_count() {
        let mut scene = scene_with(2);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(3);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "roof".into();
            m.object = ArchId(0);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 6);
        let mesh_names: Vec<String> = meshes.iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["wall-w1", "wall-w2", "stair-s5", "railing-ra3", "deck-r2", "roof-r0p0"]);

        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 7);
    }

    #[test]
    fn glb_stair_railing_deck_have_pbr_materials() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "stair".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "railing".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(1);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials = json["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 3);
        for mat in materials {
            let name = mat["name"].as_str().unwrap();
            let pbr = &mat["pbrMetallicRoughness"];
            let base = &pbr["baseColorFactor"];
            assert_eq!(base.as_array().unwrap().len(), 4, "material {} baseColorFactor must have 4 components", name);
            assert!(pbr.get("metallicFactor").is_some(), "material {} must have metallicFactor", name);
            assert!(pbr.get("roughnessFactor").is_some(), "material {} must have roughnessFactor", name);
            assert_eq!(base[3].as_f64().unwrap(), 1.0, "material {} should be opaque", name);
            assert!(mat.get("alphaMode").is_none(), "material {} should not have alphaMode", name);
        }
        let mat_names: Vec<String> = materials.iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mat_names.contains(&"stair".to_string()));
        assert!(mat_names.contains(&"railing".to_string()));
        assert!(mat_names.contains(&"deck".to_string()));
    }

    /// Floor and ceiling nodes are named by the room's id (`floor-r{n}` /
    /// `ceiling-r{n}`), not the default `<material>-o{n}` arm.
    #[test]
    fn glb_names_floors_and_ceilings_by_room() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "floor".into();
            m.object = ArchId(3);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "ceiling".into();
            m.object = ArchId(3);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"floor-r3".to_string()), "missing floor-r3: {:?}", mesh_names);
        assert!(mesh_names.contains(&"ceiling-r3".to_string()), "missing ceiling-r3: {:?}", mesh_names,);
    }

    /// Floor and ceiling with id 0 use the `r0` suffix, not `o0`.
    #[test]
    fn glb_floor_ceiling_zero_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "floor".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "ceiling".into();
            m.object = ArchId(0);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"floor-r0".to_string()), "missing floor-r0: {:?}", mesh_names);
        assert!(mesh_names.contains(&"ceiling-r0".to_string()), "missing ceiling-r0: {:?}", mesh_names);
    }

    /// Floor and ceiling with different ids get distinct names.
    #[test]
    fn glb_floor_ceiling_different_ids() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "floor".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "ceiling".into();
            m.object = ArchId(2);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"floor-r1".to_string()), "missing floor-r1: {:?}", mesh_names);
        assert!(mesh_names.contains(&"ceiling-r2".to_string()), "missing ceiling-r2: {:?}", mesh_names);
    }

    /// Floor, ceiling and deck with the same id all use the `r{id}` suffix.
    #[test]
    fn glb_floor_ceiling_deck_same_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "floor".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "ceiling".into();
            m.object = ArchId(5);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "deck".into();
            m.object = ArchId(5);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["floor-r5", "ceiling-r5", "deck-r5"]);
    }

    /// Floor and ceiling with large ids preserve the full id in the name.
    #[test]
    fn glb_floor_ceiling_large_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "floor".into();
            m.object = ArchId(65535);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "ceiling".into();
            m.object = ArchId(1048576);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"floor-r65535".to_string()), "missing floor-r65535: {:?}", mesh_names);
        assert!(mesh_names.contains(&"ceiling-r1048576".to_string()), "missing ceiling-r1048576: {:?}", mesh_names);
    }

    #[test]
    fn glb_cabinet_zero_id() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-box".into();
            m.object = ArchId(0);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "fixture-sink".into();
            m.object = ArchId(0);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"cabinet-cb0".to_string()), "missing cabinet-cb0: {:?}", mesh_names);
        assert!(mesh_names.contains(&"fixture-fx0".to_string()), "missing fixture-fx0: {:?}", mesh_names);
    }

    #[test]
    fn glb_multiple_cabinets_interleaved() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-box".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-box".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-front".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "counter".into();
            m.object = ArchId(2);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert_eq!(mesh_names, vec!["cabinet-cb1", "cabinet-cb2", "cabinet-cb1-front", "cabinet-cb2-counter"]);
    }

    #[test]
    fn glb_cabinet_material_colors() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-box".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-front".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "counter".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "fixture-sink".into();
            m.object = ArchId(2);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "fixture-range".into();
            m.object = ArchId(3);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let materials = json["materials"].as_array().unwrap();
        let box_mat = materials.iter().find(|m| m["name"] == "cabinet-box").unwrap();
        let box_color = &box_mat["pbrMetallicRoughness"]["baseColorFactor"];
        // Light wood tone: 222/255, 184/255, 135/255
        assert!((box_color[0].as_f64().unwrap() - 222.0 / 255.0).abs() < 0.01);
        assert!((box_color[1].as_f64().unwrap() - 184.0 / 255.0).abs() < 0.01);
        assert!((box_color[2].as_f64().unwrap() - 135.0 / 255.0).abs() < 0.01);
        let front_mat = materials.iter().find(|m| m["name"] == "cabinet-front").unwrap();
        let front_color = &front_mat["pbrMetallicRoughness"]["baseColorFactor"];
        // Slightly darker wood: 205/255, 164/255, 115/255
        assert!((front_color[0].as_f64().unwrap() - 205.0 / 255.0).abs() < 0.01);
        let counter_mat = materials.iter().find(|m| m["name"] == "counter").unwrap();
        let counter_color = &counter_mat["pbrMetallicRoughness"]["baseColorFactor"];
        // Stone grey: 160/255
        assert!((counter_color[0].as_f64().unwrap() - 160.0 / 255.0).abs() < 0.01);
        let sink_mat = materials.iter().find(|m| m["name"] == "fixture-sink").unwrap();
        let sink_color = &sink_mat["pbrMetallicRoughness"]["baseColorFactor"];
        // White: 240/255
        assert!((sink_color[0].as_f64().unwrap() - 240.0 / 255.0).abs() < 0.01);
        let range_mat = materials.iter().find(|m| m["name"] == "fixture-range").unwrap();
        let range_color = &range_mat["pbrMetallicRoughness"]["baseColorFactor"];
        // Stainless grey: 192/255
        assert!((range_color[0].as_f64().unwrap() - 192.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn cabinet_nodes_are_named_by_label() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-box".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "cabinet-front".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "counter".into();
            m.object = ArchId(1);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "fixture-sink".into();
            m.object = ArchId(2);
            m
        });

        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();

        assert!(mesh_names.contains(&"cabinet-cb1".to_string()), "missing cabinet-cb1: {:?}", mesh_names);
        assert!(mesh_names.contains(&"cabinet-cb1-front".to_string()), "missing cabinet-cb1-front: {:?}", mesh_names);
        assert!(mesh_names.contains(&"cabinet-cb1-counter".to_string()), "missing cabinet-cb1-counter: {:?}", mesh_names);
        assert!(mesh_names.contains(&"fixture-fx2".to_string()), "missing fixture-fx2: {:?}", mesh_names);
    }

    #[test]
    fn glb_finished_wall_interior_and_exterior_both_named() {
        let mut scene = scene_with(0);
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "wall".into();
            m.object = ArchId(3);
            m
        });
        scene.meshes.push({
            let mut m = wall_mesh();
            m.material = "wall-brick".into();
            m.object = ArchId(3);
            m
        });
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let mesh_names: Vec<String> = json["meshes"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).map(String::from).collect();
        assert!(mesh_names.contains(&"wall-w3".to_string()), "missing wall-w3: {:?}", mesh_names);
        assert!(mesh_names.contains(&"wall-w3-exterior".to_string()), "missing wall-w3-exterior: {:?}", mesh_names);
    }

    #[test]
    fn glb_texture_accessor_min_max_with_zero_uvs() {
        let mut m = wall_mesh();
        m.uvs = vec![[0.0, 0.0], [0.0, 0.0], [0.0, 0.0]];
        let mut scene = scene_with(0);
        scene.meshes.push(m);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let uv = json["accessors"].as_array().unwrap().iter().find(|a| a["type"] == "VEC2").unwrap();
        assert_eq!(uv["min"], json!([0.0, 0.0]));
        assert_eq!(uv["max"], json!([0.0, 0.0]));
    }

    #[test]
    fn glb_texture_accessor_min_max_with_negative_uvs() {
        let mut m = wall_mesh();
        m.uvs = vec![[-1.0, 2.0], [3.0, -4.0], [0.0, 0.0]];
        let mut scene = scene_with(0);
        scene.meshes.push(m);
        let bytes = write(&scene);
        let json = glb_json_chunk(&bytes);
        let uv = json["accessors"].as_array().unwrap().iter().find(|a| a["type"] == "VEC2").unwrap();
        assert_eq!(uv["min"], json!([-1.0, -4.0]));
        assert_eq!(uv["max"], json!([3.0, 2.0]));
    }

    #[test]
    fn glb_interleaved_stair_parts_from_two_stairs() {
        let scene = p17_scene(&[("stair-tread", 0), ("stair-tread", 1), ("stair-riser", 0), ("stair-riser", 1)]);
        let json = tests_json(&scene);
        let nodes: Vec<&str> = json["nodes"].as_array().map(|a| a.iter().filter_map(|n| n["name"].as_str()).collect()).unwrap_or_default();
        let s0 = nodes.iter().filter(|&&n| n == "stair-s0").count();
        let s1 = nodes.iter().filter(|&&n| n == "stair-s1").count();
        assert_eq!(s0, 1, "{nodes:?}");
        assert_eq!(s1, 1, "{nodes:?}");
        for name in ["stair-s0", "stair-s1"] {
            let mesh = json["meshes"].as_array().unwrap().iter().find(|m| m["name"] == name).unwrap();
            assert_eq!(mesh["primitives"].as_array().unwrap().len(), 2, "{name}");
        }
    }

    #[test]
    fn glb_stair_and_railing_same_object_id() {
        let scene = p17_scene(&[("stair-tread", 5), ("railing-post", 5)]);
        let names = node_names(&scene);
        assert_eq!(names, ["stair-s5", "railing-ra5"]);
        let json = tests_json(&scene);
        let nodes: Vec<&str> = json["nodes"].as_array().map(|a| a.iter().filter_map(|n| n["name"].as_str()).collect()).unwrap_or_default();
        assert_eq!(nodes.iter().filter(|&&n| n == "stair-s5").count(), 1, "{nodes:?}");
        assert_eq!(nodes.iter().filter(|&&n| n == "railing-ra5").count(), 1, "{nodes:?}");
    }

    #[test]
    fn shares_a_node_edge_cases() {
        assert!(!shares_a_node("stair"));
        assert!(!shares_a_node("railing"));
        assert!(!shares_a_node("deck"));
        assert!(shares_a_node("stair-tread"));
        assert!(shares_a_node("stair-riser"));
        assert!(shares_a_node("stair-stringer"));
        assert!(shares_a_node("stair-skirt"));
        assert!(shares_a_node("stair-landing"));
        assert!(shares_a_node("railing-post"));
        assert!(shares_a_node("railing-rail"));
        assert!(shares_a_node("railing-panel"));
        assert!(shares_a_node("railing-baluster"));
        assert!(shares_a_node("deck-ledger"));
        assert!(shares_a_node("deck-joist"));
        assert!(shares_a_node("deck-rim"));
        assert!(shares_a_node("deck-beam"));
        assert!(shares_a_node("deck-post"));
        assert!(shares_a_node("deck-footing"));
        assert!(!shares_a_node("wall"));
        assert!(!shares_a_node("roof"));
        assert!(!shares_a_node("floor"));
        assert!(!shares_a_node("window-grille"));
        assert!(!shares_a_node("door-casing-left"));
    }
}
