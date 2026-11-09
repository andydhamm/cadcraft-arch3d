//! The building pad and basement cuts. Filled in by `p9-arch-terrain-pad`.

use std::cmp::Ordering;

use cadcraft_doc::arch::{ArchModel, Level, LevelKind, OpeningKind, PAD_SLOPE};
use cadcraft_geom::{Vec2, point_in_polygon};

/// Whether a region is the pad under the first floor or a basement's cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKind {
    Pad,
    Basement,
}

/// A footprint the ground is flattened to.
#[derive(Clone, Debug, PartialEq)]
pub struct PadRegion {
    pub kind: PadKind,
    /// The footprint's outer ring (counter-clockwise).
    pub ring: Vec<Vec2>,
    /// The ground's elevation inside: the level's elevation minus its floor structure.
    pub elevation: f64,
    /// Flat apron beyond the ring (pad only; 0 for a basement).
    pub margin: f64,
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

fn level_first_floor(model: &ArchModel) -> Option<&Level> {
    let at_or_above_zero: Vec<&Level> = model.levels.iter().filter(|level| level.elevation.is_finite() && level.elevation >= 0.0).collect();
    if let Some(first) = at_or_above_zero.iter().min_by(|a, b| match a.elevation.partial_cmp(&b.elevation).unwrap_or(Ordering::Equal) {
        Ordering::Equal => a.index.cmp(&b.index),
        other => other,
    }) {
        // If the lowest level at or above 0 is within one floor height of 0, use it.
        // Otherwise (e.g. a split level with no landing at grade), use the lowest level overall.
        if first.elevation <= 1.0 {
            return Some(*first);
        }
    }
    model.levels.iter().filter(|level| level.elevation.is_finite()).min_by(|a, b| {
        match a.elevation.partial_cmp(&b.elevation).unwrap_or(Ordering::Equal) {
            Ordering::Equal => a.index.cmp(&b.index),
            other => other,
        }
    })
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let ap = p - a;
    let ab2 = ab.dot(ab);
    if ab2 <= 1e-18 {
        return p.dist(a);
    }
    let t = (ap.dot(ab) / ab2).clamp(0.0, 1.0);
    let q = a + ab * t;
    p.dist(q)
}

fn ring_distance(p: Vec2, ring: &[Vec2]) -> f64 {
    if ring.len() < 2 {
        return f64::INFINITY;
    }
    let mut best = f64::INFINITY;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let d = segment_distance(p, a, b);
        if d < best {
            best = d;
        }
    }
    best
}

fn inside_or_on_ring(p: Vec2, ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    point_in_polygon(ring, p) || ring_distance(p, ring) <= 1e-9
}

/// Depth of an entry apron from the wall's exterior face.
const APRON_DEPTH: f64 = 1.5;
/// Extra apron width beyond the door's own.
const APRON_EXTRA_WIDTH: f64 = 1.2;
/// One step: how far the apron lies below the landing.
const APRON_DROP: f64 = 0.15;
/// The flat margin around an apron.
const APRON_MARGIN: f64 = 0.6;
/// Differences thinner than this (mean width, metres) are slivers from coincident outer faces.
const MIN_SPLIT_WIDTH: f64 = 0.05;
const GEOM_EPS: f64 = 1e-7;

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Parameters in [0, 1] along `a`-`b` where it crosses or touches the ring's edges (0 and 1 included).
fn split_params(a: Vec2, b: Vec2, ring: &[Vec2]) -> Vec<f64> {
    let d = b - a;
    let len2 = d.dot(d);
    let mut ts = vec![0.0, 1.0];
    if len2 <= 1e-18 {
        return ts;
    }
    for i in 0..ring.len() {
        let (c, e) = (ring[i], ring[(i + 1) % ring.len()]);
        if segment_distance(c, a, b) <= GEOM_EPS {
            ts.push((c - a).dot(d) / len2);
        }
        let f = e - c;
        let denom = cross(d, f);
        if denom.abs() <= 1e-12 {
            continue;
        }
        let t = cross(c - a, f) / denom;
        let u = cross(c - a, d) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            ts.push(t);
        }
    }
    ts.retain(|t| t.is_finite() && (0.0..=1.0).contains(t));
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(Ordering::Equal));
    ts.dedup_by(|x, y| (*x - *y).abs() * len2.sqrt() <= GEOM_EPS);
    ts
}

/// The direction of the edge of `ring` the point `p` lies on, if any.
fn edge_direction_at(p: Vec2, ring: &[Vec2]) -> Option<Vec2> {
    (0..ring.len()).find_map(|i| {
        let (c, e) = (ring[i], ring[(i + 1) % ring.len()]);
        (segment_distance(p, c, e) <= 1e-6).then_some(e - c)
    })
}

/// `subject` minus `clip` (both counter-clockwise simple rings) as counter-clockwise rings.
/// Holes cannot occur for the footprints compared here and are dropped.
fn ring_difference(subject: &[Vec2], clip: &[Vec2]) -> Vec<Vec<Vec2>> {
    if subject.len() < 3 || clip.len() < 3 {
        return if subject.len() >= 3 { vec![subject.to_vec()] } else { Vec::new() };
    }
    let mut pieces: Vec<(Vec2, Vec2)> = Vec::new();
    for i in 0..subject.len() {
        let (a, b) = (subject[i], subject[(i + 1) % subject.len()]);
        let ts = split_params(a, b, clip);
        for w in ts.windows(2) {
            let (p, q) = (a + (b - a) * w[0], a + (b - a) * w[1]);
            if p.dist(q) <= GEOM_EPS {
                continue;
            }
            let mid = (p + q) * 0.5;
            let keep = match edge_direction_at(mid, clip) {
                Some(e) => (q - p).dot(e) < 0.0,
                None => !point_in_polygon(clip, mid),
            };
            if keep {
                pieces.push((p, q));
            }
        }
    }
    for i in 0..clip.len() {
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let ts = split_params(a, b, subject);
        for w in ts.windows(2) {
            let (p, q) = (a + (b - a) * w[0], a + (b - a) * w[1]);
            if p.dist(q) <= GEOM_EPS {
                continue;
            }
            let mid = (p + q) * 0.5;
            if edge_direction_at(mid, subject).is_none() && point_in_polygon(subject, mid) {
                pieces.push((q, p));
            }
        }
    }
    // Chain the pieces end to start into rings.
    let mut used = vec![false; pieces.len()];
    let mut rings = Vec::new();
    for first in 0..pieces.len() {
        if used.get(first).copied().unwrap_or(true) {
            continue;
        }
        let Some(&(start, mut end)) = pieces.get(first) else { continue };
        if let Some(u) = used.get_mut(first) {
            *u = true;
        }
        let mut ring = vec![start];
        let mut closed = false;
        for _ in 0..pieces.len() {
            if end.dist(start) <= 1e-6 {
                closed = true;
                break;
            }
            ring.push(end);
            let next = pieces
                .iter()
                .enumerate()
                .find(|(j, (p, _))| !used.get(*j).copied().unwrap_or(true) && p.dist(end) <= 1e-6)
                .map(|(j, (_, q))| (j, *q));
            let Some((j, q)) = next else { break };
            if let Some(u) = used.get_mut(j) {
                *u = true;
            }
            end = q;
        }
        if closed && ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
        .into_iter()
        .filter(|r| {
            let area = crate::poly::ring_area(r);
            let per = crate::poly::ring_perimeter(r);
            area > 0.0 && per > 0.0 && 2.0 * area / per >= MIN_SPLIT_WIDTH
        })
        .collect()
}

/// The parts of `level`'s footprint that reach past every footprint of the levels below it.
fn side_split_rings(model: &ArchModel, level: &Level) -> Vec<Vec<Vec2>> {
    let Some(fp) = crate::roof::outer_footprint(model, level.index) else { return Vec::new() };
    let mut rings = vec![crate::poly::ensure_ccw(&fp.outer)];
    let mut below: Vec<&Level> =
        model.levels.iter().filter(|l| l.index != level.index && l.elevation.is_finite() && l.elevation < level.elevation).collect();
    below.sort_by_key(|l| l.index);
    for lower in below {
        let Some(bfp) = crate::roof::outer_footprint(model, lower.index) else { continue };
        let clip = crate::poly::ensure_ccw(&bfp.outer);
        rings = rings.iter().flat_map(|r| ring_difference(r, &clip)).collect();
        if rings.is_empty() {
            break;
        }
    }
    rings
}

/// The flat entry aprons outside every exterior door hosted on a wall that spans a landing.
fn apron_regions(model: &ArchModel) -> Vec<PadRegion> {
    let mut out = Vec::new();
    for o in &model.openings {
        if o.kind != OpeningKind::Door || !o.offset.is_finite() || !o.width.is_finite() || o.width <= 0.0 || !o.sill.is_finite() {
            continue;
        }
        let Some(w) = model.walls.iter().find(|w| w.id == o.host) else { continue };
        if !w.exterior || w.divider || !w.start.is_finite() || !w.end.is_finite() || !w.thickness.is_finite() {
            continue;
        }
        let door_z = w.bottom_z(&model.levels) + o.sill;
        // The landing the door opens from: one the wall spans, whose floor is at the door's sill.
        let landing = model
            .levels
            .iter()
            .filter(|l| {
                l.kind == LevelKind::Landing && l.elevation.is_finite() && w.spans_level(l, &model.levels) && (l.elevation - door_z).abs() <= 0.05
            })
            .min_by(|a, b| (a.elevation - door_z).abs().partial_cmp(&(b.elevation - door_z).abs()).unwrap_or(Ordering::Equal));
        let Some(landing) = landing else { continue };
        let d = w.end - w.start;
        let len = d.len();
        if len < 1e-9 {
            continue;
        }
        let u = d * (1.0 / len);
        let mut n = Vec2::new(u.y, -u.x); // the exterior face is on the right of start -> end
        let centre = w.start + u * o.offset;
        let probe = centre + n * (w.thickness / 2.0 + 0.5);
        if crate::roof::outer_footprint(model, w.base_level).is_some_and(|fp| point_in_polygon(&fp.outer, probe)) {
            n = n * -1.0;
        }
        let half = (o.width + APRON_EXTRA_WIDTH) / 2.0;
        let near = centre + n * (w.thickness / 2.0);
        let far = near + n * APRON_DEPTH;
        let ring = vec![near - u * half, near + u * half, far + u * half, far - u * half];
        if ring.iter().any(|p| !p.is_finite()) {
            continue;
        }
        out.push(PadRegion {
            kind: PadKind::Pad,
            ring: crate::poly::ensure_ccw(&ring),
            elevation: landing.elevation - APRON_DROP,
            margin: APRON_MARGIN,
        });
    }
    out
}

/// The footprint the first floor's pad covers: the level's own outer footprint, or, when it has no
/// exterior ring of its own (an entry landing whose only exterior wall is a spanning front wall),
/// the outer footprint of the nearest level below whose exterior walls all stand past the first
/// floor's elevation: the shell the landing sits in, where the ground meets the building.
fn first_floor_footprint(model: &ArchModel, first: &Level) -> Option<Vec<Vec2>> {
    if let Some(fp) = crate::roof::outer_footprint(model, first.index) {
        return Some(fp.outer);
    }
    let mut below: Vec<&Level> =
        model.levels.iter().filter(|l| l.index != first.index && l.elevation.is_finite() && l.elevation < first.elevation).collect();
    below.sort_by(|a, b| b.elevation.partial_cmp(&a.elevation).unwrap_or(Ordering::Equal).then(a.index.cmp(&b.index)));
    below.into_iter().find_map(|lower| {
        let mut shell = model.walls.iter().filter(|w| w.exterior && !w.divider && crate::room::wall_on_level(model, w, lower.index)).peekable();
        let reaches = shell.peek().is_some() && shell.all(|w| w.top_z(&model.levels) > first.elevation);
        if reaches { crate::roof::outer_footprint(model, lower.index).map(|fp| fp.outer) } else { None }
    })
}

/// The pad of the first floor, the pad of every side-split floor, the cut of every basement
/// level that has a footprint, and the entry aprons (last, so they win over pad margins).
pub fn pad_regions(model: &ArchModel) -> Vec<PadRegion> {
    let Some(terrain) = model.terrain.as_ref() else {
        return Vec::new();
    };

    let mut regions = Vec::new();
    let Some(first) = level_first_floor(model) else {
        return regions;
    };

    if terrain.pad {
        if let Some(ring) = first_floor_footprint(model, first) {
            regions.push(PadRegion {
                kind: PadKind::Pad,
                ring,
                elevation: finite_or(first.elevation - first.structure, first.elevation),
                margin: finite_or(terrain.pad_margin, 0.0),
            });
        }

        // Floors above the first floor that reach past the levels below them.
        let mut uppers: Vec<&Level> =
            model.levels.iter().filter(|l| l.kind == LevelKind::Floor && l.elevation.is_finite() && l.elevation > first.elevation).collect();
        uppers.sort_by_key(|l| l.index);
        for level in uppers {
            for ring in side_split_rings(model, level) {
                regions.push(PadRegion {
                    kind: PadKind::Pad,
                    ring,
                    elevation: finite_or(level.elevation - level.structure, level.elevation),
                    margin: finite_or(terrain.pad_margin, 0.0),
                });
            }
        }
    }

    let mut basements: Vec<&Level> = model.levels.iter().filter(|level| level.elevation.is_finite() && level.elevation < first.elevation).collect();
    basements.sort_by_key(|level| level.index);
    for level in basements {
        let Some(fp) = crate::roof::outer_footprint(model, level.index) else {
            continue;
        };
        regions.push(PadRegion {
            kind: PadKind::Basement,
            ring: fp.outer,
            elevation: finite_or(level.elevation - level.structure, level.elevation),
            margin: 0.0,
        });
    }

    if terrain.pad {
        regions.extend(apron_regions(model));
    }

    regions
}

/// Apply `regions` to elevation `z` at `p` (containment decided a hair toward `toward`).
pub fn apply_pads(regions: &[PadRegion], p: Vec2, toward: Vec2, z: f64) -> f64 {
    if !p.is_finite() || !toward.is_finite() || !z.is_finite() {
        return z;
    }

    let mut z = z;
    for region in regions {
        if region.ring.is_empty() {
            continue;
        }

        let q = if toward == p { p } else { p + (toward - p) * 1e-6 };
        let inside = inside_or_on_ring(q, &region.ring);
        let d = if inside { 0.0 } else { ring_distance(p, &region.ring) };
        let elevation = finite_or(region.elevation, z);
        let margin = finite_or(region.margin, 0.0);

        match region.kind {
            PadKind::Pad => {
                if inside || d <= margin {
                    z = elevation;
                } else {
                    let s = d - margin;
                    if z > elevation {
                        z = z.min(elevation + PAD_SLOPE * s);
                    } else {
                        z = z.max(elevation - PAD_SLOPE * s);
                    }
                }
            }
            PadKind::Basement => {
                if inside {
                    z = z.min(elevation);
                }
            }
        }
    }

    // A cut wins inside its footprint, also over the entry aprons applied after it (an apron's
    // slope back to the ground would otherwise lift the cut under the landing).
    let q = if toward == p { p } else { p + (toward - p) * 1e-6 };
    for region in regions.iter().filter(|r| r.kind == PadKind::Basement && !r.ring.is_empty()) {
        if inside_or_on_ring(q, &region.ring) {
            z = z.min(finite_or(region.elevation, z));
        }
    }

    z
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Level, LevelKind, Terrain, Wall};

    fn make_house_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.terrain = Some(Terrain::default());
        model.levels = vec![
            Level { index: 0, name: "Basement".into(), elevation: -2.7, ..Default::default() },
            Level { index: 1, name: "Level 1".into(), elevation: 0.0, ..Default::default() },
        ];
        model.walls = vec![
            Wall {
                id: cadcraft_doc::ArchId(0),
                start: Vec2::new(15.0, 11.0),
                end: Vec2::new(25.0, 11.0),
                base_level: 0,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(1),
                start: Vec2::new(25.0, 11.0),
                end: Vec2::new(25.0, 19.0),
                base_level: 0,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(2),
                start: Vec2::new(25.0, 19.0),
                end: Vec2::new(15.0, 19.0),
                base_level: 0,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(3),
                start: Vec2::new(15.0, 19.0),
                end: Vec2::new(15.0, 11.0),
                base_level: 0,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(4),
                start: Vec2::new(15.0, 11.0),
                end: Vec2::new(25.0, 11.0),
                base_level: 1,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(5),
                start: Vec2::new(25.0, 11.0),
                end: Vec2::new(25.0, 19.0),
                base_level: 1,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(6),
                start: Vec2::new(25.0, 19.0),
                end: Vec2::new(15.0, 19.0),
                base_level: 1,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
            Wall {
                id: cadcraft_doc::ArchId(7),
                start: Vec2::new(15.0, 19.0),
                end: Vec2::new(15.0, 11.0),
                base_level: 1,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            },
        ];
        model
    }

    #[test]
    fn pad_regions_finds_the_first_floor_and_basements() {
        let model = make_house_model();
        let regions = pad_regions(&model);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].kind, PadKind::Pad);
        assert_eq!(regions[0].elevation, -0.3);
        assert_eq!(regions[1].kind, PadKind::Basement);
        assert_eq!(regions[1].elevation, -3.0);
    }

    #[test]
    fn pad_flattens_inside_and_on_the_apron() {
        let rings = vec![Vec2::new(14.9, 11.0), Vec2::new(25.1, 11.0), Vec2::new(25.1, 19.0), Vec2::new(14.9, 19.0)];
        let region = PadRegion { kind: PadKind::Pad, ring: rings, elevation: -0.3, margin: 0.3 };

        assert_eq!(apply_pads(&[region.clone()], Vec2::new(20.0, 15.0), Vec2::new(20.0, 15.0), 0.0), -0.3);
        assert_eq!(apply_pads(&[region.clone()], Vec2::new(14.8, 15.0), Vec2::new(14.8, 15.0), 0.0), -0.3);
    }

    #[test]
    fn pad_grades_back_at_one_in_two() {
        let region = PadRegion {
            kind: PadKind::Pad,
            ring: vec![Vec2::new(14.9, 11.0), Vec2::new(25.1, 11.0), Vec2::new(25.1, 19.0), Vec2::new(14.9, 19.0)],
            elevation: -0.3,
            margin: 0.3,
        };
        let z = apply_pads(&[region], Vec2::new(14.4, 15.0), Vec2::new(14.4, 15.0), 0.72);
        assert!((z - (-0.2)).abs() < 1e-9);
    }

    #[test]
    fn basement_cuts_down_inside_its_footprint() {
        let region = PadRegion {
            kind: PadKind::Basement,
            ring: vec![Vec2::new(14.9, 11.0), Vec2::new(25.1, 11.0), Vec2::new(25.1, 19.0), Vec2::new(14.9, 19.0)],
            elevation: -3.0,
            margin: 0.0,
        };
        assert_eq!(apply_pads(&[region.clone()], Vec2::new(20.0, 15.0), Vec2::new(20.0, 15.0), 0.0), -3.0);
        assert_eq!(apply_pads(&[region], Vec2::new(30.0, 15.0), Vec2::new(30.0, 15.0), 0.0), 0.0);
    }

    #[test]
    fn no_terrain_means_no_pad() {
        let model = ArchModel::default();
        assert!(pad_regions(&model).is_empty());
    }

    #[test]
    fn multiple_basements_each_get_a_region() {
        let mut model = make_house_model();
        model.levels.insert(0, Level { index: u32::MAX, name: "Lower Basement".into(), elevation: -5.4, ..Default::default() });
        for (i, (start, end)) in [
            (Vec2::new(15.0, 11.0), Vec2::new(25.0, 11.0)),
            (Vec2::new(25.0, 11.0), Vec2::new(25.0, 19.0)),
            (Vec2::new(25.0, 19.0), Vec2::new(15.0, 19.0)),
            (Vec2::new(15.0, 19.0), Vec2::new(15.0, 11.0)),
        ]
        .iter()
        .enumerate()
        {
            model.walls.push(Wall {
                id: cadcraft_doc::ArchId(8 + i as u32),
                start: *start,
                end: *end,
                base_level: u32::MAX,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            });
        }
        let regions = pad_regions(&model);
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0].kind, PadKind::Pad);
        assert_eq!(regions[1].kind, PadKind::Basement);
        assert_eq!(regions[1].elevation, -3.0);
        assert_eq!(regions[2].kind, PadKind::Basement);
        assert_eq!(regions[2].elevation, -5.7);
    }

    #[test]
    fn pad_grades_up_when_ground_is_higher() {
        let region = PadRegion {
            kind: PadKind::Pad,
            ring: vec![Vec2::new(14.9, 11.0), Vec2::new(25.1, 11.0), Vec2::new(25.1, 19.0), Vec2::new(14.9, 19.0)],
            elevation: -0.3,
            margin: 0.3,
        };
        // Ground at 0.0 is higher than pad at -0.3. At d=0.5, s=0.2:
        // z = min(0.0, -0.3 + 0.5*0.2) = min(0.0, -0.2) = -0.2
        let z = apply_pads(&[region], Vec2::new(14.4, 15.0), Vec2::new(14.4, 15.0), 0.0);
        assert!((z - (-0.2)).abs() < 1e-9);
    }

    #[test]
    fn apply_pads_handles_nan_inputs() {
        let region = PadRegion {
            kind: PadKind::Pad,
            ring: vec![Vec2::new(14.9, 11.0), Vec2::new(25.1, 11.0), Vec2::new(25.1, 19.0), Vec2::new(14.9, 19.0)],
            elevation: -0.3,
            margin: 0.3,
        };
        // NaN point returns z unchanged
        let z = apply_pads(&[region.clone()], Vec2::new(f64::NAN, 15.0), Vec2::new(20.0, 15.0), 0.5);
        assert!((z - 0.5).abs() < 1e-9);
        // NaN elevation falls back to z
        let region_nan = PadRegion { kind: PadKind::Pad, ring: region.ring.clone(), elevation: f64::NAN, margin: 0.3 };
        let z = apply_pads(&[region_nan], Vec2::new(20.0, 15.0), Vec2::new(20.0, 15.0), 0.5);
        assert!((z - 0.5).abs() < 1e-9);
    }

    #[test]
    fn degenerate_ring_distance_is_computed() {
        let region = PadRegion { kind: PadKind::Pad, ring: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)], elevation: -0.3, margin: 0.0 };
        // Point (0.5, 2.0) is 2.0 away from the segment; d=2.0, s=2.0
        // z = max(0.5, -0.3 - 0.5*2.0) = max(0.5, -1.3) = 0.5
        let z = apply_pads(&[region], Vec2::new(0.5, 2.0), Vec2::new(0.5, 2.0), 0.5);
        assert!((z - 0.5).abs() < 1e-9);
    }

    fn ring_walls(model: &mut ArchModel, pts: &[(f64, f64)], level: u32, closed: bool) {
        let base = model.walls.len() as u32;
        let count = if closed { pts.len() } else { pts.len() - 1 };
        for i in 0..count {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            model.walls.push(Wall {
                id: cadcraft_doc::ArchId(base + i as u32),
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                base_level: level,
                thickness: 0.2,
                exterior: true,
                ..Wall::default()
            });
        }
    }

    /// Lower (-1.2) with a footprint 0..10 x 0..8, Main (1.5) over it plus an extension to x = 14,
    /// and an Entry landing (0.15) whose front door sits on Lower's spanning front wall.
    fn split_house() -> ArchModel {
        let mut model = ArchModel::default();
        model.terrain = Some(Terrain { pad: true, pad_margin: 1.0, ..Terrain::default() });
        model.levels = vec![
            Level { index: 0, name: "Lower".into(), elevation: -1.2, ..Default::default() },
            Level { index: 1, name: "Main".into(), elevation: 1.5, ..Default::default() },
            Level { index: 2, name: "Entry".into(), elevation: 0.15, kind: LevelKind::Landing, ..Default::default() },
        ];
        ring_walls(&mut model, &[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)], 0, true);
        // Main's front wall is Lower's spanning wall w0.
        ring_walls(&mut model, &[(10.0, 0.0), (14.0, 0.0), (14.0, 8.0), (0.0, 8.0), (0.0, 0.0)], 1, false);
        if let Some(w) = model.walls.first_mut() {
            w.top_level = Some(1);
        }
        model
    }

    fn with_door(mut model: ArchModel) -> ArchModel {
        model.openings.push(cadcraft_doc::arch::Opening {
            id: cadcraft_doc::ArchId(100),
            host: cadcraft_doc::ArchId(0),
            offset: 4.6,
            width: 0.9,
            sill: 1.35,
            ..Default::default()
        });
        model
    }

    fn grade(regions: &[PadRegion], x: f64, y: f64, z: f64) -> f64 {
        apply_pads(regions, Vec2::new(x, y), Vec2::new(x, y), z)
    }

    #[test]
    fn an_entry_door_gets_a_flat_apron_one_step_down() {
        let model = with_door(split_house());
        let regions = pad_regions(&model);
        let apron = regions.last().expect("apron");
        assert_eq!(apron.kind, PadKind::Pad);
        assert!(apron.elevation.abs() < 1e-9, "{}", apron.elevation);
        assert_eq!(apron.margin, 0.6);
        let (min_x, max_x) = apron.ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        assert!((min_x - 3.55).abs() < 1e-9 && (max_x - 5.65).abs() < 1e-9, "{min_x} {max_x}");
        let (min_y, max_y) = apron.ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        assert!((max_y + 0.1).abs() < 1e-9 && (min_y + 1.6).abs() < 1e-9, "{min_y} {max_y}");
        assert!(grade(&regions, 4.6, -0.8, 0.5).abs() < 1e-9);
        // Without the door there is no apron.
        assert_eq!(pad_regions(&split_house()).len(), regions.len() - 1);
    }

    #[test]
    fn a_landing_first_floor_is_padded_inside_the_walls_around_it() {
        // The Entry landing (0.15) is the first floor, but its only exterior wall is Lower's spanning
        // front wall: the pad follows the shell standing at the landing's elevation (Lower's walls).
        let model = split_house();
        let regions = pad_regions(&model);
        let pads: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad && (r.elevation + 0.15).abs() < 1e-9).collect();
        assert_eq!(pads.len(), 1, "{regions:?}");
        assert_eq!(pads[0].margin, 1.0);
        // Outside the front wall, within the margin: flat at the landing's elevation - structure.
        assert!((grade(&regions, 2.5, -0.5, 0.109375) + 0.15).abs() < 1e-9);
        // Inside Lower's footprint the cut still wins, and the side split keeps its own pad.
        assert!((grade(&regions, 4.5, 1.5, 0.0) + 1.5).abs() < 1e-9);
        assert!((grade(&regions, 12.0, 4.0, 0.25) - 1.2).abs() < 1e-9);
    }

    #[test]
    fn a_landing_with_its_own_exterior_ring_keeps_its_own_pad() {
        let mut model = split_house();
        ring_walls(&mut model, &[(20.0, 0.0), (24.0, 0.0), (24.0, 4.0), (20.0, 4.0)], 2, true);
        let own = crate::roof::outer_footprint(&model, 2).map(|fp| fp.outer);
        assert!(own.is_some(), "the landing's own ring");
        let regions = pad_regions(&model);
        let pads: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad && (r.elevation + 0.15).abs() < 1e-9).collect();
        assert_eq!(pads.len(), 1, "{regions:?}");
        assert_eq!(Some(pads[0].ring.clone()), own);
    }

    #[test]
    fn a_landing_above_the_walls_below_gets_no_borrowed_pad() {
        let mut model = split_house();
        for w in model.walls.iter_mut().filter(|w| w.base_level == 0 && w.top_level.is_none()) {
            w.height = 1.0;
        }
        // Lower's back walls stop at -0.2, under the landing: the landing borrows no footprint.
        let regions = pad_regions(&model);
        assert!(!regions.iter().any(|r| r.kind == PadKind::Pad && (r.elevation + 0.15).abs() < 1e-9), "{regions:?}");
    }

    #[test]
    fn a_side_split_extension_gets_its_own_pad() {
        let model = split_house();
        let regions = pad_regions(&model);
        let split: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad && (r.elevation - 1.2).abs() < 1e-9).collect();
        assert_eq!(split.len(), 1, "{regions:?}");
        assert_eq!(split[0].margin, 1.0);
        assert!((grade(&regions, 12.0, 4.0, 0.0) - 1.2).abs() < 1e-9);
        assert!((grade(&regions, 25.0, 25.0, 0.3) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn a_basement_cut_wins_inside_its_footprint() {
        let model = split_house();
        let regions = pad_regions(&model);
        // Inside Lower's footprint, within the extension's margin: the cut, not the pad.
        assert!((grade(&regions, 9.5, 4.0, 0.0) - (-1.5)).abs() < 1e-9);
        assert!((grade(&regions, 4.5, 1.5, 0.0) - (-1.5)).abs() < 1e-9);
    }

    #[test]
    fn a_basement_cut_wins_over_the_entry_apron_slope() {
        // The apron (0.0, applied last) slopes back up 1:2 from its margin; under the landing,
        // 1.6 m from the apron, that slope must not lift Lower's cut (-1.5).
        let model = with_door(split_house());
        let regions = pad_regions(&model);
        assert!((grade(&regions, 4.5, 1.5, 0.0) + 1.5).abs() < 1e-9, "{}", grade(&regions, 4.5, 1.5, 0.0));
        assert!((grade(&regions, 4.6, 0.5, 0.0) + 1.5).abs() < 1e-9, "{}", grade(&regions, 4.6, 0.5, 0.0));
        // Outside the footprint the apron still wins: flat at 0.0, and over the landing pad's margin.
        assert!(grade(&regions, 4.6, -0.8, 0.5).abs() < 1e-9);
        assert!((grade(&regions, 2.5, -0.5, 0.109375) + 0.15).abs() < 1e-9);
    }

    #[test]
    fn pads_without_split_levels_are_unchanged() {
        let model = make_house_model();
        let regions = pad_regions(&model);
        assert_eq!(regions.len(), 2);
        assert_eq!((regions[0].kind, regions[0].elevation, regions[0].margin), (PadKind::Pad, -0.3, Terrain::default().pad_margin));
        assert_eq!((regions[1].kind, regions[1].elevation), (PadKind::Basement, -3.0));
    }

    #[test]
    fn multiple_side_split_levels_each_get_a_pad() {
        let mut model = ArchModel::default();
        model.terrain = Some(Terrain { pad: true, pad_margin: 0.5, ..Terrain::default() });
        model.levels = vec![
            Level { index: 0, name: "Lower".into(), elevation: -1.2, ..Default::default() },
            Level { index: 1, name: "Main".into(), elevation: 1.5, ..Default::default() },
            Level { index: 2, name: "Upper".into(), elevation: 4.2, ..Default::default() },
        ];
        // Lower: 0..10 x 0..8
        ring_walls(&mut model, &[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)], 0, true);
        // Main: 0..14 x 0..8 (extends past Lower on the right)
        ring_walls(&mut model, &[(0.0, 0.0), (14.0, 0.0), (14.0, 8.0), (0.0, 8.0)], 1, true);
        // Upper: 0..14 x 0..12 (extends past Main on the top)
        ring_walls(&mut model, &[(0.0, 0.0), (14.0, 0.0), (14.0, 12.0), (0.0, 12.0)], 2, true);

        let regions = pad_regions(&model);
        // Lower pad, Main side split, Upper side split
        let pads: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad).collect();
        assert_eq!(pads.len(), 3, "expected 3 pads, got {}", pads.len());
        // Main's side split should be at elevation 1.5 - 0.3 = 1.2
        let main_split = pads.iter().find(|r| (r.elevation - 1.2).abs() < 1e-9);
        assert!(main_split.is_some(), "no Main side split pad");
        // Upper's side split should be at elevation 4.2 - 0.3 = 3.9
        let upper_split = pads.iter().find(|r| (r.elevation - 3.9).abs() < 1e-9);
        assert!(upper_split.is_some(), "no Upper side split pad");
    }

    #[test]
    fn side_split_with_no_extension_gets_no_pad() {
        let mut model = ArchModel::default();
        model.terrain = Some(Terrain { pad: true, pad_margin: 0.5, ..Terrain::default() });
        model.levels = vec![
            Level { index: 0, name: "Lower".into(), elevation: -1.2, ..Default::default() },
            Level { index: 1, name: "Main".into(), elevation: 1.5, ..Default::default() },
        ];
        // Both levels have the same footprint
        ring_walls(&mut model, &[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)], 0, true);
        ring_walls(&mut model, &[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)], 1, true);

        let regions = pad_regions(&model);
        // Only the Lower pad, no Main side split
        let pads: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad).collect();
        assert_eq!(pads.len(), 1, "expected 1 pad, got {}", pads.len());
        assert!((pads[0].elevation - (-1.5)).abs() < 1e-9, "wrong elevation {}", pads[0].elevation);
    }

    #[test]
    fn multiple_doors_on_the_same_landing_get_aprons() {
        let mut model = split_house();
        // Add two doors on the same wall
        model.openings.push(cadcraft_doc::arch::Opening {
            id: cadcraft_doc::ArchId(100),
            host: cadcraft_doc::ArchId(0),
            offset: 4.6,
            width: 0.9,
            sill: 1.35,
            ..Default::default()
        });
        model.openings.push(cadcraft_doc::arch::Opening {
            id: cadcraft_doc::ArchId(101),
            host: cadcraft_doc::ArchId(0),
            offset: 7.0,
            width: 0.9,
            sill: 1.35,
            ..Default::default()
        });

        let regions = pad_regions(&model);
        // Two aprons (elevation 0.0)
        let aprons: Vec<&PadRegion> = regions.iter().filter(|r| r.kind == PadKind::Pad && r.elevation.abs() < 1e-9).collect();
        assert_eq!(aprons.len(), 2, "expected 2 aprons, got {}", aprons.len());
    }

    #[test]
    fn pad_follows_l_shaped_footprint() {
        let mut model = ArchModel::default();
        model.terrain = Some(Terrain { pad: true, pad_margin: 0.3, ..Terrain::default() });
        model.levels = vec![Level { index: 0, name: "Level 1".into(), elevation: 0.0, ..Default::default() }];
        // L-shaped footprint: (0,0) to (10,0) to (10,4) to (6,4) to (6,8) to (0,8)
        ring_walls(&mut model, &[(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (6.0, 4.0), (6.0, 8.0), (0.0, 8.0)], 0, true);

        let regions = pad_regions(&model);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].kind, PadKind::Pad);
        // The pad ring should have 6 vertices (same as the footprint)
        assert_eq!(regions[0].ring.len(), 6, "expected 6 vertices, got {}", regions[0].ring.len());
    }

    #[test]
    fn apron_for_a_wide_door_extends_past_the_wall() {
        let mut model = split_house();
        // A door wider than the wall (wall is 10m wide, door is 12m wide)
        model.openings.push(cadcraft_doc::arch::Opening {
            id: cadcraft_doc::ArchId(100),
            host: cadcraft_doc::ArchId(0),
            offset: 5.0,
            width: 12.0,
            sill: 1.35,
            ..Default::default()
        });

        let regions = pad_regions(&model);
        let apron = regions.last().expect("apron");
        assert_eq!(apron.kind, PadKind::Pad);
        // The apron should be 12 + 1.2 = 13.2m wide
        let (min_x, max_x) = apron.ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        assert!((max_x - min_x - 13.2).abs() < 1e-9, "apron width {}", max_x - min_x);
    }
}
