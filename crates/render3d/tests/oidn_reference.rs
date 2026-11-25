//! Our U-Net against the genuine Open Image Denoise answer (operator 2026-11-24).
//!
//! `fixtures/oidn-kitchen` holds the showcase kitchen at 96 x 64 and 64 spp exactly as the learned
//! denoiser receives it (`color`, `albedo`, `normal`: linear PFM, rows bottom to top) and `oidn.pfm`,
//! what the official `oidnDenoise` 2.5.1 (CPU device, RT filter, `--hdr --alb --nrm`) returned for those
//! three files. No Open Image Denoise code is linked or shipped: only its output image is kept.
//!
//! Why it is here: until this test the network was only compared with our own CPU version, so a fault
//! shared by both would not show. Measured when added: the base model agrees with it to five decimal places (median and 99th percentile).

#![cfg(not(target_arch = "wasm32"))]

use cadcraft_render3d::tza::{self, ModelChoice};
use cadcraft_render3d::unet::{self, Net};

const SIZE: (u32, u32) = (96, 64);

fn pfm(name: &str) -> Vec<[f32; 3]> {
    let path = format!("{}/tests/fixtures/oidn-kitchen/{name}.pfm", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut at = 0;
    let mut header = Vec::new();
    while header.len() < 3 {
        let end = bytes[at..].iter().position(|&b| b == b'\n').expect("PFM header line") + at;
        header.push(std::str::from_utf8(&bytes[at..end]).expect("header text").to_string());
        at = end + 1;
    }
    assert_eq!(header[0], "PF", "{path}");
    assert_eq!(header[1], format!("{} {}", SIZE.0, SIZE.1), "{path}");
    assert!(header[2].trim().parse::<f32>().expect("scale") < 0.0, "{path}: little endian");
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    assert_eq!(bytes.len() - at, w * h * 12, "{path}");
    let value = |i: usize| f32::from_le_bytes([bytes[at + 4 * i], bytes[at + 4 * i + 1], bytes[at + 4 * i + 2], bytes[at + 4 * i + 3]]);
    // rows bottom to top in the file, top to bottom in memory
    (0..h).flat_map(|y| (0..w).map(move |x| (h - 1 - y) * w + x)).map(|i| [value(3 * i), value(3 * i + 1), value(3 * i + 2)]).collect()
}

fn luminance(p: [f32; 3]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

/// (median, 99th percentile) of |ours - genuine| / (genuine + 5 % of the frame mean), over channels.
fn difference(ours: &[[f32; 3]], genuine: &[[f32; 3]]) -> (f32, f32) {
    let mean = genuine.iter().map(|p| luminance(*p)).sum::<f32>() / genuine.len() as f32;
    let mut d: Vec<f32> = ours.iter().zip(genuine).flat_map(|(a, b)| (0..3).map(move |c| (a[c] - b[c]).abs() / (b[c].abs() + 0.05 * mean))).collect();
    d.sort_by(f32::total_cmp);
    (d[d.len() / 2], d[d.len() * 99 / 100])
}

#[test]
fn the_base_model_matches_the_genuine_denoiser_on_the_kitchen() {
    let (color, albedo, normal, genuine) = (pfm("color"), pfm("albedo"), pfm("normal"), pfm("oidn"));
    let net = Net::from_model(tza::model(ModelChoice::Base).expect("base model")).expect("net");
    let ours = unet::denoise_cpu(&net, &color, &albedo, &normal, SIZE).expect("denoise");
    let (median, p99) = difference(&ours, &genuine);
    let (m_ours, m_genuine) = (ours.iter().map(|p| luminance(*p)).sum::<f32>(), genuine.iter().map(|p| luminance(*p)).sum::<f32>());
    println!("base model against oidnDenoise 2.5.1: median {median:.5}, 99th percentile {p99:.5}, mean ratio {:.5}", m_ours / m_genuine);
    assert!(median < 0.01, "median relative difference {median}");
    assert!(p99 < 0.05, "99th percentile relative difference {p99}");
    assert!((m_ours / m_genuine - 1.0).abs() < 0.01, "frame mean {m_ours} against {m_genuine}");
}

#[test]
fn the_network_keeps_the_frame_brightness() {
    // The 64 spp kitchen is lit by rare bright paths; the network spreads them and keeps the light.
    let (color, albedo, normal) = (pfm("color"), pfm("albedo"), pfm("normal"));
    let input: f32 = color.iter().map(|p| luminance(*p)).sum();
    for which in [ModelChoice::Base, ModelChoice::Small] {
        let net = Net::from_model(tza::model(which).expect("model")).expect("net");
        let ours = unet::denoise_cpu(&net, &color, &albedo, &normal, SIZE).expect("denoise");
        let out: f32 = ours.iter().map(|p| luminance(*p)).sum();
        assert!((out / input - 1.0).abs() < 0.03, "{which:?}: the frame mean moved by {:.1} %", 100.0 * (out / input - 1.0));
    }
}
