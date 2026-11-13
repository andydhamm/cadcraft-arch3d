use std::collections::{BTreeMap, BTreeSet};

use cadcraft_doc::Handle;
use cadcraft_doc::arch::{ArchId, ArchModel, Cabinet, Fixture, Furnishing, Opening, Wall};
use cadcraft_geom::Vec2;

pub const MAX_TRANSLATE_DELTA: f64 = 1e4;

fn wall_id_of_handle(h: Handle) -> Option<ArchId> {
    if !cadcraft_doc::arch::is_arch_handle(h)
        || cadcraft_doc::arch::is_opening_handle(h)
        || cadcraft_doc::arch::is_cabinet_handle(h)
        || cadcraft_doc::arch::is_fixture_handle(h)
        || cadcraft_doc::arch::is_furnishing_handle(h)
    {
        return None;
    }
    let low = h.0.checked_sub(cadcraft_doc::arch::ARCH_HANDLE_BASE)?;
    if low < cadcraft_doc::arch::OPENING_HANDLE_OFFSET { Some(ArchId(u32::try_from(low).ok()?)) } else { None }
}

fn opening_id_of_handle(h: Handle) -> Option<ArchId> {
    if !cadcraft_doc::arch::is_opening_handle(h) {
        return None;
    }
    let lo = cadcraft_doc::arch::ARCH_HANDLE_BASE.saturating_add(cadcraft_doc::arch::OPENING_HANDLE_OFFSET);
    u32::try_from(h.0.checked_sub(lo)?).ok().map(ArchId)
}

fn cabinet_id_of_handle(h: Handle) -> Option<ArchId> {
    if !cadcraft_doc::arch::is_cabinet_handle(h) {
        return None;
    }
    let lo = cadcraft_doc::arch::ARCH_HANDLE_BASE.saturating_add(cadcraft_doc::arch::casework::CABINET_HANDLE_OFFSET);
    u32::try_from(h.0.checked_sub(lo)?).ok().map(ArchId)
}

fn fixture_id_of_handle(h: Handle) -> Option<ArchId> {
    if !cadcraft_doc::arch::is_fixture_handle(h) {
        return None;
    }
    let lo = cadcraft_doc::arch::ARCH_HANDLE_BASE.saturating_add(cadcraft_doc::arch::casework::FIXTURE_HANDLE_OFFSET);
    u32::try_from(h.0.checked_sub(lo)?).ok().map(ArchId)
}

fn furnishing_id_of_handle(h: Handle) -> Option<ArchId> {
    if !cadcraft_doc::arch::is_furnishing_handle(h) {
        return None;
    }
    let lo = cadcraft_doc::arch::ARCH_HANDLE_BASE.saturating_add(cadcraft_doc::arch::furnishing::FURNISHING_HANDLE_OFFSET);
    u32::try_from(h.0.checked_sub(lo)?).ok().map(ArchId)
}

/// Move one wall and every connected neighbour that shares the moved end.
pub fn translate_wall(model: &mut ArchModel, id: ArchId, delta: Vec2) -> Result<Vec<ArchId>, String> {
    if !delta.is_finite() || delta.len() > MAX_TRANSLATE_DELTA {
        return Err("delta must be finite and no larger than 1e4 metres".to_string());
    }

    let old_walls: BTreeMap<ArchId, (Vec2, Vec2)> = model.walls.iter().map(|w| (w.id, (w.start, w.end))).collect();
    let wall = model.wall(id).ok_or_else(|| format!("wall {id} not found"))?.clone();
    let old_start = wall.start;
    let old_end = wall.end;
    let old_dir = (old_end - old_start).normalized();
    let mut moved = BTreeSet::new();
    moved.insert(id);

    let mut clone = model.clone();
    let moved_start =
        crate::join::stretch_wall(&mut clone, id, crate::join::End::Start, old_start + delta).map_err(|e| format!("start stretch failed: {e}"))?;
    for wid in moved_start {
        moved.insert(wid);
    }
    let moved_end =
        crate::join::stretch_wall(&mut clone, id, crate::join::End::End, old_end + delta).map_err(|e| format!("end stretch failed: {e}"))?;
    for wid in moved_end {
        moved.insert(wid);
    }

    for wid in &moved {
        let Some(w) = clone.wall(*wid) else { continue };
        if w.start.dist(w.end) < cadcraft_doc::arch::MIN_WALL_LENGTH {
            return Err(format!("wall {wid} would collapse"));
        }
    }

    // T-neighbour slide: if a wall endpoint sat on the moved wall's old centre line,
    // move it along the neighbour's own axis to the new intersection with the moved wall.
    if old_dir.is_finite() && old_dir != Vec2::ZERO {
        let new_w = clone.wall(id).ok_or_else(|| format!("wall {id} not found"))?.clone();
        let new_dir = (new_w.end - new_w.start).normalized();
        if new_dir.is_finite() && new_dir != Vec2::ZERO {
            for other in model.walls.iter() {
                if other.id == id || moved.contains(&other.id) || other.base_level != wall.base_level {
                    continue;
                }
                let other_dir = (other.end - other.start).normalized();
                if !other_dir.is_finite() || other_dir == Vec2::ZERO || (other_dir.dot(new_dir)).abs() > 1.0 - 1e-6 {
                    continue;
                }
                for (point, field) in [(other.start, "start"), (other.end, "end")] {
                    let v = point - old_start;
                    let t = v.dot(old_dir);
                    if !(t > 1e-6 && t < (old_end - old_start).len() - 1e-6) {
                        continue;
                    }
                    let off = v - old_dir * t;
                    if off.len() > 1e-6 {
                        continue;
                    }
                    let pivot = new_w.start + new_dir * t;
                    let pos = clone.walls.iter().position(|w| w.id == other.id).ok_or_else(|| format!("wall {} missing", other.id))?;
                    let mut shifted = clone.walls[pos].clone();
                    match field {
                        "start" => shifted.start = pivot,
                        "end" => shifted.end = pivot,
                        _ => unreachable!(),
                    }
                    if shifted.start.dist(shifted.end) < cadcraft_doc::arch::MIN_WALL_LENGTH {
                        return Err(format!("wall {} would collapse", other.id));
                    }
                    clone.walls[pos] = shifted;
                    moved.insert(other.id);
                    break;
                }
            }
        }
    }

    // Keep each opening on its host wall. When the host moved rigidly, the opening stays
    // attached to the wall and therefore its offset changes by the wall's parallel travel.
    // When a neighbour was only shifted on one end, preserve the opening's world position.
    for wid in &moved {
        let Some(old) = old_walls.get(wid) else { continue };
        let Some(host_after) = clone.wall(*wid).map(|w| (w.start, w.end)) else { continue };
        let start_delta = host_after.0 - old.0;
        let end_delta = host_after.1 - old.1;
        let rigid = (start_delta - end_delta).len() <= 1e-6;
        let old_dir = (old.1 - old.0).normalized();
        if !old_dir.is_finite() || old_dir == Vec2::ZERO {
            continue;
        }

        let positions: Vec<_> =
            clone.openings.iter().enumerate().filter(|(_, opening)| opening.host == *wid).map(|(idx, opening)| (idx, opening.clone())).collect();
        for (idx, opening) in positions {
            let check_h = crate::opening::check_height(&clone, &opening);
            let new_offset = if rigid {
                let travel = (host_after.0 - old.0).dot(old_dir);
                opening.offset + travel
            } else {
                let old_world = old.0 + old_dir * opening.offset;
                crate::opening::project_to_wall(&clone, opening.host, old_world)
                    .ok_or_else(|| format!("opening o{} could not stay on wall w{}", opening.id.0, opening.host.0))?
            };
            let validated = crate::opening::validate_placement(
                &clone,
                opening.host,
                new_offset,
                opening.width,
                opening.sill,
                check_h,
                opening.kind,
                &opening.style,
                Some(opening.id),
            )
            .map_err(|_| format!("opening o{} would leave wall w{}", opening.id.0, opening.host.0))?;
            clone.openings[idx].offset = validated;
        }
    }

    *model = clone;
    let out: Vec<ArchId> = moved.iter().copied().collect();
    Ok(out)
}

/// Move a wall, opening, cabinet, fixture or furnishing by a finite delta.
pub fn translate_object(model: &mut ArchModel, h: Handle, delta: Vec2) -> Result<(), String> {
    if !delta.is_finite() || delta.len() > MAX_TRANSLATE_DELTA {
        return Err("delta must be finite and no larger than 1e4 metres".to_string());
    }

    let mut clone = model.clone();
    if cadcraft_doc::arch::is_opening_handle(h) {
        let Some(id) = opening_id_of_handle(h) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        let Some(obj) = clone.opening(id) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        let host = clone.wall(obj.host).ok_or_else(|| format!("opening o{} has no host wall", obj.id.0))?;
        let dir = (host.end - host.start).normalized();
        if !dir.is_finite() || dir == Vec2::ZERO {
            return Err(format!("opening o{} host wall is degenerate", obj.id.0));
        }
        let along = delta.dot(dir);
        let cross = delta.dot(dir.perp());
        if cross.abs() > 1e-6 * (1.0 + delta.len()) {
            return Err("opening motion must stay along the host wall".to_string());
        }
        let new_offset = obj.offset + along;
        let check_h = crate::opening::check_height(&clone, obj);
        let validated =
            crate::opening::validate_placement(&clone, obj.host, new_offset, obj.width, obj.sill, check_h, obj.kind, &obj.style, Some(obj.id))
                .map_err(|_| format!("opening o{} would leave wall w{}", obj.id.0, obj.host.0))?;
        if let Some(pos) = clone.openings.iter().position(|entry| entry.id == obj.id) {
            clone.openings[pos].offset = validated;
        }
        *model = clone;
        return Ok(());
    }
    if cadcraft_doc::arch::is_cabinet_handle(h) {
        let Some(id) = cabinet_id_of_handle(h) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        if let Some(pos) = clone.cabinets.iter().position(|entry| entry.id == id) {
            clone.cabinets[pos].position += delta;
        }
        *model = clone;
        return Ok(());
    }
    if cadcraft_doc::arch::is_fixture_handle(h) {
        let Some(id) = fixture_id_of_handle(h) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        if let Some(pos) = clone.fixtures.iter().position(|entry| entry.id == id) {
            clone.fixtures[pos].position += delta;
        }
        *model = clone;
        return Ok(());
    }
    if cadcraft_doc::arch::is_furnishing_handle(h) {
        let Some(id) = furnishing_id_of_handle(h) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        if let Some(pos) = clone.furnishings.iter().position(|entry| entry.id == id) {
            clone.furnishings[pos].at[0] += delta.x;
            clone.furnishings[pos].at[1] += delta.y;
        }
        *model = clone;
        return Ok(());
    }
    if cadcraft_doc::arch::is_arch_handle(h) {
        let Some(id) = wall_id_of_handle(h) else {
            return Err(format!("object {h:?} is not a known arch object"));
        };
        let _ = translate_wall(&mut clone, id, delta)?;
        *model = clone;
        return Ok(());
    }
    Err(format!("object {h:?} is not a known arch object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, Wall};

    fn wall(start: Vec2, end: Vec2) -> Wall {
        Wall::new(start, end, 0.2)
    }

    #[test]
    fn translate_wall_moves_both_ends_and_shared_corners() {
        let mut model = ArchModel::default();
        let a = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        let b = model.push_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0)));
        let _c = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(0.0, 5.0)));
        let moved = translate_wall(&mut model, a, Vec2::new(2.0, 0.0)).unwrap();
        assert!(moved.iter().any(|&id| id == a));
        assert!((model.wall(a).unwrap().start.x - 2.0).abs() < 1e-6);
        assert!((model.wall(a).unwrap().end.x - 12.0).abs() < 1e-6);
        assert!((model.wall(b).unwrap().start.x - 12.0).abs() < 1e-6);
    }

    #[test]
    fn translate_wall_slides_t_neighbours_along_their_own_axis() {
        let mut model = ArchModel::default();
        let a = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        let _b = model.push_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0)));
        let c = model.push_wall(wall(Vec2::new(10.0, 2.5), Vec2::new(15.0, 2.5)));
        translate_wall(&mut model, a, Vec2::new(2.0, 0.0)).unwrap();
        let cstart = model.wall(c).unwrap().start;
        let cend = model.wall(c).unwrap().end;
        assert!((cstart.x - 12.0).abs() < 1e-6 || (cstart.y - 2.5).abs() < 1e-6);
        assert!((cend.x - 17.0).abs() < 1e-6 || (cend.y - 2.5).abs() < 1e-6);
    }

    #[test]
    fn translate_wall_keeps_hosted_openings_offsets() {
        let mut model = ArchModel::default();
        let wall_id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        model.openings.push(cadcraft_doc::Opening { id: ArchId(0), host: wall_id, offset: 4.0, width: 1.0, height: 2.0, ..Default::default() });
        translate_wall(&mut model, wall_id, Vec2::new(2.0, 0.0)).unwrap();
        assert!((model.openings[0].offset - 6.0).abs() < 1e-6);
    }

    #[test]
    fn neighbour_openings_keep_their_world_position() {
        let mut model = ArchModel::default();
        let wall_id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        let neighbour = model.push_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0)));
        model.openings.push(cadcraft_doc::Opening { id: ArchId(0), host: neighbour, offset: 2.5, width: 1.0, height: 2.0, ..Default::default() });
        let before = model.clone();
        translate_wall(&mut model, wall_id, Vec2::new(2.0, 0.0)).unwrap();
        assert!((model.openings[0].offset - 3.063973079921356).abs() < 1e-6);
        assert_eq!(model.wall(neighbour).unwrap().start.x, 12.0);
        assert_eq!(before.openings[0].host, neighbour);
    }

    #[test]
    fn refuses_collapsing_neighbours_and_off_wall_openings() {
        let mut model = ArchModel::default();
        let a = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        let b = model.push_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(15.0, 0.0)));
        let before = model.clone();
        assert!(translate_wall(&mut model, a, Vec2::new(5.0, 0.0)).is_err());
        assert_eq!(model, before);
        model = ArchModel::default();
        let wall_id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        model.openings.push(cadcraft_doc::Opening { id: ArchId(0), host: wall_id, offset: 0.05, width: 5.0, height: 2.0, ..Default::default() });
        assert!(translate_wall(&mut model, wall_id, Vec2::new(0.0, 0.0)).is_err());
    }

    #[test]
    fn translate_object_moves_cabinets_fixtures_furnishings() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet { id: ArchId(0), position: Vec2::new(1.0, 2.0), ..Default::default() });
        model.fixtures.push(Fixture { id: ArchId(0), position: Vec2::new(2.0, 3.0), ..Default::default() });
        model.furnishings.push(Furnishing { id: ArchId(0), at: [3.0, 4.0], ..Default::default() });
        let handle = cadcraft_doc::arch::cabinet_handle(ArchId(0));
        translate_object(&mut model, handle, Vec2::new(1.0, 2.0)).unwrap();
        assert!((model.cabinets[0].position.x - 2.0).abs() < 1e-6);
        assert!((model.cabinets[0].position.y - 4.0).abs() < 1e-6);
    }

    #[test]
    fn opening_moves_only_along_its_wall() {
        let mut model = ArchModel::default();
        let id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        model.openings.push(Opening { id: ArchId(0), host: id, offset: 3.0, width: 1.0, height: 2.0, ..Default::default() });
        let handle = cadcraft_doc::arch::opening_handle(ArchId(0));
        assert!(translate_object(&mut model, handle, Vec2::new(0.0, 1.0)).is_err());
    }

    #[test]
    fn hostile_deltas_refused() {
        let mut model = ArchModel::default();
        let id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        assert!(translate_wall(&mut model, id, Vec2::new(f64::NAN, 0.0)).is_err());
        assert!(translate_wall(&mut model, id, Vec2::new(1e5, 0.0)).is_err());
    }

    #[test]
    fn a_refusal_leaves_the_model_unchanged() {
        let mut model = ArchModel::default();
        let id = model.push_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        let before = model.clone();
        let _ = translate_wall(&mut model, id, Vec2::new(1e5, 0.0));
        assert_eq!(model, before);
    }
}
