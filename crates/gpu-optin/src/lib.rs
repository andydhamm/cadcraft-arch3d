//! The two wgpu calls marked `unsafe` that the renderer opts into (phase 11), kept out of
//! `render3d`, which forbids unsafe code:
//!
//! - [`experimental_features`]: the token that lets a device enable wgpu's experimental ray
//!   queries, asked for only with `render --rt` on an adapter that offers them;
//! - [`pipeline_cache`]: a pipeline cache made from bytes this program saved on disk.
//!
//! Nothing else belongs here.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

/// The experimental-features token for a device: enabled only when `ray_query` is wanted (the
/// `--rt` flag) and the adapter offers `Features::EXPERIMENTAL_RAY_QUERY`; disabled otherwise.
pub fn experimental_features(ray_query: bool, adapter_features: wgpu::Features) -> wgpu::ExperimentalFeatures {
    if ray_query && adapter_features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
        // SAFETY: wgpu asks the caller to accept that experimental features may still hold bugs
        // with undefined behaviour. Ray queries are off unless `--rt` asks for them, are used only
        // through wgpu's validated ray-query API, and must render what the compute path renders
        // (phase-11 check 10 compares the two), so a wrong result shows up as a failed check.
        #[allow(unsafe_code)]
        let token = unsafe { wgpu::ExperimentalFeatures::enabled() };
        token
    } else {
        wgpu::ExperimentalFeatures::disabled()
    }
}

/// A pipeline cache for `device`, primed with `data` when given. `None` when the device lacks
/// `Features::PIPELINE_CACHE`.
pub fn pipeline_cache(device: &wgpu::Device, label: &str, data: Option<&[u8]>) -> Option<wgpu::PipelineCache> {
    if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return None;
    }
    let desc = wgpu::PipelineCacheDescriptor { label: Some(label), data, fallback: true };
    // SAFETY: `data` is only ever bytes this program wrote from `PipelineCache::get_data`, read
    // back from a cache file whose name carries the adapter's vendor, device, driver and the wgpu
    // version (render3d's `pathtrace::cache`). `fallback: true` makes wgpu start an empty cache
    // when its header check rejects the data (another wgpu, adapter or driver).
    #[allow(unsafe_code)]
    let cache = unsafe { device.create_pipeline_cache(&desc) };
    Some(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experimental_features_stay_off_unless_asked_and_offered() {
        let off = format!("{:?}", wgpu::ExperimentalFeatures::disabled());
        assert_eq!(format!("{:?}", experimental_features(false, wgpu::Features::all())), off);
        assert_eq!(format!("{:?}", experimental_features(true, wgpu::Features::empty())), off);
        assert_ne!(format!("{:?}", experimental_features(true, wgpu::Features::EXPERIMENTAL_RAY_QUERY)), off);
    }
}
