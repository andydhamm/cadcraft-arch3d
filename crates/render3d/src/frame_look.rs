//! The per-frame look of the raster views (phase 19 groundwork): everything a frame decides once
//! (exposure, tone map, ambient, clip box, near plane, Lambert override, the directional light's
//! lux) and the per-draw fill that copies it into each draw's [`PbrUniform`].
//!
//! The default [`FrameLook`] is the pre-phase-19 look: [`fill_draw`] then leaves every draw's block
//! exactly as [`PbrUniform::new`] made it. Later packages fill the stubs below, each labelled with
//! the package that owns it; none of them changes a layout or a signature.

use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::pipeline::PbrUniform;
use crate::types::Scene3d;

/// What one frame looks like, decided once and written into every draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameLook {
    /// Exposure scale (0 = unset: the pipeline's own).
    pub exposure: f32,
    /// Tone-map code (0 = the pipeline's own; see `PbrUniform::tone_map`).
    pub tone_map: u32,
    /// Ambient illuminance in lux (0 = unset).
    pub ambient_lux: f32,
    /// `PBR_FLAG_*` bits for every draw.
    pub flags: u32,
    /// Clip box `[min, max]` in drawing space, when the view is clipped.
    pub clip: Option<[[f32; 3]; 2]>,
    /// Near plane in metres, when the view sets one.
    pub near: Option<f32>,
    /// Lambert albedo override, when the view draws a plain Lambert look.
    pub lambert: Option<f32>,
    /// Lux of the raster's directional light (0 = none: at night it stays 0).
    pub sun_lux: f32,
}

/// What exposure metering remembers between frames [p19-exposure].
#[derive(Default)]
pub struct Meter {}

/// The look of one frame: the default, then [`exposure_look`], then [`view_look`]. `ground` is the
/// render's ground flag (the raster ground quad exists only at upload, so the meter must be told).
pub fn frame_look(scene: &Scene3d, camera: &Camera, size: (u32, u32), lighting: &Lighting, ground: bool, meter: &mut Meter) -> FrameLook {
    let mut f = FrameLook::default();
    exposure_look(&mut f, scene, camera, size, lighting, ground, meter);
    view_look(&mut f, lighting);
    f
}

/// Exposure, tone map, ambient lux and the sun's lux [p19-exposure]. A stub: leaves `f` alone.
pub fn exposure_look(
    _f: &mut FrameLook,
    _scene: &Scene3d,
    _camera: &Camera,
    _size: (u32, u32),
    _lighting: &Lighting,
    _ground: bool,
    _meter: &mut Meter,
) {
}

/// Clip box, near plane and Lambert override of the view [p19-seeing-in-render]. A stub.
pub fn view_look(_f: &mut FrameLook, _lighting: &Lighting) {}

/// Copy the frame's look into one draw's block; `material` is the draw's mesh material name (the
/// ground quad's is `"ground"`).
pub fn fill_draw(p: &mut PbrUniform, f: &FrameLook, scene: &Scene3d, material: &str) {
    fill_exposure(p, f);
    fill_normal_map(p, f, scene, material);
    fill_clip(p, f);
}

/// Exposure, tone map and ambient lux into the draw [p19-exposure]. A stub.
pub fn fill_exposure(_p: &mut PbrUniform, _f: &FrameLook) {}

/// The draw's normal map layer and strength [p19-normal-maps-render]. A stub.
pub fn fill_normal_map(_p: &mut PbrUniform, _f: &FrameLook, _scene: &Scene3d, _material: &str) {}

/// Clip box and Lambert override into the draw [p19-seeing-in-render]. A stub.
pub fn fill_clip(_p: &mut PbrUniform, _f: &FrameLook) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Orbit;
    use crate::light::Light;
    use crate::pipeline::{Draw, STYLE_SHADED};

    #[test]
    fn default_frame_look_changes_no_draw() {
        let scene = Scene3d::default();
        let camera = Camera::Orbit(Orbit::at_origin(30.0, 30.0, 10.0, 45.0));
        let mut meter = Meter::default();
        let look = frame_look(&scene, &camera, (64, 64), &Lighting::default(), true, &mut meter);
        assert_eq!(look, FrameLook::default());
        let before = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_SHADED, [1.0, 2.0, 3.0]);
        let mut after = before;
        fill_draw(&mut after, &look, &scene, "wall");
        fill_draw(&mut after, &look, &scene, "ground");
        assert_eq!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&after));
    }
}
