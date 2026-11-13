//! Cabinet-run fillers: a run can end with a narrow filler against a wall.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, CabinetKind, CornerPref, Fronts, MAX_FILLER_WIDTH};
use cadcraft_geom::Vec2;

const EPS: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CornerEnd {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunGap {
    pub end: CornerEnd,
    pub gap: Option<f64>,
    pub cabinet: ArchId,
}

fn axis_of(c: &Cabinet) -> Vec2 {
    let a = c.rotation.to_radians();
    Vec2::new(a.cos(), a.sin())
}

fn same_line(a: &Cabinet, b: &Cabinet) -> bool {
    if a.kind != b.kind || a.level != b.level || (a.rotation - b.rotation).abs() > EPS || (a.depth - b.depth).abs() > EPS {
        return false;
    }
    let d = b.position - a.position;
    let dir = axis_of(a);
    (d.x * dir.y - d.y * dir.x).abs() <= EPS
}

fn same_line_and_touching(a: &Cabinet, b: &Cabinet) -> bool {
    if !same_line(a, b) {
        return false;
    }
    let da = (b.position - a.position).dot(axis_of(a));
    let width = (a.width + b.width) / 2.0;
    (da.abs() - width).abs() <= MAX_FILLER_WIDTH + 1e-6
}

pub fn run_of(model: &ArchModel, id: ArchId) -> Vec<ArchId> {
    let Some(start) = model.cabinets.iter().find(|c| c.id == id) else { return Vec::new() };
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![id];
    seen.insert(id);
    while let Some(current) = stack.pop() {
        let Some(cur) = model.cabinets.iter().find(|c| c.id == current) else { continue };
        for c in &model.cabinets {
            if seen.contains(&c.id) || !same_line(cur, c) || !same_line_and_touching(cur, c) {
                continue;
            }
            seen.insert(c.id);
            stack.push(c.id);
        }
    }
    let mut ids: Vec<_> = seen.iter().copied().collect();
    ids.sort_by(|a, b| {
        let a0 = model.cabinets.iter().find(|c| c.id == *a).map(|c| c.position.dot(axis_of(c))).unwrap_or_default();
        let b0 = model.cabinets.iter().find(|c| c.id == *b).map(|c| c.position.dot(axis_of(c))).unwrap_or_default();
        a0.total_cmp(&b0).then_with(|| a.0.cmp(&b.0))
    });
    ids
}

fn wall_face_segments(w: &cadcraft_doc::arch::Wall) -> Vec<(Vec2, Vec2)> {
    if !w.start.is_finite() || !w.end.is_finite() || !w.thickness.is_finite() || w.thickness <= 0.0 {
        return Vec::new();
    }
    let len = (w.end - w.start).len();
    if len <= EPS {
        return Vec::new();
    }
    let dir = (w.end - w.start) / len;
    let normal = Vec2::new(-dir.y, dir.x);
    let off = normal * (w.thickness / 2.0);
    vec![(w.start + off, w.end + off), (w.start - off, w.end - off)]
}

fn nearest_wall_distance(origin: Vec2, dir: Vec2, model: &ArchModel, level: u32) -> Option<f64> {
    let mut best: Option<f64> = None;
    for wall in &model.walls {
        if wall.base_level != level || wall.divider || wall.thickness <= 0.0 || !(wall.start.is_finite() && wall.end.is_finite()) {
            continue;
        }
        for (a, b) in wall_face_segments(wall) {
            let v = b - a;
            let denom = dir.x * v.y - dir.y * v.x;
            if denom.abs() <= 1e-9 {
                continue;
            }
            let p = a - origin;
            let t = (p.x * v.y - p.y * v.x) / denom;
            let u = (p.x * dir.y - p.y * dir.x) / denom;
            if t > 0.0 && u >= -1e-9 && u <= 1.0 + 1e-9 && t <= 1.0 + 1e-9 {
                let d = t * dir.len();
                if d > 1e-6 && (best.is_none() || d < best.unwrap_or(f64::INFINITY)) {
                    best = Some(d);
                }
            }
        }
    }
    best
}

pub fn run_gaps(model: &ArchModel, id: ArchId) -> Result<Vec<RunGap>, String> {
    let run = run_of(model, id);
    if run.is_empty() {
        return Err(format!("unknown cabinet `{}`", cadcraft_doc::arch::cabinet_label(id)));
    }
    let mut out = Vec::new();
    for (idx, end) in ["start", "end"].iter().enumerate() {
        let c = model
            .cabinets
            .iter()
            .find(|c| c.id == run[if idx == 0 { 0 } else { run.len() - 1 }])
            .ok_or_else(|| format!("unknown cabinet `{}`", cadcraft_doc::arch::cabinet_label(id)))?;
        let axis = axis_of(c);
        let normal = Vec2::new(-axis.y, axis.x);
        let sign = if idx == 0 { -1.0 } else { 1.0 };
        let origin = c.position + axis * (sign * c.width / 2.0) + normal * (c.depth / 2.0);
        let dir = axis * sign;
        let gap = nearest_wall_distance(origin, dir, model, c.level);
        out.push(RunGap {
            end: if idx == 0 { CornerEnd::Start } else { CornerEnd::End },
            gap: gap.filter(|g| *g > 1e-6 && *g <= 1.0),
            cabinet: c.id,
        });
    }
    Ok(out)
}

pub fn filler_for(model: &ArchModel, g: &RunGap) -> Option<Cabinet> {
    let gap = match g.gap {
        Some(v) if v > 1e-6 && v <= MAX_FILLER_WIDTH + 1e-9 => v,
        _ => return None,
    };
    let c = model.cabinets.iter().find(|cab| cab.id == g.cabinet)?;
    let mut fill = c.clone();
    fill.id = ArchId(0);
    fill.filler = true;
    fill.width = gap;
    fill.fronts = Fronts::Open;
    fill.doors = 0;
    fill.drawers = 0;
    fill.face = None;
    fill.pull = None;
    fill.pull_symbol = None;
    fill.shelves = None;
    fill.corner = CornerPref::None;
    fill.crown = c.crown.clone();
    fill.light_rail = c.light_rail.clone();
    let axis = axis_of(c);
    let normal = Vec2::new(-axis.y, axis.x);
    let sign = match g.end {
        CornerEnd::Start => -1.0,
        CornerEnd::End => 1.0,
    };
    let offset = (c.width + gap) / 2.0;
    let pos = c.position + axis * (sign * offset);
    fill.position = pos + normal * (c.depth / 2.0) - normal * (fill.depth / 2.0);
    Some(fill)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::{CabinetKind, Fronts};
    use cadcraft_geom::Vec2;

    fn cab(id: u32, x: f64, width: f64) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(x, 0.5),
            rotation: 0.0,
            width,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Cabinet::default()
        }
    }

    #[test]
    fn run_of_touching_cabinets() {
        let mut m = ArchModel::default();
        m.push_cabinet(cab(0, 0.3, 0.6));
        m.push_cabinet(cab(1, 1.0, 0.6));
        let run = run_of(&m, ArchId(0));
        assert_eq!(run, vec![ArchId(0), ArchId(1)]);
    }

    #[test]
    fn gaps_to_the_nearest_wall_face_within_1_m() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        let w = Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2);
        m.walls.push(w);
        let c = cab(0, 0.45, 0.6);
        m.push_cabinet(c);
        let gaps = run_gaps(&m, ArchId(0)).unwrap();
        assert_eq!(gaps[0].end, CornerEnd::Start);
        assert!((gaps[0].gap.unwrap() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn already_flush_is_no_gap() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(Wall::new(Vec2::new(0.3, 0.0), Vec2::new(0.3, 2.0), 0.2));
        m.push_cabinet(cab(0, 0.3, 0.6));
        let gaps = run_gaps(&m, ArchId(0)).unwrap();
        assert!(gaps[0].gap.is_none() || gaps[0].gap.unwrap() <= 1e-6);
    }

    #[test]
    fn filler_cabinet_copies_the_end_cabinet() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
        let c = cab(0, 0.45, 0.6);
        m.push_cabinet(c.clone());
        let gap = run_gaps(&m, ArchId(0)).unwrap()[0].clone();
        let filler = filler_for(&m, &gap).unwrap();
        assert!(filler.filler);
        assert_eq!(filler.fronts, Fronts::Open);
        assert_eq!(filler.doors, 0);
        assert_eq!(filler.drawers, 0);
        assert!((filler.width - 0.05).abs() < 1e-9);
    }

    #[test]
    fn filler_for_output_validates() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
        let c = cab(0, 0.45, 0.6);
        m.push_cabinet(c);
        let gap = run_gaps(&m, ArchId(0)).unwrap()[0].clone();
        let filler = filler_for(&m, &gap).unwrap();
        assert!(crate::casework::validate_cabinet(&filler).is_ok());
    }

    #[test]
    fn wall_runs_fill_too() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
        m.push_cabinet(cab(0, 0.45, 0.6));
        m.push_cabinet(cab(1, 1.05, 0.6));
        let gaps = run_gaps(&m, ArchId(0)).unwrap();
        assert!(gaps.iter().any(|g| matches!(g.end, CornerEnd::Start) && g.gap.is_some_and(|v| (v - 0.05).abs() < 1e-9)));
    }

    #[test]
    fn hostile_fill_input() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
        m.push_cabinet(cab(0, 0.45, 0.6));
        let gap = RunGap { end: CornerEnd::Start, gap: Some(MAX_FILLER_WIDTH + 0.1), cabinet: ArchId(0) };
        assert!(filler_for(&m, &gap).is_none());
        let gap = RunGap { end: CornerEnd::Start, gap: Some(0.0), cabinet: ArchId(0) };
        assert!(filler_for(&m, &gap).is_none());
    }
}
