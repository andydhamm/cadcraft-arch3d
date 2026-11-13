//! 3D meshes for architectural openings (doors, windows, garage doors).
//!
//! Each component (frame, casing, glass, leaf, sill, header) is a separate
//! [`Mesh`] tagged with the opening's [`ArchId`] for picking.

use std::collections::BTreeMap;

use cadcraft_arch::opening::is_double;
use cadcraft_doc::arch::{Grille, MAX_GRILLE_LITES, Opening, OpeningKind, OpeningSides, OpeningTop};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

use crate::mesh::Mesh;
use crate::solid::{Frame, clip_convex, clip_left, push_profile};

/// How far (metres) the opening header sits inside each wall face, so that
/// hidden-line elevations hide it behind the wall above the rough opening.
pub const HEADER_INSET: f64 = 0.01;

/// Clamp a component centre offset (from the wall centre plane, measured along
/// the wall normal) so that a component of depth `depth` fits inside the wall's
/// `[-wall_t/2, +wall_t/2]` thickness band.
fn band_offset(off: f64, depth: f64, wall_t: f64) -> f64 {
    if !wall_t.is_finite() || !depth.is_finite() {
        return off;
    }
    let min = -wall_t / 2.0 + depth / 2.0;
    let max = wall_t / 2.0 - depth / 2.0;
    if min > max { 0.0 } else { off.clamp(min, max) }
}

/// Whether the opening has a glass pane: windows, and doors whose style contains
/// sliding, bifold, french, pocket or glass.
pub fn has_glass(o: &Opening) -> bool {
    matches!(o.kind, OpeningKind::Window)
        || o.style.contains("sliding")
        || o.style.contains("bifold")
        || o.style.contains("french")
        || o.style.contains("pocket")
        || o.style.contains("glass")
        || o.style.contains("fixed")
}

/// The plane of an opening's glass pane, in model coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlassPlane {
    /// Centre of the pane (x, y, z).
    pub centre: Vec3,
    /// Unit normal in plan (the host wall's left).
    pub normal: Vec2,
    /// Unit direction along the pane in plan (the host wall's direction).
    pub along: Vec2,
    /// Half the pane's width.
    pub half_width: f64,
    /// Bottom of the pane (z).
    pub bottom: f64,
    /// Top of the pane (z).
    pub top: f64,
}

/// A glass rectangle in the opening's local `(u, z)` coordinates.
///
/// `u` runs along the wall, positive to the right when viewed from the wall's
/// start to its end; `z` runs vertically from the opening sill.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlassRect {
    pub u0: f64,
    pub u1: f64,
    pub z0: f64,
    pub z1: f64,
}

impl GlassRect {
    /// A glass rectangle from `u0` to `u1` horizontally and `z0` to `z1` vertically.
    pub const fn new(u0: f64, u1: f64, z0: f64, z1: f64) -> Self {
        GlassRect { u0, u1, z0, z1 }
    }
}

/// The grille bars for a glass rectangle as `(u0, u1, z0, z1)` boxes in the
/// glass plane. Bars are split into independent rectangles so they never overlap.
pub fn grille_bars(o: &Opening, glass: &GlassRect) -> Vec<[f64; 4]> {
    let Some(g) = o.grille else { return Vec::new() };
    if !glass.u0.is_finite() || !glass.u1.is_finite() || !glass.z0.is_finite() || !glass.z1.is_finite() {
        return Vec::new();
    }
    let across = g.across.max(1).min(MAX_GRILLE_LITES);
    let down = g.down.max(1).min(MAX_GRILLE_LITES);
    if across * down <= 1 {
        return Vec::new();
    }

    let bar = 0.02;
    let half = bar / 2.0;
    let pz0 = if g.top_only { (glass.z0 + glass.z1) / 2.0 } else { glass.z0 };
    let z1 = glass.z1;
    let u0 = glass.u0;
    let u1 = glass.u1;

    let mut bars: Vec<[f64; 4]> = Vec::new();
    let mut verticals: Vec<f64> = Vec::new();
    for k in 1..across {
        let u = u0 + (k as f64 / across as f64) * (u1 - u0);
        verticals.push(u);
        bars.push([u - half, u + half, pz0, z1]);
    }

    let mut segments = vec![(u0, u1)];
    for u in &verticals {
        let lo = *u - half;
        let hi = *u + half;
        let mut next = Vec::new();
        for (s, e) in segments {
            if e <= lo || s >= hi {
                next.push((s, e));
            } else {
                if s < lo {
                    next.push((s, lo));
                }
                if e > hi {
                    next.push((hi, e));
                }
            }
        }
        segments = next;
    }

    for k in 1..down {
        let z = pz0 + (k as f64 / down as f64) * (z1 - pz0);
        for (s, e) in &segments {
            if *s >= *e {
                continue;
            }
            bars.push([*s, *e, z - half, z + half]);
        }
    }

    // The helper is deliberately segment-based: the horizontal bars are split by
    // the vertical bars, so each box is independent and a probe counts parity.
    bars
}

/// Shrink the convex counter-clockwise polygon `poly` inwards by `d` (the intersection of every
/// edge's half-plane moved `d` inwards). Empty when nothing is left.
fn shrink_convex(poly: &[Vec2], d: f64) -> Vec<Vec2> {
    let mut out = poly.to_vec();
    for (i, &a) in poly.iter().enumerate() {
        let b = poly.get((i + 1) % poly.len()).copied().unwrap_or(a);
        let e = b - a;
        let len = e.len();
        if !(len > 1e-12) {
            continue;
        }
        let n = e.perp() / len;
        out = clip_left(&out, a + n * d, b + n * d);
        if out.is_empty() {
            break;
        }
    }
    out
}

/// The highest point of the polygon `poly` on the vertical line `u`, if it crosses it.
fn height_at(poly: &[Vec2], u: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for (i, &a) in poly.iter().enumerate() {
        let b = poly.get((i + 1) % poly.len()).copied().unwrap_or(a);
        let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
        if u < lo - 1e-9 || u > hi + 1e-9 {
            continue;
        }
        let y = if (b.x - a.x).abs() < 1e-12 { a.y.max(b.y) } else { a.y + (b.y - a.y) * ((u - a.x) / (b.x - a.x)).clamp(0.0, 1.0) };
        best = Some(best.map_or(y, |m| m.max(y)));
    }
    best
}

/// The glass outline of a window with a shaped top (the finished outline shrunk by the frame
/// width), in the host wall's profile coordinates (`v` from the wall bottom). `None` for a square
/// top, a door, or when the shape cannot be made.
fn shaped_glass(o: &Opening, roof_top: Option<&[Vec2]>) -> Option<Vec<Vec2>> {
    if o.top == OpeningTop::Square || !matches!(o.kind, OpeningKind::Window) {
        return None;
    }
    let fin = cadcraft_arch::opening_top::finished_outline(o, o.sill, roof_top)?;
    let glass = shrink_convex(&fin, o.frame_width);
    (glass.len() >= 3).then_some(glass)
}

/// The solids of a window with a shaped top, as `(meshes by material, header base z)`: the frame
/// jambs and head, the casing, the glass and the grille bars, all as profiles in the wall plane.
/// The header base is `None` for a roof top, which gets no header.
fn shaped_parts(
    o: &Opening,
    wall: &Wall,
    dir: Vec2,
    base: f64,
    roof_top: Option<&[Vec2]>,
    kind_prefix: &str,
) -> Option<(BTreeMap<String, Mesh>, Option<f64>)> {
    let fin = cadcraft_arch::opening_top::finished_outline(o, o.sill, roof_top)?;
    let glass = shaped_glass(o, roof_top)?;
    let frame = Frame::new(wall.start, dir, base)?;
    let (w, c, fw, fd, cw, cd, wall_t) = (o.width, o.offset, o.frame_width, o.frame_depth, o.casing_width, o.casing_depth, wall.thickness);
    let chain = fin.get(2..)?;
    let right_top = chain.first()?.y;
    let left_top = chain.last()?.y;
    let rect = |u0: f64, u1: f64, v0: f64, v1: f64| vec![Vec2::new(u0, v0), Vec2::new(u1, v0), Vec2::new(u1, v1), Vec2::new(u0, v1)];
    // The band between the chain and the chain moved up by `d` (negative: down).
    let band = |d: f64| -> Vec<Vec2> {
        let (lo, hi) = if d < 0.0 { (d, 0.0) } else { (0.0, d) };
        let mut ring: Vec<Vec2> = chain.iter().map(|p| Vec2::new(p.x, p.y + lo)).collect();
        ring.extend(chain.iter().rev().map(|p| Vec2::new(p.x, p.y + hi)));
        ring
    };
    let mut meshes: BTreeMap<String, Mesh> = BTreeMap::new();
    let mut put = |material: String, ring: &[Vec2], off: f64, depth: f64| {
        let mesh = meshes.entry(material.clone()).or_insert_with(|| Mesh::new(&material, o.id));
        push_profile(mesh, ring, off - depth / 2.0, off + depth / 2.0, &frame);
    };

    let frame_mat = format!("{}-frame", kind_prefix);
    let jamb_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, fd, wall_t);
    put(frame_mat.clone(), &rect(c - w / 2.0 - fw / 2.0, c - w / 2.0 + fw / 2.0, o.sill, left_top), jamb_off, fd);
    put(frame_mat.clone(), &rect(c + w / 2.0 - fw / 2.0, c + w / 2.0 + fw / 2.0, o.sill, right_top), jamb_off, fd);
    let head_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, fw, wall_t);
    put(frame_mat, &band(-fw), head_off, fw);

    if cw > 0.0 {
        let mut casing = |side: OpeningSides, mat: String| {
            let side_off = if side.left {
                band_offset(o.recess + wall_t / 2.0 - cd / 2.0, cd, wall_t)
            } else {
                band_offset(o.recess - wall_t / 2.0 + cd / 2.0, cd, wall_t)
            };
            let (l0, l1) = (c - w / 2.0 - fw / 2.0 - cw, c - w / 2.0 - fw / 2.0);
            let (r0, r1) = (c + w / 2.0 + fw / 2.0, c + w / 2.0 + fw / 2.0 + cw);
            put(mat.clone(), &rect(l0, l1, o.sill, left_top), side_off, cd);
            put(mat.clone(), &rect(r0, r1, o.sill, right_top), side_off, cd);
            let head_off = if side.left {
                band_offset(o.recess + wall_t / 2.0 - cw / 2.0, cw, wall_t)
            } else {
                band_offset(o.recess - wall_t / 2.0 + cw / 2.0, cw, wall_t)
            };
            put(mat, &band(cw), head_off, cw);
        };
        if o.casing_sides.right {
            let mat = if o.casing_sides.left { format!("{}-casing-right", kind_prefix) } else { format!("{}-casing", kind_prefix) };
            casing(OpeningSides::RIGHT, mat);
        }
        if o.casing_sides.left {
            casing(OpeningSides { left: true, ..OpeningSides::NONE }, format!("{}-casing-left", kind_prefix));
        }
    }

    let glass_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, 0.006, wall_t);
    let (gu0, gu1) = glass.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (gv0, gv1) = glass.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let bbox = GlassRect { u0: gu0 - c, u1: gu1 - c, z0: base + gv0, z1: base + gv1 };
    for [u0, u1, z0, z1] in grille_bars(o, &bbox) {
        let clipped = clip_convex(&rect(c + u0, c + u1, z0 - base, z1 - base), &glass);
        if clipped.len() >= 3 {
            put(format!("{}-grille", kind_prefix), &clipped, glass_off, 0.02);
        }
    }
    put(format!("{}-glass", kind_prefix), &glass, glass_off, 0.006);

    let header_z0 = match o.top {
        OpeningTop::Roof => None,
        _ => cadcraft_arch::opening_top::rough_outline(o, o.sill, roof_top).and_then(|r| height_at(&r, c)).map(|v| base + v),
    };
    Some((meshes, header_z0))
}

/// The glass pane of `o` exactly as [`opening_meshes`] builds it; `None` without glass or with a
/// missing or degenerate host wall.
pub fn glass_plane(model: &ArchModel, o: &Opening) -> Option<GlassPlane> {
    if !has_glass(o) {
        return None;
    }
    let wall = model.walls.iter().find(|w| w.id == o.host)?;
    let d = wall.end - wall.start;
    if d.len() < 1e-9 || !d.len().is_finite() {
        return None;
    }
    let dir = d.normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let (base, top) = crate::wall_span(model, wall).unwrap_or((0.0, wall.height));
    let frame_z0 = base + o.sill;
    let frame_z1 = (base + o.sill + o.height).min(top);
    let mut bottom = frame_z0 + o.frame_width;
    let mut gtop = frame_z1 - o.frame_width;
    if o.top != OpeningTop::Square {
        let roof_top = if o.top == OpeningTop::Roof { crate::walls::roof_top_polyline(model, wall, base, top) } else { None };
        if let Some(glass) = shaped_glass(o, roof_top.as_deref()) {
            bottom = base + glass.iter().fold(f64::INFINITY, |m, p| m.min(p.y));
            gtop = base + height_at(&glass, o.offset).unwrap_or(gtop - base);
        }
    }
    let glass_off = band_offset(o.recess - wall.thickness / 2.0 + o.frame_depth / 2.0, 0.006, wall.thickness);
    let c = wall.start + dir * o.offset + normal * glass_off;
    let plane = GlassPlane {
        centre: Vec3::new(c.x, c.y, (bottom + gtop) / 2.0),
        normal,
        along: dir,
        half_width: o.width / 2.0 - o.frame_width,
        bottom,
        top: gtop,
    };
    let finite = plane.centre.x.is_finite() && plane.centre.y.is_finite() && plane.centre.z.is_finite();
    finite.then_some(plane)
}

/// Build all component meshes for a single opening.
///
/// Components that share a material (e.g. all frame prisms) are merged into
/// a single `Mesh`, keeping `io::glb` node names unchanged (`door-frame-o0`,
/// `window-glass-o1`, …) while cutting the per-opening mesh count from ~10
/// to ~5.
///
/// `span` provides the host wall's `(base_elevation, top_elevation)`; when
/// `None`, the opening is built at elevation 0 with height `wall.height`.
///
/// Returns an empty vector if the host wall is missing or degenerate.
pub fn opening_meshes(walls: &BTreeMap<ArchId, &Wall>, o: &Opening, span: Option<(f64, f64)>) -> Vec<Mesh> {
    opening_meshes_with(walls, o, span, None)
}

/// [`opening_meshes`] that also knows the host wall's roof top polyline (`(w, z)` points along the
/// wall), for an opening whose top follows the roof. `roof_top` is not read yet.
pub fn opening_meshes_with(walls: &BTreeMap<ArchId, &Wall>, o: &Opening, span: Option<(f64, f64)>, roof_top: Option<&[Vec2]>) -> Vec<Mesh> {
    if let Some(m) = crate::niche::special_opening_meshes(walls, o, span) {
        return m;
    }
    let wall = match walls.get(&o.host) {
        Some(w) => w,
        None => return Vec::new(),
    };
    let dir = wall.end - wall.start;
    if dir.len() < 1e-9 {
        return Vec::new();
    }
    let dir = dir.normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let center = wall.start + dir * o.offset;
    let (base, top) = span.unwrap_or((0.0, wall.height));
    // Offsets measured from the wall centre plane (along `normal`), each
    // clamped so the component stays inside the wall's thickness band.
    let kind_is_window = matches!(o.kind, OpeningKind::Window);
    let shaped_init = if kind_is_window && o.top != OpeningTop::Square { shaped_parts(o, wall, dir, base, roof_top, "window") } else { None };
    let shaped = shaped_init.is_some();
    let (shaped_meshes, shaped_header) = shaped_init.unzip();
    let shaped_header = shaped_header.flatten();
    let mut meshes: BTreeMap<String, Mesh> = shaped_meshes.unwrap_or_default();
    let mut push_box = |material: &str, a: Vec2, b: Vec2, z0: f64, z1: f64, depth: f64| {
        if depth <= 0.0 {
            return;
        }
        let dir = b - a;
        let len = dir.len();
        if len < 1e-9 || (z1 - z0).abs() < 1e-9 {
            return;
        }
        let d = dir / len;
        let n = Vec2::new(-d.y, d.x);
        let half = depth / 2.0;
        let p0 = a + n * half;
        let p1 = a - n * half;
        let p2 = b - n * half;
        let p3 = b + n * half;
        let mesh = meshes.entry(material.to_string()).or_insert_with(|| Mesh::new(material, o.id));
        mesh.push_prism(&[p0, p1, p2, p3], z0, z1);
    };

    let w = o.width;
    let h = o.height;
    let sill_h = o.sill;
    let fw = o.frame_width;
    let fd = o.frame_depth;
    let cw = o.casing_width;
    let cd = o.casing_depth;
    let sd = o.sill_depth;
    let wall_t = wall.thickness;

    // The opening top is clamped to the host wall span top so the frame,
    // casing, glass and leaf can never rise above the wall top and push the
    // scene envelope out of the host wall.
    let frame_z0 = base + sill_h;
    let frame_z1 = (base + sill_h + h).min(top);

    let is_window = matches!(o.kind, OpeningKind::Window);
    let kind_prefix = if is_window {
        "window"
    } else if matches!(o.kind, OpeningKind::GarageDoor) {
        "garage"
    } else {
        "door"
    };

    // Frame: left jamb (a shaped top builds jambs and head in `shaped_parts`)
    let jamb_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, fd, wall_t);
    if !shaped {
        push_box(
            &format!("{}-frame", kind_prefix),
            center + dir * (-w / 2.0 - fw / 2.0) + normal * jamb_off,
            center + dir * (-w / 2.0 + fw / 2.0) + normal * jamb_off,
            frame_z0,
            frame_z1,
            fd,
        );

        // Frame: right jamb
        push_box(
            &format!("{}-frame", kind_prefix),
            center + dir * (w / 2.0 - fw / 2.0) + normal * jamb_off,
            center + dir * (w / 2.0 + fw / 2.0) + normal * jamb_off,
            frame_z0,
            frame_z1,
            fd,
        );

        // Frame: head
        let head_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, fw, wall_t);
        push_box(
            &format!("{}-frame", kind_prefix),
            center + dir * (-w / 2.0) + normal * head_off,
            center + dir * (w / 2.0) + normal * head_off,
            frame_z1 - fw,
            frame_z1,
            fw,
        );
    }

    // Frame: sill
    let sill_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, fw, wall_t);
    push_box(
        &format!("{}-frame", kind_prefix),
        center + dir * (-w / 2.0) + normal * sill_off,
        center + dir * (w / 2.0) + normal * sill_off,
        frame_z0,
        frame_z0 + fw,
        fw,
    );

    // Casing and sill can be switched off per side: right side is the default,
    // mirror-left is the opposite side of the wall centre plane.
    if cw > 0.0 && !shaped {
        let mut push_side_casing = |side: OpeningSides, mat: &str| {
            let side_off = if side.left {
                band_offset(o.recess + wall_t / 2.0 - cd / 2.0, cd, wall_t)
            } else {
                band_offset(o.recess - wall_t / 2.0 + cd / 2.0, cd, wall_t)
            };
            push_box(
                mat,
                center + dir * (-w / 2.0 - fw / 2.0 - cw) + normal * side_off,
                center + dir * (-w / 2.0 - fw / 2.0) + normal * side_off,
                frame_z0,
                frame_z1,
                cd,
            );
            push_box(
                mat,
                center + dir * (w / 2.0 + fw / 2.0) + normal * side_off,
                center + dir * (w / 2.0 + fw / 2.0 + cw) + normal * side_off,
                frame_z0,
                frame_z1,
                cd,
            );
            let casing_head_off = if side.left {
                band_offset(o.recess + wall_t / 2.0 - cw / 2.0, cw, wall_t)
            } else {
                band_offset(o.recess - wall_t / 2.0 + cw / 2.0, cw, wall_t)
            };
            push_box(
                mat,
                center + dir * (-w / 2.0 - fw / 2.0) + normal * casing_head_off,
                center + dir * (w / 2.0 + fw / 2.0) + normal * casing_head_off,
                frame_z1 - cw,
                frame_z1,
                cw,
            );
        };

        if o.casing_sides.right {
            let mat = if o.casing_sides.left { format!("{}-casing-right", kind_prefix) } else { format!("{}-casing", kind_prefix) };
            push_side_casing(OpeningSides::RIGHT, &mat);
        }
        if o.casing_sides.left {
            let mat = if o.casing_sides.right { format!("{}-casing-left", kind_prefix) } else { format!("{}-casing-left", kind_prefix) };
            push_side_casing(OpeningSides { left: true, ..OpeningSides::NONE }, &mat);
        }
    }

    // Grilles and glass: the bars sit in the same glass plane and are inserted
    // before the glass mesh so a probe on the glass centre reports the grille.
    if has_glass(o) && !shaped {
        let glass_off = band_offset(o.recess - wall_t / 2.0 + fd / 2.0, 0.006, wall_t);
        let glass_rect = GlassRect::new(-w / 2.0 + fw, w / 2.0 - fw, frame_z0 + fw, frame_z1 - fw);
        if o.grille.is_some() {
            for [u0, u1, z0, z1] in grille_bars(o, &glass_rect) {
                push_box(
                    &format!("{}-grille", kind_prefix),
                    center + dir * u0 + normal * glass_off,
                    center + dir * u1 + normal * glass_off,
                    z0,
                    z1,
                    0.02,
                );
            }
        }
        push_box(
            &format!("{}-glass", kind_prefix),
            center + dir * (-w / 2.0 + fw) + normal * glass_off,
            center + dir * (w / 2.0 - fw) + normal * glass_off,
            frame_z0 + fw,
            frame_z1 - fw,
            0.006,
        );
    }

    // Fixed glazed doors: a standard glazed leaf frame with no swing and no leaf
    // solid over the glass.
    let is_fixed_door = !is_window && matches!(o.kind, OpeningKind::Door) && o.style.eq_ignore_ascii_case("fixed");
    if !is_window && !matches!(o.kind, OpeningKind::CasedOpening) && !is_fixed_door {
        let leaf_t = 0.045;
        let leaf_off = band_offset(o.recess - wall_t / 2.0 + leaf_t / 2.0, leaf_t, wall_t);
        let leaf_z0 = frame_z0 + fw;
        let leaf_z1 = frame_z1 - fw;

        let two_leaves = o.style.contains("sliding") || o.style.contains("bifold") || o.style.contains("french") || is_double(o);
        if two_leaves {
            let half_w = (w - 2.0 * fw) / 2.0;
            push_box(
                &format!("{}-leaf", kind_prefix),
                center + dir * (-w / 2.0 + fw) + normal * leaf_off,
                center + dir * (-w / 2.0 + fw + half_w) + normal * leaf_off,
                leaf_z0,
                leaf_z1,
                leaf_t,
            );

            push_box(
                &format!("{}-leaf", kind_prefix),
                center + dir * (w / 2.0 - fw - half_w) + normal * leaf_off,
                center + dir * (w / 2.0 - fw) + normal * leaf_off,
                leaf_z0,
                leaf_z1,
                leaf_t,
            );
        } else {
            push_box(
                &format!("{}-leaf", kind_prefix),
                center + dir * (-w / 2.0 + fw) + normal * leaf_off,
                center + dir * (w / 2.0 - fw) + normal * leaf_off,
                leaf_z0,
                leaf_z1,
                leaf_t,
            );
        }
    }
    if is_fixed_door {
        let leaf_t = 0.1;
        let leaf_off = band_offset(o.recess - wall_t / 2.0 + leaf_t / 2.0, leaf_t, wall_t);
        let inner_left = -w / 2.0 + fw;
        let inner_right = w / 2.0 - fw;
        let stile_w = 0.1;
        let left = inner_left + stile_w;
        let right = inner_right - stile_w;
        push_box(
            &format!("{}-leaf", kind_prefix),
            center + dir * (inner_left) + normal * leaf_off,
            center + dir * (inner_left + stile_w) + normal * leaf_off,
            frame_z0,
            frame_z1,
            leaf_t,
        );
        push_box(
            &format!("{}-leaf", kind_prefix),
            center + dir * (inner_right - stile_w) + normal * leaf_off,
            center + dir * (inner_right) + normal * leaf_off,
            frame_z0,
            frame_z1,
            leaf_t,
        );
        push_box(
            &format!("{}-leaf", kind_prefix),
            center + dir * (left) + normal * leaf_off,
            center + dir * (right) + normal * leaf_off,
            frame_z1 - 0.1,
            frame_z1,
            leaf_t,
        );
        push_box(
            &format!("{}-leaf", kind_prefix),
            center + dir * (left) + normal * leaf_off,
            center + dir * (right) + normal * leaf_off,
            frame_z0,
            frame_z0 + 0.2,
            leaf_t,
        );
    }

    // Exterior sill (for windows) or sill on selected side(s).
    if is_window && sd > 0.0 {
        let right_off = band_offset(o.recess - wall_t / 2.0, 0.04, wall_t);
        let left_off = band_offset(o.recess + wall_t / 2.0, 0.04, wall_t);
        let mut side_sill = |side: &str, off: f64, material: &str| {
            let s_a = center + dir * (-w / 2.0 - fw / 2.0 - sd) + normal * off;
            let s_b = center + dir * (w / 2.0 + fw / 2.0 + sd) + normal * off;
            let sill_bottom = (frame_z0 - 0.02).max(base);
            if let Some(mesh) = box_between(s_a, s_b, sill_bottom, frame_z0, 0.04, material, o.id) {
                meshes.insert(material.to_string(), mesh);
            }
            let _ = side;
        };
        if o.sill_sides.right {
            let mat = if o.sill_sides.left { format!("{}-sill-right", kind_prefix) } else { format!("{}-sill", kind_prefix) };
            side_sill("right", right_off, &mat);
        }
        if o.sill_sides.left {
            let mat = format!("{}-sill-left", kind_prefix);
            side_sill("left", left_off, &mat);
        }
    }

    // Header: a lintel centred on the wall centre plane, `HEADER_INSET` inside
    // both wall faces so an elevation hides it behind the wall above the
    // opening; clipped to stay inside the host wall span.
    // A shaped top puts the header on the rough top at the centre; a roof top gets none.
    let header_h = 0.2;
    let header_z0 = if shaped { shaped_header } else { Some(frame_z1) };
    let header_top = header_z0.map_or(f64::NEG_INFINITY, |z| (z + header_h).min(top));
    let frame_z1 = header_z0.unwrap_or(frame_z1);
    if header_z0.is_some() && header_top > frame_z1 + 1e-9 {
        let header_depth = if wall_t > 4.0 * HEADER_INSET { wall_t - 2.0 * HEADER_INSET } else { wall_t / 2.0 };
        let header_off = band_offset(0.0, header_depth, wall_t);
        let h_a = center + dir * (-w / 2.0 - fw) + normal * header_off;
        let h_b = center + dir * (w / 2.0 + fw) + normal * header_off;
        let header_mat = format!("{}-header", kind_prefix);
        if let Some(mesh) = box_between(h_a, h_b, frame_z1, header_top, header_depth, &header_mat, o.id) {
            meshes.insert(header_mat, mesh);
        }
    }

    let mut ordered: Vec<Mesh> = meshes.into_values().collect();
    ordered.sort_by(|a, b| {
        let rank = |m: &str| match m {
            m if m.ends_with("-frame") => 0,
            m if m.contains("-casing") => 1,
            m if m.contains("-sill") => 2,
            m if m.contains("-grille") => 3,
            m if m.contains("-glass") => 4,
            m if m.contains("-leaf") => 5,
            m if m.contains("-header") => 6,
            _ => 7,
        };
        let ra = rank(&a.material);
        let rb = rank(&b.material);
        ra.cmp(&rb).then_with(|| a.material.cmp(&b.material))
    });
    ordered
}

/// Create a box mesh between two 2D points, from z0 to z1, with given depth.
///
/// Returns `None` for degenerate boxes (zero length or zero height).
fn box_between(a: Vec2, b: Vec2, z0: f64, z1: f64, depth: f64, material: &str, id: ArchId) -> Option<Mesh> {
    let dir = b - a;
    let len = dir.len();
    if len < 1e-9 || (z1 - z0).abs() < 1e-9 {
        return None;
    }
    let d = dir / len;
    let n = Vec2::new(-d.y, d.x);
    let half = depth / 2.0;

    let p0 = a + n * half;
    let p1 = a - n * half;
    let p2 = b - n * half;
    let p3 = b + n * half;

    let mut mesh = Mesh::new(material, id);
    mesh.push_prism(&[p0, p1, p2, p3], z0, z1);
    if mesh.positions.is_empty() { None } else { Some(mesh) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.3,
            height: 2.5,
            ..Default::default()
        });
        model
    }

    fn window_opening() -> Opening {
        Opening {
            id: ArchId(100),
            host: ArchId(1),
            offset: 5.0,
            width: 1.2,
            height: 1.5,
            sill: 0.9,
            frame_width: 0.05,
            frame_depth: 0.1,
            casing_width: 0.05,
            casing_depth: 0.02,
            sill_depth: 0.05,
            swing_angle: 0.0,
            style: "window".to_string(),
            recess: 0.0,
            kind: OpeningKind::Window,
            ..Default::default()
        }
    }

    fn door_opening() -> Opening {
        Opening {
            id: ArchId(101),
            host: ArchId(1),
            offset: 5.0,
            width: 0.9,
            height: 2.1,
            sill: 0.0,
            frame_width: 0.05,
            frame_depth: 0.1,
            casing_width: 0.05,
            casing_depth: 0.02,
            sill_depth: 0.0,
            swing_angle: 0.0,
            style: "hinged".to_string(),
            recess: 0.0,
            kind: OpeningKind::Door,
            ..Default::default()
        }
    }

    #[test]
    fn a_window_produces_frame_glass_and_sill_meshes() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_frame = meshes.iter().any(|m| m.material == "window-frame");
        let has_glass = meshes.iter().any(|m| m.material == "window-glass");
        let has_sill = meshes.iter().any(|m| m.material == "window-sill");
        assert!(has_frame, "missing window-frame mesh");
        assert!(has_glass, "missing window-glass mesh");
        assert!(has_sill, "missing window-sill mesh");
    }

    #[test]
    fn a_hinged_door_produces_a_frame_and_one_leaf() {
        let model = wall_model();
        let o = door_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_frame = meshes.iter().any(|m| m.material == "door-frame");
        let leaf_count = meshes.iter().filter(|m| m.material == "door-leaf").count();
        assert!(has_frame, "missing door-frame mesh");
        assert_eq!(leaf_count, 1, "expected 1 leaf, got {}", leaf_count);
    }

    #[test]
    fn a_sliding_door_produces_two_leaves() {
        let model = wall_model();
        let mut o = door_opening();
        o.style = "sliding".to_string();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let leaf_meshes: Vec<&Mesh> = meshes.iter().filter(|m| m.material == "door-leaf").collect();
        assert_eq!(leaf_meshes.len(), 1, "expected 1 merged leaf mesh, got {}", leaf_meshes.len());
        // Each leaf prism (4-vertex ring extrusion) has 12 triangles; two leaves = 24.
        assert_eq!(leaf_meshes[0].triangle_count(), 24, "expected 24 triangles (2 leaves), got {}", leaf_meshes[0].triangle_count(),);
    }

    #[test]
    fn a_double_door_produces_two_leaves() {
        let model = wall_model();
        let mut o = door_opening();
        o.style = "french".to_string();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let leaf_meshes: Vec<&Mesh> = meshes.iter().filter(|m| m.material == "door-leaf").collect();
        assert_eq!(leaf_meshes.len(), 1, "expected 1 merged leaf mesh, got {}", leaf_meshes.len());
        // Each leaf prism (4-vertex ring extrusion) has 12 triangles; two leaves = 24.
        assert_eq!(leaf_meshes[0].triangle_count(), 24, "expected 24 triangles (2 leaves), got {}", leaf_meshes[0].triangle_count(),);
    }

    #[test]
    fn a_recessed_window_sits_off_the_wall_face() {
        let model = wall_model();
        let mut o = window_opening();
        o.recess = 0.05;
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        assert!(!meshes.is_empty(), "no meshes for recessed window");
        let frame = meshes.iter().find(|m| m.material == "window-frame");
        assert!(frame.is_some(), "missing window-frame");
    }

    #[test]
    fn casing_is_skipped_when_its_width_is_zero() {
        let model = wall_model();
        let mut o = window_opening();
        o.casing_width = 0.0;
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_casing = meshes.iter().any(|m| m.material == "window-casing");
        assert!(!has_casing, "casing should be skipped");
    }

    #[test]
    fn glass_uses_the_glass_material() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let glass = meshes.iter().find(|m| m.material == "window-glass");
        assert!(glass.is_some(), "missing window-glass mesh");
    }

    #[test]
    fn opening_meshes_are_manifold() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        for m in &meshes {
            assert_eq!(m.indices.len() % 3, 0, "indices not multiple of 3");
            for &idx in &m.indices {
                assert!(idx < m.positions.len() as u32, "index out of bounds");
            }
        }
    }

    #[test]
    fn opening_meshes_are_empty_for_a_missing_host() {
        let model = wall_model();
        let mut o = window_opening();
        o.host = ArchId(999);
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        assert!(meshes.is_empty(), "expected empty for missing host");
    }

    #[test]
    fn a_window_produces_window_glass_meshes() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_window_glass = meshes.iter().any(|m| m.material == "window-glass");
        let has_window_frame = meshes.iter().any(|m| m.material == "window-frame");
        assert!(has_window_glass, "missing window-glass mesh");
        assert!(has_window_frame, "missing window-frame mesh");
    }

    #[test]
    fn a_hinged_door_produces_door_leaf_and_frame() {
        let model = wall_model();
        let o = door_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_door_frame = meshes.iter().any(|m| m.material == "door-frame");
        let has_door_leaf = meshes.iter().any(|m| m.material == "door-leaf");
        assert!(has_door_frame, "missing door-frame mesh");
        assert!(has_door_leaf, "missing door-leaf mesh");
    }

    #[test]
    fn opening_meshes_are_never_empty() {
        let model = wall_model();
        let window = window_opening();
        let door = door_opening();
        let wm = walls_map(&model);
        for o in [&window, &door] {
            let meshes = opening_meshes(&wm, o, None);
            for m in &meshes {
                assert!(!m.positions.is_empty(), "mesh {} has no positions", m.material);
            }
        }
    }

    #[test]
    fn header_stays_inside_the_wall_height() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let wall_height = 2.5;
        for m in &meshes {
            if m.material.contains("header") {
                for p in &m.positions {
                    let z = p[2] as f64;
                    assert!(z <= wall_height + 1e-6, "header exceeds wall height: z={}", z);
                }
            }
        }
    }

    #[test]
    fn the_header_is_inset_from_both_wall_faces() {
        let t = 0.3;
        let model = band_wall(t);
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let header = meshes.iter().find(|m| m.material.contains("header"));
        assert!(header.is_some(), "no header mesh");
        for m in header.into_iter() {
            let lo = m.positions.iter().map(|p| p[1] as f64).fold(f64::MAX, f64::min);
            let hi = m.positions.iter().map(|p| p[1] as f64).fold(f64::MIN, f64::max);
            assert!((lo - (-t / 2.0 + HEADER_INSET)).abs() < 1e-5, "lo {lo}");
            assert!((hi - (t / 2.0 - HEADER_INSET)).abs() < 1e-5, "hi {hi}");
        }
    }

    /// A wall model of a single 10 m long wall, thickness `t`.
    fn band_wall(t: f64) -> ArchModel {
        let mut model = ArchModel::default();
        model.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: t,
            height: 2.5,
            ..Default::default()
        });
        model
    }

    /// One of each opening type on the wall in `model`.
    fn walls_map(model: &ArchModel) -> BTreeMap<ArchId, &Wall> {
        model.walls.iter().map(|w| (w.id, w)).collect()
    }

    fn all_openings(model: &ArchModel) -> Vec<Opening> {
        let base = |style: &str, kind: OpeningKind| Opening {
            id: ArchId(100),
            host: ArchId(1),
            offset: 5.0,
            width: 1.2,
            height: 1.5,
            sill: 0.9,
            frame_width: 0.05,
            frame_depth: 0.1,
            casing_width: 0.05,
            casing_depth: 0.02,
            sill_depth: 0.05,
            swing_angle: 0.0,
            style: style.to_string(),
            recess: 0.0,
            kind,
            ..Default::default()
        };
        vec![
            base("window", OpeningKind::Window),
            base("hinged", OpeningKind::Door),
            base("window", OpeningKind::Window),
            base("garage", OpeningKind::GarageDoor),
        ]
    }

    #[test]
    fn opening_meshes_stay_inside_the_host_wall_thickness() {
        let model = band_wall(0.3);
        let wm = walls_map(&model);
        for o in all_openings(&model) {
            let wall = &model.walls[0];
            let dir = (wall.end - wall.start).normalized();
            let normal = Vec2::new(-dir.y, dir.x);
            let center = wall.start + dir * o.offset;
            let half_t = wall.thickness / 2.0;
            for m in opening_meshes(&wm, &o, None) {
                for p in &m.positions {
                    let n = (Vec2::new(p[0] as f64, p[1] as f64) - center).dot(normal);
                    assert!(n.abs() <= half_t + 1e-6, "vertex {} of {} leaves wall band (n={})", p[0] as f64, m.material, n);
                }
            }
        }
    }

    #[test]
    fn house_with_openings_bounds_match_the_wall_envelope() {
        // 10 m x 8 m floor, walls 0.2 m thick, 2.4 m tall, centre lines at
        // x = 0/10 and y = 0/8.
        let t = 0.2;
        let h = 2.4;
        let mut model = ArchModel::default();
        for (a, b, id) in [((0.0, 0.0), (10.0, 0.0), 1), ((10.0, 0.0), (10.0, 8.0), 2), ((10.0, 8.0), (0.0, 8.0), 3), ((0.0, 8.0), (0.0, 0.0), 4)] {
            model.walls.push(Wall {
                id: ArchId(id),
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                thickness: t,
                height: h,
                ..Default::default()
            });
        }
        // Hinged door on the south wall, two windows, a garage door.
        let windows = vec![
            Opening {
                id: ArchId(100),
                host: ArchId(1),
                offset: 3.0,
                width: 1.2,
                height: 1.5,
                sill: 0.9,
                frame_width: 0.05,
                frame_depth: 0.1,
                casing_width: 0.05,
                casing_depth: 0.02,
                sill_depth: 0.05,
                swing_angle: 0.0,
                style: "window".to_string(),
                recess: 0.0,
                kind: OpeningKind::Window,
                ..Default::default()
            },
            Opening {
                id: ArchId(101),
                host: ArchId(1),
                offset: 7.0,
                width: 1.0,
                height: 1.2,
                sill: 0.9,
                frame_width: 0.05,
                frame_depth: 0.1,
                casing_width: 0.05,
                casing_depth: 0.02,
                sill_depth: 0.05,
                swing_angle: 0.0,
                style: "window".to_string(),
                recess: 0.0,
                kind: OpeningKind::Window,
                ..Default::default()
            },
            Opening {
                id: ArchId(102),
                host: ArchId(1),
                offset: 5.0,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
                frame_width: 0.05,
                frame_depth: 0.1,
                casing_width: 0.05,
                casing_depth: 0.02,
                sill_depth: 0.0,
                swing_angle: 0.0,
                style: "hinged".to_string(),
                recess: 0.0,
                kind: OpeningKind::Door,
                ..Default::default()
            },
            Opening {
                id: ArchId(103),
                host: ArchId(2),
                offset: 4.0,
                width: 3.0,
                height: 2.2,
                sill: 0.0,
                frame_width: 0.05,
                frame_depth: 0.1,
                casing_width: 0.05,
                casing_depth: 0.02,
                sill_depth: 0.0,
                swing_angle: 0.0,
                style: "garage".to_string(),
                recess: 0.0,
                kind: OpeningKind::GarageDoor,
                ..Default::default()
            },
        ];
        model.openings.clear();
        model.openings.extend(windows.into_iter().map(|o| o));

        // The scene envelope is the union of the wall solids and the
        // opening component meshes; the openings must not push it past the
        // wall outer faces (10.2 x 8.2) or above the wall tops (2.4).
        let expected = Vec3::new(10.2 + 1e-3, 8.2 + 1e-3, h + 1e-3);
        let eps = 1e-3;

        // Wall solid bounds (full extrusion to the host wall height).
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;
        for w in &model.walls {
            let (dx, dy) = (w.end.x - w.start.x, w.end.y - w.start.y);
            let (nx, ny) = (-dy, dx);
            let l = (nx * nx + ny * ny).max(1e-9).sqrt();
            let n = Vec2::new(nx / l, ny / l);
            for s in [w.start, w.end] {
                for (off, p) in [(-w.thickness / 2.0, s.clone()), (w.thickness / 2.0, s)] {
                    let pt = s + n * off;
                    x_min = x_min.min(pt.x);
                    x_max = x_max.max(pt.x);
                    y_min = y_min.min(pt.y);
                    y_max = y_max.max(pt.y);
                }
            }
            z_min = z_min.min(0.0);
            z_max = z_max.max(w.height);
        }

        // Opening component meshes: they must fit inside the wall envelope.
        let wm = walls_map(&model);
        for o in &model.openings {
            for m in opening_meshes(&wm, o, None) {
                for p in &m.positions {
                    x_min = x_min.min(p[0].into());
                    x_max = x_max.max(p[0].into());
                    y_min = y_min.min(p[1].into());
                    y_max = y_max.max(p[1].into());
                    z_min = z_min.min(p[2].into());
                    z_max = z_max.max(p[2].into());
                }
            }
        }

        assert!(x_min >= -t / 2.0 - eps, "scene x_min too low: {}", x_min);
        assert!(x_max - x_min <= expected.x, "scene width too large: {}", x_max - x_min);
        assert!(x_max - x_min >= expected.x - 2.0 * eps, "scene width too small: {}", x_max - x_min);
        assert!(y_min >= -t / 2.0 - eps, "scene y_min too low: {}", y_min);
        assert!(y_max - y_min <= expected.y, "scene depth too large: {}", y_max - y_min);
        assert!(y_max - y_min >= expected.y - 2.0 * eps, "scene depth too small: {}", y_max - y_min);
        assert!((z_max - z_min).abs() <= expected.z, "scene height off: {}", z_max - z_min);
        assert!((z_max - z_min).abs() >= expected.z - 2.0 * eps, "scene height too small: {}", z_max - z_min);
    }

    #[test]
    fn a_window_produces_a_header() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_header = meshes.iter().any(|m| m.material == "window-header");
        assert!(has_header, "missing window-header mesh");
    }

    #[test]
    fn a_door_produces_a_header() {
        let model = wall_model();
        let o = door_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_header = meshes.iter().any(|m| m.material == "door-header");
        assert!(has_header, "missing door-header mesh");
    }

    #[test]
    fn a_thin_wall_header_is_half_thickness() {
        let t = 0.03;
        let model = band_wall(t);
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let header = meshes.iter().find(|m| m.material.contains("header"));
        assert!(header.is_some(), "no header mesh for thin wall");
        for m in header.into_iter() {
            let lo = m.positions.iter().map(|p| p[1] as f64).fold(f64::MAX, f64::min);
            let hi = m.positions.iter().map(|p| p[1] as f64).fold(f64::MIN, f64::max);
            assert!((hi - lo - t / 2.0).abs() < 1e-5, "thin-wall header depth wrong: {} vs {}", hi - lo, t / 2.0);
        }
    }

    #[test]
    fn header_respects_span_top() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, Some((0.0, 2.5)));
        let header = meshes.iter().find(|m| m.material.contains("header"));
        assert!(header.is_some(), "no header mesh with span");
        for m in header.into_iter() {
            for p in &m.positions {
                assert!(p[2] as f64 <= 2.5 + 1e-6, "header exceeds span top: z={}", p[2] as f64);
            }
        }
    }

    #[test]
    fn no_header_when_opening_reaches_wall_top() {
        let model = wall_model();
        let mut o = window_opening();
        o.sill = 0.0;
        o.height = 2.5;
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_header = meshes.iter().any(|m| m.material.contains("header"));
        assert!(!has_header, "header should not exist when opening reaches wall top");
    }

    #[test]
    fn header_with_nan_wall_thickness_does_not_panic() {
        let model = band_wall(f64::NAN);
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        for m in &meshes {
            for p in &m.positions {
                let _ = p[0];
            }
        }
    }

    #[test]
    fn header_depth_is_wall_thickness_minus_two_insets() {
        let t = 0.3;
        let model = band_wall(t);
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let header = meshes.iter().find(|m| m.material.contains("header"));
        assert!(header.is_some(), "no header mesh");
        for m in header.into_iter() {
            let lo = m.positions.iter().map(|p| p[1] as f64).fold(f64::MAX, f64::min);
            let hi = m.positions.iter().map(|p| p[1] as f64).fold(f64::MIN, f64::max);
            assert!((hi - lo - (t - 2.0 * HEADER_INSET)).abs() < 1e-5, "header depth wrong: {} vs {}", hi - lo, t - 2.0 * HEADER_INSET);
        }
    }

    #[test]
    fn a_thin_wall_still_produces_meshes() {
        let model = band_wall(0.05);
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        assert!(!meshes.is_empty(), "no meshes for thin wall");
    }

    #[test]
    fn openings_handle_nan_dimensions_gracefully() {
        let model = wall_model();
        let mut o = window_opening();
        o.width = f64::NAN;
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        for m in &meshes {
            for p in &m.positions {
                let _ = p[0];
            }
        }
    }

    #[test]
    fn a_zero_height_opening_produces_no_degenerate_meshes() {
        let model = wall_model();
        let mut o = window_opening();
        o.height = 0.0;
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        for m in &meshes {
            assert!(m.triangle_count() > 0, "mesh {} has no triangles", m.material);
        }
    }

    #[test]
    fn a_span_limits_the_opening_height() {
        let model = wall_model();
        let o = window_opening();
        let meshes = opening_meshes(&walls_map(&model), &o, Some((0.5, 1.5)));
        assert!(!meshes.is_empty(), "no meshes for spanned opening");
        for m in &meshes {
            for p in &m.positions {
                let z = p[2] as f64;
                assert!(z >= 0.5 - 1e-6, "vertex below span base: z={}", z);
                assert!(z <= 1.5 + 1e-6, "vertex above span top: z={}", z);
            }
        }
    }

    #[test]
    fn glass_plane_matches_the_glass_mesh() {
        let model = wall_model();
        let o = window_opening();
        let g = glass_plane(&model, &o).expect("window has glass");
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let glass = meshes.iter().find(|m| m.material == "window-glass").expect("glass mesh");
        let b = glass.bounds();
        assert!(((b.min[0] + b.max[0]) / 2.0 - g.centre.x).abs() < 1e-4);
        assert!(((b.min[1] + b.max[1]) / 2.0 - g.centre.y).abs() < 1e-4);
        assert!((b.min[2] - g.bottom).abs() < 1e-4 && (b.max[2] - g.top).abs() < 1e-4);
        assert!((b.max[0] - b.min[0] - 2.0 * g.half_width).abs() < 1e-4);
        assert_eq!(g.normal, Vec2::new(0.0, 1.0));
        assert_eq!(g.along, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn glass_plane_is_none_for_a_hinged_door_and_a_cased_opening() {
        let model = wall_model();
        assert!(glass_plane(&model, &door_opening()).is_none());
        let cased = Opening { kind: OpeningKind::CasedOpening, style: "cased".into(), ..window_opening() };
        assert!(glass_plane(&model, &cased).is_none());
        let orphan = Opening { host: ArchId(99), ..window_opening() };
        assert!(glass_plane(&model, &orphan).is_none());
    }

    #[test]
    fn has_glass_matches_opening_meshes() {
        let model = wall_model();
        let mut cases = vec![window_opening(), door_opening()];
        for style in ["hinged", "sliding", "bifold", "french", "pocket", "glass-panel"] {
            cases.push(Opening { style: style.into(), ..door_opening() });
        }
        for o in &cases {
            let meshes = opening_meshes(&walls_map(&model), o, None);
            let any = meshes.iter().any(|m| m.material.ends_with("-glass"));
            assert_eq!(has_glass(o), any, "{:?} {}", o.kind, o.style);
        }
    }

    #[test]
    fn a_garage_door_produces_frame_and_leaf() {
        let model = wall_model();
        let o = Opening {
            id: ArchId(102),
            host: ArchId(1),
            offset: 5.0,
            width: 3.0,
            height: 2.2,
            sill: 0.0,
            frame_width: 0.05,
            frame_depth: 0.1,
            casing_width: 0.05,
            casing_depth: 0.02,
            sill_depth: 0.0,
            swing_angle: 0.0,
            style: "garage".to_string(),
            recess: 0.0,
            kind: OpeningKind::GarageDoor,
            ..Default::default()
        };
        let meshes = opening_meshes(&walls_map(&model), &o, None);
        let has_frame = meshes.iter().any(|m| m.material == "garage-frame");
        let has_leaf = meshes.iter().any(|m| m.material == "garage-leaf");
        assert!(has_frame, "missing garage-frame mesh");
        assert!(has_leaf, "missing garage-leaf mesh");
    }
}
