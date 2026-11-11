//! Test-only helpers for GPU tests that must not hang the test binary.

use std::future::Future;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// Try to obtain a wgpu device and queue without blocking for longer than one poll.
///
/// Returns `None` when no adapter is available or the adapter/device request future
/// is not ready after a single poll.
pub(crate) fn device_and_queue() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = crate::offscreen::headless_instance();
    // A held turn is Pending, so bound the wait instead of reading a busy turn as no adapter.
    let request = async {
        let _turn = crate::adapter::adapter_turn().await;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.ok()?;
        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.ok()
    };
    let (device, queue) = poll_bounded(request, SHARED_DEVICE_BUDGET)??;
    Some((crate::offscreen::kept_for_the_process(device), queue))
}

/// One device per test thread. Sharing one device across all tests was racing the same queue on the
/// software adapter; each test thread now gets its own device, while still reusing a single device
/// within that thread.
pub(crate) fn shared_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    thread_local! {
        static SHARED: std::cell::OnceCell<Option<(wgpu::Device, wgpu::Queue)>> = const { std::cell::OnceCell::new() };
    }
    SHARED.with(|cell| {
        let init = || crate::offscreen::device_within(SHARED_DEVICE_BUDGET);
        cell.get_or_init(init).clone()
    })
}

/// One compute device (the software adapter) for each test thread; `None` (tests skip) without one.
pub(crate) fn shared_compute() -> Option<crate::pathtrace::ComputeGpu> {
    thread_local! {
        static SHARED: std::cell::OnceCell<Option<crate::pathtrace::ComputeGpu>> = const { std::cell::OnceCell::new() };
    }
    SHARED.with(|cell| {
        let init = || crate::pathtrace::request_compute(crate::adapter::AdapterChoice::Fallback, false).ok().flatten();
        cell.get_or_init(init).clone()
    })
}

/// One compute device on the software adapter with ray queries asked for (llvmpipe offers them), for the
/// hardware-ray-query path's tests; `None` without one. Its `ray_query` is false where the adapter has none.
pub(crate) fn shared_compute_rq() -> Option<crate::pathtrace::ComputeGpu> {
    thread_local! {
        static SHARED: std::cell::OnceCell<Option<crate::pathtrace::ComputeGpu>> = const { std::cell::OnceCell::new() };
    }
    SHARED.with(|cell| {
        let init = || crate::pathtrace::request_compute(crate::adapter::AdapterChoice::Fallback, true).ok().flatten();
        cell.get_or_init(init).clone()
    })
}

/// How long [`shared_device`] waits for the adapter and device.
pub(crate) const SHARED_DEVICE_BUDGET: Duration = Duration::from_secs(30);

/// Poll `fut` until it resolves or `budget` elapses, sleeping 5 ms between polls.
pub(crate) fn poll_bounded<F: Future>(fut: F, budget: Duration) -> Option<F::Output> {
    let mut fut = Box::pin(fut);
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let start = Instant::now();
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return Some(output),
            Poll::Pending => {}
        }
        if Instant::now().duration_since(start) >= budget {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_minimal_storage_write() {
        let Some((device, queue)) = shared_device() else {
            eprintln!("SKIP: no device");
            return;
        };
        #[repr(C)]
        #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
        struct Node {
            min: [f32; 3],
            left_or_first: u32,
            max: [f32; 3],
            count: u32,
        }
        let node = Node::default();
        let bytes = bytemuck::bytes_of(&node);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe nodes"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytes);
    }

    #[test]
    fn probe_compute_storage_write_on_rq_device() {
        let Some(gpu) = crate::pathtrace::request_compute(crate::adapter::AdapterChoice::Fallback, true).ok().flatten() else {
            eprintln!("SKIP: no rq compute device");
            return;
        };
        #[repr(C)]
        #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
        struct Node {
            min: [f32; 3],
            left_or_first: u32,
            max: [f32; 3],
            count: u32,
        }
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe rq nodes"),
            size: std::mem::size_of::<Node>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&buf, 0, bytemuck::bytes_of(&Node::default()));
    }

    #[test]
    fn probe_uniform_write_on_rq_device() {
        let Some(gpu) = crate::pathtrace::request_compute(crate::adapter::AdapterChoice::Fallback, true).ok().flatten() else {
            eprintln!("SKIP: no rq compute device");
            return;
        };
        let frame = crate::pathtrace::scene_gpu::GpuFrame {
            inv_view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            eye: [0.0, 0.0, 1.0, 0.0],
            size: [8, 8],
            sample_base: 0,
            seed: 1,
            bounces: 1,
            light_count: 0,
            flags: 0,
            spp_this_pass: 1,
            probe_count: 0,
            ..Default::default()
        };
        let bytes = bytemuck::bytes_of(&frame);
        let size = ((bytes.len() + 255) / 256) * 256;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe uniform"),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&buf, 0, bytes);
    }

    #[test]
    fn shared_device_is_created_once() {
        let start = Instant::now();
        let a = shared_device();
        let b = shared_device();
        assert_eq!(a.is_some(), b.is_some());
        assert!(start.elapsed() < SHARED_DEVICE_BUDGET + Duration::from_secs(5));
    }

    #[test]
    fn gputest_helper_returns_none_or_a_device_without_hanging() {
        let _ = device_and_queue();

        let start = Instant::now();
        let val = poll_bounded(async { 42u32 }, Duration::from_secs(1));
        assert_eq!(val, Some(42));
        assert!(start.elapsed() < Duration::from_secs(5));

        let start = Instant::now();
        let val = poll_bounded(futures::future::pending::<u32>(), Duration::from_millis(20));
        assert!(val.is_none());
        assert!(start.elapsed() >= Duration::from_millis(15));
    }

    #[test]
    fn poll_bounded_resolves_after_polls() {
        let fut = async { 99u32 };
        let val = poll_bounded(fut, Duration::from_secs(1));
        assert_eq!(val, Some(99));
    }

    #[test]
    fn poll_bounded_zero_budget() {
        let start = Instant::now();
        let val = poll_bounded(futures::future::pending::<u32>(), Duration::from_secs(0));
        assert!(val.is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn shared_device_consistent_multiple_calls() {
        let results: Vec<bool> = (0..5).map(|_| shared_device().is_some()).collect();
        assert!(results.iter().all(|&r| r == results[0]));
    }
}
