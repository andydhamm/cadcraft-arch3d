//! The material textures of the mesh pipelines: one `texture_2d_array` bound as group 3.
//!
//! Layer 0 is white (every untextured mesh samples it, so its image is unchanged); layers 1..=4 are
//! [`cadcraft_arch3d::texture::texture`] for [`Finish::ALL`] in order (phase 13). Phase 14 adds one
//! layer per [`Scene3d::textures`] key in key order ([`SceneLayers`]), each image resampled
//! bilinearly to 256 x 256 ([`resample`]), at most [`MAX_SCENE_LAYERS`] of them; a texture past
//! those draws as its mean colour ([`mean_linear`]). The chain of mip levels is built on the CPU
//! with a box filter, and one sampler repeats, filters linearly and blends mips. The array is
//! rebuilt only when the scene's set of texture keys changes
//! ([`crate::pipeline::GpuPipeline::prepare_textures`]).

use crate::types::Scene3d;
use cadcraft_arch3d::texture::{TEXTURE_SIZE, texture};
use cadcraft_doc::arch::{Finish, ImageRgba};
use wgpu::util::DeviceExt;

/// Layers of the phase-13 array: white plus one per finish. A scene's textures follow them.
pub const LAYERS: u32 = 1 + Finish::ALL.len() as u32;
/// Most layers a scene's textures add (256 layers in all, the WebGL2 limit).
pub const MAX_SCENE_LAYERS: u32 = 251;
/// Most layers the array holds.
pub const MAX_LAYERS: u32 = LAYERS + MAX_SCENE_LAYERS;
/// Mip levels of every layer (256 down to 1).
pub const MIP_LEVELS: u32 = TEXTURE_SIZE.ilog2() + 1;
/// Texture format of the array.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The finish a material shows: the deck boards' `deck` (Wood), else `wall-<f>`, `roof-<f>` or
/// `floor-<f>` (case-insensitive).
pub fn finish_of(material: &str) -> Option<Finish> {
    if let Some(f) = cadcraft_arch3d::texture::board_finish(material) {
        return Some(f);
    }
    let m = material.to_ascii_lowercase();
    let name = ["wall-", "roof-", "floor-"].iter().find_map(|p| m.strip_prefix(p))?;
    Finish::parse(name)
}

/// Array layer of a material: 1 + the finish index for `wall-<f>` / `roof-<f>` / `floor-<f>`, else 0
/// (white). A [`crate::types::MaterialDef`]'d mesh takes its texture's layer instead
/// ([`SceneLayers::layer_of`]).
pub fn layer_for(material: &str) -> u32 {
    finish_of(material).and_then(|f| Finish::ALL.iter().position(|a| *a == f)).map_or(0, |i| i as u32 + 1)
}

/// The scene textures that have a layer of the array: the keys of [`Scene3d::textures`] whose image
/// is valid ([`ImageRgba::is_valid`]), in key order, at most [`MAX_SCENE_LAYERS`]. Key `i` is layer
/// [`LAYERS`] + `i`. Two scenes with equal `SceneLayers` share one array.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneLayers {
    keys: Vec<String>,
}

impl SceneLayers {
    /// The layered texture keys of `scene`.
    pub fn of(scene: &Scene3d) -> Self {
        let keys = scene.textures.iter().filter(|(_, image)| image.is_valid()).take(MAX_SCENE_LAYERS as usize).map(|(key, _)| key.clone()).collect();
        SceneLayers { keys }
    }

    /// The keys, in layer order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// The array layer of texture `key`, when it has one.
    pub fn layer_of(&self, key: &str) -> Option<u32> {
        // The keys come from a BTreeMap, so they are sorted.
        let i = self.keys.binary_search_by(|k| k.as_str().cmp(key)).ok()?;
        u32::try_from(i).ok().map(|i| LAYERS + i)
    }

    /// Layers of the array: the phase-13 ones plus one per key.
    pub fn layer_count(&self) -> u32 {
        LAYERS + self.keys.len().min(MAX_SCENE_LAYERS as usize) as u32
    }
}

/// sRGB byte to linear light.
pub(crate) fn srgb_to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Mean linear-light colour of `image`'s pixels (as `arch3d::texture::mean_linear` for a finish);
/// `None` when the image is not valid.
pub fn mean_linear(image: &ImageRgba) -> Option<[f32; 3]> {
    if !image.is_valid() {
        return None;
    }
    let mut sum = [0f64; 3];
    let mut n = 0u64;
    for px in image.rgba.as_chunks::<4>().0 {
        for (s, v) in sum.iter_mut().zip(px) {
            *s += f64::from(srgb_to_linear(*v));
        }
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let n = n as f64;
    Some([(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32])
}

/// `image` as one 256 x 256 layer: bilinear over the sRGB bytes, texel centres aligned, edges
/// clamped; an image already 256 x 256 is copied as it is, and an invalid one is white.
pub fn resample(image: &ImageRgba) -> Vec<u8> {
    let side = TEXTURE_SIZE as usize;
    if !image.is_valid() {
        return vec![255u8; side * side * 4];
    }
    let (w, h) = (image.width as usize, image.height as usize);
    if w == side && h == side {
        return image.rgba.clone();
    }
    let texel = |x: usize, y: usize, c: usize| f32::from(image.rgba.get((y * w + x) * 4 + c).copied().unwrap_or(0));
    // Source coordinate of output texel `o` along an axis of `n` source texels: the lower texel and
    // the weight of the upper one.
    let source = |o: usize, n: usize| -> (usize, usize, f32) {
        let s = ((o as f32 + 0.5) * n as f32 / side as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let lo = s.floor() as usize;
        let hi = (lo + 1).min(n - 1);
        (lo, hi, s - lo as f32)
    };
    let columns: Vec<(usize, usize, f32)> = (0..side).map(|x| source(x, w)).collect();
    let mut out = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        let (y0, y1, fy) = source(y, h);
        for &(x0, x1, fx) in &columns {
            for c in 0..4 {
                let top = texel(x0, y0, c) * (1.0 - fx) + texel(x1, y0, c) * fx;
                let bottom = texel(x0, y1, c) * (1.0 - fx) + texel(x1, y1, c) * fx;
                out.push((top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
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

/// The phase-13 texel data in `LayerMajor` order: for each layer, its mips from the largest down.
pub fn array_data() -> Vec<u8> {
    scene_array_data(&Scene3d::default(), &SceneLayers::default())
}

/// [`array_data`] followed by the layers of `layers`: each key's image of `scene` [`resample`]d
/// (white when the scene no longer has it), with its mips.
pub fn scene_array_data(scene: &Scene3d, layers: &SceneLayers) -> Vec<u8> {
    let side = TEXTURE_SIZE as usize;
    let white = || vec![255u8; side * side * 4];
    let mut all = vec![white()];
    all.extend(Finish::ALL.iter().map(|f| texture(*f).rgba));
    all.extend(layers.keys().iter().map(|key| scene.textures.get(key).map_or_else(white, resample)));
    all.into_iter().flat_map(mip_chain).flatten().collect()
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
    /// The array texture: [`LAYERS`] plus one layer per scene texture key.
    pub texture: wgpu::Texture,
    /// Group 3 over the array and the sampler.
    pub bind_group: wgpu::BindGroup,
}

impl MaterialTextures {
    /// Upload the phase-13 array (all mips) and build the bind group over `layout`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        Self::with_layers(device, queue, layout, &Scene3d::default(), &SceneLayers::default())
    }

    /// [`MaterialTextures::new`] with `layers`' images of `scene` after the phase-13 layers
    /// ([`scene_array_data`]).
    pub fn with_layers(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, scene: &Scene3d, layers: &SceneLayers) -> Self {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("render3d material textures"),
                size: wgpu::Extent3d { width: TEXTURE_SIZE, height: TEXTURE_SIZE, depth_or_array_layers: layers.layer_count() },
                mip_level_count: MIP_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &scene_array_data(scene, layers),
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

    #[test]
    fn deck_draws_the_wood_layer() {
        assert_eq!(layer_for("deck"), layer_for("floor-wood"));
        assert_ne!(layer_for("deck"), 0);
        assert_eq!(layer_for("Deck"), layer_for("floor-wood"));
        assert_eq!(finish_of("deck"), Some(Finish::Wood));
        for other in ["deck-joist", "deck-post", "deck-beam", "deck-rim", "deck-ledger", "deck-footing"] {
            assert_eq!(layer_for(other), 0, "{other}");
        }
    }

    // ---- Phase 14: the scene's textures (symbol parts, paints, library images). ----

    use crate::pipeline::material_draw;
    use crate::types::MaterialDef;

    /// An sRGB image of `width` x `height` from its RGBA bytes.
    fn image(width: u32, height: u32, rgba: Vec<u8>) -> ImageRgba {
        let mut i = ImageRgba { width, height, ..Default::default() };
        i.rgba = rgba;
        i
    }

    /// A 1 x 1 image of one opaque sRGB colour.
    fn solid(c: [u8; 3]) -> ImageRgba {
        image(1, 1, vec![c[0], c[1], c[2], 255])
    }

    /// Four texels in a row: two red, then two blue (pure red for u < 0.375, pure blue past 0.625
    /// once resampled).
    fn red_blue() -> ImageRgba {
        image(4, 1, [[255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255]].concat())
    }

    /// The 2 m square of [`wall`] (along x, facing +y) as mesh `material` of object `object`: u runs
    /// 0 -> `span` along +x, v 0 -> `span` down the face.
    fn quad(material: &str, object: u32, span: f32) -> Mesh {
        let mut m = Mesh::new(material, ArchId(object));
        for p in [[1.0f32, 0.0, -1.0], [-1.0, 0.0, -1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, 1.0]] {
            m.positions.push(p);
            m.normals.push([0.0, 1.0, 0.0]);
            m.uvs.push([(p[0] + 1.0) / 2.0 * span, (1.0 - p[2]) / 2.0 * span]);
        }
        m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        m
    }

    fn quad_scene(meshes: Vec<Mesh>) -> Scene3d {
        let mut s = Scene3d { meshes, ..Default::default() };
        s.bounds = crate::types::Bounds3::new([-1.0; 3], [1.0; 3]);
        s
    }

    fn renderer() -> Option<Offscreen> {
        let (device, queue) = crate::gputest::shared_device()?;
        Some(Offscreen::with_device(device, queue, (256, 256)))
    }

    /// A 256 x 256 face-on render of `scene` (the camera and light of [`centre`]: the sun straight
    /// up, so a vertical face is lit by the 0.9 ambient alone and a pixel is 0.9 x albedo x texel in
    /// linear light). The quad covers about x, y = 77..179.
    fn shot(r: &mut Offscreen, scene: &Scene3d, key: u64) -> Option<Vec<u8>> {
        r.render(scene, key, &Camera::Orbit(Orbit::at_origin(0.0, 0.0, 6.0, 45.0)), &Light::new([0.0, 0.0, 1.0], 0.9));
        r.read_rgba().ok()
    }

    /// Mean sRGB colour (0..255) of the 5 x 5 pixels around (x, y) of a 256-wide render.
    fn spot(rgba: &[u8], x: usize, y: usize) -> [f64; 3] {
        let mut sum = [0.0; 3];
        for yy in y - 2..=y + 2 {
            for xx in x - 2..=x + 2 {
                for (c, s) in sum.iter_mut().enumerate() {
                    *s += f64::from(rgba.get((yy * 256 + xx) * 4 + c).copied().unwrap_or(0));
                }
            }
        }
        sum.map(|v| v / 25.0)
    }

    /// Linear light to an sRGB byte value (unrounded).
    fn to_srgb(v: f32) -> f64 {
        let v = f64::from(v).clamp(0.0, 1.0);
        255.0 * if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    }

    fn is_red(c: [f64; 3]) -> bool {
        c[0] > 200.0 && c[1] < 20.0 && c[2] < 20.0
    }

    fn is_blue(c: [f64; 3]) -> bool {
        c[2] > 200.0 && c[0] < 20.0 && c[1] < 20.0
    }

    /// One end of the quad red and the other blue (the `red_blue` texture across it once).
    fn red_and_blue(px: &[u8]) -> bool {
        let (a, b) = (spot(px, 100, 128), spot(px, 156, 128));
        (is_red(a) && is_blue(b)) || (is_red(b) && is_blue(a))
    }

    #[test]
    fn symbol_textures_get_array_layers() {
        // One layer per valid texture key, in key order, after the five phase-13 layers.
        let mut scene = Scene3d::default();
        scene.textures.insert("sym:b:t0".into(), solid([0, 255, 0]));
        scene.textures.insert("item:material-oak".into(), solid([120, 80, 40]));
        scene.textures.insert("sym:a:t0".into(), red_blue());
        scene.textures.insert("sym:bad:t0".into(), image(2, 2, vec![0; 3]));
        let layers = SceneLayers::of(&scene);
        assert_eq!(layers.keys(), ["item:material-oak", "sym:a:t0", "sym:b:t0"]);
        assert_eq!(layers.layer_of("item:material-oak"), Some(5));
        assert_eq!(layers.layer_of("sym:a:t0"), Some(6));
        assert_eq!(layers.layer_of("sym:b:t0"), Some(7));
        assert_eq!(layers.layer_of("sym:bad:t0"), None, "an invalid image gets no layer");
        assert_eq!(layers.layer_of("sym:c:t0"), None);
        assert_eq!(layers.layer_count(), 8);
        // The data: the phase-13 layers as they were, then each image resampled to 256 x 256, with mips.
        let per_layer: usize = (0..MIP_LEVELS).map(|l| (256usize >> l).pow(2) * 4).sum();
        let data = scene_array_data(&scene, &layers);
        assert_eq!(data.len(), per_layer * 8);
        assert!(data[..per_layer * 5] == array_data()[..], "the phase-13 layers are unchanged");
        let oak = &data[per_layer * 5..per_layer * 5 + 256 * 256 * 4];
        assert!(oak.as_chunks::<4>().0.iter().all(|p| *p == [120, 80, 40, 255]), "a 1 x 1 image fills its layer");
        let rb = resample(&red_blue());
        assert!(data[per_layer * 6..per_layer * 6 + 256 * 256 * 4] == rb[..]);
        // Bilinear between texel centres: red up to u = 0.375, blue from 0.625, a blend between.
        let texel = |x: usize| [rb[x * 4], rb[x * 4 + 1], rb[x * 4 + 2], rb[x * 4 + 3]];
        assert_eq!((texel(0), texel(95), texel(160), texel(255)), ([255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255]));
        assert_eq!((texel(127), texel(128)), ([129, 0, 126, 255], [126, 0, 129, 255]));
        assert_eq!(resample(&image(256, 256, vec![7; 256 * 256 * 4])), vec![7; 256 * 256 * 4], "256 x 256 is copied");
        assert_eq!(scene_array_data(&scene, &layers), data, "deterministic");

        // At most MAX_SCENE_LAYERS keys get a layer; a mesh whose texture has none draws its mean colour.
        let mut many = Scene3d::default();
        for i in 0..260 {
            many.textures.insert(format!("sym:{i:03}:t0"), solid([200, 100, 50]));
        }
        let layers = SceneLayers::of(&many);
        assert_eq!((layers.keys().len(), layers.layer_count()), (251, 256));
        assert_eq!((layers.layer_of("sym:250:t0"), layers.layer_of("sym:251:t0")), (Some(255), None));
        many.materials
            .insert("sym:k:0".into(), MaterialDef { base_color: [0.5, 1.0, 1.0, 1.0], texture: Some("sym:255:t0".into()), ..Default::default() });
        many.materials.insert("sym:k:1".into(), MaterialDef { texture: Some("sym:003:t0".into()), ..Default::default() });
        let mean = mean_linear(&solid([200, 100, 50])).unwrap_or([0.0; 3]);
        assert_eq!(mean, [srgb_to_linear(200), srgb_to_linear(100), srgb_to_linear(50)]);
        let past = material_draw(&many, &layers, "sym:k:0");
        assert_eq!((past.layer, past.albedo), (0, [0.5 * mean[0], mean[1], mean[2]]));
        let within = material_draw(&many, &layers, "sym:k:1");
        assert_eq!((within.layer, within.albedo, within.tile), (8, [1.0; 3], [1.0, 1.0]));

        // A textured quad furnishing draws its texture: one end red, the other blue.
        let Some(mut r) = renderer() else { return };
        let mut s = quad_scene(vec![quad("sym:a:0", 1, 1.0)]);
        s.textures.insert("sym:a:t0".into(), red_blue());
        s.materials.insert("sym:a:0".into(), MaterialDef { texture: Some("sym:a:t0".into()), ..Default::default() });
        let Some(px) = shot(&mut r, &s, 1) else { return };
        assert_eq!(r.gpu.texture_layers().keys(), ["sym:a:t0"]);
        assert!(red_and_blue(&px), "{:?} {:?}", spot(&px, 100, 128), spot(&px, 156, 128));
        // Pure red is 0.9 x 1 in linear light.
        let red = if is_red(spot(&px, 100, 128)) { spot(&px, 100, 128) } else { spot(&px, 156, 128) };
        assert!((red[0] - to_srgb(0.9)).abs() <= 2.0, "{red:?}");
    }

    #[test]
    fn tint_multiplies_the_texture() {
        // A painted quad with a texture and a tint: arch3d's paint materials carry the tint in the
        // base colour (white x tint here), and the renderer multiplies the texture by it, once.
        let tint = [200u8, 120, 60];
        let t = tint.map(srgb_to_linear);
        let grey = srgb_to_linear(180);
        let painted = |def: MaterialDef| {
            let mut s = quad_scene(vec![quad("paint:wall/w1/left", 1, 1.0)]);
            s.textures.insert("item:material-plaster".into(), solid([180; 3]));
            s.materials.insert("paint:wall/w1/left".into(), def);
            s
        };
        let texture = Some("item:material-plaster".to_string());
        let tinted = painted(MaterialDef { base_color: [t[0], t[1], t[2], 1.0], texture: texture.clone(), tint: Some(tint), ..Default::default() });
        let d = material_draw(&tinted, &SceneLayers::of(&tinted), "paint:wall/w1/left");
        assert_eq!((d.albedo, d.layer), (t, 5), "the base colour as given, the texture's layer");
        let untinted = painted(MaterialDef { texture, ..Default::default() });
        let flat = painted(MaterialDef { base_color: [t[0] * grey, t[1] * grey, t[2] * grey, 1.0], ..Default::default() });
        let Some(mut r) = renderer() else { return };
        let (Some(a), Some(u), Some(f)) = (shot(&mut r, &tinted, 1), shot(&mut r, &untinted, 2), shot(&mut r, &flat, 3)) else { return };
        let (a, u, f) = (spot(&a, 128, 128), spot(&u, 128, 128), spot(&f, 128, 128));
        for c in 0..3 {
            let want = to_srgb(0.9 * t[c] * grey);
            assert!((a[c] - want).abs() <= 2.0, "channel {c}: {a:?}, want {want}");
            assert!((a[c] - f[c]).abs() <= 2.0, "channel {c}: textured {a:?}, untextured {f:?}");
            assert!((u[c] - to_srgb(0.9 * grey)).abs() <= 2.0, "channel {c}: untinted {u:?}");
        }
        assert!(a[0] > a[1] + 30.0 && a[1] > a[2] + 20.0, "tinted {a:?}");
    }

    #[test]
    fn tile_divides_face_aligned_uvs() {
        // UVs in metres (0..2 across the 2 m quad) with tile [2, 2] draw as UVs 0..1 with tile [1, 1].
        let scene = |span: f32, tile: [f32; 2]| {
            let mut s = quad_scene(vec![quad("paint:wall/w1/left", 1, span)]);
            s.textures.insert("item:material-stripe".into(), red_blue());
            s.materials.insert("paint:wall/w1/left".into(), MaterialDef { texture: Some("item:material-stripe".into()), tile, ..Default::default() });
            s
        };
        let metres = scene(2.0, [2.0, 2.0]);
        assert_eq!(material_draw(&metres, &SceneLayers::of(&metres), "paint:wall/w1/left").tile, [2.0, 2.0]);
        let Some(mut r) = renderer() else { return };
        let (Some(a), Some(b), Some(c)) =
            (shot(&mut r, &metres, 1), shot(&mut r, &scene(1.0, [1.0, 1.0]), 2), shot(&mut r, &scene(2.0, [1.0, 1.0]), 3))
        else {
            return;
        };
        assert!(red_and_blue(&a) && red_and_blue(&b));
        for x in [100, 156] {
            let (pa, pb) = (spot(&a, x, 128), spot(&b, x, 128));
            assert!((0..3).all(|k| (pa[k] - pb[k]).abs() <= 2.0), "x {x}: {pa:?} vs {pb:?}");
        }
        // Without the tile the metre UVs repeat the texture twice across the face.
        let pc = spot(&c, 100, 128);
        assert!(pc[0] > 30.0 && pc[2] > 30.0, "two repeats put the blend where one repeat is red or blue: {pc:?}");
    }

    #[test]
    fn texture_array_rebuilds_only_on_new_keys() {
        let Some(mut r) = renderer() else { return };
        let mut s = quad_scene(vec![quad("sym:a:0", 1, 1.0)]);
        let _ = shot(&mut r, &s, 1);
        assert_eq!((r.gpu.texture_builds(), r.gpu.texture_layers().layer_count()), (0, LAYERS), "no textures: the phase-13 array stays");
        s.textures.insert("sym:a:t0".into(), red_blue());
        s.materials.insert("sym:a:0".into(), MaterialDef { texture: Some("sym:a:t0".into()), ..Default::default() });
        let _ = shot(&mut r, &s, 2);
        assert_eq!((r.gpu.texture_builds(), r.gpu.texture_layers().layer_count()), (1, LAYERS + 1));
        // Frames, and edits that keep the keys (a new scene key, a moved mesh), keep the array.
        for _ in 0..3 {
            let _ = shot(&mut r, &s, 2);
        }
        let _ = shot(&mut r, &s, 3);
        if let Some(p) = s.meshes.first_mut().and_then(|m| m.positions.first_mut()) {
            p[2] = -0.95;
        }
        let px = shot(&mut r, &s, 4);
        assert_eq!(r.gpu.texture_builds(), 1, "same keys, same array");
        assert!(px.is_some_and(|p| red_and_blue(&p)));
        // A key that sorts first moves the symbol's texture to the next layer; draws and array agree.
        s.textures.insert("item:material-oak".into(), solid([120, 80, 40]));
        let px = shot(&mut r, &s, 5);
        assert_eq!((r.gpu.texture_builds(), r.gpu.texture_layers().layer_of("sym:a:t0")), (2, Some(LAYERS + 1)));
        assert!(px.is_some_and(|p| red_and_blue(&p)), "the furnishing still samples its own texture");
        // Dropping the textures goes back to the phase-13 array.
        s.textures.clear();
        s.materials.clear();
        let _ = shot(&mut r, &s, 6);
        assert_eq!((r.gpu.texture_builds(), r.gpu.texture_layers().layer_count()), (3, LAYERS));
    }

    #[test]
    fn untouched_scenes_are_unchanged() {
        // A phase-13 scene: a brick wall, a plain wall behind glass, a wood floor.
        let mut scene = wall("wall-brick", false);
        let mut back = quad("wall", 2, 1.0);
        let mut glass = quad("glass", 3, 1.0);
        for (m, y) in [(&mut back, -0.5f32), (&mut glass, 0.5)] {
            for p in m.positions.iter_mut() {
                p[0] *= 0.5;
                p[1] = y;
            }
        }
        let floor = cadcraft_arch3d::uv::face_uvs(
            &Mesh {
                positions: vec![[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]],
                indices: vec![0, 1, 2, 0, 2, 3],
                ..Mesh::new("floor-wood", ArchId(4))
            },
            texture(Finish::Wood).tile,
        );
        scene.meshes.extend([back, glass, floor]);
        // Every draw is its phase-13 material: material_pbr, the finish's layer, tile [1, 1].
        let layers = SceneLayers::of(&scene);
        assert_eq!(layers.layer_count(), LAYERS);
        assert!(scene_array_data(&scene, &layers) == array_data());
        for m in &scene.meshes {
            let (albedo, metallic, roughness, alpha) = crate::pipeline::material_pbr(&m.material);
            let want = crate::pipeline::Draw { albedo, metallic, roughness, alpha, layer: layer_for(&m.material), ..Default::default() };
            assert_eq!(material_draw(&scene, &layers, &m.material), want, "{}", m.material);
        }
        let p = crate::pipeline::PbrUniform::new(&Light::DEFAULT, 1.0, &crate::pipeline::Draw::default(), 0, [0.0; 3]);
        assert_eq!(&p._p2[2..], &[1.0, 1.0], "tile [1, 1] multiplies the UVs by exactly one");
        // The same scene with materials and textures that no mesh uses (the array grows to seven
        // layers) renders byte-identically, and so does the path tracer's scene.
        let mut extra = scene.clone();
        extra.textures.insert("sym:x:t0".into(), red_blue());
        extra.textures.insert("item:material-oak".into(), solid([120, 80, 40]));
        extra
            .materials
            .insert("sym:x:0".into(), MaterialDef { base_color: [1.0, 0.0, 0.0, 1.0], texture: Some("sym:x:t0".into()), ..Default::default() });
        let gpu = |s: &Scene3d| crate::pathtrace::scene_gpu::build_gpu_scene(s, true, None);
        assert_eq!(gpu(&scene), gpu(&extra));
        let plain = gpu(&scene);
        for (m, g) in scene.meshes.iter().zip(&plain.materials) {
            assert_eq!(*g, crate::pathtrace::scene_gpu::material_for(&m.material), "{}", m.material);
        }
        let Some(mut r) = renderer() else { return };
        let (Some(a), Some(b), Some(c)) = (shot(&mut r, &scene, 1), shot(&mut r, &extra, 2), shot(&mut r, &scene, 3)) else { return };
        assert_eq!(r.gpu.texture_builds(), 2, "grown for the extra textures, then back");
        assert!(a == b, "unused materials and textures change no pixel");
        assert!(a == c);
        assert!(a.as_chunks::<4>().0.iter().any(|p| p[0] != a[0]), "the render shows the scene");
    }
}
