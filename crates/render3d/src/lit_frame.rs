//! A lit frame of the interactive view (phase 10): pick the lights to draw and shadow, bring
//! their shadow maps up to date, pack the uniforms and bind group 2.
//!
//! The choice of shadowed lights (nearest and brightest, `lights::select_shadowed`) is made when
//! the scene or the lights change and kept while only the camera moves, so a camera move never
//! builds a shadow map. See [`Offscreen::lit_frame`].

use crate::light::Light;
use crate::lit::LitFrame;
use crate::offscreen::Offscreen;
use crate::types::Scene3d;

/// What the interactive view keeps between frames: the fixture atlas, the shadowed selection and
/// the scene/lights key it was made for, and the sun map.
#[derive(Default)]
pub struct LitState {
    pub(crate) atlas: Option<crate::shadow_atlas::ShadowAtlas>,
    /// The key the selection below was made for: scene, ground, lights hash, budget.
    selection_key: Option<(u64, bool, u64, usize)>,
    drawn: Vec<usize>,
    shadowed: Vec<usize>,
    /// `occluder_key` of the scene uploaded under this key.
    occluders: Option<(u64, u64)>,
    /// The sun map and the (scene, ground, direction bits) it was built for.
    sun_key: Option<(u64, bool, [u32; 3], Option<[[f32; 3]; 2]>)>,
    sun_map: Option<crate::shadow::ShadowMap>,
    /// Slots in the last frame.
    slots: usize,
    /// The convolved SH of the environment with this key (p19-env-raster).
    sh: Option<(u64, [[f32; 3]; 9])>,
}

/// A hash of every light's shadow key and intensity.
fn lights_hash(scene: &Scene3d) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    scene.lights.len().hash(&mut h);
    for l in &scene.lights {
        crate::shadow_atlas::light_key(l).hash(&mut h);
        l.intensity.to_bits().hash(&mut h);
    }
    h.finish()
}

/// What a lit frame lights with.
#[derive(Clone, Debug, PartialEq)]
pub struct Lighting {
    /// The sun (`None` = night: the sun is off).
    pub sun: Option<Light>,
    /// The moon as a directional light (direction, lux) when the sun is down and the sky is on.
    pub moon: Option<([f64; 3], f32)>,
    /// Build and use the sun's shadow map (`--sun` / a site sun; the default light casts none).
    pub sun_shadow: bool,
    /// Lights shadowed at most (`lights::DEFAULT_SHADOW_BUDGET`); the rest light unshadowed.
    pub budget: usize,
    /// Display units per lux (`lights::DEFAULT_EXPOSURE`). A value other than the default is a
    /// bare-number exposure of the request: the frame uses it as its fixed exposure scale.
    pub exposure: f32,
    /// The render look the frame is lit for (phase 19; read by later packages).
    pub look: cadcraft_doc::arch::Look,
    /// The explicit toward-sun direction in drawing space, when one was given.
    pub sun_dir: Option<[f64; 3]>,
    /// The calibration albedo: every surface reads as plain Lambert with it (`render {albedo}`).
    pub lambert: Option<f32>,
    /// Meter an automatic exposure on a frame without daylight (false keeps the legacy 0.01 scale,
    /// as `render {sun: false}` and hand-built night lightings always had).
    pub meter_night: bool,
    /// The environment (light and/or background) the frame lights with; `None` when there is
    /// none (p19-env-path).
    pub environment: Option<std::sync::Arc<crate::environment::Environment>>,
}

impl Lighting {
    /// Moon as the directional light only when the sun is below the horizon and the sky is on.
    pub fn with_moon(mut self, moon_dir: Option<[f64; 3]>) -> Self {
        self.moon = None;
        if !self.look.sky.on || self.sun.is_some() {
            return self;
        }
        let Some(dir) = moon_dir else {
            return self;
        };
        let length = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !length.is_finite() || length <= 0.0 {
            return self;
        }
        let d = [dir[0] / length, dir[1] / length, dir[2] / length];
        let lux = self.look.sky.moon.lux.max(0.0) as f32;
        if lux > 0.0 && d[2] > 0.0 {
            self.moon = Some((d, lux));
            // The raster's directional light is the moon, with no shadow map.
            self.sun = Some(Light::new([d[0] as f32, d[1] as f32, d[2] as f32], Light::DEFAULT.ambient));
            self.sun_shadow = false;
        }
        self
    }

    /// The directional light's lux: the moon's while the moon is the directional light, else
    /// `look.sun.intensity` while the sun is up, else 0.
    pub fn directional_lux(&self) -> f32 {
        if let Some((_, lux)) = self.moon {
            return lux;
        }
        match self.sun {
            Some(_) if self.look.sun.intensity.is_finite() => self.look.sun.intensity.max(0.0) as f32,
            _ => 0.0,
        }
    }
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting {
            sun: Some(Light::DEFAULT),
            moon: None,
            sun_shadow: false,
            budget: crate::lights::DEFAULT_SHADOW_BUDGET,
            exposure: crate::lights::DEFAULT_EXPOSURE,
            look: cadcraft_doc::arch::Look::default(),
            sun_dir: None,
            lambert: None,
            meter_night: false,
            environment: None,
        }
    }
}

impl Offscreen {
    /// Lights shadowed in the last [`Offscreen::lit_frame`].
    pub fn shadowed_lights(&self) -> usize {
        self.lit_state.slots
    }

    /// Group 2 for drawing `scene` (uploaded under `scene_key`, with the ground quad when
    /// `ground`) seen from `eye`, lit by `lighting` and the scene's fixture lights. `None` when the
    /// lit pipelines are unavailable; the caller then draws without fixture lights.
    pub fn lit_frame(&mut self, scene: &Scene3d, scene_key: u64, eye: [f64; 3], lighting: &Lighting, ground: bool) -> Option<LitFrame> {
        if self.lit_pipelines.is_none() {
            self.lit_pipelines = crate::lit::LitPipelines::new(&self.device, &self.gpu);
        }
        self.lit_pipelines.as_ref()?;
        self.uploaded(scene, scene_key, ground)?;

        // Selection: remade only when the scene, ground, lights or budget changed.
        let key = (scene_key, ground, lights_hash(scene), lighting.budget);
        if self.lit_state.selection_key != Some(key) {
            let drawn = crate::lights::select_drawn(&scene.lights, eye);
            let shadowed: Vec<usize> =
                crate::lights::select_shadowed(&scene.lights, eye, lighting.budget).into_iter().filter(|i| drawn.contains(i)).collect();
            self.lit_state.drawn = drawn;
            self.lit_state.shadowed = shadowed;
            self.lit_state.selection_key = Some(key);
        }
        let occluders = match self.lit_state.occluders {
            Some((k, o)) if k == scene_key => o,
            _ => {
                let o = crate::shadow_atlas::occluder_key(scene);
                self.lit_state.occluders = Some((scene_key, o));
                o
            }
        };

        // The view's clip box: geometry outside it casts no shadow, and both shadow caches key on it.
        let clip = crate::frame_look::look_clip(&lighting.look);
        self.shadow_clip = clip;

        // The sun map, once per scene, direction and clip box.
        match lighting.sun.filter(|_| lighting.sun_shadow) {
            Some(sun) => {
                let d = sun.normalized().dir;
                let sk = (scene_key, ground, [d[0].to_bits(), d[1].to_bits(), d[2].to_bits()], clip);
                if self.lit_state.sun_key != Some(sk) {
                    self.lit_state.sun_map = self.build_shadow(scene, scene_key, &sun, ground);
                    self.lit_state.sun_key = Some(sk);
                }
            }
            None => {
                self.lit_state.sun_map = None;
                self.lit_state.sun_key = None;
            }
        }

        // The fixture atlas.
        let far = {
            let (a, b) = (scene.bounds.min, scene.bounds.max);
            let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            if d.is_finite() { (d + 1.0).max(1.0) as f32 } else { 1.0 }
        };
        let atlas =
            self.lit_state.atlas.get_or_insert_with(|| crate::shadow_atlas::ShadowAtlas::new(&self.device, crate::lights::INTERACTIVE_SHADOW_SIZE));
        atlas.set_clip(clip);
        let (_, meshes) = self.meshes.as_ref()?;
        let before = atlas.maps_built();
        let slots = atlas.update(&self.device, &self.queue, meshes, occluders, &scene.lights, &self.lit_state.shadowed, far);
        self.shadows_built += atlas.maps_built().saturating_sub(before);
        self.lit_state.slots = slots.len();

        // The environment's diffuse light: nine convolved SH coefficients, projected once per key.
        let sh = match lighting.environment.as_deref().filter(|e| e.is_valid()) {
            Some(env) => {
                if self.lit_state.sh.as_ref().is_none_or(|(k, _)| *k != env.key) {
                    self.lit_state.sh = Some((env.key, crate::sh::convolve(&crate::sh::project(env))));
                }
                self.lit_state.sh.map(|(_, c)| c)
            }
            None => {
                self.lit_state.sh = None;
                None
            }
        };
        let sun_map = self.lit_state.sun_map.as_ref();
        let params = crate::lit::LitParams {
            sh,
            sun_on: lighting.sun.is_some(),
            ambient: lighting.sun.map_or(crate::lights::NIGHT_AMBIENT, |l| l.ambient),
            exposure: lighting.exposure,
            ..Default::default()
        };
        let layout = &self.lit_pipelines.as_ref()?.layout;
        let drawn = &self.lit_state.drawn;
        let mut frame = LitFrame::new(
            &self.device,
            layout,
            &crate::lit::frame_uniform(sun_map, &params, atlas.size()),
            sun_map.map(|m| &m.view),
            &crate::lights::pack_lights(&scene.lights, drawn, &slots),
            atlas.view(),
            &params,
            (drawn.len(), slots.len()),
        );
        frame.lighting = lighting.clone();
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch3d::light::{LightKind, SceneLight};
    use cadcraft_geom::{Vec2, Vec3};

    fn floor() -> crate::types::Mesh {
        let mut m = crate::types::Mesh::new("floor", crate::types::ArchId(1));
        let r = [Vec2::new(-8.0, -8.0), Vec2::new(8.0, -8.0), Vec2::new(8.0, 8.0), Vec2::new(-8.0, 8.0)];
        m.push_prism(&r, -0.1, 0.0);
        m
    }

    fn spot(id: u32, x: f64, y: f64) -> SceneLight {
        SceneLight {
            id: crate::types::ArchId(id),
            kind: LightKind::Spot,
            position: Vec3::new(x, y, 3.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 900.0,
            color: [1.0; 3],
            cos_inner: 30f64.to_radians().cos(),
            cos_outer: 35f64.to_radians().cos(),
            ..Default::default()
        }
    }

    fn scene(lights: Vec<SceneLight>) -> Scene3d {
        let m = floor();
        Scene3d { bounds: m.bounds(), meshes: vec![m], lights, ..Default::default() }
    }

    fn four() -> Vec<SceneLight> {
        vec![spot(10, -2.0, -2.0), spot(11, 2.0, -2.0), spot(12, -2.0, 2.0), spot(13, 2.0, 2.0)]
    }

    fn off() -> Option<Offscreen> {
        let Some((device, queue)) = crate::gputest::shared_device() else {
            eprintln!("SKIP: no GPU adapter available");
            return None;
        };
        Some(Offscreen::with_device(device, queue, (64, 64)))
    }

    fn orbit(deg: f64) -> [f64; 3] {
        let a = deg.to_radians();
        [10.0 * a.cos(), 10.0 * a.sin(), 6.0]
    }

    #[test]
    fn first_lit_frame_builds_one_map_per_shadowed_light() {
        let Some(mut o) = off() else { return };
        let s = scene(four());
        assert!(o.lit_frame(&s, 1, orbit(0.0), &Lighting::default(), false).is_some());
        assert_eq!(o.shadow_maps_built(), 4);
        assert_eq!(o.shadowed_lights(), 4);
    }

    #[test]
    fn thirty_camera_moves_build_no_shadow_maps() {
        let Some(mut o) = off() else { return };
        let s = scene(four());
        assert!(o.lit_frame(&s, 1, orbit(0.0), &Lighting::default(), false).is_some());
        let n = o.shadow_maps_built();
        for i in 1..=30 {
            assert!(o.lit_frame(&s, 1, orbit(i as f64), &Lighting::default(), false).is_some());
        }
        assert_eq!(o.shadow_maps_built(), n);
    }

    #[test]
    fn moving_one_fixture_rebuilds_one_map() {
        let Some(mut o) = off() else { return };
        let l = Lighting::default();
        assert!(o.lit_frame(&scene(four()), 1, orbit(0.0), &l, false).is_some());
        let n = o.shadow_maps_built();
        let mut moved = four();
        moved[2].position = Vec3::new(-1.0, 3.0, 3.0);
        assert!(o.lit_frame(&scene(moved), 2, orbit(0.0), &l, false).is_some());
        assert_eq!(o.shadow_maps_built(), n + 1);
    }

    #[test]
    fn budget_shadows_the_nearest_and_brightest() {
        let Some(mut o) = off() else { return };
        let lights: Vec<SceneLight> = (0..12).map(|i| spot(20 + i, (i % 4) as f64 * 2.0 - 3.0, (i / 4) as f64 * 2.0 - 2.0)).collect();
        let l = Lighting { budget: 8, ..Lighting::default() };
        let f = o.lit_frame(&scene(lights), 1, orbit(0.0), &l, false);
        assert!(f.is_some());
        let Some(f) = f else { return };
        assert_eq!(f.shadowed, 8);
        assert_eq!(f.lights, 12);
        assert_eq!(o.shadowed_lights(), 8);
    }

    #[test]
    fn night_frame_turns_the_sun_off() {
        let Some(mut o) = off() else { return };
        let l = Lighting { sun: None, ..Lighting::default() };
        let f = o.lit_frame(&scene(four()), 1, orbit(0.0), &l, false);
        assert!(f.is_some());
        let Some(f) = f else { return };
        assert_eq!(f.sun_intensity, 0.0);
    }

    #[test]
    fn empty_scene_builds_no_maps() {
        let Some(mut o) = off() else { return };
        let s = scene(vec![]);
        assert!(o.lit_frame(&s, 1, orbit(0.0), &Lighting::default(), false).is_some());
        assert_eq!(o.shadow_maps_built(), 0);
        assert_eq!(o.shadowed_lights(), 0);
    }

    #[test]
    fn sun_shadow_map_cached_across_camera_moves() {
        let Some(mut o) = off() else { return };
        let s = scene(four());
        let l = Lighting { sun_shadow: true, ..Lighting::default() };
        assert!(o.lit_frame(&s, 1, orbit(0.0), &l, false).is_some());
        // 4 fixture maps + 1 sun map
        let n = o.shadow_maps_built();
        assert_eq!(n, 5);
        for i in 1..=10 {
            assert!(o.lit_frame(&s, 1, orbit(i as f64), &l, false).is_some());
        }
        assert_eq!(o.shadow_maps_built(), n);
    }

    #[test]
    fn budget_larger_than_light_count_shadows_all() {
        let Some(mut o) = off() else { return };
        let l = Lighting { budget: 100, ..Lighting::default() };
        let f = o.lit_frame(&scene(four()), 1, orbit(0.0), &l, false);
        assert!(f.is_some());
        let Some(f) = f else { return };
        assert_eq!(f.shadowed, 4);
        assert_eq!(f.lights, 4);
        assert_eq!(o.shadowed_lights(), 4);
    }

    #[test]
    fn coincident_lights_all_shadowed() {
        let Some(mut o) = off() else { return };
        let lights = vec![spot(1, 0.0, 0.0), spot(2, 0.0, 0.0), spot(3, 0.0, 0.0)];
        let f = o.lit_frame(&scene(lights), 1, orbit(0.0), &Lighting::default(), false);
        assert!(f.is_some());
        let Some(f) = f else { return };
        assert_eq!(f.shadowed, 3);
        assert_eq!(f.lights, 3);
    }

    #[test]
    fn nan_light_position_does_not_panic() {
        let Some(mut o) = off() else { return };
        let mut lights = four();
        lights[0].position = Vec3::new(f64::NAN, 0.0, 3.0);
        let f = o.lit_frame(&scene(lights), 1, orbit(0.0), &Lighting::default(), false);
        assert!(f.is_some());
    }

    #[test]
    fn ground_change_produces_valid_frame() {
        let Some(mut o) = off() else { return };
        let s = scene(four());
        let l = Lighting::default();
        assert!(o.lit_frame(&s, 1, orbit(0.0), &l, false).is_some());
        assert!(o.lit_frame(&s, 1, orbit(0.0), &l, true).is_some());
    }

    #[test]
    fn with_moon_only_when_sun_down_and_sky_on() {
        let mut look = cadcraft_doc::arch::Look::default();
        look.sky.on = true;
        let zero = Lighting { look: look.clone(), sun: Some(Light::DEFAULT), ..Default::default() }.with_moon(Some([0.0, 0.0, 1.0]));
        assert!(zero.moon.is_none(), "sun in daylight overrides the moon");

        let moon = Lighting { look: look.clone(), sun: None, ..Default::default() }.with_moon(Some([0.0, 0.0, 1.0]));
        assert!(moon.moon.is_some());

        look.sky.on = false;
        let sky_off = Lighting { look, sun: None, ..Default::default() }.with_moon(Some([0.0, 0.0, 1.0]));
        assert!(sky_off.moon.is_none());
    }

    #[test]
    fn moon_direct_light() {
        let mut look = cadcraft_doc::arch::Look::default();
        look.sky.on = true;
        look.sun.on = false;
        let light = Lighting { look, sun: None, ..Default::default() }.with_moon(Some([0.0, 0.0, 1.0]));
        assert!(light.moon.is_some());
        assert!((light.directional_lux() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn direct_moon_is_lux_times_sin_elevation() {
        let mut look = cadcraft_doc::arch::Look::default();
        look.sky.on = true;
        look.sun.on = false;
        let el = 30.0_f64;
        let dir = [0.0, el.to_radians().cos(), el.to_radians().sin()];
        let light = Lighting { look, sun: None, ..Default::default() }.with_moon(Some(dir));
        let Some((moon_dir, lux)) = light.moon else {
            panic!("moon must be active");
        };
        assert!((lux - 0.25).abs() < 1e-6);
        assert!((moon_dir[2] - el.to_radians().sin()).abs() < 1e-6);
    }
}
