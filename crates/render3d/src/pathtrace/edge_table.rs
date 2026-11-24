//! The edge table (phase 22): per triangle, the neighbouring face normal across each edge so the
//! shader can bend the shading normal near a crease without changing the geometry.

use super::PathOptions;
use super::scene_gpu::{GpuFrame, GpuScene, GpuTri, PHOTO_EDGES};
use cadcraft_doc::arch::EdgeMode;
use std::collections::BTreeMap;

const EDGE_EPS: f32 = 1e-5;
const EDGE_ANGLE: f32 = 20.0f32.to_radians();
const MAX_AUX_FLOATS: usize = 1 << 25;

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if !len.is_finite() || len <= 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn quantize(v: [f32; 3]) -> [i64; 3] {
    [(v[0] / EDGE_EPS).round() as i64, (v[1] / EDGE_EPS).round() as i64, (v[2] / EDGE_EPS).round() as i64]
}

fn same_edge_key(obj: u32, a: [f32; 3], b: [f32; 3]) -> (u32, [i64; 3], [i64; 3]) {
    let qa = quantize(a);
    let qb = quantize(b);
    let (lo, hi) = if qa <= qb { (qa, qb) } else { (qb, qa) };
    (obj, lo, hi)
}

fn oct_encode_dir(n: [f32; 3]) -> u32 {
    let n = norm(n);
    let ax = n[0].abs();
    let ay = n[1].abs();
    let az = n[2].abs();
    let denom = (ax + ay + az).max(1e-8);
    let mut x = n[0] / denom;
    let mut y = n[1] / denom;
    if n[2] < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        x = (1.0 - ay) * sx;
        y = (1.0 - ax) * sy;
    }
    let xi = ((x.clamp(-1.0, 1.0) * 32767.0).round() as i32).clamp(-32768, 32767) as u32;
    let yi = ((y.clamp(-1.0, 1.0) * 32767.0).round() as i32).clamp(-32768, 32767) as u32;
    0x8000_0000 | ((xi & 0x7fff) | ((yi & 0x7fff) << 15))
}

fn edge_vertices(tri: &GpuTri, edge: usize) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let v0 = tri.v0;
    let v1 = add(v0, tri.e1);
    let v2 = add(v0, tri.e2);
    match edge {
        0 => (v0, v1, v2),
        1 => (v1, v2, v0),
        _ => (v2, v0, v1),
    }
}

/// Build the edge table the path tracer reads from the aux buffer at `frame.photo.y`.
pub fn edge_table(tris: &[GpuTri]) -> Vec<u32> {
    let mut table = vec![0u32; tris.len() * 3];
    if tris.is_empty() {
        return table;
    }

    let mut by_edge: BTreeMap<(u32, [i64; 3], [i64; 3]), Vec<usize>> = BTreeMap::new();
    for (i, tri) in tris.iter().enumerate() {
        for edge in 0..3 {
            let (a, b, _) = edge_vertices(tri, edge);
            by_edge.entry(same_edge_key(tri.object, a, b)).or_default().push(i);
        }
    }

    for (i, tri) in tris.iter().enumerate() {
        for edge in 0..3 {
            let (a, b, far) = edge_vertices(tri, edge);
            let key = same_edge_key(tri.object, a, b);
            let Some(matches) = by_edge.get(&key) else { continue };
            if matches.len() != 2 {
                continue;
            }
            let other_idx = matches.iter().copied().find(|&j| j != i).unwrap_or(usize::MAX);
            if other_idx == usize::MAX {
                continue;
            }
            let other = &tris[other_idx];
            let n0 = norm(tri.n);
            let n1 = norm(other.n);
            let angle = dot(n0, n1).clamp(-1.0, 1.0).acos();
            if angle <= EDGE_ANGLE || angle >= std::f32::consts::PI - EDGE_ANGLE {
                continue;
            }

            let other_points = [other.v0, add(other.v0, other.e1), add(other.v0, other.e2)];
            let mut other_far = None;
            for &v in &other_points {
                let same_a = (v[0] - a[0]).abs() <= 1e-6 && (v[1] - a[1]).abs() <= 1e-6 && (v[2] - a[2]).abs() <= 1e-6;
                let same_b = (v[0] - b[0]).abs() <= 1e-6 && (v[1] - b[1]).abs() <= 1e-6 && (v[2] - b[2]).abs() <= 1e-6;
                if !same_a && !same_b {
                    other_far = Some(v);
                    break;
                }
            }
            let Some(other_far) = other_far else {
                continue;
            };

            if dot(sub(other_far, a), n0) >= 0.0 {
                continue;
            }
            if dot(sub(far, a), n0) > 0.0 {
                continue;
            }

            table[i * 3 + edge] = oct_encode_dir(n1);
        }
    }
    table
}

/// Append the edge table when `opts.edges` requests it, and record the offset in `frame.photo`.
pub(super) fn fill(scene: &mut GpuScene, frame: &mut GpuFrame, opts: &PathOptions) {
    frame.photo[0] &= !PHOTO_EDGES;
    frame.photo[1] = 0;
    frame.photo[2] = 0;
    frame.photo[3] = 0;
    frame.photo_f[0] = 0.0;

    match opts.edges {
        EdgeMode::Off => {}
        _ => {
            let table = edge_table(&scene.tris);
            let need = scene.ies.len() + table.len();
            if need > MAX_AUX_FLOATS {
                return;
            }

            let offset = scene.ies.len() as u32;
            scene.ies.extend(table.iter().map(|&word| f32::from_bits(word)));
            frame.photo[0] |= PHOTO_EDGES;
            frame.photo[1] = offset;
            frame.photo[2] = scene.tris.len() as u32;

            match opts.edges {
                EdgeMode::Radius(r) => frame.photo_f[0] = r as f32,
                EdgeMode::PerMaterial => frame.photo_f[0] = -1.0,
                EdgeMode::Off => unreachable!(),
            }
        }
    }
}
