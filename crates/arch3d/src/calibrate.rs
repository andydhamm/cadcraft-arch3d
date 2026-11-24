//! Calibration scenes (phase 22): spheres, boxes and point lights that replace the model for a
//! `render {calibrate}`.
//!
//! [`Calibration`] describes up to [`MAX_OBJECTS`] primitives, each with its own material, and up
//! to [`MAX_LIGHTS`] point lights. [`validate`] checks it against the caps; [`scene`] builds the
//! [`Scene3d`]: one closed mesh per object under the material key `calibrate-{i}`, no horizon and
//! no backdrop. Deterministic: no hashing, no randomness.

use crate::light::SceneLight;
use crate::material_ext::MaterialExt;
use crate::mesh::{Bounds3, Mesh};
use crate::{MaterialDef, Scene3d};
use cadcraft_doc::ArchId;
use cadcraft_doc::arch::LightKind;
use cadcraft_geom::Vec3;

/// The most objects a calibration scene holds.
pub const MAX_OBJECTS: usize = 16;
/// The most lights a calibration scene holds.
pub const MAX_LIGHTS: usize = 8;
/// The most longitude segments of a sphere.
pub const MAX_SEGMENTS: u32 = 512;
/// The largest size of an object, in metres.
pub const MAX_SIZE: f64 = 1000.0;
/// The largest absolute coordinate, in metres.
pub const MAX_COORD: f64 = 1e5;

/// The shape of a calibration object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CalibShape {
    /// A UV sphere.
    #[default]
    Sphere,
    /// An axis-aligned box.
    Box,
}

/// How a calibration object looks.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibMaterial {
    /// Linear RGB base colour.
    pub color: [f32; 3],
    /// Metallic factor, 0..=1.
    pub metallic: f32,
    /// Roughness, 0..=1.
    pub roughness: f32,
    /// Emitted light in nits (white).
    pub emission: f32,
    /// The optional PBR fields.
    pub ext: MaterialExt,
}

impl Default for CalibMaterial {
    /// 0.8 grey, dielectric, roughness 0.5, not emissive.
    fn default() -> Self {
        CalibMaterial { color: [0.8; 3], metallic: 0.0, roughness: 0.5, emission: 0.0, ext: MaterialExt::default() }
    }
}

/// One primitive of a calibration scene.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibObject {
    /// Sphere or box.
    pub shape: CalibShape,
    /// The centre, in metres.
    pub at: [f64; 3],
    /// The extent along x, y and z; a sphere's diameter is `size[0]`.
    pub size: [f64; 3],
    /// A sphere's longitude segments; its latitude segments are half as many.
    pub segments: u32,
    /// The surface.
    pub material: CalibMaterial,
}

impl Default for CalibObject {
    /// A one-metre sphere at the origin with 192 segments.
    fn default() -> Self {
        CalibObject { shape: CalibShape::Sphere, at: [0.0; 3], size: [1.0; 3], segments: 192, material: CalibMaterial::default() }
    }
}

/// A point light of a calibration scene.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibLight {
    /// The position, in metres.
    pub at: [f64; 3],
    /// Luminous intensity in candela.
    pub candela: f64,
    /// Colour temperature in kelvin.
    pub kelvin: f64,
}

impl Default for CalibLight {
    /// 100 cd at 6500 K (white) above the origin.
    fn default() -> Self {
        CalibLight { at: [0.0, 0.0, 3.0], candela: 100.0, kelvin: 6500.0 }
    }
}

/// The calibration scene: objects and lights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calibration {
    /// The primitives, 1..=[`MAX_OBJECTS`].
    pub objects: Vec<CalibObject>,
    /// The point lights, at most [`MAX_LIGHTS`].
    pub lights: Vec<CalibLight>,
}

fn in_range(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if v.is_finite() && v >= lo && v <= hi { Ok(()) } else { Err(format!("{name} must be within {lo}..={hi}, got {v}")) }
}

fn unit(name: &str, v: f32) -> Result<(), String> {
    in_range(name, f64::from(v), 0.0, 1.0)
}

fn coords(name: &str, p: &[f64; 3]) -> Result<(), String> {
    for v in p {
        in_range(name, *v, -MAX_COORD, MAX_COORD)?;
    }
    Ok(())
}

fn validate_material(m: &CalibMaterial) -> Result<(), String> {
    for c in m.color {
        unit("colour", c)?;
    }
    unit("metallic", m.metallic)?;
    unit("roughness", m.roughness)?;
    in_range("emission", f64::from(m.emission), 0.0, 1e9)?;
    let e = &m.ext;
    unit("clearcoat", e.clearcoat)?;
    unit("clearcoat roughness", e.clearcoat_roughness)?;
    unit("anisotropy", e.anisotropy)?;
    in_range("anisotropy rotation", f64::from(e.anisotropy_rotation), -1e3, 1e3)?;
    for c in e.sheen {
        unit("sheen", c)?;
    }
    unit("sheen roughness", e.sheen_roughness)?;
    unit("transmission", e.transmission)?;
    in_range("ior", f64::from(e.ior), 1.0, 3.0)?;
    in_range("thickness", f64::from(e.thickness), 0.0, 10.0)?;
    if let Some(r) = e.edge_radius {
        in_range("edge radius", f64::from(r), 0.0, 0.05)?;
    }
    in_range("variation scale", f64::from(e.variation_scale), 0.0, 100.0)?;
    unit("variation amount", e.variation_amount)?;
    if let Some(v) = e.metallic {
        unit("metallic override", v)?;
    }
    if let Some(v) = e.roughness {
        unit("roughness override", v)?;
    }
    Ok(())
}

/// Check a calibration against the caps: 1..=16 objects, at most 8 lights, finite coordinates
/// within [`MAX_COORD`], sizes in (0, [`MAX_SIZE`]], segments 8..=512 and every material factor in
/// its range.
pub fn validate(c: &Calibration) -> Result<(), String> {
    if c.objects.is_empty() || c.objects.len() > MAX_OBJECTS {
        return Err(format!("a calibration needs 1..={MAX_OBJECTS} objects, got {}", c.objects.len()));
    }
    if c.lights.len() > MAX_LIGHTS {
        return Err(format!("a calibration takes at most {MAX_LIGHTS} lights, got {}", c.lights.len()));
    }
    for (i, o) in c.objects.iter().enumerate() {
        let wrap = |r: Result<(), String>| r.map_err(|e| format!("object {i}: {e}"));
        wrap(coords("position", &o.at))?;
        let n = if o.shape == CalibShape::Sphere { 1 } else { 3 };
        for s in o.size.iter().take(n) {
            if !(s.is_finite() && *s > 0.0 && *s <= MAX_SIZE) {
                return Err(format!("object {i}: size must be in (0, {MAX_SIZE}], got {s}"));
            }
        }
        if o.shape == CalibShape::Sphere && !(8..=MAX_SEGMENTS).contains(&o.segments) {
            return Err(format!("object {i}: segments must be 8..={MAX_SEGMENTS}, got {}", o.segments));
        }
        wrap(validate_material(&o.material))?;
    }
    for (i, l) in c.lights.iter().enumerate() {
        let wrap = |r: Result<(), String>| r.map_err(|e| format!("light {i}: {e}"));
        wrap(coords("position", &l.at))?;
        if !(l.candela.is_finite() && l.candela > 0.0 && l.candela <= 1e9) {
            return Err(format!("light {i}: candela must be in (0, 1e9], got {}", l.candela));
        }
        wrap(in_range("kelvin", l.kelvin, 1000.0, 20000.0))?;
    }
    Ok(())
}

fn push_tri(m: &mut Mesh, p: [[f32; 3]; 3], uv: [[f32; 2]; 3]) {
    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let mut n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 {
        n = [n[0] / len, n[1] / len, n[2] / len];
    }
    let base = m.positions.len() as u32;
    for k in 0..3 {
        m.positions.push(p[k]);
        m.normals.push(n);
        m.uvs.push(uv[k]);
        m.indices.push(base + k as u32);
    }
}

fn sphere_mesh(o: &CalibObject, key: &str, id: ArchId) -> Mesh {
    let mut m = Mesh::new(key, id);
    let lon = o.segments.clamp(8, MAX_SEGMENTS) as usize;
    let lat = (lon / 2).max(2);
    let r = o.size[0] / 2.0;
    let point = |i: usize, j: usize| -> [f32; 3] {
        let theta = std::f64::consts::PI * i as f64 / lat as f64;
        let phi = std::f64::consts::TAU * (j % lon) as f64 / lon as f64;
        let (ring, z) = if i == 0 {
            (0.0, r)
        } else if i == lat {
            (0.0, -r)
        } else {
            (r * theta.sin(), r * theta.cos())
        };
        [(o.at[0] + ring * phi.cos()) as f32, (o.at[1] + ring * phi.sin()) as f32, (o.at[2] + z) as f32]
    };
    let uv = |i: usize, j: usize| [j as f32 / lon as f32, i as f32 / lat as f32];
    for i in 0..lat {
        for j in 0..lon {
            let (a, b, c, d) = (point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
            // The pole rows collapse one triangle of each quad to a point.
            if i + 1 < lat {
                push_tri(&mut m, [a, b, c], [uv(i, j), uv(i + 1, j), uv(i + 1, j + 1)]);
            }
            if i > 0 {
                push_tri(&mut m, [a, c, d], [uv(i, j), uv(i + 1, j + 1), uv(i, j + 1)]);
            }
        }
    }
    m
}

fn box_mesh(o: &CalibObject, key: &str, id: ArchId) -> Mesh {
    let mut m = Mesh::new(key, id);
    let h = [o.size[0] / 2.0, o.size[1] / 2.0, o.size[2] / 2.0];
    let corner = |c: &[f64; 3]| -> [f32; 3] { [(o.at[0] + c[0] * h[0]) as f32, (o.at[1] + c[1] * h[1]) as f32, (o.at[2] + c[2] * h[2]) as f32] };
    // Each face: four corners counter-clockwise seen from outside.
    let faces: [[[f64; 3]; 4]; 6] = [
        [[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]],
        [[-1., 1., -1.], [-1., -1., -1.], [-1., -1., 1.], [-1., 1., 1.]],
        [[1., 1., -1.], [-1., 1., -1.], [-1., 1., 1.], [1., 1., 1.]],
        [[-1., -1., -1.], [1., -1., -1.], [1., -1., 1.], [-1., -1., 1.]],
        [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
        [[-1., 1., -1.], [1., 1., -1.], [1., -1., -1.], [-1., -1., -1.]],
    ];
    let q = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    for f in &faces {
        let p = [corner(&f[0]), corner(&f[1]), corner(&f[2]), corner(&f[3])];
        push_tri(&mut m, [p[0], p[1], p[2]], [q[0], q[1], q[2]]);
        push_tri(&mut m, [p[0], p[2], p[3]], [q[0], q[2], q[3]]);
    }
    m
}

/// Build the calibration scene: one closed mesh per object with material `calibrate-{i}` and
/// object id `i + 1`, one point [`SceneLight`] per light. An invalid calibration (see
/// [`validate`]) gives an empty scene.
pub fn scene(c: &Calibration) -> Scene3d {
    let mut scene = Scene3d::default();
    if validate(c).is_err() {
        return scene;
    }
    let mut bounds: Option<Bounds3> = None;
    for (i, o) in c.objects.iter().enumerate() {
        let key = format!("calibrate-{i}");
        let id = ArchId(i as u32 + 1);
        let mesh = match o.shape {
            CalibShape::Sphere => sphere_mesh(o, &key, id),
            CalibShape::Box => box_mesh(o, &key, id),
        };
        let half = match o.shape {
            CalibShape::Sphere => [o.size[0] / 2.0; 3],
            CalibShape::Box => [o.size[0] / 2.0, o.size[1] / 2.0, o.size[2] / 2.0],
        };
        let b = Bounds3::new([o.at[0] - half[0], o.at[1] - half[1], o.at[2] - half[2]], [o.at[0] + half[0], o.at[1] + half[1], o.at[2] + half[2]]);
        match bounds.as_mut() {
            Some(acc) => acc.extend(&b),
            None => bounds = Some(b),
        }
        let m = &o.material;
        scene.materials.insert(
            key,
            MaterialDef {
                base_color: [m.color[0], m.color[1], m.color[2], 1.0],
                metallic: m.metallic,
                roughness: m.roughness,
                ext: m.ext,
                emission: m.emission,
                ..Default::default()
            },
        );
        scene.meshes.push(mesh);
    }
    scene.bounds = bounds.unwrap_or_default();
    for (i, l) in c.lights.iter().enumerate() {
        scene.lights.push(SceneLight {
            id: ArchId(i as u32 + 1),
            kind: LightKind::Point,
            position: Vec3::new(l.at[0], l.at[1], l.at[2]),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: l.candela,
            color: cadcraft_arch::light::kelvin_to_rgb(l.kelvin),
            cos_inner: -1.0,
            cos_outer: -1.0,
            ..Default::default()
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn key(p: [f32; 3]) -> [i64; 3] {
        [(p[0] * 1e4).round() as i64, (p[1] * 1e4).round() as i64, (p[2] * 1e4).round() as i64]
    }

    fn one(o: CalibObject) -> Calibration {
        Calibration { objects: vec![o], lights: vec![] }
    }

    #[test]
    fn sphere_mesh_is_closed_and_on_its_radius() {
        let o = CalibObject { size: [2.0, 2.0, 2.0], segments: 32, ..Default::default() };
        let s = scene(&one(o));
        let m = &s.meshes[0];
        for p in &m.positions {
            let d = (p[0].powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt();
            assert!((d - 1.0).abs() < 1e-4, "{d}");
        }
        // Closed: every directed edge is used once and its reverse once.
        let mut edges: BTreeMap<([i64; 3], [i64; 3]), i32> = BTreeMap::new();
        let mut volume = 0.0f64;
        for t in m.indices.chunks(3) {
            let v: Vec<[f32; 3]> = t.iter().map(|i| m.positions[*i as usize]).collect();
            for k in 0..3 {
                *edges.entry((key(v[k]), key(v[(k + 1) % 3]))).or_default() += 1;
            }
            let cr = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let x = cr(v[1], v[2]);
            volume += f64::from(v[0][0] * x[0] + v[0][1] * x[1] + v[0][2] * x[2]) / 6.0;
        }
        for ((a, b), n) in &edges {
            assert_eq!(*n, 1);
            assert_eq!(edges.get(&(*b, *a)), Some(&1));
        }
        let expect = 4.0 / 3.0 * std::f64::consts::PI;
        assert!(volume > 0.9 * expect && volume < 1.01 * expect, "{volume}");
    }

    #[test]
    fn box_has_twelve_triangles_and_its_bounds() {
        let o = CalibObject { shape: CalibShape::Box, at: [1.0, 0.0, 0.5], size: [2.0, 4.0, 1.0], ..Default::default() };
        let s = scene(&one(o));
        assert_eq!(s.meshes[0].indices.len(), 36);
        assert_eq!(s.bounds, Bounds3::new([0.0, -2.0, 0.0], [2.0, 2.0, 1.0]));
        let m = &s.meshes[0];
        for t in m.indices.chunks(3) {
            let p: Vec<[f32; 3]> = t.iter().map(|i| m.positions[*i as usize]).collect();
            let n = m.normals[t[0] as usize];
            let c = [(p[0][0] + p[1][0] + p[2][0]) / 3.0 - 1.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0, (p[0][2] + p[1][2] + p[2][2]) / 3.0 - 0.5];
            assert!(c[0] * n[0] + c[1] * n[1] + c[2] * n[2] > 0.0);
        }
    }

    #[test]
    fn each_object_has_its_own_material_and_id() {
        let mut a = CalibObject::default();
        a.material.color = [1.0, 0.0, 0.0];
        a.material.emission = 50.0;
        let b = CalibObject { shape: CalibShape::Box, at: [3.0, 0.0, 0.0], ..Default::default() };
        let s = scene(&Calibration { objects: vec![a, b], lights: vec![] });
        assert_eq!(s.meshes.len(), 2);
        assert_ne!(s.meshes[0].object, s.meshes[1].object);
        assert_eq!(s.meshes[0].material, "calibrate-0");
        assert_eq!(s.meshes[1].material, "calibrate-1");
        assert_eq!(s.materials["calibrate-0"].base_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(s.materials["calibrate-0"].emission, 50.0);
        assert_eq!(s.materials["calibrate-1"].emission, 0.0);
        assert!(s.horizon.is_none() && s.backdrop.is_none());
    }

    #[test]
    fn validate_refuses_caps_and_non_finite() {
        let ok = one(CalibObject::default());
        assert!(validate(&ok).is_ok());
        assert!(validate(&Calibration::default()).is_err());
        let many = Calibration { objects: vec![CalibObject::default(); MAX_OBJECTS + 1], lights: vec![] };
        assert!(validate(&many).is_err());
        let lights = Calibration { objects: ok.objects.clone(), lights: vec![CalibLight::default(); MAX_LIGHTS + 1] };
        assert!(validate(&lights).is_err());
        let bad = |f: &dyn Fn(&mut CalibObject)| {
            let mut o = CalibObject::default();
            f(&mut o);
            validate(&one(o)).is_err()
        };
        assert!(bad(&|o| o.at[0] = f64::NAN));
        assert!(bad(&|o| o.at[1] = MAX_COORD * 2.0));
        assert!(bad(&|o| o.size[0] = 0.0));
        assert!(bad(&|o| o.size[0] = MAX_SIZE + 1.0));
        assert!(bad(&|o| o.segments = 7));
        assert!(bad(&|o| o.segments = MAX_SEGMENTS + 1));
        assert!(bad(&|o| o.material.color[0] = 1.5));
        assert!(bad(&|o| o.material.roughness = f32::NAN));
        assert!(bad(&|o| o.material.emission = 2e9));
        assert!(bad(&|o| o.material.ext.ior = 3.5));
        assert!(bad(&|o| o.material.ext.thickness = 11.0));
        assert!(bad(&|o| o.material.ext.edge_radius = Some(0.1)));
        assert!(bad(&|o| o.material.ext.variation_scale = 101.0));
        let light = |l: CalibLight| validate(&Calibration { objects: ok.objects.clone(), lights: vec![l] }).is_err();
        assert!(light(CalibLight { candela: 0.0, ..Default::default() }));
        assert!(light(CalibLight { kelvin: 500.0, ..Default::default() }));
        assert!(light(CalibLight { at: [f64::INFINITY, 0.0, 0.0], ..Default::default() }));
        assert!(scene(&many).meshes.is_empty());
    }

    #[test]
    fn lights_carry_candela_and_kelvin_colour() {
        let c = Calibration {
            objects: vec![CalibObject::default()],
            lights: vec![CalibLight { at: [1.0, 2.0, 3.0], candela: 250.0, kelvin: 2700.0 }, CalibLight::default()],
        };
        let s = scene(&c);
        assert_eq!(s.lights.len(), 2);
        let l = &s.lights[0];
        assert_eq!(l.kind, LightKind::Point);
        assert_eq!(l.intensity, 250.0);
        assert_eq!(l.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(l.color, cadcraft_arch::light::kelvin_to_rgb(2700.0));
        assert!(l.color[0] > l.color[2]);
        assert_eq!(s.lights[1].color, cadcraft_arch::light::kelvin_to_rgb(6500.0));
    }

    #[test]
    fn sphere_with_minimum_segments_is_closed() {
        let o = CalibObject { size: [2.0, 2.0, 2.0], segments: 8, ..Default::default() };
        let s = scene(&one(o));
        let m = &s.meshes[0];
        // Every vertex lies on the unit sphere.
        for p in &m.positions {
            let d = (p[0].powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt();
            assert!((d - 1.0).abs() < 1e-4, "vertex off radius: {d}");
        }
        // The mesh is closed: every directed edge appears exactly once.
        let mut edges: BTreeMap<([i64; 3], [i64; 3]), i32> = BTreeMap::new();
        for t in m.indices.chunks(3) {
            let v: Vec<[f32; 3]> = t.iter().map(|i| m.positions[*i as usize]).collect();
            for k in 0..3 {
                *edges.entry((key(v[k]), key(v[(k + 1) % 3]))).or_default() += 1;
            }
        }
        for ((a, b), n) in &edges {
            assert_eq!(*n, 1, "edge {a:?}->{b:?} appears {n} times");
            assert_eq!(edges.get(&(*b, *a)), Some(&1), "missing reverse edge");
        }
    }

    #[test]
    fn box_with_thin_dimension_has_correct_bounds() {
        let o = CalibObject { shape: CalibShape::Box, at: [0.0, 0.0, 0.0], size: [2.0, 0.01, 2.0], ..Default::default() };
        let s = scene(&one(o));
        let b = &s.bounds;
        assert!((b.min[0] - (-1.0)).abs() < 1e-9);
        assert!((b.min[1] - (-0.005)).abs() < 1e-9);
        assert!((b.min[2] - (-1.0)).abs() < 1e-9);
        assert!((b.max[0] - 1.0).abs() < 1e-9);
        assert!((b.max[1] - 0.005).abs() < 1e-9);
        assert!((b.max[2] - 1.0).abs() < 1e-9);
        assert_eq!(s.meshes[0].indices.len(), 36);
    }

    #[test]
    fn validate_box_size_on_all_axes() {
        let bad = |f: &dyn Fn(&mut CalibObject)| {
            let mut o = CalibObject { shape: CalibShape::Box, ..Default::default() };
            f(&mut o);
            validate(&one(o)).is_err()
        };
        assert!(bad(&|o| o.size[0] = 0.0));
        assert!(bad(&|o| o.size[1] = 0.0));
        assert!(bad(&|o| o.size[2] = 0.0));
        assert!(bad(&|o| o.size[1] = f64::NAN));
        assert!(bad(&|o| o.size[2] = f64::NAN));
        assert!(bad(&|o| o.size[1] = MAX_SIZE + 1.0));
        // A sphere only validates size[0]; size[1] and size[2] are ignored.
        let mut sphere = CalibObject::default();
        sphere.size = [1.0, 0.0, 0.0];
        assert!(validate(&one(sphere)).is_ok());
    }

    #[test]
    fn empty_lights_are_valid() {
        let c = Calibration { objects: vec![CalibObject::default()], lights: vec![] };
        assert!(validate(&c).is_ok());
        let s = scene(&c);
        assert!(s.lights.is_empty());
    }

    #[test]
    fn bounds_encompass_multiple_objects() {
        let a = CalibObject { shape: CalibShape::Box, at: [-2.0, 0.0, 0.0], size: [1.0, 1.0, 1.0], ..Default::default() };
        let b = CalibObject { shape: CalibShape::Box, at: [2.0, 0.0, 0.0], size: [1.0, 1.0, 1.0], ..Default::default() };
        let s = scene(&Calibration { objects: vec![a, b], lights: vec![] });
        assert!((s.bounds.min[0] - (-2.5)).abs() < 1e-9);
        assert!((s.bounds.max[0] - 2.5).abs() < 1e-9);
        assert!((s.bounds.min[1] - (-0.5)).abs() < 1e-9);
        assert!((s.bounds.max[1] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn light_at_kelvin_boundaries() {
        let low = CalibLight { kelvin: 1000.0, ..Default::default() };
        let high = CalibLight { kelvin: 20000.0, ..Default::default() };
        let base = vec![CalibObject::default()];
        assert!(validate(&Calibration { objects: base.clone(), lights: vec![low] }).is_ok());
        assert!(validate(&Calibration { objects: base.clone(), lights: vec![high] }).is_ok());
        // Just outside the range.
        let too_low = CalibLight { kelvin: 999.0, ..Default::default() };
        let too_high = CalibLight { kelvin: 20001.0, ..Default::default() };
        assert!(validate(&Calibration { objects: base.clone(), lights: vec![too_low] }).is_err());
        assert!(validate(&Calibration { objects: base, lights: vec![too_high] }).is_err());
    }
}
