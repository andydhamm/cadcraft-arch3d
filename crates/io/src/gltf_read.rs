//! Our own reader for plain glTF 2.0 binary (`.glb`) files, producing a Z-up,
//! bottom-centred [`SymbolMesh`]. No extensions, no external or data URIs.

use crate::image_read::read_image;
use cadcraft_doc::arch::{ImageRgba, MAX_SYMBOL_TRIANGLES, SymbolMesh, SymbolPart, fnv1a64_hex};
use serde_json::Value;
use std::collections::BTreeMap;

/// Largest accepted element count of one accessor.
const MAX_ACCESSOR_COUNT: usize = 6_000_000;
/// Deepest node nesting walked.
const MAX_NODE_DEPTH: usize = 64;
/// Most node visits (instances count each time).
const MAX_NODE_VISITS: usize = 100_000;
/// Longest side of a decoded texture.
const TEXTURE_SIDE: u32 = 256;

const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

type Mat = [f64; 16];

const IDENTITY: Mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
/// glTF Y-up to Z-up: (x, y, z) -> (x, -z, y).
const Y_UP_TO_Z_UP: Mat = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// Read a plain glTF 2.0 binary into a Z-up, bottom-centred symbol mesh.
pub fn read_glb(bytes: &[u8]) -> Result<SymbolMesh, String> {
    let (json, bin) = split_chunks(bytes)?;
    check_extensions(&json)?;
    let mut rd = Reader { json: &json, bin, textures: Vec::new(), tex_map: BTreeMap::new(), parts: Vec::new(), tris: 0, verts: 0, visits: 0 };
    rd.check_buffers()?;
    for root in rd.scene_roots()? {
        rd.visit(root, &Y_UP_TO_Z_UP, 0)?;
    }
    let Reader { mut parts, textures, .. } = rd;
    if parts.is_empty() || parts.iter().all(|p| p.indices.is_empty()) {
        return Err("the file contains no triangles".into());
    }
    recentre(&mut parts)?;
    Ok(SymbolMesh { key: fnv1a64_hex(bytes), parts, textures, ..Default::default() })
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([*s.first()?, *s.get(1)?, *s.get(2)?, *s.get(3)?]))
}

/// Split the container into the parsed JSON and the optional BIN chunk.
fn split_chunks(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    if bytes.get(..4) != Some(b"glTF".as_slice()) {
        return Err("not a glTF binary (bad magic)".into());
    }
    if u32_at(bytes, 4) != Some(2) {
        return Err("only glTF version 2 is supported".into());
    }
    let total = u32_at(bytes, 8).ok_or("truncated glTF header")?;
    if total as usize != bytes.len() {
        return Err(format!("glTF length mismatch (header says {total} bytes, file has {})", bytes.len()));
    }
    let mut at = 12usize;
    let mut json: Option<Value> = None;
    let mut bin: Option<&[u8]> = None;
    let mut first = true;
    while at < bytes.len() {
        let len = u32_at(bytes, at).ok_or("truncated chunk header")? as usize;
        let kind = u32_at(bytes, at + 4).ok_or("truncated chunk header")?;
        if len % 4 != 0 {
            return Err("glTF chunk length is not a multiple of 4".into());
        }
        let start = at + 8;
        let end = start.checked_add(len).filter(|e| *e <= bytes.len()).ok_or("truncated glTF chunk")?;
        let data = bytes.get(start..end).ok_or("truncated glTF chunk")?;
        if first {
            if kind != CHUNK_JSON {
                return Err("the first glTF chunk is not JSON".into());
            }
            first = false;
            json = Some(serde_json::from_slice(data).map_err(|e| format!("invalid glTF JSON: {e}"))?);
        } else if kind == CHUNK_BIN && bin.is_none() {
            bin = Some(data);
        }
        at = end;
    }
    let json = json.ok_or("glTF has no JSON chunk")?;
    Ok((json, bin.unwrap_or(&[])))
}

fn check_extensions(json: &Value) -> Result<(), String> {
    let Some(req) = json.get("extensionsRequired") else { return Ok(()) };
    let list = req.as_array().ok_or("extensionsRequired is not an array")?;
    for e in list {
        let name = e.as_str().ok_or("extensionsRequired entry is not a string")?;
        return Err(match name {
            "KHR_draco_mesh_compression" => "KHR_draco_mesh_compression (Draco) is not supported".into(),
            "EXT_meshopt_compression" => "EXT_meshopt_compression (meshopt) is not supported".into(),
            "KHR_texture_basisu" => "KHR_texture_basisu (KTX2) is not supported".into(),
            other => format!("{other} is not supported"),
        });
    }
    Ok(())
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// An optional index/size member; present but not a non-negative integer is an error.
fn opt_usize(v: &Value, key: &str) -> Result<Option<usize>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(x) => x.as_u64().and_then(|n| usize::try_from(n).ok()).map(Some).ok_or_else(|| format!("`{key}` is not a valid index")),
    }
}

fn num_array(v: &Value, key: &str, n: usize) -> Result<Option<Vec<f64>>, String> {
    let Some(x) = v.get(key) else { return Ok(None) };
    let a = x.as_array().filter(|a| a.len() == n).ok_or_else(|| format!("`{key}` must be an array of {n} numbers"))?;
    let mut out = Vec::with_capacity(n);
    for e in a {
        let f = e.as_f64().filter(|f| f.is_finite()).ok_or_else(|| format!("`{key}` holds a non-finite number"))?;
        out.push(f);
    }
    Ok(Some(out))
}

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + row] * b[c * 4 + k];
            }
            r[c * 4 + row] = s;
        }
    }
    r
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn node_matrix(node: &Value) -> Result<Mat, String> {
    if let Some(m) = num_array(node, "matrix", 16)? {
        let mut out = IDENTITY;
        out.copy_from_slice(&m);
        return Ok(out);
    }
    let t = num_array(node, "translation", 3)?.unwrap_or_else(|| vec![0.0; 3]);
    let s = num_array(node, "scale", 3)?.unwrap_or_else(|| vec![1.0; 3]);
    let q = num_array(node, "rotation", 4)?.unwrap_or_else(|| vec![0.0, 0.0, 0.0, 1.0]);
    let len = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    if len < 1e-9 {
        return Err("node rotation is a zero quaternion".into());
    }
    let (x, y, z, w) = (q[0] / len, q[1] / len, q[2] / len, q[3] / len);
    // Rotation matrix columns, then scaled.
    let c0 = [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)];
    let c1 = [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)];
    let c2 = [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)];
    Ok([
        c0[0] * s[0],
        c0[1] * s[0],
        c0[2] * s[0],
        0.0,
        c1[0] * s[1],
        c1[1] * s[1],
        c1[2] * s[1],
        0.0,
        c2[0] * s[2],
        c2[1] * s[2],
        c2[2] * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ])
}

/// One decoded accessor: flat component values (raw, not normalised).
struct Accessor {
    data: Vec<f64>,
    normalized: bool,
    comp_type: u32,
}

/// A resolved material.
struct Material {
    base_color: [f32; 4],
    metallic: f32,
    roughness: f32,
    image: Option<usize>,
}

struct Reader<'a> {
    json: &'a Value,
    bin: &'a [u8],
    textures: Vec<ImageRgba>,
    tex_map: BTreeMap<usize, u32>,
    parts: Vec<SymbolPart>,
    tris: usize,
    verts: usize,
    visits: usize,
}

impl<'a> Reader<'a> {
    fn check_buffers(&self) -> Result<(), String> {
        for b in arr(self.json, "buffers") {
            if b.get("uri").is_some() {
                return Err("buffers with a uri (external or data URIs) are not supported".into());
            }
        }
        for i in arr(self.json, "images") {
            if i.get("uri").is_some() {
                return Err("images with a uri (external or data URIs) are not supported".into());
            }
        }
        Ok(())
    }

    fn scene_roots(&self) -> Result<Vec<usize>, String> {
        let scenes = arr(self.json, "scenes");
        let nodes = arr(self.json, "nodes");
        if !scenes.is_empty() {
            let i = opt_usize(self.json, "scene")?.unwrap_or(0);
            let scene = scenes.get(i).ok_or("default scene does not exist")?;
            let mut out = Vec::new();
            for n in arr(scene, "nodes") {
                out.push(n.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or("scene node index is invalid")?);
            }
            return Ok(out);
        }
        let mut child = vec![false; nodes.len()];
        for n in nodes {
            for c in arr(n, "children") {
                if let Some(slot) = c.as_u64().and_then(|c| usize::try_from(c).ok()).and_then(|c| child.get_mut(c)) {
                    *slot = true;
                }
            }
        }
        Ok((0..nodes.len()).filter(|i| child.get(*i) == Some(&false)).collect())
    }

    fn visit(&mut self, node: usize, parent: &Mat, depth: usize) -> Result<(), String> {
        if depth > MAX_NODE_DEPTH {
            return Err("glTF node hierarchy is too deep (or cyclic)".into());
        }
        self.visits += 1;
        if self.visits > MAX_NODE_VISITS {
            return Err("glTF scene has too many nodes".into());
        }
        let json = self.json;
        let n = arr(json, "nodes").get(node).ok_or("node index out of range")?;
        let world = mat_mul(parent, &node_matrix(n)?);
        if let Some(mi) = opt_usize(n, "mesh")? {
            let mesh = arr(json, "meshes").get(mi).ok_or("mesh index out of range")?;
            let name = mesh.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("mesh");
            for (pi, prim) in arr(mesh, "primitives").iter().enumerate() {
                self.add_primitive(name, pi, prim, &world)?;
            }
        }
        for c in arr(n, "children") {
            let c = c.as_u64().and_then(|c| usize::try_from(c).ok()).ok_or("child node index is invalid")?;
            self.visit(c, &world, depth + 1)?;
        }
        Ok(())
    }

    fn buffer_view(&self, i: usize) -> Result<(&'a [u8], Option<usize>), String> {
        let bv = arr(self.json, "bufferViews").get(i).ok_or("bufferView index out of range")?;
        if opt_usize(bv, "buffer")?.unwrap_or(0) != 0 {
            return Err("only the first (embedded) buffer is supported".into());
        }
        let off = opt_usize(bv, "byteOffset")?.unwrap_or(0);
        let len = opt_usize(bv, "byteLength")?.ok_or("bufferView has no byteLength")?;
        let end = off.checked_add(len).ok_or("bufferView range overflows")?;
        let slice = self.bin.get(off..end).ok_or("bufferView lies outside the BIN chunk")?;
        Ok((slice, opt_usize(bv, "byteStride")?))
    }

    fn accessor(&self, i: usize, ncomp: usize, allowed: &[u32]) -> Result<Accessor, String> {
        let a = arr(self.json, "accessors").get(i).ok_or("accessor index out of range")?;
        if a.get("sparse").is_some() {
            return Err("sparse accessors are not supported".into());
        }
        let ty = match a.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            _ => return Err("accessor has an unsupported type".into()),
        };
        if ty != ncomp {
            return Err("accessor type does not fit its use".into());
        }
        let comp_type = opt_usize(a, "componentType")?.and_then(|c| u32::try_from(c).ok()).ok_or("accessor has no componentType")?;
        let csize = match comp_type {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return Err("accessor has an unsupported componentType".into()),
        };
        if !allowed.contains(&comp_type) {
            return Err("accessor component type does not fit its use".into());
        }
        let count = opt_usize(a, "count")?.ok_or("accessor has no count")?;
        if count > MAX_ACCESSOR_COUNT {
            return Err("accessor is too large".into());
        }
        let normalized = a.get("normalized").and_then(Value::as_bool).unwrap_or(false);
        let bv = opt_usize(a, "bufferView")?.ok_or("accessors without a bufferView are not supported")?;
        let (slice, stride) = self.buffer_view(bv)?;
        let elem = ncomp * csize;
        let stride = stride.unwrap_or(elem);
        if stride < elem || stride > 252 {
            return Err("accessor byteStride is invalid".into());
        }
        let off = opt_usize(a, "byteOffset")?.unwrap_or(0);
        let mut data = Vec::with_capacity(count * ncomp);
        for e in 0..count {
            let base = e.checked_mul(stride).and_then(|v| v.checked_add(off)).ok_or("accessor range overflows")?;
            let item = slice.get(base..base.checked_add(elem).ok_or("accessor range overflows")?).ok_or("accessor lies outside its bufferView")?;
            for c in item.chunks_exact(csize) {
                let v = match (comp_type, c) {
                    (5120, [b]) => f64::from(*b as i8),
                    (5121, [b]) => f64::from(*b),
                    (5122, [a, b]) => f64::from(i16::from_le_bytes([*a, *b])),
                    (5123, [a, b]) => f64::from(u16::from_le_bytes([*a, *b])),
                    (5125, [a, b, c, d]) => f64::from(u32::from_le_bytes([*a, *b, *c, *d])),
                    (5126, [a, b, c, d]) => f64::from(f32::from_le_bytes([*a, *b, *c, *d])),
                    _ => return Err("malformed accessor data".into()),
                };
                if !v.is_finite() {
                    return Err("accessor holds a NaN or infinite value".into());
                }
                data.push(v);
            }
        }
        Ok(Accessor { data, normalized, comp_type })
    }

    fn material(&self, prim: &Value) -> Result<Material, String> {
        let mut m = Material { base_color: [1.0; 4], metallic: 0.0, roughness: 0.8, image: None };
        let Some(mi) = opt_usize(prim, "material")? else { return Ok(m) };
        let mat = arr(self.json, "materials").get(mi).ok_or("material index out of range")?;
        m.metallic = 1.0;
        m.roughness = 1.0;
        let Some(pbr) = mat.get("pbrMetallicRoughness") else { return Ok(m) };
        if let Some(c) = num_array(pbr, "baseColorFactor", 4)? {
            m.base_color = [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32];
        }
        let factor = |key: &str, def: f32| -> Result<f32, String> {
            match pbr.get(key) {
                None => Ok(def),
                Some(v) => v.as_f64().filter(|f| f.is_finite()).map(|f| f as f32).ok_or_else(|| format!("`{key}` is not a finite number")),
            }
        };
        m.metallic = factor("metallicFactor", 1.0)?;
        m.roughness = factor("roughnessFactor", 1.0)?;
        if let Some(bt) = pbr.get("baseColorTexture") {
            if opt_usize(bt, "texCoord")?.unwrap_or(0) == 0 {
                let ti = opt_usize(bt, "index")?.ok_or("baseColorTexture has no index")?;
                let tex = arr(self.json, "textures").get(ti).ok_or("texture index out of range")?;
                m.image = opt_usize(tex, "source")?;
            }
        }
        Ok(m)
    }

    /// The decoded texture for image `i`, decoding it the first time.
    fn texture(&mut self, i: usize) -> Result<u32, String> {
        if let Some(t) = self.tex_map.get(&i) {
            return Ok(*t);
        }
        let img = arr(self.json, "images").get(i).ok_or("image index out of range")?;
        let mime = img.get("mimeType").and_then(Value::as_str);
        if !matches!(mime, Some("image/png" | "image/jpeg")) {
            return Err("images must be image/png or image/jpeg".into());
        }
        let bv = opt_usize(img, "bufferView")?.ok_or("images must live in a bufferView")?;
        let (bytes, _) = self.buffer_view(bv)?;
        let decoded = read_image(bytes, TEXTURE_SIDE)?;
        let idx = u32::try_from(self.textures.len()).map_err(|_| "too many textures".to_string())?;
        self.textures.push(decoded);
        self.tex_map.insert(i, idx);
        Ok(idx)
    }

    fn add_primitive(&mut self, mesh: &str, pi: usize, prim: &Value, world: &Mat) -> Result<(), String> {
        if opt_usize(prim, "mode")?.unwrap_or(4) != 4 {
            return Ok(());
        }
        let attrs = prim.get("attributes").ok_or("primitive has no attributes")?;
        let pos_i = opt_usize(attrs, "POSITION")?.ok_or("primitive has no POSITION")?;
        let pos = self.accessor(pos_i, 3, &[5126])?;
        let n = pos.data.len() / 3;
        let normals = match opt_usize(attrs, "NORMAL")? {
            Some(i) => {
                let a = self.accessor(i, 3, &[5126])?;
                if a.data.len() != pos.data.len() {
                    return Err("NORMAL count differs from POSITION".into());
                }
                Some(a.data)
            }
            None => None,
        };
        let uvs = match opt_usize(attrs, "TEXCOORD_0")? {
            Some(i) => {
                let a = self.accessor(i, 2, &[5126, 5121, 5123])?;
                if a.data.len() != n * 2 {
                    return Err("TEXCOORD_0 count differs from POSITION".into());
                }
                let div = match a.comp_type {
                    5126 => 1.0,
                    _ if !a.normalized => return Err("integer TEXCOORD_0 must be normalized".into()),
                    5121 => 255.0,
                    _ => 65535.0,
                };
                Some(a.data.iter().map(|v| (v / div) as f32).collect::<Vec<f32>>())
            }
            None => None,
        };
        let mut indices: Vec<u32> = match opt_usize(prim, "indices")? {
            Some(i) => {
                let a = self.accessor(i, 1, &[5121, 5123, 5125])?;
                a.data.iter().map(|v| *v as u32).collect()
            }
            None => (0..u32::try_from(n).map_err(|_| "too many vertices".to_string())?).collect(),
        };
        if indices.len() % 3 != 0 {
            return Err("index count is not a multiple of 3".into());
        }
        if indices.iter().any(|i| *i as usize >= n) {
            return Err("an index points past the vertex count".into());
        }
        self.tris += indices.len() / 3;
        self.verts += n;
        if self.tris > MAX_SYMBOL_TRIANGLES || self.verts > MAX_SYMBOL_TRIANGLES * 3 {
            return Err(format!("the mesh has more than {MAX_SYMBOL_TRIANGLES} triangles"));
        }
        let mat = self.material(prim)?;
        let texture = match (mat.image, &uvs) {
            (Some(i), Some(_)) => Some(self.texture(i)?),
            _ => None,
        };
        // Transform: positions by the matrix, normals by the cofactor matrix.
        let m = world;
        let a0 = [m[0], m[1], m[2]];
        let a1 = [m[4], m[5], m[6]];
        let a2 = [m[8], m[9], m[10]];
        let c0 = cross(a1, a2);
        let c1 = cross(a2, a0);
        let c2 = cross(a0, a1);
        let det = a0[0] * c0[0] + a0[1] * c0[1] + a0[2] * c0[2];
        if det < 0.0 {
            for t in indices.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
        let sign = if det < 0.0 { -1.0 } else { 1.0 };
        let mut positions = Vec::with_capacity(n);
        for p in pos.data.chunks_exact(3) {
            let (x, y, z) = (p[0], p[1], p[2]);
            let q = [m[0] * x + m[4] * y + m[8] * z + m[12], m[1] * x + m[5] * y + m[9] * z + m[13], m[2] * x + m[6] * y + m[10] * z + m[14]];
            if q.iter().any(|v| !v.is_finite() || v.abs() > 1e12) {
                return Err("transformed positions are not finite".into());
            }
            positions.push([q[0] as f32, q[1] as f32, q[2] as f32]);
        }
        let normals = match normals {
            Some(nd) => {
                let mut out = Vec::with_capacity(n);
                for v in nd.chunks_exact(3) {
                    let r = [
                        sign * (v[0] * c0[0] + v[1] * c1[0] + v[2] * c2[0]),
                        sign * (v[0] * c0[1] + v[1] * c1[1] + v[2] * c2[1]),
                        sign * (v[0] * c0[2] + v[1] * c1[2] + v[2] * c2[2]),
                    ];
                    let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
                    out.push(if l > 1e-12 && l.is_finite() { [(r[0] / l) as f32, (r[1] / l) as f32, (r[2] / l) as f32] } else { [0.0, 0.0, 1.0] });
                }
                out
            }
            None => vertex_normals(&positions, &indices),
        };
        self.parts.push(SymbolPart {
            name: format!("{mesh}-{pi}"),
            positions,
            normals,
            uvs: match (uvs, texture) {
                (Some(u), _) => u.chunks_exact(2).map(|c| [c[0], c[1]]).collect(),
                _ => Vec::new(),
            },
            indices,
            base_color: mat.base_color,
            metallic: mat.metallic,
            roughness: mat.roughness,
            texture,
            ..Default::default()
        });
        Ok(())
    }
}

/// Area-weighted vertex normals.
fn vertex_normals(pos: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut acc = vec![[0f64; 3]; pos.len()];
    let p = |i: u32| pos.get(i as usize).map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]);
    for t in indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (p(t[0]), p(t[1]), p(t[2])) else { continue };
        let n = cross([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        for i in t {
            if let Some(s) = acc.get_mut(*i as usize) {
                for k in 0..3 {
                    s[k] += n[k];
                }
            }
        }
    }
    acc.iter()
        .map(|v| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 1e-18 && l.is_finite() { [(v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32] } else { [0.0, 0.0, 1.0] }
        })
        .collect()
}

/// Move the whole mesh so x and y are centred on 0 and the lowest z is 0.
fn recentre(parts: &mut [SymbolPart]) -> Result<(), String> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in parts.iter().flat_map(|p| p.positions.iter()) {
        for k in 0..3 {
            if !p[k].is_finite() {
                return Err("the mesh holds a non-finite position".into());
            }
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo.iter().any(|v| !v.is_finite()) {
        return Err("the mesh is empty".into());
    }
    let shift = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]];
    for p in parts.iter_mut().flat_map(|p| p.positions.iter_mut()) {
        for k in 0..3 {
            p[k] -= shift[k];
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    fn glb(json: &Value, bin: &[u8]) -> Vec<u8> {
        let mut j = json.to_string().into_bytes();
        while j.len() % 4 != 0 {
            j.push(b' ');
        }
        let mut b = bin.to_vec();
        while b.len() % 4 != 0 {
            b.push(0);
        }
        let mut total = 12 + 8 + j.len();
        if !b.is_empty() {
            total += 8 + b.len();
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.extend_from_slice(&j);
        if !b.is_empty() {
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
            out.extend_from_slice(&b);
        }
        out
    }

    /// Accumulates BIN data, bufferViews and accessors.
    #[derive(Default)]
    struct Builder {
        bin: Vec<u8>,
        views: Vec<Value>,
        accessors: Vec<Value>,
    }

    impl Builder {
        fn view(&mut self, bytes: &[u8]) -> usize {
            while self.bin.len() % 4 != 0 {
                self.bin.push(0);
            }
            self.views.push(json!({"buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len()}));
            self.bin.extend_from_slice(bytes);
            self.views.len() - 1
        }
        fn acc(&mut self, bytes: &[u8], comp: u32, count: usize, ty: &str, extra: Value) -> usize {
            let v = self.view(bytes);
            let mut a = json!({"bufferView": v, "componentType": comp, "count": count, "type": ty});
            if let (Some(o), Some(e)) = (a.as_object_mut(), extra.as_object()) {
                for (k, val) in e {
                    o.insert(k.clone(), val.clone());
                }
            }
            self.accessors.push(a);
            self.accessors.len() - 1
        }
        fn floats(&mut self, data: &[f32], count: usize, ty: &str) -> usize {
            let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
            self.acc(&bytes, 5126, count, ty, json!({}))
        }
        fn finish(self, mut doc: Value) -> Vec<u8> {
            doc["asset"] = json!({"version": "2.0"});
            doc["buffers"] = json!([{"byteLength": self.bin.len()}]);
            doc["bufferViews"] = Value::Array(self.views);
            doc["accessors"] = Value::Array(self.accessors);
            glb(&doc, &self.bin)
        }
    }

    const CUBE_CORNERS: [[f32; 3]; 8] = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    const CUBE_TRIS: [u16; 36] = [0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 1, 2, 6, 1, 6, 5, 3, 0, 4, 3, 4, 7];

    /// A glTF-space box of the given size (x, y, z) with a node translation; u16 indices.
    fn box_glb(size: [f32; 3], translation: [f64; 3], with_normals: bool) -> Vec<u8> {
        let mut b = Builder::default();
        let pos: Vec<f32> = CUBE_CORNERS.iter().flat_map(|c| (0..3).map(move |k| c[k] * size[k])).collect();
        let p = b.floats(&pos, 8, "VEC3");
        let idx: Vec<u8> = CUBE_TRIS.iter().flat_map(|i| i.to_le_bytes()).collect();
        let i = b.acc(&idx, 5123, 36, "SCALAR", json!({}));
        let mut attrs = json!({"POSITION": p});
        if with_normals {
            let n: Vec<f32> = CUBE_CORNERS.iter().flat_map(|_| [0.0, 1.0, 0.0]).collect();
            attrs["NORMAL"] = json!(b.floats(&n, 8, "VEC3"));
        }
        b.finish(json!({
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0, "translation": translation}],
            "meshes": [{"name": "box", "primitives": [{"attributes": attrs, "indices": i}]}],
        }))
    }

    fn quad_glb(png: &[u8], prims: usize) -> Vec<u8> {
        let mut b = Builder::default();
        let p = b.floats(&[-1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0], 4, "VEC3");
        let uv = b.floats(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0], 4, "VEC2");
        let idx: Vec<u8> = [0u8, 1, 2, 0, 2, 3].to_vec();
        let i = b.acc(&idx, 5121, 6, "SCALAR", json!({}));
        let img = b.view(png);
        let prim = json!({"attributes": {"POSITION": p, "TEXCOORD_0": uv}, "indices": i, "material": 0});
        b.finish(json!({
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": vec![prim; prims]}],
            "materials": [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "baseColorFactor": [0.5, 0.5, 0.5, 1.0], "metallicFactor": 0.25}}],
            "textures": [{"source": 0}],
            "images": [{"bufferView": img, "mimeType": "image/png"}],
        }))
    }

    fn png_2x2() -> Vec<u8> {
        let img = image::RgbaImage::from_fn(2, 2, |x, y| image::Rgba([(x * 200) as u8, (y * 200) as u8, 10, 255]));
        let mut out = Vec::new();
        assert!(img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).is_ok());
        out
    }

    fn bounds(m: &SymbolMesh) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in m.parts.iter().flat_map(|p| &p.positions) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn reads_a_cube_and_bottom_centres_it() {
        let bytes = box_glb([1.0, 1.0, 1.0], [3.0, 5.0, -2.0], true);
        let m = read_glb(&bytes).expect("reads");
        assert_eq!(m.key, fnv1a64_hex(&bytes));
        assert_eq!(m.key.len(), 16);
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.parts[0].name, "box-0");
        assert_eq!(m.triangles(), 12);
        assert_eq!(m.parts[0].base_color, [1.0; 4]);
        assert_eq!((m.parts[0].metallic, m.parts[0].roughness), (0.0, 0.8));
        for (a, b) in m.extents().iter().zip([1.0, 1.0, 1.0]) {
            assert!((a - b).abs() < 1e-6);
        }
        let (lo, hi) = bounds(&m);
        assert!((lo[0] + 0.5).abs() < 1e-6 && (hi[0] - 0.5).abs() < 1e-6);
        assert!((lo[1] + 0.5).abs() < 1e-6 && (hi[1] - 0.5).abs() < 1e-6);
        assert!(lo[2].abs() < 1e-6 && (hi[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn turns_y_up_to_z_up() {
        // 1 wide (x), 2 tall (glTF y), 3 deep (glTF z).
        let m = read_glb(&box_glb([1.0, 2.0, 3.0], [0.0; 3], true)).expect("reads");
        let e = m.extents();
        assert!((e[0] - 1.0).abs() < 1e-6 && (e[1] - 3.0).abs() < 1e-6 && (e[2] - 2.0).abs() < 1e-6, "{e:?}");
        // The supplied normal (glTF +y, up) is now +z.
        assert!(m.parts[0].normals.iter().all(|n| n[0].abs() < 1e-6 && n[1].abs() < 1e-6 && (n[2] - 1.0).abs() < 1e-6));
    }

    #[test]
    fn applies_node_trs_and_matrices() {
        let mut b = Builder::default();
        let pos: Vec<f32> = CUBE_CORNERS.iter().flatten().copied().collect();
        let p = b.floats(&pos, 8, "VEC3");
        let idx: Vec<u8> = CUBE_TRIS.iter().flat_map(|i| i.to_le_bytes()).collect();
        let i = b.acc(&idx, 5123, 36, "SCALAR", json!({}));
        let prim = json!({"attributes": {"POSITION": p}, "indices": i});
        let s = std::f64::consts::FRAC_1_SQRT_2;
        // Parent scales x by 2 and rotates 90 degrees about glTF y; child (matrix) translates by y 1.
        let bytes = b.finish(json!({
            "scenes": [{"nodes": [0, 1]}],
            "nodes": [
                {"children": [2], "scale": [2.0, 1.0, 1.0], "rotation": [0.0, s, 0.0, s], "translation": [10.0, 0.0, 0.0]},
                {"mesh": 0, "matrix": [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, -20.0,0.0,0.0,1.0]},
                {"mesh": 0, "matrix": [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,1.0,0.0,1.0]},
            ],
            "meshes": [{"primitives": [prim]}],
        }));
        let m = read_glb(&bytes).expect("reads");
        assert_eq!(m.parts.len(), 2);
        // Rotating 90 degrees about y swaps the scaled x extent into glTF z (-> plan -y), so the
        // rotated child spans 1 (x) by 2 (plan y) by 1 (z) at x = 10; the other cube sits at x = -20.
        let e = m.extents();
        assert!((e[0] - 31.0).abs() < 1e-4, "{e:?}");
        assert!((e[1] - 2.0).abs() < 1e-4, "{e:?}");
        assert!((e[2] - 2.0).abs() < 1e-4, "{e:?}");
        assert_eq!(m.parts[0].name, "mesh-0");
    }

    #[test]
    fn reads_u8_u16_u32_indices() {
        for comp in [5121u32, 5123, 5125] {
            let mut b = Builder::default();
            let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
            let ids = [0u32, 1, 2];
            let bytes: Vec<u8> = match comp {
                5121 => ids.iter().map(|i| *i as u8).collect(),
                5123 => ids.iter().flat_map(|i| (*i as u16).to_le_bytes()).collect(),
                _ => ids.iter().flat_map(|i| i.to_le_bytes()).collect(),
            };
            let i = b.acc(&bytes, comp, 3, "SCALAR", json!({}));
            let g = b.finish(json!({
                "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
                "meshes": [{"primitives": [{"attributes": {"POSITION": p}, "indices": i}]}],
            }));
            let m = read_glb(&g).expect("reads");
            assert_eq!(m.parts[0].indices, vec![0, 1, 2], "component type {comp}");
        }
        // No indices: 0..n.
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}]}));
        assert_eq!(read_glb(&g).expect("reads").parts[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn computes_normals_when_absent() {
        // glTF +z face (front) becomes plan -y.
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}]}));
        let m = read_glb(&g).expect("reads");
        for n in &m.parts[0].normals {
            assert!(n[0].abs() < 1e-6 && (n[1] + 1.0).abs() < 1e-6 && n[2].abs() < 1e-6, "{n:?}");
        }
        let cube = read_glb(&box_glb([1.0; 3], [0.0; 3], false)).expect("reads");
        assert_eq!(cube.parts[0].normals.len(), 8);
        assert!(cube.parts[0].normals.iter().all(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (l - 1.0).abs() < 1e-5
        }));
    }

    #[test]
    fn decodes_a_base_colour_texture_once() {
        let m = read_glb(&quad_glb(&png_2x2(), 2)).expect("reads");
        assert_eq!(m.parts.len(), 2);
        assert_eq!(m.textures.len(), 1);
        assert_eq!((m.textures[0].width, m.textures[0].height), (2, 2));
        assert!(m.textures[0].is_valid());
        for p in &m.parts {
            assert_eq!(p.texture, Some(0));
            assert_eq!(p.uvs.len(), 4);
            assert_eq!(p.base_color, [0.5, 0.5, 0.5, 1.0]);
            assert_eq!(p.metallic, 0.25);
            assert_eq!(p.roughness, 1.0);
        }
        assert_eq!(m.parts[1].name, "mesh-1");
        // Not a PNG/JPEG: refused.
        assert!(read_glb(&quad_glb(b"GIF89a....", 1)).is_err());
    }

    #[test]
    fn refuses_draco_meshopt_and_ktx2_naming_them() {
        for (ext, word) in [
            ("KHR_draco_mesh_compression", "Draco"),
            ("EXT_meshopt_compression", "meshopt"),
            ("KHR_texture_basisu", "KTX2"),
            ("VENDOR_odd", "VENDOR_odd"),
        ] {
            let mut b = Builder::default();
            let p = b.floats(&[0.0; 9], 3, "VEC3");
            let g = b.finish(json!({
                "extensionsRequired": [ext], "extensionsUsed": [ext],
                "nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}],
            }));
            let e = read_glb(&g).expect_err("refused");
            assert!(e.contains(ext) && e.contains(word) && e.contains("not supported"), "{e}");
        }
        // extensionsUsed alone is fine.
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let g = b.finish(json!({
            "extensionsUsed": ["KHR_materials_unlit"],
            "nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}],
        }));
        assert!(read_glb(&g).is_ok());
    }

    #[test]
    fn refuses_external_and_data_uris() {
        let ok = |doc: Value| {
            let mut b = Builder::default();
            let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
            let mut d = doc;
            d["nodes"] = json!([{"mesh": 0}]);
            d["meshes"] = json!([{"primitives": [{"attributes": {"POSITION": p}}]}]);
            let mut g = b.finish(d.clone());
            // Re-inject extra members on top of the finished document.
            let (mut j, bin) = split_chunks(&g).map(|(j, b)| (j, b.to_vec())).expect("splits");
            for k in ["images"] {
                if let Some(v) = d.get(k) {
                    j[k] = v.clone();
                }
            }
            if let Some(b0) = d.get("buffers") {
                j["buffers"] = b0.clone();
            }
            g = glb(&j, &bin);
            read_glb(&g)
        };
        let e = ok(json!({"buffers": [{"byteLength": 4, "uri": "data:application/octet-stream;base64,AAAA"}]})).expect_err("data uri");
        assert!(e.contains("uri"), "{e}");
        let e = ok(json!({"buffers": [{"byteLength": 4, "uri": "extra.bin"}]})).expect_err("external");
        assert!(e.contains("uri"), "{e}");
        let e = ok(json!({"images": [{"uri": "a.png"}]})).expect_err("image uri");
        assert!(e.contains("uri"), "{e}");
    }

    #[test]
    fn refuses_truncated_and_misaligned_files() {
        let g = box_glb([1.0; 3], [0.0; 3], true);
        assert!(read_glb(&g).is_ok());
        assert!(read_glb(&[]).is_err());
        assert!(read_glb(&g[..11]).is_err());
        for cut in [12, 20, 40, g.len() - 4, g.len() - 1] {
            assert!(read_glb(&g[..cut]).is_err(), "cut at {cut}");
        }
        // Trailing byte: the header length no longer matches.
        let mut longer = g.clone();
        longer.push(0);
        assert!(read_glb(&longer).is_err());
        // Bad magic and version.
        let mut bad = g.clone();
        bad[0] = b'X';
        assert!(read_glb(&bad).is_err());
        let mut bad = g.clone();
        bad[4] = 1;
        assert!(read_glb(&bad).is_err());
        // A JSON chunk whose length is not a multiple of 4.
        let mut j = br#"{"asset":{"version":"2.0"}}"#.to_vec();
        assert!(j.len() % 4 != 0);
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((20 + j.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.append(&mut j);
        let e = read_glb(&out).expect_err("misaligned");
        assert!(e.contains("multiple of 4"), "{e}");
        // A chunk claiming more bytes than exist.
        let mut huge = g.clone();
        huge[12..16].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        assert!(read_glb(&huge).is_err());
    }

    #[test]
    fn refuses_out_of_range_indices_and_nans() {
        let build = |pos: &[f32], idx: &[u16]| {
            let mut b = Builder::default();
            let p = b.floats(pos, pos.len() / 3, "VEC3");
            let bytes: Vec<u8> = idx.iter().flat_map(|i| i.to_le_bytes()).collect();
            let i = b.acc(&bytes, 5123, idx.len(), "SCALAR", json!({}));
            b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}, "indices": i}]}]}))
        };
        let tri = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        assert!(read_glb(&build(&tri, &[0, 1, 2])).is_ok());
        assert!(read_glb(&build(&tri, &[0, 1, 3])).is_err());
        assert!(read_glb(&build(&tri, &[0, 1])).is_err());
        let mut nan = tri;
        nan[4] = f32::NAN;
        assert!(read_glb(&build(&nan, &[0, 1, 2])).is_err());
        let mut inf = tri;
        inf[3] = f32::INFINITY;
        assert!(read_glb(&build(&inf, &[0, 1, 2])).is_err());
        // An accessor reaching past its bufferView.
        let mut b = Builder::default();
        let p = b.floats(&tri, 3, "VEC3");
        b.accessors[p]["count"] = json!(30);
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}]}));
        assert!(read_glb(&g).is_err());
        // Empty meshes, sparse accessors and cyclic nodes.
        let g = glb(&json!({"asset": {"version": "2.0"}}), &[]);
        assert!(read_glb(&g).is_err());
        let mut b = Builder::default();
        let p = b.floats(&tri, 3, "VEC3");
        b.accessors[p]["sparse"] = json!({"count": 1});
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}]}));
        assert!(read_glb(&g).is_err());
        let mut b = Builder::default();
        let p = b.floats(&tri, 3, "VEC3");
        let g = b.finish(json!({
            "nodes": [{"mesh": 0, "children": [1]}, {"children": [0]}],
            "scenes": [{"nodes": [0]}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": p}}]}],
        }));
        assert!(read_glb(&g).is_err());
    }

    #[test]
    fn buffer_view_defaults_to_buffer_zero() {
        let mut b = Builder::default();
        let bytes: Vec<u8> = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let bv = b.view(&bytes);
        let acc = b.acc(&bytes, 5126, 3, "VEC3", json!({}));
        if let Some(obj) = b.views[bv].as_object_mut() {
            obj.remove("buffer");
        }
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [{"attributes": {"POSITION": acc}}]}]}));
        assert!(read_glb(&g).is_ok());
    }

    #[test]
    fn skips_primitives_with_mode_other_than_triangles() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}, "mode": 1});
        let g = b.finish(json!({"nodes": [{"mesh": 0}], "meshes": [{"primitives": [prim]}]}));
        let e = read_glb(&g).expect_err("no triangles");
        assert!(e.contains("no triangles"), "{e}");
    }

    #[test]
    fn material_without_pbr_defaults_to_metallic_rough() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}, "material": 0});
        let g = b.finish(json!({
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [prim]}],
            "materials": [{}],
        }));
        let m = read_glb(&g).expect("reads");
        assert_eq!(m.parts[0].metallic, 1.0);
        assert_eq!(m.parts[0].roughness, 1.0);
        assert_eq!(m.parts[0].base_color, [1.0; 4]);
    }

    #[test]
    fn refuses_position_accessor_with_wrong_type() {
        let mut b = Builder::default();
        let bytes: Vec<u8> = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let bv = b.view(&bytes);
        let acc = b.acc(&bytes, 5126, 3, "VEC2", json!({}));
        let g = b.finish(json!({
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": acc}}]}],
        }));
        let e = read_glb(&g).expect_err("wrong type");
        assert!(e.contains("type"), "{e}");
    }

    #[test]
    fn node_with_no_mesh_is_skipped() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}});
        let g = b.finish(json!({
            "scenes": [{"nodes": [0, 1]}],
            "nodes": [{"name": "empty"}, {"mesh": 0}],
            "meshes": [{"primitives": [prim]}],
        }));
        let m = read_glb(&g).expect("reads");
        assert_eq!(m.parts.len(), 1);
    }

    #[test]
    fn node_with_zero_scale_collapses_mesh() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}});
        let g = b.finish(json!({
            "nodes": [{"mesh": 0, "scale": [0.0, 0.0, 0.0]}],
            "meshes": [{"primitives": [prim]}],
        }));
        let m = read_glb(&g).expect("reads");
        let e = m.extents();
        assert!(e[0].abs() < 1e-6 && e[1].abs() < 1e-6 && e[2].abs() < 1e-6, "{e:?}");
    }

    #[test]
    fn reads_a_single_triangle() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}});
        let g = b.finish(json!({
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [prim]}],
        }));
        let m = read_glb(&g).expect("reads");
        assert_eq!(m.triangles(), 1);
        assert_eq!(m.parts[0].positions.len(), 3);
    }

    #[test]
    fn negative_scale_fixes_winding() {
        let mut b = Builder::default();
        let p = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, "VEC3");
        let prim = json!({"attributes": {"POSITION": p}});
        let g = b.finish(json!({
            "nodes": [{"mesh": 0, "scale": [-1.0, 1.0, 1.0]}],
            "meshes": [{"primitives": [prim]}],
        }));
        let m = read_glb(&g).expect("reads");
        assert_eq!(m.parts[0].indices, vec![0, 2, 1]);
    }

    #[test]
    fn reads_the_sample_dishwasher_at_its_size() {
        let bytes = include_bytes!("../../../assets/library/sample/dishwasher-60.glb");
        let m = read_glb(bytes).expect("reads the sample");
        let e = m.extents();
        for (got, want) in e.iter().zip([0.6, 0.6, 0.876]) {
            assert!((got - want).abs() < 1e-4, "{e:?}");
        }
        let (lo, hi) = bounds(&m);
        assert!(lo[2].abs() < 1e-6);
        assert!((lo[0] + hi[0]).abs() < 1e-4 && (lo[1] + hi[1]).abs() < 1e-4);
        assert!(m.triangles() > 0 && m.triangles() <= MAX_SYMBOL_TRIANGLES);
        assert_eq!(read_glb(bytes).expect("again"), m);
    }
}
