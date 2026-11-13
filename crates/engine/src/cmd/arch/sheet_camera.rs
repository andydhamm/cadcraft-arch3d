//! A saved camera's render placed on a sheet (phase 21).

use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use cadcraft_arch3d::camera::{Camera, FullCamera, Orbit};
use cadcraft_doc::Handle;
use cadcraft_doc::arch::sheet::Sheet;
use cadcraft_doc::arch::{Camera3d, CameraKind, LINK_SHEET_IMAGE, SheetSource, SheetView, read_link, set_link};
use cadcraft_geom::Vec3;
use serde_json::{Value, json};

use crate::Session;
use crate::cmd::arch::look::{effective_look, effective_sun_dir};
use crate::cmd::arch::render::{FinalMode, PathParams, RenderRequest, SunChoice, render_hooks};
use crate::cmd::bad;
use crate::cmd::file::base64_encode;
use crate::error::Result;

fn sanitize_sheet_image_name(s: &str) -> String {
    let safe: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
    if safe.is_empty() { "sheet".to_string() } else { safe }
}

fn camera_from_saved(c: &Camera3d) -> Result<Camera> {
    match c.kind {
        CameraKind::Orbit | CameraKind::Overview => {
            let target = c.target.unwrap_or([0.0; 3]);
            let azimuth = c.azimuth_deg.unwrap_or(0.0);
            let elevation = c.elevation_deg.unwrap_or(30.0);
            let distance = c.distance.unwrap_or(10.0);
            let fov = c.fov_deg.unwrap_or(45.0);
            let target_vec = Vec3::new(target[0], target[1], target[2]);
            Ok(Camera::Orbit(Orbit::new(target_vec, azimuth, elevation, distance, fov)))
        }
        CameraKind::Full => {
            let eye_x = c.eye.as_ref().map_or(0.0, |e| e[0]);
            let eye_y = c.eye.as_ref().map_or(0.0, |e| e[1]);
            let eye = Vec3::new(eye_x, eye_y, c.eye_height.unwrap_or(1.6));
            let target = c.target.unwrap_or([0.0; 3]);
            let fov = c.fov_deg.unwrap_or(45.0);
            let target_vec = Vec3::new(target[0], target[1], target[2]);
            Ok(Camera::Full(FullCamera::new(eye, target_vec, fov)))
        }
    }
}

fn render_request_for_camera<'a>(doc: &'a cadcraft_doc::Drawing, saved: &'a Camera3d, pixels: [u32; 2], out: PathBuf) -> Result<RenderRequest<'a>> {
    let camera = camera_from_saved(saved)?;
    let mut look = effective_look(&doc.arch, Some(saved));
    let sun_dir = effective_sun_dir(&doc.arch, &look, None);
    let sun = if !look.sun.on {
        SunChoice::Off
    } else {
        match sun_dir {
            Some(d) if d[2] > 0.0 => SunChoice::Dir(d),
            _ => SunChoice::Default,
        }
    };
    let req = RenderRequest::new(doc, Some(camera), (pixels[0], pixels[1]), Some(out), false, sun, "shaded".into(), false, 8, Vec::new())
        .with_light_set(look.light_set.clone())
        .with_final(FinalMode::default(), PathParams::default())
        .with_look(look, sun_dir);
    Ok(req)
}

/// Place the render of the camera view `view` on the sheet's layout; the image's name when one is made.
/// The temporary PNG a sheet camera renders into. The web build has no renderer for it: an error there.
#[cfg(not(target_arch = "wasm32"))]
fn render_target(sheet_name: &str, number: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    path.push(format!("cadcraft-sheet-camera-{sheet_name}-{stamp}-{number}.png"));
    Ok(path)
}

#[cfg(target_arch = "wasm32")]
fn render_target(_sheet_name: &str, _number: &str) -> Result<PathBuf> {
    Err(bad("sheet-camera", "3D images on sheets need the desktop app or cadcraft-cli's renderer"))
}

pub(crate) fn place_camera_image(s: &mut Session, sheet: &str, view: &SheetView) -> Result<Option<String>> {
    let SheetSource::Camera { name, width, pixels } = &view.source else {
        return Ok(None);
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(bad("sheet-camera", "`camera` is empty"));
    }
    if !width.is_finite() || *width <= 0.0 {
        return Err(bad("sheet-camera", "`camera.width` must be finite and greater than zero"));
    }
    let [w, h] = *pixels;
    if !(16..=4096).contains(&w) || !(16..=4096).contains(&h) {
        return Err(bad("sheet-camera", "`camera.pixels` must have each side between 16 and 4096"));
    }
    let hooks = render_hooks().ok_or_else(|| bad("sheet-camera", "3D images on sheets need the desktop app or cadcraft-cli's renderer"))?;
    let doc = s.doc()?;
    let saved = doc.arch.camera_by_name(name).ok_or_else(|| bad("sheet-camera", format!("no camera named `{name}`")))?;
    let sheet_name = sheet.trim();
    if sheet_name.is_empty() {
        return Err(bad("sheet-camera", "`sheet` is empty"));
    }
    if doc.arch.sheet(sheet_name).is_none() {
        return Err(bad("sheet-camera", format!("no sheet named `{sheet_name}`")));
    }

    let out = render_target(sheet_name, &view.number.to_string())?;

    let req = render_request_for_camera(doc, saved, [w, h], out.clone())?;
    let _ = (hooks.render)(&req).map_err(|e| bad("sheet-camera", e))?;
    let bytes = std::fs::read(&out).map_err(|e| bad("sheet-camera", format!("render failed to write `{}`: {e}", out.display())))?;
    let _ = std::fs::remove_file(&out);

    let render_width_mm = width * 1000.0;
    let render_height_mm = render_width_mm * (f64::from(h) / f64::from(w));
    let image_name = format!("sheet-{}-{}.png", sanitize_sheet_image_name(sheet_name), view.number);
    let lower_left = [view.at[0] - render_width_mm / 2.0, view.at[1] - render_height_mm / 2.0];

    let previous_space = s.state()?.space.clone();
    s.state_mut()?.space = cadcraft_doc::Space::Paper(sheet_name.to_string());
    let value = json!({
        "path": out.to_string_lossy().to_string(),
        "data": base64_encode(&bytes),
        "at": [lower_left[0], lower_left[1]],
        "width": render_width_mm,
        "name": image_name,
    });
    let attach_result = crate::cmd::image::run_imageattach(s, &value);
    s.state_mut()?.space = previous_space;
    let image = attach_result?;
    let handle = image
        .get("handle")
        .and_then(Value::as_str)
        .and_then(Handle::parse_hex)
        .ok_or_else(|| bad("sheet-camera", "image attach did not return a handle"))?;
    s.doc_mut()?.modify_entity(handle, |e| {
        set_link(&mut e.common.xdata, LINK_SHEET_IMAGE, sheet_name, view.number);
    })?;

    let mut doc = s.doc_mut()?;
    if let Some(sheet) = doc.arch.sheet_mut(sheet_name) {
        if let Some(view) = sheet.views.iter_mut().find(|v| v.number == view.number) {
            view.size = [render_width_mm, render_height_mm];
        }
    }

    Ok(Some(image_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::file::set_image_decoder;
    use cadcraft_doc::Space;
    use cadcraft_doc::arch::{Sheet, SheetSource, SheetView};

    fn fake_render(req: &RenderRequest<'_>) -> std::result::Result<Value, String> {
        let out = req.out.as_ref().ok_or_else(|| "missing image path".to_string())?;
        let raster = cadcraft_doc::Raster::new(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]).unwrap();
        let png = cadcraft_io::raster::encode_png(&raster).map_err(|e| format!("encode PNG: {e}"))?;
        std::fs::write(out, png).map_err(|e| format!("write PNG: {e}"))?;
        Ok(json!({"ok": true, "path": out.to_string_lossy()}))
    }

    fn fake_stats() -> crate::RenderStats {
        crate::RenderStats::default()
    }

    fn ensure_renderer() {
        if render_hooks().is_none() {
            crate::set_render_hooks(crate::RenderHooks::new(fake_render, fake_stats));
        }
        set_image_decoder(|b| cadcraft_io::raster::decode(b).map_err(|e| e.to_string()));
    }

    #[test]
    fn camera_image_lands_in_paper_space_linked() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        s.state_mut().unwrap().space = Space::Model;
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: "Cover".into(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let image = place_camera_image(&mut s, "Cover", &view).unwrap().unwrap();
        assert_eq!(image, "sheet-Cover-1.png");
        let sheet = s.doc().unwrap().arch.sheet("Cover").unwrap();
        assert_eq!(sheet.views[0].size, [200.0, 100.0]);
        let paper = s.doc().unwrap().space(&Space::Paper("Cover".into())).unwrap();
        let found = paper.iter().find(|e| read_link(&e.common.xdata) == Some((LINK_SHEET_IMAGE.to_string(), "Cover".to_string(), 1))).unwrap();
        assert!(matches!(found.kind, cadcraft_doc::EntityKind::Image(_)));
        assert_eq!(s.state().unwrap().space, Space::Model);
    }

    #[test]
    fn unknown_camera_is_refused() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "missing".into(), width: 0.2, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let err = place_camera_image(&mut s, "Cover", &view).unwrap_err().to_string();
        assert!(err.contains("no camera named"), "{err}");
    }

    #[test]
    fn no_renderer_is_a_clear_error() {
        if render_hooks().is_some() {
            return;
        }
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let err = place_camera_image(&mut s, "Cover", &view).unwrap_err().to_string();
        assert!(err.contains("desktop app or cadcraft-cli's renderer"), "{err}");
    }

    #[test]
    fn image_size_follows_width_and_pixels() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [200, 100] },
            scale: 0.0,
            at: [10.0, 20.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        place_camera_image(&mut s, "Cover", &view).unwrap();
        let sheet = s.doc().unwrap().arch.sheet("Cover").unwrap();
        assert_eq!(sheet.views[0].size, [200.0, 100.0]);
    }

    #[test]
    fn nan_width_is_refused() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: f64::NAN, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let err = place_camera_image(&mut s, "Cover", &view).unwrap_err().to_string();
        assert!(err.contains("finite"), "{err}");
    }

    #[test]
    fn zero_width_is_refused() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.0, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let err = place_camera_image(&mut s, "Cover", &view).unwrap_err().to_string();
        assert!(err.contains("greater than zero"), "{err}");
    }

    #[test]
    fn pixels_out_of_range_are_refused() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [15, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        let err = place_camera_image(&mut s, "Cover", &view).unwrap_err().to_string();
        assert!(err.contains("between 16 and 4096"), "{err}");
    }

    #[test]
    fn image_name_sanitizes_special_chars() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover Sheet!", 3));
        d.arch.sheets.push(Sheet { name: "Cover Sheet!".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover Sheet!").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [200, 100] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover Sheet!").unwrap().views[0].clone();
        let image = place_camera_image(&mut s, "Cover Sheet!", &view).unwrap().unwrap();
        assert_eq!(image, "sheet-CoverSheet-1.png");
    }

    #[test]
    fn tall_aspect_ratio_height_is_correct() {
        ensure_renderer();
        let mut s = Session::new();
        s.new_drawing(true);
        let d = s.doc_mut().unwrap();
        d.arch.sheets.clear();
        d.layouts.push(cadcraft_doc::Layout::new("Cover", 3));
        d.arch.sheets.push(Sheet { name: "Cover".into(), number: "1".into(), title: String::new(), ..Sheet::default() });
        d.arch.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }).unwrap();
        d.arch.sheet_mut("Cover").unwrap().views.push(SheetView {
            number: 1,
            source: SheetSource::Camera { name: "front".into(), width: 0.2, pixels: [100, 200] },
            scale: 0.0,
            at: [0.0, 0.0],
            size: [0.0, 0.0],
            ..Default::default()
        });
        let view = s.doc().unwrap().arch.sheet("Cover").unwrap().views[0].clone();
        place_camera_image(&mut s, "Cover", &view).unwrap();
        let sheet = s.doc().unwrap().arch.sheet("Cover").unwrap();
        // width 0.2m = 200mm, pixels 100x200, so height = 200 * (200/100) = 400mm
        assert_eq!(sheet.views[0].size, [200.0, 400.0]);
    }
}
