//! The finish and library textures of the mesh pipelines (phase 13/14): one `texture_2d_array` bound as group 3.
//!
//! Layer 0 is white (every untextured mesh samples it, so its image is unchanged); layers 1..=4 are
//! [`cadcraft_arch3d::texture::texture`] for [`Finish::ALL`] in order. Subsequent layers are
//! library textures from `Scene3d.textures` (resampled to 256x256). The chain of mip levels is
//! built on the CPU with a box filter, and one sampler repeats, filters linearly and blends mips.

use crate::types::Scene3d;
use cadcraft_arch3d::texture::{TEXTURE_SIZE, texture};
use cadcraft_doc::arch::{Finish, ImageRgba};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

/// Layers in the array: white plus one per finish.
pub const BASE_LAYERS: u32 = 1 + Finish::ALL.len() as u32;
/// Max extra layers from library textures.
pub const MAX_EXTRA_LAYERS: u32 = 251;
/// Total layers in the texture array.
pub const LAYERS: u32 = BASE_LAYERS + MAX_EXTRA_LAYERS;
/// Mip levels of every layer (256 down to 1).
pub const MIP_LEVELS: u32 = TEXTURE_SIZE.ilog2() + 1;
/// Texture format of the array.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The finish a material shows: `wall-<f>`, `roof-<f>` or `floor-<f>` (case-insensitive).
pub fn finish_of(material: &str) -> Option<Finish> {
    let m = material.to_ascii_lowercase();
    let name = ["wall-", "roof-", "floor-"].iter().find_map(|p| m.strip_prefix(p))?;
    Finish::parse(name)
}

/// The texture key a material uses, if any.
///
/// Returns `Some(key)` if the material is a `MaterialDef` with a texture, or if it is a finish.
/// Returns `None` if the material uses no texture or falls back to white.
pub fn texture_key_for(material: &str, scene: &Scene3d) -> Option<String> {
    if let Some(def) = scene.materials.get(material) {
        return def.texture.clone();
    }
    // Finishes are handled by layer_for directly, but we can map them to keys if needed for consistency
    // For now, finishes are static layers 1-4.
    None
}

/// Array layer of a material: 1 + the finish index for `wall-<f>` / `roof-<f>` / `floor-<f>`,
/// or the index of its library texture, else 0 (white).
pub fn layer_for(material: &str, texture_layers: &BTreeMap<String, u32>) -> u32 {
    if let Some(&layer) = texture_layers.get(material) {
        return layer;
    }
    if let Some(f) = finish_of(material) {
        return Finish::ALL.iter().position(|a| *a == f).map_or(0, |i| i as u32 + 1);
    }
    0
}

/// Compute the texture layers for a scene.
///
/// Returns a map from material name (or texture key) to array layer index.
/// Layer 0 is white. Layers 1-4 are finishes. Layers 5+ are library textures.
pub fn layers_for_scene(scene: &Scene3d) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();

    // Map finishes to their static layers
    for (i, f) in Finish::ALL.iter().enumerate() {
        for prefix in ["wall-", "roof-", "floor-"] {
            map.insert(format!("{prefix}{}", f.name()), (i as u32) + 1);
        }
    }

    // Map library textures
    let mut next_layer = BASE_LAYERS;
    for (key, _img) in scene.textures.iter() {
        if next_layer >= BASE_LAYERS + MAX_EXTRA_LAYERS {
            break;
        }
        map.insert(key.clone(), next_layer);
        next_layer += 1;
    }

    // Map materials that reference these textures
    for (mat_name, def) in scene.materials.iter() {
        if let Some(tex_key) = &def.texture {
            if let Some(&layer) = map.get(tex_key) {
                map.insert(mat_name.clone(), layer);
            }
        }
    }

    map
}

/// The next mip level of an RGBA8 image of side `side` (box filter over 2 x 2 texels).
fn downsample(src: &[u8], side: usize) -> Vec<u8> {
    let half = (side / 2).max(1);
    let mut out = Vec::with_capacity(half * half * 4);
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let mut sum = 0u32;
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let (sy, sx) = ((2 * y + dy).min(side - 1), (2 * x + dx).min(side - 1));
                    sum += u32::from(src.get((sy * side + sx) * 4 + c).copied().unwrap_or(0));
                }
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    out
}

/// All mip levels of one layer, level 0 first.
fn mip_chain(base: Vec<u8>) -> Vec<Vec<u8>> {
    let mut levels = vec![base];
    let mut side = TEXTURE_SIZE as usize;
    while side > 1 {
        let next = downsample(levels.last().map_or(&[][..], |l| l.as_slice()), side);
        levels.push(next);
        side /= 2;
    }
    levels
}

/// Resample an image to 256x256 bilinearly.
fn resample_to_256(img: &ImageRgba) -> Vec<u8> {
    let w = img.width as usize;
    let h = img.height as usize;
    let mut out = vec![0u8; TEXTURE_SIZE as usize * TEXTURE_SIZE as usize * 4];

    if w == 0 || h == 0 {
        return out;
    }

    let scale_x = (w as f32) / (TEXTURE_SIZE as f32);
    let scale_y = (h as f32) / (TEXTURE_SIZE as f32);

    for y in 0..TEXTURE_SIZE as usize {
        for x in 0..TEXTURE_SIZE as usize {
            let src_x = (x as f32) * scale_x;
            let src_y = (y as f32) * scale_y;

            let x0 = src_x.floor() as usize;
            let y0 = src_y.floor() as usize;
            let x1 = (x0 + 1).min(w - 1);
            let y1 = (y0 + 1).min(h - 1);

            let fx = src_x - x0 as f32;
            let fy = src_y - y0 as f32;

            let get_px = |ix: usize, iy: usize| -> [u8; 4] {
                let idx = (iy * w + ix) * 4;
                [
                    img.rgba.get(idx).copied().unwrap_or(0),
                    img.rgba.get(idx + 1).copied().unwrap_or(0),
                    img.rgba.get(idx + 2).copied().unwrap_or(0),
                    img.rgba.get(idx + 3).copied().unwrap_or(0),
                ]
            };

            let p00 = get_px(x0, y0);
            let p10 = get_px(x1, y0);
            let p01 = get_px(x0, y1);
            let p11 = get_px(x1, y1);

            let lerp = |a: u8, b: u8, t: f32| -> u8 { (a as f32 * (1.0 - t) + b as f32 * t).round() as u8 };

            let r = lerp(lerp(p00[0], p10[0], fx), lerp(p01[0], p11[0], fx), fy);
            let g = lerp(lerp(p00[1], p10[1], fx), lerp(p01[1], p11[1], fx), fy);
            let b = lerp(lerp(p00[2], p10[2], fx), lerp(p01[2], p11[2], fx), fy);
            let a = lerp(lerp(p00[3], p10[3], fx), lerp(p01[3], p11[3], fx), fy);

            let out_idx = (y * TEXTURE_SIZE as usize + x) * 4;
            out[out_idx] = r;
            out[out_idx + 1] = g;
            out[out_idx + 2] = b;
            out[out_idx + 3] = a;
        }
    }
    out
}

/// The texel data in `LayerMajor` order: for each layer, its mips from the largest down.
pub fn array_data(scene: &Scene3d) -> Vec<u8> {
    let side = TEXTURE_SIZE as usize;
    let mut layers = vec![vec![255u8; side * side * 4]];
    layers.extend(Finish::ALL.iter().map(|f| texture(*f).rgba));

    let texture_layers = layers_for_scene(scene);
    let mut next_layer = BASE_LAYERS;

    for (key, _img) in scene.textures.iter() {
        if next_layer >= BASE_LAYERS + MAX_EXTRA_LAYERS {
            break;
        }
        if let Some(img) = scene.textures.get(key) {
            let resampled = resample_to_256(img);
            layers.push(resampled);
        } else {
            layers.push(vec![0u8; side * side * 4]);
        }
        next_layer += 1;
    }

    layers.into_iter().flat_map(mip_chain).flatten().collect()
}

/// Layout of group 3: the texture array (binding 0) and its sampler (binding 1).
pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render3d material textures layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// The uploaded array and the group-3 bind group over it and its sampler.
pub struct MaterialTextures {
    /// The array texture.
    pub texture: wgpu::Texture,
    /// Group 3 over the array and the sampler.
    pub bind_group: wgpu::BindGroup,
    /// The keys of the textures in the array (excluding white and finishes), used for cache invalidation.
    keys: Vec<String>,
}

impl MaterialTextures {
    /// Upload the array (all mips) and build the bind group over `layout`.
    ///
    /// If `scene` is provided, library textures are included. If `None`, only white and finishes.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, scene: Option<&Scene3d>) -> Self {
        let (keys, layer_count) = if let Some(s) = scene {
            let layers = layers_for_scene(s);
            let mut keys: Vec<String> = s.textures.keys().cloned().collect();
            keys.sort();
            let count = BASE_LAYERS + keys.len().min(MAX_EXTRA_LAYERS as usize) as u32;
            (keys, count)
        } else {
            (Vec::new(), BASE_LAYERS)
        };

        let data = if let Some(s) = scene {
            array_data(s)
        } else {
            // Static data for no scene
            let side = TEXTURE_SIZE as usize;
            let mut layers = vec![vec![255u8; side * side * 4]];
            layers.extend(Finish::ALL.iter().map(|f| texture(*f).rgba));
            layers.into_iter().flat_map(mip_chain).flatten().collect()
        };

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("render3d material textures"),
                size: wgpu::Extent3d { width: TEXTURE_SIZE, height: TEXTURE_SIZE, depth_or_array_layers: layer_count },
                mip_level_count: MIP_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("render3d material sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render3d material textures group 3"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        MaterialTextures { texture, bind_group, keys }
    }

    /// Check if the texture array needs to be rebuilt for the given scene.
    pub fn needs_rebuild(&self, scene: &Scene3d) -> bool {
        let mut keys: Vec<String> = scene.textures.keys().cloned().collect();
        keys.sort();
        self.keys != keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_array_has_five_layers_with_mips() {
        let per_layer: usize = (0..MIP_LEVELS).map(|l| (256usize >> l).pow(2) * 4).sum();
        assert_eq!(BASE_LAYERS, 5);
        assert_eq!(MIP_LEVELS, 9);
        let scene = Scene3d::default();
        let data = array_data(&scene);
        assert_eq!(data.len(), per_layer * 5);
        assert!(data.iter().take(per_layer).all(|&b| b == 255), "layer 0 is white at every mip");
        assert_eq!(array_data(&scene), data, "deterministic");
        if let Some((device, queue)) = crate::gputest::shared_device() {
            let layout = bind_group_layout(&device);
            let t = MaterialTextures::new(&device, &queue, &layout, Some(&scene));
            assert_eq!(t.texture.depth_or_array_layers(), 5);
            assert_eq!(t.texture.mip_level_count(), 9);
            assert_eq!(t.texture.width(), 256);
            assert_eq!(t.texture.format(), TEXTURE_FORMAT);
        }
    }

    #[test]
    fn symbol_textures_get_array_layers() {
        use cadcraft_doc::arch::ImageRgba;
        use std::collections::BTreeMap;

        let mut scene = Scene3d::default();
        let mut textures = BTreeMap::new();
        textures.insert(
            "sym:test:t0".to_string(),
            ImageRgba { width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255] },
        );
        scene.textures = textures;

        let layers = layers_for_scene(&scene);
        assert_eq!(layers.get("sym:test:t0"), Some(&5));

        let data = array_data(&scene);
        let per_layer: usize = (0..MIP_LEVELS).map(|l| (256usize >> l).pow(2) * 4).sum();
        assert_eq!(data.len(), per_layer * 6); // 5 base + 1 texture
    }

    #[test]
    fn texture_array_rebuilds_only_on_new_keys() {
        use cadcraft_doc::arch::ImageRgba;
        use std::collections::BTreeMap;

        let mut scene = Scene3d::default();
        let mut textures = BTreeMap::new();
        textures.insert("sym:test:t0".to_string(), ImageRgba { width: 1, height: 1, rgba: vec![0, 0, 0, 255] });
        scene.textures = textures;

        if let Some((device, queue)) = crate::gputest::shared_device() {
            let layout = bind_group_layout(&device);
            let t = MaterialTextures::new(&device, &queue, &layout, Some(&scene));
            assert!(!t.needs_rebuild(&scene));

            scene.textures.insert("sym:test:t1".to_string(), ImageRgba { width: 1, height: 1, rgba: vec![255, 255, 255, 255] });
            assert!(t.needs_rebuild(&scene));
        }
    }

    use crate::camera::{Camera, Orbit};
    use crate::light::Light;
    use crate::offscreen::Offscreen;
    use crate::types::{ArchId, Mesh};

    /// A 2 m square wall face through the origin, facing +y (running along x) or +x (along y), with
    /// face-aligned UVs for `material`'s finish.
    fn wall(material: &str, along_y: bool) -> crate::types::Scene3d {
        let mut m = Mesh::new(material, ArchId(1));
        let q: [[f32; 3]; 4] = if along_y {
            [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 1.0, 1.0], [0.0, -1.0, 1.0]]
        } else {
            [[1.0, 0.0, -1.0], [-1.0, 0.0, -1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, 1.0]]
        };
        for p in q {
            m.positions.push(p);
            m.normals.push(if along_y { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] });
            m.uvs.push([0.0, 0.0]);
        }
        m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        let tile = finish_of(material).map_or([1.0, 1.0], |f| texture(f).tile);
        let m = cadcraft_arch3d::uv::face_uvs(&m, tile);
        let mut s = crate::types::Scene3d { meshes: vec![m], ..Default::default() };
        s.bounds = crate::types::Bounds3::new([-1.0; 3], [1.0; 3]);
        s
    }

    /// Luminance (0..255) of the central 80 x 80 pixels of a 256 x 256 face-on render, or `None`
    /// without a device.
    fn centre(material: &str, along_y: bool) -> Option<Vec<Vec<f64>>> {
        let (device, queue) = crate::gputest::shared_device()?;
        let mut r = Offscreen::with_device(device, queue, (256, 256));
        let az = if along_y { 90.0 } else { 0.0 };
        r.render(&wall(material, along_y), 1, &Camera::Orbit(Orbit::at_origin(az, 0.0, 6.0, 45.0)), &Light::new([0.0, 0.0, 1.0], 0.9));
        let rgba = r.read_rgba().ok()?;
        let rows = (88..168usize)
            .map(|y| {
                (88..168usize)
                    .map(|x| {
                        let i = (y * 256 + x) * 4;
                        let px = |k: usize| f64::from(rgba.get(i + k).copied().unwrap_or(0));
                        0.2126 * px(0) + 0.7152 * px(1) + 0.0722 * px(2)
                    })
                    .collect()
            })
            .collect();
        Some(rows)
    }

    fn variance(v: &[f64]) -> f64 {
        let n = v.len().max(1) as f64;
        let mean = v.iter().sum::<f64>() / n;
        v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n
    }

    #[test]
    fn brick_wall_render_shows_mortar() {
        let (Some(brick), Some(plain)) = (centre("wall-brick", false), centre("wall", false)) else { return };
        let all = |g: &[Vec<f64>]| g.iter().flatten().copied().collect::<Vec<_>>();
        let (vb, vp) = (variance(&all(&brick)), variance(&all(&plain)));
        assert!(vp < 1.0, "an untextured wall is flat: {vp}");
        assert!(vb > 25.0 && vb > 20.0 * (vp + 1.0), "brick {vb} vs plain {vp}: no mortar lines");
    }

    #[test]
    fn siding_on_a_y_running_wall_is_horizontal() {
        let Some(g) = centre("wall-siding", true) else { return };
        let row_means: Vec<f64> = g.iter().map(|r| r.iter().sum::<f64>() / r.len().max(1) as f64).collect();
        let col_means: Vec<f64> = (0..g.first().map_or(0, Vec::len))
            .map(|x| g.iter().map(|r| r.get(x).copied().unwrap_or(0.0)).sum::<f64>() / g.len().max(1) as f64)
            .collect();
        let (vr, vc) = (variance(&row_means), variance(&col_means));
        assert!(vr > 2.0 * vc && vr > 1.0, "rows differ {vr}, columns differ {vc}: not horizontal stripes");
    }

    #[test]
    fn untextured_render_is_unchanged() {
        let (Some(a), Some(b)) = (centre("wall", false), centre("wall", false)) else { return };
        assert_eq!(a, b, "deterministic");
        // The white layer multiplies by exactly one: every pixel of the flat face is the same colour.
        let first = a.first().and_then(|r| r.first()).copied().unwrap_or(0.0);
        assert!(a.iter().flatten().all(|v| *v == first), "a flat face stays one colour");
        assert!(first > 0.0);
        assert_eq!(layer_for("wall"), 0);
    }

    #[test]
    fn layer_for_maps_finished_materials() {
        assert_eq!(layer_for("wall"), 0);
        assert_eq!(layer_for("roof"), 0);
        assert_eq!(layer_for("floor"), 0);
        assert_eq!(layer_for("wall-exterior"), 0);
        assert_eq!(layer_for(""), 0);
        for (i, f) in Finish::ALL.iter().enumerate() {
            for prefix in ["wall", "roof", "floor"] {
                assert_eq!(layer_for(&format!("{prefix}-{}", f.name())), i as u32 + 1);
            }
        }
        assert_eq!(layer_for("WALL-Brick"), 3);
        assert_eq!(layer_for("glass-brick"), 0);
    }
}
