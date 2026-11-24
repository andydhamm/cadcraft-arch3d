//! `render.ladder`: render the same view at each of the eight realism rungs, one PNG per rung,
//! and join them into a labelled contact sheet (`ladder.png`).

use std::path::Path;

use cadcraft_doc::arch::photo::RUNG_NAMES;
use serde_json::{Value, json};

use crate::cmd::arch::render::{build_request, render_hooks};
use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Session};

/// Height of the label band above the images (pixels).
const BAND: u32 = 24;
/// Label band background.
const BAND_BG: [u8; 4] = [24, 24, 24, 255];
/// Label ink.
const INK: [u8; 4] = [255, 255, 255, 255];
/// Tallest label text (pixels).
const TEXT_H: f64 = 14.0;
/// Most pixels the contact sheet may hold (guards hostile sizes).
const MAX_SHEET_PIXELS: u64 = 400_000_000;

/// The `render` params for rung `n`: the camera, size (when given), seed (default 0), the rung,
/// its output file under `out`, and the pass-through `spp`, `noiseTarget` and `timeBudget`.
pub(crate) fn rung_params(base: &Value, n: u8, out: &Path) -> Value {
    let mut m = serde_json::Map::new();
    for k in ["camera", "size"] {
        if let Some(v) = base.get(k) {
            m.insert(k.to_string(), v.clone());
        }
    }
    m.insert("seed".to_string(), base.get("seed").cloned().unwrap_or(json!(0)));
    m.insert("rung".to_string(), json!(n));
    m.insert("out".to_string(), json!(out.join(format!("rung-{n}.png")).to_string_lossy()));
    for k in ["spp", "noiseTarget", "timeBudget"] {
        if let Some(v) = base.get(k) {
            m.insert(k.to_string(), v.clone());
        }
    }
    Value::Object(m)
}

/// Mark one pixel of the band `(w, BAND)` (out-of-range points are ignored).
fn plot(buf: &mut [u8], w: u32, x: f64, y: f64) {
    if !(x.is_finite() && y.is_finite()) || x < 0.0 || y < 0.0 || x >= f64::from(w) || y >= f64::from(BAND) {
        return;
    }
    let i = (y as usize * w as usize + x as usize) * 4;
    if let Some(px) = buf.get_mut(i..i + 4) {
        px.copy_from_slice(&INK);
    }
}

/// Draw `name` centred in a `w` x [`BAND`] RGBA band (stroke font, 1 px lines).
fn draw_label(band: &mut [u8], w: u32, name: &str) {
    let shaped = cadcraft_fonts::shape_stroke(name, TEXT_H, 1.0, 0.0);
    let lines = shaped.outlines();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in lines.iter().flatten() {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) || x1 < x0 {
        return;
    }
    let (bw, bh) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
    let avail = (f64::from(w) - 8.0).max(1.0);
    let scale = (avail / bw).min(TEXT_H / bh).min(1.0);
    let ox = (f64::from(w) - bw * scale) / 2.0;
    let oy = (f64::from(BAND) - bh * scale) / 2.0;
    // Glyph y runs up; pixel y runs down.
    let map = |p: &cadcraft_geom::Vec2| ((p.x - x0) * scale + ox, (y1 - p.y) * scale + oy);
    for line in &lines {
        let mut prev: Option<(f64, f64)> = None;
        for p in line {
            let cur = map(p);
            if let Some(a) = prev {
                let steps = ((cur.0 - a.0).abs().max((cur.1 - a.1).abs()) * 2.0).ceil().clamp(1.0, 4096.0);
                let mut i = 0.0;
                while i <= steps {
                    let t = i / steps;
                    plot(band, w, (a.0 + (cur.0 - a.0) * t).floor(), (a.1 + (cur.1 - a.1) * t).floor());
                    i += 1.0;
                }
            } else {
                plot(band, w, cur.0.floor(), cur.1.floor());
            }
            prev = Some(cur);
        }
    }
}

/// Join eight equal-size RGBA images into a sheet of width `8 w` and height `h + 24`: a label
/// band on top with each rung's name, the images unchanged below at `x = n w`.
pub(crate) fn compose_sheet(images: &[(u32, u32, Vec<u8>)], names: &[&str]) -> Result<(u32, u32, Vec<u8>), String> {
    if images.len() != 8 || names.len() != 8 {
        return Err(format!("the sheet needs 8 images and 8 names (got {} and {})", images.len(), names.len()));
    }
    let (w, h) = images.first().map(|i| (i.0, i.1)).ok_or("no images")?;
    if w == 0 || h == 0 {
        return Err("the rung images are empty".to_string());
    }
    for (i, (iw, ih, px)) in images.iter().enumerate() {
        if *iw != w || *ih != h {
            return Err(format!("rung {i} is {iw}x{ih} but rung 0 is {w}x{h}"));
        }
        if px.len() as u64 != u64::from(w) * u64::from(h) * 4 {
            return Err(format!("rung {i} has the wrong pixel count"));
        }
    }
    let sw = w.checked_mul(8).ok_or("the sheet is too wide")?;
    let sh = h.checked_add(BAND).ok_or("the sheet is too tall")?;
    if u64::from(sw) * u64::from(sh) > MAX_SHEET_PIXELS {
        return Err("the sheet is too large".to_string());
    }
    let (wz, hz, swz) = (w as usize, h as usize, sw as usize);
    let mut out = vec![0u8; swz * sh as usize * 4];
    for (n, ((_, _, img), name)) in images.iter().zip(names).enumerate() {
        let mut band = vec![0u8; wz * BAND as usize * 4];
        for px in band.chunks_exact_mut(4) {
            px.copy_from_slice(&BAND_BG);
        }
        draw_label(&mut band, w, name);
        for y in 0..BAND as usize {
            let src = band.get(y * wz * 4..(y + 1) * wz * 4).ok_or("band row")?;
            let at = (y * swz + n * wz) * 4;
            out.get_mut(at..at + wz * 4).ok_or("sheet row")?.copy_from_slice(src);
        }
        for y in 0..hz {
            let src = img.get(y * wz * 4..(y + 1) * wz * 4).ok_or("image row")?;
            let at = ((y + BAND as usize) * swz + n * wz) * 4;
            out.get_mut(at..at + wz * 4).ok_or("sheet row")?.copy_from_slice(src);
        }
    }
    Ok((sw, sh, out))
}

fn run_ladder(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let camera = params.get("camera").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if camera.is_empty() {
        return Err(bad("render.ladder", "`camera` must name a saved camera"));
    }
    let out =
        params.get("out").and_then(Value::as_str).filter(|o| !o.is_empty()).ok_or_else(|| bad("render.ladder", "`out` must be a folder path"))?;
    let doc = s.doc()?;
    if doc.arch.camera_by_name(camera).is_none() {
        return Err(bad("render.ladder", format!("no camera named `{camera}`")));
    }
    let hooks = render_hooks().ok_or_else(|| bad("render.ladder", "render.ladder needs the 3D renderer: run it from cadcraft-cli run or the app"))?;
    let out = Path::new(out);
    // Refuse a bad request before any rung renders or any folder exists.
    let all: Vec<Value> = (0..=7u8).map(|n| rung_params(params, n, out)).collect();
    for p in &all {
        build_request(doc, p)?;
    }
    std::fs::create_dir_all(out).map_err(|e| bad("render.ladder", format!("cannot create `{}`: {e}", out.display())))?;
    let seed = params.get("seed").cloned().unwrap_or(json!(0));
    let mut rungs = Vec::new();
    let mut images = Vec::new();
    for (n, p) in all.iter().enumerate() {
        let name = RUNG_NAMES.get(n).copied().unwrap_or("");
        let request = build_request(doc, p)?;
        let mut reply = (hooks.render)(&request).map_err(|e| bad("render.ladder", format!("rung {n} ({name}) failed: {e}")))?;
        if let Value::Object(m) = &mut reply {
            m.insert("rung".to_string(), json!(n));
            m.insert("name".to_string(), json!(name));
            m.insert("seed".to_string(), seed.clone());
        }
        rungs.push(reply);
        let file = out.join(format!("rung-{n}.png"));
        let bytes = std::fs::read(&file).map_err(|e| bad("render.ladder", format!("rung {n} ({name}): cannot read `{}`: {e}", file.display())))?;
        let r = cadcraft_io::raster::decode(&bytes).map_err(|e| bad("render.ladder", format!("rung {n} ({name}): {e}")))?;
        images.push((r.width(), r.height(), r.rgba().to_vec()));
    }
    let (sw, sh, pixels) = compose_sheet(&images, &RUNG_NAMES).map_err(|e| bad("render.ladder", e))?;
    let png = cadcraft_io::png::encode_rgba8(sw, sh, &pixels).map_err(|e| bad("render.ladder", e.to_string()))?;
    let sheet = out.join("ladder.png");
    #[cfg(not(target_arch = "wasm32"))]
    crate::atomic_write::write_atomic_making_folders(&sheet, &png)
        .map_err(|e| bad("render.ladder", format!("cannot write `{}`: {e}", sheet.display())))?;
    #[cfg(target_arch = "wasm32")]
    {
        let _ = &png;
        return Err(bad("render.ladder", "writing the contact sheet is not available in the web build"));
    }
    let (w, h) = images.first().map_or((0, 0), |i| (i.0, i.1));
    Ok(json!({"ok": true, "camera": camera, "size": [w, h], "rungs": rungs, "sheet": sheet.to_string_lossy()}))
}

/// Command specs.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("render.ladder", "Realism Ladder", run_ladder)
            .menu(&["Architecture", "Realism Ladder"])
            .params("{camera: name, out: folder, size?: [w,h], spp?, seed?, noiseTarget?, timeBudget?}")
            .noundo(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, v: u8) -> (u32, u32, Vec<u8>) {
        (w, h, vec![v; (w * h * 4) as usize])
    }

    fn band_of(sheet: &(u32, u32, Vec<u8>), n: u32, w: u32) -> Vec<[u8; 4]> {
        let mut v = Vec::new();
        for y in 0..BAND {
            for x in 0..w {
                let i = ((y * sheet.0 + n * w + x) * 4) as usize;
                v.push([sheet.2[i], sheet.2[i + 1], sheet.2[i + 2], sheet.2[i + 3]]);
            }
        }
        v
    }

    #[test]
    fn ladder_refuses_without_a_saved_camera() {
        let dir = std::env::temp_dir().join("cadcraft-ladder-refusal-test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Session::new();
        let out = json!(dir.to_string_lossy());
        for p in [json!({"out": out}), json!({"camera": "nope", "out": out}), json!({"camera": "nope"}), json!({"camera": 3, "out": 4})] {
            assert!(s.execute("render.ladder", &p).is_err(), "{p}");
        }
        assert!(!dir.exists());
    }

    #[test]
    fn rung_params_carry_camera_seed_and_rung() {
        let base = json!({"camera": "front", "out": "ignored", "size": [64, 48], "spp": 4, "noiseTarget": 0.05, "timeBudget": 9, "seed": 7});
        let p = rung_params(&base, 5, Path::new("/tmp/x"));
        assert_eq!(p["camera"], "front");
        assert_eq!(p["rung"], 5);
        assert_eq!(p["seed"], 7);
        assert_eq!(p["size"], json!([64, 48]));
        assert_eq!(p["spp"], 4);
        assert_eq!(p["noiseTarget"], 0.05);
        assert_eq!(p["timeBudget"], 9);
        assert_eq!(p["out"], "/tmp/x/rung-5.png");
        let q = rung_params(&json!({"camera": "a"}), 0, Path::new("o"));
        assert_eq!(q["seed"], 0);
        assert!(q.get("spp").is_none() && q.get("size").is_none());
    }

    #[test]
    fn sheet_is_eight_columns_under_a_label_band() {
        let imgs: Vec<_> = (0..8).map(|i| solid(64, 10, 100 + i as u8)).collect();
        let (sw, sh, px) = compose_sheet(&imgs, &RUNG_NAMES).unwrap();
        assert_eq!((sw, sh), (512, 34));
        assert_eq!(px.len(), (512 * 34 * 4) as usize);
        assert!(compose_sheet(&imgs[..7], &RUNG_NAMES).is_err());
        let mut odd = imgs.clone();
        odd[3] = solid(63, 10, 0);
        assert!(compose_sheet(&odd, &RUNG_NAMES).is_err());
        let sheet = (sw, sh, px);
        for n in 0..8 {
            let band = band_of(&sheet, n, 64);
            let inked = band.iter().filter(|p| **p != BAND_BG).count();
            assert!(band.iter().all(|p| *p == BAND_BG || *p == INK));
            assert!(inked >= 10 && inked <= band.len() / 2, "rung {n}: {inked} inked");
        }
    }

    #[test]
    fn sheet_labels_differ_and_images_are_copied_exactly() {
        let imgs: Vec<(u32, u32, Vec<u8>)> = (0..8).map(|i| (32, 6, (0..32 * 6 * 4).map(|k| (k * 7 + i * 31) as u8).collect())).collect();
        let sheet = compose_sheet(&imgs, &RUNG_NAMES).unwrap();
        let bands: Vec<_> = (0..8).map(|n| band_of(&sheet, n, 32)).collect();
        for a in 0..8 {
            for b in a + 1..8 {
                assert_ne!(bands[a], bands[b], "bands {a} and {b} are equal");
            }
        }
        for (n, (_, _, img)) in imgs.iter().enumerate() {
            for y in 0..6usize {
                let at = ((y + BAND as usize) * 256 + n * 32) * 4;
                assert_eq!(&sheet.2[at..at + 128], &img[y * 128..(y + 1) * 128]);
            }
        }
    }

    #[test]
    fn ladder_command_is_registered_with_its_label() {
        let spec = crate::cmd::command_specs().into_iter().find(|c| c.id == "render.ladder");
        assert_eq!(spec.map(|c| c.label.to_string()).as_deref(), Some("Realism Ladder"));
    }

    #[test]
    fn compose_sheet_refuses_wrong_pixel_count() {
        let mut imgs: Vec<_> = (0..8).map(|i| solid(64, 10, 100 + i as u8)).collect();
        // Right dimensions, wrong pixel count.
        imgs[0] = (64, 10, vec![0u8; 2561]);
        let err = compose_sheet(&imgs, &RUNG_NAMES).unwrap_err();
        assert!(err.contains("wrong pixel count"), "{err}");
    }

    #[test]
    fn compose_sheet_refuses_different_height() {
        let mut imgs: Vec<_> = (0..8).map(|i| solid(64, 10, 100 + i as u8)).collect();
        imgs[3] = solid(64, 11, 0);
        let err = compose_sheet(&imgs, &RUNG_NAMES).unwrap_err();
        assert!(err.contains("rung 3"), "{err}");
    }

    #[test]
    fn compose_sheet_refuses_sheet_too_large() {
        let w = 1000u32;
        let h = 50000u32;
        // 8 * 1000 * (50000 + 24) = 400,192,000 > 400,000,000.
        let imgs: Vec<_> = (0..8).map(|i| (w, h, vec![100u8 + i as u8; (w * h * 4) as usize])).collect();
        let err = compose_sheet(&imgs, &RUNG_NAMES).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn compose_sheet_handles_1x1_image() {
        let imgs: Vec<_> = (0..8).map(|i| solid(1, 1, 100 + i as u8)).collect();
        let (sw, sh, px) = compose_sheet(&imgs, &RUNG_NAMES).unwrap();
        assert_eq!((sw, sh), (8, 25));
        assert_eq!(px.len(), (8 * 25 * 4) as usize);
    }

    #[test]
    fn rung_params_ignores_extra_fields() {
        let base = json!({"camera": "front", "extra": "ignored", "another": 42});
        let p = rung_params(&base, 0, Path::new("/tmp/x"));
        assert_eq!(p["camera"], "front");
        assert!(p.get("extra").is_none());
        assert!(p.get("another").is_none());
    }

    #[test]
    fn rung_params_replaces_out() {
        let base = json!({"camera": "front", "out": "original.png"});
        let p = rung_params(&base, 3, Path::new("/tmp/x"));
        assert_eq!(p["out"], "/tmp/x/rung-3.png");
    }

    #[test]
    fn draw_label_handles_very_small_width() {
        let mut band = vec![0u8; 1 * BAND as usize * 4];
        for px in band.chunks_exact_mut(4) {
            px.copy_from_slice(&BAND_BG);
        }
        draw_label(&mut band, 1, "Photoreal");
        assert_eq!(band.len(), 1 * BAND as usize * 4);
    }
}
