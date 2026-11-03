//! The finish textures of the mesh pipelines (phase 13): one `texture_2d_array` bound as group 3.
//!
//! Layer 0 is white (every untextured mesh samples it, so its image is unchanged); layers 1..=4 are
//! [`cadcraft_arch3d::texture::texture`] for [`Finish::ALL`] in order. The chain of mip levels is
//! built on the CPU with a box filter, and one sampler repeats, filters linearly and blends mips.

use cadcraft_arch3d::texture::{TEXTURE_SIZE, texture};
use cadcraft_doc::arch::Finish;
use wgpu::util::DeviceExt;

/// Layers in the array: white plus one per finish.
pub const LAYERS: u32 = 1 + Finish::ALL.len() as u32;
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

/// Array layer of a material: 1 + the finish index for `wall-<f>` / `roof-<f>` / `floor-<f>`, else 0
/// (white).
pub fn layer_for(material: &str) -> u32 {
    finish_of(material).and_then(|f| Finish::ALL.iter().position(|a| *a == f)).map_or(0, |i| i as u32 + 1)
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

/// The texel data in `LayerMajor` order: for each layer, its mips from the largest down.
pub fn array_data() -> Vec<u8> {
    let side = TEXTURE_SIZE as usize;
    let mut layers = vec![vec![255u8; side * side * 4]];
    layers.extend(Finish::ALL.iter().map(|f| texture(*f).rgba));
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
    /// The `LAYERS`-layer array texture.
    pub texture: wgpu::Texture,
    /// Group 3 over the array and the sampler.
    pub bind_group: wgpu::BindGroup,
}

impl MaterialTextures {
    /// Upload the array (all mips) and build the bind group over `layout`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("render3d material textures"),
                size: wgpu::Extent3d { width: TEXTURE_SIZE, height: TEXTURE_SIZE, depth_or_array_layers: LAYERS },
                mip_level_count: MIP_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &array_data(),
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
        MaterialTextures { texture, bind_group }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_array_has_five_layers_with_mips() {
        let per_layer: usize = (0..MIP_LEVELS).map(|l| (256usize >> l).pow(2) * 4).sum();
        assert_eq!(LAYERS, 5);
        assert_eq!(MIP_LEVELS, 9);
        let data = array_data();
        assert_eq!(data.len(), per_layer * 5);
        assert!(data.iter().take(per_layer).all(|&b| b == 255), "layer 0 is white at every mip");
        assert_eq!(array_data(), data, "deterministic");
        if let Some((device, queue)) = crate::gputest::shared_device() {
            let layout = bind_group_layout(&device);
            let t = MaterialTextures::new(&device, &queue, &layout);
            assert_eq!(t.texture.depth_or_array_layers(), 5);
            assert_eq!(t.texture.mip_level_count(), 9);
            assert_eq!(t.texture.width(), 256);
            assert_eq!(t.texture.format(), TEXTURE_FORMAT);
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
