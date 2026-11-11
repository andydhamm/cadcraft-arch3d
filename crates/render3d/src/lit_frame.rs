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
    sun_key: Option<(u64, bool, [u32; 3])>,
    sun_map: Option<crate::shadow::ShadowMap>,
    /// Slots in the last frame.
    slots: usize,
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// The sun (`None` = night: the sun is off).
    pub sun: Option<Light>,
    /// Build and use the sun's shadow map (`--sun` / a site sun; the default light casts none).
    pub sun_shadow: bool,
    /// Lights shadowed at most (`lights::DEFAULT_SHADOW_BUDGET`); the rest light unshadowed.
    pub budget: usize,
    /// Display units per lux (`lights::DEFAULT_EXPOSURE`).
    pub exposure: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting {
            sun: Some(Light::DEFAULT),
            sun_shadow: false,
            budget: crate::lights::DEFAULT_SHADOW_BUDGET,
            exposure: crate::lights::DEFAULT_EXPOSURE,
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

        // The sun map, once per scene and direction.
        match lighting.sun.filter(|_| lighting.sun_shadow) {
            Some(sun) => {
                let d = sun.normalized().dir;
                let sk = (scene_key, ground, [d[0].to_bits(), d[1].to_bits(), d[2].to_bits()]);
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
        let (_, meshes) = self.meshes.as_ref()?;
        let before = atlas.maps_built();
        let slots = atlas.update(&self.device, &self.queue, meshes, occluders, &scene.lights, &self.lit_state.shadowed, far);
        self.shadows_built += atlas.maps_built().saturating_sub(before);
        self.lit_state.slots = slots.len();

        let sun_map = self.lit_state.sun_map.as_ref();
        let params = crate::lit::LitParams {
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
        frame.lighting = *lighting;
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
}
