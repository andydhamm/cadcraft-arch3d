//! Electrical device placement and plan symbols for the phase-15 arch model.
//!
//! All points are in world-space metres. The device sits on the wall face rather than on the
//! wall centre-line, and the room detection follows the same face-probe approach as `arch::paint`.

use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind, OpeningKind, Side, WallSide};
use cadcraft_doc::{ArchId, ArchModel, Level, Opening, Wall};
use cadcraft_geom::{Vec2, point_in_polygon};

/// Plate dimensions: width along the wall, depth proud of the face, height.
pub const PLATE: [f64; 3] = [0.07, 0.006, 0.115];

/// A wall-mounted electrical device placed on one face of a wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevicePlace {
    /// Plate centre on the wall face.
    pub at: Vec2,
    /// Unit normal pointing out of the wall face.
    pub normal: Vec2,
    /// Absolute z coordinate of the plate centre.
    pub z: f64,
    /// Unit vector along the wall.
    pub along: Vec2,
}

fn room_at(model: &ArchModel, level: u32, p: Vec2) -> Option<ArchId> {
    if !p.is_finite() {
        return None;
    }
    model
        .rooms
        .iter()
        .find(|r| {
            r.level == level
                && r.polygon.len() >= 3
                && point_in_polygon(&r.polygon, p)
                && !r.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, p))
        })
        .map(|r| r.id)
}

fn wall_level<'a>(model: &'a ArchModel, d: &ElectricalDevice) -> Result<(&'a Wall, &'a Level), String> {
    let wall = model.wall(d.wall).ok_or_else(|| format!("wall {} does not exist", d.wall.0))?;
    let level = model.level(d.level).ok_or_else(|| format!("level {} does not exist", d.level))?;
    Ok((wall, level))
}

fn floor_reaching_opening(o: &Opening) -> bool {
    matches!(o.kind, OpeningKind::Door | OpeningKind::GarageDoor | OpeningKind::CasedOpening) && o.sill <= 1e-9
}

fn counter_top(model: &ArchModel, p: Vec2, level: u32) -> Option<f64> {
    let runs = crate::casework_layout::counter_runs(model);
    runs.iter().find_map(|run| {
        if run.level != level {
            return None;
        }
        let dir = Vec2::new(run.rotation.to_radians().cos(), run.rotation.to_radians().sin());
        let delta = p - run.origin;
        let u = delta.dot(dir);
        if !(run.u0 - 1e-6..=run.u1 + 1e-6).contains(&u) {
            return None;
        }
        Some(run.top + run.thickness + 0.15)
    })
}

/// Place a device on the face of its wall.
pub fn place_device(model: &ArchModel, d: &ElectricalDevice) -> Result<DevicePlace, String> {
    let (wall, level) = wall_level(model, d)?;
    let length = wall.start.dist(wall.end);
    if !length.is_finite() || length <= 1e-12 {
        return Err("wall has no usable length".to_string());
    }
    if !(0.05..length - 0.05).contains(&d.offset) {
        return Err("offset outside 0.05..length-0.05".to_string());
    }

    let dir = (wall.end - wall.start).normalized();
    if dir == Vec2::ZERO {
        return Err("wall direction is zero".to_string());
    }

    let normal = match d.side {
        WallSide::Left => dir.perp(),
        WallSide::Right => -dir.perp(),
    }
    .normalized();
    let along = dir;
    let at = wall.start + along * d.offset + normal * (wall.thickness / 2.0);

    for opening in &model.openings {
        if opening.host != d.wall || !floor_reaching_opening(opening) {
            continue;
        }
        let half = opening.width / 2.0;
        let lo = opening.offset - half;
        let hi = opening.offset + half;
        if d.offset >= lo - 1e-6 && d.offset <= hi + 1e-6 {
            return Err("point is inside an opening that reaches the floor".to_string());
        }
    }

    let z = if d.counter {
        counter_top(model, at, d.level).ok_or_else(|| "counter with no counter in front".to_string())?
    } else {
        level.elevation + d.height
    };

    if z > level.elevation + wall.height || z < level.elevation {
        return Err("plate is outside the wall bounds".to_string());
    }

    Ok(DevicePlace { at, normal, z, along })
}

/// The room on the selected wall side, measured 0.05 m off the face.
pub fn device_room(model: &ArchModel, d: &ElectricalDevice) -> Option<ArchId> {
    let wall = model.wall(d.wall)?;
    let dir = (wall.end - wall.start).normalized();
    let side = match d.side {
        WallSide::Left => Side::Left,
        WallSide::Right => Side::Right,
    };
    let n = match side {
        Side::Left => dir.perp(),
        Side::Right => -dir.perp(),
    };
    let probe = wall.start + dir * d.offset + n * (wall.thickness / 2.0 + 0.05);
    room_at(model, d.level, probe)
}

fn circle(center: Vec2, radius: f64) -> Vec<Vec2> {
    (0..=24)
        .map(|i| {
            let a = (i as f64 / 24.0) * std::f64::consts::TAU;
            center + Vec2::new(a.cos() * radius, a.sin() * radius)
        })
        .collect()
}

fn line(a: Vec2, b: Vec2) -> Vec<Vec2> {
    vec![a, b]
}

fn square(center: Vec2, half: f64) -> Vec<Vec2> {
    vec![
        center + Vec2::new(-half, -half),
        center + Vec2::new(half, -half),
        center + Vec2::new(half, half),
        center + Vec2::new(-half, half),
        center + Vec2::new(-half, -half),
    ]
}

fn triangle(center: Vec2, radius: f64, along: Vec2, normal: Vec2) -> Vec<Vec2> {
    let a = center + along * radius;
    let b = center - along * radius * 0.5 + normal * radius * 0.9;
    let c = center - along * radius * 0.5 - normal * radius * 0.9;
    vec![a, b, c, a]
}

fn s_shape(center: Vec2, along: Vec2, normal: Vec2, size: f64) -> Vec<Vec2> {
    let p0 = center - along * size * 0.5 - normal * size * 0.5;
    let p1 = center + along * size * 0.55 - normal * size * 0.1;
    let p2 = center - along * size * 0.55 + normal * size * 0.1;
    let p3 = center + along * size * 0.5 + normal * size * 0.5;
    vec![p0, p1, p2, p3]
}

fn arrow(center: Vec2, along: Vec2, normal: Vec2, size: f64) -> Vec<Vec2> {
    let tip = center + along * size;
    let left = tip - along * size * 0.35 + normal * size * 0.3;
    let right = tip - along * size * 0.35 - normal * size * 0.3;
    vec![center - along * size * 0.15, tip, left, tip, right]
}

/// Plan symbol polyline set for `d`, or an empty set when the device cannot be placed.
pub fn device_symbol(model: &ArchModel, d: &ElectricalDevice) -> Vec<Vec<Vec2>> {
    let Ok(place) = place_device(model, d) else {
        return Vec::new();
    };

    let center = place.at + place.normal * 0.15;
    let mut polys: Vec<Vec<Vec2>> = vec![circle(center, 0.1)];
    polys.push(line(place.at + place.normal * 0.05, center - place.normal * 0.05));

    match d.kind {
        ElectricalKind::Duplex => {
            let offset = place.normal * 0.035;
            polys.push(line(center - place.along * 0.12 + offset, center + place.along * 0.12 + offset));
            polys.push(line(center - place.along * 0.12 - offset, center + place.along * 0.12 - offset));
        }
        ElectricalKind::Gfci => {
            let offset = place.normal * 0.035;
            polys.push(line(center - place.along * 0.12 + offset, center + place.along * 0.12 + offset));
            polys.push(line(center - place.along * 0.12 - offset, center + place.along * 0.12 - offset));
            polys.push(square(center, 0.06));
        }
        ElectricalKind::Outlet240 => {
            polys.push(triangle(center, 0.05, place.along, place.normal));
        }
        ElectricalKind::SinglePole => {
            polys.push(s_shape(center, place.along, place.normal, 0.18));
        }
        ElectricalKind::ThreeWay => {
            polys.push(s_shape(center, place.along, place.normal, 0.18));
            polys.push(line(center + place.along * 0.05 + place.normal * 0.06, center + place.along * 0.09 + place.normal * 0.02));
            polys.push(line(center + place.along * 0.09 + place.normal * 0.02, center + place.along * 0.09 - place.normal * 0.02));
        }
        ElectricalKind::Dimmer => {
            polys.push(s_shape(center, place.along, place.normal, 0.18));
            polys.push(arrow(center + place.along * 0.08, place.along, place.normal, 0.08));
        }
    }

    polys
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ElectricalDevice, WallSide};
    use cadcraft_doc::{ArchId, ArchModel, Opening, Wall};

    fn wall(model: &mut ArchModel, id: u32, start: Vec2, end: Vec2) -> ArchId {
        model.push_wall(Wall { id: ArchId(id), start, end, thickness: 0.2, height: 2.4, ..Wall::default() })
    }

    #[test]
    fn a_device_sits_on_its_wall_face_at_its_offset() {
        let mut model = ArchModel::default();
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        model.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 2.0), Vec2::new(0.0, 2.0)],
            ..Default::default()
        });
        let d = ElectricalDevice { wall: w, side: WallSide::Left, offset: 1.5, height: 0.3, ..Default::default() };
        let p = place_device(&model, &d).unwrap();
        assert!((p.at.x - 1.5).abs() < 1e-9);
        assert!((p.at.y - 0.1).abs() < 1e-9);
        assert!((p.normal.x - 0.0).abs() < 1e-9 && (p.normal.y - 1.0).abs() < 1e-9);
        assert!((p.along.x - 1.0).abs() < 1e-9 && (p.along.y - 0.0).abs() < 1e-9);
        assert!((p.z - 0.3).abs() < 1e-9);
    }

    #[test]
    fn a_counter_outlet_sits_above_the_counter() {
        let mut model = ArchModel::default();
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        model.cabinets.push(cadcraft_doc::arch::Cabinet {
            id: ArchId(0),
            kind: cadcraft_doc::arch::CabinetKind::Base,
            level: 0,
            position: Vec2::new(1.5, 0.0),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            ..Default::default()
        });
        let d = ElectricalDevice { wall: w, side: WallSide::Left, offset: 1.5, height: 0.0, counter: true, ..Default::default() };
        let p = place_device(&model, &d).unwrap();
        assert!((p.z - 1.064).abs() < 1e-9);
    }

    #[test]
    fn an_outlet_in_a_doorway_is_refused() {
        let mut model = ArchModel::default();
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        model.openings.push(Opening { host: w, offset: 1.5, width: 1.0, height: 2.1, sill: 0.0, kind: OpeningKind::Door, ..Default::default() });
        let d = ElectricalDevice { wall: w, side: WallSide::Left, offset: 1.5, ..Default::default() };
        assert!(place_device(&model, &d).is_err());
    }

    #[test]
    fn an_offset_past_the_wall_end_is_refused() {
        let mut model = ArchModel::default();
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        let d = ElectricalDevice { wall: w, side: WallSide::Left, offset: 4.1, ..Default::default() };
        assert!(place_device(&model, &d).is_err());
    }

    #[test]
    fn device_symbols_are_deterministic() {
        let mut model = ArchModel::default();
        let w = wall(&mut model, 0, Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0));
        let d = ElectricalDevice { wall: w, side: WallSide::Left, offset: 1.5, ..Default::default() };
        let a = device_symbol(&model, &d);
        let b = device_symbol(&model, &d);
        assert_eq!(a, b);
    }
}
