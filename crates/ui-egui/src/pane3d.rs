//! The architectural 3D pane: a dockable panel showing the built arch scene.
//!
//! Owns an orbit camera and renders the scene offscreen into its own colour+depth
//! targets (same `egui_wgpu::Callback` pattern as [`gpu::install`]). The scene is
//! rebuilt only when `Drawing::arch_revision` advances since the last frame.
//! Without a GPU adapter the pane shows a static "3D not available" placeholder
//! rather than panicking or showing a blank window.
//!
//! Presentation (assisted building block, 2026-10-25, see `docs/arch3d/DESIGN.md`): [`install`]
//! puts a [`cadcraft_render3d::Offscreen`] on the window's device into the renderer's callback
//! resources. Each frame the pane's paint callback draws the scene into it only when the scene,
//! camera or size changed, then samples its colour target into the pane's rectangle.

use std::sync::Arc;

use egui_wgpu::wgpu;
use serde_json::json;

/// Orbit-camera parameters for the 3D pane.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    /// Angle around the vertical axis (radians).
    pub azimuth: f64,
    /// Angle from the horizontal plane (radians); positive = looking down.
    pub elevation: f64,
    /// Distance from the target point.
    pub distance: f64,
}

impl Orbit {
    const fn new_default() -> Self {
        Self { azimuth: 0.0, elevation: 0.35, distance: 10.0 }
    }
}

impl Default for Orbit {
    fn default() -> Self {
        Self::new_default()
    }
}

/// How the pane shades the scene (the header selector).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style3d {
    /// PBR-shaded faces.
    #[default]
    Shaded,
    /// Shaded faces with feature edges.
    ShadedEdges,
    /// Faces in the background colour with visible edges only.
    HiddenLine,
    /// Translucent faces: the whole building shows through.
    GlassHouse,
    /// Uniform matte clay with soft shading.
    Clay,
    /// Technical line-drawing look.
    Technical,
}

impl Style3d {
    /// Every style, in selector order.
    pub const ALL: [Style3d; 6] =
        [Style3d::Shaded, Style3d::ShadedEdges, Style3d::HiddenLine, Style3d::GlassHouse, Style3d::Clay, Style3d::Technical];

    /// The next style in selector order, wrapping after the last.
    pub fn next(self) -> Self {
        match self {
            Style3d::Shaded => Style3d::ShadedEdges,
            Style3d::ShadedEdges => Style3d::HiddenLine,
            Style3d::HiddenLine => Style3d::GlassHouse,
            Style3d::GlassHouse => Style3d::Clay,
            Style3d::Clay => Style3d::Technical,
            Style3d::Technical => Style3d::Shaded,
        }
    }

    /// The English label (translated at draw time).
    pub fn label(self) -> &'static str {
        match self {
            Style3d::Shaded => "Shaded",
            Style3d::ShadedEdges => "Shaded Edges",
            Style3d::HiddenLine => "Hidden Line",
            Style3d::GlassHouse => "Glass House",
            Style3d::Clay => "Clay",
            Style3d::Technical => "Technical",
        }
    }

    /// The renderer's style for this one.
    pub fn render_style(self) -> cadcraft_render3d::Style {
        match self {
            Style3d::Shaded => cadcraft_render3d::Style::Shaded,
            Style3d::ShadedEdges => cadcraft_render3d::Style::ShadedEdges,
            Style3d::HiddenLine => cadcraft_render3d::Style::HiddenLine,
            Style3d::GlassHouse => cadcraft_render3d::Style::GlassHouse,
            Style3d::Clay => cadcraft_render3d::Style::Clay,
            Style3d::Technical => cadcraft_render3d::Style::Technical,
        }
    }
}

/// Walk step in metres; Shift multiplies it by [`RUN_FACTOR`].
pub const WALK_STEP: f64 = 1.0;
/// Shift makes a step 4 m.
pub const RUN_FACTOR: f64 = 4.0;
/// Degrees turned per arrow-key press in the full camera.
pub const TURN_STEP_DEG: f64 = 5.0;

/// What the offscreen target (or a finished still) shows: scene key, size, view-projection,
/// style, lighting and the number of fixture lights drawn.
type ViewState = (u64, (u32, u32), [[f32; 4]; 4], Style3d, cadcraft_render3d::Lighting, usize);

/// A finished Finalize Render still and the view it was made for.
#[derive(Debug)]
struct FinalDone {
    state: ViewState,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Finalize Render state shared between the pane's UI and its paint callback.
#[derive(Debug)]
struct FinalUi {
    /// The Finalize button was pressed; the callback starts a render.
    request: bool,
    /// The Cancel button was pressed (or the view changed under a running render).
    cancel: bool,
    /// Progress 0..1 while a render runs.
    progress: Option<f32>,
    /// Why the last render failed.
    error: Option<String>,
    /// The view the running render was started for.
    started: Option<ViewState>,
    /// The finished still.
    done: Option<FinalDone>,
    /// Which Finalize the button runs: what render's `final: true` does until the user picks.
    mode: cadcraft_engine::FinalMode,
    /// The path tracer's target samples per pixel.
    samples: u32,
    /// (samples done, samples wanted) while a path-traced Finalize runs.
    path_progress: Option<(u32, u32)>,
    /// The last path Finalize ran as raster: the device has no compute shaders.
    fallback: bool,
}

impl Default for FinalUi {
    fn default() -> Self {
        FinalUi {
            request: false,
            cancel: false,
            progress: None,
            error: None,
            started: None,
            done: None,
            mode: cadcraft_engine::FinalMode::default(),
            samples: cadcraft_render3d::pathtrace::DEFAULT_SPP,
            path_progress: None,
            fallback: false,
        }
    }
}

/// The running path Finalize's label: samples done of samples wanted.
fn samples_label(done: u32, wanted: u32) -> String {
    crate::i18n::format("{spp} / {total} samples", &[("spp", done.to_string()), ("total", wanted.to_string())])
}

/// The adapter the pane's compute device comes from: the software one under test, else
/// `CADCRAFT_RENDER_ADAPTER` (as the CLI reads it), else the best one.
#[cfg(test)]
fn compute_adapter() -> cadcraft_render3d::adapter::AdapterChoice {
    cadcraft_render3d::adapter::AdapterChoice::Fallback
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn compute_adapter() -> cadcraft_render3d::adapter::AdapterChoice {
    use cadcraft_render3d::adapter::AdapterChoice;
    std::env::var("CADCRAFT_RENDER_ADAPTER").ok().and_then(|v| AdapterChoice::parse(&v)).unwrap_or(AdapterChoice::Auto)
}

/// The compute device for path-traced Finalize, asked for once.
#[cfg(not(target_arch = "wasm32"))]
fn request_pane_compute() -> Option<cadcraft_render3d::pathtrace::ComputeGpu> {
    cadcraft_render3d::pathtrace::request_compute(compute_adapter(), false).ok().flatten()
}

/// The web build has no compute device here: its adapter request cannot block, so a path
/// Finalize there is the raster still.
#[cfg(target_arch = "wasm32")]
fn request_pane_compute() -> Option<cadcraft_render3d::pathtrace::ComputeGpu> {
    None
}

impl FinalUi {
    /// Note the view being drawn: a still (or error) made for another view is dropped, and a
    /// running render for another view is cancelled.
    fn observe(&mut self, state: &ViewState) {
        if self.done.as_ref().is_some_and(|d| d.state != *state) {
            self.done = None;
        }
        if self.progress.is_some() && self.started.as_ref() != Some(state) {
            self.cancel = true;
        }
    }
}

fn lock_final(m: &std::sync::Mutex<FinalUi>) -> std::sync::MutexGuard<'_, FinalUi> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The lighting for `site`, `night`, `date` and `time`. `None` for the sun when the date or
/// time does not parse (the caller keeps its last good lighting).
fn compute_lighting(site: Option<&cadcraft_doc::arch::SiteLocation>, night: bool, date: &str, time: &str) -> Option<cadcraft_render3d::Lighting> {
    use cadcraft_render3d::{Light, Lighting};
    if night {
        return Some(Lighting { sun: None, sun_shadow: false, ..Lighting::default() });
    }
    let Some(site) = site else { return Some(Lighting::default()) };
    let sun = cadcraft_arch::sun::sun_position(site.latitude, date.trim(), time.trim())?;
    let d = cadcraft_arch::site::true_to_drawing(Some(site), cadcraft_arch::sun::sun_dir(&sun));
    let dir = [d[0] as f32, d[1] as f32, d[2] as f32];
    let light = Light::from_sun(&cadcraft_render3d::light::Sun { dir, ..Default::default() }, Light::DEFAULT.ambient);
    Some(Lighting { sun: Some(light), sun_shadow: true, ..Lighting::default() })
}

/// Per-frame state for the 3D pane.
#[derive(Debug)]
pub struct Pane3d {
    /// The current orbit camera.
    pub orbit: Orbit,
    /// The shading style.
    pub style: Style3d,
    /// The walk-through camera; `Some` replaces the orbit until [`Pane3d::use_orbit`].
    pub full: Option<cadcraft_render3d::camera::FullCamera>,
    /// Orbit target set by a loaded saved camera (`None` = the scene's centre).
    orbit_target: Option<cadcraft_geom::Vec3>,
    /// Orbit field of view set by a loaded saved camera.
    orbit_fov: Option<f64>,
    /// The saved camera last loaded from the drawing.
    loaded_camera: Option<cadcraft_doc::arch::Camera3d>,
    /// The proxy handle of the last 3D click that hit something, until the app takes it.
    picked: Option<cadcraft_doc::Handle>,
    /// The `arch_revision` the scene was last built at (0 = never built).
    pub last_revision: u64,
    /// Number of meshes in the last build (0 when no scene).
    pub mesh_count: usize,
    /// Whether a wgpu adapter is available (set on first frame if a render state is present).
    pub available: bool,
    /// Whether the built scene has visible content (walls, floors, etc.).
    pub has_content: bool,
    /// The last built scene (`None` until the first successful build).
    scene: Option<Arc<cadcraft_render3d::Scene3d>>,
    /// The incremental builder behind `scene`: seeded by the first build of a document,
    /// updated by later revisions, dropped when the document changes.
    cache: Option<cadcraft_arch3d::SceneCache>,
    /// What the last rebuild did (`None` until a build).
    pub last_rebuild: Option<cadcraft_arch3d::RebuildStats>,
    /// The document the scene was built from ([`Pane3d::sync_doc`]).
    doc: Option<u64>,
    /// Whether the orbit distance has been fitted to a scene yet.
    framed: bool,
    /// The rectangle the pane last drew into, in points.
    last_rect: Option<egui::Rect>,
    /// Night: no sun, only the fixture lights.
    pub night: bool,
    /// The sun's date, `YYYY-MM-DD`.
    pub sun_date: String,
    /// The sun's time, `HH:MM`.
    pub sun_time: String,
    /// The model's site, for the sun.
    site: Option<cadcraft_doc::arch::SiteLocation>,
    /// The lighting last computed, with the inputs it was computed from.
    lighting: cadcraft_render3d::Lighting,
    lighting_inputs: Option<(Option<cadcraft_doc::arch::SiteLocation>, bool, String, String)>,
    /// Whether the date and time entries parse.
    sun_valid: bool,
    /// Finalize Render state shared with the paint callback.
    finalize: Arc<std::sync::Mutex<FinalUi>>,
    /// The painter tool armed in the pane ([`PaintTool::Off`]: clicks select).
    pub tool: PaintTool,
    /// The painters' scope, an index into [`SCOPES`].
    pub scope: usize,
    /// The Material Painter blends with the paint already there rather than replacing it.
    pub blend: bool,
    /// The library item (or material) the painters apply: the last one clicked in the Library.
    pub paint_item: Option<String>,
    /// The library item armed for `library.place`, until the next click in the pane places it.
    pub armed: Option<String>,
    /// What the last painter or placement command reported.
    pub paint_note: String,
    /// What the last click in the pane struck, until [`apply_pick`] takes it.
    pub click: Option<Click>,
    /// Esc was pressed over the pane; [`apply_pick`] disarms.
    escaped: bool,
}

/// The painter tools of the pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaintTool {
    /// No painter: a click selects.
    #[default]
    Off,
    /// `paint.material`.
    Material,
    /// `paint.object`.
    Object,
    /// `paint.pick`.
    Eyedropper,
}

impl PaintTool {
    /// The three painters.
    pub const ALL: [PaintTool; 3] = [PaintTool::Material, PaintTool::Object, PaintTool::Eyedropper];

    /// The tool's label, which is also its command's.
    pub fn label(self) -> &'static str {
        match self {
            PaintTool::Off => "Select",
            PaintTool::Material => "Material Painter",
            PaintTool::Object => "Object Painter",
            PaintTool::Eyedropper => "Material Eyedropper",
        }
    }
}

/// The painters' scopes, as `paint.material` spells them.
pub const SCOPES: [&str; 5] = ["component", "object", "room", "floor", "plan"];

/// The label of scope `scope` (an index into [`SCOPES`]).
fn scope_label(scope: usize) -> &'static str {
    match scope {
        0 => "Component",
        1 => "Object",
        2 => "Room",
        3 => "Floor",
        _ => "Plan",
    }
}

/// What a click in the pane struck: the object and part, the mesh's material name, the surface
/// normal (turned to face the viewer), the point and the level elevation (NaN when unknown).
#[derive(Clone, Debug, PartialEq)]
pub struct Click {
    /// The arch object owning the struck mesh.
    pub object: cadcraft_doc::ArchId,
    /// Part name (`"wall"`, `"floor"`, `"cabinet"`, `"furnishing"`, ...).
    pub part: String,
    /// The kind of object owning the struck mesh (material-aware, unlike `part`).
    pub owner: cadcraft_arch3d::incremental::ObjectKind,
    /// The struck mesh's material name (`sym:<key>:<i>` for a furnishing).
    pub material: String,
    /// Unit normal of the struck face.
    pub normal: [f64; 3],
    /// World-space hit point.
    pub point: cadcraft_geom::Vec3,
    /// Elevation of the level the mesh belongs to.
    pub level: f64,
}

impl Pane3d {
    /// New pane with a default orbit and no scene built yet.
    pub fn new() -> Self {
        Self {
            orbit: Orbit::default(),
            style: Style3d::default(),
            full: None,
            orbit_target: None,
            orbit_fov: None,
            loaded_camera: None,
            picked: None,
            last_revision: 0,
            mesh_count: 0,
            available: false,
            has_content: false,
            scene: None,
            cache: None,
            last_rebuild: None,
            doc: None,
            framed: false,
            last_rect: None,
            night: false,
            sun_date: "2026-06-21".into(),
            sun_time: "15:00".into(),
            site: None,
            lighting: cadcraft_render3d::Lighting::default(),
            lighting_inputs: None,
            sun_valid: true,
            finalize: Arc::default(),
            tool: PaintTool::Off,
            scope: 0,
            blend: false,
            paint_item: None,
            armed: None,
            paint_note: String::new(),
            click: None,
            escaped: false,
        }
    }

    /// The lighting for the site, night switch, date and time. Recomputed only when one of
    /// them changed; an entry that does not parse keeps the last good sun.
    pub fn lighting(&mut self) -> cadcraft_render3d::Lighting {
        let inputs = (self.site.clone(), self.night, self.sun_date.clone(), self.sun_time.clone());
        if self.lighting_inputs.as_ref() != Some(&inputs) {
            match compute_lighting(self.site.as_ref(), self.night, &self.sun_date, &self.sun_time) {
                Some(l) => {
                    self.lighting = l;
                    self.sun_valid = true;
                }
                None => self.sun_valid = false,
            }
            self.lighting_inputs = Some(inputs);
        }
        self.lighting
    }

    /// The rectangle the pane last drew into, in points (`None` before the first draw).
    pub fn last_rect(&self) -> Option<egui::Rect> {
        self.last_rect
    }

    /// Load the drawing's active saved camera when it differs from the one last loaded.
    /// Returns `true` when the pane's camera changed.
    pub fn load_active_camera(&mut self, model: &cadcraft_doc::ArchModel) -> bool {
        use cadcraft_doc::arch::CameraKind;
        use cadcraft_render3d::camera::FullCamera;
        let Some(cam) = model.active_camera.as_deref().and_then(|n| model.camera_by_name(n)) else {
            self.loaded_camera = None;
            return false;
        };
        if self.loaded_camera.as_ref() == Some(cam) {
            return false;
        }
        let fov = cam.fov_deg.filter(|f| f.is_finite() && *f > 0.0 && *f < 180.0);
        let target = cam.target.filter(|t| t.iter().all(|v| v.is_finite())).map(|t| cadcraft_geom::Vec3::new(t[0], t[1], t[2]));
        match cam.kind {
            CameraKind::Full => {
                let eye = cam.eye.filter(|e| e.iter().all(|v| v.is_finite())).unwrap_or([0.0, 0.0]);
                let eye = cadcraft_geom::Vec3::new(eye[0], eye[1], cam.eye_height.filter(|h| h.is_finite()).unwrap_or(1.6));
                let mut target = target.unwrap_or(cadcraft_geom::Vec3::new(0.0, 0.0, 0.0));
                if (target.x - eye.x).hypot(target.y - eye.y) < 1e-6 {
                    // Looking at its own plan position: face north instead.
                    target = cadcraft_geom::Vec3::new(eye.x, eye.y + 1.0, eye.z);
                }
                self.full = Some(FullCamera::new(eye, target, fov.unwrap_or(45.0)));
            }
            CameraKind::Orbit => {
                self.full = None;
                self.orbit_target = target;
                self.orbit_fov = fov;
                if let Some(a) = cam.azimuth_deg.filter(|v| v.is_finite()) {
                    self.orbit.azimuth = a.to_radians();
                }
                if let Some(e) = cam.elevation_deg.filter(|v| v.is_finite()) {
                    self.orbit.elevation = e.to_radians().clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                }
                if let Some(d) = cam.distance.filter(|v| v.is_finite() && *v > 0.0) {
                    self.orbit.distance = d;
                    // A saved distance is the user's framing; do not refit over it.
                    self.framed = true;
                }
            }
        }
        self.loaded_camera = Some(cam.clone());
        true
    }

    /// Back to the orbit camera from the walk-through camera.
    pub fn use_orbit(&mut self) {
        self.full = None;
    }

    /// The full camera, creating it from the current view when the pane is still orbiting.
    fn ensure_full(&mut self) -> cadcraft_render3d::camera::FullCamera {
        use cadcraft_render3d::camera::FullCamera;
        match self.full {
            Some(f) => f,
            None => {
                let cam = self.camera();
                let f = FullCamera::new(cam.eye(), cam.target(), cam.fov_deg());
                self.full = Some(f);
                f
            }
        }
    }

    /// Walk the full camera `forward`/`strafe` steps of `step` metres in plan, the maths of the
    /// `walkthrough` command: forward along `(sin yaw, cos yaw)`, strafe along `(cos yaw, -sin yaw)`,
    /// yaw 0 = +Y turning toward +X. Eye and target move together, so the heading holds.
    pub fn walk(&mut self, forward: f64, strafe: f64, step: f64) {
        let mut f = self.ensure_full();
        let yaw = (f.target.x - f.eye.x).atan2(f.target.y - f.eye.y);
        let (sy, cy) = yaw.sin_cos();
        let (fwd, right) = (forward * step, strafe * step);
        let (dx, dy) = (fwd * sy + right * cy, fwd * cy - right * sy);
        if !(dx.is_finite() && dy.is_finite()) {
            return;
        }
        f.eye.x += dx;
        f.eye.y += dy;
        f.target.x += dx;
        f.target.y += dy;
        self.full = Some(f);
    }

    /// Turn the full camera `deg` degrees about its eye (positive = toward +X).
    pub fn turn(&mut self, deg: f64) {
        let mut f = self.ensure_full();
        let (dx, dy) = (f.target.x - f.eye.x, f.target.y - f.eye.y);
        let len = dx.hypot(dy);
        if !deg.is_finite() || len < 1e-9 {
            return;
        }
        let yaw = dx.atan2(dy) + deg.to_radians();
        f.target.x = f.eye.x + len * yaw.sin();
        f.target.y = f.eye.y + len * yaw.cos();
        self.full = Some(f);
    }

    /// The proxy handle under `pixel` (y-down, device pixels of a `size` view), if the ray hits.
    pub fn pick_handle_at(&self, size: (u32, u32), pixel: [f32; 2]) -> Option<cadcraft_doc::Handle> {
        let scene = self.scene.as_ref()?;
        let (origin, dir) = cadcraft_render3d::camera::ray_at(&self.camera(), size, pixel)?;
        let hit = scene.pick(origin, dir)?;
        Some(hit.owner.handle(hit.object))
    }

    /// Click at `pixel`: remember the struck object's handle for the app to select. A miss
    /// leaves the earlier pick (and the selection) alone.
    pub fn pick(&mut self, size: (u32, u32), pixel: [f32; 2]) -> Option<cadcraft_doc::Handle> {
        self.click = self.click_at(size, pixel);
        let h = self.pick_handle_at(size, pixel)?;
        self.picked = Some(h);
        Some(h)
    }

    /// What the ray through `pixel` strikes (see [`Click`]).
    pub fn click_at(&self, size: (u32, u32), pixel: [f32; 2]) -> Option<Click> {
        let scene = self.scene.as_ref()?;
        let (origin, dir) = cadcraft_render3d::camera::ray_at(&self.camera(), size, pixel)?;
        let hit = scene.pick(origin, dir)?;
        let (material, normal, level) = locate(scene, &hit, dir);
        Some(Click { object: hit.object, part: hit.part.to_owned(), owner: hit.owner, material, normal, point: hit.point, level })
    }

    /// Take the handle of the last 3D pick, for the app to add to the selection the 2D pane uses.
    pub fn take_picked(&mut self) -> Option<cadcraft_doc::Handle> {
        self.picked.take()
    }

    /// Whether the scene must be rebuilt because `current_revision` differs from the last
    /// `arch_revision` we rendered against.
    pub fn needs_rebuild(&self, current_revision: u64) -> bool {
        current_revision != self.last_revision
    }

    /// Mark the scene as rebuilt. Call after `arch3d::build` / `render3d` completes
    /// with a successful result.
    pub fn mark_built(&mut self, revision: u64, mesh_count: usize, has_content: bool) {
        self.last_revision = revision;
        self.mesh_count = mesh_count;
        self.has_content = has_content;
    }

    /// Rebuild the scene for `revision` from `model` (`None` = no document).
    ///
    /// Returns `true` when the scene was rebuilt. Never rebuilds when
    /// `revision == self.last_revision` and a scene already exists.
    pub fn sync(&mut self, revision: u64, model: Option<&cadcraft_doc::ArchModel>) -> bool {
        match model {
            Some(m) => {
                if self.site != m.site {
                    self.site = m.site.clone();
                }
                if self.last_revision == revision {
                    return false;
                }
                self.load_active_camera(m);
                let stats = match self.cache.as_mut() {
                    Some(cache) => cache.update(m),
                    None => {
                        let (cache, stats) = cadcraft_arch3d::SceneCache::new(m);
                        self.cache = Some(cache);
                        stats
                    }
                };
                self.last_rebuild = Some(stats);
                let Some(cache) = self.cache.as_ref() else {
                    return false;
                };
                let scene = cache.scene().clone();
                let mesh_count = scene.meshes.len();
                let has_content = !scene.meshes.is_empty();
                if has_content && !self.framed {
                    // The first scene sets a distance that shows all of it; later zooms are the user's.
                    self.orbit.distance = cadcraft_render3d::camera::Orbit::fit(&scene.bounds).distance;
                    self.framed = true;
                }
                self.scene = Some(Arc::new(scene));
                self.mark_built(revision, mesh_count, has_content);
                true
            }
            None => {
                self.site = None;
                self.scene = None;
                self.cache = None;
                self.last_rebuild = None;
                self.mesh_count = 0;
                self.has_content = false;
                self.last_revision = revision;
                true
            }
        }
    }

    /// [`Pane3d::sync`] for the document `doc` (the session's `DocState::uid`): switching to
    /// another document rebuilds even when its revision number happens to match.
    pub fn sync_doc(&mut self, doc: u64, revision: u64, model: Option<&cadcraft_doc::ArchModel>) -> bool {
        if self.doc != Some(doc) {
            self.doc = Some(doc);
            self.cache = None;
            self.last_revision = 0;
            self.loaded_camera = None;
            self.full = None;
            self.orbit_target = None;
            self.orbit_fov = None;
        }
        self.sync(revision, model)
    }

    /// The scene currently held (`None` when nothing has been built).
    pub fn scene(&self) -> Option<&cadcraft_arch3d::Scene3d> {
        self.scene.as_deref()
    }

    /// Identifies the held scene's content for the renderer's mesh cache.
    fn scene_key(&self) -> u64 {
        self.doc.unwrap_or(0).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ self.last_revision
    }

    /// The orbit `Camera` for the currently held scene.
    ///
    /// Frames the held scene (or an empty bounds when there is none) via
    /// [`cadcraft_render3d::camera::Orbit::fit`], then applies the pane's user
    /// orbit: `azimuth`/`elevation` in degrees and `distance` clamped to at
    /// least [`cadcraft_render3d::camera::MIN_DISTANCE`]. `fit`'s `target` and
    /// `fov_deg` are preserved.
    pub fn camera(&self) -> cadcraft_render3d::Camera {
        let mut orbit = match self.scene {
            Some(ref s) => cadcraft_render3d::camera::Orbit::fit(&s.bounds),
            None => cadcraft_render3d::camera::Orbit::fit(&cadcraft_arch3d::Bounds3::empty()),
        };
        orbit.azimuth = self.orbit.azimuth.to_degrees();
        orbit.elevation = self.orbit.elevation.to_degrees();
        orbit.distance = self.orbit.distance.max(cadcraft_render3d::camera::MIN_DISTANCE);
        if let Some(f) = self.full {
            return cadcraft_render3d::Camera::Full(f);
        }
        if let Some(t) = self.orbit_target {
            orbit.target = t;
        }
        if let Some(f) = self.orbit_fov {
            orbit.fov_deg = f;
        }
        cadcraft_render3d::Camera::Orbit(orbit)
    }
}

/// Draw the 3D pane content into `ui`.
///
/// When no GPU adapter is available or no scene has been built, a text placeholder is
/// shown. When a scene exists it is rendered through the paint callback set up by [`install`];
/// dragging orbits and the wheel zooms.
pub fn draw(ui: &mut egui::Ui, pane: &mut Pane3d) {
    let scene = pane.scene.clone().filter(|_| pane.available && pane.has_content);
    let Some(scene) = scene else {
        egui::Frame::default().show(ui, |ui| {
            ui.add_space(6.0);
            if !pane.available {
                ui.label("3D not available");
                ui.label("No GPU adapter detected.");
            } else {
                ui.label("No walls yet.");
            }
        });
        return;
    };
    ui.horizontal(|ui| {
        for style in Style3d::ALL {
            ui.selectable_value(&mut pane.style, style, crate::i18n::t(style.label()));
        }
        if pane.full.is_some() && ui.button(crate::i18n::t("Orbit")).clicked() {
            pane.use_orbit();
        }
    });
    tool_row(ui, pane);
    let lighting = pane.lighting();
    let finalize = pane.finalize.clone();
    ui.horizontal(|ui| {
        ui.checkbox(&mut pane.night, crate::i18n::t("Night"));
        let red = (!pane.sun_valid).then(|| ui.visuals().error_fg_color);
        for text in [&mut pane.sun_date, &mut pane.sun_time] {
            let mut edit = egui::TextEdit::singleline(text).desired_width(76.0);
            if let Some(c) = red {
                edit = edit.text_color(c);
            }
            ui.add(edit);
        }
        let (progress, error, has_still, path_progress, fallback) = {
            let f = lock_final(&finalize);
            (f.progress, f.error.clone(), f.done.is_some(), f.path_progress, f.fallback)
        };
        match progress {
            Some(p) => {
                match path_progress {
                    Some((done, wanted)) => ui.label(samples_label(done, wanted)),
                    None => ui.add(egui::ProgressBar::new(p.clamp(0.0, 1.0)).desired_width(120.0).show_percentage()),
                };
                if ui.button(crate::i18n::t("Cancel")).clicked() {
                    lock_final(&finalize).cancel = true;
                }
                ui.ctx().request_repaint();
            }
            None => {
                {
                    use cadcraft_engine::FinalMode;
                    let mut f = lock_final(&finalize);
                    ui.selectable_value(&mut f.mode, FinalMode::Path, crate::i18n::t("Path traced"));
                    ui.selectable_value(&mut f.mode, FinalMode::Raster, crate::i18n::t("Raster"));
                    if f.mode == FinalMode::Path {
                        let label = ui.label(crate::i18n::t("Samples"));
                        ui.add(egui::DragValue::new(&mut f.samples).range(1..=cadcraft_render3d::pathtrace::MAX_SPP)).labelled_by(label.id);
                    }
                }
                if ui.button(crate::i18n::t("Finalize Render")).clicked() {
                    let mut f = lock_final(&finalize);
                    f.request = true;
                    f.error = None;
                }
                if let Some(e) = error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            }
        }
        if fallback {
            ui.label(crate::i18n::t("No compute shaders: raster Finalize"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if has_still && ui.button(crate::i18n::t("Save PNG")).clicked() {
            save_final_png(&finalize);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = has_still;
    });
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    pane.last_rect = Some(rect);
    let ppp = ui.ctx().pixels_per_point();
    let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
    if response.dragged() {
        let d = response.drag_delta();
        if pane.full.is_some() {
            pane.turn(f64::from(d.x) * 0.3);
        } else {
            orbit_by_drag(pane, -f64::from(d.x), f64::from(d.y), 0.01);
        }
    }
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        let px = [(p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp];
        pane.pick(size, px);
    }
    if (response.hovered() || response.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        pane.tool = PaintTool::Off;
        pane.escaped = true;
    }
    if response.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        if wheel != 0.0 {
            zoom_by_wheel(pane, f64::from(wheel) / 50.0);
        }
    }
    if response.has_focus() {
        walk_keys(ui, pane);
    }
    let callback = Pane3dCallback { scene, key: pane.scene_key(), camera: pane.camera(), size, style: pane.style, lighting, finalize };
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, callback));
}

/// Ask for a file and write the finished still there as PNG.
#[cfg(not(target_arch = "wasm32"))]
fn save_final_png(finalize: &std::sync::Mutex<FinalUi>) {
    let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file() else { return };
    let mut f = lock_final(finalize);
    let Some(d) = f.done.as_ref() else { return };
    let saved = image::save_buffer(&path, &d.rgba, d.width, d.height, image::ColorType::Rgba8);
    if let Err(e) = saved {
        f.error = Some(format!("{}: {e}", crate::i18n::t("Save PNG")));
    }
}

/// Apply this frame's WASD / arrow key presses to the full camera: W/S and Up/Down walk,
/// A/D strafe, Left/Right turn; Shift runs.
fn walk_keys(ui: &egui::Ui, pane: &mut Pane3d) {
    use egui::Key;
    let (mut fwd, mut strafe, mut turn, mut run) = (0.0, 0.0, 0.0, false);
    ui.input(|i| {
        run = i.modifiers.shift;
        let n = |k: Key| if i.key_pressed(k) { 1.0 } else { 0.0 };
        fwd = n(Key::W) + n(Key::ArrowUp) - n(Key::S) - n(Key::ArrowDown);
        strafe = n(Key::D) - n(Key::A);
        turn = n(Key::ArrowRight) - n(Key::ArrowLeft);
    });
    let step = if run { WALK_STEP * RUN_FACTOR } else { WALK_STEP };
    if fwd != 0.0 || strafe != 0.0 {
        pane.walk(fwd, strafe, step);
    }
    if turn != 0.0 {
        pane.turn(turn * TURN_STEP_DEG);
    }
}

/// Set the pane up to render on the window's device. Call once with the app's render state;
/// `stencil` is the format of the window pass's stencil buffer, if it has one.
pub fn install(pane: &mut Pane3d, rs: &egui_wgpu::RenderState, stencil: Option<wgpu::TextureFormat>) {
    let res = Resources::new(&rs.device, rs.target_format, stencil.filter(|s| s.has_stencil_aspect()));
    rs.renderer.write().callback_resources.insert(res);
    pane.available = true;
}

/// Samples the offscreen colour target into the pane's rectangle.
const BLIT: &str = r#"
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: V;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}
@fragment fn fs_linear(v: V) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(image, image_sampler, v.uv).rgb, 1.0);
}
fn encode(c: f32) -> f32 {
    if (c <= 0.0031308) { return 12.92 * c; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}
@fragment fn fs_encode(v: V) -> @location(0) vec4<f32> {
    let c = textureSample(image, image_sampler, v.uv).rgb;
    return vec4<f32>(encode(c.r), encode(c.g), encode(c.b), 1.0);
}
"#;

/// The pane's GPU state, kept in the renderer's callback resources.
struct Resources {
    blit: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    offscreen: Option<cadcraft_render3d::Offscreen>,
    /// The bind group for the offscreen target of this generation.
    bound: Option<(u64, wgpu::BindGroup)>,
    /// What the offscreen target last drew: scene key, size and view-projection.
    drawn: Option<ViewState>,
    /// How many times the scene was drawn into the offscreen target.
    draws: u64,
    /// The Finalize Render in progress.
    final_render: Option<cadcraft_render3d::FinalRender>,
    /// The finished still on the GPU, with the view it was made for; while a path Finalize
    /// refines, its preview so far.
    final_image: Option<(ViewState, wgpu::BindGroup)>,
    /// The compute device for path-traced Finalize: `None` until the first path Finalize asks
    /// for it, `Some(None)` when the adapter has no compute shaders.
    compute: Option<Option<cadcraft_render3d::pathtrace::ComputeGpu>>,
}

impl Resources {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat, stencil: Option<wgpu::TextureFormat>) -> Self {
        let module =
            device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pane3d blit"), source: wgpu::ShaderSource::Wgsl(BLIT.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pane3d blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pane3d blit"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // The window pass may carry a stencil buffer (clipped viewports); a pipeline drawn in it
        // must name the same format even though it never tests it.
        let depth_stencil = stencil.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: None,
            depth_compare: None,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });
        // The offscreen target is sRGB, so sampling returns linear colour: an sRGB window takes it
        // as is, any other window gets it encoded (as the canvas does with `linear_out`).
        let fs = if target.is_srgb() { "fs_linear" } else { "fs_encode" };
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pane3d blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState { format: target, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pane3d blit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Resources { blit, layout, sampler, offscreen: None, bound: None, drawn: None, draws: 0, final_render: None, final_image: None, compute: None }
    }
}

/// One frame's request: draw `scene` from `camera` at `size` device pixels.
struct Pane3dCallback {
    scene: Arc<cadcraft_render3d::Scene3d>,
    key: u64,
    camera: cadcraft_render3d::Camera,
    size: (u32, u32),
    style: Style3d,
    lighting: cadcraft_render3d::Lighting,
    finalize: Arc<std::sync::Mutex<FinalUi>>,
}

/// Upload a finished still as an sRGB texture and bind it for the blit.
fn upload_still(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::BindGroup {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pane3d final"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width.saturating_mul(4)), rows_per_image: Some(height) },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pane3d final"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

impl egui_wgpu::CallbackTrait for Pane3dCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(res) = resources.get_mut::<Resources>() else { return Vec::new() };
        let off = res.offscreen.get_or_insert_with(|| cadcraft_render3d::Offscreen::with_device(device.clone(), queue.clone(), self.size));
        off.resize(self.size);
        let (w, h) = off.size();
        let vp = cadcraft_render3d::camera::view_proj(&self.camera, f64::from(w) / f64::from(h));
        // The shadow maps and the light selection live in `off` and are remade only when the
        // scene or the lights change, so a camera move builds none.
        let eye = self.camera.eye();
        let frame = off.lit_frame(&self.scene, self.key, [eye.x, eye.y, eye.z], &self.lighting, false);
        // Redraw only when something visible changed; an idle pane costs one textured triangle.
        let state: ViewState = (self.key, (w, h), vp, self.style, self.lighting, frame.as_ref().map_or(0, |f| f.lights));
        if res.drawn != Some(state) {
            let opts = cadcraft_render3d::RenderOptions { lit: frame.as_ref(), style: self.style.render_style(), ..Default::default() };
            let sun = self.lighting.sun.unwrap_or(cadcraft_render3d::Light::DEFAULT);
            off.encode_with(encoder, &self.scene, self.key, &self.camera, &sun, &opts);
            res.drawn = Some(state);
            res.draws += 1;
        }
        drop(frame);

        // Finalize Render: a view change drops the still and cancels a running render.
        let mut ui_state = lock_final(&self.finalize);
        ui_state.observe(&state);
        if res.final_image.as_ref().is_some_and(|(s, _)| *s != state) {
            res.final_image = None;
        }
        if std::mem::take(&mut ui_state.request) && res.final_render.is_none() {
            res.final_image = None;
            ui_state.done = None;
            ui_state.path_progress = None;
            ui_state.fallback = false;
            let render = match ui_state.mode {
                cadcraft_engine::FinalMode::Raster => {
                    cadcraft_render3d::FinalRender::start(off, &self.scene, self.key, &self.camera, &self.lighting, false, (w, h))
                }
                cadcraft_engine::FinalMode::Path => {
                    let compute = res.compute.get_or_insert_with(request_pane_compute).as_ref();
                    let spp = ui_state.samples.clamp(1, cadcraft_render3d::pathtrace::MAX_SPP);
                    let opts = cadcraft_render3d::pathtrace::PathOptions { spp, ..Default::default() };
                    let render = cadcraft_render3d::FinalRender::start_path(
                        compute,
                        off,
                        &self.scene,
                        self.key,
                        &self.camera,
                        &self.lighting,
                        false,
                        (w, h),
                        &opts,
                        None,
                    );
                    if render.fallback().is_some() {
                        ui_state.fallback = true;
                    } else {
                        ui_state.path_progress = Some((0, spp));
                    }
                    render
                }
            };
            res.final_render = Some(render);
            ui_state.started = Some(state);
            ui_state.progress = Some(0.0);
        }
        if std::mem::take(&mut ui_state.cancel)
            && let Some(r) = res.final_render.as_mut()
        {
            r.cancel();
        }
        // One step per frame keeps the pane responsive; a path Finalize shows its image so far.
        let status = res.final_render.as_mut().map(|r| r.step(off));
        match status {
            Some(cadcraft_render3d::FinalStatus::Running { progress }) => {
                ui_state.progress = Some(progress);
                if let Some(r) = res.final_render.as_ref()
                    && let Some(stats) = r.path_stats()
                {
                    let wanted = ui_state.path_progress.map_or(stats.spp_done, |(_, wanted)| wanted);
                    ui_state.path_progress = Some((stats.spp_done, wanted));
                    if let Some((pw, ph, rgba)) = r.preview() {
                        res.final_image = Some((state, upload_still(device, queue, &res.layout, &res.sampler, pw, ph, &rgba)));
                    }
                }
            }
            Some(cadcraft_render3d::FinalStatus::Done { width, height, rgba }) => {
                let group = upload_still(device, queue, &res.layout, &res.sampler, width, height, &rgba);
                res.final_image = Some((state, group));
                ui_state.done = Some(FinalDone { state, width, height, rgba });
                ui_state.progress = None;
                ui_state.path_progress = None;
                res.final_render = None;
            }
            Some(cadcraft_render3d::FinalStatus::Cancelled) => {
                ui_state.progress = None;
                ui_state.path_progress = None;
                res.final_image = None;
                res.final_render = None;
            }
            Some(cadcraft_render3d::FinalStatus::Failed(msg)) => {
                ui_state.progress = None;
                ui_state.path_progress = None;
                ui_state.error = Some(msg);
                res.final_image = None;
                res.final_render = None;
            }
            None => {}
        }
        drop(ui_state);
        if res.bound.as_ref().is_none_or(|(g, _)| *g != off.generation()) {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pane3d blit"),
                layout: &res.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(off.color_view()) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&res.sampler) },
                ],
            });
            res.bound = Some((off.generation(), group));
        }
        Vec::new()
    }

    fn paint(&self, info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        let Some(res) = resources.get::<Resources>() else { return };
        let Some((_, live)) = &res.bound else { return };
        let group = res.final_image.as_ref().map_or(live, |(_, g)| g);
        let v = info.viewport_in_pixels();
        let [sw, sh] = info.screen_size_px;
        let (x0, y0) = (v.left_px.max(0) as f32, v.top_px.max(0) as f32);
        let (x1, y1) = (((v.left_px + v.width_px) as f32).min(sw as f32), ((v.top_px + v.height_px) as f32).min(sh as f32));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        pass.set_viewport(x0, y0, x1 - x0, y1 - y0, 0.0, 1.0);
        pass.set_pipeline(&res.blit);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Apply a 2D drag delta (UI pixels) to the orbit camera.
pub fn orbit_by_drag(pane: &mut Pane3d, dx: f64, dy: f64, sensitivity: f64) {
    pane.orbit.azimuth += dx * sensitivity;
    pane.orbit.elevation = (pane.orbit.elevation + dy * sensitivity).clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
}

/// Apply a wheel-zoom delta to the orbit camera.
/// Positive `delta` zooms in (decreases distance); negative zooms out.
pub fn zoom_by_wheel(pane: &mut Pane3d, delta: f64) {
    pane.orbit.distance = (pane.orbit.distance * (1.0 - 0.1 * delta)).clamp(0.5, 1000.0);
}

/// Apply the last 3D pick as the drawing's selection, so a click in the 3D pane selects the
/// same proxy handle as a click in the 2D pane (phase-6 carry-over). Takes the pending handle
/// and replaces the selection with it, returning it so the caller can repaint. A miss (or no
/// pending pick) leaves the selection alone.
///
/// With a painter armed the click runs the painter's command instead, and with a library item
/// armed (`library.place` waiting for its point) it places the item at the struck point on the
/// struck level; neither changes the selection. Esc over the pane disarms.
pub fn apply_pick(pane: &mut Pane3d, session: &mut cadcraft_engine::Session) -> Option<cadcraft_doc::Handle> {
    let placing = session.running.as_ref().is_some_and(|r| r.id == PLACE);
    if std::mem::take(&mut pane.escaped) {
        pane.armed = None;
        pane.click = None;
        if placing {
            let _ = session.input(cadcraft_engine::Input::Cancel);
        }
        return None;
    }
    if !placing {
        pane.armed = None;
    }
    if let Some(click) = pane.click.take() {
        let handle = click.owner.handle(click.object);
        if pane.tool != PaintTool::Off {
            pane.picked = None;
            paint_click(pane, session, &click);
            return Some(handle);
        }
        if placing && let Some(item) = pane.armed.clone() {
            pane.picked = None;
            place_click(pane, session, &click, &item);
            return Some(handle);
        }
    }
    let h = pane.take_picked()?;
    session.set_selection(vec![h]);
    Some(h)
}

/// The command a library item is placed with.
const PLACE: &str = "library.place";

/// The tool row above the pane: the three painters with their scope and mode.
fn tool_row(ui: &mut egui::Ui, pane: &mut Pane3d) {
    ui.horizontal(|ui| {
        for tool in PaintTool::ALL {
            let on = pane.tool == tool;
            if ui.selectable_label(on, crate::i18n::t(tool.label())).clicked() {
                pane.tool = if on { PaintTool::Off } else { tool };
            }
        }
        if pane.tool != PaintTool::Off {
            egui::ComboBox::from_id_salt("paint-scope").selected_text(crate::i18n::t(scope_label(pane.scope))).show_ui(ui, |ui| {
                for i in 0..SCOPES.len() {
                    ui.selectable_value(&mut pane.scope, i, crate::i18n::t(scope_label(i)));
                }
            });
        }
        if pane.tool == PaintTool::Material {
            ui.selectable_value(&mut pane.blend, false, crate::i18n::t("Paint"));
            ui.selectable_value(&mut pane.blend, true, crate::i18n::t("Blend"));
        }
        if let Some(item) = pane.paint_item.as_ref().filter(|_| pane.tool != PaintTool::Off) {
            ui.label(item.as_str());
        }
        if !pane.paint_note.is_empty() {
            ui.label(pane.paint_note.as_str());
        }
    });
}

/// Run the armed painter on the surface `click` struck: one command, so one undo step.
fn paint_click(pane: &mut Pane3d, session: &mut cadcraft_engine::Session, click: &Click) {
    use serde_json::Value;
    let surface = session.doc().ok().and_then(|d| surface_of(&d.arch, click.object, &click.part, &click.material, click.normal));
    let Some(surface) = surface else {
        pane.paint_note = "Nothing paintable there".into();
        return;
    };
    let target = surface.key();
    let scope = SCOPES.get(pane.scope).copied().unwrap_or("component");
    let item = pane.paint_item.clone();
    let result: Result<Value, String> = match (pane.tool, item) {
        (PaintTool::Eyedropper, _) => session.execute("paint.pick", &json!({ "target": target })).map_err(|e| e.to_string()),
        (PaintTool::Material, Some(item)) => {
            let mode = if pane.blend { "blend" } else { "paint" };
            session.execute("paint.material", &json!({ "material": item, "target": target, "scope": scope, "mode": mode })).map_err(|e| e.to_string())
        }
        (PaintTool::Object, Some(item)) => {
            session.execute("paint.object", &json!({ "item": item, "target": target, "scope": scope })).map_err(|e| e.to_string())
        }
        (PaintTool::Material | PaintTool::Object, None) => Err("Pick a library item first".into()),
        (PaintTool::Off, _) => return,
    };
    match result {
        Ok(v) if pane.tool == PaintTool::Eyedropper => match v.get("material").and_then(Value::as_str).filter(|m| !m.starts_with("default:")) {
            Some(m) => {
                pane.paint_item = Some(m.to_owned());
                pane.tool = PaintTool::Material;
                pane.paint_note.clear();
            }
            None => pane.paint_note = "That surface has the default material".into(),
        },
        Ok(_) => pane.paint_note.clear(),
        Err(e) => {
            pane.paint_note = e.clone();
            session.echo(e);
        }
    }
}

/// Place `item` at the point `click` struck, on the level it struck, ending the prompt that was
/// waiting for the point.
fn place_click(pane: &mut Pane3d, session: &mut cadcraft_engine::Session, click: &Click, item: &str) {
    let level = session.doc().ok().and_then(|d| d.arch.levels.iter().find(|l| (l.elevation - click.level).abs() < 1e-6).map(|l| l.index));
    let _ = session.input(cadcraft_engine::Input::Cancel);
    pane.armed = None;
    let mut params = json!({ "item": item, "at": [click.point.x, click.point.y] });
    if let Some(level) = level {
        params["level"] = json!(level);
    }
    if let Err(e) = session.execute(PLACE, &params) {
        pane.paint_note = e.to_string();
        session.echo(e.to_string());
    } else {
        pane.paint_note.clear();
    }
}

/// The paintable surface of the part `part` of object `object` that a click struck: `material`
/// is the struck mesh's material name and `normal` the face normal (facing the viewer). A wall's
/// side comes from the normal against the wall's direction (left of start to end, or right); the
/// wall's top and ends are no surface.
pub fn surface_of(
    model: &cadcraft_doc::ArchModel,
    object: cadcraft_doc::ArchId,
    part: &str,
    material: &str,
    normal: [f64; 3],
) -> Option<cadcraft_doc::arch::Surface> {
    use cadcraft_doc::arch::{CabinetPart, Side, Surface};
    // A painted countertop or backsplash has part `counter` / `backsplash`: resolve its own key
    // first, or the cabinet arm below would hand back another object's surface.
    if let Some(s) = material.strip_prefix("paint:").and_then(Surface::parse) {
        match s {
            Surface::Countertop(c) if model.countertop(c).is_some() => return Some(s),
            Surface::Backsplash(b) if model.backsplash(b).is_some() => return Some(s),
            _ => {}
        }
    }
    if material == "countertop" || material.starts_with("countertop-") {
        return model.countertop(object).map(|_| Surface::Countertop(object));
    }
    if material == "backsplash" {
        return model.backsplash(object).map(|_| Surface::Backsplash(object));
    }
    if material == "glass-wall" || material.starts_with("molding-") {
        return None;
    }
    match part {
        "wall" => {
            let w = model.wall(object)?;
            let dir = (w.end - w.start).normalized();
            let along = normal[0] * dir.x + normal[1] * dir.y;
            let across = -normal[0] * dir.y + normal[1] * dir.x;
            if !(across.abs() > along.abs() && across.abs() > normal[2].abs()) {
                return None;
            }
            Some(Surface::WallSide { wall: object, side: if across > 0.0 { Side::Left } else { Side::Right } })
        }
        "floor" => model.room(object).map(|_| Surface::RoomFloor(object)),
        "ceiling" => model.room(object).map(|_| Surface::RoomCeiling(object)),
        "roof" => model.roof(object).map(|_| Surface::Roof(object)),
        "cabinet" | "counter" | "pull" => {
            model.cabinets.iter().find(|c| c.id == object)?;
            let part = if material.starts_with("pull:") {
                CabinetPart::Pull
            } else if material == "cabinet-front" {
                CabinetPart::Front
            } else if material == "counter" || material.starts_with("counter-") {
                CabinetPart::Counter
            } else {
                CabinetPart::Carcass
            };
            Some(Surface::Cabinet { cabinet: object, part })
        }
        "fixture" => model.fixtures.iter().find(|f| f.id == object).map(|_| Surface::Fixture(object)),
        "furnishing" => {
            model.furnishing(object)?;
            let index = material.rsplit_once(':')?.1.parse::<u32>().ok()?;
            Some(Surface::Furnishing { furnishing: object, part: index })
        }
        _ => None,
    }
}

/// The material, face normal (facing the viewer, whose ray runs along `dir`) and level elevation
/// of the triangle of `hit`'s object and part that contains the hit point.
fn locate(scene: &cadcraft_render3d::Scene3d, hit: &cadcraft_arch3d::pick::PickHit, dir: cadcraft_geom::Vec3) -> (String, [f64; 3], f64) {
    let p = [hit.point.x, hit.point.y, hit.point.z];
    let mut best: Option<(f64, &cadcraft_arch3d::mesh::Mesh, [f64; 3])> = None;
    for mesh in scene.meshes.iter().filter(|m| {
        m.object == hit.object && cadcraft_arch3d::probe::pick_part(m) == hit.part && cadcraft_arch3d::incremental::object_kind_of(m) == hit.owner
    }) {
        for t in mesh.indices.chunks_exact(3) {
            let v = |i: u32| mesh.positions.get(i as usize).map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]);
            let (Some(a), Some(b), Some(c)) = (v(t[0]), v(t[1]), v(t[2])) else { continue };
            let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
            let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
            let (e1, e2, ap) = (sub(b, a), sub(c, a), sub(p, a));
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let len = dot(n, n).sqrt();
            if !(len > 1e-12) {
                continue;
            }
            let n = [n[0] / len, n[1] / len, n[2] / len];
            let off = dot(n, ap).abs();
            // Barycentric coordinates of the point in the triangle.
            let (d11, d12, d22, d1p, d2p) = (dot(e1, e1), dot(e1, e2), dot(e2, e2), dot(e1, ap), dot(e2, ap));
            let den = d11 * d22 - d12 * d12;
            if !(den.abs() > 1e-18) {
                continue;
            }
            let u = (d22 * d1p - d12 * d2p) / den;
            let w = (d11 * d2p - d12 * d1p) / den;
            let eps = 1e-4;
            if off > 1e-3 || u < -eps || w < -eps || u + w > 1.0 + eps {
                continue;
            }
            if best.is_none_or(|(d, _, _)| off < d) {
                best = Some((off, mesh, n));
            }
        }
    }
    let Some((_, mesh, mut n)) = best else { return (String::new(), [0.0, 0.0, 1.0], f64::NAN) };
    if n[0] * dir.x + n[1] * dir.y + n[2] * dir.z > 0.0 {
        n = [-n[0], -n[1], -n[2]];
    }
    (mesh.material.clone(), n, mesh.level_elevation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_handle_follows_the_owner() {
        use cadcraft_arch3d::incremental::ObjectKind as K;
        use cadcraft_doc::arch as a;
        let id = cadcraft_doc::ArchId(3);
        assert_eq!(K::Stair.handle(id), a::stair_handle(id));
        assert_eq!(K::Railing.handle(id), a::railing_handle(id));
        assert_eq!(K::Opening.handle(id), a::opening_handle(id));
        assert_eq!(K::Room.handle(id), a::room_handle(id));
        assert_eq!(K::Wall.handle(id), a::arch_handle(id));
        assert_eq!(K::Countertop.handle(id), a::countertop_handle(id));
        assert_eq!(K::Backsplash.handle(id), a::backsplash_handle(id));
        assert_ne!(K::Countertop.handle(id), K::Cabinet.handle(id));
    }

    #[test]
    fn surface_of_countertop_and_backsplash() {
        use cadcraft_doc::arch::Surface;
        let mut m = cadcraft_doc::ArchModel::default();
        let ct = m.push_countertop(Default::default());
        let bs = m.push_backsplash(Default::default());
        let n = [0.0, 0.0, 1.0];
        assert_eq!(surface_of(&m, ct, "counter", "countertop", n), Some(Surface::Countertop(ct)));
        assert_eq!(surface_of(&m, ct, "counter", "countertop-waterfall", n), Some(Surface::Countertop(ct)));
        assert_eq!(surface_of(&m, bs, "backsplash", "backsplash", n), Some(Surface::Backsplash(bs)));
        let key = Surface::Countertop(ct).object_key();
        assert_eq!(surface_of(&m, ct, "counter", &format!("paint:{key}"), n), Some(Surface::Countertop(ct)));
        let key = Surface::Backsplash(bs).object_key();
        assert_eq!(surface_of(&m, bs, "backsplash", &format!("paint:{key}"), n), Some(Surface::Backsplash(bs)));
        // No such object: no surface, and never a cabinet's.
        assert_eq!(surface_of(&m, cadcraft_doc::ArchId(9), "counter", "countertop", n), None);
        assert_eq!(surface_of(&m, cadcraft_doc::ArchId(9), "backsplash", "backsplash", n), None);
        assert_eq!(surface_of(&m, ct, "glass", "glass-wall", n), None);
        assert_eq!(surface_of(&m, ct, "molding", "molding-room-base-0", n), None);
    }

    #[test]
    fn locate_filters_by_owner() {
        use cadcraft_arch3d::incremental::ObjectKind;
        use cadcraft_arch3d::mesh::Mesh;
        use cadcraft_geom::{Vec2, Vec3};
        let ring = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        // Same object number and pick part (`counter`), different owners; the cabinet's is lower.
        let mut cabinet = Mesh::new("counter", cadcraft_doc::ArchId(1));
        cabinet.push_prism(&ring, 0.0, 0.2);
        let mut top = Mesh::new("countertop", cadcraft_doc::ArchId(1));
        top.push_prism(&ring, 0.5, 0.7);
        let scene = cadcraft_render3d::Scene3d { meshes: vec![cabinet, top], ..Default::default() };
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let hit = scene.pick(Vec3::new(0.5, 0.5, 2.0), dir).expect("hit");
        assert_eq!(hit.owner, ObjectKind::Countertop);
        let (material, _, _) = locate(&scene, &hit, dir);
        assert_eq!(material, "countertop");
        let mut other = hit;
        other.owner = ObjectKind::Cabinet;
        let (material, _, _) = locate(&scene, &other, dir);
        assert_ne!(material, "countertop");
    }

    #[test]
    fn locate_matches_by_pick_part() {
        use cadcraft_arch3d::mesh::Mesh;
        use cadcraft_geom::{Vec2, Vec3};
        let mut mesh = Mesh::new("deck-joist", cadcraft_doc::ArchId(1));
        let ring = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        mesh.push_prism(&ring, 0.0, 0.2);
        let scene = cadcraft_render3d::Scene3d { meshes: vec![mesh], ..Default::default() };
        let (origin, dir) = (Vec3::new(0.5, 0.5, 2.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = scene.pick(origin, dir).expect("hit");
        assert_eq!(hit.part, "deck");
        let (material, n, _) = locate(&scene, &hit, dir);
        assert_eq!(material, "deck-joist");
        assert!(n[2] > 0.9, "{n:?}");
    }

    use cadcraft_engine::Session;
    use serde_json::json;

    /// The phase-1 house (four exterior walls) with the phase-2 front door on `w0` at offset
    /// 3.0 — the same two commands as `tests/acceptance/phase-7/p7lib.inc::HOUSE`.
    fn house_session() -> Session {
        let mut session = Session::new();
        let _walls = session
            .execute(
                "wall",
                &json!({
                    "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]],
                    "thickness": 0.2,
                    "height": 2.4,
                    "exterior": true
                }),
            )
            .expect("the phase-1 house walls");
        let _door = session
            .execute(
                "door",
                &json!({
                    "wall": "w0",
                    "offset": 3.0,
                    "width": 0.9,
                    "height": 2.1,
                    "style": "hinged"
                }),
            )
            .expect("the front door on w0");
        session
    }

    /// The selection's handles as hex strings, for assertions independent of `Handle`'s derives.
    fn sel_handles(s: &Session) -> Vec<String> {
        s.selection().iter().map(|h| h.hex()).collect()
    }

    /// The phase-6 front camera: eye [3,-8,1.6] -> target [3,0,1.05], fov 60, view 480x320.
    /// The 0.9 m door at offset 3.0 on the south wall is dead-centre; [8,8] is sky.
    fn front_camera() -> (cadcraft_render3d::camera::FullCamera, (u32, u32)) {
        let size = (480u32, 320u32);
        (cadcraft_render3d::camera::FullCamera::new(cadcraft_geom::Vec3::new(3.0, -8.0, 1.6), cadcraft_geom::Vec3::new(3.0, 0.0, 1.05), 60.0), size)
    }

    /// A `Pane3d` with the phase-1 house scene synced and the front camera installed.
    fn house_pane() -> (Pane3d, Session) {
        let mut session = house_session();
        let mut pane = Pane3d::new();
        {
            let st = session.state().expect("session state");
            assert!(pane.sync_doc(st.uid, st.doc.arch_revision, Some(&st.doc.arch)), "the phase-1 house scene must build");
        }
        let (cam, _) = front_camera();
        pane.full = Some(cam);
        (pane, session)
    }

    fn wall_click(normal: [f64; 3]) -> Click {
        Click {
            object: cadcraft_doc::ArchId(0),
            part: "wall".into(),
            owner: cadcraft_arch3d::incremental::ObjectKind::Wall,
            material: "wall".into(),
            normal,
            point: cadcraft_geom::Vec3::new(5.0, 0.0, 1.0),
            level: 0.0,
        }
    }

    #[test]
    fn a_picked_wall_face_maps_to_its_side() {
        use cadcraft_doc::arch::{Side, Surface};
        let session = house_session();
        let st = session.state().expect("session state");
        let model = &st.doc.arch;
        let w = ArchIdWall::first(model);
        // w0 runs from (0, 0) to (10, 0): its left is +y.
        let side = |n: [f64; 3]| surface_of(model, w, "wall", "wall", n);
        assert_eq!(side([0.0, 1.0, 0.0]), Some(Surface::WallSide { wall: w, side: Side::Left }));
        assert_eq!(side([0.0, -1.0, 0.0]), Some(Surface::WallSide { wall: w, side: Side::Right }));
        assert_eq!(side([0.0, 1.0, 0.0]).map(|s| s.key()), Some("wall/w0/left".to_owned()));
        // The top and the ends belong to no side.
        assert_eq!(side([0.0, 0.0, 1.0]), None);
        assert_eq!(side([1.0, 0.0, 0.0]), None);
        // No such wall, no such surface.
        assert_eq!(surface_of(model, cadcraft_doc::ArchId(99), "wall", "wall", [0.0, 1.0, 0.0]), None);
        assert_eq!(surface_of(model, w, "terrain", "terrain", [0.0, 1.0, 0.0]), None);
    }

    /// The id of the model's first wall.
    struct ArchIdWall;
    impl ArchIdWall {
        fn first(model: &cadcraft_doc::ArchModel) -> cadcraft_doc::ArchId {
            model.walls.first().map(|w| w.id).expect("a wall")
        }
    }

    #[test]
    fn painter_click_runs_one_command() {
        let mut session = house_session();
        let mut pane = Pane3d::new();
        let undo = |s: &Session| s.state().map(|st| st.undo.len()).unwrap_or_default();
        // Without an armed item the painter refuses and changes nothing.
        pane.tool = PaintTool::Material;
        pane.click = Some(wall_click([0.0, 1.0, 0.0]));
        let before = undo(&session);
        apply_pick(&mut pane, &mut session);
        assert_eq!(undo(&session), before);
        assert!(!pane.paint_note.is_empty());
        // Armed, one click is one command and one undo step.
        pane.paint_item = Some("siding".into());
        pane.click = Some(wall_click([0.0, 1.0, 0.0]));
        apply_pick(&mut pane, &mut session);
        assert_eq!(undo(&session), before + 1, "one undo step: {}", pane.paint_note);
        assert!(session.doc().expect("doc").arch.paints.contains_key("wall/w0/left"));
        // The eyedropper reads it back and arms the Material Painter with it.
        pane.tool = PaintTool::Eyedropper;
        pane.paint_item = None;
        pane.click = Some(wall_click([0.0, 1.0, 0.0]));
        apply_pick(&mut pane, &mut session);
        assert_eq!(pane.tool, PaintTool::Material, "{}", pane.paint_note);
        assert_eq!(pane.paint_item.as_deref(), Some("finish:siding"));
        // A click that struck no surface does nothing.
        pane.click = Some(wall_click([0.0, 0.0, 1.0]));
        let n = undo(&session);
        apply_pick(&mut pane, &mut session);
        assert_eq!(undo(&session), n);
    }

    #[test]
    fn pane3d_builds_without_an_adapter() {
        let mut pane = Pane3d::new();
        assert!(!pane.available);
        assert!(!pane.has_content);
        assert_eq!(pane.last_revision, 0);
        assert_eq!(pane.mesh_count, 0);

        let ctx = egui::Context::default();
        let raw = egui::RawInput::default();
        let mut out = ctx.run_ui(raw, |ui| draw(ui, &mut pane));
        out.textures_delta.clear();

        assert!(!pane.available);
        assert!(!pane.has_content);
    }

    #[test]
    fn pane3d_rebuilds_only_when_the_revision_changes() {
        let mut pane = Pane3d::new();

        assert!(!pane.needs_rebuild(0));

        pane.mark_built(1, 5, true);
        assert_eq!(pane.last_revision, 1);
        assert_eq!(pane.mesh_count, 5);
        assert!(pane.has_content);
        assert!(!pane.needs_rebuild(1));
        assert!(!pane.needs_rebuild(1));

        assert!(pane.needs_rebuild(2));

        pane.mark_built(2, 7, true);
        assert_eq!(pane.last_revision, 2);
        assert_eq!(pane.mesh_count, 7);
        assert!(!pane.needs_rebuild(2));
        assert!(!pane.needs_rebuild(2));

        assert!(pane.needs_rebuild(3));
    }

    #[test]
    fn pane3d_renders_after_a_revision_bump() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(0.0, 0.0),
            end: cadcraft_geom::Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });

        let mut pane = Pane3d::new();
        assert!(pane.scene().is_none());
        assert!(!pane.has_content);

        // First successful build at revision 1.
        assert!(pane.sync(1, Some(&arch)));
        assert!(pane.scene().is_some());
        assert!(pane.has_content);
        assert_eq!(pane.last_revision, 1);

        // "No document" clears the scene.
        assert!(pane.sync(2, None));
        assert!(pane.scene().is_none());
        assert!(!pane.has_content);
        assert_eq!(pane.last_revision, 2);

        // A revision bump rebuilds.
        assert!(pane.sync(3, Some(&arch)));
        assert!(pane.scene().is_some());
        assert!(pane.has_content);
        assert_eq!(pane.last_revision, 3);
    }

    #[test]
    fn pane3d_sync_skips_an_unchanged_revision() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(0.0, 0.0),
            end: cadcraft_geom::Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });

        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        let first_mesh_count = pane.mesh_count;

        // Same revision, same model: no rebuild.
        assert!(!pane.sync(1, Some(&arch)));
        assert!(pane.scene().is_some());
        assert_eq!(pane.mesh_count, first_mesh_count);
    }

    #[test]
    fn pane3d_camera_frames_the_scene_bounds() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(0.0, 0.0),
            end: cadcraft_geom::Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });

        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        assert!(pane.scene().is_some());

        let cam = pane.camera();
        let o = match cam {
            cadcraft_render3d::Camera::Orbit(o) => o,
            _ => cadcraft_render3d::camera::Orbit::at_origin(0.0, 0.0, 1.0, 45.0),
        };

        // `fit` centres the camera on the scene; for a 0..10 m wall that is x = 5.
        assert!((o.target.x - 5.0).abs() < 0.25);
        assert!(o.distance >= cadcraft_render3d::camera::MIN_DISTANCE);
        // The first scene with content sets the distance that frames it.
        let fit = cadcraft_render3d::camera::Orbit::fit(&pane.scene().expect("scene").bounds).distance;
        assert!((o.distance - fit).abs() < 1e-9);
        // The render3d Orbit stores angles in degrees; the pane default is 0.0 / 0.35 rad.
        assert!((o.azimuth - 0.0).abs() < 1e-9);
        assert!((o.elevation - 0.35 * (180.0 / std::f64::consts::PI)).abs() < 1e-6);
    }

    fn house() -> cadcraft_doc::ArchModel {
        let mut arch = cadcraft_doc::ArchModel::default();
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0), (0.0, 0.0)];
        for w in pts.windows(2) {
            arch.push_wall(cadcraft_doc::Wall {
                start: cadcraft_geom::Vec2::new(w[0].0, w[0].1),
                end: cadcraft_geom::Vec2::new(w[1].0, w[1].1),
                thickness: 0.2,
                height: 2.7,
                ..Default::default()
            });
        }
        arch
    }

    #[test]
    fn switching_documents_rebuilds_even_at_the_same_revision() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 5, Some(&arch)));
        assert!(!pane.sync_doc(1, 5, Some(&arch)));
        assert!(pane.sync_doc(2, 5, Some(&arch)));
        assert!(pane.sync_doc(1, 5, Some(&arch)));
    }

    #[test]
    fn only_the_first_scene_sets_the_orbit_distance() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        let fitted = pane.orbit.distance;
        zoom_by_wheel(&mut pane, 2.0);
        let zoomed = pane.orbit.distance;
        assert!(zoomed < fitted);
        assert!(pane.sync(2, Some(&arch)));
        assert!((pane.orbit.distance - zoomed).abs() < 1e-12, "a rebuild undid the user's zoom");
    }

    /// A device for GPU tests, or `None` (the test skips) without an adapter or when the driver
    /// does not answer within 30 s.
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        cadcraft_render3d::offscreen::device_within(std::time::Duration::from_secs(30))
    }

    fn read_target(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, (w, h): (u32, u32)) -> Vec<u8> {
        let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(padded * h),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let index = queue.submit([enc.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(std::time::Duration::from_secs(20)) }).expect("readback");
        let mapped = buffer.slice(..).get_mapped_range().expect("mapped range");
        (0..h as usize).flat_map(|r| mapped[r * padded as usize..r * padded as usize + w as usize * 4].to_vec()).collect()
    }

    #[test]
    fn pane3d_house_scene_has_multiple_mesh_types() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        let scene = pane.scene().expect("scene");
        assert!(!scene.meshes.is_empty(), "house scene should have meshes");

        let mut materials = std::collections::HashSet::new();
        for mesh in &scene.meshes {
            materials.insert(mesh.material.clone());
        }
        assert!(materials.contains("wall"), "house scene should have wall meshes");
    }

    #[test]
    fn pane3d_house_scene_bounds_include_full_height() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        let scene = pane.scene().expect("scene");
        assert!(scene.bounds.max[2] >= 2.0, "scene bounds should include wall height");
        assert!(scene.bounds.min[2] <= 0.1, "scene bounds should include ground level");
    }

    #[test]
    fn pane3d_camera_frames_house_scene() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));

        let cam = pane.camera();
        let o = match cam {
            cadcraft_render3d::Camera::Orbit(o) => o,
            _ => cadcraft_render3d::camera::Orbit::at_origin(0.0, 0.0, 1.0, 45.0),
        };

        assert!(o.distance >= cadcraft_render3d::camera::MIN_DISTANCE);
        // House is 10x8m; camera should target near its center (5, 4)
        assert!((o.target.x - 5.0).abs() < 1.0, "camera should target near house center x");
        assert!((o.target.y - 4.0).abs() < 1.0, "camera should target near house center y");
    }

    #[test]
    fn pane3d_orbit_and_zoom_with_house_scene() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));

        let initial_azimuth = pane.orbit.azimuth;
        let initial_distance = pane.orbit.distance;

        orbit_by_drag(&mut pane, 100.0, 0.0, 0.01);
        assert_ne!(pane.orbit.azimuth, initial_azimuth, "orbit should change azimuth");

        zoom_by_wheel(&mut pane, 1.0);
        assert_ne!(pane.orbit.distance, initial_distance, "zoom should change distance");
        assert!(pane.orbit.distance < initial_distance, "positive zoom delta should zoom in");
    }

    #[test]
    fn pane_sync_rebuilds_incrementally_after_an_edit() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        let first = pane.last_rebuild.expect("first build");
        assert!(!first.incremental, "the first build seeds the cache");
        arch.walls[0].height = 3.0;
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let second = pane.last_rebuild.expect("second build");
        assert!(second.incremental, "{second:?}");
        assert!(second.rebuilt > 0 && second.rebuilt < second.objects, "{second:?}");
        assert_eq!(pane.scene().expect("scene").meshes.len(), cadcraft_arch3d::build(&arch).meshes.len());
    }

    #[test]
    fn pane_sync_reseeds_for_another_document() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        arch.walls[0].height = 3.0;
        assert!(pane.sync_doc(2, 1, Some(&arch)));
        let stats = pane.last_rebuild.expect("build");
        assert!(!stats.incremental, "another document is a full build: {stats:?}");
        assert_eq!(stats.rebuilt, stats.objects);
    }

    #[test]
    fn pane_sync_incremental_after_adding_wall() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        let first = pane.last_rebuild.expect("first build");
        assert!(!first.incremental);
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(5.0, 4.0),
            end: cadcraft_geom::Vec2::new(5.0, 8.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let second = pane.last_rebuild.expect("second build");
        assert!(second.incremental, "adding a wall should be incremental: {second:?}");
        assert!(second.rebuilt > 0 && second.rebuilt < second.objects, "only some objects rebuilt: {second:?}");
    }

    #[test]
    fn pane_sync_incremental_after_removing_wall() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        let first = pane.last_rebuild.expect("first build");
        assert!(!first.incremental);
        let initial_objects = first.objects;
        arch.walls.pop();
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let second = pane.last_rebuild.expect("second build");
        assert!(second.incremental, "removing a wall should be incremental: {second:?}");
        assert!(second.rebuilt > 0, "some objects rebuilt: {second:?}");
        assert_eq!(second.objects, initial_objects - 1, "one fewer object");
    }

    #[test]
    fn pane_sync_multiple_incremental_rebuilds() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        assert!(!pane.last_rebuild.expect("first").incremental);
        for i in 0..3 {
            arch.walls[0].height = 2.8 + (i as f64) * 0.1;
            assert!(pane.sync_doc(1, 2 + i as u64, Some(&arch)));
            let stats = pane.last_rebuild.expect("rebuild");
            assert!(stats.incremental, "edit {i} should be incremental: {stats:?}");
            assert!(stats.rebuilt > 0 && stats.rebuilt < stats.objects, "edit {i}: {stats:?}");
        }
    }

    #[test]
    fn pane_sync_same_doc_different_revision_is_incremental() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        assert!(!pane.last_rebuild.expect("first").incremental);
        arch.walls[1].thickness = 0.3;
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let stats = pane.last_rebuild.expect("rebuild");
        assert!(stats.incremental, "same doc, new revision should be incremental: {stats:?}");
    }

    #[test]
    fn pane_sync_incremental_scene_matches_full_build() {
        let mut arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        arch.walls[0].height = 3.5;
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let incremental_meshes = pane.scene().expect("scene").meshes.len();
        let full_scene = cadcraft_arch3d::build(&arch);
        assert_eq!(incremental_meshes, full_scene.meshes.len(), "incremental mesh count matches full build");
    }

    #[test]
    fn pane_sync_revision_backwards_rebuilds() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 5, Some(&arch)));
        assert_eq!(pane.last_revision, 5);
        assert!(pane.sync_doc(1, 3, Some(&arch)));
        assert_eq!(pane.last_revision, 3);
        assert!(!pane.sync_doc(1, 3, Some(&arch)));
    }

    #[test]
    fn pane_sync_huge_revision() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, u64::MAX, Some(&arch)));
        assert_eq!(pane.last_revision, u64::MAX);
        assert!(pane.scene().is_some());
    }

    #[test]
    fn pane_sync_empty_then_add_wall_is_incremental() {
        let mut arch = cadcraft_doc::ArchModel::default();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        assert!(!pane.last_rebuild.expect("first").incremental);
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(0.0, 0.0),
            end: cadcraft_geom::Vec2::new(5.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });
        assert!(pane.sync_doc(1, 2, Some(&arch)));
        let stats = pane.last_rebuild.expect("rebuild");
        assert!(stats.incremental, "adding to empty model should be incremental: {stats:?}");
        assert!(pane.has_content);
    }

    #[test]
    fn pane_sync_nan_site_does_not_panic() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        let mut model = arch.clone();
        model.site = Some(cadcraft_doc::arch::SiteLocation { latitude: f64::NAN, ..Default::default() });
        pane.sync_doc(1, 2, Some(&model));
        assert_eq!(pane.last_revision, 2);
    }

    #[test]
    fn pane_sync_doc_none_clears_cache() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        assert!(pane.cache.is_some(), "cache exists after build");
        assert!(pane.sync_doc(1, 2, None));
        assert!(pane.cache.is_none(), "cache cleared with None model");
        assert!(pane.scene().is_none());
        assert!(pane.sync_doc(1, 3, Some(&arch)));
        assert!(pane.cache.is_some(), "cache re-seeded");
        assert!(!pane.last_rebuild.expect("rebuild").incremental, "re-seed is a full build");
    }

    #[test]
    fn pane3d_empty_model_has_no_content() {
        let arch = cadcraft_doc::ArchModel::default();
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        assert!(!pane.has_content, "empty model should have no content");
        assert!(pane.scene().expect("scene").meshes.is_empty(), "empty model should have no meshes");
    }

    #[test]
    fn pane3d_single_wall_scene_has_content() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(0.0, 0.0),
            end: cadcraft_geom::Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        assert!(pane.has_content, "single wall should have content");
        assert!(!pane.scene().expect("scene").meshes.is_empty(), "single wall should have meshes");
    }

    #[test]
    fn pane3d_camera_with_no_scene_uses_empty_bounds() {
        let mut pane = Pane3d::new();
        // No scene built yet; camera should still be valid
        let cam = pane.camera();
        let o = match cam {
            cadcraft_render3d::Camera::Orbit(o) => o,
            _ => cadcraft_render3d::camera::Orbit::at_origin(0.0, 0.0, 1.0, 45.0),
        };
        assert!(o.distance >= cadcraft_render3d::camera::MIN_DISTANCE);
        assert!(o.target.is_finite(), "camera target must be finite with no scene");
    }

    #[test]
    fn pane3d_orbit_elevation_clamps_to_vertical() {
        let mut pane = Pane3d::new();
        // Drag up a lot (positive dy increases elevation)
        orbit_by_drag(&mut pane, 0.0, 1000.0, 0.01);
        assert!(pane.orbit.elevation <= std::f64::consts::FRAC_PI_2 + 1e-9, "elevation should clamp at +90 degrees");
        // Drag down a lot
        orbit_by_drag(&mut pane, 0.0, -1000.0, 0.01);
        assert!(pane.orbit.elevation >= -std::f64::consts::FRAC_PI_2 - 1e-9, "elevation should clamp at -90 degrees");
    }

    #[test]
    fn pane3d_zoom_clamps_distance() {
        let mut pane = Pane3d::new();
        // Zoom in many times
        for _ in 0..100 {
            zoom_by_wheel(&mut pane, 1.0);
        }
        assert!(pane.orbit.distance >= 0.5 - 1e-9, "distance should clamp at 0.5");
        // Zoom out many times
        for _ in 0..100 {
            zoom_by_wheel(&mut pane, -1.0);
        }
        assert!(pane.orbit.distance <= 1000.0 + 1e-9, "distance should clamp at 1000");
    }

    #[test]
    fn pane3d_zero_length_wall_builds_scene() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(5.0, 5.0),
            end: cadcraft_geom::Vec2::new(5.0, 5.0),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });
        let mut pane = Pane3d::new();
        // A zero-length wall should build without panicking
        assert!(pane.sync(1, Some(&arch)));
        assert_eq!(pane.last_revision, 1);
    }

    #[test]
    fn pane3d_wall_with_nan_coordinates_does_not_panic() {
        let mut arch = cadcraft_doc::ArchModel::default();
        arch.push_wall(cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(f64::NAN, 0.0),
            end: cadcraft_geom::Vec2::new(10.0, f64::NAN),
            thickness: 0.2,
            height: 2.7,
            ..Default::default()
        });
        let mut pane = Pane3d::new();
        // Should not panic on NaN coordinates
        pane.sync(1, Some(&arch));
        assert_eq!(pane.last_revision, 1);
    }

    #[test]
    fn pane3d_sync_doc_clears_with_none_model() {
        let arch = house();
        let mut pane = Pane3d::new();
        assert!(pane.sync_doc(1, 1, Some(&arch)));
        assert!(pane.has_content);
        // Clear with None model
        assert!(pane.sync_doc(1, 2, None));
        assert!(!pane.has_content, "scene should be cleared");
        assert!(pane.scene().is_none(), "scene should be None");
        assert_eq!(pane.last_revision, 2);
    }

    /// One frame's callback for `pane` at `size` device pixels, in the default light.
    fn callback_for(pane: &Pane3d, size: (u32, u32)) -> Pane3dCallback {
        Pane3dCallback {
            scene: pane.scene.clone().expect("scene"),
            key: pane.scene_key(),
            camera: pane.camera(),
            size,
            style: pane.style,
            lighting: cadcraft_render3d::Lighting::default(),
            finalize: pane.finalize.clone(),
        }
    }

    /// A window target and the pane's callback resources on one device: runs the paint
    /// callback's frames as egui_wgpu would (prepare, then paint into the window pass).
    struct Rig {
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: (u32, u32),
        target: wgpu::Texture,
        view: wgpu::TextureView,
        resources: egui_wgpu::CallbackResources,
    }

    impl Rig {
        fn new(device: wgpu::Device, queue: wgpu::Queue, size: (u32, u32)) -> Self {
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let mut resources = egui_wgpu::CallbackResources::default();
            resources.insert(Resources::new(&device, format, None));
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = target.create_view(&Default::default());
            Rig { device, queue, size, target, view, resources }
        }

        fn frame(&mut self, cb: &Pane3dCallback) {
            let (w, h) = self.size;
            let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: 1.0 };
            let mut enc = self.device.create_command_encoder(&Default::default());
            let extra = egui_wgpu::CallbackTrait::prepare(cb, &self.device, &self.queue, &screen, &mut enc, &mut self.resources);
            {
                let mut pass = enc
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    })
                    .forget_lifetime();
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32));
                let info = egui::PaintCallbackInfo { viewport: rect, clip_rect: rect, pixels_per_point: 1.0, screen_size_px: [w, h] };
                egui_wgpu::CallbackTrait::paint(cb, info, &mut pass, &self.resources);
            }
            self.queue.submit(extra.into_iter().chain([enc.finish()]));
        }

        fn res(&mut self) -> &mut Resources {
            self.resources.get_mut::<Resources>().expect("resources")
        }

        fn pixels(&self) -> Vec<u8> {
            read_target(&self.device, &self.queue, &self.target, self.size)
        }
    }

    /// The software adapter's compute device, made once for every test that path-traces.
    fn test_compute() -> Option<cadcraft_render3d::pathtrace::ComputeGpu> {
        static GPU: std::sync::OnceLock<Option<cadcraft_render3d::pathtrace::ComputeGpu>> = std::sync::OnceLock::new();
        GPU.get_or_init(request_pane_compute).clone()
    }

    #[test]
    fn the_paint_callback_draws_the_scene_into_the_window_and_redraws_only_on_change() {
        let Some((device, queue)) = test_device() else { return };
        let size = (64u32, 48u32);
        let mut rig = Rig::new(device, queue, size);
        let mut pane = Pane3d::new();
        pane.available = true;
        assert!(pane.sync(1, Some(&house())));
        rig.frame(&callback_for(&pane, size));
        let px = rig.pixels();
        assert!(px.chunks_exact(4).all(|p| p != [255, 255, 255, 255]), "the blit left part of the pane unpainted");
        let first = &px[..4];
        assert!(px.chunks_exact(4).any(|p| p != first), "the pane shows only the background: the scene was not drawn");
        assert_eq!(rig.res().draws, 1);
        // An unchanged camera and scene: the next frames only blit.
        rig.frame(&callback_for(&pane, size));
        rig.frame(&callback_for(&pane, size));
        assert_eq!(rig.res().draws, 1);
        // Orbiting draws again.
        orbit_by_drag(&mut pane, 30.0, 0.0, 0.01);
        rig.frame(&callback_for(&pane, size));
        assert_eq!(rig.res().draws, 2);
    }

    #[test]
    fn pane3d_style_selector_cycles_all_four() {
        let mut s = Style3d::default();
        assert_eq!(s, Style3d::Shaded);
        let mut seen = vec![s];
        for _ in 0..5 {
            s = s.next();
            assert!(!seen.contains(&s));
            seen.push(s);
        }
        assert_eq!(seen, Style3d::ALL.to_vec());
        assert_eq!(s.next(), Style3d::Shaded);
        let styles: std::collections::HashSet<_> = Style3d::ALL.iter().map(|s| format!("{:?}", s.render_style())).collect();
        assert_eq!(styles.len(), 6);
    }

    #[test]
    fn style_selector_offers_clay_and_technical() {
        assert_eq!(
            Style3d::ALL,
            [Style3d::Shaded, Style3d::ShadedEdges, Style3d::HiddenLine, Style3d::GlassHouse, Style3d::Clay, Style3d::Technical]
        );
        let mut s = Style3d::Shaded;
        let mut seen = vec![s];
        for _ in 0..5 {
            s = s.next();
            seen.push(s);
        }
        assert_eq!(seen, Style3d::ALL.to_vec());
        assert_eq!(s.next(), Style3d::Shaded);
        assert_eq!(Style3d::Clay.label(), "Clay");
        assert_eq!(Style3d::Technical.label(), "Technical");
        assert!(matches!(Style3d::Clay.render_style(), cadcraft_render3d::Style::Clay));
        assert!(matches!(Style3d::Technical.render_style(), cadcraft_render3d::Style::Technical));
    }

    fn full_of(pane: &Pane3d) -> cadcraft_render3d::camera::FullCamera {
        match pane.camera() {
            cadcraft_render3d::Camera::Full(f) => f,
            other => panic!("expected a full camera, got {other:?}"),
        }
    }

    #[test]
    fn pane3d_walkthrough_moves_the_eye_in_plan() {
        let mut pane = Pane3d::new();
        let eye = cadcraft_geom::Vec3::new(1.0, 2.0, 1.6);
        pane.full = Some(cadcraft_render3d::camera::FullCamera::new(eye, cadcraft_geom::Vec3::new(1.0, 12.0, 1.6), 60.0));
        // Yaw 0 faces +Y: forward 2 steps adds (0, 2); strafe 3 adds (3, 0).
        pane.walk(2.0, 0.0, WALK_STEP);
        let f = full_of(&pane);
        assert!((f.eye.x - 1.0).abs() < 1e-9 && (f.eye.y - 4.0).abs() < 1e-9 && (f.eye.z - 1.6).abs() < 1e-9);
        pane.walk(0.0, 3.0, WALK_STEP);
        let f = full_of(&pane);
        assert!((f.eye.x - 4.0).abs() < 1e-9 && (f.eye.y - 4.0).abs() < 1e-9);
        // Shift runs 4 m; turning to yaw 90 (facing +X) then walking forward adds (4, 0).
        pane.turn(90.0);
        pane.walk(1.0, 0.0, WALK_STEP * RUN_FACTOR);
        let f = full_of(&pane);
        assert!((f.eye.x - 8.0).abs() < 1e-9 && (f.eye.y - 4.0).abs() < 1e-9, "{:?}", f.eye);
        // From the orbit camera the walk starts at its eye and the pane switches to the full camera.
        let mut orbiting = Pane3d::new();
        assert!(orbiting.full.is_none());
        orbiting.walk(1.0, 0.0, WALK_STEP);
        assert!(orbiting.full.is_some());
        orbiting.use_orbit();
        assert!(matches!(orbiting.camera(), cadcraft_render3d::Camera::Orbit(_)));
    }

    #[test]
    fn pane3d_pick_selects_the_same_proxy_handle_as_2d() {
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&house())));
        let size = (400u32, 300u32);
        let cam = pane.camera();
        // The open-topped house may let the centre ray through: take the first pixel that strikes it.
        let (px, hit) = (0..30)
            .flat_map(|y| (0..40).map(move |x| [x as f32 * 10.0 + 5.0, y as f32 * 10.0 + 5.0]))
            .find_map(|px| {
                let (o, d) = cadcraft_render3d::camera::ray_at(&cam, size, px)?;
                Some((px, pane.scene()?.pick(o, d)?))
            })
            .expect("some pixel of the view hits the house");
        let handle = pane.pick(size, px).expect("pick");
        assert_eq!(handle, hit.owner.handle(hit.object));
        assert_eq!(handle, cadcraft_doc::arch::arch_handle(hit.object), "a wall hit is the wall's proxy handle");
        assert_eq!(cadcraft_engine::grips::arch_id_from_handle(handle), Some(cadcraft_engine::grips::ArchTarget::Wall(hit.object)));
        assert_eq!(pane.take_picked(), Some(handle));
        // A miss reports nothing and clears nothing.
        pane.pick(size, px);
        assert_eq!(pane.pick(size, [f32::NAN, 0.0]), None);
        assert_eq!(pane.take_picked(), Some(handle));
    }

    #[test]
    fn pane3d_camera_move_does_not_reupload_meshes() {
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&house())));
        let key = pane.scene_key();
        orbit_by_drag(&mut pane, 40.0, 10.0, 0.01);
        zoom_by_wheel(&mut pane, 1.0);
        pane.walk(1.0, 1.0, WALK_STEP);
        pane.turn(10.0);
        assert_eq!(pane.scene_key(), key, "camera moves must not change the mesh cache key");
        assert!(!pane.sync(1, Some(&house())), "and must not rebuild the scene");
        let Some((device, queue)) = test_device() else { return };
        let mut off = cadcraft_render3d::Offscreen::with_device(device, queue, (32, 24));
        let scene = pane.scene.clone().expect("scene");
        off.render(&scene, key, &pane.camera(), &cadcraft_render3d::Light::DEFAULT);
        pane.walk(2.0, 0.0, WALK_STEP);
        off.render(&scene, key, &pane.camera(), &cadcraft_render3d::Light::DEFAULT);
        pane.use_orbit();
        off.render(&scene, key, &pane.camera(), &cadcraft_render3d::Light::DEFAULT);
        assert_eq!(off.uploads(), 1);
    }

    #[test]
    fn pane3d_loads_the_active_saved_camera() {
        use cadcraft_doc::arch::{Camera3d, CameraKind};
        let mut arch = house();
        arch.set_camera(Camera3d {
            name: "walk".into(),
            kind: CameraKind::Full,
            eye: Some([5.0, -3.0]),
            eye_height: Some(1.7),
            target: Some([5.0, 4.0, 1.7]),
            fov_deg: Some(70.0),
            ..Camera3d::default()
        })
        .expect("camera");
        arch.set_camera(Camera3d {
            name: "high".into(),
            kind: CameraKind::Orbit,
            azimuth_deg: Some(45.0),
            elevation_deg: Some(30.0),
            distance: Some(25.0),
            target: Some([1.0, 2.0, 3.0]),
            ..Camera3d::default()
        })
        .expect("camera");
        arch.active_camera = Some("walk".into());
        let mut pane = Pane3d::new();
        assert!(pane.sync(1, Some(&arch)));
        let f = full_of(&pane);
        assert!((f.eye.x - 5.0).abs() < 1e-9 && (f.eye.y + 3.0).abs() < 1e-9 && (f.eye.z - 1.7).abs() < 1e-9);
        assert!((f.fov_deg - 70.0).abs() < 1e-9);
        // Unchanged: nothing reloads, so the user's walking is kept.
        pane.walk(1.0, 0.0, WALK_STEP);
        assert!(!pane.load_active_camera(&arch));
        assert!((full_of(&pane).eye.y + 2.0).abs() < 1e-9);
        // Switching the active camera loads the orbit one.
        arch.active_camera = Some("high".into());
        assert!(pane.sync(2, Some(&arch)));
        match pane.camera() {
            cadcraft_render3d::Camera::Orbit(o) => {
                assert!((o.azimuth - 45.0).abs() < 1e-9 && (o.elevation - 30.0).abs() < 1e-9 && (o.distance - 25.0).abs() < 1e-9);
                assert!((o.target.x - 1.0).abs() < 1e-9 && (o.target.z - 3.0).abs() < 1e-9);
            }
            other => panic!("expected an orbit camera, got {other:?}"),
        }
    }

    #[test]
    fn a_3d_pick_becomes_the_2d_selection() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];

        // The centre pixel strikes the door; its proxy handle is the opening's (0x8000 range),
        // not the wall's.
        assert!(pane.pick(size, center).is_some(), "the centre pixel strikes the door");
        assert!(apply_pick(&mut pane, &mut session).is_some(), "the door pick must be applied to the selection");
        assert_eq!(
            sel_handles(&session),
            vec![cadcraft_doc::arch::opening_handle(cadcraft_doc::ArchId(0)).hex()],
            "a 3D door pick selects the opening handle"
        );
    }

    #[test]
    fn a_3d_miss_leaves_the_selection_alone() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];
        let sky = [8.0, 8.0];

        // Establish a baseline selection via a real 3D pick.
        assert!(pane.pick(size, center).is_some(), "the centre pixel strikes the door");
        assert!(apply_pick(&mut pane, &mut session).is_some());
        let before = sel_handles(&session);
        assert_eq!(
            before,
            vec![cadcraft_doc::arch::opening_handle(cadcraft_doc::ArchId(0)).hex()],
            "the baseline 3D pick selects the opening handle"
        );

        // A sky pick strikes nothing and must leave that selection exactly as it was.
        assert!(pane.pick(size, sky).is_none(), "a sky pixel must strike nothing");
        assert!(apply_pick(&mut pane, &mut session).is_none(), "a miss has no handle to apply");
        assert_eq!(sel_handles(&session), before, "a sky pick leaves the selection alone");
    }

    #[test]
    fn a_second_3d_pick_replaces_the_first() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];

        // First pick: the door at the centre.
        let door_handle = pane.pick(size, center).expect("first pick hits the door");
        apply_pick(&mut pane, &mut session).expect("first pick applied");
        assert_eq!(sel_handles(&session), vec![cadcraft_doc::arch::opening_handle(cadcraft_doc::ArchId(0)).hex()], "first pick selects the door");

        // Second pick: to the right, hitting the wall (not the door).
        let wall_pixel = [350.0, 160.0];
        let wall_handle = pane.pick(size, wall_pixel).expect("second pick hits the wall");
        apply_pick(&mut pane, &mut session).expect("second pick applied");
        let handles = sel_handles(&session);
        assert_eq!(handles.len(), 1, "selection should have exactly one handle");
        assert_eq!(handles[0], wall_handle.hex(), "selection should be the wall handle");
        assert_ne!(handles[0], door_handle.hex(), "selection should not still be the door");
        assert_eq!(
            cadcraft_engine::grips::arch_id_from_handle(wall_handle),
            Some(cadcraft_engine::grips::ArchTarget::Wall(cadcraft_doc::ArchId(0))),
            "second pick should select the wall"
        );
    }

    #[test]
    fn apply_pick_with_nothing_picked_leaves_selection_alone() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];

        // Establish a selection.
        pane.pick(size, center).expect("pick hits the door");
        apply_pick(&mut pane, &mut session).expect("pick applied");
        let before = sel_handles(&session);

        // Calling apply_pick without a new pick is a no-op.
        assert!(apply_pick(&mut pane, &mut session).is_none(), "no pick, no apply");
        assert_eq!(sel_handles(&session), before, "selection unchanged");
    }

    #[test]
    fn a_3d_pick_replaces_a_multi_selection() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];

        // Establish a 2D multi-selection.
        let wall0 = cadcraft_doc::arch::arch_handle(cadcraft_doc::ArchId(0));
        let wall1 = cadcraft_doc::arch::arch_handle(cadcraft_doc::ArchId(1));
        session.set_selection(vec![wall0, wall1]);
        assert_eq!(sel_handles(&session).len(), 2, "two handles selected");

        // A 3D pick replaces the multi-selection with a single handle.
        pane.pick(size, center).expect("pick hits the door");
        apply_pick(&mut pane, &mut session).expect("pick applied");
        let handles = sel_handles(&session);
        assert_eq!(handles.len(), 1, "3D pick replaces multi-selection with single");
        assert_eq!(handles[0], cadcraft_doc::arch::opening_handle(cadcraft_doc::ArchId(0)).hex(), "the single selection is the picked opening");
    }

    #[test]
    fn a_3d_pick_round_trips_through_arch_id_from_handle() {
        let (mut pane, mut session) = house_pane();
        let (_, size) = front_camera();
        let center = [240.0, 160.0];

        pane.pick(size, center).expect("pick hits the door");
        let handle = apply_pick(&mut pane, &mut session).expect("pick applied");

        // The handle must resolve back to the opening through the single disambiguator.
        let target = cadcraft_engine::grips::arch_id_from_handle(handle);
        assert_eq!(
            target,
            Some(cadcraft_engine::grips::ArchTarget::Opening(cadcraft_doc::ArchId(0))),
            "the picked handle must resolve back to the opening"
        );
    }

    #[test]
    fn a_pick_with_no_scene_returns_none() {
        let mut pane = Pane3d::new();
        let mut session = Session::new();

        // No scene built: pick and apply are both no-ops.
        assert!(pane.pick((100, 100), [50.0, 50.0]).is_none(), "no scene, no pick");
        assert!(apply_pick(&mut pane, &mut session).is_none(), "no pick, no apply");
    }

    /// The house with a site at 51.05 N and true north 30, synced into a fresh pane.
    fn site_pane() -> Pane3d {
        let (mut pane, session) = house_pane();
        let st = session.state().expect("session state");
        let mut model = st.doc.arch.clone();
        model.site = Some(cadcraft_doc::arch::SiteLocation { latitude: 51.05, true_north: 30.0, ..Default::default() });
        assert!(pane.sync(st.doc.arch_revision + 1, Some(&model)), "the model with a site rebuilds the scene");
        pane
    }

    #[test]
    fn pane_night_turns_the_sun_off() {
        let mut pane = site_pane();
        assert!(pane.lighting().sun.is_some(), "daytime has a sun");
        pane.night = true;
        let l = pane.lighting();
        assert!(l.sun.is_none(), "night has no sun");
        assert!(!l.sun_shadow, "and no sun shadow");
        pane.night = false;
        assert!(pane.lighting().sun.is_some(), "day brings the sun back");
    }

    #[test]
    fn pane_sun_comes_from_the_site() {
        let mut pane = site_pane();
        let l = pane.lighting();
        assert!(l.sun_shadow, "a site sun casts shadows");
        let sun = cadcraft_arch::sun::sun_position(51.05, "2026-06-21", "15:00").expect("a valid sun");
        let site = cadcraft_doc::arch::SiteLocation { latitude: 51.05, true_north: 30.0, ..Default::default() };
        let want = cadcraft_arch::site::true_to_drawing(Some(&site), cadcraft_arch::sun::sun_dir(&sun));
        let got = l.sun.expect("a sun").dir;
        for i in 0..3 {
            assert!((f64::from(got[i]) - want[i]).abs() < 1e-5, "axis {i}: {got:?} vs {want:?}");
        }
        // A bad time keeps the last good sun and flags the entry.
        pane.sun_time = "99:99".into();
        assert_eq!(pane.lighting().sun.map(|s| s.dir), l.sun.map(|s| s.dir));
        assert!(!pane.sun_valid);
        // No site: the default light, no sun shadow.
        let mut bare = Pane3d::new();
        assert_eq!(bare.lighting(), cadcraft_render3d::Lighting::default());
    }

    #[test]
    fn pane_camera_move_keeps_lighting_and_scene_key() {
        let mut pane = site_pane();
        let (lighting, key) = (pane.lighting(), pane.scene_key());
        orbit_by_drag(&mut pane, 40.0, 10.0, 0.01);
        zoom_by_wheel(&mut pane, 2.0);
        pane.walk(1.0, 0.0, WALK_STEP);
        assert_eq!(pane.lighting(), lighting);
        assert_eq!(pane.scene_key(), key);
    }

    #[test]
    fn pane_final_image_clears_on_camera_change() {
        let mut pane = site_pane();
        let vp = |c: f32| [[c; 4]; 4];
        let state: ViewState = (pane.scene_key(), (320, 200), vp(1.0), pane.style, pane.lighting(), 2);
        let still = || FinalDone { state, width: 1, height: 1, rgba: vec![0; 4] };
        let mut f = FinalUi { done: Some(still()), progress: Some(0.5), started: Some(state), ..Default::default() };
        f.observe(&state);
        assert!(f.done.is_some() && !f.cancel, "the same view keeps the still and the render");
        let moved: ViewState = (state.0, state.1, vp(2.0), state.3, state.4, state.5);
        f.observe(&moved);
        assert!(f.done.is_none(), "a camera move drops the still");
        assert!(f.cancel, "and cancels a running render");
        // A lighting change does too.
        let mut g = FinalUi { done: Some(still()), ..Default::default() };
        pane.night = true;
        let night: ViewState = (state.0, state.1, state.2, state.3, pane.lighting(), state.5);
        g.observe(&night);
        assert!(g.done.is_none(), "night drops the still");
    }

    use egui_kittest::kittest::Queryable;

    /// The house pane in a kittest harness drawing the real pane UI. The accessibility check is
    /// off: the sun's date and time entries (phase 9) have no accessible name yet.
    fn pane_harness() -> egui_kittest::Harness<'static, Pane3d> {
        let mut pane = Pane3d::new();
        pane.available = true;
        assert!(pane.sync(1, Some(&house())));
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 300.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, pane: &mut Pane3d| draw(ui, pane), pane)
    }

    #[test]
    fn finalize_offers_path_traced_and_raster() {
        use cadcraft_engine::FinalMode;
        let mut h = pane_harness();
        h.run();
        assert!(h.query_by_label("Path traced").is_some() && h.query_by_label("Raster").is_some(), "the Finalize control lists both");
        let mode = |h: &egui_kittest::Harness<'_, Pane3d>| lock_final(&h.state().finalize).mode;
        assert_eq!(mode(&h), FinalMode::default(), "it starts as render's final: true does");
        h.get_by_label("Path traced").click();
        h.run();
        assert_eq!(mode(&h), FinalMode::Path);
        assert!(h.query_by_label("Samples").is_some(), "a path Finalize asks for its samples");
        assert_eq!(lock_final(&h.state().finalize).samples, cadcraft_render3d::pathtrace::DEFAULT_SPP);
        h.get_by_label("Raster").click();
        h.run();
        assert_eq!(mode(&h), FinalMode::Raster);
        assert!(h.query_by_label("Samples").is_none(), "a raster Finalize has no samples");
    }

    #[test]
    fn path_finalize_shows_its_samples_and_refines() {
        let Some((device, queue)) = test_device() else { return };
        let Some(compute) = test_compute() else {
            eprintln!("SKIP: no compute device on the software adapter");
            return;
        };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(Some(compute));
        let mut h = pane_harness();
        h.run();
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.samples = 16;
        }
        // The samples entry appears and moves the button: lay it out before clicking.
        h.run();
        h.get_by_label("Finalize Render").click();
        h.run();
        assert!(lock_final(&h.state().finalize).request, "the button asks the paint callback for a render");
        let (mut seen, mut previewed) = (Vec::new(), false);
        for _ in 0..400 {
            rig.frame(&callback_for(h.state(), size));
            let (path, finished) = {
                let f = lock_final(&h.state().finalize);
                (f.path_progress, f.done.is_some())
            };
            if finished {
                break;
            }
            let (done, wanted) = path.expect("a running path Finalize reports its samples");
            assert_eq!(wanted, 16);
            h.step();
            assert!(h.query_by_label(&samples_label(done, wanted)).is_some(), "the pane shows {done} / {wanted} samples");
            previewed |= rig.res().final_image.is_some();
            seen.push(done);
        }
        assert!(previewed, "the image so far is on screen while it refines");
        assert!(seen.windows(2).all(|w| w[0] <= w[1]) && seen.first() < seen.last(), "the samples rise: {seen:?}");
        let f = lock_final(&h.state().finalize);
        let still = f.done.as_ref().expect("the path Finalize ends with a still");
        assert_eq!((still.width, still.height), size);
        assert_eq!(still.rgba.len(), (size.0 * size.1 * 4) as usize);
        assert!(f.progress.is_none() && f.path_progress.is_none() && !f.fallback && f.error.is_none());
        drop(f);
        assert!(rig.res().final_image.is_some(), "the still replaces the live view");
    }

    #[test]
    fn cancel_stops_a_path_finalize() {
        let Some((device, queue)) = test_device() else { return };
        let Some(compute) = test_compute() else {
            eprintln!("SKIP: no compute device on the software adapter");
            return;
        };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(Some(compute));
        let mut h = pane_harness();
        h.run();
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.samples = 4096;
            f.request = true;
        }
        for _ in 0..3 {
            rig.frame(&callback_for(h.state(), size));
            h.step();
        }
        assert!(lock_final(&h.state().finalize).path_progress.is_some(), "the path Finalize runs");
        assert!(rig.res().final_render.is_some());
        h.get_by_label("Cancel").click();
        h.step();
        rig.frame(&callback_for(h.state(), size));
        {
            let f = lock_final(&h.state().finalize);
            assert!(f.progress.is_none() && f.path_progress.is_none(), "Cancel stops it");
            assert!(f.done.is_none() && f.error.is_none(), "with no still and no error");
        }
        assert!(rig.res().final_render.is_none(), "the tracer is dropped");
        assert!(rig.res().final_image.is_none(), "the live view is back");
        h.step();
        assert!(h.query_by_label("Finalize Render").is_some(), "the button is back");
        assert!(h.query_by_label("Cancel").is_none());
    }

    #[test]
    fn path_finalize_without_compute_is_the_raster_still_with_a_note() {
        let Some((device, queue)) = test_device() else { return };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(None);
        let mut h = pane_harness();
        h.run();
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.request = true;
        }
        for _ in 0..200 {
            rig.frame(&callback_for(h.state(), size));
            if lock_final(&h.state().finalize).done.is_some() {
                break;
            }
        }
        let f = lock_final(&h.state().finalize);
        assert!(f.fallback, "the render notes it ran as raster");
        assert!(f.path_progress.is_none(), "a raster Finalize counts no samples");
        let still = f.done.as_ref().expect("the raster still");
        assert_eq!((still.width, still.height), size);
        drop(f);
        h.run();
        assert!(h.query_by_label("No compute shaders: raster Finalize").is_some(), "the pane says why");
    }

    #[test]
    fn camera_move_cancels_running_path_finalize() {
        let Some((device, queue)) = test_device() else { return };
        let Some(compute) = test_compute() else {
            eprintln!("SKIP: no compute device on the software adapter");
            return;
        };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(Some(compute));
        let mut h = pane_harness();
        h.run();
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.samples = 4096;
            f.request = true;
        }
        // Let it run a few frames
        for _ in 0..3 {
            rig.frame(&callback_for(h.state(), size));
            h.step();
        }
        assert!(lock_final(&h.state().finalize).path_progress.is_some(), "the path Finalize runs");
        // Orbit the camera
        orbit_by_drag(h.state_mut(), 40.0, 10.0, 0.01);
        rig.frame(&callback_for(h.state(), size));
        {
            let f = lock_final(&h.state().finalize);
            assert!(f.path_progress.is_none(), "camera move cancels the path Finalize");
            assert!(f.done.is_none(), "no still after cancel");
            assert!(f.error.is_none(), "no error after cancel");
        }
        h.step();
        assert!(h.query_by_label("Finalize Render").is_some(), "the button is back");
    }

    #[test]
    fn sequential_path_finalizes_work() {
        let Some((device, queue)) = test_device() else { return };
        let Some(compute) = test_compute() else {
            eprintln!("SKIP: no compute device on the software adapter");
            return;
        };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(Some(compute));
        let mut h = pane_harness();
        h.run();
        // First finalize
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.samples = 16;
            f.request = true;
        }
        for _ in 0..400 {
            rig.frame(&callback_for(h.state(), size));
            if lock_final(&h.state().finalize).done.is_some() {
                break;
            }
        }
        assert!(lock_final(&h.state().finalize).done.is_some(), "first finalize completes");
        // Second finalize with different samples
        {
            let mut f = lock_final(&h.state().finalize);
            f.samples = 32;
            f.request = true;
        }
        for _ in 0..400 {
            rig.frame(&callback_for(h.state(), size));
            if lock_final(&h.state().finalize).done.is_some() {
                break;
            }
        }
        let f = lock_final(&h.state().finalize);
        assert!(f.done.is_some(), "second finalize completes");
        assert_eq!(f.done.as_ref().unwrap().width, size.0);
        assert_eq!(f.done.as_ref().unwrap().height, size.1);
    }

    #[test]
    fn path_finalize_clamps_samples() {
        let Some((device, queue)) = test_device() else { return };
        let Some(compute) = test_compute() else {
            eprintln!("SKIP: no compute device on the software adapter");
            return;
        };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        rig.res().compute = Some(Some(compute));
        let mut h = pane_harness();
        h.run();
        // Set samples to 0 (below minimum)
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Path;
            f.samples = 0;
            f.request = true;
        }
        // The render should start with 1 sample (clamped)
        let mut saw_progress = false;
        for _ in 0..400 {
            rig.frame(&callback_for(h.state(), size));
            let path = lock_final(&h.state().finalize).path_progress;
            if let Some((_, wanted)) = path {
                assert_eq!(wanted, 1, "samples below 1 clamp to 1");
                saw_progress = true;
            }
            if lock_final(&h.state().finalize).done.is_some() {
                break;
            }
        }
        assert!(saw_progress, "the path Finalize reported progress");
        assert!(lock_final(&h.state().finalize).done.is_some(), "finalize completes with clamped samples");
    }

    #[test]
    fn raster_finalize_works_after_path_finalize() {
        let Some((device, queue)) = test_device() else { return };
        let size = (48u32, 32u32);
        let mut rig = Rig::new(device, queue, size);
        let mut h = pane_harness();
        h.run();
        // Run a path finalize first (if compute available)
        let compute = test_compute();
        if let Some(c) = compute {
            rig.res().compute = Some(Some(c));
            {
                let mut f = lock_final(&h.state().finalize);
                f.mode = cadcraft_engine::FinalMode::Path;
                f.samples = 16;
                f.request = true;
            }
            for _ in 0..400 {
                rig.frame(&callback_for(h.state(), size));
                if lock_final(&h.state().finalize).done.is_some() {
                    break;
                }
            }
            assert!(lock_final(&h.state().finalize).done.is_some(), "path finalize completes");
        }
        // Now run a raster finalize
        {
            let mut f = lock_final(&h.state().finalize);
            f.mode = cadcraft_engine::FinalMode::Raster;
            f.request = true;
        }
        for _ in 0..200 {
            rig.frame(&callback_for(h.state(), size));
            if lock_final(&h.state().finalize).done.is_some() {
                break;
            }
        }
        assert!(lock_final(&h.state().finalize).done.is_some(), "raster finalize completes after path");
    }
}
