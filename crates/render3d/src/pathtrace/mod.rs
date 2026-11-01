//! Path-traced Finalize Render (phase 11): a wgpu compute path tracer over a BVH of the scene's
//! triangles, progressive (samples accumulate and the image refines until a target or a cancel),
//! with bounced light, the sun and sky, every fixture (shadow rays toward the lights), rectangular
//! area lights, emissive materials, glossy and mirror reflections from the PBR roughness and
//! metalness, glass with refraction and Fresnel, IES distributions, and an edge-aware denoiser
//! guided by albedo and normal buffers. The phase-10 shadow-mapped still stays as the raster
//! Finalize, the only one where the device has no compute shaders (the web build's WebGL2).
//!
//! Units: radiance in cd/m² (nits). A fixture of intensity `I` cd gives `I·cos/d²` lux; the sun
//! gives [`SUN_LUX`] on a surface facing it; the sky's radiance runs from [`SKY_HORIZON_NITS`] at the
//! horizon to [`SKY_ZENITH_NITS`] overhead. A fixture's RGB is its colour divided by the colour's
//! [`luminance`], so its luminance is its candela and `illuminance` in lux is the luminance of the
//! irradiance. Pixels are `ACES(L · exposure)`, sRGB-encoded: the night exposure is the scene's
//! (`lights::DEFAULT_EXPOSURE`, as raster), with the sun on it is [`DAY_EXPOSURE`] (a white surface
//! facing the sun reads white).
//!
//! Modules: [`device`] (the compute device), [`scene_gpu`] (the scene as GPU buffers and a CPU
//! reference traversal), [`shaders`] (the WGSL and its pipelines), [`tracer`] (the progressive
//! driver and the irradiance probes), [`denoise`] (the à-trous filter), [`cache`] (the on-disk
//! pipeline cache).

#[cfg(test)]
mod bsdf_tests;
pub mod cache;
pub mod denoise;
pub mod device;
pub mod scene_gpu;
pub mod shaders;
pub mod tracer;

pub use device::{ComputeGpu, request_compute};
pub use tracer::{PathTracer, irradiance};

/// Samples per pixel a Finalize aims for, and the most a request may ask for.
pub const DEFAULT_SPP: u32 = 256;
pub const MAX_SPP: u32 = 65_536;
/// Bounces traced in full before Russian roulette takes over, and the most a request may ask for.
/// `0` = direct light only (no bounce, no roulette).
pub const DEFAULT_BOUNCES: u32 = 4;
pub const MAX_BOUNCES: u32 = 32;
/// Hard cap on a path's length, roulette included.
pub const MAX_PATH_DEPTH: u32 = 64;
/// Samples per pixel one step adds (a step stays short so the pane keeps drawing).
pub const DEFAULT_SPP_PER_STEP: u32 = 4;
/// Most intermediate images a request may write (`snapshots`).
pub const MAX_SNAPSHOTS: usize = 8;
/// Daylight: the sun's illuminance on a surface facing it (lx), the sky's radiance (nits).
pub const SUN_LUX: f32 = 100_000.0;
pub const SKY_ZENITH_NITS: f32 = 2_500.0;
pub const SKY_HORIZON_NITS: f32 = 5_000.0;
/// Exposure with the sun on: a white Lambertian surface facing the sun reads 1.0 before tone mapping.
pub const DAY_EXPOSURE: f32 = std::f32::consts::PI / SUN_LUX;

/// The white-furnace test: every surface diffuse with `albedo`, emitting `emission` nits, the
/// lights, sun and sky off and the environment at `emission / (1 − albedo)` (what a closed box
/// converges to), so every pixel converges to that value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Furnace {
    pub albedo: f32,
    pub emission: f32,
}

impl Furnace {
    /// The radiance every pixel converges to.
    pub fn expected(&self) -> f32 {
        self.emission / (1.0 - self.albedo)
    }
}

/// What a path-traced render does.
#[derive(Clone, Debug, PartialEq)]
pub struct PathOptions {
    /// Samples per pixel to reach (1..=[`MAX_SPP`]).
    pub spp: u32,
    /// Full bounces before Russian roulette (0..=[`MAX_BOUNCES`]).
    pub bounces: u32,
    /// The random sequence: the same seed gives the same image on the same adapter.
    pub seed: u32,
    /// Run the denoiser on the final image.
    pub denoise: bool,
    /// Use hardware ray queries where the adapter has them (the image must match the compute path).
    pub rt: bool,
    /// Samples per pixel each [`PathTracer::step`] adds.
    pub spp_per_step: u32,
    pub furnace: Option<Furnace>,
    /// Sample counts at which [`PathTracer::step`] keeps an image (ascending, each <= `spp`).
    pub snapshots: Vec<u32>,
}

impl Default for PathOptions {
    fn default() -> Self {
        PathOptions {
            spp: DEFAULT_SPP,
            bounces: DEFAULT_BOUNCES,
            seed: 0,
            denoise: true,
            rt: false,
            spp_per_step: DEFAULT_SPP_PER_STEP,
            furnace: None,
            snapshots: Vec::new(),
        }
    }
}

/// Linear radiance over a still's pixels, before exposure (the furnace test reads it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadianceStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
}

/// Whether the compiled pipelines came from the disk cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CacheOutcome {
    /// The backend has no pipeline cache (anything but Vulkan), or no cache directory.
    #[default]
    Unsupported,
    /// There was no cache file for this adapter; one is written after the render.
    Miss,
    /// The pipelines were created from this adapter's cache file.
    Hit,
}

impl CacheOutcome {
    /// `unsupported`, `miss` or `hit` (the reply's `pipelineCache`).
    pub fn name(self) -> &'static str {
        match self {
            CacheOutcome::Unsupported => "unsupported",
            CacheOutcome::Miss => "miss",
            CacheOutcome::Hit => "hit",
        }
    }
}

/// What a path-traced render did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathStats {
    /// Samples per pixel accumulated so far.
    pub spp_done: u32,
    pub denoised: bool,
    /// Hardware ray queries were used.
    pub rt: bool,
    pub pipeline_cache: CacheOutcome,
    /// Linear radiance over the pixels (every render reports it; the furnace test checks it).
    pub radiance: Option<RadianceStats>,
    /// `(spp, rgba)` images kept at the requested `snapshots`.
    pub snapshots: Vec<(u32, Vec<u8>)>,
}

/// Where a path-traced render is.
#[derive(Clone, Debug, PartialEq)]
pub enum PathStatus {
    /// Still working: `progress` 0..1 and the samples per pixel reached.
    Running {
        progress: f32,
        spp: u32,
    },
    /// Finished: the RGBA8 pixels (`width·height·4`, rows top to bottom).
    Done {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Cancelled,
    Failed(String),
}

/// Rec. 709 luminance of linear RGB.
pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnace_converges_to_emission_over_one_minus_albedo() {
        let f = Furnace { albedo: 0.5, emission: 1.0 };
        assert!((f.expected() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn day_exposure_maps_full_sun_on_white_to_one() {
        let radiance = SUN_LUX / std::f32::consts::PI;
        assert!((radiance * DAY_EXPOSURE - 1.0).abs() < 1e-6);
        assert!((luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cache_outcomes_have_reply_names() {
        assert_eq!([CacheOutcome::Unsupported, CacheOutcome::Miss, CacheOutcome::Hit].map(|c| c.name()), ["unsupported", "miss", "hit"]);
    }
}
