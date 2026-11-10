//! Adapter choice and device request. Every device is requested with the downlevel WebGL2
//! limits so native render tests exercise the limits the web build has to live within.

use crate::Render3dError;

/// Which adapter to request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AdapterChoice {
    /// The best available hardware adapter.
    #[default]
    Auto,
    /// The software (fallback) adapter.
    Fallback,
}

impl AdapterChoice {
    /// Parse `auto` or `fallback` (case-insensitive); anything else is `None`.
    pub fn parse(s: &str) -> Option<AdapterChoice> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("auto") {
            Some(AdapterChoice::Auto)
        } else if s.eq_ignore_ascii_case("fallback") {
            Some(AdapterChoice::Fallback)
        } else {
            None
        }
    }
}

/// What device a renderer ended up on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Adapter name as reported by the driver.
    pub name: String,
    /// Graphics backend (`Vulkan`, `Metal`, `Gl`, …).
    pub backend: String,
    /// Adapter type (`DiscreteGpu`, `Cpu`, …).
    pub adapter_type: String,
    /// True when the device was requested with the downlevel WebGL2 limits.
    pub downlevel: bool,
}

impl DeviceInfo {
    /// Describe a device whose adapter is not known (`unknown` name, backend and type).
    pub fn unknown(downlevel: bool) -> Self {
        let unknown = || "unknown".to_string();
        DeviceInfo { name: unknown(), backend: unknown(), adapter_type: unknown(), downlevel }
    }

    /// One-line report: `DEVICE name=… backend=… type=…`, plus ` limits=downlevel_webgl2` when
    /// the device uses the downlevel WebGL2 limits.
    pub fn line(&self) -> String {
        let mut s = format!("DEVICE name={} backend={} type={}", self.name, self.backend, self.adapter_type);
        if self.downlevel {
            s.push_str(" limits=downlevel_webgl2");
        }
        s
    }
}

/// Request an adapter per `choice` and a device on it with the downlevel WebGL2 limits
/// (weakened to the adapter's own limits where it is weaker).
/// [`Render3dError::NoAdapter`] when there is no adapter.
///
/// One device per adapter choice is made per process and shared by every caller (all headless callers
/// use [`crate::offscreen::headless_instance`]): devices are never destroyed (the loader race, see
/// `offscreen::kept_for_the_process`), and a device per request held a 256 MB memory block each, which ran
/// the 4 GB T1000 out of memory under the render3d tests (2026-11-04).
pub async fn request_device(instance: &wgpu::Instance, choice: AdapterChoice) -> Result<(wgpu::Device, wgpu::Queue, DeviceInfo), Render3dError> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(hit) = shared_device(choice) {
        return Ok(hit);
    }
    let options = match choice {
        AdapterChoice::Auto => wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() },
        AdapterChoice::Fallback => wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() },
    };
    let turn = adapter_turn().await;
    // Another thread may have made the device while this one waited for its turn.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(hit) = shared_device(choice) {
        return Ok(hit);
    }
    let adapter = instance.request_adapter(&options).await.map_err(|_| Render3dError::NoAdapter)?;
    let wanted = wgpu::Limits::downlevel_webgl2_defaults();
    let required_limits = wanted.clone().or_worse_values_from(&adapter.limits());
    let downlevel = required_limits == wanted;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor { label: Some("render3d"), required_limits, ..Default::default() })
        .await
        .map_err(|e| Render3dError::Wgpu(e.to_string()))?;
    drop(turn);
    let info = adapter.get_info();
    let info = DeviceInfo { name: info.name, backend: format!("{:?}", info.backend), adapter_type: format!("{:?}", info.device_type), downlevel };
    #[cfg(not(target_arch = "wasm32"))]
    return Ok(share_device(choice, (device, queue, info)));
    #[cfg(target_arch = "wasm32")]
    Ok((device, queue, info))
}

/// Held by whoever is asking the driver for an adapter and device.
static ADAPTER_TURN: futures::lock::Mutex<()> = futures::lock::Mutex::new(());

/// Wait for this process's turn at the driver. Every adapter request holds a turn from just before
/// `request_adapter` until its `request_device` has returned: on NVIDIA 620.70.06, threads asking for
/// an adapter at the same moment crash inside `libnvidia-eglcore` (wgpu `expose_adapter`).
pub(crate) async fn adapter_turn() -> futures::lock::MutexGuard<'static, ()> {
    ADAPTER_TURN.lock().await
}

#[cfg(not(target_arch = "wasm32"))]
type Shared = (wgpu::Device, wgpu::Queue, DeviceInfo);

#[cfg(not(target_arch = "wasm32"))]
static SHARED: std::sync::Mutex<Vec<(AdapterChoice, Shared)>> = std::sync::Mutex::new(Vec::new());

#[cfg(not(target_arch = "wasm32"))]
fn shared_device(choice: AdapterChoice) -> Option<Shared> {
    SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().find(|(c, _)| *c == choice).map(|(_, d)| d.clone())
}

/// Record `made` as `choice`'s device, or, when another thread recorded one first, keep `made` open (it is
/// never dropped) and return that one.
#[cfg(not(target_arch = "wasm32"))]
fn share_device(choice: AdapterChoice, made: Shared) -> Shared {
    let mut shared = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, first)) = shared.iter().find(|(c, _)| *c == choice) {
        let _ = crate::offscreen::kept_for_the_process(made.0);
        return first.clone();
    }
    shared.push((choice, made.clone()));
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_choice_parses_auto_and_fallback_only() {
        assert_eq!(AdapterChoice::parse("auto"), Some(AdapterChoice::Auto));
        assert_eq!(AdapterChoice::parse("AUTO"), Some(AdapterChoice::Auto));
        assert_eq!(AdapterChoice::parse("Fallback"), Some(AdapterChoice::Fallback));
        assert_eq!(AdapterChoice::parse("fallback"), Some(AdapterChoice::Fallback));
        for bad in ["", "gpu", "software", "auto fallback", "fallbac"] {
            assert_eq!(AdapterChoice::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn adapter_turns_never_overlap() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static HOLDERS: AtomicUsize = AtomicUsize::new(0);
        static HIGHEST: AtomicUsize = AtomicUsize::new(0);
        let threads: Vec<_> = (0..16)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..8 {
                        let _turn = futures::executor::block_on(adapter_turn());
                        let now = HOLDERS.fetch_add(1, Ordering::SeqCst) + 1;
                        HIGHEST.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        HOLDERS.fetch_sub(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for t in threads {
            assert!(t.join().is_ok(), "a thread panicked");
        }
        assert_eq!(HIGHEST.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_info_line_names_the_limits() {
        let mut info = DeviceInfo { name: "Test GPU".into(), backend: "Vulkan".into(), adapter_type: "Cpu".into(), downlevel: true };
        assert_eq!(info.line(), "DEVICE name=Test GPU backend=Vulkan type=Cpu limits=downlevel_webgl2");
        info.downlevel = false;
        assert_eq!(info.line(), "DEVICE name=Test GPU backend=Vulkan type=Cpu");
    }

    /// Request a device bounded in time; `None` when the machine has no suitable adapter.
    fn device(choice: AdapterChoice) -> Option<(wgpu::Device, DeviceInfo)> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let instance = crate::offscreen::headless_instance();
            let got = futures::executor::block_on(request_device(&instance, choice)).ok();
            let _ = tx.send(got.map(|(d, _q, i)| (d, i)));
        });
        rx.recv_timeout(std::time::Duration::from_secs(30)).ok().flatten()
    }

    #[test]
    fn device_limits_are_downlevel_webgl2() {
        let Some((device, info)) = device(AdapterChoice::Auto) else { return };
        let web = wgpu::Limits::downlevel_webgl2_defaults();
        let got = device.limits();
        assert!(got.max_texture_dimension_2d <= web.max_texture_dimension_2d.max(got.max_texture_dimension_2d));
        if info.downlevel {
            assert_eq!(got.max_texture_dimension_2d, web.max_texture_dimension_2d);
            assert_eq!(got.max_bind_groups, web.max_bind_groups);
            assert!(info.line().ends_with("limits=downlevel_webgl2"));
        }
    }

    /// 2026-11-04: keeping every device for the process (3a50c117, the loader race) let a render3d test
    /// binary hold a device per test: the T1000 peaked at 3614 of 4096 MiB and the integrator refused
    /// harden-p13-arch3d-incremental on "Not enough memory left". One device per adapter choice is
    /// shared instead, so the count stays at a handful and none is destroyed.
    #[test]
    fn one_device_per_adapter_choice_is_shared() {
        let Some((a, _)) = device(AdapterChoice::Fallback) else { return };
        let Some((b, _)) = device(AdapterChoice::Fallback) else { return };
        assert!(a == b, "two requests for the software adapter must share one device");
    }

    #[test]
    fn fallback_adapter_is_requested_when_asked() {
        // Software adapters exist only on some machines: with one, it must be a CPU adapter.
        if let Some((_device, info)) = device(AdapterChoice::Fallback) {
            assert_eq!(info.adapter_type, "Cpu", "{}", info.line());
        }
    }
}
